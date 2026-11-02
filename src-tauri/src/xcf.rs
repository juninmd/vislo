//! GIMP XCF (`.xcf`): versions 0 to 13 at 8 bits per channel, RGB/grey/indexed layers, raw, RLE and zlib tiles.
//! The picture is composed bottom-up with the "normal" mode, layer opacity and layer masks; 16/32-bit precision is refused. Hidden groups hide their children and group opacity multiplies in; the
//! blend modes of layers and groups other than normal are not modelled.

use image::{DynamicImage, RgbaImage};
use std::path::Path;

const MAX_FILE: u64 = 1 << 30;
const MAX_LAYERS: usize = 512;
const TILE: usize = 64;

struct Reader<'a> {
    f: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).ok_or("not a valid XCF file")?;
        let b = self.f.get(self.at..end).ok_or("truncated XCF data")?;
        self.at = end;
        Ok(b)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn ptr(&mut self, wide: bool) -> Result<usize, String> {
        if wide {
            usize::try_from(u64::from_be_bytes(self.take(8)?.try_into().unwrap())).map_err(|_| "not a valid XCF file".to_string())
        } else {
            Ok(self.u32()? as usize)
        }
    }
    fn at(&self, ptr: usize) -> Reader<'a> {
        Reader { f: self.f, at: ptr }
    }
    /// Properties up to PROP_END as `(type, payload)`.
    fn props(&mut self) -> Result<Vec<(u32, &'a [u8])>, String> {
        let mut out = Vec::new();
        loop {
            let (kind, len) = (self.u32()?, self.u32()? as usize);
            if kind == 0 {
                return Ok(out);
            }
            out.push((kind, self.take(len)?));
            if out.len() > 4096 {
                return Err("not a valid XCF file".to_string());
            }
        }
    }
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// One RLE plane of `n` bytes starting at `r`.
fn rle_plane(r: &mut Reader, n: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(n);
    while out.len() < n {
        let op = r.take(1)?[0];
        let (count, literal) = match op {
            0..=126 => (usize::from(op) + 1, false),
            127 => (usize::from(u16::from_be_bytes(r.take(2)?.try_into().unwrap())), false),
            128 => (usize::from(u16::from_be_bytes(r.take(2)?.try_into().unwrap())), true),
            _ => (256 - usize::from(op), true),
        };
        if count > n - out.len() {
            return Err("corrupt XCF tile".to_string());
        }
        if literal {
            out.extend_from_slice(r.take(count)?);
        } else {
            let v = r.take(1)?[0];
            out.extend(std::iter::repeat_n(v, count));
        }
    }
    Ok(out)
}

/// Whole-layer pixels (`w * h * bpp`, interleaved) from the hierarchy at `ptr`.
fn read_hierarchy(r: &Reader, ptr: usize, compression: u8, wide: bool, bpp_expected: usize, budget: usize) -> Result<(usize, usize, Vec<u8>), String> {
    let mut h = r.at(ptr);
    let (w, hh, bpp) = (h.u32()? as usize, h.u32()? as usize, h.u32()? as usize);
    if bpp != bpp_expected || w.checked_mul(hh).and_then(|p| p.checked_mul(bpp)).is_none_or(|n| n > budget) {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let mut level = r.at(h.ptr(wide)?);
    if (level.u32()? as usize, level.u32()? as usize) != (w, hh) {
        return Err("not a valid XCF file".to_string());
    }
    let mut out = vec![0u8; w * hh * bpp];
    let across = w.div_ceil(TILE);
    for t in 0..across * hh.div_ceil(TILE) {
        let (tx, ty) = ((t % across) * TILE, (t / across) * TILE);
        let (tw, th) = (TILE.min(w - tx), TILE.min(hh - ty));
        let ptr = level.ptr(wide)?;
        if ptr == 0 {
            break; // the list ends early: the remaining tiles were never written and stay transparent
        }
        let mut tile = r.at(ptr);
        let n = tw * th;
        let px: Vec<u8> = match compression {
            0 => tile.take(n * bpp)?.to_vec(),
            1 => {
                let planes = (0..bpp).map(|_| rle_plane(&mut tile, n)).collect::<Result<Vec<_>, _>>()?;
                (0..n).flat_map(|i| planes.iter().map(move |p| p[i])).collect()
            }
            2 => {
                use std::io::Read;
                let mut raw = Vec::with_capacity(n * bpp);
                flate2::read::ZlibDecoder::new(&r.f[tile.at.min(r.f.len())..]).take((n * bpp) as u64).read_to_end(&mut raw).map_err(|e| e.to_string())?;
                if raw.len() != n * bpp {
                    return Err("corrupt XCF tile".to_string());
                }
                raw
            }
            c => return Err(format!("unsupported XCF compression {c}")),
        };
        for y in 0..th {
            let dst = ((ty + y) * w + tx) * bpp;
            out[dst..dst + tw * bpp].copy_from_slice(&px[y * tw * bpp..(y + 1) * tw * bpp]);
        }
    }
    Ok((w, hh, out))
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    use std::io::Read;
    let mut f = Vec::new();
    std::fs::File::open(path).map_err(|e| e.to_string())?.take(MAX_FILE + 1).read_to_end(&mut f).map_err(|e| e.to_string())?;
    if f.len() as u64 > MAX_FILE {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let bad = || "not a valid XCF file".to_string();
    if !f.starts_with(b"gimp xcf ") || f.get(13) != Some(&0) {
        return Err(bad());
    }
    let version: u32 = match &f[9..13] {
        b"file" => 0,
        v if v[0] == b'v' => std::str::from_utf8(&v[1..]).ok().and_then(|s| s.parse().ok()).ok_or_else(bad)?,
        _ => return Err(bad()),
    };
    let wide = version >= 11;
    let mut r = Reader { f: &f, at: 14 };
    let (w, h, base) = (r.u32()?, r.u32()?, r.u32()?);
    if version >= 4 && !matches!(r.u32()?, 100 | 150) {
        return Err("only 8-bit XCF images are supported".to_string());
    }
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels || base > 2 {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let (mut compression, mut palette) = (0u8, Vec::new());
    for (kind, data) in r.props()? {
        match kind {
            1 => palette = data.get(4..).unwrap_or_default().as_chunks::<3>().0.to_vec(),
            17 => compression = data.first().copied().unwrap_or(0),
            _ => {}
        }
    }
    let mut layers = Vec::new();
    loop {
        let p = r.ptr(wide)?;
        if p == 0 {
            break;
        }
        layers.push(p);
        if layers.len() > MAX_LAYERS {
            return Err("too many layers in this XCF file".to_string());
        }
    }
    struct Info {
        size: (usize, usize),
        kind: u32,
        opacity: u32,
        visible: bool,
        group: bool,
        apply_mask: bool,
        off: (i64, i64),
        hier: usize,
        mask: usize,
        path: Vec<u32>,
    }
    // Pass 1: the headers of all layers. A layer outside every group has no PROP_ITEM_PATH; its path is its index
    // among such layers, which is how the paths of group children name their parents.
    let mut infos = Vec::new();
    let mut top_level = 0u32;
    for &lp in &layers {
        let mut l = r.at(lp);
        let (lw, lh, kind) = (l.u32()? as usize, l.u32()? as usize, l.u32()?);
        let name_len = l.u32()? as usize;
        l.take(name_len)?;
        let mut i = Info { size: (lw, lh), kind, opacity: 255, visible: true, group: false, apply_mask: true, off: (0, 0), hier: 0, mask: 0, path: Vec::new() };
        let mut explicit_path = None;
        for (k, d) in l.props()? {
            match k {
                6 => i.opacity = be32(d, 0).unwrap_or(255).min(255),
                8 => i.visible = be32(d, 0).unwrap_or(1) != 0,
                11 => i.apply_mask = be32(d, 0).unwrap_or(1) != 0,
                15 => i.off = (i64::from(be32(d, 0).unwrap_or(0) as i32), i64::from(be32(d, 4).unwrap_or(0) as i32)),
                29 => i.group = true,
                30 => explicit_path = Some((0..d.len() / 4).filter_map(|n| be32(d, n * 4)).collect::<Vec<u32>>()),
                _ => {}
            }
        }
        i.path = explicit_path.unwrap_or_else(|| {
            top_level += 1;
            vec![top_level - 1]
        });
        (i.hier, i.mask) = (l.ptr(wide)?, l.ptr(wide)?);
        infos.push(i);
    }
    let groups: std::collections::HashMap<&[u32], (bool, u32)> = infos.iter().filter(|i| i.group).map(|i| (&i.path[..], (i.visible, i.opacity))).collect();
    let (cw, ch) = (w as usize, h as usize);
    let mut canvas = vec![0u8; cw * ch * 4];
    let budget = (crate::decode::MAX_ALLOC as usize).saturating_sub(canvas.len());
    let mut work = 0usize; // layer bytes decoded so far: repeated pointers must not multiply the cost
    for i in infos.iter().rev() {
        // A hidden group hides its whole subtree; group opacity multiplies into its children.
        let mut opacity = i.opacity;
        let mut shown = i.visible && !i.group;
        for n in 1..i.path.len() {
            if let Some((v, o)) = groups.get(&i.path[..n]) {
                shown &= *v;
                opacity = opacity * o / 255;
            }
        }
        if !shown || opacity == 0 {
            continue;
        }
        let (lw, lh) = i.size;
        let bpp = [3, 4, 1, 2, 1, 2].get(i.kind as usize).copied().ok_or_else(bad)?;
        let (hw, hh, px) = read_hierarchy(&r, i.hier, compression, wide, bpp, budget)?;
        work = work.saturating_add(px.len());
        if (hw, hh) != (lw, lh) {
            return Err(bad());
        }
        let mask_px = if i.mask != 0 && i.apply_mask {
            let mut m = r.at(i.mask);
            let (mw, mh, nlen) = (m.u32()? as usize, m.u32()? as usize, m.u32()? as usize);
            m.take(nlen)?;
            m.props()?;
            let (hw, hh, mpx) = read_hierarchy(&r, m.ptr(wide)?, compression, wide, 1, budget.saturating_sub(px.len()))?;
            work = work.saturating_add(mpx.len());
            if (hw, hh) != (lw, lh) || (mw, mh) != (lw, lh) {
                return Err(bad());
            }
            Some(mpx)
        } else {
            None
        };
        if work > 4 * crate::decode::MAX_ALLOC as usize {
            return Err(crate::decode::TOO_LARGE.to_string());
        }
        // Only the part of the layer that lands on the canvas is visited.
        let (y0, y1) = ((-i.off.1).max(0) as usize, ((i64::from(h) - i.off.1).clamp(0, lh as i64)) as usize);
        let (x0, x1) = ((-i.off.0).max(0) as usize, ((i64::from(w) - i.off.0).clamp(0, lw as i64)) as usize);
        for ly in y0..y1 {
            for lx in x0..x1 {
                let (x, y) = ((lx as i64 + i.off.0) as usize, (ly as i64 + i.off.1) as usize);
                let k = ly * lw + lx;
                let s = &px[k * bpp..(k + 1) * bpp];
                let pal = |v: u8| palette.get(usize::from(v)).copied().unwrap_or([v; 3]);
                let (c, a) = match i.kind {
                    0 => ([s[0], s[1], s[2]], 255),
                    1 => ([s[0], s[1], s[2]], s[3]),
                    2 => ([s[0]; 3], 255),
                    3 => ([s[0]; 3], s[1]),
                    4 => (pal(s[0]), 255),
                    _ => (pal(s[0]), s[1]),
                };
                let mut a = u32::from(a) * opacity / 255;
                if let Some(m) = &mask_px {
                    a = a * u32::from(m[k]) / 255;
                }
                let o = (y * cw + x) * 4;
                let below = u32::from(canvas[o + 3]);
                let out_a = a + below * (255 - a) / 255;
                if out_a > 0 {
                    for c_i in 0..3 {
                        let v = (u32::from(c[c_i]) * a * 255 + u32::from(canvas[o + c_i]) * below * (255 - a)) / (out_a * 255);
                        canvas[o + c_i] = v.min(255) as u8;
                    }
                }
                canvas[o + 3] = out_a as u8;
            }
        }
    }
    RgbaImage::from_raw(w, h, canvas).map(DynamicImage::ImageRgba8).ok_or_else(bad)
}
