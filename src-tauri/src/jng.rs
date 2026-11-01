//! JNG (`.jng`, also the image objects of an MNG): a baseline or progressive JPEG for colour (JDAT), plus an optional
//! alpha plane stored as a greyscale PNG stream (IDAT) or a greyscale JPEG (JDAA). Only 8-bit colour data is supported.

use image::{codecs::jpeg::JpegDecoder, codecs::png::PngDecoder, DynamicImage, GrayImage, ImageDecoder, RgbaImage};
use std::io::Cursor;
use std::path::Path;

const SIG: &[u8] = b"\x8bJNG\r\n\x1a\n";

struct Header {
    w: u32,
    h: u32,
    color: u8,
    alpha_depth: u8,
    alpha_jpeg: bool,
    alpha_interlace: u8,
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn header(d: &[u8]) -> Result<Header, String> {
    let bad = |what: &str| format!("unsupported JNG header ({what})");
    if d.len() != 16 {
        return Err(bad("size"));
    }
    let h = Header { w: be32(&d[0..4]), h: be32(&d[4..8]), color: d[8], alpha_depth: d[12], alpha_jpeg: d[13] == 8, alpha_interlace: d[15] };
    if h.w == 0 || h.h == 0 || h.w > 0x7FFF_FFFF || h.h > 0x7FFF_FFFF {
        return Err(bad("size"));
    }
    if !matches!(h.color, 8 | 10 | 12 | 14) || d[10] != 8 || !matches!(d[11], 0 | 8) {
        return Err(bad("colour type or compression"));
    }
    if d[9] != 8 {
        return Err(format!("{}-bit JNG images are not supported", d[9]));
    }
    let has_alpha = h.color >= 12;
    let alpha_ok = !has_alpha || (if h.alpha_jpeg { h.alpha_depth == 8 } else { matches!(h.alpha_depth, 1 | 2 | 4 | 8 | 16) && d[13] == 0 });
    if !alpha_ok || d[14] != 0 || h.alpha_interlace > 1 {
        return Err(bad("alpha"));
    }
    Ok(h)
}

fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(data);
    [&(data.len() as u32).to_be_bytes()[..], kind, data, &crc.sum().to_be_bytes()].concat()
}

fn grey_jpeg(data: &[u8], (w, h): (u32, u32), what: &str) -> Result<GrayImage, String> {
    let dec = JpegDecoder::new(Cursor::new(data)).map_err(|e| format!("{what}: {e}"))?;
    if dec.dimensions() != (w, h) {
        return Err(format!("{what} does not match the JNG header"));
    }
    DynamicImage::from_decoder(dec).map(|i| i.to_luma8()).map_err(|e| format!("{what}: {e}"))
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    if !f.starts_with(SIG) {
        return Err("not a valid JNG file".into());
    }
    decode_from(&f, SIG.len(), max_pixels)
}

/// Decodes the JNG image object whose JHDR chunk starts at `at`.
pub fn decode_from(f: &[u8], mut at: usize, max_pixels: u64) -> Result<DynamicImage, String> {
    let bad = || "not a valid JNG file".to_string();
    let (mut hdr, mut jdat, mut jdaa, mut idat) = (None, Vec::new(), Vec::new(), Vec::new());
    loop {
        let len = f.get(at..at + 4).map(|b| be32(b) as usize).ok_or_else(bad)?;
        let kind = f.get(at + 4..at + 8).ok_or_else(bad)?;
        let next = at.checked_add(12).and_then(|a| a.checked_add(len)).filter(|n| *n <= f.len()).ok_or_else(bad)?;
        let data = &f[at + 8..next - 4];
        match kind {
            b"JHDR" if hdr.is_none() => hdr = Some(header(data)?),
            b"JDAT" => jdat.extend_from_slice(data),
            b"JDAA" => jdaa.extend_from_slice(data),
            b"IDAT" => idat.extend_from_slice(&f[at..next]),
            b"IEND" => break,
            _ if hdr.is_none() => return Err(bad()),
            _ => {}
        }
        at = next;
    }
    let h = hdr.ok_or_else(bad)?;
    let px = u64::from(h.w) * u64::from(h.h);
    if px > max_pixels || px * 8 > crate::decode::MAX_ALLOC {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let dec = JpegDecoder::new(Cursor::new(&jdat)).map_err(|e| format!("JNG colour data: {e}"))?;
    if dec.dimensions() != (h.w, h.h) {
        return Err("the JNG colour data does not match its header".into());
    }
    let color = DynamicImage::from_decoder(dec).map_err(|e| format!("JNG colour data: {e}"))?;
    if h.color < 12 {
        return Ok(if h.color == 8 { DynamicImage::ImageLuma8(color.to_luma8()) } else { DynamicImage::ImageRgb8(color.to_rgb8()) });
    }
    let alpha = if h.alpha_jpeg {
        grey_jpeg(&jdaa, (h.w, h.h), "JNG alpha")?
    } else {
        let ihdr = [&h.w.to_be_bytes()[..], &h.h.to_be_bytes(), &[h.alpha_depth, 0, 0, 0, h.alpha_interlace]].concat();
        let png = [b"\x89PNG\r\n\x1a\n".to_vec(), chunk(b"IHDR", &ihdr), idat, chunk(b"IEND", &[])].concat();
        let dec = PngDecoder::new(Cursor::new(&png)).map_err(|e| format!("JNG alpha: {e}"))?;
        DynamicImage::from_decoder(dec).map_err(|e| format!("JNG alpha: {e}"))?.to_luma8()
    };
    let rgb = color.to_rgb8();
    let mut out = RgbaImage::new(h.w, h.h);
    for ((o, c), a) in out.pixels_mut().zip(rgb.pixels()).zip(alpha.pixels()) {
        *o = image::Rgba([c[0], c[1], c[2], a[0]]);
    }
    Ok(DynamicImage::ImageRgba8(out))
}

