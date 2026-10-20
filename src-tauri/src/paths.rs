//! Trust boundary for paths that arrive from the webview.
//!
//! The frontend is our own code, but every path it sends is still treated as input: it must
//! resolve to a supported image inside a folder the user actually opened.

use crate::album;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
pub struct Allowed(Mutex<HashSet<PathBuf>>);

impl Allowed {
    pub fn allow_dir(&self, dir: &Path) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(dir.to_path_buf());
    }

    /// Canonicalises `raw` and requires a supported image whose folder was opened earlier.
    pub fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        let path = Path::new(raw);
        let file = dunce::canonicalize(path)
            .map_err(|_| format!("\"{}\" could not be found", album::display_name(path)))?;
        if !file.is_file() || !album::is_image(&file) {
            return Err(format!("\"{}\" is not a supported image file", album::display_name(&file)));
        }
        let parent = file.parent().ok_or("file has no parent directory")?;
        let allowed = self.0.lock().unwrap_or_else(|e| e.into_inner()).contains(parent);
        if allowed {
            Ok(file)
        } else {
            Err("that folder was not opened in Vislo".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-paths-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn resolves_images_only_inside_opened_folders() {
        let (open, other) = (temp("open"), temp("other"));
        std::fs::write(open.join("a.jpg"), b"x").unwrap();
        std::fs::write(other.join("b.jpg"), b"x").unwrap();
        let allowed = Allowed::default();
        allowed.allow_dir(&dunce::canonicalize(&open).unwrap());
        assert!(allowed.resolve(open.join("a.jpg").to_str().unwrap()).is_ok());
        assert!(allowed.resolve(other.join("b.jpg").to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(open);
        let _ = std::fs::remove_dir_all(other);
    }

    #[test]
    fn rejects_traversal_out_of_an_opened_folder() {
        let (open, other) = (temp("trav-open"), temp("trav-other"));
        std::fs::write(other.join("secret.png"), b"x").unwrap();
        let allowed = Allowed::default();
        allowed.allow_dir(&dunce::canonicalize(&open).unwrap());
        let sneaky = open.join("..").join(other.file_name().unwrap()).join("secret.png");
        assert!(allowed.resolve(sneaky.to_str().unwrap()).is_err());
        let _ = std::fs::remove_dir_all(open);
        let _ = std::fs::remove_dir_all(other);
    }

    #[test]
    fn rejects_non_images_even_in_an_opened_folder() {
        let open = temp("nonimg");
        std::fs::write(open.join("notes.txt"), b"x").unwrap();
        let allowed = Allowed::default();
        allowed.allow_dir(&dunce::canonicalize(&open).unwrap());
        let err = allowed.resolve(open.join("notes.txt").to_str().unwrap()).unwrap_err();
        assert!(err.contains("not a supported image"), "{err}");
        assert!(!err.contains(open.to_str().unwrap()), "error must not leak the full path");
        let _ = std::fs::remove_dir_all(open);
    }
}
