import { srcFor } from "./api";
import { copyPlan } from "./clipboard-plan";
import type { Key } from "./i18n";

/** A copy the page path could not finish; `key` is the message to show. */
export class CopyError extends Error {
  constructor(readonly key: Extract<Key, "msg.copyDenied" | "msg.copyTooLarge" | "msg.copyFailed">) {
    super(key);
  }
}

/** The WebView pieces the page path uses, injected so the logic runs without a DOM. */
export interface PageClipboard {
  decode(path: string): Promise<{ width: number; height: number; source: unknown }>;
  /** A canvas of `width` x `height`; `draw` is false when the WebView refuses that size. */
  canvas(width: number, height: number): { draw(source: unknown): boolean; png(): Promise<Blob> };
  /** Hands the clipboard a promise of the PNG. Called at once so the click's user activation is still live. */
  write(png: Promise<Blob>): Promise<void>;
}

/**
 * Copies a WebView-native picture (AVIF, SVG) by drawing the image the WebView already decoded.
 * Resolves to the size it was shrunk to, or null when it went over unchanged.
 */
export async function copyFromPage(path: string, clip: PageClipboard = webClipboard): Promise<{ w: number; h: number } | null> {
  let shrunk: { w: number; h: number } | null = null;
  const png = (async () => {
    const img = await clip.decode(path);
    if (img.width < 1 || img.height < 1) throw new CopyError("msg.copyFailed"); // e.g. an SVG without a size
    const plan = copyPlan(img.width, img.height);
    const [w, h] = plan.kind === "downscale" ? [plan.w, plan.h] : [img.width, img.height];
    if (plan.kind === "downscale") shrunk = { w, h };
    const canvas = clip.canvas(w, h);
    if (!canvas.draw(img.source)) throw new CopyError("msg.copyTooLarge");
    return canvas.png();
  })();
  png.catch(() => {}); // write() reports the failure; this only keeps a rejection nobody reads from going unhandled
  await clip.write(png);
  return shrunk;
}

/** Maps a clipboard write failure to the message the user should see. */
export function toCopyError(err: unknown): CopyError {
  if (err instanceof CopyError) return err;
  const denied = (err as { name?: unknown } | null)?.name === "NotAllowedError";
  return new CopyError(denied ? "msg.copyDenied" : "msg.copyFailed");
}

const webClipboard: PageClipboard = {
  async decode(path) {
    const img = new Image();
    // The /o route sends Access-Control-Allow-Origin, so the canvas stays readable; without it toBlob throws.
    img.crossOrigin = "anonymous";
    img.src = srcFor(path);
    try {
      await img.decode();
    } catch {
      throw new CopyError("msg.copyFailed");
    }
    return { width: img.naturalWidth, height: img.naturalHeight, source: img };
  },
  canvas(width, height) {
    const el = document.createElement("canvas");
    el.width = width;
    el.height = height;
    return {
      draw(source) {
        const ctx = el.getContext("2d");
        if (!ctx) return false;
        ctx.drawImage(source as CanvasImageSource, 0, 0, width, height);
        return true;
      },
      png: () =>
        new Promise<Blob>((resolve, reject) =>
          el.toBlob((blob) => (blob ? resolve(blob) : reject(new CopyError("msg.copyTooLarge"))), "image/png"),
        ),
    };
  },
  async write(png) {
    try {
      await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
    } catch (err) {
      throw toCopyError(err);
    }
  },
};
