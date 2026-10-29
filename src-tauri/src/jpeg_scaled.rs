//! Decoding a JPEG straight to a fraction of its size. The IDCT can emit 1/2, 1/4 or 1/8 of the
//! pixels, so a 24 MP photo headed for a 192 px thumbnail never materialises as 24 MP.

use image::{DynamicImage, GrayImage, RgbImage};
use jpeg_decoder::PixelFormat;
use std::io::BufReader;
use std::path::Path;

/// Only shrinks of 4x or more are worth it, see the benchmark in `bench.rs`.
const MAX_SHRINK_RATIO: f64 = 0.25;

/// `Ok(None)` when scaling cannot or should not be used (a shrink under 4x, CMYK, an unreadable header):
/// the caller then takes the normal full decode, which also reports the real error.
/// The returned image is at least `box_w` x `box_h` scaled to the picture's aspect, never smaller.
pub fn decode(path: &Path, box_w: u32, box_h: u32, max_pixels: u64) -> Result<Option<DynamicImage>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut dec = jpeg_decoder::Decoder::new(BufReader::new(file));
    if dec.read_info().is_err() {
        return Ok(None);
    }
    let Some(info) = dec.info() else { return Ok(None) };
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    if u64::from(w) * u64::from(h) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    let k = (f64::from(box_w) / f64::from(w)).min(f64::from(box_h) / f64::from(h));
    // Measured: at 1/2 this decoder is no faster than the full SIMD decode; from 1/4 on it wins.
    if k > MAX_SHRINK_RATIO || box_w == 0 || box_h == 0 {
        return Ok(None);
    }
    let (want_w, want_h) = (((f64::from(w) * k).round() as u32).max(1), ((f64::from(h) * k).round() as u32).max(1));
    let Ok((sw, sh)) = dec.scale(want_w.min(u32::from(u16::MAX)) as u16, want_h.min(u32::from(u16::MAX)) as u16) else {
        return Ok(None);
    };
    let (sw, sh) = (u32::from(sw), u32::from(sh));
    if sw < want_w || sh < want_h || (sw, sh) == (w, h) {
        return Ok(None);
    }
    let Ok(data) = dec.decode() else { return Ok(None) };
    Ok(match info.pixel_format {
        PixelFormat::RGB24 => RgbImage::from_raw(sw, sh, data).map(DynamicImage::ImageRgb8),
        PixelFormat::L8 => GrayImage::from_raw(sw, sh, data).map(DynamicImage::ImageLuma8),
        _ => None, // CMYK and 16-bit grey need the full decoder's colour handling
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exif_info::tests::{field, jpeg_with_exif, tiff_block};
    use crate::{decode, resize};
    use exif::{In, Tag, Value};
    use fast_image_resize::FilterType;
    use image::{codecs::jpeg::JpegEncoder, ExtendedColorType, GenericImageView};

    /// Smooth gradients plus a sine texture: the kind of content a downscale must preserve.
    fn jpeg(w: u32, h: u32, gray: bool) -> Vec<u8> {
        let mut px = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let t = ((x as f32 * 0.05).sin() * 30.0) as i32;
                let (r, g, b) = ((x * 255 / w) as i32 + t, (y * 255 / h) as i32, 128 + t);
                if gray {
                    px.push(((r + g) / 2).clamp(0, 255) as u8);
                } else {
                    px.extend([r, g, b].map(|c| c.clamp(0, 255) as u8));
                }
            }
        }
        let mut out = Vec::new();
        let kind = if gray { ExtendedColorType::L8 } else { ExtendedColorType::Rgb8 };
        JpegEncoder::new_with_quality(&mut out, 92).encode(&px, w, h, kind).unwrap();
        out
    }

    fn file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vislo-jscaled-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("p.jpg"), bytes).unwrap();
        dir.join("p.jpg")
    }

    fn mean_abs_error(a: &image::DynamicImage, b: &image::DynamicImage) -> f64 {
        let (a, b) = (a.to_rgb8(), b.to_rgb8());
        let sum: u64 = a.as_raw().iter().zip(b.as_raw()).map(|(x, y)| u64::from(x.abs_diff(*y))).sum();
        sum as f64 / a.as_raw().len() as f64
    }

    #[test]
    fn scaled_pipeline_matches_the_full_one() {
        let f = file("match", &jpeg(1600, 1200, false));
        let fast = resize::fit_owned(decode::load_fit(&f, 200, 150).unwrap(), 200, 150, FilterType::CatmullRom).unwrap();
        let slow = resize::fit(&decode::load(&f).unwrap(), 200, 150, FilterType::CatmullRom).unwrap();
        assert_eq!(fast.dimensions(), (200, 150));
        let err = mean_abs_error(&fast, &slow);
        assert!(err < 3.0, "mean abs error {err} / 255");
    }

    #[test]
    fn the_decoder_really_returns_a_fraction_of_the_pixels() {
        let f = file("fraction", &jpeg(1600, 1200, false));
        let img = decode(&f, 200, 150, 150_000_000).unwrap().expect("1/8 applies");
        assert_eq!(img.dimensions(), (200, 150));
    }

    #[test]
    fn small_shrinks_and_unusable_files_fall_back_to_the_full_decode() {
        let f = file("fallback", &jpeg(1600, 1200, false));
        assert!(decode(&f, 800, 600, 150_000_000).unwrap().is_none(), "a 2x shrink is not worth it");
        let junk = file("junk", b"\xFF\xD8\xFF\xE0 truncated");
        assert!(decode(&junk, 10, 10, 150_000_000).unwrap().is_none());
        assert!(decode::load_fit(&junk, 10, 10).is_err(), "the full decoder reports the real error");
    }

    #[test]
    fn oversized_headers_are_refused_before_decoding() {
        let f = file("big", &jpeg(1600, 1200, false));
        assert_eq!(decode(&f, 100, 100, 1_000_000).unwrap_err(), crate::decode::TOO_LARGE);
    }

    #[test]
    fn exif_orientation_is_applied_and_the_box_stays_upright() {
        let tiff = tiff_block(&[field(Tag::Orientation, In::PRIMARY, Value::Short(vec![6]))], None);
        let f = file("orient", &jpeg_with_exif(&jpeg(1200, 800, false), &tiff));
        let img = decode::load_fit(&f, 100, 150).unwrap();
        assert!(img.height() > img.width(), "rotated upright: {:?}", img.dimensions());
        let fit = resize::fit_owned(img, 100, 150, FilterType::CatmullRom).unwrap();
        let legacy = resize::fit(&decode::load(&f).unwrap(), 100, 150, FilterType::CatmullRom).unwrap();
        assert_eq!(fit.dimensions(), legacy.dimensions());
        assert!(mean_abs_error(&fit, &legacy) < 3.0);
    }

    #[test]
    fn grayscale_jpegs_scale_too() {
        let f = file("gray", &jpeg(1600, 1200, true));
        let img = decode(&f, 200, 150, 150_000_000).unwrap().expect("grayscale is handled");
        assert_eq!((img.dimensions(), img.color()), ((200, 150), image::ColorType::L8));
    }
}
