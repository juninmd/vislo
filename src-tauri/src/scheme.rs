//! `vislo://` protocol: images the webview cannot (or should not) decode itself.
//!   /t?p=<path>&s=<px>        cached thumbnail (JPEG)
//!   /v?p=<path>               EXIF-embedded preview, upright (JPEG), 404 when absent
//!   /f?p=<path>&w=<px>&h=<px> sharp (Lanczos3) downscale to fit a box
//!   /c?p=<path>               full-resolution conversion of TIFF/PSD/RAW (JPEG, or PNG with alpha)
//!   /o?p=<path>               the file itself, for formats the webview decodes (read off the UI thread)

use crate::{decode, paths::Allowed, pool::Pool, resize, thumbs};
use fast_image_resize::FilterType;
use image::{DynamicImage, ImageFormat};
use percent_encoding::percent_decode_str;
use std::borrow::Cow;
use std::io::Cursor;
use std::path::PathBuf;
use tauri::http::{Response, StatusCode};

pub struct Services {
    pub allowed: Allowed,
    pub pool: Pool,
    pub cache_dir: PathBuf,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    Thumb { path: String, size: u32 },
    Preview { path: String },
    Fit { path: String, w: u32, h: u32 },
    Convert { path: String },
    Original { path: String },
}

fn query(q: &str, key: &str) -> Option<String> {
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| {
            percent_decode_str(v)
                .decode_utf8()
                .ok()
                .map(Cow::into_owned)
        })?
    })
}

fn number(q: &str, key: &str, lo: u32, hi: u32) -> Option<u32> {
    query(q, key)?.parse::<u32>().ok().map(|n| n.clamp(lo, hi))
}

pub fn parse(path: &str, q: &str) -> Option<Route> {
    let p = query(q, "p")?;
    match path {
        "/t" => Some(Route::Thumb {
            path: p,
            size: number(q, "s", thumbs::MIN_SIZE, thumbs::MAX_SIZE).unwrap_or(256),
        }),
        "/v" => Some(Route::Preview { path: p }),
        "/f" => Some(Route::Fit {
            path: p,
            w: number(q, "w", 1, 8192)?,
            h: number(q, "h", 1, 8192)?,
        }),
        "/c" => Some(Route::Convert { path: p }),
        "/o" => Some(Route::Original { path: p }),
        _ => None,
    }
}

fn reply(status: StatusCode, mime: &str, body: Vec<u8>) -> Response<Cow<'static, [u8]>> {
    Response::builder()
        .status(status)
        .header("Content-Type", mime)
        .header("Cache-Control", "private, max-age=300")
        .header("Access-Control-Allow-Origin", "*")
        .body(Cow::Owned(body))
        .expect("static response parts are valid")
}

fn error(status: StatusCode, message: &str) -> Response<Cow<'static, [u8]>> {
    reply(
        status,
        "text/plain; charset=utf-8",
        message.as_bytes().to_vec(),
    )
}

fn mime_of(path: &std::path::Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg" | "jpe" | "jfif" | "jif" | "jfi" | "pjpeg" | "pjp") => "image/jpeg",
        Some("png" | "apng") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp" | "dib") => "image/bmp",
        Some("avif" | "avifs") => "image/avif",
        Some("ico" | "cur") => "image/x-icon",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// The file as it is on disk. Read on a pool thread: a read on the UI thread would stall IPC
/// (window sizing, album listing) while a big photo loads.
fn original(file: &std::path::Path) -> Response<Cow<'static, [u8]>> {
    match std::fs::read(file) {
        Ok(bytes) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", mime_of(file))
            .header("Cache-Control", "private, max-age=5")
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Owned(bytes))
            .expect("static response parts are valid"),
        Err(e) => error(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

fn png(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(Vec::new());
    img.write_to(&mut out, ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

pub fn serve(svc: &Services, route: Route) -> Response<Cow<'static, [u8]>> {
    let raw = match &route {
        Route::Thumb { path, .. }
        | Route::Preview { path }
        | Route::Fit { path, .. }
        | Route::Convert { path }
        | Route::Original { path } => path,
    };
    let file = match svc.allowed.resolve(raw) {
        Ok(f) => f,
        Err(m) => return error(StatusCode::FORBIDDEN, &m),
    };
    let jpeg = |r: Result<Vec<u8>, String>| match r {
        Ok(b) => reply(StatusCode::OK, "image/jpeg", b),
        Err(m) => error(StatusCode::UNPROCESSABLE_ENTITY, &m),
    };
    match route {
        Route::Original { .. } => original(&file),
        Route::Thumb { size, .. } => jpeg(thumbs::thumbnail(&svc.cache_dir, &file, size)),
        Route::Preview { .. } => match decode::embedded_preview(&file) {
            Some(b) => reply(StatusCode::OK, "image/jpeg", b),
            None => error(StatusCode::NOT_FOUND, "no embedded preview"),
        },
        Route::Fit { w, h, .. } => jpeg(
            decode::load_fit(&file, w, h)
                .and_then(|img| resize::fit_owned(img, w, h, FilterType::Lanczos3))
                .and_then(|img| resize::to_jpeg(&img, 90)),
        ),
        Route::Convert { .. } => match decode::load(&file) {
            Ok(img) if img.color().has_alpha() => match png(&img) {
                Ok(b) => reply(StatusCode::OK, "image/png", b),
                Err(m) => error(StatusCode::UNPROCESSABLE_ENTITY, &m),
            },
            Ok(img) => jpeg(resize::to_jpeg(&img, 92)),
            Err(m) => error(StatusCode::UNPROCESSABLE_ENTITY, &m),
        },
    }
}

pub fn bad_request() -> Response<Cow<'static, [u8]>> {
    error(StatusCode::BAD_REQUEST, "unknown vislo route")
}

pub fn failed() -> Response<Cow<'static, [u8]>> {
    error(StatusCode::INTERNAL_SERVER_ERROR, "image processing failed")
}

pub fn shed() -> Response<Cow<'static, [u8]>> {
    error(StatusCode::SERVICE_UNAVAILABLE, "request superseded")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_routes_and_decodes_percent_escapes() {
        let q = "p=C%3A%5Cpics%5Ca%20b.jpg&s=300";
        assert_eq!(
            parse("/t", q),
            Some(Route::Thumb {
                path: r"C:\pics\a b.jpg".into(),
                size: 300
            })
        );
        assert_eq!(
            parse("/v", "p=x.jpg"),
            Some(Route::Preview {
                path: "x.jpg".into()
            })
        );
        assert_eq!(
            parse("/c", "p=x.tif"),
            Some(Route::Convert {
                path: "x.tif".into()
            })
        );
    }

    #[test]
    fn clamps_sizes_and_requires_fit_dimensions() {
        assert_eq!(
            parse("/t", "p=a&s=1"),
            Some(Route::Thumb {
                path: "a".into(),
                size: thumbs::MIN_SIZE
            })
        );
        assert_eq!(
            parse("/t", "p=a"),
            Some(Route::Thumb {
                path: "a".into(),
                size: 256
            })
        );
        assert_eq!(
            parse("/f", "p=a&w=99999&h=0"),
            Some(Route::Fit {
                path: "a".into(),
                w: 8192,
                h: 1
            })
        );
        assert_eq!(parse("/f", "p=a&w=10"), None);
    }

    #[test]
    fn original_route_serves_the_file_bytes_with_its_mime_type() {
        let dir = std::env::temp_dir().join(format!("vislo-orig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.JPG"), b"jpegbytes").unwrap();
        let svc = Services {
            allowed: Allowed::default(),
            pool: Pool::new(1, 4),
            cache_dir: dir.clone(),
        };
        svc.allowed.allow_dir(&dunce::canonicalize(&dir).unwrap());
        let path = dir.join("a.JPG").to_string_lossy().into_owned();
        let slashed = path.replace('\\', "/");
        assert_eq!(
            parse("/o", &format!("p={slashed}")),
            Some(Route::Original { path: slashed })
        );
        let r = serve(&svc, Route::Original { path });
        assert_eq!(
            (r.status(), r.headers()["Content-Type"].to_str().unwrap()),
            (StatusCode::OK, "image/jpeg")
        );
        assert_eq!(r.body().as_ref(), b"jpegbytes");
        assert_eq!(
            serve(
                &svc,
                Route::Original {
                    path: "Z:/nope/x.png".into()
                }
            )
            .status(),
            StatusCode::FORBIDDEN
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_unknown_routes_and_missing_paths() {
        assert_eq!(parse("/x", "p=a"), None);
        assert_eq!(parse("/t", "s=100"), None);
    }

    #[test]
    fn unopened_folders_are_forbidden() {
        let svc = Services {
            allowed: Allowed::default(),
            pool: Pool::new(1, 4),
            cache_dir: std::env::temp_dir(),
        };
        let r = serve(
            &svc,
            Route::Convert {
                path: "Z:\\nowhere\\x.tif".into(),
            },
        );
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }
}
