//! Windows animated cursors (`.ani`): a RIFF/ACON file whose `fram` list holds one ICO/CUR image per frame. The first
//! frame is shown; the sequence and rate chunks are ignored.

use image::DynamicImage;
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

/// The data of the first `icon` chunk inside the `fram` list.
fn first_frame(f: &[u8]) -> Option<&[u8]> {
    let le32 = |at: usize| f.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let (mut at, end) = (12usize, f.len());
    while at + 8 <= end {
        let (kind, len) = (&f[at..at + 4], le32(at + 4)?);
        let body = at + 8;
        if kind == b"LIST" && f.get(body..body + 4) == Some(b"fram") {
            let (mut c, stop) = (body + 4, body.saturating_add(len).min(end));
            while c + 8 <= stop {
                let clen = le32(c + 4)?;
                if &f[c..c + 4] == b"icon" {
                    return f.get(c + 8..(c + 8).checked_add(clen)?);
                }
                c = c.checked_add(8)?.checked_add(clen)?.checked_add(clen & 1)?;
            }
            return None;
        }
        at = body.checked_add(len)?.checked_add(len & 1)?;
    }
    None
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    if f.get(..4) != Some(b"RIFF") || f.get(8..12) != Some(b"ACON") {
        return Err("not a valid ANI file".into());
    }
    let icon = first_frame(f).ok_or("this ANI file has no frames")?;
    let img = image::load_from_memory_with_format(icon, image::ImageFormat::Ico).map_err(|e| e.to_string())?;
    if u64::from(img.width()) * u64::from(img.height()) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    Ok(img)
}

