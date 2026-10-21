import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { isNative } from "./formats";

export interface Album {
  dir: string;
  files: string[];
  index: number;
  size: [number, number] | null;
}

export interface Info {
  name: string;
  folder: string;
  size: number;
  modified_ms: number | null;
  width: number | null;
  height: number | null;
  rows: [string, string][];
}

export type WheelMode = "zoom" | "navigate";
export type Background = "dark" | "black" | "transparent";

export interface Settings {
  wheel: WheelMode;
  fullscreen_on_start: boolean;
  open_maximized: boolean;
  autohide_controls: boolean;
  background: Background;
  sharp_downscale: boolean;
  embedded_preview: boolean;
  filmstrip_3d: boolean;
  hover_preview: boolean;
  slideshow_display_ms: number;
  slideshow_fade_ms: number;
  keep_ready: boolean;
  start_with_windows: boolean;
}

export const openPath = (path: string) => invoke<Album>("open_path", { path });
export const fileInfo = (path: string) => invoke<Info>("file_info", { path });
export const getSettings = () => invoke<Settings>("get_settings");
export const setSettings = (settings: Settings) => invoke<Settings>("set_settings", { settings });
export const initialPath = () => invoke<string | null>("initial_path");
/** Format groups offered for default-app registration; an empty selection means all of them. */
export const registerAssociations = (groups: string[] = []) => invoke<void>("register_associations", { groups });
export const showFitted = (width: number | null, height: number | null, fullscreen: boolean) =>
  invoke<void>("show_fitted", { width, height, fullscreen });
export const setFullscreen = (on: boolean) => invoke<void>("set_fullscreen", { on });
/** Maximized <-> windowed (windowed sizes the window to the picture); resolves to the new maximized state. */
export const toggleMaximized = (width: number | null, height: number | null) =>
  invoke<boolean>("toggle_maximized", { width, height });
export const copyImage = (path: string) => invoke<void>("copy_image", { path });
export const pickImage = () => invoke<string | null>("pick_image");
export const revealInFolder = (path: string) => invoke<void>("reveal_in_folder", { path });
export const openWith = (path: string) => invoke<void>("open_with", { path });

export const query = (params: Record<string, string | number>): string =>
  Object.entries(params)
    .map(([k, v]) => `${k}=${encodeURIComponent(String(v))}`)
    .join("&");

const route = (name: string, params: Record<string, string | number>) =>
  `${convertFileSrc(name, "vislo")}?${query(params)}`;

/** What the webview should load for the full picture: the file itself, or converted in Rust. */
export const srcFor = (path: string): string => route(isNative(path) ? "o" : "c", { p: path });
export const thumbSrc = (path: string, size: number) => route("t", { p: path, s: size });
export const previewSrc = (path: string) => route("v", { p: path });
export const fitSrc = (path: string, w: number, h: number) => route("f", { p: path, w, h });
