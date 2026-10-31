// Stand-in for @tauri-apps/api/core so the real frontend runs in a plain browser for screenshots.
import { files, FOLDER, PHOTOS, size } from "../scenes.mjs";

const scene = new URLSearchParams(location.search).get("scene") ?? "viewer";

const settings = {
  wheel: "zoom",
  fullscreen_on_start: false,
  open_maximized: true,
  autohide_controls: false,
  background: "dark",
  sharp_downscale: true,
  embedded_preview: true,
  filmstrip_3d: true,
  hover_preview: true,
  slideshow_display_ms: 4000,
  slideshow_fade_ms: 1000,
  keep_ready: false,
  start_with_windows: false,
  check_updates: true,
  language: "auto",
};

const info = {
  name: PHOTOS[0].name,
  folder: FOLDER,
  size: 4_812_345,
  modified_ms: Date.UTC(2026, 8, 14, 21, 42),
  width: size[0],
  height: size[1],
  created_ms: Date.UTC(2026, 8, 14, 21, 40),
  format: "JPEG",
  color: "RGB · 8 bit",
  rows: [
    { group: "file", key: "taken", value: "2026:09:14 17:42:00 -03:00", code: null },
    { group: "image", key: "resolution", value: "300 dpi", code: null },
    { group: "image", key: "colorSpace", value: "sRGB", code: 1 },
    { group: "camera", key: "camera", value: "Fujifilm X-T5", code: null },
    { group: "camera", key: "lens", value: "XF 16-55mm F2.8 R LM WR", code: null },
    { group: "exposure", key: "exposure", value: "1/500 s", code: null },
    { group: "exposure", key: "aperture", value: "f/8", code: null },
    { group: "exposure", key: "iso", value: "125", code: null },
    { group: "exposure", key: "focal", value: "35 mm", code: null },
    { group: "exposure", key: "program", value: "Aperture priority", code: 3 },
    { group: "authorship", key: "artist", value: "Ana Souza", code: null },
    { group: "location", key: "gps", value: "-24.73700, 15.29000", code: null },
    { group: "location", key: "altitude", value: "1240 m", code: null },
  ],
};

/** A folder longer than the viewer's preload window, so `perf:motion` can step to a picture that is cold. */
const PERF_FILES = Array.from({ length: 30 }, (_, i) => `${FOLDER}\\Photo ${String(i + 1).padStart(2, "0")}.jpg`);
const album = () => ({ dir: FOLDER, files: scene === "perf" ? PERF_FILES : files(), index: 0, size });

const handlers: Record<string, (args: any) => unknown> = {
  get_settings: () => settings,
  set_settings: (a) => a.settings,
  initial_path: () => (scene === "empty" ? null : album().files[0]),
  open_path: () => album(),
  file_info: () => info,
  check_update: () =>
    scene === "update"
      ? { version: "0.6.0", notes: "## Features\n- **viewer:** smoother zoom animation", page: "", installable: true }
      : null,
  toggle_maximized: () => true,
};

export const invoke = async (cmd: string, args: unknown = {}) => handlers[cmd]?.(args);
export const convertFileSrc = (name: string, _protocol = "vislo") => `/mock/${name}`;
