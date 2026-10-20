//! The main window is built in code (not tauri.conf.json) because transparency and
//! fullscreen-on-start come from user settings and are fixed at creation time.

use crate::settings::{Background, Settings};
use tauri::{window::Color, Manager, Theme, WebviewUrl, WebviewWindowBuilder};

pub fn background_color(bg: Background) -> Color {
    match bg {
        Background::Dark => Color(14, 14, 16, 255),
        Background::Black => Color(0, 0, 0, 255),
        Background::Transparent => Color(0, 0, 0, 0),
    }
}

pub fn build(app: &impl Manager<tauri::Wry>, settings: &Settings) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transparent_background_has_zero_alpha() {
        assert_eq!(background_color(Background::Transparent).3, 0);
        assert_eq!(background_color(Background::Black), Color(0, 0, 0, 255));
    }
}
