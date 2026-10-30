//! Reading the GitHub release JSON and the `SHA256SUMS.txt` format; no I/O here.

use serde::Serialize;
use serde_json::Value;

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

pub(super) const SUMS_ASSET: &str = "SHA256SUMS.txt";

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

fn download_url(release: &Value, name: &str, prefix: &str) -> Option<String> {
    release["assets"]
        .as_array()?
        .iter()
        .find(|a| a["name"].as_str() == Some(name))
        .and_then(|a| a["browser_download_url"].as_str())
        .filter(|u| u.starts_with(prefix)) // never follow a URL outside this repository's releases
        .map(str::to_string)
}

/// The newest stable release, if it is newer than `current` and ships an installer and checksums
/// whose download URLs start with `download_prefix`.
pub fn pick(release: &Value, current: &str, download_prefix: &str) -> Option<Candidate> {
    if release["draft"].as_bool() == Some(true) || release["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = release["tag_name"].as_str()?;
    if !is_newer(current, tag) {
        return None;
    }
    let version = tag.trim_start_matches('v').to_string();
    // the name the release workflow publishes; anything else (a stray or hostile asset) is not offered
    let installer_name = format!("Vislo_{version}_x64-setup.exe");
    Some(Candidate {
        update: Update {
            version,
            notes: release["body"].as_str().unwrap_or_default().trim().to_string(),
            page: release["html_url"].as_str().unwrap_or_default().to_string(),
            installable: false,
        },
        installer_url: download_url(release, &installer_name, download_prefix)?,
        sums_url: download_url(release, SUMS_ASSET, download_prefix)?,
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
