//! Formats decoded by the codecs Windows ships or offers: JPEG XR out of the box, HEIC/HEIF once the user has
//! the Microsoft HEIF and HEVC extensions. Going through the Windows Imaging Component keeps patent-encumbered
//! and copyleft decoders out of Vislo; without the codec the file reports a clear error instead.

use image::DynamicImage;
use std::path::Path;

#[cfg(windows)]
pub fn decode(path: &Path, max_pixels: u64) -> Result<DynamicImage, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::GENERIC_READ;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

    // COM must be initialised once per worker thread; a thread already in another apartment is fine as is.
    thread_local!(static COM: () = unsafe { let _ = CoInitializeEx(None, COINIT_MULTITHREADED); });
    COM.with(|_| ());

    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
    let hint = |e: windows::core::Error| {
        let more = if matches!(ext.as_str(), "heic" | "heif" | "hif") { "; HEIC needs the HEIF and HEVC extensions from the Microsoft Store" } else { "" };
        format!("Windows has no working codec for this file ({}){more}", e.message().trim())
    };
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` outlives the calls and is NUL terminated; every interface is owned and released by `windows`.
    unsafe {
        let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(hint)?;
        let decoder = factory
            .CreateDecoderFromFilename(PCWSTR(wide.as_ptr()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(hint)?;
        let frame = decoder.GetFrame(0).map_err(hint)?;
        let (mut w, mut h) = (0u32, 0u32);
        frame.GetSize(&mut w, &mut h).map_err(hint)?;
        if w == 0 || h == 0 || u64::from(w) * u64::from(h) > max_pixels {
            return Err(crate::decode::TOO_LARGE.to_string());
        }
        let converter = factory.CreateFormatConverter().map_err(hint)?;
        converter
            .Initialize(&frame, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None::<&IWICPalette>, 0.0, WICBitmapPaletteTypeCustom)
            .map_err(hint)?;
        let mut pixels = vec![0u8; w as usize * h as usize * 4];
        converter.CopyPixels(std::ptr::null(), w * 4, &mut pixels).map_err(hint)?;
        image::RgbaImage::from_raw(w, h, pixels).map(DynamicImage::ImageRgba8).ok_or_else(|| "decoded buffer has the wrong size".to_string())
    }
}

#[cfg(not(windows))]
pub fn decode(_path: &Path, _max_pixels: u64) -> Result<DynamicImage, String> {
    Err("this format is decoded by Windows codecs, which this platform does not have".to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use image::GenericImageView;
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::GENERIC_WRITE;
    use windows::Win32::Graphics::Imaging::*;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};

    /// Writes a JPEG XR file through WIC's own encoder, the only JXR source available without a fixture.
    fn write_jxr(path: &Path, w: u32, h: u32, rgb: [u8; 3]) {
        use std::os::windows::ffi::OsStrExt;
        let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let bgr: Vec<u8> = (0..w * h).flat_map(|_| [rgb[2], rgb[1], rgb[0]]).collect();
        // SAFETY: test-only COM calls on interfaces owned by this function.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let factory: IWICImagingFactory = CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).unwrap();
            let stream = factory.CreateStream().unwrap();
            stream.InitializeFromFilename(PCWSTR(wide.as_ptr()), GENERIC_WRITE.0).unwrap();
            let encoder = factory.CreateEncoder(&GUID_ContainerFormatWmp, std::ptr::null()).unwrap();
            encoder.Initialize(&stream, WICBitmapEncoderNoCache).unwrap();
            let (mut frame, mut options) = (None, None);
            encoder.CreateNewFrame(&mut frame, &mut options).unwrap();
            let frame = frame.unwrap();
            frame.Initialize(options.as_ref()).unwrap();
            frame.SetSize(w, h).unwrap();
            let mut format = GUID_WICPixelFormat24bppBGR;
            frame.SetPixelFormat(&mut format).unwrap();
            assert_eq!(format, GUID_WICPixelFormat24bppBGR, "the JXR encoder must keep the requested layout");
            frame.WritePixels(h, w * 3, &bgr).unwrap();
            frame.Commit().unwrap();
            encoder.Commit().unwrap();
        }
    }

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-wic-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn jpeg_xr_decodes_through_windows() {
        let d = dir("jxr");
        write_jxr(&d.join("a.jxr"), 8, 6, [200, 100, 50]);
        let img = decode(&d.join("a.jxr"), 1_000).unwrap();
        assert_eq!(img.dimensions(), (8, 6));
        let p = img.to_rgb8().get_pixel(3, 3).0;
        assert!(p.iter().zip([200u8, 100, 50]).all(|(a, b)| a.abs_diff(b) <= 4), "colour survives the round trip, got {p:?}");
        assert_eq!(decode(&d.join("a.jxr"), 10).unwrap_err(), crate::decode::TOO_LARGE);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn files_no_codec_can_read_are_errors_not_panics() {
        let d = dir("junk");
        for name in ["a.heic", "a.jxr", "a.hdp"] {
            std::fs::write(d.join(name), b"definitely not an image").unwrap();
            assert!(decode(&d.join(name), 1_000).is_err(), "{name}");
        }
        assert!(decode(&d.join("missing.heic"), 1_000).unwrap_err().contains("HEIF"));
        let _ = std::fs::remove_dir_all(d);
    }
}
