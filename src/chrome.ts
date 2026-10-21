import { getCurrentWindow } from "@tauri-apps/api/window";
import * as api from "./api";
import type { Box } from "./zoom";

const win = getCurrentWindow();

/** Sizes the (still hidden) window around the first image and shows it, in a single IPC call. */
export const showFitted = (natural: Box | null, fullscreen = false): Promise<void> =>
  api.showFitted(natural?.w ?? null, natural?.h ?? null, fullscreen);

export async function isFullscreen(): Promise<boolean> {
  return win.isFullscreen();
}

// Fullscreen and maximize go through the backend: it drops the maximized state around fullscreen
// (a maximized window would not cover the taskbar) and restores it afterwards.
export async function toggleFullscreen(): Promise<void> {
  await api.setFullscreen(!(await win.isFullscreen()));
}

export const setFullscreen = (on: boolean) => api.setFullscreen(on);

export const toggleMaximized = (natural: Box | null): Promise<boolean> => api.toggleMaximized(natural?.w ?? null, natural?.h ?? null);

/** Drags the window by the top bar; a maximized window stays put. */
export async function dragWindow(): Promise<void> {
  if (!(await win.isMaximized()) && !(await win.isFullscreen())) await win.startDragging();
}

export const setTitle = (title: string) => win.setTitle(title);
export const closeWindow = () => win.close();
export const minimizeWindow = () => win.minimize();
