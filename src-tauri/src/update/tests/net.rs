//! The download path end to end, against a loopback server that stands in for GitHub.

use crate::update::install::fetch_installer_from;
use crate::update::testsupport::{serve, Reply};
use crate::update::transport::Source;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const INSTALLER: &[u8] = b"MZ synthetic installer bytes, not an executable";
const NAME: &str = "Vislo_0.2.0_x64-setup.exe";
const MAX: u64 = 1 << 20;

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn release_json(installer_url: &str, sums_url: &str) -> String {
    serde_json::json!({
        "tag_name": "v0.2.0",
        "draft": false,
        "prerelease": false,
        "body": "notes",
        "html_url": "https://github.com/juninmd/vislo/releases/tag/v0.2.0",
        "assets": [
            { "name": NAME, "browser_download_url": installer_url },
            { "name": "SHA256SUMS.txt", "browser_download_url": sums_url }
        ]
    })
    .to_string()
}

/// A GitHub stand-in serving `installer` under `{origin}/dl/` and the checksum file `sums(sha256(installer))`.
fn release_server(installer: &'static [u8], sums: impl Fn(&str) -> String) -> String {
    serve(|origin| {
        let dl = format!("{origin}/dl/");
        vec![
            (
                "/api/releases/latest".into(),
                Reply::Body(release_json(&format!("{dl}{NAME}"), &format!("{dl}SHA256SUMS.txt")).into_bytes()),
            ),
            (format!("/dl/{NAME}"), Reply::Body(installer.to_vec())),
            ("/dl/SHA256SUMS.txt".into(), Reply::Body(sums(&hex(installer)).into_bytes())),
        ]
    })
}

fn run(origin: &str, max_bytes: u64, dest: &Path) -> Result<PathBuf, String> {
    let api = format!("{origin}/api");
    let dl = format!("{origin}/dl/");
    let src = Source { api_base: &api, download_prefix: &dl, max_bytes };
    fetch_installer_from(&src, "0.1.0", dest)
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vislo-net-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// How many files called `name` sit anywhere under `dir`.
fn files_named(dir: &Path, name: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                files_named(&p, name)
            } else {
                usize::from(p.file_name().is_some_and(|n| n == name))
            }
        })
        .sum()
}

#[test]
fn a_matching_checksum_keeps_the_installer() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let path = run(&origin, MAX, &scratch("keep")).expect("verified installer");
    assert_eq!(path.file_name().unwrap(), NAME);
    assert_eq!(std::fs::read(&path).unwrap(), INSTALLER);
}

#[test]
fn a_wrong_checksum_is_an_error_and_the_installer_is_discarded() {
    let origin = release_server(INSTALLER, |_| format!("{}  {NAME}\n", "0".repeat(64)));
    let dest = scratch("mismatch");
    let err = run(&origin, MAX, &dest).unwrap_err();
    assert!(err.contains("failed its checksum"), "{err}");
    assert_eq!(files_named(&dest, NAME), 0, "a file that failed its checksum must not stay on disk");
}

#[test]
fn an_installer_over_the_size_limit_is_refused() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let err = run(&origin, 16, &scratch("limit")).unwrap_err();
    assert!(err.contains("size limit"), "{err}");
}

#[test]
fn both_sha256sum_spellings_are_accepted() {
    for sep in ["  ", " *"] {
        let origin = release_server(INSTALLER, move |h| format!("{h}{sep}{NAME}\n"));
        let path = run(&origin, MAX, &scratch("spelling")).unwrap_or_else(|e| panic!("{sep:?}: {e}"));
        assert_eq!(path.file_name().unwrap(), NAME);
    }
}

#[test]
fn an_installer_url_outside_the_download_prefix_is_refused() {
    let origin = serve(|origin| {
        let release = release_json(&format!("{origin}/elsewhere/{NAME}"), &format!("{origin}/dl/SHA256SUMS.txt"));
        vec![("/api/releases/latest".into(), Reply::Body(release.into_bytes()))]
    });
    let err = run(&origin, MAX, &scratch("prefix")).unwrap_err();
    assert!(err.contains("from this repository"), "{err}");
}
