# Vislo

A fast, minimal image viewer for Windows, built with [Tauri 2](https://tauri.app).
Double-click an image and it opens instantly; flip through every image in the same folder
with the keyboard or the wheel, with smooth slide animations and a 3D filmstrip.

Scope is deliberately small: **view on open**. No library, no editing, no import.

## Features

* **Instant open** – the window stays hidden until the first picture is decoded, then appears
  already sized to it. Large JPEGs and RAW files show their embedded preview first.
* **Folder navigation** – every image next to the one you opened, in Explorer order
  (`img2` before `img10`), with slide animations and look-ahead preloading.
* **Zoom to 1000%** – wheel / pinch / `+` `-` zoom at the cursor; a `NN%` tooltip shows the level
  and fades after 2 s. Drag to pan, double-click toggles fit ↔ 100%.
* **Sharp downscaling** – photos shown much smaller than their pixels are re-sampled with
  Lanczos in Rust instead of the browser's cheaper filter.
* **3D filmstrip** (`T`) – thumbnails rendered in Rust and cached on disk, a continuously eased
  position, and an enlarged preview when you hover a thumbnail.
* **Slideshow** (`S` / `F5`) – fullscreen, 4 s per picture and a 1 s cross-fade by default, loops.
* **Rotate the view** (`R` / `Shift+R`), **info panel** (`I`) with size, dimensions, camera,
  exposure, date taken and GPS.
* **Many formats** – JPEG, PNG, GIF, WebP, BMP, AVIF, ICO, plus TIFF, PSD and the embedded
  JPEG of DNG / CR2 / NEF / ARW / ORF / RW2 RAW files. EXIF orientation is applied.
* **Default viewer** – registers itself for the image types (see below).
* **Settings panel** (gear icon) – wheel behaviour, fullscreen on start, auto-hiding controls,
  background (dark / black / transparent), sharp downscaling, instant preview, 3D filmstrip,
  hover preview and slideshow timing. Saved in the app config folder.

## Use

* **Make it the default viewer** (installed app or portable `.exe`, no admin needed): click
  *Set as default image viewer* on the empty screen or in the settings panel, or run
  `vislo.exe --register-associations`. It registers Vislo for the image types and opens
  *Settings → Default apps*, where Windows asks you to confirm (Windows does not let apps take
  over defaults silently). `vislo.exe --unregister-associations` removes it.
* Or use *Open with → Vislo* on any image.
* Opening another image while Vislo is running reuses the same window.
* No file given: drop an image onto the window.

| Input | Action |
|---|---|
| Wheel, pinch, `+` `-` | zoom at the cursor, from fit up to **1000%** of the image's pixels |
| `Shift` + wheel, horizontal swipe | previous / next image (the wheel can be switched to navigate in settings; `Ctrl` + wheel always zooms) |
| `←` `→`, `PageUp` `PageDown`, `Space`, `Backspace`, on-screen arrows | previous / next image |
| `Home` / `End` | first / last image |
| Double-click, `1` | toggle fit ↔ actual pixels (100%) |
| `0` | fit to window |
| Drag (when zoomed) | pan |
| `R` / `Shift+R` | rotate the view clockwise / counter-clockwise |
| `T` | filmstrip |
| `I` | info panel |
| `S` / `F5` | slideshow |
| `F` / `F11` | fullscreen |
| `Esc` | close panels, stop the slideshow, leave fullscreen, then close |

## Why it is fast

* Images load straight from disk through Tauri's asset protocol – no base64 over IPC.
* Decoding is off the main thread (`img.decode()`); the next 4 and previous 2 images are kept
  decoded, biased toward the direction you are moving.
* Navigation is latest-wins: holding an arrow key never queues stale work.
* Thumbnails, previews and format conversions run on a small LIFO worker pool, so the picture
  you are looking at is always served before the backlog.

## Develop

Requires Node 20+, Rust stable and the
[Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows (WebView2, MSVC build tools).

```sh
npm install
npm test                                        # frontend unit tests
cargo test --manifest-path src-tauri/Cargo.toml # backend tests
npm run tauri dev                               # hot-reload frontend, empty state (drop a file)
npm run tauri build                             # NSIS installer with image file associations
.\src-tauri\target\release\vislo.exe "C:\path\to\image.jpg"
```

Manual-test photos (EXIF, rotated, PSD, RAW) can be generated with
`VISLO_FIXTURES=<dir> cargo test --manifest-path src-tauri/Cargo.toml write_fixtures -- --ignored`.

## Security

* The asset protocol scope starts empty and is widened at runtime to the **single directory**
  of the image you open (non-recursive).
* The `vislo://` thumbnail / conversion protocol only serves image files inside an opened
  folder; every path is canonicalised first (traversal and non-image files get a 403).
* Strict CSP; only window size, fullscreen, title and close permissions are granted to the webview.
* Registry changes for file associations are per-user (`HKCU`) only.
