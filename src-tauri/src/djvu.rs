//! DjVu (`.djvu`, `.djv`): the first page of a single- or multi-page document, composited from its IW44 background,
//! JB2/G4 mask and foreground by the pure-Rust `djvu-rs` crate. Text, annotations and the other pages are ignored.

use image::{DynamicImage, RgbImage};
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: Vec<u8>, max_pixels: u64) -> Result<DynamicImage, String> {
    if f.get(..4) != Some(b"AT&T") {
        return Err("not a valid DjVu file".into());
    }
    let doc = djvu_rs::Document::from_bytes(f).map_err(|e| e.to_string())?;
    let page = doc.page(0).map_err(|e| e.to_string())?;
    let px = u64::from(page.width()) * u64::from(page.height());
    if px == 0 || px > max_pixels || px * 8 > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let pm = page.render().map_err(|e| e.to_string())?;
    RgbImage::from_raw(pm.width, pm.height, pm.to_rgb()).map(DynamicImage::ImageRgb8).ok_or_else(|| "corrupt DjVu page".to_string())
}

