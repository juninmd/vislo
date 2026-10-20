# OpenPic

A fast, minimal image viewer for Windows built with [Tauri 2](https://tauri.app).
Double-click an image and it opens instantly; flip through every image in the same
folder with the wheel or arrow keys, with smooth slide animations.

Scope is deliberately small: **view on open**. No library, no editing, no import.

## Use

* Set OpenPic as the default app for images (or *Open with → OpenPic*).
* Opening another image while OpenPic is running reuses the same window.
* No file given: drop an image onto the window.

| Input | Action |
|---|---|
| Mouse wheel, pinch, `+` `-` | zoom at the cursor, from fit up to **1000%** of the image's pixels; a `NN%` tooltip shows the level for 2 s |
| `←` `→`, `PageUp` `PageDown`, `Space`, `Backspace`, on-screen arrows | previous / next image |
| `Shift` + wheel, horizontal swipe | previous / next image |
| `Home` / `End` | first / last image |
| Double-click, `1` | toggle fit ↔ actual pixels (100%) |
| `0` | fit to window |
| Drag (when zoomed) | pan |
| `T` | filmstrip |
| `F` / `F11` | fullscreen |
| `Esc` | leave fullscreen, then close |

Supported formats: JPEG, PNG, GIF, WebP, BMP, AVIF, ICO (whatever WebView2 decodes).
Files in a folder are ordered like Explorer (natural order: `img2` before `img10`).

## Why it is fast

* The window stays hidden until the first image is decoded, then opens already sized
  to the image: no blank frame, no resize flash.
* Images load straight from disk through Tauri's asset protocol — no base64 over IPC.
* Decoding is off the main thread (`img.decode()`); the next 4 and previous 2 images are
  kept decoded, biased toward the direction you are moving.
* Navigation requests are latest-wins: holding an arrow key never queues stale work.

## Develop

Requires Node 20+, Rust stable, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for Windows (WebView2, MSVC build tools).

```sh
npm install
npm test                    # frontend unit tests
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri dev           # hot-reload frontend, empty state (drop a file)
npm run tauri build         # NSIS installer with image file associations
.\src-tauri\target\release\openpic.exe "C:\path\to\image.jpg"   # run the built binary on a file
```

## Security

* The asset protocol scope starts empty and is widened at runtime to the **single
  directory** of the image you open (non-recursive).
* Strict CSP; only window size/fullscreen/title/close permissions are granted to the webview.
* Paths coming from the webview are canonicalised and must be a supported image file.
