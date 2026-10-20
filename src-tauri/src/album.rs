use serde::Serialize;
use std::path::{Path, PathBuf};

/// Formats WebView2 can decode natively; served straight from disk through the asset protocol.
/// Keep in sync with `src/formats.ts` (checked by `native_list_matches_frontend`).
pub const NATIVE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "bmp", "avif", "ico",
];

/// Formats decoded in Rust and served as JPEG/PNG through the `vislo` protocol.
/// RAW files show their embedded full-size JPEG preview, not a developed image.
pub const CONVERTED_EXTENSIONS: &[&str] = &["tif", "tiff", "tga", "psd", "dng", "cr2", "nef", "arw", "orf", "rw2"];

pub fn extensions() -> impl Iterator<Item = &'static str> {
    NATIVE_EXTENSIONS.iter().chain(CONVERTED_EXTENSIONS).copied()
}

fn has_ext(path: &Path, list: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| list.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

pub fn is_native(path: &Path) -> bool {
    has_ext(path, NATIVE_EXTENSIONS)
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Album {
    pub dir: String,
    pub files: Vec<String>,
    pub index: usize,
    /// Upright pixel size of the opened file when cheap to know (sizes the window up front).
    pub size: Option<(u32, u32)>,
}

pub fn is_image(path: &Path) -> bool {
    is_native(path) || has_ext(path, CONVERTED_EXTENSIONS)
}

/// File name only: error text must not leak full paths into the UI.
pub fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// First non-flag argument after the executable name (what Explorer passes on "Open with").
pub fn path_from_args(args: impl IntoIterator<Item = String>) -> Option<String> {
    args.into_iter().skip(1).find(|a| !a.starts_with('-'))
}

fn file_name(p: &Path) -> &str {
    p.file_name().and_then(|n| n.to_str()).unwrap_or_default()
}

/// Lists the images beside `path` in Explorer-like natural order and locates `path` in it.
pub fn open(path: &Path) -> Result<Album, String> {
    let file = dunce::canonicalize(path)
        .map_err(|_| format!("\"{}\" could not be found", display_name(path)))?;
    if !file.is_file() || !is_image(&file) {
        return Err(format!("\"{}\" is not a supported image file", display_name(&file)));
    }
    let dir = file.parent().ok_or("file has no parent directory")?.to_path_buf();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("cannot list {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_image(p))
        .collect();
    files.sort_by(|a, b| natord::compare_ignore_case(file_name(a), file_name(b)));
    let index = files
        .iter()
        .position(|p| p == &file)
        .ok_or("opened file missing from its directory listing")?;
    Ok(Album {
        size: crate::decode::dimensions(&file),
        dir: dir.to_string_lossy().into_owned(),
        files: files.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn native_list_matches_frontend() {
        let ts = include_str!("../../src/formats.ts");
        let line = ts.lines().find(|l| l.contains("NATIVE_EXTENSIONS = [")).expect("list in formats.ts");
        let listed: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
        assert_eq!(listed, NATIVE_EXTENSIONS);
    }

    #[test]
    fn friendly_errors_name_the_file_but_never_the_folder() {
        let d = temp_dir("friendly");
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        let e = open(&d.join("a.txt")).unwrap_err();
        assert_eq!(e, "\"a.txt\" is not a supported image file");
        let e = open(&d.join("gone.jpg")).unwrap_err();
        assert_eq!(e, "\"gone.jpg\" could not be found");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn is_image_ignores_case_and_rejects_others() {
        assert!(is_image(Path::new("a/B.JPG")));
        assert!(is_image(Path::new("x.webp")));
        assert!(is_image(Path::new("x.CR2")) && !is_native(Path::new("x.CR2")));
        assert!(!is_image(Path::new("notes.txt")));
        assert!(!is_image(Path::new("noext")));
    }

    #[test]
    fn path_from_args_skips_exe_and_flags() {
        let args = ["vislo.exe", "--flag", r"C:\pics\a.jpg"].map(String::from);
        assert_eq!(path_from_args(args), Some(r"C:\pics\a.jpg".to_string()));
        assert_eq!(path_from_args(["vislo.exe".to_string()]), None);
    }

    #[test]
    fn open_sorts_naturally_and_finds_index() {
        let d = temp_dir("sort");
        for n in ["img10.jpg", "img2.jpg", "IMG1.png", "note.txt"] {
            std::fs::write(d.join(n), b"x").unwrap();
        }
        std::fs::create_dir(d.join("sub.jpg")).unwrap(); // a directory named like an image
        let album = open(&d.join("img2.jpg")).unwrap();
        let names: Vec<_> = album.files.iter().map(|f| file_name(Path::new(f)).to_string()).collect();
        assert_eq!(names, ["IMG1.png", "img2.jpg", "img10.jpg"]);
        assert_eq!(album.index, 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn open_rejects_non_images_and_missing_files() {
        let d = temp_dir("reject");
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        assert!(open(&d.join("a.txt")).is_err());
        assert!(open(&d.join("missing.jpg")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }
}
