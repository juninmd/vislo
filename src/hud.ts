const IDLE_MS = 2200;
const ZOOM_TIP_MS = 2000; // the zoom readout fades after 2 s
const MSG_MS = 3500;

/** Overlay chrome: title bar text, zoom readout, toasts and the idle auto-hide. */
export class Hud {
  private timers = { idle: 0, msg: 0, zoom: 0 };

  constructor(private readonly app: HTMLElement) {}

  private q(sel: string): HTMLElement {
    return this.app.querySelector<HTMLElement>(sel)!;
  }

  setPicture(name: string, index: number, total: number): void {
    this.q("#name").textContent = name;
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
    if (autohide) this.timers.idle = window.setTimeout(() => this.app.classList.add("idle"), IDLE_MS);
  }
}
