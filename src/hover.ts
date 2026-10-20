import { thumbSrc } from "./api";

const DELAY_MS = 260;
const PREVIEW_PX = 640;

/** Enlarged preview of a filmstrip cell, shown after the pointer rests on it. */
export class HoverPreview {
  private timer = 0;
  private img = new Image();

  constructor(private readonly el: HTMLElement) {
    this.img.draggable = false;
    this.img.alt = "";
    el.append(this.img);
  }

  show(path: string, anchor: DOMRect): void {
    this.hide();
    this.timer = window.setTimeout(() => {
      this.img.src = thumbSrc(path, PREVIEW_PX);
      this.img.onload = () => {
        const w = this.el.offsetWidth;
        const left = Math.min(window.innerWidth - w - 8, Math.max(8, anchor.left + anchor.width / 2 - w / 2));
        this.el.style.left = `${left}px`;
        this.el.style.bottom = `${window.innerHeight - anchor.top + 12}px`;
        this.el.classList.add("show");
      };
    }, DELAY_MS);
  }

  hide(): void {
    clearTimeout(this.timer);
    this.img.onload = null;
    this.el.classList.remove("show");
  }
}
