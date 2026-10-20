//! User settings, persisted as JSON. Mirrors Picasa's viewer options (wheel behaviour,
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
    pub autohide_controls: bool,
    pub background: Background,
    pub sharp_downscale: bool,
    pub embedded_preview: bool,
    pub filmstrip_3d: bool,
    pub hover_preview: bool,
    pub slideshow_display_ms: u32,
    pub slideshow_fade_ms: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            wheel: WheelMode::Zoom,
            fullscreen_on_start: false,
            autohide_controls: true,
            background: Background::Dark,
            sharp_downscale: true,
            embedded_preview: true,
            filmstrip_3d: true,
            hover_preview: true,
            slideshow_display_ms: 4000, // Picasa's slideshow constants
            slideshow_fade_ms: 1000,
        }
    }
}

impl Settings {
    /// Clamps free-form numbers so a hand-edited file cannot create a zero-length slideshow.
    pub fn normalised(mut self) -> Self {
        self.slideshow_display_ms = self.slideshow_display_ms.clamp(500, 60_000);
        self.slideshow_fade_ms = self.slideshow_fade_ms.min(5_000).min(self.slideshow_display_ms);
        self
    }
}

/// A missing or corrupt file falls back to defaults; the bad file is left for the user to fix.
pub fn load(path: &Path) -> Settings {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<Settings>(&bytes).unwrap_or_else(|e| {
            eprintln!("vislo: ignoring unreadable settings ({e})");
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
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
        self.current.lock().unwrap_or_else(|e| e.into_inner()).clone()
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
    fn defaults_match_picasa_slideshow_timing_and_wheel_zoom() {
        let d = Settings::default();
        assert_eq!((d.slideshow_display_ms, d.slideshow_fade_ms), (4000, 1000));
        assert_eq!(d.wheel, WheelMode::Zoom);
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
        let s = Settings { slideshow_display_ms: 1, slideshow_fade_ms: 99_999, ..Settings::default() }.normalised();
        assert_eq!((s.slideshow_display_ms, s.slideshow_fade_ms), (500, 500));
    }
}
