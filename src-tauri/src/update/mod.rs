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
pub use download::Progress;
pub use install::{fetch_installer, is_installed, run_installer};
pub use release::Update;

use std::sync::atomic::{AtomicBool, Ordering};
use transport::{api_agent, get_text, Source};

pub const REPO: &str = "juninmd/vislo";

static IN_FLIGHT: AtomicBool = AtomicBool::new(false);
/// Set by [`cancel_install`]; read by the download loop.
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Held from the download to the launch of the installer. Two runs would race for the same folder and
/// could start two silent installers, so a second start is refused while this exists.
pub struct InstallSession(());

impl InstallSession {
    pub fn start() -> Result<Self, String> {
        if IN_FLIGHT.swap(true, Ordering::SeqCst) {
            return Err("an update is already in progress".into());
        }
        CANCEL.store(false, Ordering::SeqCst);
        Ok(Self(()))
    }
}

/// Asks the running install to stop; its partial download is discarded.
pub fn cancel_install() {
    CANCEL.store(true, Ordering::SeqCst);
}

impl Drop for InstallSession {
    fn drop(&mut self) {
        IN_FLIGHT.store(false, Ordering::SeqCst);
    }
}

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
