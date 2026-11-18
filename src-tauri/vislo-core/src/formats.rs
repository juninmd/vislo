//! Which files are images: the extension lists Vislo knows, grouped by how each format is shown.

use std::path::Path;

/// Formats WebView2 can decode natively; served from disk by the `/o` route.
/// Keep in sync with the app's `src/formats.ts` (checked by `native_list_matches_frontend` there).
pub const NATIVE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "jif", "jfi", "pjpeg", "pjp", "png", "apng", "gif", "webp",
    "bmp", "dib", "avif", "avifs", "ico", "cur", "svg", "svgz",
];

/// Formats decoded in Rust and served as JPEG/PNG through the `vislo` protocol.
pub const CONVERTED_EXTENSIONS: &[&str] = &[
    "tif", "tiff", "tga", "vda", "icb", "vst", "psd", "pbm", "pgm", "ppm", "pnm", "pam", "hdr",
    "exr", "dds", "qoi", "ff", "jxl", "jp2", "j2k", "j2c", "jpc", "ora", "kra",
    "heic", "heif", "hif", "jxr", "wdp", "hdp", "wmp", "ras", "sun", "fits", "fit", "fts", "dpx", "mpo", "jps",
    "rgbe", "xyze", "jpf", "jpx", "jb2", "jbig2", "cals", "psb", "wmf", "emf", "mng", "jng", "pct", "pict", "iff", "lbm", "ilbm", "ani", "xwd", "dcm", "dicom", "xcf", "jls", "djvu", "djv", "blp", "dcx", "gbr", "fli", "flc", "ktx", "ktx2",
];

/// Legacy and niche bitmaps decoded by `image-extras` (registered through `codecs::register_extras`).
/// `.rgb`/`.rgba` stay out: the names are too generic to claim from other programs or to list as images.
pub const EXTRA_EXTENSIONS: &[&str] = &[
    "icns", "otb", "pcx", "sgi", "iris", "bw", "wbmp", "xbm", "bm", "xpm",
];

/// Formats Vislo opens but the installer does not register itself for: their extensions belong to specialised viewers
/// (DICOM) or to an editor (GIMP's XCF), so taking over the default would break users.
pub const NOT_REGISTERED: &[&str] = &["dcm", "dicom", "xcf", "ani", "djvu", "djv", "gbr", "fli", "flc", "ktx", "ktx2"];

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
        .chain(EXTRA_EXTENSIONS)
        .chain(RAW_EXTENSIONS)
        .copied()
}

pub fn has_ext(path: &Path, list: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| list.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

pub fn is_native(path: &Path) -> bool {
    has_ext(path, NATIVE_EXTENSIONS)
}

pub fn is_image(path: &Path) -> bool {
    is_native(path) || has_ext(path, CONVERTED_EXTENSIONS) || has_ext(path, EXTRA_EXTENSIONS) || has_ext(path, RAW_EXTENSIONS)
}

/// File name only: error text must not leak full paths into the UI.
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
