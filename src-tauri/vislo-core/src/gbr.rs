//! GIMP brushes (`.gbr`): version 1 or 2, a grey mask (1 byte per pixel) or colour with alpha (4 bytes).

use image::{DynamicImage, GrayImage, RgbaImage};
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let be32 = |at: usize| f.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let bad = || "not a valid GIMP brush".to_string();
    let (header, version, w, h, bytes) = (be32(0).ok_or_else(bad)?, be32(4).ok_or_else(bad)?, be32(8).ok_or_else(bad)?, be32(12).ok_or_else(bad)?, be32(16).ok_or_else(bad)?);
    if !(1..=2).contains(&version) || header < 20 || (version == 2 && f.get(20..24) != Some(b"GIMP")) || !matches!(bytes, 1 | 4) || w == 0 || h == 0 {
        return Err(bad());
    }
    if (w as u64).saturating_mul(h as u64) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let data = f.get(header..header.checked_add(w * h * bytes).ok_or_else(bad)?).ok_or("truncated GIMP brush")?;
    Ok(if bytes == 1 {
        DynamicImage::ImageLuma8(GrayImage::from_raw(w as u32, h as u32, data.to_vec()).ok_or_else(bad)?)
    } else {
        DynamicImage::ImageRgba8(RgbaImage::from_raw(w as u32, h as u32, data.to_vec()).ok_or_else(bad)?)
    })
}
