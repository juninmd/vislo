//! MNG (`.mng`): the first embedded PNG image object (IHDR..IEND), shown like the first frame of an APNG.
//! Positioning, delta and animation chunks are ignored, and a JNG-only first image is reported as unsupported.

use image::{DynamicImage, ImageDecoder};
use std::path::Path;

const MNG: &[u8] = b"\x8aMNG\r\n\x1a\n";
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = || "not a valid MNG file".to_string();
    if !f.starts_with(MNG) {
        return Err(bad());
    }
    let (mut at, mut start) = (MNG.len(), None);
    let png = loop {
        let len = f.get(at..at + 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()) as usize).ok_or("no PNG image inside this MNG")?;
        let kind = f.get(at + 4..at + 8).ok_or_else(bad)?;
        let next = at.checked_add(12).and_then(|a| a.checked_add(len)).filter(|n| *n <= f.len()).ok_or_else(bad)?;
        match kind {
            b"IHDR" if start.is_none() => start = Some(at),
            b"JHDR" if start.is_none() => return Err("this MNG starts with a JNG image, which is not supported".to_string()),
            b"IEND" if start.is_some() => break [PNG, &f[start.unwrap()..next]].concat(),
            b"MEND" => return Err("no PNG image inside this MNG".to_string()),
            _ => {}
        }
        at = next;
    };
    let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(&png)).map_err(|e| e.to_string())?;
    let (w, h) = decoder.dimensions();
    if u64::from(w) * u64::from(h) > max_pixels || decoder.total_bytes() > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba([4, 5, 6, 255]))).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        [&(data.len() as u32).to_be_bytes()[..], kind, data, &[0; 4]].concat() // the CRC is not checked here
    }

    fn mng(parts: &[Vec<u8>]) -> Vec<u8> {
        [MNG.to_vec(), chunk(b"MHDR", &[0; 28])].into_iter().chain(parts.iter().cloned()).collect::<Vec<_>>().concat()
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-mng-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.mng"), bytes).unwrap();
        decode(&d.join("a.mng"), max)
    }

    /// The chunks of a PNG file without its signature.
    fn chunks_of(png: &[u8]) -> Vec<u8> {
        png[8..].to_vec()
    }

    #[test]
    fn first_png_object_is_shown_and_animation_chunks_around_it_are_skipped() {
        let (first, second) = (png(6, 4), png(2, 2));
        let file = mng(&[chunk(b"FRAM", &[1]), chunks_of(&first), chunk(b"FRAM", &[1]), chunks_of(&second), chunk(b"MEND", &[])]);
        let img = run(&file, 100).unwrap();
        assert_eq!(img.dimensions(), (6, 4));
        assert_eq!(img.to_rgb8().get_pixel(0, 0).0, [4, 5, 6]);
    }

    #[test]
    fn junk_jng_empty_truncated_and_huge_files_are_errors() {
        assert!(run(b"not an mng", 100).is_err());
        assert!(run(&mng(&[chunk(b"MEND", &[])]), 100).unwrap_err().contains("no PNG"));
        assert!(run(&mng(&[chunk(b"JHDR", &[0; 16])]), 100).unwrap_err().contains("JNG"));
        let file = mng(&[chunks_of(&png(6, 4))]);
        assert!(run(&file[..file.len() - 20], 100).is_err(), "cut inside the image");
        assert_eq!(run(&mng(&[chunks_of(&png(40, 40)), chunk(b"MEND", &[])]), 100).unwrap_err(), crate::decode::TOO_LARGE);
    }
}
