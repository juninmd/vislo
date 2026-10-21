# Vislo

A fast, minimal image viewer for Windows, built with [Tauri 2](https://tauri.app).
Double-click an image and it opens instantly; flip through every image in the same folder
with the keyboard or the wheel, with smooth slide animations and a 3D filmstrip.

Scope is deliberately small: **view on open**. No library, no editing, no import.

## Features

* **Opens like Picasa** – maximized, no title bar, over a dimmed transparent background; the picture
  zooms in as it appears. On a cold start a native first-paint window shows the decoded picture
  while WebView2 is still starting (≈100 ms to first pixels for typical images), then hands over
  seamlessly. The top bar is the title bar: drag it to move, double-click it (or use the ▢ button)
  to switch between maximized and a window sized to the picture. `F11` is true fullscreen (covers
  the taskbar) and returns to where it came from. *Settings → Open maximized* turns this off.
* **Instant open** – the window stays hidden until the first picture is decoded, then appears
  already sized to it. Large JPEGs and RAW files show their embedded preview first.
* **Folder navigation** – every image next to the one you opened, in Explorer order
  (`img2` before `img10`), with slide animations and look-ahead preloading.
* **Zoom from 1% to 10,000%** – wheel / pinch / `+` `-` zoom at the cursor with an eased, even
  animation (1% → 100% takes one smooth move, not a late rush); a `NN%` tooltip shows the level
  and fades after 2 s. From 400% up, pixels are drawn as crisp squares. Drag to pan (at any zoom level, also below 100%; `0` recentres),
  double-click toggles fit ↔ 100%. Honours the OS "reduce motion" setting.
* **Sharp downscaling** – photos shown much smaller than their pixels are re-sampled with
  Lanczos in Rust instead of the browser's cheaper filter.
* **3D filmstrip** (`T`) – thumbnails rendered in Rust and cached on disk, a continuously eased
  position, and an enlarged preview when you hover a thumbnail.
* **Slideshow** (`S` / `F5`) – fullscreen, 4 s per picture and a 1 s cross-fade by default, loops.
* **Size in view** – the pixel size is shown beside the file name and in the window title.
* **Rotate the view** (`R` / `Shift+R`), **info panel** (`I`) with size, dimensions, camera,
  exposure, date taken and GPS.
* **Many formats** – JPEG (incl. JIF/JFI/PJPEG), PNG/APNG, GIF, WebP, BMP/DIB, AVIF, ICO, plus TIFF,
  TGA, PSD, Netpbm (PBM/PGM/PPM/PNM/PAM), Radiance HDR and OpenEXR (tone-mapped to sRGB), DDS (DXT),
  QOI, farbfeld, and the embedded JPEG of 26 camera RAW types (DNG, CR2, CR3, CRW, NEF, NRW, ARW,
  SRF, SR2, ORF, RW2, RAF, PEF, SRW, X3F, 3FR, FFF, IIQ, MEF, MOS, MRW, DCR, KDC, ERF, RWL, RAW).
  EXIF orientation is applied.
* **Keep ready** (settings, off by default) – closing the window parks Vislo in the tray instead of
  quitting, so the next image opens in tens of milliseconds; *Start ready when I sign in* adds a
  per-user Run entry (`vislo.exe --background`). Tray menu: *Quit Vislo*.
* **Always in sync** – files added or deleted in Explorer appear when the window regains focus;
  deleting the picture on screen opens its neighbour.
* **Robust decoding** – oversized or corrupt images are refused with a message instead of
  crashing the app.
* **Default viewer** – registers itself for the image types you tick (see below).
* **Settings panel** (gear icon) – wheel behaviour, fullscreen on start, auto-hiding controls,
  background (dark / black / transparent), sharp downscaling, instant preview, 3D filmstrip,
  hover preview and slideshow timing. Saved in the app config folder.

## Use

* **Make it the default viewer** (installed app or portable `.exe`, no admin needed): click
  *Set as default image viewer* on the empty screen or in the settings panel, or run
  `vislo.exe --register-associations` (optionally `--register-associations jpeg,png,raw` to pick format
  groups; the settings panel has a checkbox per group). It registers Vislo for the image types and opens
  *Settings → Default apps*, where Windows asks you to confirm (Windows does not let apps take
  over defaults silently). `vislo.exe --unregister-associations` removes it.
* Or use *Open with → Vislo* on any image.
* Opening another image while Vislo is running reuses the same window.
* No file given: drop an image onto the window.

| Input | Action |
|---|---|
| Wheel, pinch | zoom at the cursor, from **1%** to **10,000%** of the image's pixels |
| `+` `-`, `↑` `↓` | zoom in steps of 15%; a held key snaps to fit and 100% and stops there |
| `Shift` + wheel, horizontal swipe | previous / next image (the wheel can be switched to navigate in settings; `Ctrl` + wheel always zooms) |
| `←` `→`, `PageUp` `PageDown`, `Space`, `Backspace`, on-screen arrows | previous / next image |
| `Ctrl` + `←` `→` | jump 10 images |
| `J` / `K` | previous / next image |
| `Home` / `End` | first / last image |
| Double-click, `1` | toggle fit ↔ actual pixels (100%) |
| `0` | fit to window |
| Drag | pan the picture at any zoom level |
| Drag / double-click the top bar | move the window / maximize ↔ window |
| `Ctrl+C` | copy the picture to the clipboard |
| `O`, `Ctrl+O` | open an image (file dialog) |
| Right-click | menu: copy, open, show in folder, open with, info, slideshow, … |
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
pwsh scripts/ci-local.ps1                       # everything the CI pipeline runs
cargo test --manifest-path src-tauri/Cargo.toml # backend tests
npm run tauri dev                               # hot-reload frontend, empty state (drop a file)
npm run tauri build                             # NSIS installer with image file associations
cargo build --profile devrel --features tauri/custom-protocol --manifest-path src-tauri/Cargo.toml  # fast optimised build (no LTO) for perf work
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
