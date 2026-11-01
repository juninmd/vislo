//! FITS (`.fits`, `.fit`, `.fts`): the first 2-D image of the file as greyscale, stretched between the 0.5th and
//! 99.5th percentile because raw sensor ranges (and their hot pixels) would otherwise render nearly black.

use image::{DynamicImage, GrayImage};
use std::path::Path;

const BLOCK: usize = 2880;

struct Hdu {
    bitpix: i32,
    axes: Vec<usize>,
    bzero: f64,
    bscale: f64,
    data: usize,
    image: bool,
}

fn card<'a>(cards: &'a [u8], key: &str) -> Option<&'a str> {
    cards.chunks(80).find_map(|c| {
        let text = std::str::from_utf8(c).ok()?;
        let (k, rest) = text.split_once('=')?;
        (k.trim() == key).then(|| rest.split('/').next().unwrap_or("").trim())
    })
}

/// Parses the header at `at`; returns it with the byte length of its data (before block padding).
fn header(f: &[u8], at: usize) -> Result<(Hdu, usize), String> {
    let bad = || "not a valid FITS file".to_string();
    let is_end = |i: usize| f.get(i..i + 80).is_some_and(|c| c.starts_with(b"END") && c[3..].iter().all(|b| *b == b' '));
    let end = (at..f.len()).step_by(80).find(|&i| is_end(i)).ok_or_else(bad)?;
    let cards = &f[at..end];
    let num = |k: &str| card(cards, k).and_then(|v| v.parse::<f64>().ok());
    let bitpix = num("BITPIX").ok_or_else(bad)? as i32;
    let naxis = num("NAXIS").ok_or_else(bad)? as usize;
    if naxis > 8 || !matches!(bitpix, 8 | 16 | 32 | -32 | -64) {
        return Err(bad());
    }
    let axes = (1..=naxis).map(|i| num(&format!("NAXIS{i}")).map(|v| v as usize).ok_or_else(bad)).collect::<Result<Vec<_>, _>>()?;
    let bytes = if axes.is_empty() { 0 } else { axes.iter().try_fold(bitpix.unsigned_abs() as usize / 8, |a, n| a.checked_mul(*n)).ok_or_else(bad)? };
    // Standard length: |BITPIX|/8 * GCOUNT * (PCOUNT + product of the axes), the same for primary and extension HDUs.
    let (pcount, gcount) = (num("PCOUNT").unwrap_or(0.0) as usize, num("GCOUNT").unwrap_or(1.0) as usize);
    let bytes = bytes.checked_add(pcount.saturating_mul(bitpix.unsigned_abs() as usize / 8)).and_then(|b| b.checked_mul(gcount.max(1))).ok_or_else(bad)?;
    let data = (end + 80 - at).div_ceil(BLOCK).checked_mul(BLOCK).and_then(|n| n.checked_add(at)).ok_or_else(bad)?;
    let image = card(cards, "XTENSION").is_none_or(|x| x.trim_matches('\'').trim() == "IMAGE");
    Ok((Hdu { bitpix, axes, bzero: num("BZERO").unwrap_or(0.0), bscale: num("BSCALE").unwrap_or(1.0), data, image }, bytes))
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    if !f.starts_with(b"SIMPLE") {
        return Err("not a valid FITS file".to_string());
    }
    let mut at = 0;
    for _ in 0..16 {
        let (hdu, bytes) = header(&f, at)?;
        if hdu.image && hdu.axes.len() >= 2 && hdu.axes[0] > 0 && hdu.axes[1] > 0 {
            return render(&f, &hdu, max_pixels);
        }
        at = bytes.div_ceil(BLOCK).checked_mul(BLOCK).and_then(|n| n.checked_add(hdu.data)).ok_or("not a valid FITS file")?;
        if at >= f.len() {
            break;
        }
    }
    Err("no 2-D image in this FITS file".to_string())
}

fn render(f: &[u8], hdu: &Hdu, max_pixels: u64) -> Result<DynamicImage, String> {
    let (w, h) = (hdu.axes[0], hdu.axes[1]);
    if (w as u64).saturating_mul(h as u64) > max_pixels || w > u32::MAX as usize || h > u32::MAX as usize {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let size = hdu.bitpix.unsigned_abs() as usize / 8;
    let raw = f.get(hdu.data..hdu.data.saturating_add(w * h * size)).ok_or("truncated FITS data")?;
    let at = |i: usize| -> f64 {
        let b = &raw[i * size..(i + 1) * size];
        let v = match hdu.bitpix {
            8 => f64::from(b[0]),
            16 => f64::from(i16::from_be_bytes([b[0], b[1]])),
            32 => f64::from(i32::from_be_bytes([b[0], b[1], b[2], b[3]])),
            -32 => f64::from(f32::from_be_bytes([b[0], b[1], b[2], b[3]])),
            _ => f64::from_be_bytes(b.try_into().unwrap()),
        };
        v * hdu.bscale + hdu.bzero
    };
    // Percentiles from at most ~1M evenly spaced samples; NaN (blank) pixels are ignored.
    let step = (w * h / 1_000_000).max(1);
    let mut sample: Vec<f64> = (0..w * h).step_by(step).map(at).filter(|v| v.is_finite()).collect();
    sample.sort_by(f64::total_cmp);
    let pick = |q: f64| sample.get(((sample.len() as f64 - 1.0) * q) as usize).copied().unwrap_or(0.0);
    let (lo, hi) = (pick(0.005), pick(0.995));
    let span = if hi > lo { hi - lo } else { 1.0 };
    let mut img = GrayImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let v = at(y * w + x);
            let g = if v.is_finite() { ((v - lo) / span * 255.0).clamp(0.0, 255.0) } else { 0.0 };
            img.put_pixel(x as u32, (h - 1 - y) as u32, image::Luma([g as u8])); // FITS rows run bottom-up
        }
    }
    Ok(DynamicImage::ImageLuma8(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn pad(cards: &[String]) -> Vec<u8> {
        let mut h: Vec<u8> = cards.iter().flat_map(|c| format!("{c:<80}").into_bytes()).collect();
        h.extend(format!("{:<80}", "END").into_bytes());
        h.resize(h.len().div_ceil(BLOCK) * BLOCK, b' ');
        h
    }

    fn file(bitpix: i32, w: usize, h: usize, data: Vec<u8>) -> Vec<u8> {
        let mut f = pad(&["SIMPLE  =                    T".into(), format!("BITPIX  = {bitpix:>20}"), "NAXIS   =                    2".into(), format!("NAXIS1  = {w:>20}"), format!("NAXIS2  = {h:>20}")]);
        f.extend(data);
        f.resize(f.len().div_ceil(BLOCK) * BLOCK, 0);
        f
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-fits-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.fits"), bytes).unwrap();
        decode(&d.join("a.fits"), max)
    }

    #[test]
    fn sixteen_bit_data_is_stretched_and_flipped_to_top_down() {
        // Bottom row first: [0, 1000] then the top row [1000, 0].
        let data: Vec<u8> = [0i16, 1000, 1000, 0].iter().flat_map(|v| v.to_be_bytes()).collect();
        let img = run(&file(16, 2, 2, data), 100).unwrap();
        assert_eq!(img.dimensions(), (2, 2));
        let g = img.to_luma8();
        assert_eq!((g.get_pixel(0, 0).0[0], g.get_pixel(1, 0).0[0]), (255, 0), "the file's last row is the top of the picture");
        assert_eq!((g.get_pixel(0, 1).0[0], g.get_pixel(1, 1).0[0]), (0, 255));
    }

    #[test]
    fn float_data_with_nan_blanks_still_renders() {
        let data: Vec<u8> = [0.0f32, f32::NAN, 1.0, 0.5].iter().flat_map(|v| v.to_be_bytes()).collect();
        let g = run(&file(-32, 2, 2, data), 100).unwrap().to_luma8();
        assert_eq!(g.get_pixel(1, 1).0[0], 0, "a blank (NaN) pixel is black, not part of the stretch");
        assert_eq!((g.get_pixel(0, 1).0[0], g.get_pixel(0, 0).0[0]), (0, 255), "0.0 and 1.0 span the full range");
    }

    #[test]
    fn tables_are_skipped_and_the_image_extension_behind_them_is_found() {
        let mut f = pad(&["SIMPLE  =                    T".into(), "BITPIX  =                    8".into(), "NAXIS   =                    0".into()]);
        // A binary table with 8 bytes of rows and a 4 byte heap, then an IMAGE extension.
        f.extend(pad(&["XTENSION= 'BINTABLE'".into(), "BITPIX  =                    8".into(), "NAXIS   =                    2".into(), "NAXIS1  =                    4".into(), "NAXIS2  =                    2".into(), "PCOUNT  =                    4".into(), "GCOUNT  =                    1".into()]));
        f.extend(vec![0u8; BLOCK]);
        f.extend(pad(&["XTENSION= 'IMAGE   '".into(), "BITPIX  =                    8".into(), "NAXIS   =                    2".into(), "NAXIS1  =                    3".into(), "NAXIS2  =                    1".into()]));
        f.extend([0u8, 128, 255]);
        f.resize(f.len().div_ceil(BLOCK) * BLOCK, 0);
        assert_eq!(run(&f, 100).unwrap().dimensions(), (3, 1));
        let mut only_table = f.clone();
        only_table.truncate(BLOCK * 3);
        only_table[BLOCK * 2..].fill(0);
        assert!(run(&only_table, 100).is_err(), "a table alone is not a picture");
    }

    #[test]
    fn absurd_axis_lengths_do_not_overflow() {
        let f = pad(&["SIMPLE  =                    T".into(), "BITPIX  =                    8".into(), "NAXIS   =                    1".into(), "NAXIS1  =  1000000000000000000000000000000".into()]);
        assert!(run(&f, 100).is_err());
    }

    #[test]
    fn junk_truncation_and_huge_headers_are_errors() {
        assert!(run(b"definitely not fits", 100).is_err());
        let full = file(16, 4, 4, vec![0; 32]);
        assert!(run(&full[..BLOCK + 8], 100).is_err(), "header promises 32 bytes of data, 8 are left");
        assert_eq!(run(&file(8, 50_000, 50_000, vec![]), 1_000_000).unwrap_err(), crate::decode::TOO_LARGE);
    }
}
