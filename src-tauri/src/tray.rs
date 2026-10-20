//! The tray icon that keeps a parked Vislo discoverable (and quittable).

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle,
};

const ID: &str = "vislo";

/// Creates the icon the first time Vislo parks; later calls are no-ops.
pub fn ensure(app: &AppHandle) -> tauri::Result<()> {
    if app.tray_by_id(ID).is_some() {
        return Ok(());
    }
    let quit = MenuItem::with_id(app, "quit", "Quit Vislo", true, None::<&str>)?;
    let mut builder = TrayIconBuilder::with_id(ID)
        .tooltip("Vislo is ready (open an image to use it)")
        .menu(&Menu::with_items(app, &[&quit])?)
        .on_menu_event(|app, event| {
            if event.id.as_ref() == "quit" {
                app.exit(0);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app).map(|_| ())
}
