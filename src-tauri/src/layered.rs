//! OpenRaster (`.ora`) and Krita (`.kra`) documents: ZIP containers whose `mergedimage.png` is the flattened picture.
//! Only that one entry is located through the central directory and inflated in memory with a hard size cap;
//! nothing is extracted to disk, so ZIP path and symlink tricks have nowhere to land.

use flate2::read::DeflateDecoder;
use image::{DynamicImage, ImageDecoder};
use std::io::Read;
use std::path::Path;

const ENTRY: &str = "mergedimage.png";
const MAX_FILE: u64 = 512 << 20;
const MAX_PNG: u64 = 256 << 20;

fn u16le(b: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?) as usize)
}

fn u32le(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize)
}

/// The packed bytes of `mergedimage.png` and whether they are deflated (else stored).
fn find_entry(zip: &[u8]) -> Result<(&[u8], bool), String> {
    let bad = || "not a valid OpenRaster/Krita archive".to_string();
    let first = zip.len().saturating_sub(22 + 65_535);
    let eocd = (first..zip.len().saturating_sub(21))
        .rev()
        .find(|&i| zip[i..].starts_with(b"PK\x05\x06"))
        .ok_or_else(bad)?;
    let (count, mut at) = (u16le(zip, eocd + 10).ok_or_else(bad)?, u32le(zip, eocd + 16).ok_or_else(bad)?);
    for _ in 0..count {
        if zip.get(at..at + 4) != Some(b"PK\x01\x02") {
            return Err(bad());
        }
        let fields = (
            u16le(zip, at + 10),
            u32le(zip, at + 20),
            u16le(zip, at + 28),
            u16le(zip, at + 30),
            u16le(zip, at + 32),
            u32le(zip, at + 42),
        );
        let (Some(method), Some(csize), Some(nlen), Some(elen), Some(clen), Some(local)) = fields else {
            return Err(bad());
        };
        if zip.get(at + 46..at + 46 + nlen) == Some(ENTRY.as_bytes()) {
            if zip.get(local..local + 4) != Some(b"PK\x03\x04") || !matches!(method, 0 | 8) || csize == 0xFFFF_FFFF {
                return Err(bad());
            }
            let start = local + 30 + u16le(zip, local + 26).ok_or_else(bad)? + u16le(zip, local + 28).ok_or_else(bad)?;
            let end = start.checked_add(csize).ok_or_else(bad)?;
            return Ok((zip.get(start..end).ok_or_else(bad)?, method == 8));
        }
        at += 46 + nlen + elen + clen;
    }
    Err(format!("no {ENTRY} inside (save the document with a preview image)"))
}

/// Reads at most `cap` bytes; a longer stream is refused, so a deflate bomb never gets past `cap` in memory.
fn read_capped(reader: impl Read, cap: u64) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    reader.take(cap + 1).read_to_end(&mut out).map_err(|e| e.to_string())?;
    if out.len() as u64 > cap {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    Ok(out)
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_with(path, max_pixels, crate::decode::MAX_ALLOC)
}

fn decode_with(path: &Path, max_pixels: u64, max_alloc: u64) -> Result<DynamicImage, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let zip = read_capped(file, MAX_FILE)?;
    let (data, deflated) = find_entry(&zip)?;
    let png = if deflated {
        std::borrow::Cow::Owned(read_capped(DeflateDecoder::new(data), MAX_PNG)?)
    } else if data.len() as u64 > MAX_PNG {
        return Err(crate::decode::TOO_LARGE.to_string());
    } else {
        std::borrow::Cow::Borrowed(data)
    };
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(&*png)).map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    // The PNG decoder itself runs without limits, and a 16-bit RGBA canvas needs 4x the bytes of its pixel count.
    if u64::from(w) * u64::from(h) > max_pixels || decoder.total_bytes() > max_alloc {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;
    use std::io::Write;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([1, 2, 3, 255])))
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    /// A ZIP of `(name, bytes, deflate?)` entries; the CRC is left zero, which this reader never checks.
    fn zip(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let (mut out, mut central) = (Vec::new(), Vec::new());
        for (name, data, deflate) in entries {
            let packed = if *deflate {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            let (offset, method) = (out.len() as u32, if *deflate { 8u16 } else { 0 });
            let common = [
                20u16.to_le_bytes().as_slice(),
                &[0; 2],
                &method.to_le_bytes(),
                &[0; 8],
                &(packed.len() as u32).to_le_bytes(),
                &(data.len() as u32).to_le_bytes(),
                &(name.len() as u16).to_le_bytes(),
                &[0; 2],
            ]
            .concat();
            out.extend([b"PK\x03\x04".as_slice(), &common, name.as_bytes(), &packed].concat());
            central.extend([b"PK\x01\x02\x14\x00".as_slice(), &common, &[0; 10], &offset.to_le_bytes(), name.as_bytes()].concat());
        }
        let (n, size, at) = (entries.len() as u16, central.len() as u32, out.len() as u32);
        out.extend(&central);
        out.extend([b"PK\x05\x06\0\0\0\0".as_slice(), &n.to_le_bytes(), &n.to_le_bytes(), &size.to_le_bytes(), &at.to_le_bytes(), &[0; 2]].concat());
        out
    }

    fn file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-layered-{tag}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.ora"), bytes).unwrap();
        d.join("a.ora")
    }

    #[test]
    fn merged_image_is_read_stored_or_deflated_and_other_entries_are_ignored() {
        for deflate in [false, true] {
            let (layer, merged) = (png(2, 2), png(6, 4));
            let z = zip(&[("mimetype", b"image/openraster", false), ("data/layer.png", &layer, deflate), ("mergedimage.png", &merged, deflate)]);
            let img = decode(&file("ok", &z), 1_000).unwrap();
            assert_eq!(img.dimensions(), (6, 4), "deflate={deflate}");
            assert_eq!(img.to_rgb8().get_pixel(0, 0).0, [1, 2, 3]);
        }
    }

    #[test]
    fn archives_without_a_merged_image_or_with_junk_are_errors() {
        let no_merge = zip(&[("stack.xml", b"<image/>", false)]);
        assert!(decode(&file("nomerge", &no_merge), 1_000).unwrap_err().contains("mergedimage.png"));
        assert!(decode(&file("junk", b"PK-not-really-a-zip"), 1_000).is_err());
        let z = zip(&[("mergedimage.png", &png(6, 4), true)]);
        assert!(decode(&file("cut", &z[..z.len() / 2]), 1_000).is_err());
    }

    #[test]
    fn oversized_merged_images_are_refused_before_decoding_pixels() {
        let z = zip(&[("mergedimage.png", &png(40, 40), true)]);
        assert_eq!(decode(&file("big", &z), 1_000).unwrap_err(), crate::decode::TOO_LARGE);
    }

    #[test]
    fn reading_stops_at_the_cap_even_from_an_endless_stream() {
        assert_eq!(read_capped(std::io::repeat(0), 100).unwrap_err(), crate::decode::TOO_LARGE);
        assert_eq!(read_capped(&[7u8; 100][..], 100).unwrap().len(), 100);
    }

    #[test]
    fn a_picture_that_fits_the_pixel_ceiling_but_not_the_allocator_is_refused() {
        let z = zip(&[("mergedimage.png", &png(40, 40), true)]); // 6 400 bytes decoded
        assert_eq!(decode_with(&file("alloc", &z), u64::MAX, 1_000).unwrap_err(), crate::decode::TOO_LARGE);
        assert!(decode_with(&file("alloc-ok", &z), u64::MAX, 10_000).is_ok());
    }
}
