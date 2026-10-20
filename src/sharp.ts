import { fitSrc } from "./api";
import type { Slide } from "./slide";

const DELAY_MS = 160;

/**
 * Re-renders a heavily downscaled picture with Lanczos in Rust (the WebView's own
 * downscaler is a cheaper filter) and swaps it in once decoded. The browser's scaling stays if anything fails.
 */
export class Sharpener {
  private timer = 0;

  cancel(): void {
    clearTimeout(this.timer);
  }

  schedule(slide: Slide, path: string, stillCurrent: (s: Slide) => boolean): void {
    this.cancel();
    this.timer = window.setTimeout(async () => {
      const dpr = window.devicePixelRatio || 1;
      const box = slide.sharpBox(dpr);
      if (!box) return;
      const key = `${box.w}x${box.h}`;
      if (slide.sharpKey === key) return;
      const img = new Image();
      img.decoding = "async";
      img.src = fitSrc(path, box.w, box.h);
      try {
        await img.decode();
      } catch {
        return;
      }
      const now = slide.sharpBox(dpr);
      if (stillCurrent(slide) && now && `${now.w}x${now.h}` === key) slide.setSharp(img, key);
    }, DELAY_MS);
  }
}
