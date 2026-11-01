//! JPEG-LS checks. The fixtures were produced by an independent encoder (CharLS); each `.raw` is that encoder's own
//! decode scaled to 8 bits, so a match means the same pixels, not just the same size.

use crate::jpegls::decode;
use std::path::PathBuf;

fn write(bytes: &[u8]) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!("vislo-jls-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("x.jls");
    std::fs::write(&p, bytes).unwrap();
    p
}

fn run(bytes: &[u8], max: u64) -> Result<image::DynamicImage, String> {
    let p = write(bytes);
    let r = decode(&p, max);
    let _ = std::fs::remove_dir_all(p.parent().unwrap());
    r
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
