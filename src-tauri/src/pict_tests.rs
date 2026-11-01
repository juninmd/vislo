//! PICT checks. The `pict-*` fixtures were written by an independent encoder built from the format description,
//! each with the pixels it was made from; `pict-rose-direct32` is a real ImageMagick-made file (32-bit, packed).

use crate::pict::decode_bytes;

fn run(bytes: &[u8], max: u64) -> Result<image::DynamicImage, String> {
    decode_bytes(bytes, max)
}

macro_rules! fixture {
    ($name:ident, $file:literal, $w:expr, $h:expr, $why:literal) => {
        #[test]
        fn $name() {
            let img = run(include_bytes!(concat!("test_data/", $file, ".pct")), 1 << 20).unwrap();
            assert_eq!((img.width(), img.height()), ($w, $h));
            assert_eq!(img.to_rgb8().into_raw(), include_bytes!(concat!("test_data/", $file, ".raw")), $why);
        }
    };
}

fixture!(indexed_8_bit_through_the_colour_table, "pict-indexed8", 13, 7, "after a clip and an odd-length text op, so the pad byte matters");
fixture!(indexed_4_bit, "pict-indexed4", 17, 5, "two pixels per byte, odd width");
fixture!(indexed_2_bit_with_a_mask_region, "pict-indexed2-rgn", 9, 6, "PackBitsRgn carries a region after the transfer mode");
fixture!(indexed_1_bit, "pict-indexed1", 19, 4, "1-bit PixMap with its own colour table");
fixture!(plain_bitmap_where_one_is_black, "pict-bitmap1", 21, 5, "BitsRect without a PixMap: no palette, 1 = black");
fixture!(direct_32_bit_in_planes_with_two_byte_row_counts, "pict-direct32", 100, 4, "row counts are 2 bytes when the unpacked row exceeds 250");
fixture!(direct_16_bit_in_two_byte_units, "pict-direct16", 11, 5, "1-5-5-5 pixels, PackBits over 16-bit units");

#[test]
fn a_real_imagemagick_picture_has_the_rose_colours() {
    let img = run(include_bytes!("test_data/pict-rose-direct32.pct"), 100_000).unwrap().to_rgb8();
    assert_eq!(img.dimensions(), (70, 46));
    let mean = |c: usize| img.pixels().map(|p| f64::from(p[c])).sum::<f64>() / 3220.0;
    // The same picture stored as a JNG decodes to 145.7 / 89.0 / 81.0 (lossy), so this is a cross-format check.
    for (c, want) in [145.7, 89.3, 80.5].into_iter().enumerate() {
        assert!((mean(c) - want).abs() < 0.5, "channel {c}: {} vs {want}", mean(c));
    }
}

#[test]
fn the_512_byte_lead_in_is_optional() {
    let bare = include_bytes!("test_data/pict-indexed4.pct");
    let with = [vec![0u8; 512], bare.to_vec()].concat();
    assert_eq!(run(&with, 1 << 20).unwrap().to_rgb8(), run(bare, 1 << 20).unwrap().to_rgb8());
}

#[test]
fn version_1_bitmaps_are_read() {
    let rect = [0, 0, 0, 0, 0, 2, 0, 8]; // top 0, left 0, bottom 2, right 8
    let v1 = [&[0u8, 0][..], &rect, &[0x11, 0x01, 0x90, 0, 1], &rect, &rect, &rect, &[0, 1, 0xA5, 0x0F, 0xFF]].concat();
    let img = run(&v1, 100).unwrap().to_luma8();
    assert_eq!(img.dimensions(), (8, 2));
    assert_eq!(img.as_raw()[..8], [0, 255, 0, 255, 255, 0, 255, 0], "0xA5: bits set are black");
}

#[test]
fn what_cannot_be_shown_says_so() {
    let vector_only = [&[0u8; 10][..], &[0x00, 0x11, 0x02, 0xFF, 0x0C, 0x00], &[0; 24], &[0x00, 0x00, 0x00, 0xFF]].concat();
    assert!(run(&vector_only, 100).unwrap_err().contains("drawing commands"));
    let qt = [&[0u8; 10][..], &[0x00, 0x11, 0x02, 0xFF, 0x0C, 0x00], &[0; 24], &[0x82, 0x00, 0, 0, 0, 0]].concat();
    assert!(run(&qt, 100).unwrap_err().contains("QuickTime"));
    assert!(run(b"definitely not a picture file", 100).is_err());
    assert!(run(&[], 100).is_err());
}

#[test]
fn truncated_and_corrupt_files_are_errors_not_panics() {
    let full = include_bytes!("test_data/pict-direct32.pct");
    for cut in [5, 12, 40, 100, 200, full.len() - 3] {
        assert!(run(&full[..cut], 1 << 20).is_err(), "cut at {cut}");
    }
    for name in [&include_bytes!("test_data/pict-indexed8.pct")[..], &include_bytes!("test_data/pict-direct16.pct")[..]] {
        for at in 0..name.len() {
            for flip in [0x01u8, 0x80, 0xFF] {
                let mut b = name.to_vec();
                b[at] ^= flip;
                let _ = run(&b, 1 << 20);
            }
        }
    }
}

#[test]
fn a_huge_frame_is_refused_before_allocating() {
    let mut b = include_bytes!("test_data/pict-indexed8.pct").to_vec();
    let at = b.windows(2).position(|w| w == [0x00, 0x98]).unwrap() + 2;
    b[at + 6..at + 10].copy_from_slice(&[0x7F, 0xFF, 0x7F, 0xFF]); // bottom and right of the PixMap bounds
    assert_eq!(run(&b, 150_000_000).unwrap_err(), crate::decode::TOO_LARGE);
}
