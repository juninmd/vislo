//! `.svgz`: gzip-compressed SVG. It is inflated here, under a ceiling, and served as plain SVG, so the webview never
//! has to honour a `Content-Encoding` on a custom scheme.

use crate::scheme::error;
use flate2::read::GzDecoder;
use std::borrow::Cow;
use std::io::Read;
use std::path::Path;
use tauri::http::{Response, StatusCode};

/// Largest SVG text accepted after inflating; a gzip bomb stops here instead of filling memory.
const MAX_INFLATED: u64 = 32 * 1024 * 1024;

pub fn is_svgz(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("svgz"))
}

/// The first `limit` bytes of the inflated file (all of it when shorter).
pub fn head(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    GzDecoder::new(file).take(limit).read_to_end(&mut out).map_err(|e| format!("not a valid gzip-compressed SVG: {e}"))?;
    Ok(out)
}

pub fn respond(file: &Path) -> Response<Cow<'static, [u8]>> {
    match head(file, MAX_INFLATED + 1) {
        Ok(b) if b.len() as u64 > MAX_INFLATED => error(StatusCode::PAYLOAD_TOO_LARGE, "Image file is too large to open"),
        Ok(b) => Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "image/svg+xml")
            .header("Cache-Control", "private, max-age=5")
            .body(Cow::Owned(b))
            .expect("static response parts are valid"),
        Err(m) => error(StatusCode::UNPROCESSABLE_ENTITY, &m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;

    fn gz(data: &[u8]) -> Vec<u8> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn file(tag: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("vislo-svgz-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("a.svgz");
        std::fs::write(&f, bytes).unwrap();
        f
    }

    #[test]
    fn a_compressed_svg_is_served_as_plain_svg() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 30 20"><rect width="30" height="20"/></svg>"#;
        let res = respond(&file("ok", &gz(svg)));
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["Content-Type"], "image/svg+xml");
        assert_eq!(res.body().as_ref(), svg);
        assert_eq!(crate::codecs::svg_dimensions(&file("dim", &gz(svg))), Some((30, 20)));
    }

    #[test]
    fn a_gzip_bomb_is_refused_without_filling_memory() {
        let bomb = gz(&vec![b' '; (MAX_INFLATED + 10) as usize]);
        assert!(bomb.len() < 100_000, "the bomb itself is tiny");
        assert_eq!(respond(&file("bomb", &bomb)).status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn plain_or_corrupt_data_is_an_error_not_a_panic() {
        assert_eq!(respond(&file("plain", b"<svg></svg>")).status(), StatusCode::UNPROCESSABLE_ENTITY);
        let mut cut = gz(b"<svg viewBox=\"0 0 5 5\"></svg>");
        cut.truncate(cut.len() - 6);
        assert_eq!(respond(&file("cut", &cut)).status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(is_svgz(Path::new("x/Logo.SVGZ")) && !is_svgz(Path::new("x/logo.svg")));
    }
}
