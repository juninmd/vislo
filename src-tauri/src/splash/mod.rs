//! Native first paint. The WebView2 environment takes ~270 ms to come up, so on a cold start the
//! picture is decoded and drawn by a bare Win32 layered window meanwhile, with the same zoom-in
//! the page would play. The real window takes over (identical pixels, same place) once it is ready.
//!
//! Only used when it cannot be told apart from the real window: opening maximized over a
//! transparent background, no other instance running, and a file Rust can decode.

mod geometry;
#[cfg(windows)]
mod win;

use crate::{
    album,
    settings::{Background, Settings},
};
use std::path::PathBuf;
use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// Tauri app identifier (tauri.conf.json): names the config folder and the single-instance window.
pub const IDENTIFIER: &str = "dev.juninmd.vislo";

/// How long the real window waits for the zoom-in to finish before showing itself.
const ANIMATION_WAIT: Duration = Duration::from_millis(400);
/// The page releases the splash once it paints its own dimming; if it never does, the splash leaves anyway.
const RELEASE_TIMEOUT: Duration = Duration::from_millis(1000);

#[derive(Default)]
struct State {
    started: bool,
    shown: bool,
    animation_done: bool,
    dismissed: bool,
}

static STATE: Mutex<State> = Mutex::new(State {
    started: false,
    shown: false,
    animation_done: false,
    dismissed: false,
});
static CHANGED: Condvar = Condvar::new();

fn update(f: impl FnOnce(&mut State)) {
    f(&mut STATE.lock().unwrap_or_else(|e| e.into_inner()));
    CHANGED.notify_all();
}

/// The file to paint ahead of the window, if this launch qualifies.
pub fn eligible(settings: &Settings, arg: Option<&str>, background: bool) -> Option<PathBuf> {
    if background
        || !settings.open_maximized
        || settings.fullscreen_on_start
        || settings.background != Background::Transparent
    {
        return None;
    }
    let path = dunce::canonicalize(arg?).ok()?;
    // AVIF has no Rust decoder here, so the page would show it without us.
    (path.is_file()
        && album::is_image(&path)
        && path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("avif")))
    .then_some(path)
}

/// Starts painting on a background thread (Windows only). Returns whether the page should skip
/// its own zoom-in because the splash plays it.
pub fn start(path: PathBuf) -> bool {
    #[cfg(windows)]
    {
        if win::another_instance_running() {
            return false;
        }
        update(|s| s.started = true);
        std::thread::spawn(move || win::run(&path));
        true
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        false
    }
}

/// Called just before the real window is shown. A splash that has not painted yet is cancelled
/// (the page animates itself); one mid-zoom is awaited so the hand-over is seamless.
pub fn before_show() {
    let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if !s.started {
        return;
    }
    if !s.shown {
        s.dismissed = true;
        return;
    }
    let (_s, _) = CHANGED
        .wait_timeout_while(s, ANIMATION_WAIT, |s| !s.animation_done)
        .unwrap_or_else(|e| e.into_inner());
}

/// Called right after the real window is shown. The splash keeps dimming the desktop until the page
/// asks for the hand-over (`release`), because the page needs a moment to paint its own dimming.
pub fn after_show() {
    if !STATE.lock().unwrap_or_else(|e| e.into_inner()).started {
        return;
    }
    std::thread::spawn(|| {
        std::thread::sleep(RELEASE_TIMEOUT);
        update(|s| s.dismissed = true);
    });
}

/// The page is painting its own dimming: the splash may start fading out its own.
pub fn release() {
    update(|s| s.dismissed = true);
}

fn is_dismissed() -> bool {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).dismissed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings {
            background: Background::Transparent,
            ..Settings::default()
        }
    }

    fn png(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vislo-splash-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a.png");
        std::fs::write(&file, b"x").unwrap();
        file
    }

    #[test]
    fn identifier_matches_tauri_conf() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], IDENTIFIER);
    }

    #[test]
    fn only_a_transparent_maximized_foreground_launch_with_an_image_qualifies() {
        let f = png("elig");
        let arg = f.to_str();
        assert!(eligible(&settings(), arg, false).is_some());
        assert!(
            eligible(&settings(), arg, true).is_none(),
            "background start shows nothing"
        );
        assert!(
            eligible(&Settings::default(), arg, false).is_some(),
            "transparent maximized is the default"
        );
        assert!(eligible(
            &Settings {
                background: Background::Dark,
                ..settings()
            },
            arg,
            false
        )
        .is_none());
        assert!(eligible(
            &Settings {
                open_maximized: false,
                ..settings()
            },
            arg,
            false
        )
        .is_none());
        assert!(eligible(
            &Settings {
                fullscreen_on_start: true,
                ..settings()
            },
            arg,
            false
        )
        .is_none());
        assert!(eligible(&settings(), None, false).is_none());
        assert!(eligible(&settings(), Some("Z:/nope.png"), false).is_none());
        let avif = f.with_file_name("b.avif");
        std::fs::write(&avif, b"x").unwrap();
        assert!(eligible(&settings(), avif.to_str(), false).is_none());
        let _ = std::fs::remove_dir_all(f.parent().unwrap());
    }
}
