use serde::Serialize;
use std::path::{Path, PathBuf};

/// Formats WebView2 can decode natively; anything else would open as a broken image.
const EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "gif", "webp", "bmp", "avif", "ico",
];

#[derive(Debug, Serialize, PartialEq)]
pub struct Album {
    pub dir: String,
    pub files: Vec<String>,
    pub index: usize,
}

pub fn is_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
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
    let file = dunce::canonicalize(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    if !file.is_file() || !is_image(&file) {
        return Err(format!("not a supported image: {}", file.display()));
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
        dir: dir.to_string_lossy().into_owned(),
        files: files.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("openpic-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn is_image_ignores_case_and_rejects_others() {
        assert!(is_image(Path::new("a/B.JPG")));
        assert!(is_image(Path::new("x.webp")));
        assert!(!is_image(Path::new("notes.txt")));
        assert!(!is_image(Path::new("noext")));
    }

    #[test]
    fn path_from_args_skips_exe_and_flags() {
        let args = ["openpic.exe", "--flag", r"C:\pics\a.jpg"].map(String::from);
        assert_eq!(path_from_args(args), Some(r"C:\pics\a.jpg".to_string()));
        assert_eq!(path_from_args(["openpic.exe".to_string()]), None);
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
