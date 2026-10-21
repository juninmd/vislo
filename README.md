<div align="center">

<img src="app-icon.svg" alt="" width="96" height="96" />

# Vislo

**A fast, minimal image viewer for Windows.**<br/>
Double-click a picture and it is on screen before you finish blinking. Flip through the folder, zoom from 1% to 10,000%, and let it keep itself up to date.

[![Release](https://img.shields.io/github/v/release/juninmd/vislo?style=for-the-badge&color=4da3ff&label=release)](https://github.com/juninmd/vislo/releases/latest)
[![Build](https://img.shields.io/github/actions/workflow/status/juninmd/vislo/release.yml?branch=main&style=for-the-badge&label=build)](https://github.com/juninmd/vislo/actions/workflows/release.yml)
[![Downloads](https://img.shields.io/github/downloads/juninmd/vislo/total?style=for-the-badge&color=2ea043)](https://github.com/juninmd/vislo/releases)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24c8db?style=for-the-badge)
![Rust](https://img.shields.io/badge/Rust-backend-dea584?style=for-the-badge)
![Conventional Commits](https://img.shields.io/badge/commits-conventional-fe5196?style=for-the-badge)

<br/>

<img src="assets/screenshots/viewer.png" alt="Vislo showing a photo maximized, with the top bar and navigation arrows" width="860" />

</div>

---

## Why Vislo

Scope is deliberately small: **view on open**. No library, no editing, no import.

* **Instant.** A native first-paint window shows the decoded picture while WebView2 is still starting
  (about 100 ms to first pixels for typical images); a resident *keep ready* mode brings reopening down to tens of milliseconds.
* **Smooth.** Eased slide animations, look-ahead preloading, a 3D filmstrip and a zoom that never jumps.
* **Everything opens.** JPEG, PNG, GIF, WebP, AVIF, BMP, TIFF, PSD, HDR, EXR, 26 camera RAW types and more.
* **Self-updating.** New releases are found, verified and installed from this repository in one click.

## Tour

<table>
<tr>
<td width="50%"><img src="assets/screenshots/filmstrip.png" alt="3D filmstrip below the picture" /><br/><b>3D filmstrip</b> (<kbd>T</kbd>) &ndash; thumbnails rendered in Rust, cached on disk, eased continuously, with an enlarged hover preview.</td>
<td width="50%"><img src="assets/screenshots/zoom.png" alt="Picture zoomed in with a percentage tooltip" /><br/><b>Zoom 1% &rarr; 10,000%</b> &ndash; wheel, pinch or <kbd>+</kbd> <kbd>-</kbd> at the cursor; crisp pixels from 400%.</td>
</tr>
<tr>
<td width="50%"><img src="assets/screenshots/info.png" alt="Information panel with camera, exposure and GPS" /><br/><b>Info panel</b> (<kbd>I</kbd>) &ndash; size, dimensions, camera, lens, exposure, date taken and GPS.</td>
<td width="50%"><img src="assets/screenshots/menu.png" alt="Right-click menu" /><br/><b>Right-click menu</b> &ndash; copy, open, show in folder, open with, slideshow and more.</td>
</tr>
<tr>
<td width="50%"><img src="assets/screenshots/update.png" alt="Settings panel and the update banner" /><br/><b>Settings &amp; updates</b> &ndash; a banner offers new versions; one click downloads, verifies and restarts.</td>
<td width="50%"><img src="assets/screenshots/empty.png" alt="Empty state with open button" /><br/><b>Drop a file</b> anywhere, press <kbd>O</kbd>, or make Vislo your default viewer.</td>
</tr>
</table>

## Install

1. Download **`Vislo_<version>_x64-setup.exe`** from the [latest release](https://github.com/juninmd/vislo/releases/latest)
   (or the `-portable.exe` if you prefer no installer) and run it. No admin rights needed.
2. Click *Set as default image viewer* on the empty screen, or in the settings panel. Windows asks you to
   confirm the choice in *Settings &rarr; Default apps* (it does not let apps take defaults silently).

Command line: `vislo.exe --register-associations [jpeg,png,raw]` registers the chosen format groups,
`vislo.exe --unregister-associations` removes them. *Open with &rarr; Vislo* works on any image.

## Automatic updates

Vislo updates itself from this repository's releases:

```
start / every 6 h ──► GET releases/latest ──► newer tag? ──► banner "Vislo 1.4.0 is available"
                                                                      │ Update & restart
   new Vislo starts ◄── silent install ◄── SHA256 matches? ◄── download Vislo_1.4.0_x64-setup.exe
```

* The check runs a few seconds after the first picture is up, so it never delays opening. Turn it off in
  *Settings &rarr; Updates*; *Check for updates* checks on demand.
* The installer is downloaded over HTTPS from this repository only and compared with the release's
  `SHA256SUMS.txt`; a mismatch discards it.
* Portable copies cannot replace themselves, so the banner offers a *Download* button for the release page.

### Every commit on `main` is a release

`.github/workflows/release.yml` turns each push to `main` into a release, with no manual steps:

1. CI runs (tests, build, clippy).
2. The next version is computed from the [semantic commits](AGENTS.md) since the last tag
   (`feat` &rarr; minor, `fix` and the rest &rarr; patch, `!` / `BREAKING CHANGE` &rarr; major).
3. The installer and the portable exe are built, `SHA256SUMS.txt` is written.
4. `chore(release): vX.Y.Z` is committed, tagged `vX.Y.Z`, and a GitHub release is published with notes grouped by type.
5. Running copies see the release and offer the update.

## Features

* **Opens maximized** &ndash; no title bar, over a dimmed transparent background; the picture zooms in as it
  appears. The top bar is the title bar: drag it to move, double-click it (or use the &#9634; button) to switch
  between maximized and a window sized to the picture. `F11` is true fullscreen (covers the taskbar) and
  returns to where it came from. *Settings &rarr; Open maximized* turns this off.
* **Instant open** &ndash; the window stays hidden until the first picture is decoded, then appears already sized
  to it. Large JPEGs and RAW files show their embedded preview first.
* **Folder navigation** &ndash; every image next to the one you opened, in Explorer order (`img2` before `img10`),
  with slide animations and look-ahead preloading.
* **Zoom from 1% to 10,000%** &ndash; wheel / pinch / `+` `-` zoom at the cursor with an eased, even animation
  (1% &rarr; 100% takes one smooth move, not a late rush); a `NN%` tooltip shows the level and fades after 2 s.
  From 400% up, pixels are drawn as crisp squares. Drag to pan (at any zoom level, also below 100%; `0`
  recentres), double-click toggles fit &harr; 100%. Honours the OS "reduce motion" setting.
* **Sharp downscaling** &ndash; photos shown much smaller than their pixels are re-sampled with Lanczos in Rust
  instead of the browser's cheaper filter.
* **Slideshow** (`S` / `F5`) &ndash; fullscreen, 4 s per picture and a 1 s cross-fade by default, loops.
* **Rotate the view** (`R` / `Shift+R`), pixel size beside the file name and in the window title.
* **Many formats** &ndash; JPEG (incl. JIF/JFI/PJPEG), PNG/APNG, GIF, WebP, BMP/DIB, AVIF, ICO, plus TIFF, TGA, PSD,
  Netpbm (PBM/PGM/PPM/PNM/PAM), Radiance HDR and OpenEXR (tone-mapped to sRGB), DDS (DXT), QOI, farbfeld, and the
  embedded JPEG of 26 camera RAW types (DNG, CR2, CR3, CRW, NEF, NRW, ARW, SRF, SR2, ORF, RW2, RAF, PEF, SRW,
  X3F, 3FR, FFF, IIQ, MEF, MOS, MRW, DCR, KDC, ERF, RWL, RAW). EXIF orientation is applied.
* **Keep ready** (settings, off by default) &ndash; closing the window parks Vislo in the tray instead of quitting,
  so the next image opens in tens of milliseconds; *Start ready when I sign in* adds a per-user Run entry
  (`vislo.exe --background`). Tray menu: *Quit Vislo*.
* **Always in sync** &ndash; files added or deleted in Explorer appear when the window regains focus; deleting the
  picture on screen opens its neighbour.
* **Robust decoding** &ndash; oversized or corrupt images are refused with a message instead of crashing the app.
* **Default viewer** &ndash; registers itself for the image types you tick.
* **Settings panel** (gear icon) &ndash; wheel behaviour, fullscreen on start, auto-hiding controls, background
  (dark / black / transparent), sharp downscaling, instant preview, 3D filmstrip, hover preview, slideshow
  timing, tray mode and updates. Saved in the app config folder.

## Keyboard & mouse

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
npm install                                     # also installs the commit-msg hook (semantic commits)
npm test                                        # frontend + release-tooling unit tests
pwsh scripts/ci-local.ps1                       # everything the CI pipeline runs
cargo test --manifest-path src-tauri/Cargo.toml # backend tests
npm run tauri dev                               # hot-reload frontend, empty state (drop a file)
npm run tauri build                             # NSIS installer with image file associations
npm run screenshots                             # regenerate assets/screenshots (headless Chromium, mocked backend)
cargo build --profile devrel --features tauri/custom-protocol --manifest-path src-tauri/Cargo.toml  # fast optimised build (no LTO) for perf work
.\src-tauri\target\release\vislo.exe "C:\path\to\image.jpg"
```

Manual-test photos (EXIF, rotated, PSD, RAW) can be generated with
`VISLO_FIXTURES=<dir> cargo test --manifest-path src-tauri/Cargo.toml write_fixtures -- --ignored`.

The screenshots above are rendered from the real frontend running against a mocked Tauri backend and
procedurally drawn photos (`scripts/screenshots/`), so they are reproducible and need no stock images.

## Contributing

Commit messages and PR titles must be [Conventional Commits](https://www.conventionalcommits.org) &ndash; the hook,
CI and the release planner all rely on it. The rules, the release rules and the code conventions are in
[AGENTS.md](AGENTS.md).

## Security

* The asset protocol scope starts empty and is widened at runtime to the **single directory**
  of the image you open (non-recursive).
* The `vislo://` thumbnail / conversion protocol only serves image files inside an opened
  folder; every path is canonicalised first (traversal and non-image files get a 403).
* Strict CSP; only window size, fullscreen, title and close permissions are granted to the webview.
* Registry changes for file associations are per-user (`HKCU`) only.
* Updates come only from this repository's GitHub releases over HTTPS, and the installer is checked
  against the release's `SHA256SUMS.txt` before it is run.
