//! Blizzard textures (`.blp`): BLP1 and BLP2, the first (largest) mip level. Palettised (with 0/1/4/8-bit alpha),
//! DXT1/3/5 and uncompressed BGRA are read. BLP1's JPEG form is not supported.

use image::{DynamicImage, RgbaImage};
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

fn rgb565(v: u16) -> [u8; 3] {
    let (r, g, b) = ((v >> 11) as u8 & 31, (v >> 5) as u8 & 63, v as u8 & 31);
    [r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2] // replicate the top bits so 31 -> 255
}

/// The 16 pixels of one DXT colour block (its 8 bytes). `dxt1` allows the three-colour mode, with transparency when
/// `one_bit_alpha`; DXT3/5 always use four colours.
fn colour_block(c: &[u8], dxt1: bool, one_bit_alpha: bool) -> [[u8; 4]; 16] {
    let (c0, c1) = (u16::from_le_bytes([c[0], c[1]]), u16::from_le_bytes([c[2], c[3]]));
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |p: u32, q: u32| [0, 1, 2].map(|i| ((u32::from(a[i]) * p + u32::from(b[i]) * q) / (p + q)) as u8);
    let full = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    // In DXT1, c0 <= c1 selects the three-colour mode whatever the header says; index 3 is then black, clear only when alpha is in use.
    let pal = if c0 > c1 || !dxt1 { [full(a), full(b), full(mix(2, 1)), full(mix(1, 2))] } else { [full(a), full(b), full(mix(1, 1)), [0, 0, 0, if one_bit_alpha { 0 } else { 255 }]] };
    let idx = u32::from_le_bytes([c[4], c[5], c[6], c[7]]);
    std::array::from_fn(|i| pal[(idx >> (2 * i)) as usize & 3])
}

/// `kind` is 1, 3 or 5 for DXT1, DXT3 and DXT5.
fn dxt(data: &[u8], w: usize, h: usize, kind: u8, one_bit_alpha: bool) -> Result<RgbaImage, String> {
    let step = if kind == 1 { 8 } else { 16 };
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let blocks = data.get(..bw * bh * step).ok_or("truncated BLP data")?;
    let mut img = RgbaImage::new(w as u32, h as u32);
    for (n, blk) in blocks.chunks_exact(step).enumerate() {
        let (bx, by) = (n % bw, n / bw);
        let px = colour_block(&blk[step - 8..], kind == 1, one_bit_alpha);
        for (i, p) in px.iter().enumerate() {
            let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
            if x >= w || y >= h {
                continue;
            }
            let a = match kind {
                3 => (blk[i / 2] >> (4 * (i % 2)) & 15) * 17,
                5 => {
                    let (a0, a1) = (u16::from(blk[0]), u16::from(blk[1]));
                    let bits = u64::from_le_bytes([blk[2], blk[3], blk[4], blk[5], blk[6], blk[7], 0, 0]);
                    let code = ((bits >> (3 * i)) & 7) as u16;
                    (match code {
                        0 => a0,
                        1 => a1,
                        c if a0 > a1 => ((8 - c) * a0 + (c - 1) * a1) / 7,
                        6 => 0,
                        7 => 255,
                        c => ((6 - c) * a0 + (c - 1) * a1) / 5,
                    }) as u8
                }
                _ => p[3],
            };
            img.put_pixel(x as u32, y as u32, image::Rgba([p[0], p[1], p[2], a]));
        }
    }
    Ok(img)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let bad = || "not a valid BLP file".to_string();
    let u32at = |at: usize| f.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let v2 = match f.get(..4) {
        Some(b"BLP2") => true,
        Some(b"BLP1") => false,
        _ => return Err(bad()),
    };
    // BLP2: type, encoding, alpha depth, alpha encoding, mips, size, offsets, lengths, palette.
    // BLP1: content, alpha depth, size, extra, mips, offsets, lengths, palette.
    let (enc, alpha_depth, alpha_enc, w, h, off_at, hdr) = if v2 {
        if u32at(4) != Some(1) {
            return Err("unsupported BLP2 type".into());
        }
        (*f.get(8).ok_or_else(bad)?, *f.get(9).ok_or_else(bad)?, *f.get(10).ok_or_else(bad)?, u32at(12).ok_or_else(bad)?, u32at(16).ok_or_else(bad)?, 20, 148)
    } else {
        if u32at(4) != Some(1) {
            return Err("BLP1 files with JPEG data are not supported".into());
        }
        (1, u32at(8).ok_or_else(bad)? as u8, 0, u32at(12).ok_or_else(bad)?, u32at(16).ok_or_else(bad)?, 28, 156)
    };
    if w == 0 || h == 0 {
        return Err(bad());
    }
    if (w as u64).saturating_mul(h as u64) > max_pixels || (w as u64) * (h as u64) * 8 > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let (off, len) = (u32at(off_at).ok_or_else(bad)?, u32at(off_at + 64).ok_or_else(bad)?);
    let data = f.get(off..off.checked_add(len).ok_or_else(bad)?).ok_or("truncated BLP file")?;
    let img = match enc {
        1 => {
            let pal = f.get(hdr..hdr + 1024).ok_or("truncated BLP file")?;
            let idx = data.get(..w * h).ok_or("truncated BLP data")?;
            let alpha = &data[w * h..];
            let a = |i: usize| -> u8 {
                match alpha_depth {
                    8 => alpha.get(i).copied().unwrap_or(255),
                    4 => alpha.get(i / 2).map_or(255, |b| (b >> (4 * (i % 2)) & 15) * 17),
                    1 => alpha.get(i / 8).map_or(255, |b| if b >> (i % 8) & 1 == 1 { 255 } else { 0 }),
                    _ => 255,
                }
            };
            let mut img = RgbaImage::new(w as u32, h as u32);
            for (i, p) in idx.iter().enumerate() {
                let c = &pal[usize::from(*p) * 4..][..4];
                img.put_pixel((i % w) as u32, (i / w) as u32, image::Rgba([c[2], c[1], c[0], a(i)]));
            }
            img
        }
        2 if v2 => dxt(data, w, h, match alpha_enc { 1 => 3, 7 => 5, _ => 1 }, alpha_depth > 0)?,
        3 if v2 => {
            let px = data.get(..w * h * 4).ok_or("truncated BLP data")?;
            RgbaImage::from_fn(w as u32, h as u32, |x, y| {
                let p = &px[(y as usize * w + x as usize) * 4..][..4];
                image::Rgba([p[2], p[1], p[0], p[3]])
            })
        }
        _ => return Err("unsupported BLP encoding".into()),
    };
    Ok(DynamicImage::ImageRgba8(img))
}
