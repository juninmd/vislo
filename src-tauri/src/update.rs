//! Self-update from this project's own GitHub releases.
//!
//! The release workflow publishes `Vislo_<version>_x64-setup.exe` and a `SHA256SUMS.txt`; the app
//! compares the latest release tag with its own version, downloads the installer over HTTPS,
//! checks the digest and runs it silently. Portable copies (no `uninstall.exe` beside the exe)
//! are only pointed at the release page.

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const REPO: &str = "juninmd/vislo";
const MAX_INSTALLER_BYTES: u64 = 300 * 1024 * 1024;
const SUMS_ASSET: &str = "SHA256SUMS.txt";

/// What the page needs to offer an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Update {
    pub version: String,
    pub notes: String,
    pub page: String,
    /// False for portable copies: they are sent to the release page instead.
    pub installable: bool,
}

/// An update plus where its files live (kept on the Rust side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub update: Update,
    pub installer_name: String,
    pub installer_url: String,
    pub sums_url: String,
}

pub fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.trim().trim_start_matches('v').split('.');
    let mut next = || parts.next()?.parse::<u64>().ok();
    let v = (next()?, next()?, next()?);
    parts.next().is_none().then_some(v)
}

pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (parse_version(current), parse_version(candidate)) {
        (Some(c), Some(n)) => n > c,
        _ => false,
    }
}

fn download_url(release: &Value, name: &str) -> Option<String> {
    let prefix = format!("https://github.com/{REPO}/releases/download/");
    release["assets"]
        .as_array()?
        .iter()
        .find(|a| a["name"].as_str() == Some(name))
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with(&prefix)) // never follow a URL outside this repository's releases
        .map(str::to_string)
}

/// The newest stable release, if it is newer than `current` and ships an installer and checksums.
pub fn pick(release: &Value, current: &str) -> Option<Candidate> {
    if release["draft"].as_bool() == Some(true) || release["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    if !is_newer(current, tag) {
        return None;
    }
    let version = tag.trim_start_matches('v').to_string();
    let installer_name = release["assets"]
        .as_array()?
        .iter()
        .filter_map(|a| a["name"].as_str())
        .find(|n| n.ends_with("-setup.exe"))?
        .to_string();
    Some(Candidate {
        update: Update {
            version,
            notes: release["body"].as_str().unwrap_or_default().trim().to_string(),
            page: release["html_url"].as_str().unwrap_or_default().to_string(),
            installable: false,
        },
        installer_url: download_url(release, &installer_name)?,
        sums_url: download_url(release, SUMS_ASSET)?,
        installer_name,
    })
}

/// Looks `file` up in `sha256sum` output (`<hex>  name` or `<hex> *name`).
pub fn expected_hash(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim().trim_start_matches('*');
        (name == file && hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            .then(|| hash.to_ascii_lowercase())
    })
}

/// Installed copies sit beside the `uninstall.exe` the NSIS installer writes.
pub fn is_installed(exe: &Path) -> bool {
    exe.parent().is_some_and(|d| d.join("uninstall.exe").is_file())
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(concat!("vislo/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get_text(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("update check failed: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("update check failed: {e}"))
}

/// Asks GitHub for the latest release; `None` when already up to date.
pub fn check(current: &str) -> Result<Option<Update>, String> {
    let body = get_text(&agent(), &format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    let release: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let installed = std::env::current_exe().is_ok_and(|exe| is_installed(&exe));
    Ok(pick(&release, current).map(|mut c| {
        c.update.installable = installed;
        c.update
    }))
}

pub fn sha256_hex(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn download(agent: &ureq::Agent, url: &str, dest: &Path) -> Result<(), String> {
    let mut response = agent.get(url).call().map_err(|e| format!("download failed: {e}"))?;
    let mut reader = response
        .body_mut()
        .with_config()
        .limit(MAX_INSTALLER_BYTES)
        .reader();
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut out).map_err(|e| format!("download failed: {e}"))?;
    out.flush().map_err(|e| e.to_string())
}

/// Downloads the installer of the latest release and verifies it against `SHA256SUMS.txt`.
pub fn fetch_installer(current: &str) -> Result<PathBuf, String> {
    let agent = agent();
    let body = get_text(&agent, &format!("https://api.github.com/repos/{REPO}/releases/latest"))?;
    let release: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let c = pick(&release, current).ok_or("already up to date")?;
    let expected = expected_hash(&get_text(&agent, &c.sums_url)?, &c.installer_name)
        .ok_or("the release has no checksum for its installer")?;

    let dir = std::env::temp_dir().join(format!("vislo-update-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(&c.installer_name);
    download(&agent, &c.installer_url, &file)?;
    if sha256_hex(&file)? != expected {
        let _ = std::fs::remove_file(&file);
        return Err("the downloaded installer failed its checksum and was discarded".into());
    }
    Ok(file)
}

/// Runs the installer silently, then starts the new Vislo from the same place.
#[cfg(windows)]
pub fn run_installer(setup: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new("cmd.exe")
        .raw_arg(format!(
            "/C start \"\" /wait \"{}\" /S && start \"\" \"{}\"",
            setup.display(),
            exe.display()
        ))
        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[cfg(not(windows))]
pub fn run_installer(_setup: &Path) -> Result<(), String> {
    Err("installing updates is Windows-only".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
            sha256_hex(&path).unwrap(),
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
        let setup = fetch_installer("0.0.1").expect("download and checksum");
        let name = setup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("Vislo_") && name.ends_with("_x64-setup.exe"), "{name}");
        assert!(name.contains(&update.version), "{name} is not {}", update.version);
        assert_eq!(std::fs::read(&setup).unwrap()[..2], *b"MZ", "an installer is a Windows executable");
        assert!(fetch_installer(&update.version).is_err(), "the latest copy has nothing to update to");
        println!("verified {name} ({} bytes)", std::fs::metadata(&setup).unwrap().len());
        let _ = std::fs::remove_dir_all(setup.parent().unwrap());
    }
}
