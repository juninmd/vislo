//! High-quality downscaling (SIMD convolution) and JPEG/PNG encoding for the `vislo` protocol.

use fast_image_resize::{images::Image, FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{DynamicImage, GenericImageView};

/// Scales `img` down to fit inside `max_w` x `max_h`, never upscaling; aspect ratio is kept.
pub fn fit(img: &DynamicImage, max_w: u32, max_h: u32, filter: FilterType) -> Result<DynamicImage, String> {
    let Some((dw, dh)) = target(img, max_w, max_h)? else { return Ok(img.clone()) };
    let alpha = img.color().has_alpha();
    let buf = if alpha { img.to_rgba8().into_raw() } else { img.to_rgb8().into_raw() };
    shrink(img.dimensions(), (dw, dh), alpha, buf, filter)
}

/// Same as [`fit`] for an image nobody needs afterwards: an RGB8/RGBA8 buffer is reused instead of
/// copied (a 24 MP photo is 72 MB, about 35 ms of memcpy).
pub fn fit_owned(img: DynamicImage, max_w: u32, max_h: u32, filter: FilterType) -> Result<DynamicImage, String> {
    let Some((dw, dh)) = target(&img, max_w, max_h)? else { return Ok(img) };
    let (dims, alpha) = (img.dimensions(), img.color().has_alpha());
    let buf = if alpha { img.into_rgba8().into_raw() } else { img.into_rgb8().into_raw() };
    shrink(dims, (dw, dh), alpha, buf, filter)
}

/// Size `img` shrinks to inside the box, or `None` when it already fits.
fn target(img: &DynamicImage, max_w: u32, max_h: u32) -> Result<Option<(u32, u32)>, String> {
    let (sw, sh) = img.dimensions();
    if max_w == 0 || max_h == 0 || sw == 0 || sh == 0 {
        return Err("empty image".into());
    }
    if sw <= max_w && sh <= max_h {
        return Ok(None);
    }
    let k = (max_w as f64 / sw as f64).min(max_h as f64 / sh as f64);
    Ok(Some((((sw as f64 * k).round() as u32).max(1), ((sh as f64 * k).round() as u32).max(1))))
}

fn shrink((sw, sh): (u32, u32), (dw, dh): (u32, u32), alpha: bool, buf: Vec<u8>, filter: FilterType) -> Result<DynamicImage, String> {
    let px = if alpha { PixelType::U8x4 } else { PixelType::U8x3 };
    let src = Image::from_vec_u8(sw, sh, buf, px).map_err(|e| e.to_string())?;
    let mut dst = Image::new(dw, dh, px);
    let opts = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(filter));
    Resizer::new().resize(&src, &mut dst, &opts).map_err(|e| e.to_string())?;
    let raw = dst.into_vec();
    let out = if alpha {
        image::RgbaImage::from_raw(dw, dh, raw).map(DynamicImage::ImageRgba8)
    } else {
        image::RgbImage::from_raw(dw, dh, raw).map(DynamicImage::ImageRgb8)
    };
    out.ok_or_else(|| "resize produced a malformed buffer".to_string())
}

/// Viewer background that transparent pixels are flattened onto when encoding as JPEG.
const BACKGROUND: [u8; 3] = [14, 14, 16];

/// Encodes as JPEG, flattening transparency over the viewer's dark background.
/// RGB8 input (every resized image) is encoded in place: a 24 MP copy costs tens of milliseconds.
pub fn to_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let owned;
    let rgb: &image::RgbImage = match img {
        DynamicImage::ImageRgb8(rgb) => rgb,
        DynamicImage::ImageRgba8(rgba) => {
            owned = flatten(rgba);
            &owned
        }
        _ if img.color().has_alpha() => {
            owned = flatten(&img.to_rgba8());
            &owned
        }
        _ => {
            owned = img.to_rgb8();
            &owned
        }
    };
    let (w, h) = (u16::try_from(rgb.width()), u16::try_from(rgb.height()));
    let (Ok(w), Ok(h)) = (w, h) else { return Err("image too large for JPEG".into()) };
    let mut out = Vec::with_capacity(rgb.as_raw().len() / 8);
    let mut enc = jpeg_encoder::Encoder::new(&mut out, quality);
    // Full-resolution colour at every quality, as before: same file size and fidelity, only faster.
    enc.set_sampling_factor(jpeg_encoder::SamplingFactor::F_1_1);
    enc.encode(rgb.as_raw(), w, h, jpeg_encoder::ColorType::Rgb).map_err(|e| e.to_string())?;
    Ok(out)
}

/// Straight-alpha "over" onto the opaque background, written once into the RGB buffer the encoder reads.
fn flatten(rgba: &image::RgbaImage) -> image::RgbImage {
    let mut out = Vec::with_capacity(rgba.as_raw().len() / 4 * 3);
    for p in rgba.as_raw().as_chunks::<4>().0 {
        let a = u32::from(p[3]);
        for (c, bg) in p[..3].iter().zip(BACKGROUND) {
            out.push(((u32::from(*c) * a + u32::from(bg) * (255 - a) + 127) / 255) as u8);
        }
    }
    image::RgbImage::from_raw(rgba.width(), rgba.height(), out).expect("3 bytes per pixel")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(w, h, image::Rgb([200, 40, 40])))
    }

    #[test]
    fn fits_inside_the_box_keeping_aspect() {
        let out = fit(&solid(4000, 3000), 800, 800, FilterType::Lanczos3).unwrap();
        assert_eq!(out.dimensions(), (800, 600));
    }

    #[test]
    fn owned_fit_matches_the_borrowed_one_and_keeps_alpha() {
        let ramp = DynamicImage::ImageRgb8(image::RgbImage::from_fn(400, 300, |x, y| image::Rgb([x as u8, y as u8, (x ^ y) as u8])));
        let a = fit(&ramp, 100, 100, FilterType::Lanczos3).unwrap();
        let b = fit_owned(ramp, 100, 100, FilterType::Lanczos3).unwrap();
        assert_eq!(a.as_bytes(), b.as_bytes());
        let ga = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(80, 40, image::Rgba([9, 9, 9, 77])));
        let out = fit_owned(ga, 40, 40, FilterType::Lanczos3).unwrap();
        assert_eq!((out.dimensions(), out.color().has_alpha()), ((40, 20), true));
        assert_eq!(fit_owned(solid(10, 10), 50, 50, FilterType::Lanczos3).unwrap().dimensions(), (10, 10));
    }

    #[test]
    fn jpeg_keeps_colour_detail_at_full_resolution() {
        // One-pixel colour stripes vanish under chroma subsampling; at full resolution they survive.
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_fn(64, 64, |x, _| {
            if x % 2 == 0 { image::Rgb([220, 30, 30]) } else { image::Rgb([30, 30, 220]) }
        }));
        let back = image::load_from_memory(&to_jpeg(&img, 90).unwrap()).unwrap().to_rgb8();
        let red = back.get_pixel(10, 10);
        assert!(red[0] > 150 && red[2] < 110, "{red:?}");
    }

    #[test]
    fn never_upscales() {
        let out = fit(&solid(300, 200), 800, 800, FilterType::Lanczos3).unwrap();
        assert_eq!(out.dimensions(), (300, 200));
    }

    #[test]
    fn rejects_empty_boxes() {
        assert!(fit(&solid(10, 10), 0, 5, FilterType::Lanczos3).is_err());
    }

    #[test]
    fn jpeg_flattens_transparent_pixels_onto_the_dark_background() {
        let img = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 255, 255, 0])));
        let decoded = image::load_from_memory(&to_jpeg(&img, 95).unwrap()).unwrap().to_rgb8();
        let p = decoded.get_pixel(4, 4);
        assert!(p[0] < 40 && p[1] < 40 && p[2] < 40, "{p:?}");
    }
}
