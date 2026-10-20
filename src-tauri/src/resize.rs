//! High-quality downscaling (SIMD convolution) and JPEG/PNG encoding for the `vislo` protocol.

use fast_image_resize::{images::Image, FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::{codecs::jpeg::JpegEncoder, DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder};

/// Scales `img` down to fit inside `max_w` x `max_h`, never upscaling; aspect ratio is kept.
pub fn fit(img: &DynamicImage, max_w: u32, max_h: u32, filter: FilterType) -> Result<DynamicImage, String> {
    let (sw, sh) = img.dimensions();
    if max_w == 0 || max_h == 0 || sw == 0 || sh == 0 {
        return Err("empty image".into());
    }
    if sw <= max_w && sh <= max_h {
        return Ok(img.clone());
    }
    let k = (max_w as f64 / sw as f64).min(max_h as f64 / sh as f64);
    let (dw, dh) = (((sw as f64 * k).round() as u32).max(1), ((sh as f64 * k).round() as u32).max(1));

    let alpha = img.color().has_alpha();
    let (buf, px) = if alpha { (img.to_rgba8().into_raw(), PixelType::U8x4) } else { (img.to_rgb8().into_raw(), PixelType::U8x3) };
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

/// Encodes as JPEG, flattening transparency over the viewer's dark background.
pub fn to_jpeg(img: &DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
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
        .map_err(|e| e.to_string())?;
    Ok(out)
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
