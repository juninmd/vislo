//! Desktop wallpaper and lock screen. Windows reads BMP, JPEG and PNG itself; any other format is
//! decoded and written once as a PNG that stays on disk (the system keeps referring to it).

use crate::decode;
use std::path::{Path, PathBuf};

/// Where the picture handed to Windows lives: the source itself or a converted copy to write first.
#[derive(Debug, PartialEq, Eq)]
pub enum Source {
    Direct(PathBuf),
    Convert(PathBuf),
}

/// BMP, JPEG and PNG go to Windows untouched; everything else becomes `<dir>/<name>.png`.
/// A fixed name keeps at most one converted copy per purpose, so nothing piles up.
pub fn plan(src: &Path, dir: &Path, name: &str) -> Source {
    let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "bmp") {
        Source::Direct(src.to_path_buf())
    } else {
        Source::Convert(dir.join(format!("{name}.png")))
    }
}

fn prepare(src: &Path, dir: &Path, name: &str) -> Result<PathBuf, String> {
    match plan(src, dir, name) {
        Source::Direct(path) => Ok(path),
        Source::Convert(out) => {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            decode::load(src)?.to_rgb8().save(&out).map_err(|e| e.to_string())?;
            Ok(out)
        }
    }
}

/// Sets the desktop wallpaper; the fit mode (fill, fit, tile ...) stays what the user chose in Windows.
pub fn set_wallpaper(src: &Path, dir: &Path) -> Result<(), String> {
    apply_wallpaper(&prepare(src, dir, "wallpaper")?)
}

/// Sets the lock screen picture of the signed-in user.
pub fn set_lock_screen(src: &Path, dir: &Path) -> Result<(), String> {
    apply_lock_screen(&prepare(src, dir, "lockscreen")?)
}

#[cfg(windows)]
fn apply_wallpaper(file: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
    };
    let mut wide: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: `wide` is a NUL-terminated path that outlives the call.
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_mut_ptr().cast()),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    }
    .map_err(|e| e.to_string())
}

#[cfg(windows)]
fn apply_lock_screen(file: &Path) -> Result<(), String> {
    use windows::core::HSTRING;
    use windows::Storage::StorageFile;
    use windows::System::UserProfile::UserProfilePersonalizationSettings;
    let fail = |e: windows::core::Error| e.to_string();
    if !UserProfilePersonalizationSettings::IsSupported().map_err(fail)? {
        return Err("changing the lock screen is not supported on this device".into());
    }
    let item = StorageFile::GetFileFromPathAsync(&HSTRING::from(file.to_string_lossy().as_ref()))
        .map_err(fail)?
        .join()
        .map_err(fail)?;
    let done = UserProfilePersonalizationSettings::Current()
        .map_err(fail)?
        .TrySetLockScreenImageAsync(&item)
        .map_err(fail)?
        .join()
        .map_err(fail)?;
    if done {
        Ok(())
    } else {
        Err("Windows did not accept the picture as the lock screen".into())
    }
}

#[cfg(not(windows))]
fn apply_wallpaper(_file: &Path) -> Result<(), String> {
    Err("setting the wallpaper is Windows-only".into())
}

#[cfg(not(windows))]
fn apply_lock_screen(_file: &Path) -> Result<(), String> {
    Err("setting the lock screen is Windows-only".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_formats_are_used_in_place() {
        for name in ["a.jpg", "b.JPEG", "c.png", "d.BMP"] {
            let src = Path::new("C:/pics").join(name);
            assert_eq!(plan(&src, Path::new("C:/data"), "wallpaper"), Source::Direct(src));
        }
    }

    #[test]
    fn other_formats_are_converted_to_one_fixed_png() {
        for name in ["a.webp", "b.heic", "c.psd", "noext"] {
            let plan = plan(Path::new("C:/pics").join(name).as_path(), Path::new("C:/data"), "lockscreen");
            assert_eq!(plan, Source::Convert(Path::new("C:/data").join("lockscreen.png")));
        }
    }

    #[test]
    fn a_converted_copy_is_written_to_disk() {
        let dir = std::env::temp_dir().join(format!("vislo-wall-{}", std::process::id()));
        let src = dir.join("in.tga");
        std::fs::create_dir_all(&dir).unwrap();
        image::RgbaImage::from_pixel(4, 3, image::Rgba([10, 20, 30, 255])).save(&src).unwrap();
        let out = prepare(&src, &dir, "wallpaper").unwrap();
        assert_eq!(out, dir.join("wallpaper.png"));
        assert_eq!(image::open(&out).unwrap().to_rgb8().dimensions(), (4, 3));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
