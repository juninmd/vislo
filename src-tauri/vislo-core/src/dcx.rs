//! DCX (`.dcx`): a multi-page PCX container. The first page is a plain PCX stream, handed to the PCX decoder.

use image::{DynamicImage, ImageDecoder};
use image_extras::pcx::PCXDecoder;
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    if f.get(..4) != Some(&[0xB1, 0x68, 0xDE, 0x3A]) {
        return Err("not a valid DCX file".into());
    }
    let first = f.get(4..8).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).filter(|o| *o != 0).ok_or("this DCX file has no pages")?;
    // The PCX decoder reads a 256-colour palette from the end of its input, so the slice must stop at this page.
    let end = f.get(8..12).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).filter(|e| *e > first && *e <= f.len()).unwrap_or(f.len());
    let page = f.get(first..end).ok_or("truncated DCX file")?;
    let dec = PCXDecoder::new(std::io::Cursor::new(page)).map_err(|e| e.to_string())?;
    let (w, h) = dec.dimensions();
    if u64::from(w) * u64::from(h) > max_pixels || dec.total_bytes() > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    DynamicImage::from_decoder(dec).map_err(|e| e.to_string())
}
