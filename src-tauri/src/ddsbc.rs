//! DDS textures the `image` crate cannot decode: BC4/BC5 (ATI1/ATI2/BC4U/BC5U), and DX10-header files (BC1-BC5, BC7,
//! plus 8-bit RGBA/BGRA/R). Only the first image (largest mip, first slice or face) is read. BC4 is shown as grey,
//! BC5 as red and green. BC6H is refused: the texture crate's BC6H decoder is wrong for mode 15 and for some endpoints.

use image::{DynamicImage, RgbaImage};
use std::{io::Read, path::Path};
use texture2ddecoder as t2d;

#[derive(Clone, Copy)]
enum Kind {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Bc7,
    Rgba,
    Bgra,
    Grey,
}

const DX10_HEADER_END: usize = 148;

/// The format named by the header and where its pixel data starts.
fn kind(f: &[u8]) -> Result<(Kind, usize), String> {
    let u32at = |at: usize| f.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let unsupported = |what: String| Err(format!("unsupported DDS format ({what})"));
    match f.get(84..88) {
        Some(b"ATI1" | b"BC4U") => Ok((Kind::Bc4, 128)),
        Some(b"ATI2" | b"BC5U") => Ok((Kind::Bc5, 128)),
        Some(b"BC4S" | b"BC5S") => unsupported("signed BC4/BC5".into()),
        Some(b"DX10") => {
            let k = match u32at(128).ok_or("truncated DDS file")? {
                70..=72 => Kind::Bc1,
                73..=75 => Kind::Bc2,
                76..=78 => Kind::Bc3,
                79 | 80 => Kind::Bc4,
                82 | 83 => Kind::Bc5,
                94..=96 => return unsupported("BC6H".into()),
                97..=99 => Kind::Bc7,
                28 | 29 => Kind::Rgba,
                87 | 91 => Kind::Bgra,
                61 => Kind::Grey,
                n => return unsupported(format!("DXGI {n}")),
            };
            Ok((k, DX10_HEADER_END))
        }
        _ => unsupported("neither DX10 nor BC4/BC5".into()),
    }
}

/// True when the header asks for a format only this module reads, so the caller can leave the rest to `image`.
pub fn applies(path: &Path) -> bool {
    let mut head = Vec::with_capacity(DX10_HEADER_END);
    let read = std::fs::File::open(path).and_then(|f| f.take(DX10_HEADER_END as u64).read_to_end(&mut head));
    read.is_ok() && wants(&head)
}

pub(crate) fn wants(head: &[u8]) -> bool {
    head.get(..4) == Some(b"DDS ") && matches!(head.get(84..88), Some(b"DX10" | b"ATI1" | b"ATI2" | b"BC4U" | b"BC5U" | b"BC4S" | b"BC5S"))
}

type BlockDecoder = fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>;

/// Runs a `texture2ddecoder` function (BGRA words out) and returns RGBA bytes; `grey` repeats the red channel, where
/// the crate stores a single-channel texture.
fn via_crate(decode: BlockDecoder, data: &[u8], w: usize, h: usize, grey: bool) -> Result<Vec<u8>, String> {
    let mut px = vec![0u32; w * h];
    decode(data, w, h, &mut px).map_err(str::to_string)?;
    Ok(px
        .iter()
        .flat_map(|p| {
            let [b, g, r, a] = p.to_le_bytes();
            if grey { [r, r, r, 255] } else { [r, g, b, a] }
        })
        .collect())
}

fn rgb565(v: u16) -> [u8; 3] {
    let (r, g, b) = ((v >> 11) as u8 & 31, (v >> 5) as u8 & 63, v as u8 & 31);
    [r << 3 | r >> 2, g << 2 | g >> 4, b << 3 | b >> 2]
}

/// BC2 (explicit 4-bit alpha) and BC3 (interpolated alpha). Their colour block always has four colours, whatever
/// the order of its endpoints, unlike BC1; the texture crate applies the BC1 rule here, which is wrong for them.
fn bc2_bc3(data: &[u8], w: usize, h: usize, bc3: bool) -> Vec<u8> {
    let (bw, bh) = (w.div_ceil(4), h.div_ceil(4));
    let mut out = vec![0u8; w * h * 4];
    for (n, blk) in data.as_chunks::<16>().0.iter().take(bw * bh).enumerate() {
        let (a, b) = (rgb565(u16::from_le_bytes([blk[8], blk[9]])), rgb565(u16::from_le_bytes([blk[10], blk[11]])));
        let mix = |p: u32, q: u32| [0, 1, 2].map(|i| ((u32::from(a[i]) * p + u32::from(b[i]) * q) / (p + q)) as u8);
        let pal = [a, b, mix(2, 1), mix(1, 2)];
        let idx = u32::from_le_bytes([blk[12], blk[13], blk[14], blk[15]]);
        let (a0, a1) = (u16::from(blk[0]), u16::from(blk[1]));
        let codes = u64::from_le_bytes([blk[2], blk[3], blk[4], blk[5], blk[6], blk[7], 0, 0]);
        for i in 0..16 {
            let (x, y) = ((n % bw) * 4 + i % 4, (n / bw) * 4 + i / 4);
            if x >= w || y >= h {
                continue;
            }
            let alpha = if bc3 {
                match ((codes >> (3 * i)) & 7) as u16 {
                    0 => a0,
                    1 => a1,
                    c if a0 > a1 => ((8 - c) * a0 + (c - 1) * a1) / 7,
                    6 => 0,
                    7 => 255,
                    c => ((6 - c) * a0 + (c - 1) * a1) / 5,
                }
            } else {
                u16::from(blk[i / 2] >> (4 * (i % 2)) & 15) * 17
            };
            let c = pal[(idx >> (2 * i)) as usize & 3];
            out[(y * w + x) * 4..][..4].copy_from_slice(&[c[0], c[1], c[2], alpha as u8]);
        }
    }
    out
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let u32at = |at: usize| f.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    if f.get(..4) != Some(b"DDS ") || u32at(4) != Some(124) {
        return Err("not a valid DDS file".into());
    }
    let (h, w) = (u32at(12).ok_or("truncated DDS file")?, u32at(16).ok_or("truncated DDS file")?);
    let (kind, at) = kind(f)?;
    if w == 0 || h == 0 {
        return Err("not a valid DDS file".into());
    }
    let pixels = (w as u64).saturating_mul(h as u64);
    if pixels > max_pixels || pixels * 8 > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let blocks = (w.div_ceil(4) as u64) * (h.div_ceil(4) as u64);
    let needed = match kind {
        Kind::Bc1 | Kind::Bc4 => blocks * 8,
        Kind::Rgba | Kind::Bgra => pixels * 4,
        Kind::Grey => pixels,
        _ => blocks * 16,
    };
    let data = f.get(at..).filter(|d| d.len() as u64 >= needed).ok_or("truncated DDS file")?;
    let raw = match kind {
        Kind::Bc1 => via_crate(t2d::decode_bc1a, data, w, h, false)?,
        Kind::Bc2 | Kind::Bc3 => bc2_bc3(data, w, h, matches!(kind, Kind::Bc3)),
        Kind::Bc4 => via_crate(t2d::decode_bc4, data, w, h, true)?,
        Kind::Bc5 => via_crate(t2d::decode_bc5, data, w, h, false)?,
        Kind::Bc7 => via_crate(t2d::decode_bc7, data, w, h, false)?,
        Kind::Rgba => data[..w * h * 4].to_vec(),
        Kind::Bgra => data[..w * h * 4].as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect(),
        Kind::Grey => data[..w * h].iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
    };
    RgbaImage::from_raw(w as u32, h as u32, raw).map(DynamicImage::ImageRgba8).ok_or_else(|| "DDS pixel buffer has the wrong size".into())
}
