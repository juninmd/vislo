//! Self-update from this project's own GitHub releases.
//!
//! The release workflow publishes `Vislo_<version>_x64-setup.exe` and a `SHA256SUMS.txt`; the app
//! compares the latest release tag with its own version, downloads the installer over HTTPS,
//! checks the digest and runs it silently. Portable copies (no `uninstall.exe` beside the exe)
//! are only pointed at the release page.

mod cleanup;
mod download;
mod install;
mod release;
mod transport;

#[cfg(test)]
mod testsupport;
#[cfg(test)]
mod tests;

pub use cleanup::remove_stale_runs_at_startup;
pub use install::{fetch_installer, is_installed, run_installer};
pub use release::Update;

use transport::{api_agent, get_text, Source};

pub const REPO: &str = "juninmd/vislo";

/// Asks GitHub for the latest release; `None` when already up to date.
pub fn check(current: &str) -> Result<Option<Update>, String> {
    let src = Source::GITHUB;
    let body = get_text(&api_agent(&src), &src, &format!("{}/releases/latest", src.api_base))?;
    let release: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let installed = std::env::current_exe().is_ok_and(|exe| is_installed(&exe));
    Ok(release::pick(&release, current, src.download_prefix).map(|mut c| {
        c.update.installable = installed;
        c.update
    }))
}
