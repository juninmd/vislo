mod cleanup;
mod net;

use super::install::{fetch_installer, is_installed, sha256_hex};
use super::release::{expected_hash, is_newer, parse_version, Candidate};
use super::transport::Source;
use super::{check, REPO};
use serde_json::{json, Value};

fn pick(release: &Value, current: &str) -> Option<Candidate> {
    super::release::pick(release, current, Source::GITHUB.download_prefix)
}

fn release(tag: &str) -> Value {
    let base = format!("https://github.com/{REPO}/releases/download/{tag}");
    json!({
        "tag_name": tag,
        "draft": false,
        "prerelease": false,
        "body": "## Features\n- faster",
        "html_url": format!("https://github.com/{REPO}/releases/tag/{tag}"),
        "assets": [
            { "name": format!("Vislo_{}_x64-setup.exe", tag.trim_start_matches('v')), "browser_download_url": format!("{base}/Vislo_x64-setup.exe") },
            { "name": "SHA256SUMS.txt", "browser_download_url": format!("{base}/SHA256SUMS.txt") }
        ]
    })
}

#[test]
fn versions_compare_numerically_not_textually() {
    assert!(is_newer("0.9.0", "v0.10.0"));
    assert!(is_newer("1.2.3", "1.2.4"));
    assert!(!is_newer("1.2.3", "v1.2.3"));
    assert!(!is_newer("1.2.3", "1.2.2"));
}

#[test]
fn malformed_versions_never_count_as_newer() {
    assert!(!is_newer("0.1.0", "nightly"));
    assert!(!is_newer("0.1.0", "v1.0.0-beta.1"));
    assert!(!is_newer("0.1.0", "1.0"));
    assert_eq!(parse_version(" v2.0.1 "), Some((2, 0, 1)));
}

#[test]
fn picks_a_newer_stable_release_with_installer_and_checksums() {
    let c = pick(&release("v0.2.0"), "0.1.0").unwrap();
    assert_eq!(c.update.version, "0.2.0");
    assert_eq!(c.installer_name, "Vislo_0.2.0_x64-setup.exe");
    assert!(c.update.notes.starts_with("## Features"));
    assert!(pick(&release("v0.1.0"), "0.1.0").is_none());
}

#[test]
fn a_second_install_is_refused_while_one_is_running() {
    let first = super::InstallSession::start().expect("the first install starts");
    assert!(super::InstallSession::start().is_err(), "two installers must never run at once");
    drop(first);
    assert!(super::InstallSession::start().is_ok(), "the guard is released when the install ends");
}

#[test]
fn only_the_installer_named_for_the_release_version_is_picked() {
    // a stray `*-setup.exe` listed first must not win over the installer the release workflow names
    let mut r = release("v0.2.0");
    r["assets"].as_array_mut().unwrap().insert(0, json!({ "name": "other-setup.exe", "browser_download_url": "https://github.com/juninmd/vislo/releases/download/v0.2.0/other-setup.exe" }));
    assert_eq!(pick(&r, "0.1.0").unwrap().installer_name, "Vislo_0.2.0_x64-setup.exe");
    for name in ["..\\evil-setup.exe", "Vislo_9.9.9_x64-setup.exe", "Vislo_0.2.0_x64-setup.exe.bak", "Vislo_0.2.0-setup.exe"] {
        let mut r = release("v0.2.0");
        r["assets"][0]["name"] = json!(name);
        assert!(pick(&r, "0.1.0").is_none(), "{name} must not be installed");
    }
}

#[test]
fn skips_drafts_prereleases_and_incomplete_releases() {
    let mut r = release("v0.2.0");
    r["prerelease"] = json!(true);
    assert!(pick(&r, "0.1.0").is_none());
    let mut r = release("v0.2.0");
    r["draft"] = json!(true);
    assert!(pick(&r, "0.1.0").is_none());
    let mut r = release("v0.2.0");
    r["assets"] = json!([]);
    assert!(pick(&r, "0.1.0").is_none());
    let mut r = release("v0.2.0");
    r["assets"].as_array_mut().unwrap().pop(); // no SHA256SUMS.txt: refuse rather than install unchecked
    assert!(pick(&r, "0.1.0").is_none());
}

#[test]
fn refuses_download_urls_outside_this_repository() {
    let mut r = release("v0.2.0");
    r["assets"][0]["browser_download_url"] = json!("https://evil.example/Vislo_0.2.0_x64-setup.exe");
    assert!(pick(&r, "0.1.0").is_none());
}

#[test]
fn finds_the_checksum_of_a_named_file() {
    let h = "a".repeat(64);
    let sums = format!("{h}  Vislo_0.2.0_x64-setup.exe\n{}  *other.exe\n", "B".repeat(64));
    assert_eq!(expected_hash(&sums, "Vislo_0.2.0_x64-setup.exe"), Some(h));
    assert_eq!(expected_hash(&sums, "other.exe"), Some("b".repeat(64)));
    assert_eq!(expected_hash(&sums, "missing.exe"), None);
    assert_eq!(expected_hash("short  Vislo.exe", "Vislo.exe"), None);
}

#[test]
fn hashes_files_like_sha256sum() {
    let path = std::env::temp_dir().join(format!("vislo-sha-{}", std::process::id()));
    std::fs::write(&path, b"abc").unwrap();
    assert_eq!(
        sha256_hex(&mut std::fs::File::open(&path).unwrap()).unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let _ = std::fs::remove_file(path);
}

#[test]
fn only_copies_next_to_an_uninstaller_count_as_installed() {
    let dir = std::env::temp_dir().join(format!("vislo-inst-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let exe = dir.join("vislo.exe");
    assert!(!is_installed(&exe));
    std::fs::write(dir.join("uninstall.exe"), b"x").unwrap();
    assert!(is_installed(&exe));
    let _ = std::fs::remove_dir_all(dir);
}

/// Network: what an installed older copy does against the published release, minus running the installer.
/// `cargo test live_release -- --ignored --nocapture`
#[test]
#[ignore]
fn live_release_is_offered_downloaded_and_verified() {
    let update = check("0.0.1").expect("GitHub answers").expect("an old copy is offered the latest release");
    assert!(update.page.starts_with("https://github.com/juninmd/vislo/releases/tag/v"));
    let setup = fetch_installer("0.0.1").expect("download and checksum").path().to_path_buf();
    let name = setup.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("Vislo_") && name.ends_with("_x64-setup.exe"), "{name}");
    assert!(name.contains(&update.version), "{name} is not {}", update.version);
    assert_eq!(std::fs::read(&setup).unwrap()[..2], *b"MZ", "an installer is a Windows executable");
    assert!(fetch_installer(&update.version).is_err(), "the latest copy has nothing to update to");
    println!("verified {name} ({} bytes)", std::fs::metadata(&setup).unwrap().len());
    let _ = std::fs::remove_dir_all(setup.parent().unwrap());
}
