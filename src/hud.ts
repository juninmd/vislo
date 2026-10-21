import type { Box } from "./zoom";

const IDLE_MS = 2000; // the action bar hides 2 s after the last input
const ZOOM_TIP_MS = 2000; // the zoom readout fades after 2 s
const MSG_MS = 3500;

/** Window title: `name (W × H) — Vislo`, without the size while it is still unknown. */
export const titleFor = (name: string, px: Box | null): string => `${name}${px ? ` (${px.w} × ${px.h})` : ""} — Vislo`;

/** Overlay chrome: title bar text, zoom readout, toasts and the idle auto-hide. */
export class Hud {
  private timers = { idle: 0, msg: 0, zoom: 0 };

  constructor(private readonly app: HTMLElement) {}

  private q(sel: string): HTMLElement {
    return this.app.querySelector<HTMLElement>(sel)!;
  }

  setPicture(name: string, index: number, total: number, px: Box | null): void {
    this.q("#name").textContent = name;
    this.q("#dims").textContent = px ? `${px.w} × ${px.h}` : "";
    this.q("#count").textContent = `${index + 1} / ${total}`;
    this.q("#caption").textContent = name;
  }

  toast(text: string): void {
    const el = this.q("#msg");
    el.textContent = text;
    el.classList.add("show");
    clearTimeout(this.timers.msg);
    this.timers.msg = window.setTimeout(() => el.classList.remove("show"), MSG_MS);
  }

  showZoom(percent: number): void {
    const tip = this.q("#zoomtip");
    tip.textContent = `${percent}%`;
    tip.classList.add("show");
    clearTimeout(this.timers.zoom);
    this.timers.zoom = window.setTimeout(() => tip.classList.remove("show"), ZOOM_TIP_MS);
  }

  hideZoom(): void {
    this.q("#zoomtip").classList.remove("show");
  }

  /** Any input wakes the controls; with auto-hide off they simply stay. */
  touch(autohide: boolean): void {
    this.app.classList.remove("idle");
    clearTimeout(this.timers.idle);
    if (autohide) this.armIdle();
  }

  /** The bar never vanishes from under a resting pointer: it waits until the pointer leaves. */
  private armIdle(): void {
    this.timers.idle = window.setTimeout(() => {
      if (this.q(".hud").matches(":hover")) this.armIdle();
      else this.app.classList.add("idle");
    }, IDLE_MS);
  }
}
