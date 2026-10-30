//! The download path end to end, against a loopback server that stands in for GitHub.

use crate::update::download::{Control, Progress};
use crate::update::install::fetch_installer_from;
use crate::update::testsupport::{serve, Reply};
use crate::update::transport::{Source, Timeouts};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

const INSTALLER: &[u8] = b"MZ synthetic installer bytes, not an executable";
const NAME: &str = "Vislo_0.2.0_x64-setup.exe";
const MAX: u64 = 1 << 20;
const FAST: Timeouts = Timeouts {
    api: Duration::from_secs(5),
    connect: Duration::from_secs(5),
    inactivity: Duration::from_millis(300),
};

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn release_json(installer_url: &str, sums_url: &str) -> String {
    serde_json::json!({
        "tag_name": "v0.2.0",
        "draft": false,
        "prerelease": false,
        "body": "notes",
        "html_url": "https://github.com/juninmd/vislo/releases/tag/v0.2.0",
        "assets": [
            { "name": NAME, "browser_download_url": installer_url },
            { "name": "SHA256SUMS.txt", "browser_download_url": sums_url }
        ]
    })
    .to_string()
}

/// A GitHub stand-in serving `installer` under `{origin}/dl/` and the checksum file `sums(sha256(installer))`.
fn release_server(installer: &'static [u8], sums: impl Fn(&str) -> String) -> String {
    serve(|origin| {
        let dl = format!("{origin}/dl/");
        vec![
            (
                "/api/releases/latest".into(),
                Reply::Body(release_json(&format!("{dl}{NAME}"), &format!("{dl}SHA256SUMS.txt")).into_bytes()),
            ),
            (format!("/dl/{NAME}"), Reply::Body(installer.to_vec())),
            ("/dl/SHA256SUMS.txt".into(), Reply::Body(sums(&hex(installer)).into_bytes())),
        ]
    })
}

/// A fetch with nothing watching it: no cancel, progress discarded.
fn fetch_from(src: &Source, dest: &Path) -> Result<crate::update::install::Installer, String> {
    let cancel = AtomicBool::new(false);
    let mut progress = |_: Progress| {};
    let mut ctl = Control { cancel: &cancel, progress: &mut progress };
    fetch_installer_from(src, "0.1.0", dest, &mut ctl)
}

/// Runs against `origin` as the test stand-in for GitHub; plain http is allowed on that origin only.
fn run(origin: &str, max_bytes: u64, dest: &Path) -> Result<PathBuf, String> {
    let api = format!("{origin}/api");
    let dl = format!("{origin}/dl/");
    let plain = format!("{origin}/");
    let src = Source { api_base: &api, download_prefix: &dl, max_bytes, plain_http_origin: Some(&plain), timeouts: FAST };
    fetch_from(&src, dest).map(|installer| installer.path().to_path_buf())
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("vislo-net-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// How many files called `name` sit anywhere under `dir`.
fn files_named(dir: &Path, name: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                files_named(&p, name)
            } else {
                usize::from(p.file_name().is_some_and(|n| n == name))
            }
        })
        .sum()
}

#[test]
fn a_matching_checksum_keeps_the_installer() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let path = run(&origin, MAX, &scratch("keep")).expect("verified installer");
    assert_eq!(path.file_name().unwrap(), NAME);
    assert_eq!(std::fs::read(&path).unwrap(), INSTALLER);
}

#[test]
fn a_wrong_checksum_is_an_error_and_the_installer_is_discarded() {
    let origin = release_server(INSTALLER, |_| format!("{}  {NAME}\n", "0".repeat(64)));
    let dest = scratch("mismatch");
    let err = run(&origin, MAX, &dest).unwrap_err();
    assert!(err.contains("failed its checksum"), "{err}");
    assert_eq!(files_named(&dest, NAME), 0, "a file that failed its checksum must not stay on disk");
}

#[test]
fn an_installer_over_the_size_limit_is_refused() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let err = run(&origin, 16, &scratch("limit")).unwrap_err();
    assert!(err.contains("size limit"), "{err}");
}

#[test]
fn both_sha256sum_spellings_are_accepted() {
    for sep in ["  ", " *"] {
        let origin = release_server(INSTALLER, move |h| format!("{h}{sep}{NAME}\n"));
        let path = run(&origin, MAX, &scratch("spelling")).unwrap_or_else(|e| panic!("{sep:?}: {e}"));
        assert_eq!(path.file_name().unwrap(), NAME);
    }
}

#[cfg(windows)]
#[test]
fn a_verified_installer_cannot_be_replaced_until_it_runs() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let api = format!("{origin}/api");
    let dl = format!("{origin}/dl/");
    let plain = format!("{origin}/");
    let src = Source { api_base: &api, download_prefix: &dl, max_bytes: MAX, plain_http_origin: Some(&plain), timeouts: FAST };
    let installer = fetch_from(&src, &scratch("locked")).expect("verified installer");
    let writer = std::fs::OpenOptions::new().write(true).open(installer.path());
    assert!(writer.is_err(), "a writer could swap the installer after its checksum was taken");
}

#[test]
fn each_run_gets_its_own_folder_under_the_root() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let root = scratch("folders");
    let first = run(&origin, MAX, &root).expect("first run");
    let second = run(&origin, MAX, &root).expect("second run");
    assert_ne!(first.parent(), second.parent(), "two runs must not share a folder");
    assert_eq!(first.parent().unwrap().parent(), Some(root.as_path()));
}

#[test]
fn a_redirect_to_another_plain_http_origin_is_refused() {
    let elsewhere = serve(|_| vec![(format!("/{NAME}"), Reply::Body(INSTALLER.to_vec()))]);
    let origin = serve(|origin| {
        let dl = format!("{origin}/dl/");
        vec![
            ("/api/releases/latest".into(), Reply::Body(release_json(&format!("{dl}{NAME}"), &format!("{dl}SHA256SUMS.txt")).into_bytes())),
            (format!("/dl/{NAME}"), Reply::Redirect(format!("{elsewhere}/{NAME}"))),
            ("/dl/SHA256SUMS.txt".into(), Reply::Body(format!("{}  {NAME}\n", hex(INSTALLER)).into_bytes())),
        ]
    });
    let err = run(&origin, MAX, &scratch("redirect")).unwrap_err();
    assert!(err.contains("redirect"), "{err}");
}

#[test]
fn a_redirect_that_stays_on_the_same_origin_is_followed() {
    let origin = serve(|origin| {
        let dl = format!("{origin}/dl/");
        vec![
            ("/api/releases/latest".into(), Reply::Body(release_json(&format!("{dl}{NAME}"), &format!("{dl}SHA256SUMS.txt")).into_bytes())),
            (format!("/dl/{NAME}"), Reply::Redirect(format!("{origin}/moved/{NAME}"))),
            (format!("/moved/{NAME}"), Reply::Body(INSTALLER.to_vec())),
            ("/dl/SHA256SUMS.txt".into(), Reply::Body(format!("{}  {NAME}\n", hex(INSTALLER)).into_bytes())),
        ]
    });
    let path = run(&origin, MAX, &scratch("moved")).expect("same-origin redirect");
    assert_eq!(std::fs::read(&path).unwrap(), INSTALLER);
}

#[test]
fn production_refuses_plain_http_even_when_the_server_answers() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let api = format!("{origin}/api");
    let dl = format!("{origin}/dl/");
    let src = Source { api_base: &api, download_prefix: &dl, max_bytes: MAX, plain_http_origin: None, timeouts: FAST };
    let err = fetch_from(&src, &scratch("https-only")).unwrap_err();
    assert!(err.contains("https"), "{err}");
}

#[test]
fn an_installer_url_outside_the_download_prefix_is_refused() {
    let origin = serve(|origin| {
        let release = release_json(&format!("{origin}/elsewhere/{NAME}"), &format!("{origin}/dl/SHA256SUMS.txt"));
        vec![("/api/releases/latest".into(), Reply::Body(release.into_bytes()))]
    });
    let err = run(&origin, MAX, &scratch("prefix")).unwrap_err();
    assert!(err.contains("from this repository"), "{err}");
}

/// A GitHub stand-in whose installer is `body`, served through `reply`, with a checksum file matching `body`.
fn server_serving(body: &'static [u8], reply: impl FnOnce(&[u8]) -> Reply) -> String {
    serve(|origin| {
        let dl = format!("{origin}/dl/");
        vec![
            ("/api/releases/latest".into(), Reply::Body(release_json(&format!("{dl}{NAME}"), &format!("{dl}SHA256SUMS.txt")).into_bytes())),
            (format!("/dl/{NAME}"), reply(body)),
            ("/dl/SHA256SUMS.txt".into(), Reply::Body(format!("{}  {NAME}\n", hex(body)).into_bytes())),
        ]
    })
}

#[test]
fn a_slow_download_that_keeps_moving_completes_past_the_inactivity_window() {
    // 16 pieces 50 ms apart: about 800 ms in all, well past the 300 ms window, yet never idle for long
    static BODY: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| INSTALLER.repeat(1500));
    let origin = server_serving(&BODY, |b| Reply::Trickle { body: b.to_vec(), chunk: b.len() / 16 + 1, every: Duration::from_millis(50) });
    let started = Instant::now();
    let path = run(&origin, u64::MAX, &scratch("slow")).expect("a steady trickle is not a stall");
    assert!(started.elapsed() > FAST.inactivity * 2, "the test must outlast the window");
    assert_eq!(std::fs::read(&path).unwrap(), *BODY);
}

#[test]
fn a_stalled_download_fails_after_the_inactivity_window_not_at_the_end_of_the_stall() {
    static BODY: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| INSTALLER.repeat(1500));
    let origin = server_serving(&BODY, |b| Reply::Stall { body: b.to_vec(), head: 4096, hold: Duration::from_secs(3) });
    let started = Instant::now();
    let err = run(&origin, u64::MAX, &scratch("stall")).unwrap_err();
    assert!(started.elapsed() < Duration::from_secs(2), "gave up after {:?}", started.elapsed());
    assert!(err.contains("stalled"), "{err}");
}

#[test]
fn progress_reports_the_bytes_so_far_and_ends_at_the_full_size() {
    let origin = release_server(INSTALLER, |h| format!("{h}  {NAME}\n"));
    let api = format!("{origin}/api");
    let dl = format!("{origin}/dl/");
    let plain = format!("{origin}/");
    let src = Source { api_base: &api, download_prefix: &dl, max_bytes: MAX, plain_http_origin: Some(&plain), timeouts: FAST };
    let cancel = AtomicBool::new(false);
    let mut seen = Vec::new();
    let mut progress = |p: Progress| seen.push(p);
    let mut ctl = Control { cancel: &cancel, progress: &mut progress };
    fetch_installer_from(&src, "0.1.0", &scratch("progress"), &mut ctl).expect("verified installer");
    let last = *seen.last().expect("at least one report");
    assert_eq!(last, Progress { received: INSTALLER.len() as u64, total: Some(INSTALLER.len() as u64) });
    assert!(seen.windows(2).all(|w| w[0].received <= w[1].received), "progress never goes backwards");
}

#[test]
fn cancelling_stops_the_download_and_discards_the_partial_file() {
    static BODY: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| INSTALLER.repeat(1500));
    let origin = server_serving(&BODY, |b| Reply::Trickle { body: b.to_vec(), chunk: b.len() / 16 + 1, every: Duration::from_millis(50) });
    let api = format!("{origin}/api");
    let dl = format!("{origin}/dl/");
    let plain = format!("{origin}/");
    let src = Source { api_base: &api, download_prefix: &dl, max_bytes: u64::MAX, plain_http_origin: Some(&plain), timeouts: FAST };
    let dest = scratch("cancel");
    let cancel = AtomicBool::new(false);
    let mut progress = |_: Progress| {};
    let mut ctl = Control { cancel: &cancel, progress: &mut progress };
    let started = Instant::now();
    let flag = &cancel;
    let err = std::thread::scope(|s| {
        s.spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            flag.store(true, Ordering::SeqCst);
        });
        fetch_installer_from(&src, "0.1.0", &dest, &mut ctl).unwrap_err()
    });
    assert!(err.contains("cancelled"), "{err}");
    assert!(started.elapsed() < Duration::from_millis(700), "a cancel must not wait for the whole download");
    assert_eq!(files_named(&dest, NAME), 0, "the partial installer must be removed");
}
