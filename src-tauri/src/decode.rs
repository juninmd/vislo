//! Decoding for everything the webview cannot show itself, and for thumbnails of everything.

use crate::{exif_info, raw};
use image::{DynamicImage, ImageReader};
use std::path::Path;

fn ext(path: &Path) -> String {
    path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase()
}

fn is_raw(ext: &str) -> bool {
    matches!(ext, "dng" | "cr2" | "nef" | "arw" | "orf" | "rw2")
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
    let psd = psd::Psd::from_bytes(&bytes).map_err(|e| e.to_string())?;
    let (w, h) = (psd.width(), psd.height());
    image::RgbaImage::from_raw(w, h, psd.rgba())
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| "PSD pixel buffer has the wrong size".to_string())
}

fn decode_raw_preview(path: &Path) -> Result<DynamicImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let jpeg = raw::largest_embedded_jpeg(&bytes).ok_or("no embedded preview in this RAW file")?;
    image::load_from_memory_with_format(jpeg, image::ImageFormat::Jpeg).map_err(|e| e.to_string())
}

/// Full-resolution, upright pixels for any supported format.
pub fn load(path: &Path) -> Result<DynamicImage, String> {
    let ext = ext(path);
    let img = if ext == "psd" {
        decode_psd(path)?
    } else if is_raw(&ext) {
        decode_raw_preview(path)?
    } else {
        ImageReader::open(path)
            .and_then(ImageReader::with_guessed_format)
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?
    };
    // PNG/WebP/GIF orientation is ignored by browsers too, so only photo formats are rotated.
    let orientation = if is_photo(&ext) { exif_info::read(path).map_or(1, |e| exif_info::orientation(&e)) } else { 1 };
    Ok(upright(img, orientation))
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
    let turned = is_photo(&ext) && exif_info::read(path).is_some_and(|e| (5..=8).contains(&exif_info::orientation(&e)));
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
        JpegEncoder::new_with_quality(&mut out, 80).encode(&px, w, h, ExtendedColorType::Rgb8).unwrap();
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
        let tiff = tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))], None);
        std::fs::write(dir.join("p.jpg"), jpeg_with_exif(&jpeg(40, 20), &tiff)).unwrap();
        assert_eq!(load(&dir.join("p.jpg")).unwrap().dimensions(), (20, 40));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn dimensions_come_from_the_header_and_follow_orientation() {
        let dir = temp("dims");
        let tiff = tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))], None);
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
        let tiff = tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))], Some(&thumb));
        std::fs::write(dir.join("p.jpg"), jpeg_with_exif(&jpeg(400, 200), &tiff)).unwrap();
        let out = embedded_preview(&dir.join("p.jpg")).expect("preview");
        assert_eq!(image::load_from_memory(&out).unwrap().dimensions(), (16, 32));
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
    fn tiff_decodes_through_the_image_crate() {
        let dir = temp("tiff");
        image::RgbImage::from_pixel(12, 7, image::Rgb([1, 2, 3])).save(dir.join("t.tif")).unwrap();
        assert_eq!(load(&dir.join("t.tif")).unwrap().dimensions(), (12, 7));
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
}
