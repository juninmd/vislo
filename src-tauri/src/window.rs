//! The main window is built in code (not tauri.conf.json) because transparency and
//! fullscreen-on-start come from user settings and are fixed at creation time.

use crate::settings::{Background, Settings};
use std::sync::Mutex;
use tauri::{
    window::Color, LogicalSize, Manager, State, Theme, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
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
    let maximized = frameless(settings);
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
        .maximized(maximized) // fullscreen-on-start is applied by `show_fitted`, over this state
        .decorations(!maximized)
        .build()?;
    Ok(())
}

/// Opening maximized means no title bar: the page draws its own drag area and window buttons.
pub fn frameless(settings: &Settings) -> bool {
    settings.open_maximized
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

/// Whether the window was maximized when fullscreen started, so leaving fullscreen restores it.
/// (Fullscreen on a window that is still maximized does not cover the taskbar, so the maximized
/// state is dropped first.)
#[derive(Default)]
pub struct Mode(Mutex<bool>);

fn enter_fullscreen(window: &WebviewWindow, mode: &Mode) -> Result<(), String> {
    let was = window.is_maximized().unwrap_or(false);
    *mode.0.lock().unwrap_or_else(|e| e.into_inner()) = was;
    if was {
        window.unmaximize().map_err(|e| e.to_string())?;
    }
    window.set_fullscreen(true).map_err(|e| e.to_string())
}

fn leave_fullscreen(window: &WebviewWindow, mode: &Mode) -> Result<(), String> {
    window.set_fullscreen(false).map_err(|e| e.to_string())?;
    let was = std::mem::take(&mut *mode.0.lock().unwrap_or_else(|e| e.into_inner()));
    if was {
        window.maximize().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Resizes the window around a picture of `width` x `height` CSS px and centres it.
fn size_to_picture(window: &WebviewWindow, width: f64, height: f64) -> Result<(), String> {
    let Ok(Some(mon)) = window.current_monitor() else {
        return Ok(());
    };
    let scale = mon.scale_factor();
    let monitor = (
        f64::from(mon.size().width) / scale,
        f64::from(mon.size().height) / scale,
    );
    let (w, h) = fitted((width, height), monitor);
    window
        .set_size(LogicalSize::new(w, h))
        .map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_fullscreen(window: WebviewWindow, mode: State<Mode>, on: bool) -> Result<(), String> {
    match (on, window.is_fullscreen().unwrap_or(false)) {
        (true, false) => enter_fullscreen(&window, &mode),
        (false, true) => leave_fullscreen(&window, &mode),
        _ => Ok(()),
    }
}

/// Maximized <-> windowed. Windowed sizes the window to the picture on screen. From fullscreen it
/// just returns to the state fullscreen started from. Returns whether the window is now maximized.
#[tauri::command]
pub fn toggle_maximized(
    window: WebviewWindow,
    mode: State<Mode>,
    width: Option<f64>,
    height: Option<f64>,
) -> Result<bool, String> {
    if window.is_fullscreen().unwrap_or(false) {
        leave_fullscreen(&window, &mode)?;
        return Ok(window.is_maximized().unwrap_or(false));
    }
    if window.is_maximized().unwrap_or(false) {
        window.unmaximize().map_err(|e| e.to_string())?;
        if let (Some(w), Some(h)) = (width, height) {
            size_to_picture(&window, w, h)?;
        }
        Ok(false)
    } else {
        window.maximize().map_err(|e| e.to_string())?;
        Ok(true)
    }
}

/// Sizes the hidden window around the first picture, centres it and shows it: one IPC call
/// instead of four (monitor, size, centre, show). A maximized window is left as it is.
#[tauri::command]
pub fn show_fitted(
    window: WebviewWindow,
    mode: State<Mode>,
    width: Option<f64>,
    height: Option<f64>,
    fullscreen: bool,
) -> Result<(), String> {
    if fullscreen {
        enter_fullscreen(&window, &mode)?;
    } else if window.is_maximized().unwrap_or(false) {
        // already filling the screen: nothing to size or centre
    } else if let (Some(w), Some(h)) = (width, height) {
        size_to_picture(&window, w, h)?;
    }
    crate::splash::before_show(); // let a native zoom-in finish, or cancel a splash that is not up yet
    without_open_animation(&window);
    window.show().map_err(|e| e.to_string())?;
    crate::splash::after_show();
    let _ = window.set_focus(); // a revealed parked window must not open behind other apps
    Ok(())
}

/// Windows zooms a window out of the screen centre when it is shown. Over a native splash (or a transparent
/// background) that reads as a second window opening, so the animation is off for the show itself and back on
/// right after, which keeps the system's minimize and restore animations.
#[cfg(windows)]
fn without_open_animation(window: &WebviewWindow) {
    const RESTORE_AFTER: std::time::Duration = std::time::Duration::from_millis(600);
    set_transitions_disabled(window, true);
    let window = window.clone(); // an HWND is not Send; the window handle is
    std::thread::spawn(move || {
        std::thread::sleep(RESTORE_AFTER);
        set_transitions_disabled(&window, false);
    });
}

#[cfg(windows)]
fn set_transitions_disabled(window: &WebviewWindow, disabled: bool) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
    let Ok(hwnd) = window.hwnd() else { return };
    let value = i32::from(disabled);
    unsafe {
        let _ = DwmSetWindowAttribute(
            HWND(hwnd.0),
            DWMWA_TRANSITIONS_FORCEDISABLED,
            std::ptr::from_ref(&value).cast(),
            std::mem::size_of::<i32>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn without_open_animation(_window: &WebviewWindow) {}

/// The page paints its own background dimming from now on: a native splash may fade out its own.
#[tauri::command]
pub fn splash_release() {
    crate::splash::release();
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
    fn maximized_opening_is_frameless() {
        let mut s = Settings::default();
        assert!(frameless(&s));
        s.fullscreen_on_start = true;
        assert!(
            frameless(&s),
            "fullscreen starts on top of the maximized state"
        );
        s.open_maximized = false;
        assert!(!frameless(&s));
    }

    #[test]
    fn transparent_background_has_zero_alpha() {
        assert_eq!(background_color(Background::Transparent).3, 0);
        assert_eq!(background_color(Background::Black), Color(0, 0, 0, 255));
    }
}
