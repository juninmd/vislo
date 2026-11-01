//! DICOM (`.dcm`, `.dicom`): the first frame of uncompressed little-endian files, 8/16/32-bit greyscale (with the
//! file's window, rescale and MONOCHROME1 inversion) or 8-bit RGB. Compressed transfer syntaxes are reported as such.

use image::{DynamicImage, GrayImage, RgbImage};
use std::path::Path;

const EXPLICIT_LE: &str = "1.2.840.10008.1.2.1";
const IMPLICIT_LE: &str = "1.2.840.10008.1.2";

#[derive(Default)]
struct Meta {
    rows: u32,
    cols: u32,
    samples: u32,
    bits: u32,
    signed: bool,
    planar: u32,
    photometric: String,
    center: Option<f64>,
    width: Option<f64>,
    slope: f64,
    intercept: f64,
    syntax: String,
    pixels: Option<(usize, usize)>,
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string()
}

fn number(b: &[u8]) -> Option<f64> {
    text(b).split('\\').next()?.trim().parse().ok()
}

/// Tag, value length (`u32::MAX` = undefined) and header size of the element at `at`.
fn header(f: &[u8], at: usize, explicit: bool) -> Option<((u16, u16), u32, usize)> {
    let u16at = |o: usize| f.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    let u32at = |o: usize| f.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let tag = (u16at(at)?, u16at(at + 2)?);
    if !explicit || tag.0 == 0xFFFE {
        return Some((tag, u32at(at + 4)?, 8));
    }
    let vr = f.get(at + 4..at + 6)?;
    if matches!(vr, b"OB" | b"OW" | b"OF" | b"SQ" | b"UT" | b"UN" | b"OD" | b"OL" | b"UC" | b"UR") {
        Some((tag, u32at(at + 8)?, 12))
    } else {
        Some((tag, u32::from(u16at(at + 6)?), 8))
    }
}

/// Skips elements from `at` up to and including the `until` delimiter, descending into nested undefined lengths.
fn skip(f: &[u8], mut at: usize, explicit: bool, depth: u32, until: (u16, u16)) -> Option<usize> {
    if depth > 16 {
        return None;
    }
    loop {
        let (tag, len, hl) = header(f, at, explicit)?;
        if tag == until {
            return Some(at + 8);
        }
        let body = at.checked_add(hl)?;
        at = match (tag, len) {
            ((0xFFFE, 0xE000), u32::MAX) => skip(f, body, explicit, depth + 1, (0xFFFE, 0xE00D))?, // item of unknown length
            ((_, _), u32::MAX) => skip(f, body, explicit, depth + 1, (0xFFFE, 0xE0DD))?, // sequence of unknown length
            _ => body.checked_add(len as usize)?,
        };
    }
}

fn parse(f: &[u8]) -> Result<Meta, String> {
    let bad = || "not a valid DICOM file".to_string();
    if f.get(128..132) != Some(b"DICM") {
        return Err(bad());
    }
    let mut m = Meta { samples: 1, slope: 1.0, ..Meta::default() };
    let (mut at, mut explicit) = (132usize, true);
    for _ in 0..20_000 {
        let Some(((group, elem), len, hl)) = header(f, at, explicit) else { break };
        if group != 0x0002 {
            // The meta group is always explicit; the transfer syntax it names governs the rest.
            if !m.syntax.is_empty() && m.syntax != EXPLICIT_LE && m.syntax != IMPLICIT_LE {
                return Err(format!("unsupported DICOM transfer syntax {} (only uncompressed little-endian)", m.syntax));
            }
            explicit = m.syntax != IMPLICIT_LE;
        }
        let (_, len, hl) = if group != 0x0002 { header(f, at, explicit).ok_or_else(bad)? } else { ((group, elem), len, hl) };
        let body = at.checked_add(hl).ok_or_else(bad)?;
        if (group, elem) == (0x7FE0, 0x0010) {
            let end = if len == u32::MAX { f.len() } else { body.checked_add(len as usize).ok_or_else(bad)? };
            m.pixels = Some((body, end.min(f.len())));
            break;
        }
        if len == u32::MAX {
            at = skip(f, body, explicit, 0, (0xFFFE, 0xE0DD)).ok_or_else(bad)?;
            continue;
        }
        let value = f.get(body..body.checked_add(len as usize).ok_or_else(bad)?).ok_or_else(bad)?;
        let u16v = || value.get(..2).map(|b| u32::from(u16::from_le_bytes([b[0], b[1]])));
        match (group, elem) {
            (0x0002, 0x0010) => m.syntax = text(value),
            (0x0028, 0x0002) => m.samples = u16v().unwrap_or(1),
            (0x0028, 0x0004) => m.photometric = text(value),
            (0x0028, 0x0006) => m.planar = u16v().unwrap_or(0),
            (0x0028, 0x0010) => m.rows = u16v().unwrap_or(0),
            (0x0028, 0x0011) => m.cols = u16v().unwrap_or(0),
            (0x0028, 0x0100) => m.bits = u16v().unwrap_or(0),
            (0x0028, 0x0103) => m.signed = u16v() == Some(1),
            (0x0028, 0x1050) => m.center = number(value),
            (0x0028, 0x1051) => m.width = number(value),
            (0x0028, 0x1052) => m.intercept = number(value).unwrap_or(0.0),
            (0x0028, 0x1053) => m.slope = number(value).unwrap_or(1.0),
            _ => {}
        }
        at = body + len as usize;
    }
    if !m.syntax.is_empty() && m.syntax != EXPLICIT_LE && m.syntax != IMPLICIT_LE {
        return Err(format!("unsupported DICOM transfer syntax {} (only uncompressed little-endian)", m.syntax));
    }
    Ok(m)
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    let m = parse(&f)?;
    let (w, h) = (m.cols, m.rows);
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let (start, end) = m.pixels.ok_or("no pixel data in this DICOM file")?;
    let (n, depth) = ((w * h) as usize, (m.bits / 8) as usize);
    let rgb = m.samples == 3 && m.photometric == "RGB" && m.bits == 8;
    let grey = m.samples == 1 && m.photometric.starts_with("MONOCHROME") && matches!(m.bits, 8 | 16 | 32);
    if !rgb && !grey {
        return Err(format!("unsupported DICOM layout ({} samples, {} bits, {})", m.samples, m.bits, m.photometric));
    }
    let need = n * if rgb { 3 } else { depth };
    let data = f.get(start..end).filter(|d| d.len() >= need).ok_or("truncated DICOM pixel data")?;
    if rgb {
        let mut img = RgbImage::new(w, h);
        for i in 0..n {
            let at = |c: usize| if m.planar == 1 { data[c * n + i] } else { data[i * 3 + c] };
            img.put_pixel((i as u32) % w, (i as u32) / w, image::Rgb([at(0), at(1), at(2)]));
        }
        return Ok(DynamicImage::ImageRgb8(img));
    }
    let value = |i: usize| -> f64 {
        let v = if depth == 1 {
            f64::from(data[i])
        } else if depth == 4 {
            let b = [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]];
            if m.signed { f64::from(i32::from_le_bytes(b)) } else { f64::from(u32::from_le_bytes(b)) }
        } else if m.signed { f64::from(i16::from_le_bytes([data[i * 2], data[i * 2 + 1]])) } else { f64::from(u16::from_le_bytes([data[i * 2], data[i * 2 + 1]])) };
        v * m.slope + m.intercept
    };
    let (lo, hi) = match (m.center, m.width) {
        (Some(c), Some(width)) if width >= 1.0 => (c - 0.5 - (width - 1.0) / 2.0, c - 0.5 + (width - 1.0) / 2.0),
        _ => (0..n).map(value).fold((f64::MAX, f64::MIN), |(lo, hi), v| (lo.min(v), hi.max(v))),
    };
    let span = if hi > lo { hi - lo } else { 1.0 };
    let invert = m.photometric == "MONOCHROME1";
    let mut img = GrayImage::new(w, h);
    for i in 0..n {
        let g = ((value(i) - lo) / span * 255.0).clamp(0.0, 255.0) as u8;
        img.put_pixel((i as u32) % w, (i as u32) / w, image::Luma([if invert { 255 - g } else { g }]));
    }
    Ok(DynamicImage::ImageLuma8(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn el(group: u16, elem: u16, vr: &str, value: &[u8]) -> Vec<u8> {
        let mut v = value.to_vec();
        if v.len() % 2 == 1 {
            v.push(if vr == "UI" { 0 } else { b' ' });
        }
        let mut out = [group.to_le_bytes(), elem.to_le_bytes()].concat();
        if matches!(vr, "OB" | "OW") {
            out.extend(vr.as_bytes());
            out.extend([0, 0]);
            out.extend((v.len() as u32).to_le_bytes());
        } else {
            out.extend(vr.as_bytes());
            out.extend((v.len() as u16).to_le_bytes());
        }
        out.extend(v);
        out
    }

    fn us(group: u16, elem: u16, v: u16) -> Vec<u8> {
        el(group, elem, "US", &v.to_le_bytes())
    }

    fn file(syntax: &str, photometric: &str, samples: u16, bits: u16, w: u16, h: u16, extra: &[Vec<u8>], pixels: &[u8]) -> Vec<u8> {
        let mut f = vec![0u8; 128];
        f.extend(b"DICM");
        f.extend(el(0x0002, 0x0010, "UI", syntax.as_bytes()));
        for e in [us(0x0028, 0x0002, samples), el(0x0028, 0x0004, "CS", photometric.as_bytes()), us(0x0028, 0x0010, h), us(0x0028, 0x0011, w), us(0x0028, 0x0100, bits)] {
            f.extend(e);
        }
        f.extend(extra.concat());
        f.extend(el(0x7FE0, 0x0010, "OW", pixels));
        f
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-dcm-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.dcm"), bytes).unwrap();
        decode(&d.join("a.dcm"), max)
    }

    #[test]
    fn sixteen_bit_greyscale_is_windowed_and_monochrome1_is_inverted() {
        let px: Vec<u8> = [0u16, 500, 1000, 4000].iter().flat_map(|v| v.to_le_bytes()).collect();
        let win = [el(0x0028, 0x1050, "DS", b"500"), el(0x0028, 0x1051, "DS", b"1000")];
        let img = run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 16, 2, 2, &win, &px), 10).unwrap();
        assert_eq!(img.dimensions(), (2, 2));
        let g = img.to_luma8();
        assert_eq!((g.get_pixel(0, 0).0[0], g.get_pixel(1, 1).0[0]), (0, 255), "values outside the window clip");
        assert!((120..136).contains(&g.get_pixel(1, 0).0[0]), "window centre is mid-grey");
        let inv = run(&file(EXPLICIT_LE, "MONOCHROME1", 1, 16, 2, 2, &win, &px), 10).unwrap().to_luma8();
        assert_eq!((inv.get_pixel(0, 0).0[0], inv.get_pixel(1, 1).0[0]), (255, 0));
        let auto = run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 16, 2, 2, &[], &px), 10).unwrap().to_luma8();
        assert_eq!((auto.get_pixel(0, 0).0[0], auto.get_pixel(1, 1).0[0]), (0, 255), "without a window the data range is used");
    }

    #[test]
    fn implicit_vr_little_endian_files_are_parsed_without_value_representations() {
        let imp = |g: u16, e: u16, v: &[u8]| [g.to_le_bytes().as_slice(), &e.to_le_bytes(), &(v.len() as u32).to_le_bytes(), v].concat();
        let mut f = vec![0u8; 128];
        f.extend(b"DICM");
        f.extend(el(0x0002, 0x0010, "UI", IMPLICIT_LE.as_bytes())); // the meta group stays explicit
        for e in [imp(0x0028, 0x0002, &1u16.to_le_bytes()), imp(0x0028, 0x0004, b"MONOCHROME2 "), imp(0x0028, 0x0010, &1u16.to_le_bytes()), imp(0x0028, 0x0011, &2u16.to_le_bytes()), imp(0x0028, 0x0100, &8u16.to_le_bytes()), imp(0x7FE0, 0x0010, &[0, 200])] {
            f.extend(e);
        }
        let g = run(&f, 10).unwrap().to_luma8();
        assert_eq!((g.get_pixel(0, 0).0[0], g.get_pixel(1, 0).0[0]), (0, 255));
    }

    #[test]
    fn sequences_of_defined_and_undefined_length_are_skipped() {
        let item_end = [0xFEu8, 0xFF, 0x0D, 0xE0, 0, 0, 0, 0];
        let seq_end = [0xFEu8, 0xFF, 0xDD, 0xE0, 0, 0, 0, 0];
        let item_open = [0xFEu8, 0xFF, 0x00, 0xE0, 0xFF, 0xFF, 0xFF, 0xFF];
        let sq = |items: Vec<u8>| [vec![0x08, 0x00, 0x40, 0x11, b'S', b'Q', 0, 0, 0xFF, 0xFF, 0xFF, 0xFF], items, seq_end.to_vec()].concat();
        let nested = [item_open.to_vec(), el(0x0008, 0x1150, "UI", b"1.2"), sq([item_open.to_vec(), item_end.to_vec()].concat()), item_end.to_vec()].concat();
        let empty_item = [item_open.to_vec(), item_end.to_vec()].concat();
        let defined = [vec![0xFE, 0xFF, 0x00, 0xE0, 10, 0, 0, 0], el(0x0008, 0x0100, "SH", b"AB")].concat(); // 10 bytes of item data
        let px = [0u8, 200];
        for items in [nested, empty_item, defined] {
            let extra = [sq(items)];
            let g = run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 8, 2, 1, &extra, &px), 10).unwrap().to_luma8();
            assert_eq!((g.get_pixel(0, 0).0[0], g.get_pixel(1, 0).0[0]), (0, 255));
        }
    }

    #[test]
    fn rescale_and_signed_data_are_applied_before_windowing() {
        // Stored -1000..1000 with slope 2: the data range after rescale is -2000..2000.
        let px: Vec<u8> = [-1000i16, 0, 1000, 1000].iter().flat_map(|v| v.to_le_bytes()).collect();
        let extra = [us(0x0028, 0x0103, 1), el(0x0028, 0x1053, "DS", b"2")];
        let g = run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 16, 2, 2, &extra, &px), 10).unwrap().to_luma8();
        assert_eq!((g.get_pixel(0, 0).0[0], g.get_pixel(1, 1).0[0]), (0, 255));
        assert!((125..131).contains(&g.get_pixel(1, 0).0[0]), "stored zero sits mid-range only if the sign was honoured");
    }

    #[test]
    fn thirty_two_bit_data_such_as_rt_dose_is_windowed_like_the_rest() {
        let px: Vec<u8> = [0u32, 1_000_000, 4_000_000_000, 2_000_000_000].iter().flat_map(|v| v.to_le_bytes()).collect();
        let g = run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 32, 2, 2, &[], &px), 10).unwrap().to_luma8();
        assert_eq!((g.get_pixel(0, 0).0[0], g.get_pixel(0, 1).0[0]), (0, 255), "values above i32::MAX must not wrap negative");
        assert!((125..131).contains(&g.get_pixel(1, 1).0[0]));
    }

    #[test]
    fn big_endian_syntax_is_named_not_reported_as_corrupt() {
        let be = file("1.2.840.10008.1.2.2", "MONOCHROME2", 1, 8, 1, 1, &[], &[0, 0]);
        assert!(run(&be, 10).unwrap_err().contains("transfer syntax"));
    }

    #[test]
    fn rgb_interleaved_and_planar_layouts() {
        let inter = run(&file(EXPLICIT_LE, "RGB", 3, 8, 2, 1, &[us(0x0028, 0x0006, 0)], &[1, 2, 3, 4, 5, 6]), 10).unwrap().to_rgb8();
        assert_eq!(inter.get_pixel(1, 0).0, [4, 5, 6]);
        let planar = run(&file(EXPLICIT_LE, "RGB", 3, 8, 2, 1, &[us(0x0028, 0x0006, 1)], &[1, 4, 2, 5, 3, 6]), 10).unwrap().to_rgb8();
        assert_eq!(planar.get_pixel(1, 0).0, [4, 5, 6]);
    }

    #[test]
    fn compressed_junk_truncated_and_huge_files_are_errors() {
        let jpeg = file("1.2.840.10008.1.2.4.50", "MONOCHROME2", 1, 8, 1, 1, &[], &[0, 0]);
        assert!(run(&jpeg, 10).unwrap_err().contains("transfer syntax"));
        assert!(run(b"definitely not dicom", 10).is_err());
        assert!(run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 16, 4, 4, &[], &[0; 4]), 100).unwrap_err().contains("truncated"));
        assert_eq!(run(&file(EXPLICIT_LE, "MONOCHROME2", 1, 16, 60_000, 60_000, &[], &[]), 1_000).unwrap_err(), crate::decode::TOO_LARGE);
        assert!(run(&file(EXPLICIT_LE, "YBR_FULL", 3, 8, 1, 1, &[], &[0; 3]), 10).unwrap_err().contains("unsupported DICOM layout"));
    }
}
