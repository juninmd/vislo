//! DPX (`.dpx`): the first image element, uncompressed, 8/10/16 bit, RGB, RGBA or luma. The transfer curve is not
//! applied, so log-encoded film scans look flat the way they do in any viewer that does not grade.

use image::{DynamicImage, RgbaImage};
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = || "not a valid DPX file".to_string();
    let little = match f.get(..4) {
        Some(b"SDPX") => false,
        Some(b"XPDX") => true,
        _ => return Err(bad()),
    };
    let u32at = |at: usize| f.get(at..at + 4).map(|b| if little { u32::from_le_bytes(b.try_into().unwrap()) } else { u32::from_be_bytes(b.try_into().unwrap()) });
    let u16at = |at: usize| f.get(at..at + 2).map(|b| if little { u16::from_le_bytes(b.try_into().unwrap()) } else { u16::from_be_bytes(b.try_into().unwrap()) });
    let (w, h) = (u32at(772).ok_or_else(bad)?, u32at(776).ok_or_else(bad)?);
    let (descriptor, depth, packing, encoding) = (*f.get(800).ok_or_else(bad)?, *f.get(803).ok_or_else(bad)?, u16at(804).ok_or_else(bad)?, u16at(806).ok_or_else(bad)?);
    let element = u32at(808).filter(|o| *o != 0).or(u32at(4)).ok_or_else(bad)? as usize;
    let eol = u32at(812).ok_or_else(bad)? as usize;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let channels = match descriptor {
        6 => 1,
        50 => 3,
        51 => 4,
        _ => return Err(format!("unsupported DPX layout (descriptor {descriptor})")),
    };
    if encoding != 0 || !matches!((depth, packing), (8, _) | (16, _) | (10, 1 | 2)) || (depth == 10 && channels != 3) {
        return Err(format!("unsupported DPX encoding ({depth}-bit, packing {packing}, encoding {encoding})"));
    }
    let row = match depth {
        10 => w as usize * 4,
        16 => w as usize * channels * 2,
        _ => w as usize * channels,
    };
    let stride = row.checked_add(eol).ok_or_else(bad)?;
    let data = f.get(element..element.checked_add(stride.checked_mul(h as usize).ok_or_else(bad)?).ok_or_else(bad)?).ok_or("truncated DPX data")?;
    let mut img = RgbaImage::new(w, h);
    for (y, line) in data.chunks(stride).enumerate() {
        for x in 0..w as usize {
            let sample = |c: usize| -> u8 {
                match depth {
                    8 => line[x * channels + c],
                    16 => line[(x * channels + c) * 2 + usize::from(little)], // the high byte
                    _ => {
                        let b: [u8; 4] = line[x * 4..x * 4 + 4].try_into().unwrap();
                        let word = if little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) };
                        let top = if packing == 1 { 22 } else { 20 }; // method B leaves the high bits unused instead of the low
                        (((word >> (top - 10 * c as u32)) & 0x3ff) >> 2) as u8
                    }
                }
            };
            let px = match channels {
                1 => [sample(0), sample(0), sample(0), 255],
                3 => [sample(0), sample(1), sample(2), 255],
                _ => [sample(0), sample(1), sample(2), sample(3)],
            };
            img.put_pixel(x as u32, y as u32, image::Rgba(px));
        }
    }
    Ok(DynamicImage::ImageRgba8(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn file(little: bool, w: u32, h: u32, descriptor: u8, depth: u8, packing: u16, data: &[u8]) -> Vec<u8> {
        let mut f = vec![0u8; 2048];
        f[..4].copy_from_slice(if little { b"XPDX" } else { b"SDPX" });
        let put32 = |f: &mut Vec<u8>, at: usize, v: u32| f[at..at + 4].copy_from_slice(&if little { v.to_le_bytes() } else { v.to_be_bytes() });
        put32(&mut f, 4, 2048);
        put32(&mut f, 772, w);
        put32(&mut f, 776, h);
        f[800] = descriptor;
        f[803] = depth;
        f[804..806].copy_from_slice(&if little { packing.to_le_bytes() } else { packing.to_be_bytes() });
        f.extend(data);
        f
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-dpx-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.dpx"), bytes).unwrap();
        decode(&d.join("a.dpx"), max)
    }

    #[test]
    fn eight_and_sixteen_bit_rgb_decode_in_both_byte_orders() {
        let img = run(&file(false, 1, 1, 50, 8, 0, &[10, 20, 30]), 10).unwrap();
        assert_eq!(img.to_rgb8().get_pixel(0, 0).0, [10, 20, 30]);
        let be16 = run(&file(false, 1, 1, 50, 16, 0, &[0x80, 0, 0x40, 0, 0xff, 0xff]), 10).unwrap();
        assert_eq!(be16.to_rgb8().get_pixel(0, 0).0, [0x80, 0x40, 0xff]);
        let le16 = run(&file(true, 1, 1, 50, 16, 0, &[0, 0x80, 0, 0x40, 0xff, 0xff]), 10).unwrap();
        assert_eq!(le16.to_rgb8().get_pixel(0, 0).0, [0x80, 0x40, 0xff]);
    }

    #[test]
    fn ten_bit_words_unpack_by_method() {
        // R=1023, G=512, B=0 in method A (low two bits are padding) and in method B (high two bits are padding).
        let a = (1023u32 << 22) | (512 << 12);
        let b = (1023u32 << 20) | (512 << 10);
        let px = run(&file(false, 1, 1, 50, 10, 1, &a.to_be_bytes()), 10).unwrap().to_rgb8().get_pixel(0, 0).0;
        assert_eq!(px, [255, 128, 0]);
        let px = run(&file(true, 1, 1, 50, 10, 2, &b.to_le_bytes()), 10).unwrap().to_rgb8().get_pixel(0, 0).0;
        assert_eq!(px, [255, 128, 0]);
    }

    #[test]
    fn luma_and_rgba_layouts() {
        let g = run(&file(false, 2, 1, 6, 8, 0, &[7, 9]), 10).unwrap();
        assert_eq!((g.dimensions(), g.to_rgb8().get_pixel(1, 0).0), ((2, 1), [9, 9, 9]));
        let a = run(&file(false, 1, 1, 51, 8, 0, &[1, 2, 3, 4]), 10).unwrap();
        assert_eq!(a.to_rgba8().get_pixel(0, 0).0, [1, 2, 3, 4]);
    }

    #[test]
    fn junk_unsupported_truncated_and_huge_are_errors() {
        assert!(run(b"not a dpx file", 10).is_err());
        assert!(run(&file(false, 4, 4, 50, 8, 0, &[0; 5]), 100).is_err());
        assert!(run(&file(false, 1, 1, 50, 12, 0, &[0; 8]), 10).unwrap_err().contains("unsupported"));
        assert_eq!(run(&file(false, 60_000, 60_000, 50, 8, 0, &[]), 1_000).unwrap_err(), crate::decode::TOO_LARGE);
    }
}
