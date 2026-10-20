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

export async function toggleFullscreen(): Promise<void> {
  await win.setFullscreen(!(await win.isFullscreen()));
}

export const setFullscreen = (on: boolean) => win.setFullscreen(on);

export const setTitle = (title: string) => win.setTitle(title);
export const closeWindow = () => win.close();
