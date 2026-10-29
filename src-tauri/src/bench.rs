//! Reproducible timings (ignored by default):
//!   cargo test --profile devrel --manifest-path src-tauri/Cargo.toml bench_ -- --ignored --nocapture
//! Synthetic 24 MP photographs, so the numbers do not depend on anyone's library.

use crate::{decode, resize, thumbs};
use fast_image_resize::FilterType;
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};
use std::path::{Path, PathBuf};
use std::time::Instant;

const RUNS: usize = 7;

/// Smooth sky, a detailed ground band and sensor-like noise: compresses like a real photo.
/// `grain` is the noise amplitude in 1/16ths: 8 resembles a high-ISO shot, 1 a clean low-ISO one.
fn photo(w: u32, h: u32, grain: i32) -> Vec<u8> {
    let mut seed = 0x2545_F491u32;
    let mut px = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = ((seed >> 28) as i32 - 8) * grain / 8;
            let detail = if y > h / 2 { (((x as f32 * 0.31).sin() + (y as f32 * 0.17).cos()) * 40.0) as i32 } else { 0 };
            let base = (y * 200 / h) as i32;
            for c in [base, base / 2 + (x * 90 / w) as i32, 60 + detail] {
                px.push((c + noise + detail / 2).clamp(0, 255) as u8);
            }
        }
    }
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 90).encode(&px, w, h, ExtendedColorType::Rgb8).unwrap();
    out
}

fn median_ms(mut f: impl FnMut()) -> f64 {
    let mut t: Vec<f64> = (0..RUNS)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    t.sort_by(|a, b| a.partial_cmp(b).unwrap());
    t[RUNS / 2]
}

fn fixture(name: &str, grain: i32) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("vislo-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join(name);
    std::fs::write(&file, photo(6000, 4000, grain)).unwrap();
    (dir, file)
}

fn report(label: &str, file: &Path, ms: f64) {
    let mb = std::fs::metadata(file).unwrap().len() as f64 / 1e6;
    println!("{label:<34} {ms:>8.1} ms   ({mb:.1} MB source)");
}

/// What the code did before scaled decoding: decode everything, then shrink a borrowed copy.
fn legacy(file: &Path, w: u32, h: u32, filter: FilterType, quality: u8) -> Vec<u8> {
    let small = resize::fit(&decode::load(file).unwrap(), w, h, filter).unwrap();
    resize::to_jpeg(&small, quality).unwrap()
}

#[test]
#[ignore = "timing run"]
fn bench_jpeg_thumbnail_and_fit() {
    for (name, grain) in [("noisy.jpg", 8), ("clean.jpg", 1)] {
        let (dir, file) = fixture(name, grain);
        for size in [192, 512] {
            let before = median_ms(|| drop(legacy(&file, size, size, FilterType::CatmullRom, 80)));
            let after = median_ms(|| {
                let cache = dir.join(format!("cache-{size}"));
                let _ = std::fs::remove_dir_all(&cache);
                thumbs::thumbnail(&cache, &file, size).unwrap();
            });
            report(&format!("{name} thumbnail {size}: before"), &file, before);
            report(&format!("{name} thumbnail {size}: after"), &file, after);
        }
        let before = median_ms(|| drop(legacy(&file, 1920, 1080, FilterType::Lanczos3, 90)));
        let after = median_ms(|| {
            let img = decode::load_fit(&file, 1920, 1080).unwrap();
            let img = resize::fit_owned(img, 1920, 1080, FilterType::Lanczos3).unwrap();
            drop(resize::to_jpeg(&img, 90).unwrap());
        });
        report(&format!("{name} fit 1920x1080: before"), &file, before);
        report(&format!("{name} fit 1920x1080: after"), &file, after);
        let _ = std::fs::remove_dir_all(dir);
    }
}
