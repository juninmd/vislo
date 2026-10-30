//! HTTP access to GitHub for the updater; every request goes through here.

use std::time::Duration;

/// Where the updater talks to. Production is [`Source::GITHUB`]; tests point it at a loopback server.
#[derive(Clone, Copy)]
pub struct Source<'a> {
    /// `https://api.github.com/repos/<owner>/<repo>`, without a trailing slash.
    pub api_base: &'a str,
    /// Installer and checksum URLs must start with this, so nothing outside the repository's releases runs.
    pub download_prefix: &'a str,
    /// Largest installer accepted, in bytes.
    pub max_bytes: u64,
}

impl Source<'static> {
    pub const GITHUB: Source<'static> = Source {
        api_base: "https://api.github.com/repos/juninmd/vislo",
        download_prefix: "https://github.com/juninmd/vislo/releases/download/",
        max_bytes: 300 * 1024 * 1024,
    };
}

pub(super) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(60)))
        .user_agent(concat!("vislo/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

pub(super) fn get_text(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("update check failed: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("update check failed: {e}"))
}
