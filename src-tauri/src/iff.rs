//! Amiga IFF pictures (`.iff`, `.lbm`, `.ilbm`): ILBM (bitplanes) and PBM (chunky), with ByteRun1 or raw rows, up to
//! 8 planes through the palette, Extra-Half-Brite, HAM6/HAM8 and 24-bit planes. A mask plane is skipped (opaque).

use image::{DynamicImage, RgbImage};
use std::path::Path;

struct Bmhd {
    w: usize,
    h: usize,
    planes: usize,
    masking: u8,
    compression: u8,
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

fn byte_run(src: &[u8], at: &mut usize, len: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let n = *src.get(*at).ok_or("truncated IFF body")? as i8;
        *at += 1;
        if n >= 0 {
            let c = n as usize + 1;
            out.extend_from_slice(src.get(*at..*at + c).ok_or("truncated IFF body")?);
            *at += c;
        } else if n != -128 {
            let b = *src.get(*at).ok_or("truncated IFF body")?;
            *at += 1;
            out.resize(out.len() + 1 + (-i32::from(n)) as usize, b);
        }
    }
    out.truncate(len); // a run may spill over the row end; the row is what counts
    Ok(out)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let pbm = match f.get(..12) {
        Some(h) if &h[..4] == b"FORM" && &h[8..12] == b"ILBM" => false,
        Some(h) if &h[..4] == b"FORM" && &h[8..12] == b"PBM " => true,
        _ => return Err("not a valid IFF picture".into()),
    };
    let (mut at, mut bmhd, mut cmap, mut camg, mut body) = (12, None, Vec::<[u8; 3]>::new(), 0u32, None);
    while at + 8 <= f.len() {
        let len = u32::from_be_bytes([f[at + 4], f[at + 5], f[at + 6], f[at + 7]]) as usize;
        let data = f.get(at + 8..at.saturating_add(8).saturating_add(len).min(f.len())).unwrap_or_default();
        match &f[at..at + 4] {
            b"BMHD" if data.len() >= 20 => {
                let u16at = |i: usize| usize::from(u16::from_be_bytes([data[i], data[i + 1]]));
                bmhd = Some(Bmhd { w: u16at(0), h: u16at(2), planes: usize::from(data[8]), masking: data[9], compression: data[10] });
            }
            b"CMAP" => cmap = data.as_chunks::<3>().0.to_vec(),
            b"CAMG" if data.len() >= 4 => camg = u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            b"BODY" => body = Some(data),
            _ => {}
        }
        at = at.saturating_add(8).saturating_add(len).saturating_add(len & 1);
    }
    let (b, body) = (bmhd.ok_or("IFF picture has no BMHD")?, body.ok_or("IFF picture has no BODY")?);
    if b.w == 0 || b.h == 0 || b.compression > 1 || !(if pbm { b.planes == 8 } else { matches!(b.planes, 1..=8 | 24 | 32) }) {
        return Err("unsupported IFF picture".into());
    }
    if (b.w * b.h) as u64 > max_pixels || (b.w * b.h * 3) as u64 > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let stride = b.w.div_ceil(16) * 2;
    let nplanes = if pbm { 1 } else { b.planes + usize::from(b.masking == 1) };
    let row_len = if pbm { b.w + b.w % 2 } else { stride * nplanes };
    let least = if b.compression == 1 { 2 * nplanes } else { row_len };
    if b.h.saturating_mul(least) > body.len() {
        return Err("truncated IFF body".into());
    }
    let ham = !pbm && camg & 0x800 != 0 && matches!(b.planes, 6 | 8);
    let ehb = !pbm && !ham && b.planes == 6 && (camg & 0x80 != 0 || cmap.len() <= 32);
    let pal = |i: usize| -> [u8; 3] {
        if cmap.is_empty() { let v = (i * 255 / ((1usize << b.planes.min(8)) - 1)) as u8; [v; 3] } else if ehb && i >= 32 { cmap.get(i - 32).map_or([0; 3], |c| c.map(|v| v >> 1)) } else { cmap.get(i).copied().unwrap_or([0; 3]) }
    };
    let mut img = RgbImage::new(b.w as u32, b.h as u32);
    let mut pos = 0;
    for y in 0..b.h {
        // ByteRun1 packs each plane's scan line on its own; a PBM row is a single line.
        let row = if b.compression == 1 {
            let unit = if pbm { row_len } else { stride };
            let mut row = Vec::with_capacity(row_len);
            for _ in 0..row_len / unit {
                row.extend(byte_run(body, &mut pos, unit)?);
            }
            row
        } else {
            body.get(pos..pos + row_len).ok_or("truncated IFF body")?.to_vec()
        };
        if b.compression == 0 {
            pos += row_len;
        }
        let mut held = pal(0);
        for x in 0..b.w {
            let bit = |p: usize| usize::from(row[p * stride + x / 8] >> (7 - x % 8) & 1);
            let px = if pbm {
                pal(usize::from(row[x]))
            } else if b.planes >= 24 {
                [0, 8, 16].map(|o| (0..8).fold(0u8, |a, i| a | (bit(o + i) as u8) << i))
            } else {
                let idx = (0..b.planes).fold(0, |a, p| a | bit(p) << p);
                if ham {
                    let hb = b.planes - 2;
                    let v = idx & ((1 << hb) - 1);
                    let e = if hb == 4 { (v << 4 | v) as u8 } else { (v << 2 | v >> 4) as u8 };
                    match idx >> hb {
                        0 => held = pal(v),
                        1 => held[2] = e,
                        2 => held[0] = e,
                        _ => held[1] = e,
                    }
                    held
                } else {
                    pal(idx)
                }
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(px));
        }
    }
    Ok(DynamicImage::ImageRgb8(img))
}

