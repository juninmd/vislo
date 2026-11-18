//! X Window Dump (`.xwd`): version 7 ZPixmap files with 8-bit colour map or 16/24/32-bit masked pixels.

use image::{DynamicImage, RgbImage};
use std::path::Path;

pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let f = std::fs::read(path).map_err(|e| e.to_string())?;
    let bad = || "not a valid XWD file".to_string();
    let word = |big: bool, at: usize| f.get(at..at + 4).map(|b| if big { u32::from_be_bytes(b.try_into().unwrap()) } else { u32::from_le_bytes(b.try_into().unwrap()) });
    let big = match (word(true, 4), word(false, 4)) {
        (Some(7), _) => true,
        (_, Some(7)) => false,
        _ => return Err(bad()),
    };
    let field = |i: usize| word(big, i * 4).ok_or_else(bad);
    let (header, format, w, h) = (field(0)? as usize, field(2)?, field(4)?, field(5)?);
    let (image_big, bpp, stride, red, green, blue, entries) = (field(7)? == 1, field(11)?, field(12)? as usize, field(14)?, field(15)?, field(16)?, field(19)? as usize);
    if format != 2 {
        return Err("only ZPixmap XWD files are supported".to_string());
    }
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    if !matches!(bpp, 8 | 16 | 24 | 32) || (stride as u64) < u64::from(w) * u64::from(bpp) / 8 || stride == 0 {
        return Err("unsupported XWD pixel layout".to_string());
    }
    let map_at = header;
    let data_at = entries.checked_mul(12).and_then(|n| n.checked_add(map_at)).ok_or_else(bad)?;
    let data = f.get(data_at..data_at.checked_add(stride.checked_mul(h as usize).ok_or_else(bad)?).ok_or_else(bad)?).ok_or("truncated XWD data")?;
    // Colour map entries: pixel value (u32), red, green, blue (u16 each), flags, pad. Indexed once, first match wins.
    let mut table: [Option<[u8; 3]>; 256] = [None; 256];
    for i in 0..entries {
        let e = map_at + i * 12;
        let Some(pixel) = word(big, e).filter(|p| *p < 256) else { continue };
        let c = |o: usize| f.get(e + o..e + o + 2).map_or(0, |b| (if big { u16::from_be_bytes([b[0], b[1]]) } else { u16::from_le_bytes([b[0], b[1]]) } >> 8) as u8);
        table[pixel as usize].get_or_insert([c(4), c(6), c(8)]);
    }
    let channel = |v: u32, mask: u32| -> u8 {
        if mask == 0 {
            return 0;
        }
        let max = (1u64 << mask.count_ones()) - 1;
        (u64::from((v & mask) >> mask.trailing_zeros()) * 255 / max) as u8
    };
    let step = (bpp / 8) as usize;
    let mut img = RgbImage::new(w, h);
    for (y, row) in data.chunks(stride).enumerate() {
        for x in 0..w as usize {
            let b = &row[x * step..x * step + step];
            let v = b.iter().enumerate().fold(0u32, |a, (i, byte)| {
                let shift = if image_big { 8 * (step - 1 - i) } else { 8 * i };
                a | u32::from(*byte) << shift
            });
            let px = if bpp == 8 && entries > 0 { table[v as usize & 255].unwrap_or([v as u8; 3]) } else if bpp == 8 { [v as u8; 3] } else { [channel(v, red), channel(v, green), channel(v, blue)] };
            img.put_pixel(x as u32, y as u32, image::Rgb(px));
        }
    }
    Ok(DynamicImage::ImageRgb8(img))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn file(big: bool, w: u32, h: u32, bpp: u32, image_big: bool, masks: [u32; 3], colormap: &[(u32, [u16; 3])], data: &[u8]) -> Vec<u8> {
        let put = |v: u32| if big { v.to_be_bytes() } else { v.to_le_bytes() };
        let stride = ((u64::from(w) * u64::from(bpp) / 8 + 3) & !3) as u32;
        let fields = [100, 7, 2, 24, w, h, 0, u32::from(image_big), 32, 0, 32, bpp, stride, 4, masks[0], masks[1], masks[2], 8, colormap.len() as u32, colormap.len() as u32, w, h, 0, 0, 0];
        let mut f: Vec<u8> = fields.iter().flat_map(|v| put(*v)).collect();
        for (pixel, [r, g, b]) in colormap {
            f.extend(put(*pixel));
            for c in [r, g, b] {
                f.extend(if big { c.to_be_bytes() } else { c.to_le_bytes() });
            }
            f.extend([0, 0]);
        }
        f.extend(data);
        f.resize(f.len() + 8, 0);
        f
    }

    fn run(bytes: &[u8], max: u64) -> Result<DynamicImage, String> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("vislo-xwd-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("a.xwd"), bytes).unwrap();
        decode(&d.join("a.xwd"), max)
    }

    #[test]
    fn masked_32_bit_pixels_decode_in_both_byte_orders() {
        let masks = [0xff0000, 0x00ff00, 0x0000ff];
        // Pixel value 0x00102030 -> R=0x10 G=0x20 B=0x30, stored most or least significant byte first.
        let be = run(&file(true, 1, 1, 32, true, masks, &[], &[0x00, 0x10, 0x20, 0x30]), 10).unwrap();
        assert_eq!(be.to_rgb8().get_pixel(0, 0).0, [0x10, 0x20, 0x30]);
        let le = run(&file(false, 1, 1, 32, false, masks, &[], &[0x30, 0x20, 0x10, 0x00]), 10).unwrap();
        assert_eq!(le.dimensions(), (1, 1));
        assert_eq!(le.to_rgb8().get_pixel(0, 0).0, [0x10, 0x20, 0x30]);
    }

    #[test]
    fn eight_bit_pixels_go_through_the_colour_map() {
        let map = [(0, [0xffff, 0, 0]), (7, [0, 0xffff, 0])];
        let img = run(&file(true, 2, 1, 8, true, [0; 3], &map, &[7, 0]), 10).unwrap().to_rgb8();
        assert_eq!((img.get_pixel(0, 0).0, img.get_pixel(1, 0).0), ([0, 255, 0], [255, 0, 0]));
    }

    #[test]
    fn narrower_pixel_formats_and_unmapped_indices() {
        // 24-bit big-endian, stride padded to 4: R G B then a pad byte.
        let rgb24 = run(&file(true, 1, 1, 24, true, [0xff0000, 0xff00, 0xff], &[], &[9, 8, 7, 0]), 10).unwrap().to_rgb8();
        assert_eq!(rgb24.get_pixel(0, 0).0, [9, 8, 7]);
        // RGB565, little-endian: 0xF800 is pure red.
        let rgb565 = run(&file(false, 1, 1, 16, false, [0xf800, 0x07e0, 0x001f], &[], &[0x00, 0xf8]), 10).unwrap().to_rgb8();
        assert_eq!(rgb565.get_pixel(0, 0).0, [255, 0, 0]);
        // An index missing from the colour map falls back to grey instead of failing.
        let lone = run(&file(true, 1, 1, 8, true, [0; 3], &[(0, [0xffff; 3])], &[42]), 10).unwrap().to_rgb8();
        assert_eq!(lone.get_pixel(0, 0).0, [42; 3]);
        // A full 32-bit mask must not overflow the normalisation.
        let wide = run(&file(true, 1, 1, 32, true, [0xffff_ffff, 0, 0], &[], &[0xff; 4]), 10).unwrap().to_rgb8();
        assert_eq!(wide.get_pixel(0, 0).0[0], 255);
    }

    #[test]
    fn a_row_length_that_wraps_in_32_bits_is_rejected() {
        // 134 217 728 px x 32 bit = 2^32 bits: w * bpp wrapped to 0 in 32-bit arithmetic and let a zero stride through.
        let mut f = file(true, 134_217_728, 1, 32, true, [0xff0000, 0xff00, 0xff], &[], &[0; 16]);
        f[48..52].copy_from_slice(&0u32.to_be_bytes()); // bytes_per_line
        assert!(run(&f, 200_000_000).unwrap_err().contains("unsupported XWD pixel layout"));
    }

    #[test]
    fn junk_unsupported_truncated_and_huge_files_are_errors() {
        assert!(run(b"not an xwd file at all, no way", 10).is_err());
        let mut xy = file(true, 1, 1, 32, true, [0; 3], &[], &[0; 4]);
        xy[8..12].copy_from_slice(&0u32.to_be_bytes()); // XYBitmap
        assert!(run(&xy, 10).unwrap_err().contains("ZPixmap"));
        let ok = file(true, 4, 4, 32, true, [0xff0000, 0xff00, 0xff], &[], &[0; 16]);
        assert!(run(&ok[..110], 100).is_err(), "pixel data cut short");
        assert_eq!(run(&file(true, 60_000, 60_000, 32, true, [0; 3], &[], &[]), 1_000).unwrap_err(), crate::decode::TOO_LARGE);
    }
}
