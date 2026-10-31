import { previewSrc, type Settings, srcFor } from "./api";
import { isJpeg, isNative } from "./formats";
import { beginHandoff } from "./handoff";

/** Injected by the backend before any script runs (see boot.rs). */
export interface Boot {
  settings: Settings;
  path: string | null;
  file: string | null;
  native: boolean;
  background: boolean;
  /** A native splash already played the opening zoom-in. */
  splash: boolean;
}

declare global {
  interface Window {
    __VISLO__?: Boot;
  }
}

export const boot: Boot | undefined = window.__VISLO__;
beginHandoff(document.documentElement, boot?.splash === true);

interface Preloaded {
  src: string;
  img: HTMLImageElement;
  ready: Promise<boolean>;
  /** True once `ready` settled, so a caller can use the picture without waiting. */
  done: boolean;
  /** Whether the decode succeeded; only meaningful once `done`. */
  ok: boolean;
}

let picture: Preloaded | null = null;
/** Stand-ins kept in flight: the launch preview plus the one the next step will need. */
const previews = new Map<string, Preloaded>();
/** Two previews are worth keeping decoded: more would cost more memory than the wait they save. */
const MAX_PREVIEWS = 2;

function start(src: string): Preloaded {
  const img = new Image();
  img.decoding = "async";
  img.draggable = false;
  img.src = src;
  const pre: Preloaded = { src, img, ready: img.decode().then(() => true, () => false), done: false, ok: false };
  void pre.ready.then((ok) => {
    pre.done = true;
    pre.ok = ok;
  });
  return pre;
}

function keep(src: string): Preloaded {
  const seen = previews.get(src);
  if (seen) return seen;
  const pre = start(src);
  previews.set(src, pre);
  for (const [key, old] of previews) {
    if (previews.size <= MAX_PREVIEWS) break;
    if (old !== pre) previews.delete(key); // oldest first: a Map keeps insertion order
  }
  return pre;
}

/** Starts fetching and decoding the launch picture (and its EXIF preview) right away, in parallel with the IPC calls. */
export function prefetchLaunchImage(): void {
  const file = boot?.file;
  if (!boot || !file) return;
  picture = start(srcFor(file));
  if (boot.settings.embedded_preview && (isJpeg(file) || !isNative(file))) keep(previewSrc(file));
}

/** Starts fetching a stand-in for `path`, so a slow picture can be shown blurred instead of blank. */
export function prefetchPreview(path: string): void {
  keep(previewSrc(path));
}

/** Hands over the prefetched picture once, if `src` is the one it was started for. */
export function takePreloaded(src: string): Preloaded | null {
  const hit = picture?.src === src ? picture : null;
  if (hit) picture = null;
  return hit;
}

/** The prefetched stand-in for `src`, or null. Awaited by callers that can wait for it. */
export function takePreloadedPreview(src: string): Preloaded | null {
  const pre = previews.get(src) ?? null;
  if (pre) previews.delete(src);
  return pre;
}

/**
 * The prefetched stand-in for `src`, but only once it is decoded: a caller that shows it can skip the
 * wait it would otherwise have to see whether the full picture arrives first.
 */
export function takeReadyPreview(src: string): HTMLImageElement | null {
  const pre = previews.get(src);
  if (!pre || !pre.done) return null;
  previews.delete(src);
  return pre.ok ? pre.img : null;
}
