//! Manual-test fixtures. Not part of the normal run:
//!   VISLO_FIXTURES=<dir> cargo test --manifest-path src-tauri/Cargo.toml write_fixtures -- --ignored

use crate::exif_info::tests::{field, jpeg_with_exif, tiff_block};
use exif::{In, Tag, Value};
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};

fn encode(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            px.extend(f(x, y));
        }
    }
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, 85).encode(&px, w, h, ExtendedColorType::Rgb8).unwrap();
    out
}

/// Gradient with fine one-pixel stripes: shows aliasing when downscaled badly.
fn detailed(x: u32, y: u32) -> [u8; 3] {
    let stripe = if (x / 2 + y / 2).is_multiple_of(2) { 30 } else { 0 };
    [((x * 255) / 6000) as u8, ((y * 255) / 4000) as u8, 90 + stripe]
}

fn rational(v: &[(u32, u32)]) -> Value {
    Value::Rational(v.iter().map(|&(n, d)| (n, d).into()).collect())
}

fn psd(w: u32, h: u32) -> Vec<u8> {
    let mut b = b"8BPS".to_vec();
    b.extend([0, 1, 0, 0, 0, 0, 0, 0, 0, 3]); // version 1, reserved, 3 channels
    b.extend(h.to_be_bytes());
    b.extend(w.to_be_bytes());
    b.extend([0, 8, 0, 3]); // 8 bit, RGB
    b.extend([0u8; 12]); // colour mode data, image resources, layer & mask info: all empty
    b.extend([0, 0]); // raw image data
    for ch in 0..3u32 {
        for i in 0..w * h {
            b.push(if ch == 0 { 220 } else if ch == 1 { (i % 256) as u8 } else { 60 });
        }
    }
    b
}

#[test]
#[ignore = "writes fixture files; run on demand"]
fn write_fixtures() {
    let dir = std::path::PathBuf::from(std::env::var("VISLO_FIXTURES").expect("set VISLO_FIXTURES"));
    std::fs::create_dir_all(&dir).unwrap();

    let thumb = encode(160, 107, |_, _| [255, 140, 0]); // orange: unmistakable placeholder
    let exif = tiff_block(
        &[
            field(Tag::Orientation, In::PRIMARY, Value::Short(vec![1])),
            field(Tag::Make, In::PRIMARY, Value::Ascii(vec![b"Vislo Labs".to_vec()])),
            field(Tag::Model, In::PRIMARY, Value::Ascii(vec![b"Test Camera 24".to_vec()])),
            field(Tag::FNumber, In::PRIMARY, rational(&[(28, 10)])),
            field(Tag::ExposureTime, In::PRIMARY, rational(&[(1, 250)])),
            field(Tag::PhotographicSensitivity, In::PRIMARY, Value::Short(vec![400])),
            field(Tag::DateTimeOriginal, In::PRIMARY, Value::Ascii(vec![b"2026:10:20 14:30:00".to_vec()])),
            field(Tag::GPSLatitudeRef, In::PRIMARY, Value::Ascii(vec![b"S".to_vec()])),
            field(Tag::GPSLatitude, In::PRIMARY, rational(&[(23, 1), (33, 1), (0, 1)])),
            field(Tag::GPSLongitudeRef, In::PRIMARY, Value::Ascii(vec![b"W".to_vec()])),
            field(Tag::GPSLongitude, In::PRIMARY, rational(&[(46, 1), (38, 1), (0, 1)])),
        ],
        Some(&thumb),
    );
    let big = encode(6000, 4000, detailed);
    std::fs::write(dir.join("b1_big_photo.jpg"), jpeg_with_exif(&big, &exif)).unwrap();

    let rot = tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))], None);
    let side = encode(1200, 800, |x, y| [(x * 255 / 1200) as u8, 60, (y * 255 / 800) as u8]);
    std::fs::write(dir.join("b2_rotated.jpg"), jpeg_with_exif(&side, &rot)).unwrap();

    std::fs::write(dir.join("b3_layered.psd"), psd(64, 48)).unwrap();

    let mut raw = vec![0x49, 0x49, 0x2A, 0x00, 8, 0, 0, 0];
    raw.extend(encode(100, 70, |_, _| [10, 10, 10]));
    raw.extend([0u8; 64]);
    raw.extend(encode(900, 600, |x, y| [(x * 255 / 900) as u8, (y * 255 / 600) as u8, 200]));
    std::fs::write(dir.join("b4_shot.dng"), raw).unwrap();
}

/// A photo large enough that the WebView needs a noticeable time to decode it, so the embedded
/// preview is shown first.
#[test]
#[ignore = "writes a large fixture; run on demand"]
fn write_huge_fixture() {
    let dir = std::path::PathBuf::from(std::env::var("VISLO_FIXTURES").expect("set VISLO_FIXTURES"));
    std::fs::create_dir_all(&dir).unwrap();
    let thumb = encode(160, 107, |_, _| [255, 140, 0]);
    let exif = tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![1]))], Some(&thumb));
    let huge = encode(12000, 8000, detailed);
    std::fs::write(dir.join("huge.jpg"), jpeg_with_exif(&huge, &exif)).unwrap();
}
