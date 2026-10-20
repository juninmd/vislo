mod album;

use std::path::Path;
use tauri::{AppHandle, Emitter, Manager, State};

/// Path the process was launched with (file association / "Open with").
struct Launch(Option<String>);

#[tauri::command]
fn initial_path(launch: State<Launch>) -> Option<String> {
    launch.0.clone()
}

/// Lists the folder of `path` and grants the asset protocol read access to that folder only.
#[tauri::command]
fn open_path(app: AppHandle, path: String) -> Result<album::Album, String> {
    let album = album::open(Path::new(&path))?;
    app.asset_protocol_scope()
        .allow_directory(&album.dir, false)
        .map_err(|e| e.to_string())?;
    Ok(album)
}

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch forwards its file to the running window.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(path) = album::path_from_args(argv) {
                let _ = app.emit("open-path", path);
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .manage(Launch(album::path_from_args(std::env::args())))
        .invoke_handler(tauri::generate_handler![initial_path, open_path])
        .run(tauri::generate_context!())
        .expect("error while running OpenPic");
}
