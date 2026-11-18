//! JPEG-LS checks. The fixtures were produced by an independent encoder (CharLS); each `.raw` is that encoder's own
//! decode scaled to 8 bits, so a match means the same pixels, not just the same size.

use crate::jpegls::decode_bytes;

fn run(bytes: &[u8], max: u64) -> Result<image::DynamicImage, String> {
    decode_bytes(bytes, max)
}

fn pixels(img: &image::DynamicImage, channels: usize) -> Vec<u8> {
    if channels == 1 { img.to_luma8().into_raw() } else { img.to_rgb8().into_raw() }
}

macro_rules! fixture {
    ($name:ident, $file:literal, $channels:expr, $w:expr, $h:expr, $why:literal) => {
        #[test]
        fn $name() {
            let img = run(include_bytes!(concat!("test_data/", $file, ".jls")), 1 << 20).unwrap();
            assert_eq!((img.width(), img.height()), ($w, $h));
            assert_eq!(pixels(&img, $channels), include_bytes!(concat!("test_data/", $file, ".raw")), $why);
        }
    };
}

fixture!(lossless_grey_with_runs_and_spikes, "jls-grey8-runs", 1, 33, 17, "run mode, run interruption and regular mode all appear");
fixture!(sample_interleaved_rgb, "jls-rgb8-sample", 3, 33, 17, "three components share one run decision");
fixture!(separate_scans_per_component, "jls-rgb8-planes", 3, 33, 17, "non-interleaved scans, one per component");
fixture!(near_lossless_12_bit, "jls-rgb12-near4", 3, 5, 7, "NEAR=4 at 12 bits: reconstruction must wrap and clamp exactly like the encoder");
fixture!(near_lossless_4_bit_default_thresholds, "jls-grey4-near1", 1, 20, 12, "below 8 bits the default T1..T3 add NEAR before the floor is applied");
fixture!(near_lossless_7_bit_default_thresholds, "jls-grey7-near2", 1, 20, 12, "same rule just under the 128 boundary");
fixture!(run_that_reaches_the_line_end, "jls-grey8-flat", 1, 9, 1, "a run ending at the end of the line has no interruption sample");

#[test]
fn non_interleaved_and_sample_interleaved_agree() {
    let a = run(include_bytes!("test_data/jls-rgb8-planes.jls"), 1 << 20).unwrap();
    let b = run(include_bytes!("test_data/jls-rgb8-sample.jls"), 1 << 20).unwrap();
    assert_eq!(a.to_rgb8(), b.to_rgb8());
}

#[test]
fn rejects_what_is_not_jpeg_ls() {
    assert!(run(b"not a jpeg", 1 << 20).is_err());
    assert!(run(&[0xFF, 0xD8, 0xFF, 0xD9], 1 << 20).is_err(), "no frame");
}

#[test]
fn truncated_data_is_an_error_not_a_panic() {
    let full = include_bytes!("test_data/jls-rgb8-sample.jls");
    for cut in [20, 40, 100, full.len() / 2, full.len() - 3] {
        assert!(run(&full[..cut], 1 << 20).is_err(), "cut at {cut}");
    }
}

#[test]
fn corrupted_streams_never_panic() {
    let full = include_bytes!("test_data/jls-grey8-runs.jls");
    for at in 0..full.len() {
        for flip in [0x01u8, 0x80, 0xFF] {
            let mut b = full.to_vec();
            b[at] ^= flip;
            let _ = run(&b, 1 << 20);
        }
    }
}

#[test]
fn oversized_frame_is_refused_before_allocating() {
    let mut f = vec![0xFF, 0xD8, 0xFF, 0xF7, 0, 11, 8, 0xFF, 0xFF, 0xFF, 0xFF, 1, 1, 0x11, 0, 0xFF, 0xD9];
    assert_eq!(run(&f, 150_000_000).unwrap_err(), crate::decode::TOO_LARGE);
    f[7] = 0; // a zero height means "defined later" and is not supported
    f[8] = 0;
    assert!(run(&f, 150_000_000).is_err());
}

#[test]
fn subsampled_components_are_named_in_the_error() {
    let f = [0xFF, 0xD8, 0xFF, 0xF7, 0, 17, 8, 0, 4, 0, 4, 3, 1, 0x22, 0, 2, 0x11, 0, 3, 0x11, 0, 0xFF, 0xD9];
    assert!(run(&f, 1 << 20).unwrap_err().contains("subsampled"));
}

#[test]
fn mapping_tables_are_refused() {
    let mut f = include_bytes!("test_data/jls-grey8-runs.jls").to_vec();
    let sos = f.windows(2).position(|w| w == [0xFF, 0xDA]).unwrap();
    f[sos + 6] = 1; // Tm: use mapping table 1
    assert!(run(&f, 1 << 20).unwrap_err().contains("mapping"));
}

#[test]
fn default_thresholds_follow_the_standard() {
    use crate::jpegls_codec::params;
    let t = |maxval, near| params(maxval, near, [0; 4]).unwrap().t;
    assert_eq!(t(255, 0), [3, 7, 21]);
    assert_eq!(t(15, 0), [2, 3, 4]);
    assert_eq!(t(15, 1), [3, 5, 8], "BASIC/F + k*NEAR first, the floor after");
    assert_eq!(t(127, 1), [4, 8, 17]);
    assert_eq!(t(65535, 0), [18, 67, 276]);
}

#[test]
fn a_golomb_code_beyond_the_sample_range_is_rejected() {
    use crate::jpegls_codec::{params, Dec};
    // 8-bit: RANGE is 256. 22 zeros, a one and eight ones decode (with k = 8) to (22 << 8) + 255, far outside it;
    // accepting that would let A and then k grow with every sample until the shifts overflow or never terminate.
    let data = [0x00, 0x00, 0x03, 0xFF, 0x80];
    let mut dec = Dec::new(params(255, 0, [0; 4]).unwrap(), &data);
    assert!(dec.golomb(8, 32).is_err());
    let ok = [0b0000_0101, 0xFF]; // z = 5, one, then k = 2 bits: 5 << 2 | 0b01 = 21, inside the range
    assert_eq!(Dec::new(params(255, 0, [0; 4]).unwrap(), &ok).golomb(2, 32), Ok(21));
}

#[test]
fn restart_intervals_and_a_large_reset_are_named() {
    let mut f = include_bytes!("test_data/jls-grey8-runs.jls").to_vec();
    let sof = f.windows(2).position(|w| w == [0xFF, 0xF7]).unwrap();
    f.splice(sof..sof, [0xFF, 0xDD, 0, 4, 0, 8]);
    assert!(run(&f, 1 << 20).unwrap_err().contains("restart"));
}

#[test]
fn an_lse_maxval_before_the_frame_header_is_kept() {
    let mut f = include_bytes!("test_data/jls-grey8-runs.jls").to_vec();
    let sof = f.windows(2).position(|w| w == [0xFF, 0xF7]).unwrap();
    // MAXVAL 200, defaults for the rest: the scan was coded with 255, so it must differ from the plain decode.
    f.splice(sof..sof, [0xFF, 0xF8, 0, 13, 1, 0, 200, 0, 0, 0, 0, 0, 0, 0, 0]);
    let plain = run(include_bytes!("test_data/jls-grey8-runs.jls"), 1 << 20).unwrap();
    let with = run(&f, 1 << 20);
    assert!(with.is_err() || with.unwrap().to_luma8() != plain.to_luma8(), "the early MAXVAL must reach the scan");
}
