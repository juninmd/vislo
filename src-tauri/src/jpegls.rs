//! JPEG-LS (`.jls`): ITU-T T.87 lossless and near-lossless, 2 to 16 bits, grey or three components, none/line/sample
//! interleaved. Subsampled components, mapping tables and point transforms are refused.

use crate::jpegls_codec::{params, Comp, Dec};
use image::{DynamicImage, GrayImage, RgbImage};
use std::path::Path;

struct Frame {
    h: usize,
    w: usize,
    ids: Vec<u8>,
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let bad = || "not a valid JPEG-LS file".to_string();
    let be16 = |at: usize| f.get(at..at + 2).map(|b| usize::from(u16::from_be_bytes([b[0], b[1]])));
    if f.get(..2) != Some(&[0xFF, 0xD8]) {
        return Err(bad());
    }
    let (mut at, mut frame, mut preset, mut maxval) = (2usize, None::<Frame>, [0i32; 4], 0i32);
    let mut planes: Vec<Vec<u16>> = Vec::new();
    let mut done: Vec<bool> = Vec::new();
    while at < f.len() {
        if f[at] != 0xFF {
            return Err(bad());
        }
        let marker = *f.get(at + 1).ok_or_else(bad)?;
        if marker == 0xD9 {
            break;
        }
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) || marker == 0xFF {
            at += if marker == 0xFF { 1 } else { 2 };
            continue;
        }
        let len = be16(at + 2).filter(|l| *l >= 2).ok_or_else(bad)?;
        let seg = f.get(at + 4..at + 2 + len).ok_or_else(bad)?;
        at += 2 + len;
        match marker {
            0xF7 => {
                let nf = usize::from(*seg.get(5).ok_or_else(bad)?);
                if frame.is_some() || seg.len() != 6 + 3 * nf {
                    return Err(bad());
                }
                let (bits, h, w) = (u32::from(seg[0]), usize::from(u16::from_be_bytes([seg[1], seg[2]])), usize::from(u16::from_be_bytes([seg[3], seg[4]])));
                if h == 0 || w == 0 || !matches!(nf, 1 | 3) || !(2..=16).contains(&bits) {
                    return Err("unsupported JPEG-LS frame (need 2-16 bits, 1 or 3 components, known height)".into());
                }
                if seg[6..].chunks(3).any(|c| c[1] != 0x11) {
                    return Err("subsampled JPEG-LS components are not supported".into());
                }
                let px = (w * h) as u64;
                if px > max_pixels || px * nf as u64 * 3 > crate::decode::MAX_ALLOC {
                    return Err(crate::decode::TOO_LARGE.to_string());
                }
                maxval = (1 << bits) - 1;
                planes = vec![vec![0; w * h]; nf];
                done = vec![false; nf];
                frame = Some(Frame { h, w, ids: seg[6..].chunks(3).map(|c| c[0]).collect() });
            }
            0xF8 => match seg {
                [1, rest @ ..] if rest.len() >= 10 => {
                    let v = |i: usize| i32::from(u16::from_be_bytes([rest[2 * i], rest[2 * i + 1]]));
                    preset = [v(1), v(2), v(3), v(4)];
                    if v(0) != 0 {
                        maxval = v(0);
                    }
                }
                _ => return Err("JPEG-LS mapping tables are not supported".into()),
            },
            0xDA => {
                let fr = frame.as_ref().ok_or_else(bad)?;
                let ns = usize::from(*seg.first().ok_or_else(bad)?);
                if seg.len() != 4 + 2 * ns || ns == 0 || ns > fr.ids.len() {
                    return Err(bad());
                }
                let (near, ilv, pt) = (i32::from(seg[1 + 2 * ns]), seg[2 + 2 * ns], seg[3 + 2 * ns] & 0x0F);
                let comp: Vec<usize> = (0..ns).map(|i| fr.ids.iter().position(|id| *id == seg[1 + 2 * i])).collect::<Option<_>>().ok_or_else(bad)?;
                if (0..ns).any(|i| seg[2 + 2 * i] != 0) {
                    return Err("JPEG-LS mapping tables are not supported".into());
                }
                if pt != 0 || ilv > 2 || (ns > 1 && ilv == 0) || (ilv == 2 && ns != 3) || !(1..65536).contains(&maxval) || near > 255.min(maxval / 2) || comp.iter().any(|c| done[*c]) {
                    return Err("unsupported JPEG-LS scan".into());
                }
                let start = at;
                let end = (start..f.len().saturating_sub(1)).find(|&i| f[i] == 0xFF && f[i + 1] >= 0x80).unwrap_or(f.len());
                let mut dec = Dec::new(params(maxval, near, preset)?, &f[start..end]);
                let (w, h) = (fr.w, fr.h);
                let mut cs: Vec<Comp> = comp.iter().map(|_| Comp { prev: vec![0; w + 2], cur: vec![0; w + 2] }).collect();
                let mut ri = vec![0usize; ns];
                for y in 0..h {
                    if ilv == 2 {
                        dec.line(&mut cs, &mut ri[0], w)?;
                    } else {
                        for (c, r) in cs.chunks_mut(1).zip(ri.iter_mut()) {
                            dec.line(c, r, w)?;
                        }
                    }
                    for (c, k) in cs.iter().zip(&comp) {
                        for (dst, src) in planes[*k][y * w..(y + 1) * w].iter_mut().zip(&c.prev[1..=w]) {
                            *dst = *src as u16;
                        }
                    }
                }
                comp.iter().for_each(|c| done[*c] = true);
                at = end;
            }
            _ => {}
        }
    }
    let fr = frame.ok_or_else(bad)?;
    if done.iter().any(|d| !d) {
        return Err("JPEG-LS file has a missing scan".into());
    }
    let (w, h) = (fr.w as u32, fr.h as u32);
    let to8 = |v: u16| (u32::from(v) * 255 / maxval.max(1) as u32).min(255) as u8;
    let at = |c: usize, x: u32, y: u32| to8(planes[c][y as usize * fr.w + x as usize]);
    Ok(if planes.len() == 1 {
        DynamicImage::ImageLuma8(GrayImage::from_fn(w, h, |x, y| image::Luma([at(0, x, y)])))
    } else {
        DynamicImage::ImageRgb8(RgbImage::from_fn(w, h, |x, y| image::Rgb([at(0, x, y), at(1, x, y), at(2, x, y)])))
    })
}

