//! IFF checks. The `iff-*` fixtures come from an independent ILBM writer; each `.raw` is ffmpeg's decode of the same
//! file, so a match means the same pixels as a third-party decoder, not just the same size.

use crate::iff::decode_bytes;

macro_rules! fixture {
    ($name:ident, $file:literal, $why:literal) => {
        #[test]
        fn $name() {
            let img = decode_bytes(include_bytes!(concat!("test_data/", $file, ".iff")), 1 << 20).unwrap();
            assert_eq!((img.width(), img.height()), (37, 9));
            assert_eq!(img.to_rgb8().into_raw(), include_bytes!(concat!("test_data/", $file, ".raw")), $why);
        }
    };
}

fixture!(one_plane, "iff-bitmap1", "1 plane, raw rows, width not a multiple of 16");
fixture!(four_planes_with_byte_run_per_plane, "iff-indexed4-packed", "ByteRun1 restarts at every plane scan line");
fixture!(five_planes_raw, "iff-indexed5-raw", "uncompressed rows");
fixture!(eight_planes_packed, "iff-indexed8-packed", "256-colour palette");
fixture!(mask_plane_is_skipped, "iff-mask-plane", "masking=1 adds a plane that is not a colour bit");
fixture!(extra_half_brite, "iff-ehb", "colours 32..63 are the first 32 at half brightness");
fixture!(ham6, "iff-ham6", "two control bits, four colour bits, held colour per row");
fixture!(ham8, "iff-ham8", "two control bits, six colour bits");
fixture!(true_colour_24_planes, "iff-rgb24", "red planes first, then green, then blue");
fixture!(chunky_pbm_with_odd_width_padding, "iff-pbm-packed", "PBM rows are padded to an even length");

#[test]
fn what_is_not_an_iff_picture_is_an_error() {
    assert!(decode_bytes(b"", 100).is_err());
    assert!(decode_bytes(b"FORM\0\0\0\x04AIFF", 100).is_err(), "an IFF container that is not a picture");
    assert!(decode_bytes(b"FORM\0\0\0\x04ILBM", 100).unwrap_err().contains("BMHD"));
}

#[test]
fn truncated_and_corrupted_files_are_errors_not_panics() {
    let full = include_bytes!("test_data/iff-indexed4-packed.iff");
    for cut in [11, 20, 40, 80, full.len() - 6] {
        assert!(decode_bytes(&full[..cut], 1 << 20).is_err(), "cut at {cut}");
    }
    for name in [&include_bytes!("test_data/iff-ham6.iff")[..], &include_bytes!("test_data/iff-pbm-packed.iff")[..], full] {
        for at in 0..name.len() {
            for flip in [0x01u8, 0x80, 0xFF] {
                let mut b = name.to_vec();
                b[at] ^= flip;
                let _ = decode_bytes(&b, 1 << 20);
            }
        }
    }
}

#[test]
fn a_frame_bigger_than_the_file_can_hold_is_refused_early() {
    let mut b = include_bytes!("test_data/iff-indexed4-packed.iff").to_vec();
    let at = b.windows(4).position(|w| w == b"BMHD").unwrap() + 8;
    b[at..at + 4].copy_from_slice(&[0x7F, 0xFF, 0x7F, 0xFF]); // 32767 x 32767
    assert_eq!(decode_bytes(&b, 150_000_000).unwrap_err(), crate::decode::TOO_LARGE);
    b[at..at + 4].copy_from_slice(&[0x10, 0x00, 0x10, 0x00]); // 4096 x 4096 fits the cap but not this small file
    assert!(decode_bytes(&b, 150_000_000).unwrap_err().contains("truncated"));
}
