//! Photoshop Large Document (`.psb`): the merged picture stored after the layers, 8 or 16 bit grey or RGB(A),
//! raw or PackBits-compressed. The layer data is skipped by its length, never parsed.

use image::{DynamicImage, RgbaImage};
use std::path::Path;

const MAX_FILE: u64 = 1 << 30;

fn be(f: &[u8], at: usize, n: usize) -> Option<u64> {
    Some(f.get(at..at.checked_add(n)?)?.iter().fold(0u64, |a, b| a << 8 | u64::from(*b)))
}

/// One PackBits row, exactly `want` bytes.
fn unpack_row(src: &[u8], want: usize) -> Result<Vec<u8>, String> {
    let (mut out, mut i) = (Vec::with_capacity(want.min(src.len().saturating_mul(128))), 0);
    while out.len() < want {
        let n = *src.get(i).ok_or("truncated PSB data")? as i8;
        i += 1;
        if n >= 0 {
            out.extend_from_slice(src.get(i..i + n as usize + 1).ok_or("truncated PSB data")?);
            i += n as usize + 1;
        } else if n != -128 {
            let v = *src.get(i).ok_or("truncated PSB data")?;
            i += 1;
            out.extend(std::iter::repeat_n(v, (1 - isize::from(n)) as usize));
        }
    }
    out.truncate(want);
    Ok(out)
}

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_FILE {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = || "not a valid PSB file".to_string();
    if f.get(..4) != Some(b"8BPS") || be(&f, 4, 2) != Some(2) {
        return Err(bad());
    }
    let (channels, h, w, depth, mode) = (be(&f, 12, 2).ok_or_else(bad)? as usize, be(&f, 14, 4).ok_or_else(bad)?, be(&f, 18, 4).ok_or_else(bad)?, be(&f, 22, 2).ok_or_else(bad)? as usize, be(&f, 24, 2).ok_or_else(bad)?);
    if w == 0 || h == 0 || w * h > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let colour = match mode {
        1 => 1,
        3 => 3,
        _ => return Err(format!("unsupported PSB colour mode {mode} (only greyscale and RGB)")),
    };
    if !matches!(depth, 8 | 16) || channels < colour || channels > 56 {
        return Err("unsupported PSB layout".to_string());
    }
    // Sections: colour mode data (u32 length), image resources (u32), layers and masks (u64 in PSB).
    let mut at: usize = 26;
    let mut layers = None;
    for len_bytes in [4, 4, 8] {
        let len = be(&f, at, len_bytes).ok_or_else(bad)? as usize;
        layers = Some((at + len_bytes, len)); // ends as the layers and masks section
        at = at.checked_add(len_bytes).and_then(|a| a.checked_add(len)).ok_or_else(bad)?;
    }
    // The first extra channel is transparency only when the layer count is negative (PSD spec); otherwise it is a
    // saved selection or spot colour and must not hide the picture.
    let (start, len) = layers.ok_or_else(bad)?;
    let has_alpha = len >= 10 && be(&f, start + 8, 2).is_some_and(|c| c & 0x8000 != 0);
    let (compression, mut at) = (be(&f, at, 2).ok_or_else(bad)?, at + 2);
    let (w, h) = (w as usize, h as usize);
    let row = w * depth / 8;
    let used = colour + usize::from(has_alpha && channels > colour);
    let mut planes: Vec<Vec<u8>> = Vec::new();
    let counts = at;
    if compression == 1 {
        at += channels * h * 4; // PSB row byte counts are 32 bit
    } else if compression != 0 {
        return Err(format!("unsupported PSB compression {compression}"));
    }
    let mut count_at = counts;
    for _ in 0..used {
        let mut plane = Vec::with_capacity(row.checked_mul(h).ok_or_else(bad)?.min(f.len().saturating_mul(128)));
        for _ in 0..h {
            if compression == 0 {
                plane.extend_from_slice(f.get(at..at + row).ok_or("truncated PSB data")?);
                at += row;
            } else {
                let n = be(&f, count_at, 4).ok_or_else(bad)? as usize;
                count_at += 4;
                plane.extend(unpack_row(f.get(at..at.checked_add(n).ok_or_else(bad)?).ok_or("truncated PSB data")?, row)?);
                at += n;
            }
        }
        planes.push(plane);
    }
    let step = depth / 8;
    let mut img = RgbaImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let s = |c: usize| planes[c][y * row + x * step]; // the high byte of a 16-bit sample is first
            let px = match (colour, used) {
                (1, 1) => [s(0), s(0), s(0), 255],
                (1, _) => [s(0), s(0), s(0), s(1)],
                (_, 3) => [s(0), s(1), s(2), 255],
                _ => [s(0), s(1), s(2), s(3)],
            };
            img.put_pixel(x as u32, y as u32, image::Rgba(px));
        }
    }
    Ok(DynamicImage::ImageRgba8(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn file(w: u32, h: u32, channels: u16, depth: u16, mode: u16, compression: u16, body: &[u8]) -> Vec<u8> {
        file_with_layers(w, h, channels, depth, mode, compression, &[9; 5], body)
    }

    #[allow(clippy::too_many_arguments)]
    fn file_with_layers(w: u32, h: u32, channels: u16, depth: u16, mode: u16, compression: u16, layers: &[u8], body: &[u8]) -> Vec<u8> {
        let mut f = b"8BPS".to_vec();
        f.extend(2u16.to_be_bytes());
        f.extend([0; 6]);
        f.extend(channels.to_be_bytes());
        f.extend(h.to_be_bytes());
        f.extend(w.to_be_bytes());
        f.extend(depth.to_be_bytes());
        f.extend(mode.to_be_bytes());
        f.extend(0u32.to_be_bytes()); // colour mode data
        f.extend(0u32.to_be_bytes()); // image resources
        f.extend((layers.len() as u64).to_be_bytes()); // layers and masks: bytes the reader must skip
        f.extend(layers);
        f.extend(compression.to_be_bytes());
        f.extend(body);
        f
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-psb-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.psb"), bytes).unwrap();
        decode(&d.join("a.psb"), max)
    }

    #[test]
    fn raw_rgb_planes_are_interleaved_and_the_layer_section_is_skipped() {
        // 2x1: R=[10,40] G=[20,50] B=[30,60]
        let img = run(&file(2, 1, 3, 8, 3, 0, &[10, 40, 20, 50, 30, 60]), 10).unwrap();
        assert_eq!(img.dimensions(), (2, 1));
        assert_eq!((img.to_rgb8().get_pixel(0, 0).0, img.to_rgb8().get_pixel(1, 0).0), ([10, 20, 30], [40, 50, 60]));
    }

    #[test]
    fn packbits_rows_use_32_bit_counts_and_sixteen_bit_keeps_the_high_byte() {
        // Grey, 4x1, one PackBits run of four 0x77 (header 0xFD = -3 -> 4 copies), count 2.
        let mut body = 2u32.to_be_bytes().to_vec();
        body.extend([0xFD, 0x77]);
        assert_eq!(run(&file(4, 1, 1, 8, 1, 1, &body), 10).unwrap().to_luma8().get_pixel(3, 0).0[0], 0x77);
        let img = run(&file(1, 1, 1, 16, 1, 0, &[0xAB, 0xCD]), 10).unwrap();
        assert_eq!(img.to_luma8().get_pixel(0, 0).0[0], 0xAB);
    }

    #[test]
    fn a_fourth_channel_is_alpha_only_when_the_layer_count_is_negative() {
        let section = |count: i16| [2u64.to_be_bytes().as_slice(), &count.to_be_bytes()].concat(); // layer info length, count
        let with = |count| run(&file_with_layers(1, 1, 4, 8, 3, 0, &section(count), &[1, 2, 3, 0]), 10).unwrap().to_rgba8().get_pixel(0, 0).0;
        assert_eq!(with(-1), [1, 2, 3, 0], "negative count: the channel is transparency");
        assert_eq!(with(1), [1, 2, 3, 255], "positive count: a selection channel must not hide the picture");
        let none = run(&file(1, 1, 4, 8, 3, 0, &[1, 2, 3, 0]), 10).unwrap().to_rgba8().get_pixel(0, 0).0;
        assert_eq!(none, [1, 2, 3, 255], "no layer section at all");
    }

    #[test]
    fn junk_unsupported_modes_truncation_and_huge_headers_are_errors() {
        assert!(run(b"8BPS\0\x01 plain psd is not handled here", 10).is_err());
        assert!(run(&file(1, 1, 4, 8, 4, 0, &[0; 4]), 10).unwrap_err().contains("colour mode"));
        assert!(run(&file(2, 2, 1, 8, 1, 0, &[0; 2]), 10).unwrap_err().contains("truncated"));
        assert!(run(&file(1, 1, 1, 8, 1, 2, &[0]), 10).unwrap_err().contains("compression"));
        assert_eq!(run(&file(300_000, 300_000, 1, 8, 1, 0, &[]), 1_000_000).unwrap_err(), crate::decode::TOO_LARGE);
    }
}
