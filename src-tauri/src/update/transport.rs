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
    /// Tests only: the one `http://` origin (with its trailing slash) allowed in place of https. `None` in production.
    pub plain_http_origin: Option<&'a str>,
    pub timeouts: Timeouts,
}

/// Time limits. The installer has no short total limit: a slow link that keeps delivering bytes may take up to
/// `BODY_BACKSTOP`, and a stall is caught by the inactivity window long before that.
#[derive(Clone, Copy)]
pub struct Timeouts {
    /// The whole small JSON request.
    pub api: Duration,
    /// Opening the connection, TLS included.
    pub connect: Duration,
    /// Longest silence while the installer streams in.
    pub inactivity: Duration,
}

impl Source<'static> {
    pub const GITHUB: Source<'static> = Source {
        api_base: "https://api.github.com/repos/juninmd/vislo",
        download_prefix: "https://github.com/juninmd/vislo/releases/download/",
        max_bytes: 300 * 1024 * 1024,
        plain_http_origin: None,
        timeouts: Timeouts {
            api: Duration::from_secs(15),
            connect: Duration::from_secs(10),
            inactivity: Duration::from_secs(30),
        },
    };
}

/// Redirects followed before giving up; GitHub's download hop is one, so this leaves room.
const MAX_REDIRECTS: usize = 5;

/// Ends a download whose connection never closes; stalls are caught earlier by the inactivity window.
const BODY_BACKSTOP: Duration = Duration::from_secs(60 * 60);
const USER_AGENT: &str = concat!("vislo/", env!("CARGO_PKG_VERSION"));

/// Agents never follow redirects themselves: [`send`] does, checking every hop.
pub(super) fn api_agent(src: &Source) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(src.plain_http_origin.is_none())
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_global(Some(src.timeouts.api))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

pub(super) fn download_agent(src: &Source) -> ureq::Agent {
    ureq::Agent::config_builder()
        .https_only(src.plain_http_origin.is_none())
        .max_redirects(0)
        .max_redirects_will_error(false)
        .timeout_connect(Some(src.timeouts.connect))
        .timeout_recv_response(Some(src.timeouts.inactivity))
        .timeout_recv_body(Some(BODY_BACKSTOP))
        .user_agent(USER_AGENT)
        .build()
        .into()
}

fn allowed(src: &Source, url: &str) -> bool {
    url.starts_with("https://") || src.plain_http_origin.is_some_and(|origin| url.starts_with(origin))
}

/// Resolves a `Location` header; only absolute URLs and root-relative paths are understood.
fn resolve(base: &str, location: &str) -> Result<String, String> {
    if location.starts_with("https://") || location.starts_with("http://") {
        return Ok(location.to_string());
    }
    if location.starts_with('/') && !location.starts_with("//") {
        let origin = base.splitn(4, '/').take(3).collect::<Vec<_>>().join("/");
        return Ok(format!("{origin}{location}"));
    }
    Err(format!("unsupported redirect to {location}"))
}

/// GET that follows redirects itself, so every hop meets the same rule as the first request.
pub(super) fn send(
    agent: &ureq::Agent,
    src: &Source,
    url: &str,
    accept: &str,
) -> Result<ureq::http::Response<ureq::Body>, String> {
    let mut next = url.to_string();
    for hop in 0..=MAX_REDIRECTS {
        if !allowed(src, &next) {
            return Err(if hop == 0 {
                format!("refusing an insecure URL {next}: only https is allowed")
            } else {
                format!("refusing a redirect to {next}: only https is allowed")
            });
        }
        let response = agent
            .get(&next)
            .header("Accept", accept)
            .call()
            .map_err(|e| format!("could not reach GitHub: {e}"))?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or("GitHub redirected without a location")?;
        next = resolve(&next, location)?;
    }
    Err("GitHub redirected too many times".into())
}

pub(super) fn get_text(agent: &ureq::Agent, src: &Source, url: &str) -> Result<String, String> {
    send(agent, src, url, "application/vnd.github+json")?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("could not read GitHub's answer: {e}"))
}
