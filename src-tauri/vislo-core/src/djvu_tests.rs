//! DjVu checks. The fixtures are Unlicense test files from the djvu.js project; each expected hash is the FNV-1a of
//! the RGB pixels that DjVuLibre's own `ddjvu -format=ppm` renders for them, so a match means the same pixels as the
//! reference implementation (the pure-Rust `djvu-rs` crate does the decoding).

use crate::djvu::decode_bytes;

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3))
}

macro_rules! fixture {
    ($name:ident, $file:literal, $w:expr, $h:expr, $hash:expr, $why:literal) => {
        #[test]
        fn $name() {
            let img = decode_bytes(include_bytes!(concat!("test_data/", $file, ".djvu")).to_vec(), 1 << 24).unwrap();
            assert_eq!((img.width(), img.height()), ($w, $h));
            assert_eq!(fnv(img.to_rgb8().as_raw()), $hash, $why);
        }
    };
}

fixture!(wavelet_background_only, "djvu-chicken", 181, 240, 0x8a6c_9f4a_8450_77ec, "IW44 colour photo, no mask layer");
fixture!(small_wavelet_grey, "djvu-boy", 192, 256, 0x5b60_6749_96b5_18a3, "IW44 with a single luminance plane");
fixture!(bitonal_jb2_mask_page, "djvu-boy-jb2", 192, 256, 0x06fa_3042_b8b2_17f1, "JB2 text-style page over white");
fixture!(scanned_page_in_ccitt_g4, "djvu-ccitt-2", 1728, 2376, 0x59e1_3285_6bb7_43c2, "a full-size 200 dpi scan");

#[test]
fn what_is_not_djvu_is_an_error() {
    assert!(decode_bytes(Vec::new(), 100).is_err());
    assert!(decode_bytes(b"not a djvu file at all".to_vec(), 100).is_err());
    assert!(decode_bytes(b"AT&TFORM\0\0\0\x04DJVU".to_vec(), 100).is_err(), "a container with no page");
}

#[test]
fn the_pixel_cap_applies_before_rendering() {
    let f = include_bytes!("test_data/djvu-ccitt-2.djvu").to_vec();
    assert_eq!(decode_bytes(f, 1_000_000).unwrap_err(), crate::decode::TOO_LARGE);
}

#[test]
fn truncated_and_corrupted_files_are_errors_or_images_never_panics() {
    let full = include_bytes!("test_data/djvu-boy-jb2.djvu");
    for cut in [4, 11, 20, 60, 120, full.len() - 1] {
        let _ = decode_bytes(full[..cut].to_vec(), 1 << 24);
    }
    for at in 0..full.len() {
        for flip in [0x01u8, 0x80, 0xFF] {
            let mut b = full.to_vec();
            b[at] ^= flip;
            let _ = decode_bytes(b, 1 << 24);
        }
    }
    let big = include_bytes!("test_data/djvu-boy.djvu");
    for at in (0..big.len()).step_by(37) {
        let mut b = big.to_vec();
        b[at] ^= 0xFF;
        let _ = decode_bytes(b, 1 << 24);
    }
}

#[test]
fn a_page_that_claims_a_huge_size_is_refused_not_allocated() {
    let mut b = include_bytes!("test_data/djvu-boy-jb2.djvu").to_vec();
    let info = b.windows(4).position(|w| w == b"INFO").unwrap() + 8;
    b[info..info + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]); // 65535 x 65535
    assert!(decode_bytes(b, 150_000_000).is_err());
}
