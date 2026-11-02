//! KTX 1 and KTX 2 textures (`.ktx`, `.ktx2`): the first image of the first level, in BC1-BC5, BC7 or 8-bit RGBA/BGRA
//! (KTX 2 also R8). KTX 2 files with supercompression, Basis Universal, ETC, ASTC, PVRTC and BC6H are refused.
//! Orientation follows the `KTXorientation` key: KTX 1 data is bottom-up (OpenGL order) unless the key says `T=d`,
//! KTX 2 data is top-down unless the key says `ru`. BC1 without alpha (VK 131/132, GL 0x83F0/0x8C4C) is opaque.

use crate::ddsbc::{decode_pixels, Kind};
use image::DynamicImage;
use std::path::Path;

const KTX1_ID: &[u8] = b"\xABKTX 11\xBB\r\n\x1A\n";
const KTX2_ID: &[u8] = b"\xABKTX 20\xBB\r\n\x1A\n";

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    match f.get(..12) {
        Some(KTX1_ID) => v1(f, max_pixels),
        Some(KTX2_ID) => v2(f, max_pixels),
        _ => Err("not a valid KTX file".into()),
    }
}

fn u32at(f: &[u8], at: usize) -> Option<usize> {
    f.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
}

fn u64at(f: &[u8], at: usize) -> Option<usize> {
    let b = f.get(at..at + 8)?;
    usize::try_from(u64::from_le_bytes(b.try_into().ok()?)).ok()
}

/// The value of the `KTXorientation` entry, when there is one.
fn orientation(kv: &[u8]) -> Option<&[u8]> {
    let mut at = 0;
    while let Some(size) = u32at(kv, at) {
        let entry = kv.get(at + 4..at + 4 + size)?;
        if let Some(value) = entry.strip_prefix(b"KTXorientation\0") {
            return Some(value);
        }
        at += (4 + size).div_ceil(4) * 4;
    }
    None
}

/// KTX 1 writes `S=r,T=d`; true when its `T=` says the rows run bottom to top (or is absent, the OpenGL order).
fn v1_bottom_up(kv: &[u8]) -> bool {
    let t = orientation(kv).and_then(|v| v.windows(2).position(|w| w == b"T=").and_then(|i| v.get(i + 2).copied()));
    t != Some(b'd')
}

/// KTX 2 writes `rd` or `ru` (optionally followed by `o`/`i`): the second letter is the vertical direction.
fn v2_bottom_up(kv: &[u8]) -> bool {
    orientation(kv).and_then(|v| v.get(1).copied()) == Some(b'u')
}

fn finish(img: DynamicImage, flip: bool) -> DynamicImage {
    if flip { DynamicImage::ImageRgba8(image::imageops::flip_vertical(&img.to_rgba8())) } else { img }
}

fn v1(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let bad = || "truncated KTX file".to_string();
    if u32at(f, 12) != Some(0x0403_0201) {
        return Err("big-endian KTX files are not supported".into());
    }
    let (gl_type, gl_format, internal) = (u32at(f, 16).ok_or_else(bad)?, u32at(f, 24).ok_or_else(bad)?, u32at(f, 28).ok_or_else(bad)?);
    let (w, h) = (u32at(f, 36).ok_or_else(bad)?, u32at(f, 40).ok_or_else(bad)?.max(1));
    let kv_len = u32at(f, 60).ok_or_else(bad)?;
    let kind = match (internal, gl_type, gl_format) {
        (0x83F0 | 0x8C4C, 0, 0) => Kind::Bc1Opaque,
        (0x83F1 | 0x8C4D, 0, 0) => Kind::Bc1,
        (0x83F2 | 0x8C4E, 0, 0) => Kind::Bc2,
        (0x83F3 | 0x8C4F, 0, 0) => Kind::Bc3,
        (0x8DBB, 0, 0) => Kind::Bc4,
        (0x8DBD, 0, 0) => Kind::Bc5,
        (0x8E8C | 0x8E8D, 0, 0) => Kind::Bc7,
        (0x8E8E | 0x8E8F, 0, 0) => return Err("unsupported KTX format (BC6H)".into()),
        (_, 0x1401, 0x1908) => Kind::Rgba,
        (_, 0x1401, 0x80E1) => Kind::Bgra,
        _ => return Err(format!("unsupported KTX format (GL internal format {internal:#x})")),
    };
    let kv = f.get(64..64usize.checked_add(kv_len).ok_or_else(bad)?).ok_or_else(bad)?;
    let start = 64 + kv_len;
    let size = u32at(f, start).ok_or_else(bad)?;
    let payload = f.get(start + 4..(start + 4).checked_add(size).ok_or_else(bad)?).ok_or_else(bad)?;
    let img = decode_pixels(kind, payload, w, h, max_pixels)?;
    Ok(finish(img, v1_bottom_up(kv)))
}

fn v2(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let bad = || "truncated KTX file".to_string();
    let (vk, w, h) = (u32at(f, 12).ok_or_else(bad)?, u32at(f, 20).ok_or_else(bad)?, u32at(f, 24).ok_or_else(bad)?.max(1));
    match u32at(f, 44).ok_or_else(bad)? {
        0 => {}
        n => return Err(format!("unsupported KTX2 supercompression ({})", ["none", "BasisLZ", "Zstandard", "zlib"].get(n).unwrap_or(&"unknown"))),
    }
    let kind = match vk {
        37 | 43 => Kind::Rgba,
        44 | 50 => Kind::Bgra,
        9 | 15 => Kind::Grey,
        131 | 132 => Kind::Bc1Opaque,
        133 | 134 => Kind::Bc1,
        135 | 136 => Kind::Bc2,
        137 | 138 => Kind::Bc3,
        139 => Kind::Bc4,
        141 => Kind::Bc5,
        145 | 146 => Kind::Bc7,
        143 | 144 => return Err("unsupported KTX2 format (BC6H)".into()),
        0 => return Err("unsupported KTX2 format (Basis Universal)".into()),
        n => return Err(format!("unsupported KTX2 format (VkFormat {n})")),
    };
    let (offset, length) = (u64at(f, 80).ok_or_else(bad)?, u64at(f, 88).ok_or_else(bad)?);
    let payload = f.get(offset..offset.checked_add(length).ok_or_else(bad)?).ok_or_else(bad)?;
    let (kv_at, kv_len) = (u32at(f, 56).ok_or_else(bad)?, u32at(f, 60).ok_or_else(bad)?);
    let kv = f.get(kv_at..kv_at.saturating_add(kv_len)).unwrap_or(&[]);
    let img = decode_pixels(kind, payload, w, h, max_pixels)?;
    Ok(finish(img, v2_bottom_up(kv)))
}
