import { previewSrc, type Settings, srcFor } from "./api";
import { isJpeg, isNative } from "./formats";

/** Injected by the backend before any script runs (see boot.rs). */
export interface Boot {
  settings: Settings;
  path: string | null;
  file: string | null;
  native: boolean;
  background: boolean;
}

declare global {
  interface Window {
    __VISLO__?: Boot;
  }
}

export const boot: Boot | undefined = window.__VISLO__;

interface Preloaded {
  src: string;
  img: HTMLImageElement;
  ready: Promise<boolean>;
}

let picture: Preloaded | null = null;
let preview: Preloaded | null = null;

function start(src: string): Preloaded {
  const img = new Image();
  img.decoding = "async";
  img.draggable = false;
  img.src = src;
  return { src, img, ready: img.decode().then(() => true, () => false) };
}

/** Starts fetching and decoding the launch picture (and its EXIF preview) right away, in parallel with the IPC calls. */
export function prefetchLaunchImage(): void {
  const file = boot?.file;
  if (!boot || !file) return;
  picture = start(srcFor(file));
  if (boot.settings.embedded_preview && (isJpeg(file) || !isNative(file))) preview = start(previewSrc(file));
}

/** Hands over the prefetched picture once, if `src` is the one it was started for. */
export function takePreloaded(src: string): Preloaded | null {
  const hit = picture?.src === src ? picture : null;
  if (hit) picture = null;
  return hit;
}

export function takePreloadedPreview(src: string): Preloaded | null {
  const hit = preview?.src === src ? preview : null;
  if (hit) preview = null;
  return hit;
}
