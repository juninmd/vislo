use serde::Serialize;
use std::path::{Component, Path, PathBuf};

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

/// Makes a command-line path absolute as Windows would for the process that typed it (`cwd` is
/// that process's directory). Pure: the disk is not read, so `..` is folded lexically.
pub fn absolutize(arg: &str, cwd: &Path) -> PathBuf {
    if arg.is_empty() {
        return PathBuf::new();
    }
    let path = Path::new(arg);
    let joined = match path.components().next() {
        // "C:name" is relative to the cwd only on the cwd's drive; another drive's cwd is unknown
        // here, so it means that drive's root.
        Some(Component::Prefix(drive)) if !path.has_root() => {
            let rest = Path::new(&arg[drive.as_os_str().len()..]);
            if cwd_drive(cwd).is_some_and(|d| d.eq_ignore_ascii_case(drive.as_os_str())) {
                cwd.join(rest)
            } else {
                PathBuf::from(format!(r"{}\", drive.as_os_str().to_string_lossy())).join(rest)
            }
        }
        // "\name" has no drive of its own: it takes the cwd's.
        Some(Component::RootDir) => cwd_drive_path(cwd).join(arg),
        _ => cwd.join(path),
    };
    normalize(&joined)
}

/// Resolves `arg` for another process to open: the canonical path when the file exists (the
/// form the album uses), else the joined name, so the error still names the file.
pub fn resolve_arg(arg: &str, cwd: &Path) -> String {
    let abs = absolutize(arg, cwd);
    dunce::canonicalize(&abs)
        .unwrap_or(abs)
        .to_string_lossy()
        .into_owned()
}

fn cwd_drive(cwd: &Path) -> Option<&std::ffi::OsStr> {
    match cwd.components().next() {
        Some(Component::Prefix(p)) => Some(p.as_os_str()),
        _ => None,
    }
}

fn cwd_drive_path(cwd: &Path) -> PathBuf {
    cwd_drive(cwd).map(PathBuf::from).unwrap_or_default()
}

/// Folds `.` and `..` without touching the disk; `..` never climbs above a root or drive.
fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => match out.components().next_back() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(".."),
            },
            other => out.push(other),
        }
    }
    out
}

fn file_name(p: &Path) -> &str {
    p.file_name().and_then(|n| n.to_str()).unwrap_or_default()
}

/// Lists the images beside `path` in Explorer-like natural order and locates `path` in it.
pub fn open(path: &Path) -> Result<Album, String> {
    let file = dunce::canonicalize(path)
        .map_err(|_| format!("\"{}\" could not be found", display_name(path)))?;
    if !file.is_file() || !is_image(&file) {
        return Err(format!(
            "\"{}\" is not a supported image file",
            display_name(&file)
        ));
    }
    let dir = file
        .parent()
        .ok_or("file has no parent directory")?
        .to_path_buf();
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
    fn asset_protocol_stays_disabled() {
        // The webview reads images only through `vislo://`, which checks each path; the Tauri
        // asset protocol serves whole folders unchecked, so turning it back on must be deliberate.
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let security = &conf["app"]["security"];
        assert_ne!(security["assetProtocol"]["enable"].as_bool(), Some(true));
        let csp = security["csp"].as_str().expect("csp string");
        assert!(!csp.contains("asset:") && !csp.contains("asset.localhost"), "{csp}");
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

    #[test]
    fn open_rejects_non_images_and_missing_files() {
        let d = temp_dir("reject");
        std::fs::write(d.join("a.txt"), b"x").unwrap();
        assert!(open(&d.join("a.txt")).is_err());
        assert!(open(&d.join("missing.jpg")).is_err());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(windows)]
    #[test]
    fn absolutize_resolves_relative_arguments_against_the_callers_cwd() {
        let cwd = Path::new(r"C:\fotos");
        assert_eq!(absolutize("a.png", cwd), PathBuf::from(r"C:\fotos\a.png"));
        assert_eq!(absolutize(r".\a.png", cwd), PathBuf::from(r"C:\fotos\a.png"));
        assert_eq!(absolutize(r"..\x\a.png", cwd), PathBuf::from(r"C:\x\a.png"));
        assert_eq!(absolutize(r"..\..\..\a.png", cwd), PathBuf::from(r"C:\a.png"));
        assert_eq!(absolutize("my pic.png", cwd), PathBuf::from(r"C:\fotos\my pic.png"));
    }

    #[cfg(windows)]
    #[test]
    fn absolutize_leaves_absolute_and_unc_paths_alone() {
        let cwd = Path::new(r"C:\fotos");
        for p in [r"D:\pics\b.png", r"\\srv\share\a.jpg", r"\\?\C:\a.png"] {
            assert_eq!(absolutize(p, cwd), PathBuf::from(p));
        }
    }

    #[cfg(windows)]
    #[test]
    fn absolutize_handles_drive_and_root_relative_forms() {
        // "C:a.png" is relative to the cwd only on its drive; elsewhere it means that drive's root.
        let fotos = Path::new(r"C:\fotos");
        assert_eq!(absolutize("C:a.png", fotos), PathBuf::from(r"C:\fotos\a.png"));
        assert_eq!(absolutize("C:a.png", Path::new(r"D:\x")), PathBuf::from(r"C:\a.png"));
        // "\a.png" has no drive of its own: it takes the cwd's drive.
        assert_eq!(absolutize(r"\a.png", Path::new(r"D:\fotos")), PathBuf::from(r"D:\a.png"));
    }

    #[test]
    fn absolutize_keeps_empty_input_and_empty_cwd_as_given() {
        assert_eq!(absolutize("", Path::new(r"C:\fotos")), PathBuf::new());
        assert_eq!(absolutize("a.png", Path::new("")), PathBuf::from("a.png"));
    }

    #[test]
    fn resolve_arg_falls_back_to_the_absolute_name_when_missing() {
        let d = temp_dir("resolve");
        std::fs::write(d.join("a.png"), b"x").unwrap();
        let found = dunce::canonicalize(d.join("a.png")).unwrap();
        assert_eq!(resolve_arg("a.png", &d), found.to_string_lossy());
        // Missing files keep the full name so the error still says which file.
        assert_eq!(resolve_arg("gone.png", &d), d.join("gone.png").to_string_lossy());
        let _ = std::fs::remove_dir_all(&d);
    }
}
