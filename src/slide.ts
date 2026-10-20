import {
  actualScale, type Box, clampView, effectiveBox, elementSize, FIT, fitSize, isQuarterTurn,
  maxScale, normaliseTurn, type View, zoomAt, zoomPercent,
} from "./zoom";

const ZOOM_MS = 110;
/** Re-rendering is only worth it when the browser would shrink the picture at least this much. */
const SHARP_RATIO = 1.6;

/** One picture on the stage: its element, zoom/pan/rotation state and optional sharp variant. */
export class Slide {
  readonly el = document.createElement("div");
  view: View = FIT;
  sharpKey = "";
  private img: HTMLImageElement | null = null;
  private sharp: HTMLImageElement | null = null;
  private nat: Box | null = null; // upright size in CSS px
  private rot = 0;
  private stage: Box = { w: 1, h: 1 };

  constructor() {
    this.el.className = "slide";
  }

  /** False while only a placeholder (preview or error) is on screen. */
  get ready(): boolean {
    return this.nat !== null;
  }

  /** Upright size in CSS px, once the full picture is loaded. */
  natural(): Box | null {
    return this.nat;
  }

  /** Pixel size of the picture itself (before rotation), once loaded. */
  pixels(): Box | null {
    return this.img ? { w: this.img.naturalWidth, h: this.img.naturalHeight } : null;
  }

  get zoomed(): boolean {
    return this.view.scale > 1;
  }

  setImage(img: HTMLImageElement): void {
    const dpr = window.devicePixelRatio || 1;
    this.img = img;
    this.nat = { w: img.naturalWidth / dpr, h: img.naturalHeight / dpr };
    this.el.className = "slide";
    this.el.replaceChildren(img);
    this.sharp = null;
    this.sharpKey = "";
  }

  /** Low-res stand-in shown while the full picture decodes; no zoom until it is replaced. */
  setPreview(img: HTMLImageElement): void {
    this.img = null;
    this.nat = null;
    this.el.className = "slide is-preview";
    this.el.replaceChildren(img);
  }

  setError(text: string): void {
    this.img = null;
    this.nat = null;
    this.el.className = "slide broken";
    this.el.textContent = text;
  }

  setSharp(img: HTMLImageElement, key: string): void {
    this.sharp?.remove();
    this.sharp = img;
    this.sharpKey = key;
    this.el.append(img);
    this.layout(this.stage);
  }

  clearSharp(): void {
    this.sharp?.remove();
    this.sharp = null;
    this.sharpKey = "";
  }

  private eff(): Box | null {
    return this.nat && effectiveBox(this.nat, this.rot);
  }

  /** Bounding box the picture occupies on screen at scale 1. */
  fit(): Box | null {
    const eff = this.eff();
    return eff && fitSize(eff, this.stage);
  }

  layout(stage: Box): void {
    this.stage = stage;
    const fit = this.fit();
    if (!fit) return;
    const size = elementSize(fit, this.rot);
    for (const img of [this.img, this.sharp]) {
      if (img) {
        img.style.width = `${size.w}px`;
        img.style.height = `${size.h}px`;
      }
    }
    this.view = clampView(this.view, fit, stage);
    this.apply(false);
  }

  /** `smooth` eases wheel/key zoom and rotation; panning must follow the pointer 1:1. */
  apply(smooth: boolean): void {
    const { scale, x, y } = this.view;
    const transform = `translate(${x}px, ${y}px) scale(${scale}) rotate(${this.rot}deg)`;
    const sharpOk = this.sharp !== null && scale === 1 && normaliseTurn(this.rot) === 0;
    for (const img of [this.img, this.sharp]) {
      if (!img) continue;
      img.style.transition = smooth ? `transform ${ZOOM_MS}ms var(--ease)` : "none";
      img.style.transform = transform;
    }
    if (this.sharp) this.sharp.style.visibility = sharpOk ? "visible" : "hidden";
    this.el.classList.toggle("zoomed", scale > 1);
  }

  zoomBy(factor: number, cx = 0, cy = 0): void {
    const eff = this.eff();
    const fit = this.fit();
    if (!eff || !fit) return;
    this.view = zoomAt(this.view, factor, cx, cy, fit, this.stage, maxScale(eff, fit));
    this.apply(true);
  }

  toggleActual(cx: number, cy: number): void {
    const eff = this.eff();
    const fit = this.fit();
    if (!eff || !fit) return;
    this.zoomBy(this.view.scale > 1 ? 1 / this.view.scale : actualScale(eff, fit) / this.view.scale, cx, cy);
  }

  panTo(x: number, y: number): void {
    const fit = this.fit();
    if (!fit) return;
    this.view = clampView({ scale: this.view.scale, x, y }, fit, this.stage);
    this.apply(false);
  }

  rotate(dir: 1 | -1): void {
    if (!this.nat) return;
    this.rot += 90 * dir;
    this.view = FIT;
    this.layout(this.stage);
    this.apply(true);
  }

  /** Scales where a held zoom key stops: fit and one image pixel per device pixel. */
  detents(): number[] | null {
    const eff = this.eff();
    const fit = this.fit();
    return eff && fit ? [1, actualScale(eff, fit)] : null;
  }

  /** Zoom as a share of the image's own pixels, for the tooltip. */
  percent(): number | null {
    const eff = this.eff();
    const fit = this.fit();
    return eff && fit ? zoomPercent(this.view, eff, fit) : null;
  }

  /** Device-pixel box worth re-rendering sharply, or null when the browser's own scaling is fine. */
  sharpBox(dpr: number): Box | null {
    const fit = this.fit();
    if (!fit || !this.nat || normaliseTurn(this.rot) !== 0 || isQuarterTurn(this.rot)) return null;
    if (this.nat.w / fit.w < SHARP_RATIO) return null;
    return { w: Math.round(fit.w * dpr), h: Math.round(fit.h * dpr) };
  }
}
