//! Data the page needs on its very first frame, injected before any script runs so start-up
//! does not wait for IPC round trips.

use crate::{album, settings::Settings};
use serde_json::json;
use std::path::{Path, PathBuf};

/// The initialization script plus the folder of the launch file (to be authorised up front).
pub fn script(
    settings: &Settings,
    arg: Option<&str>,
    background: bool,
) -> (String, Option<PathBuf>) {
    let canon = arg
        .and_then(|a| dunce::canonicalize(a).ok())
        .filter(|p| p.is_file() && album::is_image(p));
    let value = json!({
        "settings": settings,
        "path": arg,
        // canonical form: identical to the album's own file names, so the page can reuse its prefetch
        "file": canon.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "native": canon.as_deref().is_some_and(album::is_native),
        "background": background,
    });
    (
        format!("window.__VISLO__={value};"),
        canon.and_then(|p| p.parent().map(Path::to_path_buf)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_a_valid_launch_file() {
        let dir = std::env::temp_dir().join(format!("vislo-boot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.png");
        std::fs::write(&file, b"x").unwrap();
        let (js, folder) = script(&Settings::default(), file.to_str(), false);
        assert!(js.starts_with("window.__VISLO__={") && js.ends_with(";"));
        assert!(js.contains("\"native\":true") && js.contains("\"background\":false"));
        assert_eq!(folder, Some(dunce::canonicalize(&dir).unwrap()));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unusable_arguments_still_reach_the_page_but_authorise_nothing() {
        let (js, folder) = script(&Settings::default(), Some("C:/nope/missing.png"), true);
        assert!(
            js.contains("\"file\":null")
                && js.contains("missing.png")
                && js.contains("\"background\":true")
        );
        assert_eq!(folder, None);
        let (js, folder) = script(&Settings::default(), None, false);
        assert!(js.contains("\"path\":null"));
        assert_eq!(folder, None);
    }
}
