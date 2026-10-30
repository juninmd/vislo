mod album;
mod assoc;
mod autostart;
#[cfg(test)]
mod bench;
mod boot;
mod clipboard;
mod codecs;
mod decode;
mod exif_info;
mod metadata;
#[cfg(test)]
mod fixtures;
mod paths;
mod jpeg_scaled;
mod pool;
mod raw;
mod resize;
mod scheme;
mod settings;
mod shell;
mod splash;
mod thumbs;
mod tray;
mod update;
mod window;

use paths::Allowed;
use pool::Pool;
use scheme::Services;
use std::{path::Path, sync::Arc};
use tauri::{AppHandle, Emitter, Manager, State};

const THUMB_CACHE_BUDGET: u64 = 256 * 1024 * 1024;
const JOB_BACKLOG: usize = 256;

/// How the process was launched: the file it was given (file association / "Open with") and
/// whether it should start parked in the tray (`--background`, from the login entry).
struct Launch {
    path: Option<String>,
    background: bool,
    /// A native splash is already painting the picture (see splash.rs).
    splash: bool,
}

#[tauri::command]
fn initial_path(launch: State<Launch>) -> Option<String> {
    launch.path.clone()
}

/// Lists the folder of `path` and remembers it as opened so the `vislo` protocol and `file_info`
/// accept paths inside it.
#[tauri::command]
async fn open_path(svc: State<'_, Arc<Services>>, path: String) -> Result<album::Album, String> {
    let album = album::open(Path::new(&path))?;
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
fn set_settings(
    store: State<settings::Store>,
    settings: settings::Settings,
) -> Result<settings::Settings, String> {
    let saved = store.set(settings)?;
    autostart::apply(saved.start_with_windows)?;
    Ok(saved)
}

/// Registers Vislo as handler of the chosen format groups (none = all), then opens Settings so
/// the user can confirm it.
#[tauri::command]
fn register_associations(groups: Vec<String>) -> Result<(), String> {
    assoc::register(&groups)?;
    assoc::open_settings()
}

/// The "Image files" open dialog; `None` when cancelled.
#[tauri::command]
async fn pick_image(app: AppHandle, window: tauri::WebviewWindow) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let exts: Vec<&str> = album::extensions().collect();
    // Parented, so the viewer is blocked while the dialog is up and a second one cannot stack.
    let file = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("Image files", &exts)
        .blocking_pick_file()?;
    file.into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
async fn copy_image(svc: State<'_, Arc<Services>>, path: String) -> Result<(), String> {
    clipboard::copy_image(&svc.allowed.resolve(&path)?)
}

#[tauri::command]
async fn reveal_in_folder(svc: State<'_, Arc<Services>>, path: String) -> Result<(), String> {
    shell::reveal(&svc.allowed.resolve(&path)?)
}

#[tauri::command]
async fn open_with(svc: State<'_, Arc<Services>>, path: String) -> Result<(), String> {
    shell::open_with(&svc.allowed.resolve(&path)?)
}

/// Looks for a newer release on GitHub; `None` when this build is current.
#[tauri::command]
async fn check_update() -> Result<Option<update::Update>, String> {
    tauri::async_runtime::spawn_blocking(|| update::check(env!("CARGO_PKG_VERSION")))
        .await
        .map_err(|e| e.to_string())?
}

/// Downloads and verifies the latest installer, starts it and quits so it can replace the files.
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<(), String> {
    let setup = tauri::async_runtime::spawn_blocking(|| update::fetch_installer(env!("CARGO_PKG_VERSION")))
        .await
        .map_err(|e| e.to_string())??;
    update::run_installer(&setup)?;
    app.exit(0);
    Ok(())
}

/// Portable copies cannot replace themselves: send the user to the releases page.
#[tauri::command]
fn open_releases() -> Result<(), String> {
    shell::open_url(&format!("https://github.com/{}/releases/latest", update::REPO))
}

/// `--register-associations` / `--unregister-associations`: headless, for scripts and portable use.
fn run_cli_action() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    let flag = args.next()?;
    let result = match flag.as_str() {
        "--register-associations" => {
            // Optional comma-separated format groups, e.g. `jpeg,png,raw`.
            let groups: Vec<String> = args
                .next()
                .map(|g| g.split(',').map(str::to_string).collect())
                .unwrap_or_default();
            assoc::register(&groups).and_then(|()| assoc::open_settings())
        }
        "--unregister-associations" => assoc::unregister(),
        _ => return None,
    };
    Some(if result.is_ok() { 0 } else { 1 })
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let cache_dir = app.path().app_cache_dir()?.join("thumbs");
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .clamp(2, 6);
    app.manage(Arc::new(Services {
        allowed: Allowed::default(),
        pool: Pool::new(workers, JOB_BACKLOG),
        cache_dir: cache_dir.clone(),
    }));

    let store = settings::Store::open(app.path().app_config_dir()?.join("settings.json"));
    let current = store.get();
    app.manage(store);

    let launch = app.state::<Launch>();
    if launch.background && !current.keep_ready {
        let _ = autostart::apply(false); // drop the stale login entry that started us
        app.handle().exit(0);
        return Ok(());
    }
    let (init_script, launch_dir) = boot::script(
        &current,
        launch.path.as_deref(),
        launch.background,
        launch.splash,
    );
    if let Some(dir) = launch_dir {
        // Authorised now so the page can start loading the picture before any IPC round trip.
        app.state::<Arc<Services>>().allowed.allow_dir(&dir);
    }
    // Neither may stop the viewer from starting (policy-blocked Run key, tray not ready at sign-in).
    if let Err(e) = autostart::apply(current.start_with_windows) {
        eprintln!("vislo: login entry not updated: {e}");
    }
    if current.keep_ready {
        if let Err(e) = tray::ensure(app.handle()) {
            eprintln!("vislo: tray icon unavailable: {e}");
        }
    }

    std::thread::spawn(move || {
        if let Err(e) = thumbs::evict(&cache_dir, THUMB_CACHE_BUDGET) {
            eprintln!("vislo: thumbnail cache eviction failed: {e}");
        }
    });
    window::build(app, &current, &init_script)?;
    Ok(())
}

/// Starts the native first paint before anything slow (WebView2) is touched. Settings are read
/// straight from the config folder because Tauri's path resolver does not exist yet.
fn early_splash(launch: &Launch) -> bool {
    let Some(dir) = std::env::var_os("APPDATA") else {
        return false;
    };
    let settings = settings::load(
        &Path::new(&dir)
            .join(splash::IDENTIFIER)
            .join("settings.json"),
    );
    splash::eligible(&settings, launch.path.as_deref(), launch.background)
        .is_some_and(splash::start)
}

pub fn run() {
    if let Some(code) = run_cli_action() {
        std::process::exit(code);
    }
    let mut launch = Launch {
        path: album::path_from_args(std::env::args()),
        background: std::env::args().any(|a| a == "--background"),
        splash: false,
    };
    launch.splash = early_splash(&launch);
    tauri::Builder::default()
        // Must be first: a second launch forwards its file to the running window.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            let Some(w) = app.get_webview_window("main") else {
                return;
            };
            if w.is_visible().unwrap_or(false) {
                if let Some(path) = album::path_from_args(argv) {
                    let _ = app.emit("open-path", path);
                }
                let _ = w.unminimize();
                let _ = w.set_focus();
            } else if let Some(path) = album::path_from_args(argv) {
                // Parked: the page reveals the window once the new picture is decoded.
                let _ = app.emit("open-path", path);
            } else {
                let _ = app.emit("reveal", ()); // plain launch of a parked instance
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .manage(launch)
        .manage(window::Mode::default())
        .register_asynchronous_uri_scheme_protocol("vislo", |ctx, request, responder| {
            let svc = Arc::clone(ctx.app_handle().state::<Arc<Services>>().inner());
            let Some(route) = scheme::parse(
                request.uri().path(),
                request.uri().query().unwrap_or_default(),
            ) else {
                responder.respond(scheme::bad_request());
                return;
            };
            let worker = Arc::clone(&svc);
            svc.pool.spawn(move |shed| {
                let reply = if shed {
                    scheme::shed()
                } else {
                    // A decoder panic becomes a 500; a dropped responder would leave the request pending.
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        scheme::serve(&worker, route)
                    }))
                    .unwrap_or_else(|_| scheme::failed())
                };
                responder.respond(reply);
            });
        })
        .on_window_event(|window, event| {
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };
            let app = window.app_handle();
            if app.state::<settings::Store>().get().keep_ready && tray::ensure(app).is_ok() {
                api.prevent_close();
                let _ = window.set_fullscreen(false);
                let _ = window.hide();
                let _ = window.emit("parked", ());
            }
        })
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            initial_path,
            open_path,
            file_info,
            get_settings,
            set_settings,
            register_associations,
            pick_image,
            copy_image,
            reveal_in_folder,
            open_with,
            check_update,
            install_update,
            open_releases,
            window::show_fitted,
            window::splash_release,
            window::set_fullscreen,
            window::toggle_maximized
        ])
        .run(tauri::generate_context!())
        .expect("error while running Vislo");
}
