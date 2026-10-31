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
    /// Folder modification stamp in ns, as text (JSON numbers cannot hold it exactly); `None` when
    /// the platform gives none, which makes every sync check relist in full.
    pub stamp: Option<String>,
    /// Upright pixel size of the opened file when cheap to know (sizes the window up front).
    pub size: Option<(u32, u32)>,
    /// One `mtime_ns-len` stamp per entry of `files`, so the page's URLs change when a file is rewritten.
    pub file_stamps: Vec<String>,
}

/// `mtime_ns-len`: changes whenever the file is rewritten, and lets the WebView cache key on it.
pub fn stamp(meta: &std::fs::Metadata) -> String {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos());
    format!("{mtime}-{}", meta.len())
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

/// Resolves `path` to the image file it names.
fn resolve(path: &Path) -> Result<PathBuf, OpenError> {
    let file = dunce::canonicalize(path).map_err(|e| missing_or_io(path, &e))?;
    if !file.is_file() || !is_image(&file) {
        return Err(OpenError {
            kind: OpenErrorKind::NotImage,
            message: format!("\"{}\" is not a supported image file", display_name(&file)),
        });
    }
    Ok(file)
}

/// Modification time of a folder. Adding, removing or renaming an entry moves it; overwriting a
/// file in place does not, which the re-list does not need either.
fn dir_stamp(dir: &Path) -> Option<String> {
    let modified = std::fs::metadata(dir).ok()?.modified().ok()?;
    let since_epoch = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(since_epoch.as_nanos().to_string())
}

/// Lists the images beside `path` in Explorer-like natural order and locates `path` in it.
pub fn open(path: &Path) -> Result<Album, OpenError> {
    build(&resolve(path)?, true)
}

/// Sync check for a folder already on screen: `None` while its stamp still equals `stamp`, otherwise
/// the fresh listing. A missing stamp, on either side, always relists.
pub fn list_changed(path: &Path, stamp: Option<&str>) -> Result<Option<Album>, OpenError> {
    let file = resolve(path)?;
    if stamp.is_some() && file.parent().and_then(dir_stamp).as_deref() == stamp {
        return Ok(None);
    }
    build(&file, false).map(Some)
}

fn build(file: &Path, with_size: bool) -> Result<Album, OpenError> {
    let dir = file
        .parent()
        .ok_or_else(|| OpenError {
            kind: OpenErrorKind::Io,
            message: "file has no parent directory".into(),
        })?
        .to_path_buf();
    // Read before listing: a change made during the listing then shows up as a stamp mismatch.
    let folder_stamp = dir_stamp(&dir);
    let mut files: Vec<(PathBuf, String)> = std::fs::read_dir(&dir)
        .map_err(|e| OpenError {
            kind: OpenErrorKind::Io,
            message: format!("cannot list {}: {e}", dir.display()),
        })?
        .filter_map(Result::ok)
        // Extension first: a folder full of non-images costs no file-type lookup at all.
        .filter(|e| is_image(Path::new(&e.file_name())) && is_regular_file(e))
        .map(|e| e.path())
        .filter_map(|p| {
            // follows symlinks, as `is_file` did before
            let meta = std::fs::metadata(&p).ok().filter(std::fs::Metadata::is_file)?;
            Some((p, stamp(&meta)))
        })
        .collect();
    files.sort_by(|a, b| natord::compare_ignore_case(file_name(&a.0), file_name(&b.0)));
    let index = files
        .iter()
        .position(|(p, _)| p.as_path() == file)
        .ok_or_else(|| OpenError {
            kind: OpenErrorKind::NotFound,
            message: "opened file missing from its directory listing".into(),
        })?;
    Ok(Album {
        size: with_size.then(|| crate::decode::dimensions(file)).flatten(),
        dir: dir.to_string_lossy().into_owned(),
        files: files
            .iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect(),
        index,
        stamp: folder_stamp,
        file_stamps: files.into_iter().map(|(_, s)| s).collect(),
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
        let stamp = open(&shown).unwrap().stamp;
        let mut best_sync = f64::MAX;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            assert!(list_changed(&shown, stamp.as_deref()).unwrap().is_none());
            best_sync = best_sync.min(t.elapsed().as_secs_f64() * 1e3);
        }
        eprintln!(
            "album_bench: {FILES} files, best of 5 list_changed() unchanged: {best_sync:.2} ms"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Folder times have coarse resolution on some systems; a pause keeps consecutive changes apart.
    fn tick() {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }

    #[test]
    fn list_changed_is_none_until_the_folder_changes() {
        let d = temp_dir("sync");
        std::fs::write(d.join("a.jpg"), b"x").unwrap();
        std::fs::write(d.join("b.jpg"), b"x").unwrap();
        let shown = d.join("a.jpg");
        let album = open(&shown).unwrap();
        let stamp = album.stamp.clone().expect("Windows reports folder times");
        assert_eq!(list_changed(&shown, Some(&stamp)).unwrap(), None);

        tick();
        std::fs::write(d.join("c.jpg"), b"x").unwrap();
        let added = list_changed(&shown, Some(&stamp)).unwrap().expect("added");
        assert_eq!(added.files.len(), 3);
        let stamp = added.stamp.expect("stamp");
        assert_eq!(list_changed(&shown, Some(&stamp)).unwrap(), None);

        tick();
        std::fs::remove_file(d.join("b.jpg")).unwrap();
        let removed = list_changed(&shown, Some(&stamp))
            .unwrap()
            .expect("removed");
        assert_eq!(removed.files.len(), 2);
        let stamp = removed.stamp.expect("stamp");

        tick();
        std::fs::rename(d.join("c.jpg"), d.join("d.jpg")).unwrap();
        let renamed = list_changed(&shown, Some(&stamp))
            .unwrap()
            .expect("renamed");
        assert!(renamed.files.iter().any(|f| f.ends_with("d.jpg")));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn list_changed_relists_when_no_stamp_is_known() {
        let d = temp_dir("nostamp");
        std::fs::write(d.join("a.jpg"), b"x").unwrap();
        let shown = d.join("a.jpg");
        assert!(list_changed(&shown, None).unwrap().is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn list_changed_reports_a_deleted_shown_file_as_not_found() {
        let d = temp_dir("sync-gone");
        std::fs::write(d.join("a.jpg"), b"x").unwrap();
        let shown = d.join("a.jpg");
        let stamp = open(&shown).unwrap().stamp;
        std::fs::remove_file(&shown).unwrap();
        let e = list_changed(&shown, stamp.as_deref()).unwrap_err();
        assert_eq!(e.kind, OpenErrorKind::NotFound);
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

    #[test]
    fn file_stamps_run_parallel_to_files_and_follow_rewrites() {
        let d = temp_dir("stamps");
        std::fs::write(d.join("a.jpg"), b"x").unwrap();
        std::fs::write(d.join("b.jpg"), b"yy").unwrap();
        let before = open(&d.join("b.jpg")).unwrap();
        assert_eq!(before.file_stamps.len(), before.files.len());
        std::fs::write(d.join("b.jpg"), b"yyy").unwrap();
        let after = open(&d.join("b.jpg")).unwrap();
        assert_ne!(before.file_stamps[1], after.file_stamps[1]);
        assert_eq!(before.file_stamps[0], after.file_stamps[0]);
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
