//! Windows metafiles (`.wmf`, `.emf`) drawn by GDI+, which ships with Windows. They are vector pictures, so they are
//! rendered over white at no less than 1024 px on the long side, the same way small SVGs are enlarged.

use image::DynamicImage;
use std::path::Path;

#[cfg(windows)]
pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    use std::os::windows::ffi::OsStrExt;
    use std::sync::Mutex;
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::GdiPlus::*;

    // GDI+ keeps process-wide state behind its startup token; decoding one metafile at a time keeps that simple.
    static GDIP: Mutex<()> = Mutex::new(());
    const PIXEL_FORMAT_32BPP_PARGB: i32 = 0x000E_200B;

    let ok = |s: Status| if s == Ok { Result::Ok(()) } else { Err(format!("GDI+ could not draw this file (status {})", s.0)) };
    let _serial = GDIP.lock().unwrap_or_else(|e| e.into_inner());
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut pixels = Vec::new();
    let mut size = (0u32, 0u32);
    // SAFETY: every GDI+ object created below is released before returning, in every path; `pixels` outlives the
    // bitmap that draws into it, and the bitmap is disposed before the buffer is read.
    unsafe {
        let mut token = 0usize;
        let input = GdiplusStartupInput { GdiplusVersion: 1, ..Default::default() };
        ok(GdiplusStartup(&mut token, &input, std::ptr::null_mut()))?;
        let (mut image, mut bitmap, mut graphics) = (std::ptr::null_mut::<GpImage>(), std::ptr::null_mut::<GpBitmap>(), std::ptr::null_mut::<GpGraphics>());
        let result = (|| -> Result<(), String> {
            ok(GdipLoadImageFromFile(PCWSTR(wide.as_ptr()), &mut image))?;
            let (mut w, mut h) = (0u32, 0u32);
            ok(GdipGetImageWidth(image, &mut w))?;
            ok(GdipGetImageHeight(image, &mut h))?;
            if w == 0 || h == 0 {
                return Err("this metafile has no size".to_string());
            }
            let scale = (1024.0 / f64::from(w.max(h))).max(1.0);
            let (tw, th) = ((f64::from(w) * scale).round() as u64, (f64::from(h) * scale).round() as u64);
            if tw.saturating_mul(th) > max_pixels || tw > 32_000 || th > 32_000 {
                return Err(crate::decode::TOO_LARGE.to_string());
            }
            size = (tw as u32, th as u32);
            pixels = vec![0u8; tw as usize * th as usize * 4];
            ok(GdipCreateBitmapFromScan0(size.0 as i32, size.1 as i32, size.0 as i32 * 4, PIXEL_FORMAT_32BPP_PARGB, Some(pixels.as_mut_ptr().cast_const()), &mut bitmap))?;
            ok(GdipGetImageGraphicsContext(bitmap.cast(), &mut graphics))?;
            ok(GdipGraphicsClear(graphics, 0xFFFF_FFFF))?;
            ok(GdipSetSmoothingMode(graphics, SmoothingModeAntiAlias))?;
            ok(GdipSetInterpolationMode(graphics, InterpolationModeHighQualityBicubic))?;
            ok(GdipDrawImageRectI(graphics, image, 0, 0, size.0 as i32, size.1 as i32))
        })();
        if !graphics.is_null() {
            GdipDeleteGraphics(graphics);
        }
        if !bitmap.is_null() {
            GdipDisposeImage(bitmap.cast());
        }
        if !image.is_null() {
            GdipDisposeImage(image);
        }
        GdiplusShutdown(token);
        result?;
    }
    // Premultiplied BGRA -> straight RGBA (the page is opaque white, so alpha is 255 and this is a channel swap).
    for p in pixels.as_chunks_mut::<4>().0 {
        let a = u32::from(p[3]);
        let un = |c: u8| if a == 0 || a == 255 { c } else { (u32::from(c) * 255 / a).min(255) as u8 };
        (p[0], p[2]) = (un(p[2]), un(p[0]));
        p[1] = un(p[1]);
    }
    image::RgbaImage::from_raw(size.0, size.1, pixels).map(DynamicImage::ImageRgba8).ok_or_else(|| "decoded buffer has the wrong size".to_string())
}

#[cfg(not(windows))]
pub fn decode(_path: &Path, _max_pixels: u64) -> Result<DynamicImage, String> {
    Err("metafiles are drawn by Windows, which this platform does not have".to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use image::GenericImageView;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::*;

    /// An enhanced metafile with a red square in its middle, recorded through GDI itself (frame in 0.01 mm).
    fn write_emf(path: &Path, frame: RECT) {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: test-only GDI calls on handles created and released in this function.
        unsafe {
            let dc = CreateEnhMetaFileW(None, PCWSTR(wide.as_ptr()), Some(&frame), PCWSTR::null());
            let brush = CreateSolidBrush(COLORREF(0x0000_00FF)); // 0x00BBGGRR: red
            let old = SelectObject(dc, brush.into());
            SetMapMode(dc, MM_HIMETRIC); // 0.01 mm units like the frame, y pointing up
            let _ = Rectangle(dc, 800, -400, 1200, -600);
            SelectObject(dc, old);
            let _ = DeleteObject(brush.into());
            let meta = CloseEnhMetaFile(dc);
            let _ = DeleteEnhMetaFile(Some(meta));
        }
    }

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-gdip-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn emf_is_drawn_over_white_at_a_usable_size() {
        let d = dir("emf");
        write_emf(&d.join("a.emf"), RECT { left: 0, top: 0, right: 2000, bottom: 1000 });
        let img = decode(&d.join("a.emf"), 10_000_000).unwrap();
        let (w, h) = img.dimensions();
        assert!(w.max(h) >= 1024 && (f64::from(w) / f64::from(h) - 2.0).abs() < 0.1, "2:1 picture enlarged, got {w}x{h}");
        let rgb = img.to_rgb8();
        let p = rgb.get_pixel(w / 2, h / 2).0;
        assert!(p[0] > 200 && p[1] < 60 && p[2] < 60, "the red fill must survive the BGR/RGB swap, got {p:?}");
        assert_eq!(img.get_pixel(2, 2).0, [255, 255, 255, 255], "empty areas are opaque white, not transparent");
        assert_eq!(decode(&d.join("a.emf"), 100).unwrap_err(), crate::decode::TOO_LARGE);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_metafile_wider_than_the_side_limit_is_refused() {
        let d = dir("wide");
        write_emf(&d.join("w.emf"), RECT { left: 0, top: 0, right: 2_000_000, bottom: 100 }); // 20 m wide
        assert_eq!(decode(&d.join("w.emf"), u64::MAX).unwrap_err(), crate::decode::TOO_LARGE);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn files_gdi_plus_cannot_draw_are_errors() {
        let d = dir("junk");
        std::fs::write(d.join("a.wmf"), b"definitely not a metafile").unwrap();
        assert!(decode(&d.join("a.wmf"), 1_000_000).is_err());
        assert!(decode(&d.join("missing.emf"), 1_000_000).is_err());
        let _ = std::fs::remove_dir_all(d);
    }
}
