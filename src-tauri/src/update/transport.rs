//! HTTP access to GitHub for the updater; every request goes through here.

use std::time::Duration;

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
