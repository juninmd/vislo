//! `.svgz`: gzip-compressed SVG, inflated under a caller-chosen ceiling.

use flate2::read::GzDecoder;
use std::io::Read;
use std::path::Path;

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
