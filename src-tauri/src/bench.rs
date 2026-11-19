//! Reproducible timings (ignored by default):
//!   cargo test --profile devrel --manifest-path src-tauri/Cargo.toml bench_ -- --ignored --nocapture
//! Synthetic 24 MP photographs, so the numbers do not depend on anyone's library.

use crate::{decode, resize, thumbs};
use fast_image_resize::FilterType;
use image::{codecs::jpeg::JpegEncoder, DynamicImage, ExtendedColorType};
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

/// The JPEG path as it was before: a full RGB copy before every encode, three buffers to flatten alpha,
/// all through the `image` crate's baseline encoder.
mod legacy_encode {
    use image::{codecs::jpeg::JpegEncoder, DynamicImage, ExtendedColorType, ImageEncoder};

    pub fn to_jpeg(img: &DynamicImage, quality: u8) -> Vec<u8> {
        let rgb = if img.color().has_alpha() {
            let mut bg = image::RgbaImage::from_pixel(img.width(), img.height(), image::Rgba([14, 14, 16, 255]));
            image::imageops::overlay(&mut bg, &img.to_rgba8(), 0, 0);
            DynamicImage::ImageRgba8(bg).to_rgb8()
        } else {
            img.to_rgb8()
        };
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, quality)
            .write_image(rgb.as_raw(), rgb.width(), rgb.height(), ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

}

/// A 24 MP RGB picture and the same with a soft alpha ramp: what the `/c` route encodes for a large
/// TIFF/PSD/EXR, and (RGB, smaller) what every thumbnail and `/f` reply encodes.
fn decoded(w: u32, h: u32) -> (DynamicImage, DynamicImage) {
    let rgb = image::load_from_memory(&photo(w, h, 4)).unwrap().into_rgb8();
    let mut rgba = DynamicImage::ImageRgb8(rgb.clone()).into_rgba8();
    for (x, _, p) in rgba.enumerate_pixels_mut() {
        p[3] = (x * 255 / w) as u8;
    }
    (DynamicImage::ImageRgb8(rgb), DynamicImage::ImageRgba8(rgba))
}

#[test]
#[ignore = "timing run"]
fn bench_encode() {
    let (rgb, rgba) = decoded(6000, 4000);
    let row = |label: &str, before: f64, after: f64, bytes: (usize, usize)| {
        println!(
            "{label:<28} before {before:>7.1} ms   after {after:>7.1} ms   {:>4.2}x   ({:.1} MB -> {:.1} MB)",
            before / after,
            bytes.0 as f64 / 1e6,
            bytes.1 as f64 / 1e6
        );
    };
    let sizes = |a: Vec<u8>, b: Vec<u8>| (a.len(), b.len());
    for (label, img) in [("24 MP RGB  -> JPEG q92", &rgb), ("24 MP RGBA -> JPEG q92", &rgba)] {
        let b = median_ms(|| drop(legacy_encode::to_jpeg(img, 92)));
        let a = median_ms(|| drop(resize::to_jpeg(img, 92).unwrap()));
        row(label, b, a, sizes(legacy_encode::to_jpeg(img, 92), resize::to_jpeg(img, 92).unwrap()));
    }
    let small = resize::fit(&rgb, 1920, 1080, FilterType::Lanczos3).unwrap();
    let b = median_ms(|| drop(legacy_encode::to_jpeg(&small, 90)));
    let a = median_ms(|| drop(resize::to_jpeg(&small, 90).unwrap()));
    row("1620x1080 RGB -> JPEG q90", b, a, sizes(legacy_encode::to_jpeg(&small, 90), resize::to_jpeg(&small, 90).unwrap()));
}

/// What `album::build` did before: a file-type check, then a second `stat` by full path for each image.
fn legacy_listing(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut files: Vec<(PathBuf, String)> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| crate::album::is_image(Path::new(&e.file_name())) && e.file_type().is_ok_and(|t| t.is_file() || e.path().is_file()))
        .map(|e| e.path())
        .filter_map(|p| {
            let meta = std::fs::metadata(&p).ok().filter(std::fs::Metadata::is_file)?;
            Some((p, crate::album::stamp(&meta)))
        })
        .collect();
    files.sort_by(|a, b| natord::compare_ignore_case(&a.0.file_name().unwrap().to_string_lossy(), &b.0.file_name().unwrap().to_string_lossy()));
    files
}

#[test]
#[ignore = "timing run"]
fn bench_album_listing() {
    let dir = std::env::temp_dir().join(format!("vislo-bench-album-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..5000 {
        std::fs::write(dir.join(format!("IMG_{i}.jpg")), b"x").unwrap();
        if i % 5 == 0 {
            std::fs::write(dir.join(format!("IMG_{i}.xmp")), b"x").unwrap(); // sidecars are skipped
        }
    }
    let first = dir.join("IMG_0.jpg");
    let before = median_ms(|| drop(legacy_listing(&dir)));
    let after = median_ms(|| drop(crate::album::list_changed(&first, None).unwrap()));
    println!("album 5000 images + 1000 sidecars: before {before:.2} ms   after {after:.2} ms   {:.2}x", before / after);
    let _ = std::fs::remove_dir_all(dir);
}

/// `to_display` as it was: a `powf` per channel, a full RGBA float copy first and an RGB copy last.
fn legacy_to_display(img: &DynamicImage) -> DynamicImage {
    let encode = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
        (s * 255.0 + 0.5) as u8
    };
    let src = img.to_rgba32f();
    let mut out = image::RgbaImage::new(src.width(), src.height());
    for (o, p) in out.pixels_mut().zip(src.pixels()) {
        *o = image::Rgba([encode(p[0]), encode(p[1]), encode(p[2]), (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8]);
    }
    if img.color().has_alpha() { DynamicImage::ImageRgba8(out) } else { DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(out).to_rgb8()) }
}

#[test]
#[ignore = "timing run"]
fn bench_hdr_to_display() {
    let (w, h) = (6000u32, 4000u32);
    let hdr = image::Rgb32FImage::from_fn(w, h, |x, y| image::Rgb([x as f32 / w as f32 * 1.5, y as f32 / h as f32, ((x ^ y) & 255) as f32 / 300.0]));
    let rgb = DynamicImage::ImageRgb32F(hdr);
    let rgba = DynamicImage::ImageRgba32F(rgb.to_rgba32f());
    for (label, img) in [("24 MP RGB float  -> 8-bit", &rgb), ("24 MP RGBA float -> 8-bit", &rgba)] {
        let before = median_ms(|| drop(legacy_to_display(img)));
        let after = median_ms(|| drop(decode::to_display(img.clone())));
        let clone = median_ms(|| drop(img.clone()));
        // `to_display` takes its input by value, so the clone the timing needs is subtracted.
        let after = after - clone;
        let (a, b) = (legacy_to_display(img).into_bytes(), decode::to_display(img.clone()).into_bytes());
        let worst = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
        println!("{label:<28} before {before:>7.1} ms   after {after:>7.1} ms   {:>4.1}x   (max diff {worst} level)", before / after);
    }
}

/// What the `/c` route sent before: PNG for any alpha channel, even a fully opaque one.
fn legacy_convert(img: &DynamicImage) -> Vec<u8> {
    if img.color().has_alpha() {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    } else {
        resize::to_jpeg(&DynamicImage::ImageRgb8(img.to_rgb8()), 92).unwrap()
    }
}

#[test]
#[ignore = "timing run"]
fn bench_convert_route() {
    let (rgb, _) = decoded(6000, 4000);
    let opaque = DynamicImage::ImageRgba8(rgb.to_rgba8());
    let grey = DynamicImage::ImageLuma8(rgb.to_luma8());
    for (label, img) in [("24 MP opaque RGBA (PSD-like)", &opaque), ("24 MP greyscale", &grey)] {
        let before = median_ms(|| drop(legacy_convert(img)));
        let after = median_ms(|| drop(resize::for_webview(img).unwrap()));
        let (b, a) = (legacy_convert(img).len(), resize::for_webview(img).unwrap().1.len());
        println!(
            "{label:<30} before {before:>7.1} ms   after {after:>7.1} ms   {:>4.1}x   ({:.1} MB -> {:.1} MB)",
            before / after,
            b as f64 / 1e6,
            a as f64 / 1e6
        );
    }
}

/// A RAW file as cameras write it: a raw-sensor container whose largest embedded JPEG is a full 24 MP preview (here
/// after a RAF-like header and 8 MB standing in for the sensor data, so the scan for the preview is paid too).
#[test]
#[ignore = "timing run"]
fn bench_raw_thumbnail() {
    for (name, grain) in [("noisy.raf", 8), ("clean.raf", 1)] {
        let dir = std::env::temp_dir().join(format!("vislo-bench-raw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(name);
        let mut blob = b"FUJIFILMCCD-RAW ".to_vec();
        blob.extend(photo(6000, 4000, grain));
        blob.extend(std::iter::repeat_n(0x5Au8, 8 << 20));
        std::fs::write(&file, blob).unwrap();
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
        let _ = std::fs::remove_dir_all(dir);
    }
}
