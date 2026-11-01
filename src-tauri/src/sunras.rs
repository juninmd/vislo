//! Sun Raster (`.ras`, `.sun`): 1, 8, 24 and 32 bit, optional colour map, optional byte-run encoding.

use image::{DynamicImage, RgbImage};
use std::path::Path;

const MAGIC: u32 = 0x59A6_6A95;

fn be(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// Byte-run decoding: 0x80 introduces a run (count - 1, value); a count of 0 is a literal 0x80.
fn unpack_rle(src: &[u8], want: usize) -> Result<Vec<u8>, String> {
    // A run expands 3 bytes into at most 256, so the input bounds what is worth reserving up front.
    let (mut out, mut i) = (Vec::with_capacity(want.min(src.len().saturating_mul(86))), 0);
    while out.len() < want {
        let b = *src.get(i).ok_or("truncated Sun Raster data")?;
        i += 1;
        if b != 0x80 {
            out.push(b);
            continue;
        }
        let n = *src.get(i).ok_or("truncated Sun Raster data")?;
        i += 1;
        if n == 0 {
            out.push(0x80);
        } else {
            let v = *src.get(i).ok_or("truncated Sun Raster data")?;
            i += 1;
            out.extend(std::iter::repeat_n(v, usize::from(n) + 1));
        }
    }
    out.truncate(want);
    Ok(out)
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = || "not a valid Sun Raster file".to_string();
    if be(&f, 0) != Some(MAGIC) {
        return Err(bad());
    }
    let (w, h, depth) = (be(&f, 4).ok_or_else(bad)?, be(&f, 8).ok_or_else(bad)?, be(&f, 12).ok_or_else(bad)?);
    let (kind, map_type, map_len) = (be(&f, 20).ok_or_else(bad)?, be(&f, 24).ok_or_else(bad)?, be(&f, 28).ok_or_else(bad)? as usize);
    if kind > 3 {
        return Err(format!("unsupported Sun Raster type {kind}"));
    }
    if !matches!(depth, 1 | 8 | 24 | 32) {
        return Err(format!("unsupported Sun Raster depth {depth}"));
    }
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let map = f.get(32..32usize.checked_add(map_len).ok_or_else(bad)?).ok_or_else(bad)?;
    let stride = (((w as usize * depth as usize).div_ceil(8)) + 1) & !1; // rows are padded to 16 bits
    let want = stride.checked_mul(h as usize).ok_or_else(bad)?;
    let packed = &f[32 + map_len..];
    let data = if kind == 2 { unpack_rle(packed, want)? } else { packed.get(..want).ok_or("truncated Sun Raster data")?.to_vec() };
    let n = map_len / 3;
    let palette = |v: u8| -> [u8; 3] {
        if map_type == 1 && usize::from(v) < n {
            [map[usize::from(v)], map[n + usize::from(v)], map[2 * n + usize::from(v)]]
        } else {
            [v; 3]
        }
    };
    let rgb_order = kind == 3; // RT_FORMAT_RGB; the standard layouts store blue first
    let mut img = RgbImage::new(w, h);
    for (y, row) in data.chunks(stride).take(h as usize).enumerate() {
        for x in 0..w as usize {
            let px = match depth {
                1 => [if row[x / 8] >> (7 - x % 8) & 1 == 1 { 0 } else { 255 }; 3],
                8 => palette(row[x]),
                24 | 32 => {
                    let at = x * (depth as usize / 8) + usize::from(depth == 32);
                    let c = [row[at], row[at + 1], row[at + 2]];
                    if rgb_order { c } else { [c[2], c[1], c[0]] }
                }
                _ => unreachable!("depth was validated"),
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(px));
        }
    }
    Ok(DynamicImage::ImageRgb8(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn header(w: u32, h: u32, depth: u32, kind: u32, map_type: u32, map_len: u32) -> Vec<u8> {
        let words = [MAGIC, w, h, depth, 0, kind, map_type, map_len];
        words.iter().flat_map(|v| v.to_be_bytes()).collect()
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-sun-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.ras"), bytes).unwrap();
        decode(&d.join("a.ras"), max)
    }

    #[test]
    fn truecolour_is_read_blue_first_and_rows_are_padded_to_16_bits() {
        let mut f = header(1, 2, 24, 1, 0, 0);
        f.extend([30, 20, 10, 0, 60, 50, 40, 0]); // BGR + one pad byte per 3-byte row
        let img = run(&f, 100).unwrap();
        assert_eq!(img.dimensions(), (1, 2));
        assert_eq!(img.to_rgb8().get_pixel(0, 0).0, [10, 20, 30]);
        assert_eq!(img.to_rgb8().get_pixel(0, 1).0, [40, 50, 60]);
    }

    #[test]
    fn palette_and_bitmap_depths_map_to_colours() {
        let mut f = header(2, 1, 8, 1, 1, 6);
        f.extend([255, 0, 0, 255, 0, 0, 0, 1]); // R[2] G[2] B[2] colour map, then one row: red, green
        let img = run(&f, 100).unwrap().to_rgb8();
        assert_eq!((img.get_pixel(0, 0).0, img.get_pixel(1, 0).0), ([255, 0, 0], [0, 255, 0]));
        let mut bits = header(8, 1, 1, 1, 0, 0);
        bits.extend([0b1000_0000, 0]);
        let img = run(&bits, 100).unwrap().to_rgb8();
        assert_eq!((img.get_pixel(0, 0).0[0], img.get_pixel(1, 0).0[0]), (0, 255), "a set bit is black ink");
    }

    #[test]
    fn byte_runs_expand_and_escaped_escape_is_literal() {
        let mut f = header(4, 1, 8, 2, 0, 0);
        f.extend([0x80, 2, 9, 0x80, 0]); // three 9s, then a literal 0x80
        let img = run(&f, 100).unwrap().to_rgb8();
        assert_eq!([img.get_pixel(0, 0).0[0], img.get_pixel(2, 0).0[0], img.get_pixel(3, 0).0[0]], [9, 9, 0x80]);
    }

    #[test]
    fn truncated_runs_and_unsupported_headers_are_errors_without_reserving_the_canvas() {
        let mut cut = header(4, 1, 8, 2, 0, 0);
        cut.extend([0x80]); // an escape with nothing after it
        assert!(run(&cut, 100).unwrap_err().contains("truncated"));
        // 150 Mpx of 32-bit RLE data announced by a file with no data: must fail cleanly, not reserve ~600 MB.
        assert!(run(&header(150_000_000, 1, 32, 2, 0, 0), 150_000_000).unwrap_err().contains("truncated"));
        assert!(run(&header(1, 1, 16, 1, 0, 0), 100).unwrap_err().contains("depth"));
        assert!(run(&header(1, 1, 8, 4, 0, 0), 100).unwrap_err().contains("type"));
        assert!(run(&header(1, 1, 8, 1, 1, 9_999), 100).is_err(), "colour map longer than the file");
    }

    #[test]
    fn junk_truncation_and_huge_headers_are_errors() {
        assert!(run(b"not a sun raster file at all....", 100).is_err());
        let mut cut = header(4, 4, 24, 1, 0, 0);
        cut.extend([0; 10]);
        assert!(run(&cut, 1_000).is_err());
        assert_eq!(run(&header(60_000, 60_000, 24, 1, 0, 0), 1_000_000).unwrap_err(), crate::decode::TOO_LARGE);
    }
}
