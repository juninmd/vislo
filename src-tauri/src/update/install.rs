//! Downloading the installer, checking its digest and starting it.

use super::release::{expected_hash, is_newer, pick};
use super::transport::{agent, get_text, send, Source};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Installed copies sit beside the `uninstall.exe` the NSIS installer writes.
pub fn is_installed(exe: &Path) -> bool {
    exe.parent().is_some_and(|d| d.join("uninstall.exe").is_file())
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

fn download(agent: &ureq::Agent, src: &Source, url: &str, dest: &Path) -> Result<(), String> {
    let mut response = send(agent, src, url, "application/octet-stream")?;
    if response.body().content_length().is_some_and(|n| n > src.max_bytes) {
        return Err(format!("the installer is larger than the {}-byte size limit", src.max_bytes));
    }
    let mut reader = response.body_mut().with_config().limit(src.max_bytes).reader();
    let mut out = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    std::io::copy(&mut reader, &mut out).map_err(|e| format!("download failed: {e}"))?;
    out.flush().map_err(|e| e.to_string())
}

/// Downloads the installer of the latest release and verifies it against `SHA256SUMS.txt`.
pub fn fetch_installer(current: &str) -> Result<PathBuf, String> {
    fetch_installer_from(&Source::GITHUB, current, &updates_root()?)
}

/// Per-user and not shared with other programs, unlike the temp folder.
fn updates_root() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(|dir| PathBuf::from(dir).join("Vislo").join("updates"))
        .ok_or_else(|| "LOCALAPPDATA is not set, so the update cannot be staged".into())
}

/// A new folder with an unguessable name, so nothing can be planted in it ahead of the download.
fn new_run_dir(root: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    for _ in 0..8 {
        let dir = root.join(random_name());
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("could not create a private update folder".into())
}

/// 128 bits; std seeds `RandomState` from the OS random source, so the names are not guessable.
fn random_name() -> String {
    use std::hash::{BuildHasher, Hasher};
    (0..2)
        .map(|_| {
            let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
            hasher.write_u64(u64::from(std::process::id()));
            format!("{:016x}", hasher.finish())
        })
        .collect()
}

/// [`fetch_installer`] against any `src`, writing under `dest_root`.
pub fn fetch_installer_from(src: &Source, current: &str, dest_root: &Path) -> Result<PathBuf, String> {
    let agent = agent(src);
    let body = get_text(&agent, src, &format!("{}/releases/latest", src.api_base))?;
    let release: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let c = pick(&release, current, src.download_prefix).ok_or_else(|| no_candidate_reason(&release, current))?;
    let expected = expected_hash(&get_text(&agent, src, &c.sums_url)?, &c.installer_name)
        .ok_or("the release has no checksum for its installer")?;

    let file = new_run_dir(dest_root)?.join(&c.installer_name);
    download(&agent, src, &c.installer_url, &file)?;
    if sha256_hex(&file)? != expected {
        let _ = std::fs::remove_file(&file);
        return Err("the downloaded installer failed its checksum and was discarded".into());
    }
    Ok(file)
}

fn no_candidate_reason(release: &Value, current: &str) -> &'static str {
    match release["tag_name"].as_str() {
        Some(tag) if is_newer(current, tag) => "the latest release has no installer or checksums from this repository",
        _ => "already up to date",
    }
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
