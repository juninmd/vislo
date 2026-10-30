//! Downloading the installer, checking its digest and starting it.

use super::release::{expected_hash, pick};
use super::transport::{agent, get_text};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use super::REPO;

const MAX_INSTALLER_BYTES: u64 = 300 * 1024 * 1024;

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
