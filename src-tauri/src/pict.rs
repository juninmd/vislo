//! PICT (`.pct`, `.pict`): the first bitmap of a QuickDraw picture (version 1 or 2): 1-bit bitmaps and 1/2/4/8-bit
//! indexed PixMaps (PackBits or raw), and 16/32-bit direct PixMaps. Drawing commands are skipped, not rendered, so a
//! vector-only PICT is an error; QuickTime-compressed images and pixel patterns are not supported.

use image::{DynamicImage, RgbImage};
use std::path::Path;

struct Reader<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.p.checked_add(n).and_then(|e| self.d.get(self.p..e)).ok_or("truncated PICT file")?;
        self.p += n;
        Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, String> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        self.take(4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }
    /// A region or polygon: its 2-byte size counts itself.
    fn sized(&mut self) -> Result<(), String> {
        let n = usize::from(self.u16()?);
        self.skip(n.checked_sub(2).ok_or("corrupt PICT size")?)
    }
    fn text(&mut self, before: usize) -> Result<(), String> {
        self.skip(before)?;
        let n = usize::from(self.u8()?);
        self.skip(n)
    }
}

/// Skips the operand of a drawing opcode; `Ok(false)` for the end of the picture.
fn skip(r: &mut Reader, op: u16) -> Result<bool, String> {
    match op {
        0x00FF => return Ok(false),
        0x0000 | 0x001C | 0x001E | 0x0038..=0x003F | 0x0048..=0x004F | 0x0058..=0x005F | 0x0078..=0x007F | 0x0088..=0x008F | 0x00B0..=0x00CF | 0x8000..=0x80FF => {}
        0x0001 | 0x0070..=0x0077 | 0x0080..=0x0087 => r.sized()?,
        0x0004 => r.skip(1)?,
        0x0003 | 0x0005 | 0x0008 | 0x000D | 0x0015 | 0x0016 | 0x0023 | 0x00A0 => r.skip(2)?,
        0x0006 | 0x0007 | 0x000B | 0x000C | 0x000E | 0x000F | 0x0021 | 0x0068..=0x006F => r.skip(4)?,
        0x001A | 0x001B | 0x001D | 0x001F | 0x0022 => r.skip(6)?,
        0x0002 | 0x0009 | 0x000A | 0x0010 | 0x0020 | 0x002E | 0x0030..=0x0037 | 0x0040..=0x0047 | 0x0050..=0x0057 => r.skip(8)?,
        0x002D => r.skip(10)?,
        0x0060..=0x0067 => r.skip(12)?,
        0x0C00 => r.skip(24)?,
        0x0028 => r.text(4)?,
        0x0029 | 0x002A => r.text(1)?,
        0x002B => r.text(2)?,
        0x002C => {
            let n = usize::from(r.u16()?);
            r.skip(n)?;
        }
        0x00A1 => {
            r.skip(2)?;
            let n = usize::from(r.u16()?);
            r.skip(n)?;
        }
        0x00D0..=0x00FE | 0x8100..=0xFFFF => {
            let n = r.u32()? as usize;
            r.skip(n)?;
        }
        0x0100..=0x7FFF => r.skip(usize::from(op >> 8) * 2)?,
        _ => return Err(format!("unsupported PICT opcode {op:#06x}")),
    }
    Ok(true)
}

/// Expands PackBits data to exactly `len` bytes, in units of `unit` bytes (2 for 16-bit pixels).
fn unpack(src: &[u8], len: usize, unit: usize) -> Result<Vec<u8>, String> {
    let (mut out, mut i) = (Vec::with_capacity(len), 0);
    while out.len() < len {
        let n = *src.get(i).ok_or("truncated PICT pixel data")? as i8;
        i += 1;
        if n >= 0 {
            let c = (n as usize + 1) * unit;
            out.extend_from_slice(src.get(i..i + c).ok_or("truncated PICT pixel data")?);
            i += c;
        } else if n != -128 {
            let unit_bytes = src.get(i..i + unit).ok_or("truncated PICT pixel data")?;
            i += unit;
            for _ in 0..(1 - i32::from(n)) {
                out.extend_from_slice(unit_bytes);
            }
        }
    }
    if out.len() != len {
        return Err("corrupt PICT pixel data".into());
    }
    Ok(out)
}

struct Map {
    w: usize,
    h: usize,
    row_bytes: usize,
    bits: usize,
    cmp: usize,
    direct: bool,
    packed: bool,
    palette: Vec<[u8; 3]>,
}

fn bitmap(r: &mut Reader, op: u16, max_pixels: u64) -> Result<DynamicImage, String> {
    let direct = matches!(op, 0x009A | 0x009B);
    let (region, packed) = (matches!(op, 0x0091 | 0x0099 | 0x009B), op >= 0x0098);
    if direct {
        r.skip(4)?; // base address
    }
    let rb = r.u16()?;
    let (is_pix, row_bytes) = (rb & 0x8000 != 0, usize::from(rb & 0x3FFF));
    let rect: Vec<i32> = (0..4).map(|_| r.u16().map(|v| i32::from(v as i16))).collect::<Result<_, _>>()?;
    let (w, h) = (rect[3] - rect[1], rect[2] - rect[0]);
    let (mut pack_type, mut bits, mut cmp) = (0, 1, 1);
    if is_pix {
        r.skip(2)?;
        pack_type = r.u16()?;
        r.skip(14)?; // pack size, resolutions, pixel type
        (bits, cmp) = (usize::from(r.u16()?), usize::from(r.u16()?));
        r.skip(14)?; // component size, plane bytes, colour table handle, reserved
    }
    let mut palette = vec![[255; 3], [0; 3]]; // a plain BitMap: 1 is black
    if is_pix && !direct {
        r.skip(4)?; // seed
        let device = r.u16()? & 0x8000 != 0; // a device table is indexed by position; the value fields mean nothing
        let n = usize::from(r.u16()?) + 1;
        if n > 256 {
            return Err("corrupt PICT colour table".into());
        }
        palette = vec![[0; 3]; 256];
        for i in 0..n {
            let (v, red, green, blue) = (usize::from(r.u16()?), r.u16()?, r.u16()?, r.u16()?);
            palette[if !device && v < 256 { v } else { i }] = [(red >> 8) as u8, (green >> 8) as u8, (blue >> 8) as u8];
        }
    }
    r.skip(18)?; // source and destination rectangles, transfer mode
    if region {
        r.sized()?;
    }
    if w <= 0 || h <= 0 || !(if direct { matches!(bits, 16 | 32) } else { matches!(bits, 1 | 2 | 4 | 8) }) || !matches!(cmp, 1 | 3 | 4) || (direct && bits == 32 && cmp == 1) {
        return Err("unsupported PICT pixel format".into());
    }
    if direct && bits == 32 && pack_type == 2 {
        return Err("PICT 32-bit pixels without a pad byte are not supported".into());
    }
    let (w, h) = (w as usize, h as usize);
    let needed = if direct && bits == 32 { w * 4 } else { (w * bits).div_ceil(8) };
    if (w * h) as u64 > max_pixels || (w * h * 3) as u64 > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    if row_bytes < needed {
        return Err("corrupt PICT row size".into());
    }
    // Every row costs at least a count and a byte (packed) or a whole row, so a tiny file cannot claim a big frame.
    let least = if packed && pack_type != 1 && row_bytes >= 8 { 2 } else { row_bytes };
    if h.saturating_mul(least) > r.d.len().saturating_sub(r.p) {
        return Err("truncated PICT file".into());
    }
    decode_rows(r, &Map { w, h, row_bytes, bits, cmp, direct, packed: packed && pack_type != 1, palette })
}

fn decode_rows(r: &mut Reader, m: &Map) -> Result<DynamicImage, String> {
    let mut img = RgbImage::new(m.w as u32, m.h as u32);
    let planar = m.direct && m.bits == 32 && m.packed;
    let row_len = if planar { m.cmp * m.w } else { m.row_bytes };
    for y in 0..m.h {
        let row = if m.packed && m.row_bytes >= 8 {
            let n = if row_len > 250 { usize::from(r.u16()?) } else { usize::from(r.u8()?) };
            unpack(r.take(n)?, row_len, if m.direct && m.bits == 16 { 2 } else { 1 })?
        } else {
            r.take(m.row_bytes)?.to_vec()
        };
        for x in 0..m.w {
            let px = if !m.direct {
                let idx = (row[x * m.bits / 8] >> (8 - m.bits - x * m.bits % 8)) & ((1u16 << m.bits) - 1) as u8;
                m.palette[usize::from(idx)]
            } else if m.bits == 16 {
                let v = u16::from_be_bytes([row[2 * x], row[2 * x + 1]]);
                [10, 5, 0].map(|s| {
                    let c = ((v >> s) & 31) as u8;
                    c << 3 | c >> 2
                })
            } else if planar {
                let first = m.cmp - 3; // a leading alpha plane is ignored
                [row[first * m.w + x], row[(first + 1) * m.w + x], row[(first + 2) * m.w + x]]
            } else {
                [row[4 * x + 1], row[4 * x + 2], row[4 * x + 3]]
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(px));
        }
    }
    Ok(DynamicImage::ImageRgb8(img))
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    decode_bytes(&std::fs::read(path).map_err(|e| e.to_string())?, max_pixels)
}

pub(crate) fn decode_bytes(f: &[u8], max_pixels: u64) -> Result<DynamicImage, String> {
    // Files normally carry 512 unused bytes first; a picture saved without them starts right away.
    let start = [512usize, 0].into_iter().find(|s| matches!(f.get(s + 10..s + 14), Some([0, 0x11, 2, 0xFF])) || matches!(f.get(s + 10..s + 12), Some([0x11, 1]))).ok_or("not a valid PICT file")?;
    let v2 = f[start + 10] == 0;
    let mut r = Reader { d: f, p: start + 10 + if v2 { 4 } else { 2 } };
    loop {
        if v2 && (r.p - start) % 2 == 1 {
            r.p += 1; // version 2 opcodes are word aligned
        }
        let op = if v2 { r.u16()? } else { u16::from(r.u8()?) };
        if matches!(op, 0x0090 | 0x0091 | 0x0098 | 0x0099 | 0x009A | 0x009B) {
            return bitmap(&mut r, op, max_pixels);
        }
        match op {
            0x8200 | 0x8201 => return Err("QuickTime-compressed PICT images are not supported".into()),
            0x0012..=0x0014 => return Err("PICT pixel patterns are not supported".into()),
            _ if !skip(&mut r, op)? => return Err("this PICT has only drawing commands, no bitmap".into()),
            _ => {}
        }
    }
}

