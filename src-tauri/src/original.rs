//! The `/o` route: the file itself, read whole only up to a ceiling, or just the bytes a `Range` asks for.

use crate::scheme::{error, mime_of};
use std::borrow::Cow;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tauri::http::{Response, StatusCode};

/// Largest original `/o` will read into memory; bigger files are refused unread.
pub const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// What a `Range` header asks for, against a file of a known length.
#[derive(Debug, PartialEq, Eq)]
enum Span {
    Whole,
    Part { start: u64, end: u64 },
    Unsatisfiable,
}

/// One `bytes=` range. Multi-range and malformed headers are ignored (the whole file is sent), as RFC 9110 allows.
fn span(header: Option<&str>, len: u64) -> Span {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else {
        return Span::Whole;
    };
    let Some((first, last)) = spec.split_once('-') else {
        return Span::Whole;
    };
    if spec.contains(',') {
        return Span::Whole;
    }
    let (first, last) = (first.trim(), last.trim());
    if first.is_empty() {
        // suffix form `bytes=-N`: the last N bytes
        let Ok(n) = last.parse::<u64>() else {
            return Span::Whole;
        };
        if len == 0 || n == 0 {
            return Span::Unsatisfiable;
        }
        return Span::Part {
            start: len.saturating_sub(n),
            end: len - 1,
        };
    }
    let Ok(start) = first.parse::<u64>() else {
        return Span::Whole;
    };
    let end = if last.is_empty() {
        len.saturating_sub(1)
    } else {
        match last.parse::<u64>() {
            Ok(end) if end >= start => end.min(len.saturating_sub(1)),
            _ => return Span::Whole,
        }
    };
    if start >= len {
        return Span::Unsatisfiable;
    }
    Span::Part { start, end }
}

fn read_span(file: &Path, start: u64, count: u64) -> std::io::Result<Vec<u8>> {
    let mut f = std::fs::File::open(file)?;
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(count as usize);
    (&mut f).take(count).read_to_end(&mut buf)?;
    Ok(buf)
}

/// The file as it is on disk, or the bytes a `Range` asks for. Read on a pool thread: the built-in
/// asset protocol reads on the UI thread, which stalls IPC (window sizing, album listing) while a
/// big photo loads. The size check comes first so an oversized file is never read at all.
pub fn respond(file: &Path, range: Option<&str>, max: u64) -> Response<Cow<'static, [u8]>> {
    let len = match std::fs::metadata(file) {
        Ok(m) => m.len(),
        Err(e) => return error(StatusCode::NOT_FOUND, &e.to_string()),
    };
    if len > max {
        return error(StatusCode::PAYLOAD_TOO_LARGE, "Image file is too large to open");
    }
    let (status, start, count, content_range) = match span(range, len) {
        Span::Whole => (StatusCode::OK, 0, len, None),
        Span::Part { start, end } => (
            StatusCode::PARTIAL_CONTENT,
            start,
            end - start + 1,
            Some(format!("bytes {start}-{end}/{len}")),
        ),
        Span::Unsatisfiable => {
            return Response::builder()
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header("Content-Range", format!("bytes */{len}"))
                .body(Cow::Owned(Vec::new()))
                .expect("static response parts are valid")
        }
    };
    let body = match read_span(file, start, count) {
        Ok(bytes) => bytes,
        Err(e) => return error(StatusCode::NOT_FOUND, &e.to_string()),
    };
    let mut res = Response::builder()
        .status(status)
        .header("Content-Type", mime_of(file))
        .header("Cache-Control", "private, max-age=5")
        .header("Accept-Ranges", "bytes");
    if let Some(cr) = content_range {
        res = res.header("Content-Range", cr);
    }
    res.body(Cow::Owned(body))
        .expect("static response parts are valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{paths::Allowed, pool::Pool, scheme::{serve, Route, Services}};

    fn sample(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vislo-range-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.png");
        std::fs::write(&file, bytes).unwrap();
        file
    }

    #[test]
    fn parses_single_byte_ranges_only() {
        assert_eq!(span(None, 10), Span::Whole);
        assert_eq!(span(Some("bytes=2-4"), 10), Span::Part { start: 2, end: 4 });
        assert_eq!(span(Some("bytes=-3"), 10), Span::Part { start: 7, end: 9 });
        assert_eq!(span(Some("bytes=8-99"), 10), Span::Part { start: 8, end: 9 });
        assert_eq!(span(Some("bytes=10-"), 10), Span::Unsatisfiable);
        assert_eq!(span(Some("bytes=0-0"), 0), Span::Unsatisfiable);
        assert_eq!(span(Some("bytes=0-1,5-6"), 10), Span::Whole);
        assert_eq!(span(Some("bytes=9-3"), 10), Span::Whole);
    }

    #[test]
    fn refuses_files_over_the_ceiling_before_reading_them() {
        let file = sample("cap", &[7u8; 1024]);
        // The ceiling is a parameter so the test stays a few bytes, not a few GiB.
        assert_eq!(respond(&file, None, 1023).status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(respond(&file, None, 1024).status(), StatusCode::OK);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn range_returns_only_the_requested_bytes() {
        let content: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let file = sample("span", &content);
        let r = respond(&file, Some("bytes=0-99"), MAX_BYTES);
        assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(r.headers()["Content-Range"], "bytes 0-99/1000");
        assert_eq!(r.headers()["Accept-Ranges"], "bytes");
        assert_eq!(r.body().as_ref(), &content[..100]);
        let tail = respond(&file, Some("bytes=-10"), MAX_BYTES);
        assert_eq!(tail.headers()["Content-Range"], "bytes 990-999/1000");
        assert_eq!(tail.body().as_ref(), &content[990..]);
        let open = respond(&file, Some("bytes=995-"), MAX_BYTES);
        assert_eq!(open.body().as_ref(), &content[995..]);
        let clipped = respond(&file, Some("bytes=900-5000"), MAX_BYTES);
        assert_eq!(clipped.headers()["Content-Range"], "bytes 900-999/1000");
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn unsatisfiable_ranges_get_416_and_unsupported_ones_get_the_whole_file() {
        let file = sample("unsat", &[1u8; 1000]);
        let r = respond(&file, Some("bytes=1000-1100"), MAX_BYTES);
        assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(r.headers()["Content-Range"], "bytes */1000");
        for ignored in ["bytes=0-1,5-6", "items=0-9", "bytes=9-3", "bytes=-"] {
            let r = respond(&file, Some(ignored), MAX_BYTES);
            assert_eq!((ignored, r.status(), r.body().len()), (ignored, StatusCode::OK, 1000));
        }
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn missing_original_is_not_found() {
        let file = sample("gone", b"x");
        let gone = file.with_file_name("gone.png");
        assert_eq!(respond(&gone, None, MAX_BYTES).status(), StatusCode::NOT_FOUND);
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }

    #[test]
    fn range_header_reaches_the_route() {
        let file = sample("route", &[9u8; 500]);
        let svc = Services {
            allowed: Allowed::default(),
            pool: Pool::new(1, 4),
            cache_dir: std::env::temp_dir(),
        };
        svc.allowed.allow_dir(&dunce::canonicalize(file.parent().unwrap()).unwrap());
        let path = file.to_string_lossy().into_owned();
        let r = serve(&svc, Route::Original { path }, Some("bytes=0-99"));
        assert_eq!((r.status(), r.body().len()), (StatusCode::PARTIAL_CONTENT, 100));
        let _ = std::fs::remove_dir_all(file.parent().unwrap());
    }
}
