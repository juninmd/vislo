import { bump, slide } from "./anim";
import { ImageCache } from "./cache";
import * as chrome from "./chrome";
import { Filmstrip } from "./filmstrip";
import { type Dir, preloadWindow, stepIndex, WheelAccumulator } from "./nav";
import { actualScale, type Box, clampView, FIT, fitSize, maxScale, type View, zoomAt, zoomPercent } from "./zoom";

export interface Album {
  dir: string;
  files: string[];
  index: number;
}

const IDLE_MS = 2200;
const ZOOM_MS = 110;
const ZOOM_TIP_MS = 2000; // Picasa hides its zoom readout after 2 s
/** Zoom per wheel pixel: one 100px mouse notch is about +22%. */
const WHEEL_ZOOM = 0.002;
const basename = (p: string) => p.slice(Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/")) + 1);

export class Viewer {
  private album: Album | null = null;
  private cache = new ImageCache(() => "");
  private index = 0; // shown
  private target = 0; // requested; runs ahead of `index` while a decode is pending
  private seq = 0;
  private dir: Dir = 1;
  private slideEl: HTMLElement | null = null;
  private img: HTMLImageElement | null = null;
  private view: View = FIT;
  private anims: Animation[] = [];
  private firstShow = true;
  private idleTimer = 0;
  private msgTimer = 0;
  private zoomTimer = 0;
  private wheel = new WheelAccumulator();
  private strip: Filmstrip;

  constructor(
    private readonly app: HTMLElement,
    private readonly stage: HTMLElement,
  ) {
    this.strip = new Filmstrip(this.q("#strip"), (i) => this.jump(i));
    new ResizeObserver(() => this.layout()).observe(stage);
    this.bindInput();
    this.wake();
  }

  private q<T extends HTMLElement>(sel: string): T {
    return this.app.querySelector<T>(sel)!;
  }

  // ---- album ---------------------------------------------------------------

  async load(album: Album): Promise<void> {
    this.cache.clear();
    this.album = album;
    this.cache = new ImageCache((i) => album.files[i]);
    this.index = this.target = album.index;
    this.app.classList.remove("empty");
    this.app.classList.toggle("single", album.files.length < 2);
    if (this.app.classList.contains("strip-on")) this.strip.build(album.files);
    await this.show(album.index, 1, !this.firstShow);
  }

  get hasAlbum(): boolean {
    return this.album !== null;
  }

  showEmpty(): void {
    this.app.classList.add("empty");
  }

  notify(text: string): void {
    const el = this.q("#msg");
    el.textContent = text;
    el.classList.add("show");
    clearTimeout(this.msgTimer);
    this.msgTimer = window.setTimeout(() => el.classList.remove("show"), 3500);
  }

  // ---- navigation ----------------------------------------------------------

  go(step: number): void {
    if (!this.album) return;
    const next = stepIndex(this.target, step, this.album.files.length);
    if (next === null) {
      if (this.slideEl) bump(this.slideEl, step > 0 ? -1 : 1);
      return;
    }
    this.target = next;
    void this.show(next, step > 0 ? 1 : -1, true);
  }

  private jump(index: number): void {
    if (!this.album || index === this.target) return;
    const dir: Dir = index > this.target ? 1 : -1;
    this.target = index;
    void this.show(index, dir, true);
  }

  private async show(index: number, dir: Dir, animate: boolean): Promise<void> {
    const album = this.album;
    if (!album) return;
    const seq = ++this.seq;
    this.dir = dir;
    const loaded = this.cache.get(index);
    const ok = await loaded.ready;
    if (seq !== this.seq) return; // a newer request superseded this one while decoding
    this.settle();

    const el = document.createElement("div");
    el.className = "slide";
    if (ok) {
      this.img = loaded.img;
      el.append(loaded.img);
    } else {
      this.img = null;
      el.className += " broken";
      el.textContent = `Cannot display ${basename(album.files[index])}`;
    }
    const prev = this.slideEl;
    this.stage.append(el);
    this.slideEl = el;
    this.index = index;
    this.view = FIT;
    this.q("#zoomtip").classList.remove("show");
    this.layout();
    this.updateHud(album, index);
    this.strip.setActive(index, animate);

    if (animate && prev) {
      this.anims = slide(el, prev, dir, this.stage.clientWidth);
      const [incoming] = this.anims;
      void Promise.all(this.anims.map((a) => a.finished)).then(
        () => {
          prev.remove();
          incoming.cancel();
        },
        () => prev.remove(),
      );
    } else {
      prev?.remove();
    }

    this.prefetch(album, index);
    if (this.firstShow) {
      this.firstShow = false;
      await chrome.showFitted(this.img ? this.naturalBox(this.img) : null);
    }
  }

  /** Fast-forwards a running transition so the next one starts from a clean state. */
  private settle(): void {
    for (const a of this.anims) a.finish();
    this.anims = [];
  }

  private prefetch(album: Album, index: number): void {
    const keep = preloadWindow(index, album.files.length, this.dir);
    this.cache.retain(keep);
    for (const i of keep) this.cache.prefetch(i);
  }

  private updateHud(album: Album, index: number): void {
    const name = basename(album.files[index]);
    this.q("#name").textContent = name;
    this.q("#count").textContent = `${index + 1} / ${album.files.length}`;
    void chrome.setTitle(`${name} — OpenPic`);
  }

  // ---- layout & zoom -------------------------------------------------------

  private naturalBox(img: HTMLImageElement): Box {
    const dpr = window.devicePixelRatio || 1;
    return { w: img.naturalWidth / dpr, h: img.naturalHeight / dpr };
  }

  private stageBox(): Box {
    return { w: this.stage.clientWidth, h: this.stage.clientHeight };
  }

  private fit(): Box | null {
    return this.img ? fitSize(this.naturalBox(this.img), this.stageBox()) : null;
  }

  /** Re-fits the image to the stage and re-applies the zoom transform. */
  private layout(): void {
    const fit = this.fit();
    const img = this.img;
    if (!fit || !img) return;
    img.style.width = `${fit.w}px`;
    img.style.height = `${fit.h}px`;
    this.view = clampView(this.view, fit, this.stageBox());
    this.apply();
  }

  /** `smooth` eases wheel/key zoom; panning and re-layout must follow the pointer 1:1. */
  private apply(smooth = false): void {
    const { scale, x, y } = this.view;
    if (this.img) {
      this.img.style.transition = smooth ? `transform ${ZOOM_MS}ms var(--ease)` : "none";
      this.img.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
    }
    this.slideEl?.classList.toggle("zoomed", scale > 1);
  }

  private zoomBy(factor: number, cx = 0, cy = 0): void {
    const fit = this.fit();
    if (!fit) return;
    const max = maxScale(this.naturalBox(this.img!), fit);
    this.view = zoomAt(this.view, factor, cx, cy, fit, this.stageBox(), max);
    this.apply(true);
    this.showZoom();
  }

  private showZoom(): void {
    const fit = this.fit();
    if (!fit || !this.img) return;
    const tip = this.q("#zoomtip");
    tip.textContent = `${zoomPercent(this.view, this.naturalBox(this.img), fit)}%`;
    tip.classList.add("show");
    clearTimeout(this.zoomTimer);
    this.zoomTimer = window.setTimeout(() => tip.classList.remove("show"), ZOOM_TIP_MS);
  }

  /** Cursor position relative to the stage centre, the origin zoom math works in. */
  private fromCentre(e: { clientX: number; clientY: number }): [number, number] {
    const r = this.stage.getBoundingClientRect();
    return [e.clientX - r.left - r.width / 2, e.clientY - r.top - r.height / 2];
  }

  private toggleActual(cx: number, cy: number): void {
    const fit = this.fit();
    if (!fit || !this.img) return;
    const target = actualScale(this.naturalBox(this.img), fit);
    const factor = this.view.scale > 1 ? 1 / this.view.scale : target / this.view.scale;
    this.zoomBy(factor, cx, cy);
  }

  // ---- input ---------------------------------------------------------------

  private wake(): void {
    this.app.classList.remove("idle");
    clearTimeout(this.idleTimer);
    this.idleTimer = window.setTimeout(() => this.app.classList.add("idle"), IDLE_MS);
  }

  private toggleStrip(): void {
    const on = this.app.classList.toggle("strip-on");
    if (on && this.album) {
      this.strip.build(this.album.files);
      this.strip.setActive(this.index, false);
    }
  }

  private bindInput(): void {
    for (const ev of ["mousemove", "pointerdown", "keydown"]) addEventListener(ev, () => this.wake());
    this.q("#prev").addEventListener("click", () => this.go(-1));
    this.q("#next").addEventListener("click", () => this.go(1));
    this.q("#btn-strip").addEventListener("click", () => this.toggleStrip());
    this.q("#btn-full").addEventListener("click", () => void chrome.toggleFullscreen());

    this.stage.addEventListener(
      "wheel",
      (e) => {
        e.preventDefault();
        const scale = e.deltaMode === 1 ? 40 : 1; // lines -> px
        const dx = e.deltaX * scale;
        const dy = e.deltaY * scale;
        // Shift+wheel and horizontal swipes flip through the folder; everything else zooms.
        if (e.shiftKey || Math.abs(dx) > Math.abs(dy)) {
          const steps = this.wheel.push(dx || dy);
          for (let i = 0; i < Math.min(Math.abs(steps), 3); i++) this.go(Math.sign(steps));
          return;
        }
        this.wheel.reset();
        this.zoomBy(Math.exp(-dy * WHEEL_ZOOM), ...this.fromCentre(e));
      },
      { passive: false },
    );

    this.stage.addEventListener("dblclick", (e) => this.toggleActual(...this.fromCentre(e)));
    this.bindPan();
    addEventListener("keydown", (e) => this.onKey(e));
    this.app.addEventListener("contextmenu", (e) => e.preventDefault());
  }

  private onKey(e: KeyboardEvent): void {
    if (e.altKey || e.metaKey) return;
    const k = e.key;
    const last = (this.album?.files.length ?? 1) - 1;
    if (k === "ArrowRight" || k === "PageDown" || (k === " " && !e.shiftKey)) this.go(1);
    else if (k === "ArrowLeft" || k === "PageUp" || k === "Backspace" || (k === " " && e.shiftKey)) this.go(-1);
    else if (k === "Home") this.jump(0);
    else if (k === "End") this.jump(last);
    else if (k === "+" || k === "=") this.zoomBy(1.25);
    else if (k === "-" || k === "_") this.zoomBy(0.8);
    else if (k === "0") this.zoomBy(1 / this.view.scale);
    else if (k === "1") this.toggleActual(0, 0);
    else if (k === "t" || k === "T") this.toggleStrip();
    else if (k === "f" || k === "F" || k === "F11") void chrome.toggleFullscreen();
    else if (k === "Escape") void this.escape();
    else return;
    e.preventDefault();
  }

  private async escape(): Promise<void> {
    if (await chrome.isFullscreen()) await chrome.toggleFullscreen();
    else await chrome.closeWindow();
  }

  private bindPan(): void {
    let drag: { x: number; y: number; vx: number; vy: number } | null = null;
    this.stage.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || this.view.scale <= 1) return;
      drag = { x: e.clientX, y: e.clientY, vx: this.view.x, vy: this.view.y };
      this.stage.setPointerCapture(e.pointerId);
      this.slideEl?.classList.add("panning");
    });
    this.stage.addEventListener("pointermove", (e) => {
      const fit = this.fit();
      if (!drag || !fit) return;
      this.view = clampView(
        { scale: this.view.scale, x: drag.vx + e.clientX - drag.x, y: drag.vy + e.clientY - drag.y },
        fit,
        this.stageBox(),
      );
      this.apply();
    });
    const end = () => {
      drag = null;
      this.slideEl?.classList.remove("panning");
    };
    this.stage.addEventListener("pointerup", end);
    this.stage.addEventListener("pointercancel", end);
  }
}
