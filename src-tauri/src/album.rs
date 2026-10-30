use serde::Serialize;
use std::path::{Path, PathBuf};

/// Formats WebView2 can decode natively; served from disk by the `/o` route.
/// Keep in sync with `src/formats.ts` (checked by `native_list_matches_frontend`).
pub const NATIVE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "jif", "jfi", "pjpeg", "pjp", "png", "apng", "gif", "webp",
    "bmp", "dib", "avif", "avifs", "ico", "cur", "svg",
];

/// Formats decoded in Rust and served as JPEG/PNG through the `vislo` protocol.
pub const CONVERTED_EXTENSIONS: &[&str] = &[
    "tif", "tiff", "tga", "vda", "icb", "vst", "psd", "pbm", "pgm", "ppm", "pnm", "pam", "hdr",
    "exr", "dds", "qoi", "ff", "jxl", "jp2", "j2k", "j2c", "jpc",
];

/// Camera RAW files: Vislo shows the embedded full-size JPEG preview, never a developed image.
pub const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "orf", "rw2", "raf", "pef",
    "srw", "x3f", "3fr", "fff", "iiq", "mef", "mos", "mrw", "dcr", "kdc", "erf", "rwl", "raw",
    "arq", "gpr", "ori", "k25",
];

pub fn extensions() -> impl Iterator<Item = &'static str> {
    NATIVE_EXTENSIONS
        .iter()
        .chain(CONVERTED_EXTENSIONS)
        .chain(RAW_EXTENSIONS)
        .copied()
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
    is_native(path) || has_ext(path, CONVERTED_EXTENSIONS) || has_ext(path, RAW_EXTENSIONS)
}

/// File name only: error text must not leak full paths into the UI.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// First non-flag argument after the executable name (what Explorer passes on "Open with").
pub fn path_from_args(args: impl IntoIterator<Item = String>) -> Option<String> {
    args.into_iter().skip(1).find(|a| !a.starts_with('-'))
}

fn file_name(p: &Path) -> &str {
    p.file_name().and_then(|n| n.to_str()).unwrap_or_default()
}

/// `file_type` comes with the directory enumeration, so no stat per entry; symlinks still
/// resolve to their target, as `Path::is_file` did.
fn is_regular_file(e: &std::fs::DirEntry) -> bool {
    match e.file_type() {
        Ok(t) if t.is_symlink() => e.path().is_file(),
        Ok(t) => t.is_file(),
        Err(_) => e.path().is_file(),
    }
}

/// The frontend's contract: only `NotFound` means the picture was deleted, so only it may move the view.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenErrorKind {
    NotFound,
    NotImage,
    Io,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct OpenError {
    pub kind: OpenErrorKind,
    /// File name only: error text must not leak full paths into the UI.
    pub message: String,
}

/// A missing file means "deleted" only while its folder is reachable: a share that dropped out
/// reports the same not-found error, and must not move the view to a neighbour.
fn missing_or_io(path: &Path, e: &std::io::Error) -> OpenError {
    let folder_reachable = path
        .parent()
        .and_then(|d| std::fs::metadata(d).ok())
        .is_some_and(|m| m.is_dir());
    if e.kind() == std::io::ErrorKind::NotFound && folder_reachable {
        OpenError {
            kind: OpenErrorKind::NotFound,
            message: format!("\"{}\" could not be found", display_name(path)),
        }
    } else {
        OpenError {
            kind: OpenErrorKind::Io,
            message: format!("\"{}\" cannot be read: {e}", display_name(path)),
        }
    }
}

/// Lists the images beside `path` in Explorer-like natural order and locates `path` in it.
pub fn open(path: &Path) -> Result<Album, OpenError> {
    let file = dunce::canonicalize(path).map_err(|e| missing_or_io(path, &e))?;
    if !file.is_file() || !is_image(&file) {
        return Err(OpenError {
            kind: OpenErrorKind::NotImage,
            message: format!("\"{}\" is not a supported image file", display_name(&file)),
        });
    }
    let dir = file
        .parent()
        .ok_or_else(|| OpenError {
            kind: OpenErrorKind::Io,
            message: "file has no parent directory".into(),
        })?
        .to_path_buf();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| OpenError {
            kind: OpenErrorKind::Io,
            message: format!("cannot list {}: {e}", dir.display()),
        })?
        .filter_map(Result::ok)
        // Extension first: a folder full of non-images costs no file-type lookup at all.
        .filter(|e| is_image(Path::new(&e.file_name())) && is_regular_file(e))
        .map(|e| e.path())
        .collect();
    files.sort_by(|a, b| natord::compare_ignore_case(file_name(a), file_name(b)));
    let index = files
        .iter()
        .position(|p| p == &file)
        .ok_or_else(|| OpenError {
            kind: OpenErrorKind::NotFound,
            message: "opened file missing from its directory listing".into(),
        })?;
    Ok(Album {
        size: crate::decode::dimensions(&file),
        dir: dir.to_string_lossy().into_owned(),
        files: files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
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
        let line = ts
            .lines()
            .find(|l| l.contains("NATIVE_EXTENSIONS = ["))
            .expect("list in formats.ts");
        let listed: Vec<&str> = line.split('"').skip(1).step_by(2).collect();
        assert_eq!(listed, NATIVE_EXTENSIONS);
    }

    #[test]
    fn no_extension_is_listed_twice() {
        let mut all: Vec<_> = extensions().collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n);
    }

    #[test]
    fn bundle_file_associations_cover_every_supported_extension() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let mut declared: Vec<&str> = conf["bundle"]["fileAssociations"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|a| {
                a["ext"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|e| e.as_str().unwrap())
            })
            .collect();
        let mut want: Vec<_> = extensions().collect();
        declared.sort_unstable();
        want.sort_unstable();
        assert_eq!(declared, want);
    }

    #[test]
    fn friendly_errors_name_the_file_but_never_the_folder() {
        let d = temp_dir("friendly");
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        let e = open(&d.join("a.txt")).unwrap_err();
        assert_eq!(e.message, "\"a.txt\" is not a supported image file");
        let e = open(&d.join("gone.jpg")).unwrap_err();
        assert_eq!(e.message, "\"gone.jpg\" could not be found");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn errors_say_whether_the_picture_was_deleted() {
        let d = temp_dir("kinds");
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        assert_eq!(
            open(&d.join("a.txt")).unwrap_err().kind,
            OpenErrorKind::NotImage
        );
        assert_eq!(
            open(&d.join("gone.jpg")).unwrap_err().kind,
            OpenErrorKind::NotFound
        );
        // A folder that cannot be reached is a transient failure, not a deleted file.
        let e = open(&d.join("unplugged").join("gone.jpg")).unwrap_err();
        assert_eq!(e.kind, OpenErrorKind::Io);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn is_image_ignores_case_and_rejects_others() {
        assert!(is_image(Path::new("a/B.JPG")));
        assert!(is_image(Path::new("x.webp")));
        assert!(is_image(Path::new("x.CR2")) && !is_native(Path::new("x.CR2")));
        assert!(
            is_image(Path::new("x.RAF"))
                && is_image(Path::new("x.exr"))
                && is_image(Path::new("x.apng"))
        );
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
        let names: Vec<_> = album
            .files
            .iter()
            .map(|f| file_name(Path::new(f)).to_string())
            .collect();
        assert_eq!(names, ["IMG1.png", "img2.jpg", "img10.jpg"]);
        assert_eq!(album.index, 1);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Benchmark, not a gate: `cargo test --release album_bench -- --ignored --nocapture`.
    #[test]
    #[ignore = "benchmark: run with --release --ignored --nocapture"]
    fn album_bench() {
        const FILES: usize = 50_000;
        let d = temp_dir("bench");
        for i in 0..FILES {
            std::fs::write(d.join(format!("img{i:05}.jpg")), b"").unwrap();
        }
        let shown = d.join("img00000.jpg");
        let mut best = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            let album = open(&shown).unwrap();
            best = best.min(t.elapsed().as_secs_f64() * 1e3);
            assert_eq!(album.files.len(), FILES);
        }
        eprintln!("album_bench: {FILES} files, best of 5 open(): {best:.1} ms");
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
