import { currentMonitor, getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import type { Box } from "./zoom";

const win = getCurrentWindow();
const MIN: Box = { w: 480, h: 360 };

/** Sizes the (still hidden) window around the first image so it opens without any resize flash. */
export async function showFitted(natural: Box | null, fullscreen = false): Promise<void> {
  if (fullscreen) {
    await win.setFullscreen(true);
  } else if (natural) {
    const mon = await currentMonitor();
    const sf = mon?.scaleFactor ?? 1;
    const maxW = (mon ? mon.size.width / sf : 1600) * 0.9;
    const maxH = (mon ? mon.size.height / sf : 900) * 0.9;
    const f = Math.min(1, maxW / natural.w, maxH / natural.h);
    const w = Math.max(MIN.w, Math.round(natural.w * f));
    const h = Math.max(MIN.h, Math.round(natural.h * f));
    await win.setSize(new LogicalSize(w, h));
    await win.center();
  }
  await win.show();
}

export async function isFullscreen(): Promise<boolean> {
  return win.isFullscreen();
}

export async function toggleFullscreen(): Promise<void> {
  await win.setFullscreen(!(await win.isFullscreen()));
}

export const setFullscreen = (on: boolean) => win.setFullscreen(on);

export const setTitle = (title: string) => win.setTitle(title);
export const closeWindow = () => win.close();
