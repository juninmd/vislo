//! User settings, persisted as JSON. Viewer options (wheel behaviour,
//! fullscreen on start, auto-hiding action bar, background) plus Vislo's own toggles.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WheelMode {
    Zoom,
    Navigate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Background {
    Dark,
    Black,
    Transparent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub wheel: WheelMode,
    pub fullscreen_on_start: bool,
    /// Open filling the screen without a title bar; off sizes the window to the picture.
    pub open_maximized: bool,
    pub autohide_controls: bool,
    pub background: Background,
    pub sharp_downscale: bool,
    pub embedded_preview: bool,
    pub filmstrip_3d: bool,
    pub hover_preview: bool,
    pub slideshow_display_ms: u32,
    pub slideshow_fade_ms: u32,
    /// Closing the window parks it in the tray, so the next open skips the WebView start-up.
    pub keep_ready: bool,
    /// Launch parked when the user signs in (only meaningful with `keep_ready`).
    pub start_with_windows: bool,
    /// Look for a newer release on GitHub when Vislo starts (and every few hours while it runs).
    pub check_updates: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            wheel: WheelMode::Zoom,
            fullscreen_on_start: false,
            open_maximized: true,
            autohide_controls: true,
            background: Background::Transparent,
            sharp_downscale: true,
            embedded_preview: true,
            filmstrip_3d: true,
            hover_preview: true,
            slideshow_display_ms: 4000, // 4 s per picture, 1 s fade
            slideshow_fade_ms: 1000,
            keep_ready: false,
            start_with_windows: false,
            check_updates: true,
        }
    }
}

impl Settings {
    /// Clamps free-form numbers so a hand-edited file cannot create a zero-length slideshow.
    pub fn normalised(mut self) -> Self {
        self.slideshow_display_ms = self.slideshow_display_ms.clamp(500, 60_000);
        self.slideshow_fade_ms = self
            .slideshow_fade_ms
            .min(5_000)
            .min(self.slideshow_display_ms);
        self.start_with_windows &= self.keep_ready;
        self
    }
}

/// Applies each field of `value` on top of the defaults, one at a time, so one bad value
/// (an unknown enum variant, a negative number) costs only that field, not the whole file.
fn lenient(value: serde_json::Value) -> Option<Settings> {
    let serde_json::Value::Object(fields) = value else {
        return None;
    };
    let mut merged = serde_json::to_value(Settings::default()).ok()?;
    for (key, v) in fields {
        let mut trial = merged.clone();
        trial[&key] = v;
        match serde_json::from_value::<Settings>(trial.clone()) {
            Ok(_) => merged = trial,
            Err(e) => eprintln!("vislo: ignoring invalid setting {key} ({e})"),
        }
    }
    serde_json::from_value(merged).ok()
}

/// A missing file falls back to defaults. Valid fields of a partly bad file are kept; a file
/// that is not a JSON object at all is copied to `<name>.bak` (once) before defaults take over.
pub fn load(path: &Path) -> Settings {
    let Ok(bytes) = std::fs::read(path) else {
        return Settings::default();
    };
    let parsed = serde_json::from_slice(&bytes).ok().and_then(lenient);
    parsed
        .unwrap_or_else(|| {
            eprintln!("vislo: ignoring unreadable settings");
            let bak = path.with_extension("json.bak");
            if !bak.exists() {
                let _ = std::fs::copy(path, bak);
            }
            Settings::default()
        })
        .normalised()
}

pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub struct Store {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl Store {
    pub fn open(path: PathBuf) -> Self {
        let current = Mutex::new(load(&path));
        Store { path, current }
    }

    pub fn get(&self) -> Settings {
        self.current
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn set(&self, next: Settings) -> Result<Settings, String> {
        let next = next.normalised();
        save(&self.path, &next)?;
        *self.current.lock().unwrap_or_else(|e| e.into_inner()) = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vislo-settings-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_have_slideshow_timing_and_wheel_zoom() {
        let d = Settings::default();
        assert_eq!((d.slideshow_display_ms, d.slideshow_fade_ms), (4000, 1000));
        assert_eq!(d.wheel, WheelMode::Zoom);
    }

    #[test]
    fn defaults_open_maximized_over_a_transparent_background() {
        let d = Settings::default();
        assert!(d.open_maximized && !d.fullscreen_on_start);
        assert_eq!(d.background, Background::Transparent);
    }

    #[test]
    fn settings_files_written_before_open_maximized_existed_still_load() {
        let dir = temp("legacy");
        std::fs::write(
            dir.join("s.json"),
            br#"{"wheel":"navigate","background":"dark"}"#,
        )
        .unwrap();
        let s = load(&dir.join("s.json"));
        assert_eq!(
            (s.wheel, s.background, s.open_maximized),
            (WheelMode::Navigate, Background::Dark, true)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn update_checks_are_on_unless_a_settings_file_says_otherwise() {
        assert!(Settings::default().check_updates);
        let dir = temp("updates");
        std::fs::write(dir.join("s.json"), br#"{"check_updates":false}"#).unwrap();
        assert!(!load(&dir.join("s.json")).check_updates);
        std::fs::write(dir.join("old.json"), br#"{"wheel":"zoom"}"#).unwrap();
        assert!(load(&dir.join("old.json")).check_updates);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn login_start_needs_keep_ready() {
        let s = Settings {
            start_with_windows: true,
            keep_ready: false,
            ..Settings::default()
        }
        .normalised();
        assert!(!s.start_with_windows);
        let s = Settings {
            start_with_windows: true,
            keep_ready: true,
            ..Settings::default()
        }
        .normalised();
        assert!(s.start_with_windows);
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = temp("rt");
        let store = Store::open(dir.join("settings.json"));
        let mut s = store.get();
        s.wheel = WheelMode::Navigate;
        s.background = Background::Transparent;
        store.set(s.clone()).unwrap();
        assert_eq!(Store::open(dir.join("settings.json")).get(), s);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn partial_and_corrupt_files_fall_back_safely() {
        let dir = temp("bad");
        std::fs::write(dir.join("partial.json"), br#"{"wheel":"navigate"}"#).unwrap();
        let p = load(&dir.join("partial.json"));
        assert_eq!(p.wheel, WheelMode::Navigate);
        assert!(p.autohide_controls); // missing fields take defaults
        std::fs::write(dir.join("junk.json"), b"{{{").unwrap();
        assert_eq!(load(&dir.join("junk.json")), Settings::default());
        assert_eq!(load(&dir.join("absent.json")), Settings::default());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn slideshow_timing_is_clamped() {
        let s = Settings {
            slideshow_display_ms: 1,
            slideshow_fade_ms: 99_999,
            ..Settings::default()
        }
        .normalised();
        assert_eq!((s.slideshow_display_ms, s.slideshow_fade_ms), (500, 500));
    }

    #[test]
    fn one_invalid_field_keeps_the_others() {
        let dir = temp("lenient");
        let f = dir.join("s.json");
        std::fs::write(
            &f,
            br#"{"keep_ready":true,"background":"checker","slideshow_display_ms":9000,"slideshow_fade_ms":-5,"future":1}"#,
        )
        .unwrap();
        let s = load(&f);
        assert!(s.keep_ready);
        assert_eq!(s.background, Background::Transparent);
        assert_eq!((s.slideshow_display_ms, s.slideshow_fade_ms), (9000, 1000));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unreadable_file_is_backed_up_once() {
        let dir = temp("bak");
        let f = dir.join("s.json");
        std::fs::write(&f, b"{\"keep_ready\":tru").unwrap();
        assert_eq!(load(&f), Settings::default());
        let bak = dir.join("s.json.bak");
        assert_eq!(std::fs::read(&bak).unwrap(), b"{\"keep_ready\":tru");
        std::fs::write(&f, b"").unwrap();
        load(&f);
        assert_eq!(std::fs::read(&bak).unwrap(), b"{\"keep_ready\":tru");
        let _ = std::fs::remove_dir_all(dir);
    }
}
