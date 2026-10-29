//! Decoding for everything the webview cannot show itself, and for thumbnails of everything.

use crate::{album::RAW_EXTENSIONS, exif_info, jpeg_scaled, raw};
use image::{DynamicImage, ImageError, ImageReader, Limits};
use std::path::Path;

/// Ceiling for decoded pixels (RGBA, so about 600 MB): bigger files are refused, not attempted.
const MAX_PIXELS: u64 = 150_000_000;
const MAX_ALLOC: u64 = 1 << 30;
pub(crate) const TOO_LARGE: &str = "Image is too large";

fn describe(e: ImageError) -> String {
    match e {
        ImageError::Limits(_) => TOO_LARGE.to_string(),
        other => other.to_string(),
    }
}

fn ext(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn is_raw(ext: &str) -> bool {
    RAW_EXTENSIONS.contains(&ext)
}

/// HDR/EXR decode to linear floats; the webview needs 8-bit sRGB (highlights above 1.0 clip).
fn to_display(img: DynamicImage) -> DynamicImage {
    use image::ColorType::{Rgb32F, Rgba32F};
    if !matches!(img.color(), Rgb32F | Rgba32F) {
        return img;
    }
    let encode = |c: f32| {
        let c = c.clamp(0.0, 1.0);
        let s = if c <= 0.003_130_8 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        };
        (s * 255.0 + 0.5) as u8
    };
    let src = img.to_rgba32f();
    let mut out = image::RgbaImage::new(src.width(), src.height());
    for (o, p) in out.pixels_mut().zip(src.pixels()) {
        *o = image::Rgba([
            encode(p[0]),
            encode(p[1]),
            encode(p[2]),
            (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        ]);
    }
    let alpha = img.color().has_alpha();
    let rgba = DynamicImage::ImageRgba8(out);
    if alpha {
        rgba
    } else {
        DynamicImage::ImageRgb8(rgba.to_rgb8())
    }
}

fn upright(img: DynamicImage, orientation: u8) -> DynamicImage {
    match image::metadata::Orientation::from_exif(orientation) {
        Some(o) => {
            let mut img = img;
            img.apply_orientation(o);
            img
        }
        None => img,
    }
}

fn decode_psd(path: &Path) -> Result<DynamicImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // Header: signature, version, 6 reserved, channels, then height and width (big endian).
    if let Some(dims) = bytes.get(14..22) {
        let h = u64::from(u32::from_be_bytes([dims[0], dims[1], dims[2], dims[3]]));
        let w = u64::from(u32::from_be_bytes([dims[4], dims[5], dims[6], dims[7]]));
        if w * h > MAX_PIXELS {
            return Err(TOO_LARGE.to_string());
        }
    }
    let psd = psd::Psd::from_bytes(&bytes).map_err(|e| e.to_string())?;
    let (w, h) = (psd.width(), psd.height());
    image::RgbaImage::from_raw(w, h, psd.rgba())
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "PSD pixel buffer has the wrong size".to_string())
}

/// The embedded preview and the orientation stored beside it, for containers the EXIF reader
/// cannot open directly (RAF, CRW, MRW, X3F ...).
fn decode_raw_preview(path: &Path) -> Result<(DynamicImage, Option<u8>), String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let jpeg = raw::largest_embedded_jpeg(&bytes).ok_or("no embedded preview in this RAW file")?;
    let own = exif_info::read_bytes(jpeg).map(|e| exif_info::orientation(&e));
    let img = image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    Ok((img, own))
}

/// Full-resolution, upright pixels for any supported format.
pub fn load(path: &Path) -> Result<DynamicImage, String> {
    let ext = ext(path);
    let mut preview_orientation = None;
    let img = if ext == "psd" {
        decode_psd(path)?
    } else if is_raw(&ext) {
        let (img, own) = decode_raw_preview(path)?;
        preview_orientation = own;
        img
    } else {
        let mut reader = ImageReader::open(path)
            .and_then(ImageReader::with_guessed_format)
            .map_err(|e| e.to_string())?;
        let mut limits = Limits::default();
        limits.max_alloc = Some(MAX_ALLOC);
        reader.limits(limits);
        to_display(reader.decode().map_err(describe)?)
    };
    // PNG/WebP/GIF orientation is ignored by browsers too, so only photo formats are rotated.
    let file_orientation = || exif_info::read(path).map(|e| exif_info::orientation(&e));
    let orientation = if is_photo(&ext) {
        file_orientation().or(preview_orientation).unwrap_or(1)
    } else {
        1
    };
    Ok(upright(img, orientation))
}

/// Upright pixels that are enough to be shrunk to fit `max_w` x `max_h` (also upright): for a JPEG that
/// will shrink by 4x or more, only the fraction of the picture the box needs is decoded.
/// The result may be larger than the box; callers still run `resize::fit`.
pub fn load_fit(path: &Path, max_w: u32, max_h: u32) -> Result<DynamicImage, String> {
    if is_jpeg(&ext(path)) {
        let orientation = exif_info::read(path).map_or(1, |e| exif_info::orientation(&e));
        let (bw, bh) = if (5..=8).contains(&orientation) { (max_h, max_w) } else { (max_w, max_h) }; // the box is upright, the raster is not
        if let Some(img) = jpeg_scaled::decode(path, bw, bh, MAX_PIXELS)? {
            return Ok(upright(img, orientation));
        }
    }
    load(path)
}

fn is_jpeg(ext: &str) -> bool {
    matches!(ext, "jpg" | "jpeg" | "jpe" | "jfif" | "jif" | "jfi" | "pjpeg" | "pjp")
}

fn is_photo(ext: &str) -> bool {
    is_raw(ext) || matches!(ext, "jpg" | "jpeg" | "jpe" | "jfif" | "tif" | "tiff")
}

/// Upright pixel size from the file header only (cheap); `None` when it cannot be known without
/// decoding (PSD, RAW previews).
pub fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let ext = ext(path);
    if ext == "psd" || is_raw(&ext) {
        return None;
    }
    let (w, h) = image::image_dimensions(path).ok()?;
    let turned = is_photo(&ext)
        && exif_info::read(path).is_some_and(|e| (5..=8).contains(&exif_info::orientation(&e)));
    Some(if turned { (h, w) } else { (w, h) })
}

/// The EXIF preview of a photo, rotated upright, as JPEG; `None` when the file has none.
pub fn embedded_preview(path: &Path) -> Option<Vec<u8>> {
    let exif = exif_info::read(path)?;
    let thumb = exif_info::thumbnail(&exif)?;
    let img = image::load_from_memory_with_format(&thumb, image::ImageFormat::Jpeg).ok()?;
    crate::resize::to_jpeg(&upright(img, exif_info::orientation(&exif)), 85).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif_info::tests::{field, jpeg_with_exif, tiff_block};
    use exif::{In, Tag, Value};
    use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, GenericImageView};

    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let px: Vec<u8> = (0..w * h * 3).map(|i| (i % 200) as u8).collect();
        let mut out = Vec::new();
        JpegEncoder::new_with_quality(&mut out, 80)
            .encode(&px, w, h, ExtendedColorType::Rgb8)
            .unwrap();
        out
    }

    fn temp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-decode-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn jpeg_orientation_6_is_rotated_upright() {
        let dir = temp("orient");
        let tiff = tiff_block(
            &[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))],
            None,
        );
        std::fs::write(dir.join("p.jpg"), jpeg_with_exif(&jpeg(40, 20), &tiff)).unwrap();
        assert_eq!(load(&dir.join("p.jpg")).unwrap().dimensions(), (20, 40));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dimensions_come_from_the_header_and_follow_orientation() {
        let dir = temp("dims");
        let tiff = tiff_block(
            &[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))],
            None,
        );
        std::fs::write(dir.join("p.jpg"), jpeg_with_exif(&jpeg(40, 20), &tiff)).unwrap();
        assert_eq!(dimensions(&dir.join("p.jpg")), Some((20, 40)));
        std::fs::write(dir.join("flat.jpg"), jpeg(40, 20)).unwrap();
        assert_eq!(dimensions(&dir.join("flat.jpg")), Some((40, 20)));
        std::fs::write(dir.join("x.dng"), b"raw").unwrap();
        assert_eq!(dimensions(&dir.join("x.dng")), None);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn embedded_preview_is_extracted_and_oriented() {
        let dir = temp("preview");
        let thumb = jpeg(32, 16);
        let tiff = tiff_block(
            &[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))],
            Some(&thumb),
        );
        std::fs::write(dir.join("p.jpg"), jpeg_with_exif(&jpeg(400, 200), &tiff)).unwrap();
        let out = embedded_preview(&dir.join("p.jpg")).expect("preview");
        assert_eq!(
            image::load_from_memory(&out).unwrap().dimensions(),
            (16, 32)
        );
        std::fs::write(dir.join("plain.jpg"), jpeg(40, 20)).unwrap();
        assert!(embedded_preview(&dir.join("plain.jpg")).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn raw_files_show_their_largest_embedded_jpeg() {
        let dir = temp("raw");
        let mut blob = vec![0x49, 0x49, 0x2A, 0x00, 8, 0, 0, 0];
        blob.extend(jpeg(8, 8));
        blob.extend([0u8; 16]);
        blob.extend(jpeg(64, 48));
        std::fs::write(dir.join("shot.dng"), blob).unwrap();
        assert_eq!(load(&dir.join("shot.dng")).unwrap().dimensions(), (64, 48));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn raw_outside_a_tiff_container_is_turned_by_its_embedded_orientation() {
        let dir = temp("rawexif");
        let tiff = tiff_block(
            &[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))],
            None,
        );
        let mut blob = b"FUJIFILMCCD-RAW ".to_vec(); // RAF-like: not readable by the EXIF container reader
        blob.extend(jpeg_with_exif(&jpeg(64, 48), &tiff));
        std::fs::write(dir.join("shot.raf"), blob).unwrap();
        assert_eq!(load(&dir.join("shot.raf")).unwrap().dimensions(), (48, 64));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn netpbm_qoi_and_farbfeld_decode_by_content() {
        let dir = temp("misc");
        for name in ["a.ppm", "b.pgm", "c.qoi", "d.ff", "e.pam"] {
            let px = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                5,
                3,
                image::Rgba([10, 20, 30, 255]),
            ));
            // farbfeld stores 16 bits per channel, the other encoders take 8
            if name.ends_with(".ff") {
                DynamicImage::ImageRgba16(px.to_rgba16())
                    .save(dir.join(name))
                    .unwrap()
            } else {
                px.save(dir.join(name)).unwrap()
            }
            assert_eq!(
                load(&dir.join(name)).unwrap().dimensions(),
                (5, 3),
                "{name}"
            );
            assert_eq!(dimensions(&dir.join(name)), Some((5, 3)), "{name}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hdr_and_exr_are_tone_mapped_to_display_pixels() {
        let dir = temp("hdr");
        let half_grey = DynamicImage::ImageRgb32F(image::Rgb32FImage::from_pixel(
            4,
            2,
            image::Rgb([0.5, 0.5, 4.0]),
        ));
        for name in ["a.hdr", "b.exr"] {
            half_grey.save(dir.join(name)).unwrap();
            let img = load(&dir.join(name)).unwrap();
            assert_eq!(img.color(), image::ColorType::Rgb8, "{name}");
            let px = img.to_rgb8().get_pixel(0, 0).0;
            // linear 0.5 -> sRGB ~188 (a plain cast would give 127); values above 1.0 clip to white
            assert!((180..=195).contains(&px[0]), "{name}: {px:?}");
            assert_eq!(px[2], 255, "{name}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tiff_decodes_through_the_image_crate() {
        let dir = temp("tiff");
        image::RgbImage::from_pixel(12, 7, image::Rgb([1, 2, 3]))
            .save(dir.join("t.tif"))
            .unwrap();
        assert_eq!(load(&dir.join("t.tif")).unwrap().dimensions(), (12, 7));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tga_has_no_magic_number_so_it_is_decoded_by_extension() {
        let dir = temp("tga");
        image::RgbImage::from_pixel(9, 5, image::Rgb([9, 8, 7]))
            .save(dir.join("t.tga"))
            .unwrap();
        assert_eq!(load(&dir.join("t.tga")).unwrap().dimensions(), (9, 5));
        assert_eq!(dimensions(&dir.join("t.tga")), Some((9, 5)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn hand_written_tga_header_decodes() {
        let dir = temp("tga2");
        let mut tga = vec![0u8; 18];
        tga[2] = 2; // uncompressed true-colour
        tga[12..14].copy_from_slice(&4u16.to_le_bytes());
        tga[14..16].copy_from_slice(&3u16.to_le_bytes());
        tga[16] = 24;
        tga[17] = 0x20; // top-left origin
        tga.extend(std::iter::repeat_n(0x80, 4 * 3 * 3));
        std::fs::write(dir.join("h.tga"), &tga).unwrap();
        let r = load(&dir.join("h.tga"));
        assert_eq!(r.map(|i| i.dimensions()), Ok((4, 3)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_files_error_instead_of_panicking() {
        let dir = temp("bad");
        std::fs::write(dir.join("bad.png"), b"not a png").unwrap();
        std::fs::write(dir.join("bad.psd"), b"8BPS-nope").unwrap();
        assert!(load(&dir.join("bad.png")).is_err());
        assert!(load(&dir.join("bad.psd")).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for b in bytes {
            crc ^= u32::from(*b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn absurd_dimensions_are_refused_as_too_large() {
        let dir = temp("huge");
        // A real 1x1 PNG whose IHDR claims 60000x60000: decoding it would need ~14 GB.
        let mut png = Vec::new();
        image::RgbImage::new(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        png[16..20].copy_from_slice(&60000u32.to_be_bytes());
        png[20..24].copy_from_slice(&60000u32.to_be_bytes());
        let crc = crc32(&png[12..29]);
        png[29..33].copy_from_slice(&crc.to_be_bytes());
        std::fs::write(dir.join("huge.png"), &png).unwrap();
        assert_eq!(load(&dir.join("huge.png")).unwrap_err(), TOO_LARGE);

        let mut psd = b"8BPS".to_vec();
        psd.extend([0, 1, 0, 0, 0, 0, 0, 0, 0, 3]);
        psd.extend(50000u32.to_be_bytes());
        psd.extend(50000u32.to_be_bytes());
        std::fs::write(dir.join("huge.psd"), psd).unwrap();
        assert_eq!(load(&dir.join("huge.psd")).unwrap_err(), TOO_LARGE);
        let _ = std::fs::remove_dir_all(dir);
    }
}
