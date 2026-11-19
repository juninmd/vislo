import { srcFor, thumbSrc } from "./api";
import { isNative, isPageCopy } from "./formats";

/** What a thumbnail request should load first: the backend has no decoder for some native formats (AVIF, SVG), so
 * asking it would only cost a failed round trip before the webview draws the file itself. */
export const firstSrc = (path: string, size: number): string => (isPageCopy(path) ? srcFor(path) : thumbSrc(path, size));

/** What to load after a thumbnail failed: the webview can still decode a native file, but for any other format the
 * converter runs the same decoder that just failed, so retrying would only repeat the error at full size. */
export const fallbackSrc = (path: string): string | null => (isNative(path) ? srcFor(path) : null);

/**
 * Points `img` at the thumbnail of `path`. `.loaded` fades it in; a file nothing can draw marks the image `.broken`
 * so its cell shows a placeholder instead of an empty slot. `settled` runs once, after the final load or failure.
 */
export function loadThumb(img: HTMLImageElement, path: string, size: number, settled?: () => void): void {
  img.classList.remove("broken"); // `.loaded` stays: a resized cell keeps its old picture until the new one is in
  let first = firstSrc(path, size);
  img.onload = () => {
    img.classList.add("loaded");
    settled?.();
  };
  img.onerror = () => {
    const next = fallbackSrc(path);
    if (next && next !== first) {
      img.src = first = next;
      return;
    }
    img.classList.add("broken");
    settled?.();
  };
  img.src = first;
}

/**
 * Keeps the number of thumbnail requests in flight small. The backend's worker pool serves the newest request first
 * and sheds the oldest once its backlog overflows, so a gallery that asked for every cell near the viewport at once
 * (hundreds) had the cells on screen shed, then fall back to a full-size decode or stay empty. Here, cells start in
 * the order they became visible, and a cell scrolled away before its turn is dropped without costing a decode.
 */
export class ThumbQueue<K> {
  private readonly waiting = new Map<K, () => void>(); // insertion order is arrival order
  private readonly running = new Set<K>();

  constructor(private readonly limit: number) {}

  /** `start` runs now or once a slot frees up; it must lead to exactly one `done(key)`. */
  want(key: K, start: () => void): void {
    if (this.running.has(key) || this.waiting.has(key)) return;
    this.waiting.set(key, start);
    this.pump();
  }

  /** The key is no longer wanted: forget it unless it already started. */
  drop(key: K): void {
    this.waiting.delete(key);
  }

  done(key: K): void {
    if (this.running.delete(key)) this.pump();
  }

  /** Forget everything, including the requests in flight (their `done` becomes a no-op). */
  clear(): void {
    this.waiting.clear();
    this.running.clear();
  }

  get inFlight(): number {
    return this.running.size;
  }

  get pending(): number {
    return this.waiting.size;
  }

  private pump(): void {
    for (const [key, start] of this.waiting) {
      if (this.running.size >= this.limit) break;
      this.waiting.delete(key);
      this.running.add(key);
      start();
    }
  }
}
