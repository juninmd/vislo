//! Autodesk Animator FLI/FLC (`.fli`, `.flc`): the first frame of the animation as a 256-colour image. The chunk types
//! a first frame uses are read (palettes, BRUN, LC, DELTA_FLC, COPY, BLACK); later frames are ignored.

use image::{DynamicImage, RgbImage};
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

struct Cur<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Cur<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.p.checked_add(n).and_then(|e| self.d.get(self.p..e)).ok_or("truncated FLI data")?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn i8(&mut self) -> Result<i8, String> {
        Ok(self.u8()? as i8)
    }
    fn u16(&mut self) -> Result<u16, String> {
        self.take(2).map(|b| u16::from_le_bytes([b[0], b[1]]))
    }
}

fn palette(c: &mut Cur, pal: &mut [[u8; 3]; 256], shift: u32) -> Result<(), String> {
    let (mut at, packets) = (0usize, c.u16()?);
    for _ in 0..packets {
        at += usize::from(c.u8()?);
        let n = match c.u8()? { 0 => 256, n => usize::from(n) };
        for _ in 0..n {
            let rgb = c.take(3)?;
            if at < 256 {
                pal[at] = [rgb[0] << shift, rgb[1] << shift, rgb[2] << shift];
            }
            at += 1;
        }
    }
    Ok(())
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    let u16at = |at: usize| f.get(at..at + 2).map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    if !matches!(u16at(4), Some(0xAF11 | 0xAF12)) {
        return Err("not a valid FLI/FLC file".into());
    }
    let (w, h) = (u16at(8).ok_or("truncated FLI file")?, u16at(10).ok_or("truncated FLI file")?);
    if !matches!(u16at(12), Some(0 | 8)) {
        return Err("FLI files with a colour depth other than 8 bits are not supported".into());
    }
    if w == 0 || h == 0 || (w * h) as u64 > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    // FLC stores where the first frame starts; a 0xF100 prefix chunk may sit before it.
    let oframe1 = f.get(80..84).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
    let mut at = if u16at(4) == Some(0xAF12) && oframe1 >= 128 { oframe1 } else { 128 };
    if u16at(at + 4) == Some(0xF100) {
        at = at.saturating_add(f.get(at..at + 4).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize));
    }
    let mut c = Cur { d: f, p: at };
    let (_size, magic) = (c.take(4)?, c.u16()?);
    let chunks = c.u16()?;
    c.take(8)?;
    if magic != 0xF1FA {
        return Err("FLI file has no first frame".into());
    }
    let (mut pix, mut pal) = (vec![0u8; w * h], [[0u8; 3]; 256]);
    // A BLACK chunk wipes what was drawn before it, so earlier drawing is skipped: a frame of many BLACK chunks
    // would otherwise clear the whole picture once per 6 bytes.
    let last_black = {
        let (mut p, mut last) = (c.p, None);
        for i in 0..chunks {
            let Some(head) = f.get(p..p + 6) else { break };
            let size = u32::from_le_bytes([head[0], head[1], head[2], head[3]]) as usize;
            if u16::from_le_bytes([head[4], head[5]]) == 13 {
                last = Some(i);
            }
            p = p.saturating_add(size.max(6));
        }
        last
    };
    for n in 0..chunks {
        let start = c.p;
        let (size, kind) = (u32::from_le_bytes(c.take(4)?.try_into().unwrap()) as usize, c.u16()?);
        let end = start.checked_add(size).filter(|e| *e <= f.len() && size >= 6).ok_or("corrupt FLI chunk")?;
        let mut d = Cur { d: &f[..end], p: c.p };
        let overdrawn = last_black.is_some_and(|b| n < b);
        match kind {
            7 | 12 | 15 | 16 if overdrawn => {}
            4 => palette(&mut d, &mut pal, 0)?,
            11 => palette(&mut d, &mut pal, 2)?,
            13 => {} // the picture starts black and everything drawn before this chunk was skipped
            16 => pix.copy_from_slice(d.take(w * h)?),
            15 => {
                for y in 0..h {
                    d.u8()?; // packet count: obsolete, the row is filled to its width
                    let mut x = 0;
                    while x < w {
                        let n = d.i8()?;
                        let k = usize::from(n.unsigned_abs()).min(w - x);
                        if n >= 0 {
                            let v = d.u8()?;
                            pix[y * w + x..y * w + x + k].fill(v);
                        } else {
                            pix[y * w + x..y * w + x + k].copy_from_slice(&d.take(usize::from(n.unsigned_abs()))?[..k]);
                        }
                        x += k.max(1);
                    }
                }
            }
            12 => {
                let (first, lines) = (usize::from(d.u16()?), usize::from(d.u16()?));
                for y in first..(first + lines).min(h) {
                    let mut x = 0;
                    for _ in 0..d.u8()? {
                        x += usize::from(d.u8()?);
                        let n = d.i8()?;
                        let k = usize::from(n.unsigned_abs());
                        let row = &mut pix[y * w..(y + 1) * w];
                        if n >= 0 {
                            let src = d.take(k)?;
                            for (i, s) in src.iter().enumerate().take(w.saturating_sub(x)) { row[x + i] = *s; }
                        } else {
                            let v = d.u8()?;
                            for i in 0..k.min(w.saturating_sub(x)) { row[x + i] = v; }
                        }
                        x += k;
                    }
                }
            }
            7 => {
                let (mut lines, mut y) = (d.u16()?, 0usize);
                while lines > 0 && y < h {
                    let op = d.u16()?;
                    match op >> 14 {
                        0 => {
                            let mut x = 0;
                            for _ in 0..op {
                                x += usize::from(d.u8()?);
                                let n = d.i8()?;
                                let k = usize::from(n.unsigned_abs());
                                let row = &mut pix[y * w..(y + 1) * w];
                                if n >= 0 {
                                    for i in 0..k * 2 { let b = d.u8()?; if x + i < w { row[x + i] = b; } }
                                } else {
                                    let pair = d.take(2)?;
                                    for i in 0..k { for (j, b) in pair.iter().enumerate() { if x + 2 * i + j < w { row[x + 2 * i + j] = *b; } } }
                                }
                                x += 2 * k;
                            }
                            y += 1;
                            lines -= 1;
                        }
                        3 => y += usize::from(op.wrapping_neg()), // skip lines
                        2 => pix[y * w + w - 1] = op as u8,       // last byte of the line
                        _ => return Err("corrupt FLI line opcode".into()),
                    }
                }
            }
            _ => {}
        }
        c.p = end;
    }
    let img = RgbImage::from_fn(w as u32, h as u32, |x, y| image::Rgb(pal[usize::from(pix[y as usize * w + x as usize])]));
    Ok(DynamicImage::ImageRgb8(img))
}
