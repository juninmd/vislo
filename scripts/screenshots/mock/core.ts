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
};

const info = {
  name: PHOTOS[0].name,
  folder: FOLDER,
  size: 4_812_345,
  modified_ms: Date.UTC(2026, 8, 14, 21, 42),
  width: size[0],
  height: size[1],
  rows: [
    ["Camera", "Fujifilm X-T5"],
    ["Lens", "XF 16-55mm F2.8 R LM WR"],
    ["Exposure", "1/500 s · f/8 · ISO 125"],
    ["Focal length", "35 mm"],
    ["Taken", "14/09/2026 17:42"],
    ["GPS", "24.7370° S, 15.2900° E"],
  ],
};

const handlers: Record<string, (args: any) => unknown> = {
  get_settings: () => settings,
  set_settings: (a) => a.settings,
  initial_path: () => (scene === "empty" ? null : files()[0]),
  open_path: () => ({ dir: FOLDER, files: files(), index: 0, size }),
  file_info: () => info,
  check_update: () =>
    scene === "update"
      ? { version: "0.4.0", notes: "## Features\n- **viewer:** smoother zoom animation", page: "", installable: true }
      : null,
  toggle_maximized: () => true,
};

export const invoke = async (cmd: string, args: unknown = {}) => handlers[cmd]?.(args);
export const convertFileSrc = (name: string, _protocol = "asset") => `/mock/${name}`;
