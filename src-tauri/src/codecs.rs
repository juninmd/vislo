//! Pure-Rust decoders for formats the `image` crate does not read (JPEG XL, JPEG 2000) and a
//! header-only size reader for SVG. Both bitmap decoders refuse oversized files before allocating.

use image::{DynamicImage, ImageDecoder};
use std::io::BufReader;
use std::path::Path;

fn within(dims: (u32, u32), max_pixels: u64) -> Result<(), String> {
    if u64::from(dims.0) * u64::from(dims.1) > max_pixels {
        return Err(crate::decode::TOO_LARGE.to_string());
    }
    Ok(())
}

pub fn decode_jxl(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let decoder = jxl_oxide::integration::JxlDecoder::new(BufReader::new(file)).map_err(|e| e.to_string())?;
    within(decoder.dimensions(), max_pixels)?;
    DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())
}

pub fn decode_jp2(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let image = hayro_jpeg2000::Image::new(&bytes, &hayro_jpeg2000::DecodeSettings::default()).map_err(|e| e.to_string())?;
    within((image.width(), image.height()), max_pixels)?;
    DynamicImage::from_decoder(image).map_err(|e| e.to_string())
}

/// Pixel size an SVG announces on its root element: `width`/`height` in px (or unitless), else the
/// `viewBox`. Only the first 4 KiB is read; relative units and missing sizes give `None`.
pub fn svg_dimensions(path: &Path) -> Option<(u32, u32)> {
    use std::io::Read;
    let mut head = Vec::with_capacity(4096);
    std::fs::File::open(path).ok()?.take(4096).read_to_end(&mut head).ok()?;
    let text = String::from_utf8_lossy(&head);
    let start = text.find("<svg")?;
    let tag = &text[start..start + text[start..].find('>')?];
    let attr = |name: &str| {
        let key = format!("{name}=");
        let at = tag.match_indices(&key).find(|(i, _)| tag[..*i].ends_with(char::is_whitespace))?.0 + key.len();
        let quote = tag[at..].chars().next().filter(|c| matches!(c, '"' | '\'' ))?;
        let rest = &tag[at + 1..];
        Some(&rest[..rest.find(quote)?])
    };
    let px = |v: &str| {
        let v = v.trim();
        let n = v.strip_suffix("px").unwrap_or(v).trim();
        n.parse::<f64>().ok().filter(|n| n.is_finite() && *n >= 1.0 && *n <= 65_535.0)
    };
    let (w, h) = (attr("width").and_then(px), attr("height").and_then(px));
    let view = || {
        let nums: Vec<f64> = attr("viewBox")?.split(|c: char| c == ',' || c.is_whitespace()).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
        let [_, _, vw, vh] = nums[..] else { return None };
        (vw >= 1.0 && vh >= 1.0 && vw <= 65_535.0 && vh <= 65_535.0).then_some((vw, vh))
    };
    let (w, h) = match (w, h) {
        (Some(w), Some(h)) => (w, h),
        (Some(w), None) => (w, view().map_or(w, |(vw, vh)| w * vh / vw)),
        (None, Some(h)) => (view().map_or(h, |(vw, vh)| h * vw / vh), h),
        (None, None) => view()?,
    };
    Some((w.round() as u32, h.round() as u32))
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use image::GenericImageView;

    // 16x8 gradient (x*16, y*32, 200), written with libjxl / OpenJPEG through Pillow, lossless.
    pub const JXL: &str = "ff0ac1412408000100a4054b28a3bae574e851186dc6f7151414b60f282838a2b1a260143132322285d11111318c88418c8c6d01ce3138c31823622086d8ea18cc6863b0728c312206c3314630c66243188359883106628cc6a6184c730c968831861708404009421482181b381c34060e870320263a80033848f7810b0017002e80952b5c00b80070c76800e002c0058018fc0b0017002e00c8645d00b8007001063ee102c0050019950b0017002e008c225f00b8000710b87a000770003e7d381c6e610c24972c4c111c1cf80a54f0c00ffc813f70bf6a0ffe0ffe0ffe0ffe5f7f19f01ffe1efc1ffc1fb459e70ff90ff80fa79f7f0ffc87fd83fe21fb5fffc17fe8df8422f01ff60f7ac8c12f077cf8038706828a8707c0a509000000000000000000000025000000000000000000a02c000000000000000c40086a0003180014d4c0c00080c4c018d012630c019004e037415ab35e99ad865869171ecc892fe968e407dbc6382b091b3a2b01510158090600";
    pub const JP2: &str = "0000000c6a5020200d0a870a00000014667479706a703220000000006a7032200000002d6a703268000000166968647200000008000000100003070700000000000f636f6c7201000000000010000000d06a703263ff4fff51002f000000000010000000080000000000000000000000100000000800000000000000000003070101070101070101ff52000c00000001000304040001ff5c000d4040484850484850484850ff640025000143726561746564206279204f70656e4a5045472076657273696f6e20322e352e34ff90000a00000000004f0001ff93df80200713ff7fc7d4040873cfb40c02659fc7da04000b91a7e00602655380c3ea0300158689a3ed05000b416fac0780c1f38400369dff7fa1f502000384a90f80ffd9";

    pub fn bytes(hex: &str) -> Vec<u8> {
        (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect()
    }

    fn temp(tag: &str, name: &str, data: &[u8]) -> std::path::PathBuf {
        // Tests run in parallel: every call gets its own directory.
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("vislo-codecs-{tag}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), data).unwrap();
        dir.join(name)
    }

    fn assert_gradient(img: &DynamicImage) {
        assert_eq!(img.dimensions(), (16, 8));
        let p = img.to_rgb8().get_pixel(5, 3).0;
        assert_eq!(p, [80, 96, 200], "lossless: the decoded pixel must equal the source");
    }

    #[test]
    fn jxl_decodes_losslessly() {
        let f = temp("jxl", "a.jxl", &bytes(JXL));
        assert_gradient(&decode_jxl(&f, 1_000).unwrap());
    }

    #[test]
    fn jp2_decodes_losslessly() {
        let f = temp("jp2", "a.jp2", &bytes(JP2));
        assert_gradient(&decode_jp2(&f, 1_000).unwrap());
    }

    #[test]
    fn oversized_images_are_refused_before_decoding() {
        let jxl = temp("big-jxl", "a.jxl", &bytes(JXL));
        let jp2 = temp("big-jp2", "a.jp2", &bytes(JP2));
        assert_eq!(decode_jxl(&jxl, 100).unwrap_err(), crate::decode::TOO_LARGE); // 128 px > 100
        assert_eq!(decode_jp2(&jp2, 100).unwrap_err(), crate::decode::TOO_LARGE);
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        let f = temp("junk", "a.jxl", b"definitely not an image");
        assert!(decode_jxl(&f, 1_000).is_err());
        assert!(decode_jp2(&f, 1_000).is_err());
    }

    fn svg(tag: &str) -> Option<(u32, u32)> {
        let f = temp("svg", "a.svg", format!("<?xml version=\"1.0\"?>\n<!-- c -->{tag}</svg>").as_bytes());
        svg_dimensions(&f)
    }

    #[test]
    fn svg_size_comes_from_width_height_then_viewbox() {
        assert_eq!(svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80px">"#), Some((120, 80)));
        assert_eq!(svg(r#"<svg viewBox="0 0 24 12">"#), Some((24, 12)));
        assert_eq!(svg(r#"<svg width='200' viewBox='0 0 100 50'>"#), Some((200, 100)));
        assert_eq!(svg(r#"<svg viewBox="0,0,48,48" stroke-width="2">"#), Some((48, 48)));
    }

    #[test]
    fn svg_without_a_usable_size_is_unknown() {
        assert_eq!(svg(r#"<svg width="100%" height="100%">"#), None);
        assert_eq!(svg(r#"<svg>"#), None);
        assert_eq!(svg(r#"<svg width="0" height="0">"#), None);
        assert_eq!(svg_dimensions(Path::new("missing.svg")), None);
    }
}
