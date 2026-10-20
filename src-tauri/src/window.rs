//! The main window is built in code (not tauri.conf.json) because transparency and
//! fullscreen-on-start come from user settings and are fixed at creation time.

use crate::settings::{Background, Settings};
use tauri::{
    window::Color, LogicalSize, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

const MIN_SIZE: (f64, f64) = (480.0, 360.0);
const SCREEN_SHARE: f64 = 0.9;

pub fn background_color(bg: Background) -> Color {
    match bg {
        Background::Dark => Color(14, 14, 16, 255),
        Background::Black => Color(0, 0, 0, 255),
        Background::Transparent => Color(0, 0, 0, 0),
    }
}

pub fn build(
    app: &impl Manager<tauri::Wry>,
    settings: &Settings,
    init_script: &str,
) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .initialization_script(init_script)
        .title("Vislo")
        .inner_size(960.0, 640.0)
        .min_inner_size(360.0, 280.0)
        .center()
        .visible(false) // shown by the frontend once the first image is decoded
        .background_color(background_color(settings.background))
        .theme(Some(Theme::Dark))
        .transparent(settings.background == Background::Transparent)
        .fullscreen(settings.fullscreen_on_start)
        .build()?;
    Ok(())
}

/// Window size (logical px) for a picture of `natural` size on a monitor of `monitor` size:
/// the picture's own size, shrunk to 90% of the screen, never below the minimum.
pub fn fitted(natural: (f64, f64), monitor: (f64, f64)) -> (f64, f64) {
    let k = (monitor.0 * SCREEN_SHARE / natural.0)
        .min(monitor.1 * SCREEN_SHARE / natural.1)
        .min(1.0);
    (
        (natural.0 * k).round().max(MIN_SIZE.0),
        (natural.1 * k).round().max(MIN_SIZE.1),
    )
}

/// Sizes the hidden window around the first picture, centres it and shows it: one IPC call
/// instead of four (monitor, size, centre, show).
#[tauri::command]
pub fn show_fitted(
    window: WebviewWindow,
    width: Option<f64>,
    height: Option<f64>,
    fullscreen: bool,
) -> Result<(), String> {
    if fullscreen {
        window.set_fullscreen(true).map_err(|e| e.to_string())?;
    } else if let (Some(w), Some(h), Ok(Some(mon))) = (width, height, window.current_monitor()) {
        let scale = mon.scale_factor();
        let (w, h) = fitted(
            (w, h),
            (
                f64::from(mon.size().width) / scale,
                f64::from(mon.size().height) / scale,
            ),
        );
        window
            .set_size(LogicalSize::new(w, h))
            .map_err(|e| e.to_string())?;
        window.center().map_err(|e| e.to_string())?;
    }
    window.show().map_err(|e| e.to_string())?;
    let _ = window.set_focus(); // a revealed parked window must not open behind other apps
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_pictures_keep_their_size_but_not_below_the_minimum() {
        assert_eq!(fitted((800.0, 600.0), (2560.0, 1440.0)), (800.0, 600.0));
        assert_eq!(fitted((100.0, 50.0), (2560.0, 1440.0)), (480.0, 360.0));
    }

    #[test]
    fn big_pictures_shrink_to_ninety_percent_of_the_screen() {
        assert_eq!(fitted((6000.0, 4000.0), (2560.0, 1440.0)), (1944.0, 1296.0));
        assert_eq!(fitted((1000.0, 4000.0), (2560.0, 1440.0)), (480.0, 1296.0));
    }

    #[test]
    fn transparent_background_has_zero_alpha() {
        assert_eq!(background_color(Background::Transparent).3, 0);
        assert_eq!(background_color(Background::Black), Color(0, 0, 0, 255));
    }
}
