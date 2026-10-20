import { fade, slide as slideAnim } from "./anim";
import { previewSrc } from "./api";
import * as chrome from "./chrome";
import type { Dir } from "./nav";
import type { Slide } from "./slide";

export type Transition = { kind: "slide" } | { kind: "fade"; ms: number } | { kind: "none" };

/** Runs one transition at a time; a new one first fast-forwards the previous so none overlap. */
export class Transitioner {
  private anims: Animation[] = [];

  settle(): void {
    for (const a of this.anims) a.finish();
    this.anims = [];
  }

  /** Puts `slide` on the stage and retires `prev` once the animation ends. */
  swap(stage: HTMLElement, slide: Slide, prev: Slide | null, t: Transition, dir: Dir): void {
    this.settle();
    stage.append(slide.el);
    if (!prev || t.kind === "none") {
      prev?.el.remove();
      return;
    }
    this.anims = t.kind === "fade" ? fade(slide.el, prev.el, t.ms) : slideAnim(slide.el, prev.el, dir, stage.clientWidth);
    const [incoming] = this.anims;
    void Promise.all(this.anims.map((a) => a.finished)).then(
      () => {
        prev.el.remove();
        incoming.cancel();
      },
      () => prev.el.remove(),
    );
  }
}

/** The EXIF preview of a photo, or null when it has none (the full decode is simply awaited). */
export async function loadPreview(path: string): Promise<HTMLImageElement | null> {
  const img = new Image();
  img.src = previewSrc(path);
  try {
    await img.decode();
    return img;
  } catch {
    return null;
  }
}

/** Slideshows go fullscreen and restore the previous state when they end. */
export function slideshowFullscreen(): (running: boolean) => void {
  let entered = false;
  return (running) => {
    void (async () => {
      if (running) {
        entered = !(await chrome.isFullscreen());
        if (entered) await chrome.setFullscreen(true);
      } else if (entered) {
        entered = false;
        await chrome.setFullscreen(false);
      }
    })();
  };
}
