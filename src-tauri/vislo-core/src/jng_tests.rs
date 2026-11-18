//! JNG checks: files are assembled here from JPEG and PNG encoder output, plus one real ImageMagick-made file.

use crate::jng::decode;
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, ImageEncoder};

fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(data);
    [&(data.len() as u32).to_be_bytes()[..], kind, data, &crc.sum().to_be_bytes()].concat()
}

fn jpeg(w: u32, h: u32, channels: u8) -> Vec<u8> {
    let px: Vec<u8> = (0..w * h * u32::from(channels)).map(|i| (i * 7 % 251) as u8).collect();
    let mut out = Vec::new();
    let ct = if channels == 1 { ExtendedColorType::L8 } else { ExtendedColorType::Rgb8 };
    JpegEncoder::new_with_quality(&mut out, 100).encode(&px, w, h, ct).unwrap();
    out
}

/// The IDAT chunks (with their headers) of a greyscale PNG whose pixels are `alpha`.
fn png_alpha(w: u32, h: u32, alpha: &[u8]) -> Vec<u8> {
    let mut png = Vec::new();
    image::codecs::png::PngEncoder::new(&mut png).write_image(alpha, w, h, ExtendedColorType::L8).unwrap();
    let (mut at, mut idat) = (8, Vec::new());
    while at < png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        if &png[at + 4..at + 8] == b"IDAT" {
            idat.extend_from_slice(&png[at..at + 12 + len]);
        }
        at += 12 + len;
    }
    idat
}

fn jhdr(w: u32, h: u32, color: u8, depth: u8, alpha_depth: u8, alpha_comp: u8) -> Vec<u8> {
    chunk(b"JHDR", &[&w.to_be_bytes()[..], &h.to_be_bytes(), &[color, depth, 8, 0, alpha_depth, alpha_comp, 0, 0]].concat())
}

fn jng(header: Vec<u8>, alpha: Vec<u8>, jdat: &[u8]) -> Vec<u8> {
    let (a, b) = jdat.split_at(jdat.len() / 2); // JDAT may be split over several chunks
    [b"\x8bJNG\r\n\x1a\n".to_vec(), header, alpha, chunk(b"JDAT", a), chunk(b"JDAT", b), chunk(b"IEND", &[])].concat()
}

fn run(bytes: &[u8], max: u64) -> Result<image::DynamicImage, String> {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let d = std::env::temp_dir().join(format!("vislo-jng-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.jng"), bytes).unwrap();
    let r = decode(&d.join("a.jng"), max);
    let _ = std::fs::remove_dir_all(&d);
    r
}

#[test]
fn colour_and_grey_without_alpha() {
    let rgb = run(&jng(jhdr(8, 6, 10, 8, 0, 0), vec![], &jpeg(8, 6, 3)), 1000).unwrap();
    assert_eq!((rgb.width(), rgb.height(), rgb.color()), (8, 6, image::ColorType::Rgb8));
    let grey = run(&jng(jhdr(8, 6, 8, 8, 0, 0), vec![], &jpeg(8, 6, 1)), 1000).unwrap();
    assert_eq!(grey.color(), image::ColorType::L8);
}

#[test]
fn png_alpha_is_applied_exactly() {
    let alpha: Vec<u8> = (0..48).map(|i| (i * 5) as u8).collect();
    let img = run(&jng(jhdr(8, 6, 14, 8, 8, 0), png_alpha(8, 6, &alpha), &jpeg(8, 6, 3)), 1000).unwrap().to_rgba8();
    assert_eq!(img.pixels().map(|p| p[3]).collect::<Vec<_>>(), alpha, "alpha is lossless in the PNG form");
}

#[test]
fn jpeg_alpha_is_applied() {
    let img = run(&jng(jhdr(8, 6, 14, 8, 8, 8), chunk(b"JDAA", &jpeg(8, 6, 1)), &jpeg(8, 6, 3)), 1000).unwrap();
    assert_eq!(img.color(), image::ColorType::Rgba8);
}

#[test]
fn a_real_imagemagick_file_has_its_exact_alpha_and_the_picture_colours() {
    let img = run(include_bytes!("test_data/jng-rose-alpha.jng"), 100_000).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (70, 46));
    let alpha: Vec<u8> = img.pixels().map(|p| p[3]).collect();
    assert_eq!(alpha, include_bytes!("test_data/jng-rose-alpha.alpha"), "reference decoded with Pillow");
    let mean = |c: usize| img.pixels().map(|p| u64::from(p[c])).sum::<u64>() as f64 / 3220.0;
    // JPEG decoders round differently, so the colour is compared by channel mean against the reference decode.
    for (c, want) in [145.39, 89.21, 80.52].into_iter().enumerate() {
        assert!((mean(c) - want).abs() < 1.5, "channel {c}: {} vs {want}", mean(c));
    }
}

#[test]
fn a_jng_first_mng_shows_the_jng() {
    let inner = jng(jhdr(8, 6, 10, 8, 0, 0), vec![], &jpeg(8, 6, 3));
    let file = [b"\x8aMNG\r\n\x1a\n".to_vec(), chunk(b"MHDR", &[0; 28]), inner[8..].to_vec(), chunk(b"MEND", &[])].concat();
    let d = std::env::temp_dir().join(format!("vislo-jng-mng-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("a.mng"), file).unwrap();
    let img = crate::mng::decode(&d.join("a.mng"), 1000).unwrap();
    assert_eq!((img.width(), img.height()), (8, 6));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn what_is_not_supported_or_not_valid_is_an_error() {
    let ok = jng(jhdr(8, 6, 10, 8, 0, 0), vec![], &jpeg(8, 6, 3));
    assert!(run(b"junk", 1000).is_err());
    assert!(run(&ok[..ok.len() - 30], 1000).is_err(), "truncated");
    assert!(run(&jng(jhdr(9, 6, 10, 8, 0, 0), vec![], &jpeg(8, 6, 3)), 1000).unwrap_err().contains("match"), "header size differs from the JPEG");
    assert!(run(&jng(jhdr(8, 6, 10, 12, 0, 0), vec![], &jpeg(8, 6, 3)), 1000).unwrap_err().contains("12-bit"));
    assert!(run(&jng(jhdr(8, 6, 11, 8, 0, 0), vec![], &jpeg(8, 6, 3)), 1000).is_err(), "unknown colour type");
    assert!(run(&jng(jhdr(8, 6, 14, 8, 3, 0), vec![], &jpeg(8, 6, 3)), 1000).is_err(), "invalid alpha depth");
    assert!(run(&jng(jhdr(8, 6, 14, 8, 8, 0), vec![], &jpeg(8, 6, 3)), 1000).is_err(), "alpha announced but missing");
    assert_eq!(run(&ok, 20).unwrap_err(), crate::decode::TOO_LARGE);
    let huge = jng(jhdr(0x7FFF_FFFF, 0x7FFF_FFFF, 10, 8, 0, 0), vec![], &[]);
    assert_eq!(run(&huge, 150_000_000).unwrap_err(), crate::decode::TOO_LARGE, "refused before touching the pixels");
}
