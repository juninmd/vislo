mod album;
mod assoc;
mod decode;
mod exif_info;
#[cfg(test)]
mod fixtures;
mod paths;
mod pool;
mod raw;
mod resize;
mod scheme;
mod settings;
mod thumbs;
mod window;

use paths::Allowed;
use pool::Pool;
use scheme::Services;
use std::{path::Path, sync::Arc};
use tauri::{AppHandle, Emitter, Manager, State};

const THUMB_CACHE_BUDGET: u64 = 256 * 1024 * 1024;
const JOB_BACKLOG: usize = 256;

/// Path the process was launched with (file association / "Open with").
struct Launch(Option<String>);

#[tauri::command]
fn initial_path(launch: State<Launch>) -> Option<String> {
    launch.0.clone()
}

/// Lists the folder of `path`, grants the asset protocol read access to that folder only, and
/// remembers it as opened so the `vislo` protocol and `file_info` accept paths inside it.
#[tauri::command]
async fn open_path(app: AppHandle, svc: State<'_, Arc<Services>>, path: String) -> Result<album::Album, String> {
    let album = album::open(Path::new(&path))?;
    app.asset_protocol_scope().allow_directory(&album.dir, false).map_err(|e| e.to_string())?;
    svc.allowed.allow_dir(Path::new(&album.dir));
    Ok(album)
}

#[tauri::command]
async fn file_info(svc: State<'_, Arc<Services>>, path: String) -> Result<exif_info::Info, String> {
    let file = svc.allowed.resolve(&path)?;
    Ok(exif_info::info(&file))
}

#[tauri::command]
fn get_settings(store: State<settings::Store>) -> settings::Settings {
    store.get()
}

#[tauri::command]
fn set_settings(store: State<settings::Store>, settings: settings::Settings) -> Result<settings::Settings, String> {
    store.set(settings)
}

/// Registers Vislo as an image handler, then opens Settings so the user can confirm it.
#[tauri::command]
fn register_associations() -> Result<(), String> {
    assoc::register()?;
    assoc::open_settings()
}

/// `--register-associations` / `--unregister-associations`: headless, for scripts and portable use.
fn run_cli_action() -> Option<i32> {
    let flag = std::env::args().nth(1)?;
    let result = match flag.as_str() {
        "--register-associations" => assoc::register().and_then(|()| assoc::open_settings()),
        "--unregister-associations" => assoc::unregister(),
        _ => return None,
    };
    Some(if result.is_ok() { 0 } else { 1 })
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = app.path().app_cache_dir()?.join("thumbs");
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(2, 6);
    app.manage(Arc::new(Services { allowed: Allowed::default(), pool: Pool::new(workers, JOB_BACKLOG), cache_dir: cache_dir.clone() }));

    let store = settings::Store::open(app.path().app_config_dir()?.join("settings.json"));
    let current = store.get();
    app.manage(store);

    std::thread::spawn(move || {
        if let Err(e) = thumbs::evict(&cache_dir, THUMB_CACHE_BUDGET) {
            eprintln!("vislo: thumbnail cache eviction failed: {e}");
        }
    });
    window::build(app, &current)?;
    Ok(())
}

pub fn run() {
    if let Some(code) = run_cli_action() {
        std::process::exit(code);
    }
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
        .register_asynchronous_uri_scheme_protocol("vislo", |ctx, request, responder| {
            let svc = Arc::clone(ctx.app_handle().state::<Arc<Services>>().inner());
            let Some(route) = scheme::parse(request.uri().path(), request.uri().query().unwrap_or_default()) else {
                responder.respond(scheme::bad_request());
                return;
            };
            let worker = Arc::clone(&svc);
            svc.pool.spawn(move |shed| {
                let reply = if shed {
                    scheme::shed()
                } else {
                    // A decoder panic becomes a 500; a dropped responder would leave the request pending.
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scheme::serve(&worker, route))).unwrap_or_else(|_| scheme::failed())
                };
                responder.respond(reply);
            });
        })
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            initial_path,
            open_path,
            file_info,
            get_settings,
            set_settings,
            register_associations
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vislo");
}
