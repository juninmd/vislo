//! Decode checks for the legacy formats served by `image-extras`; every file is built by hand here.

use crate::decode::{dimensions, load};
use image::{DynamicImage, GenericImageView};
use std::path::PathBuf;

fn dir(tag: &str) -> PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let d = std::env::temp_dir().join(format!("vislo-extra-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn png(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(w, h, image::Rgba(px)))
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn pcx_4x2() -> Vec<u8> {
    let mut f = vec![0x0a, 5, 1, 8, 0, 0, 0, 0, 3, 0, 1, 0, 0x48, 0, 0x48, 0];
    f.resize(65, 0); // 16-colour palette and the reserved byte
    f.extend([3, 4, 0, 1, 0]); // planes, bytes per line, palette info
    f.resize(128, 0);
    f.extend(std::iter::repeat_n(0x40u8, 4 * 3 * 2)); // literal bytes below 0xC0: no RLE runs
    f
}

fn sgi_3x2() -> Vec<u8> {
    let mut f = vec![0x01, 0xda, 0, 1, 0, 3, 0, 3, 0, 2, 0, 3, 0, 0, 0, 0, 0, 0, 0xff];
    f.resize(512, 0);
    f.extend(std::iter::repeat_n(0x80u8, 3 * 2 * 3));
    f
}

fn icns_with(png: &[u8]) -> Vec<u8> {
    let chunk = (png.len() + 8) as u32;
    let total = chunk + 8;
    [b"icns".as_slice(), &total.to_be_bytes(), b"ic07", &chunk.to_be_bytes(), png].concat()
}

#[test]
fn legacy_formats_decode_with_the_right_size() {
    let d = dir("ok");
    let files: Vec<(&str, Vec<u8>, (u32, u32))> = vec![
        ("a.xbm", b"#define t_width 8\n#define t_height 2\nstatic unsigned char t_bits[] = {\n 0xff, 0x00 };\n".to_vec(), (8, 2)),
        ("a.bm", b"#define t_width 8\n#define t_height 2\nstatic unsigned char t_bits[] = {\n 0xff, 0x00 };\n".to_vec(), (8, 2)),
        ("a.xpm", b"/* XPM */\nstatic char *t[] = {\n\"3 2 2 1\",\n\"a c #FF0000\",\n\"b c #0000FF\",\n\"aab\",\n\"abb\"\n};\n".to_vec(), (3, 2)),
        ("a.wbmp", vec![0, 0, 8, 2, 0xff, 0x00], (8, 2)),
        ("a.otb", vec![0, 8, 2, 1, 0xff, 0x00], (8, 2)),
        ("a.pcx", pcx_4x2(), (4, 2)),
        ("a.sgi", sgi_3x2(), (3, 2)),
        ("a.bw", sgi_3x2(), (3, 2)),
        ("a.icns", icns_with(&png(128, 128, [9, 9, 9, 255])), (128, 128)),
    ];
    for (name, bytes, size) in files {
        std::fs::write(d.join(name), bytes).unwrap();
        assert_eq!(load(&d.join(name)).unwrap_or_else(|e| panic!("{name}: {e}")).dimensions(), size, "{name}");
        assert_eq!(dimensions(&d.join(name)), Some(size), "{name} header size");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn xbm_ink_is_black_on_white() {
    let d = dir("xbm");
    std::fs::write(d.join("a.xbm"), b"#define t_width 8\n#define t_height 1\nstatic unsigned char t_bits[] = {\n 0x01 };\n").unwrap();
    let img = load(&d.join("a.xbm")).unwrap().to_luma8();
    let (set, unset) = (img.get_pixel(0, 0).0[0], img.get_pixel(1, 0).0[0]);
    assert!(set < unset, "bit 0 of an XBM byte is the leftmost pixel and draws as ink ({set} vs {unset})");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn corrupt_legacy_files_are_errors_not_panics() {
    let d = dir("bad");
    for name in ["a.pcx", "a.sgi", "a.icns", "a.xpm", "a.xbm", "a.wbmp", "a.otb"] {
        std::fs::write(d.join(name), b"definitely not this format").unwrap();
        assert!(load(&d.join(name)).is_err(), "{name}");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn oversized_legacy_images_are_refused_by_the_header() {
    let d = dir("big");
    // 65 535 x 65 535 px (4.3 Gpx) announced by an SGI header with no pixel data behind it.
    let mut sgi = vec![0x01, 0xda, 0, 1, 0, 2, 0xff, 0xff, 0xff, 0xff, 0, 1];
    sgi.resize(512, 0);
    std::fs::write(d.join("huge.sgi"), sgi).unwrap();
    assert_eq!(load(&d.join("huge.sgi")).unwrap_err(), crate::decode::TOO_LARGE);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn targa_with_an_image_id_is_not_mistaken_for_pcx() {
    // id_length 10 + colour-map type 0 is also the start of a PCX signature, which content sniffing must not follow.
    let d = dir("tga-id");
    let mut tga = vec![10, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0, 3, 0, 24, 0x20];
    tga.extend([b'i'; 10]);
    tga.extend(std::iter::repeat_n(0x80u8, 4 * 3 * 3));
    for name in ["id.tga", "id.vda"] {
        std::fs::write(d.join(name), &tga).unwrap();
        assert_eq!(load(&d.join(name)).unwrap_or_else(|e| panic!("{name}: {e}")).dimensions(), (4, 3), "{name}");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn headers_that_pass_the_allocator_limit_still_hit_the_pixel_ceiling() {
    // 20 000 x 10 000 = 200 Mpx > MAX_PIXELS but only 200 MB as L8, under the allocator limit: only the header check refuses it.
    let d = dir("pix");
    std::fs::write(d.join("big.xbm"), b"#define t_width 20000\n#define t_height 10000\nstatic unsigned char t_bits[] = {\n 0x00 };\n").unwrap();
    assert_eq!(load(&d.join("big.xbm")).unwrap_err(), crate::decode::TOO_LARGE);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn truncated_bodies_behind_valid_headers_are_errors() {
    let d = dir("cut");
    for (name, full) in [("a.pcx", pcx_4x2()), ("a.sgi", sgi_3x2())] {
        std::fs::write(d.join(name), &full[..full.len() - 4]).unwrap();
        assert!(load(&d.join(name)).is_err(), "{name}");
    }
    let _ = std::fs::remove_dir_all(d);
}
