import { bump } from "./anim";
import type { Album, Settings, WheelMode } from "./api";
import { ImageCache, type Loaded } from "./cache";
import * as chrome from "./chrome";
import { Filmstrip } from "./filmstrip";
import { isJpeg, isNative, isStill } from "./formats";
import { HoverPreview } from "./hover";
import { InfoPanel } from "./info";
import { Hud } from "./hud";
import { bindButtons, bindInput, type Controller } from "./input";
import { type Dir, preloadWindow, stepIndex } from "./nav";
import { Sharpener } from "./sharp";
import { Slide } from "./slide";
import { Slideshow } from "./slideshow";
import { nextSlide } from "./slideshow-logic";
import { loadPreview, slideshowFullscreen, type Transition, Transitioner } from "./transitions";
import type { Box } from "./zoom";

export type { Album };

const PREVIEW_DELAY_MS = 70; // decode slower than this and the embedded preview is shown first
const basename = (p: string) => p.slice(Math.max(p.lastIndexOf("\\"), p.lastIndexOf("/")) + 1);
const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

export class Viewer implements Controller {
  readonly stage: HTMLElement;
  private album: Album | null = null;
  private cache = new ImageCache(() => "");
  private index = 0; // shown
  private target = 0; // requested; runs ahead of `index` while a decode is pending
  private seq = 0;
  private dir: Dir = 1;
  private current: Slide | null = null;
  private transitions = new Transitioner();
  private firstShow = true;
  private hud: Hud;
  private sharpener = new Sharpener();
  private strip: Filmstrip;
  private hover: HoverPreview;
  private info: InfoPanel;
  private slideshow: Slideshow;

  constructor(
    private readonly app: HTMLElement,
    private settings: Settings,
  ) {
    this.stage = this.q("#stage");
    this.hud = new Hud(app);
    this.strip = new Filmstrip(this.q("#strip"), {
      select: (i) => this.jumpTo(i),
      step: (d) => this.go(d),
      hover: (i, rect) => this.onHover(i, rect),
    });
    this.hover = new HoverPreview(this.q("#hover"));
    this.info = new InfoPanel(this.q("#info"));
    const fullscreen = slideshowFullscreen();
    this.slideshow = new Slideshow({
      displayMs: () => this.settings.slideshow_display_ms,
      fadeMs: () => this.settings.slideshow_fade_ms,
      advance: (ms) => this.advance(ms),
      changed: (running) => {
        this.app.classList.toggle("slideshow", running);
        fullscreen(running);
      },
    });
    new ResizeObserver(() => {
      this.current?.layout(this.stageBox());
      this.scheduleSharp();
    }).observe(this.stage);
    bindButtons(app, this);
    bindInput(this);
    this.applySettings(settings);
  }

  private q<T extends HTMLElement>(sel: string): T {
    return this.app.querySelector<T>(sel)!;
  }

  wheelMode = (): WheelMode => this.settings.wheel;
  slide = (): Slide | null => this.current;
  pointerDown = (): void => this.slideshow.stop();

  async load(album: Album): Promise<void> {
    this.slideshow.stop();
    this.cache.clear();
    this.adopt(album);
    await this.show(album.index, 1, this.firstShow ? { kind: "none" } : { kind: "slide" });
  }

  /** Re-lists the folder under the picture on screen (files added or removed elsewhere). */
  refresh(album: Album): void {
    this.adopt(album);
    this.updateHud(album, album.index);
    for (const i of preloadWindow(album.index, album.files.length, this.dir).slice(1)) this.cache.prefetch(i);
  }

  files = (): readonly string[] => this.album?.files ?? [];
  position = (): number => this.index;

  private adopt(album: Album): void {
    this.album = album;
    this.cache = new ImageCache((i) => album.files[i]);
    this.index = this.target = album.index;
    this.app.classList.remove("empty");
    this.app.classList.toggle("single", album.files.length < 2);
    this.strip.build(album.files, album.index);
  }

  showEmpty = (): void => this.app.classList.add("empty");
  notify = (text: string): void => this.hud.toast(text);

  applySettings(s: Settings): void {
    this.settings = s;
    for (const bg of ["dark", "black", "transparent"]) this.app.classList.toggle(`bg-${bg}`, s.background === bg);
    this.strip.set3d(s.filmstrip_3d);
    if (s.sharp_downscale) this.scheduleSharp();
    else this.current?.clearSharp();
    this.touch();
  }

  go(step: number): void {
    if (!this.album) return;
    const next = stepIndex(this.target, step, this.album.files.length);
    if (next === null) {
      if (this.current) bump(this.current.el, step > 0 ? -1 : 1);
      return;
    }
    this.slideshow.restart();
    this.target = next;
    void this.show(next, step > 0 ? 1 : -1, { kind: "slide" });
  }

  jump(where: "first" | "last"): void {
    if (this.album) this.jumpTo(where === "first" ? 0 : this.album.files.length - 1);
  }

  private jumpTo(index: number): void {
    if (!this.album || index === this.target) return;
    this.slideshow.restart();
    const dir: Dir = index > this.target ? 1 : -1;
    this.target = index;
    void this.show(index, dir, { kind: "slide" });
  }

  private advance(fadeMs: number): boolean {
    const next = this.album && nextSlide(this.target, this.album.files.length);
    if (next === null || next === undefined) return false;
    this.target = next;
    void this.show(next, 1, { kind: "fade", ms: fadeMs });
    return true;
  }

  private async show(index: number, dir: Dir, t: Transition): Promise<void> {
    const album = this.album;
    if (!album) return;
    const seq = ++this.seq;
    this.dir = dir;
    const path = album.files[index];
    const loaded = this.cache.get(index);
    const quick = await Promise.race([loaded.ready, sleep(PREVIEW_DELAY_MS).then(() => false)]);
    if (seq !== this.seq) return; // a newer request superseded this one while decoding

    const placeholder =
      !quick && this.settings.embedded_preview && (isJpeg(path) || !isNative(path))
        ? await Promise.race([loadPreview(path), loaded.ready.then(() => null)])
        : null;
    if (seq !== this.seq) return;

    const slide = new Slide();
    if (placeholder) {
      slide.setPreview(placeholder);
      this.present(slide, index, t);
      await this.reveal(slide);
      const ok = await loaded.ready;
      if (seq !== this.seq) return;
      this.fill(slide, loaded, ok, path);
      return;
    }
    this.fill(slide, loaded, quick || (await loaded.ready), path);
    if (seq !== this.seq) return;
    this.present(slide, index, t);
    await this.reveal(slide);
  }

  private fill(slide: Slide, loaded: Loaded, ok: boolean, path: string): void {
    if (ok) slide.setImage(loaded.img);
    else slide.setError(`${basename(path)} is not a valid image file`);
    slide.layout(this.stageBox());
    if (slide === this.current) this.scheduleSharp();
  }

  private present(slide: Slide, index: number, t: Transition): void {
    const album = this.album!;
    this.transitions.swap(this.stage, slide, this.current, t, this.dir);
    this.current = slide;
    this.index = index;
    this.hud.hideZoom();
    slide.layout(this.stageBox());
    this.updateHud(album, index);
    this.strip.setActive(index, t.kind !== "none");
    this.info.refresh(album.files[index]);
    const keep = preloadWindow(index, album.files.length, this.dir);
    this.cache.retain(keep);
    for (const i of keep) this.cache.prefetch(i);
    this.scheduleSharp();
  }

  /** Shows the hidden window once, sized to the first picture (known from its header if need be). */
  private async reveal(slide: Slide): Promise<void> {
    if (!this.firstShow) return;
    this.firstShow = false;
    const dpr = window.devicePixelRatio || 1;
    const size = this.album?.size;
    const natural = slide.natural() ?? (size ? { w: size[0] / dpr, h: size[1] / dpr } : null);
    await chrome.showFitted(natural, this.settings.fullscreen_on_start);
  }

  private updateHud(album: Album, index: number): void {
    const name = basename(album.files[index]);
    this.hud.setPicture(name, index, album.files.length);
    void chrome.setTitle(`${name} — Vislo`);
  }

  private stageBox(): Box {
    return { w: this.stage.clientWidth, h: this.stage.clientHeight };
  }

  zoomBy(factor: number, cx = 0, cy = 0): void {
    this.current?.zoomBy(factor, cx, cy);
    this.showZoom();
  }

  zoomFit(): void {
    if (this.current) this.zoomBy(1 / this.current.view.scale);
  }

  toggleActual(cx: number, cy: number): void {
    this.current?.toggleActual(cx, cy);
    this.showZoom();
  }

  rotate(dir: 1 | -1): void {
    this.current?.rotate(dir);
    this.showZoom();
  }

  private showZoom(): void {
    const pct = this.current?.percent();
    if (pct !== null && pct !== undefined) this.hud.showZoom(pct);
  }

  private scheduleSharp(): void {
    const slide = this.current;
    const path = this.album?.files[this.index];
    if (!slide || !path || !this.settings.sharp_downscale || !isStill(path)) return this.sharpener.cancel();
    this.sharpener.schedule(slide, path, (s) => s === this.current);
  }

  private onHover(index: number | null, rect?: DOMRect): void {
    const path = index === null ? undefined : this.album?.files[index];
    if (!this.settings.hover_preview || !path || !rect) this.hover.hide();
    else this.hover.show(path, rect);
  }

  toggleStrip(): void {
    this.app.classList.toggle("strip-on");
    this.hover.hide();
  }

  toggleInfo(): void {
    this.info.toggle(this.album?.files[this.index] ?? null);
  }

  toggleSlideshow(): void {
    this.slideshow.toggle();
  }

  toggleSettings(): void {
    this.q("#panel").classList.toggle("show");
  }

  toggleFullscreen(): void {
    void chrome.toggleFullscreen();
  }

  escape(): void {
    void (async () => {
      const panel = this.q("#panel");
      if (panel.classList.contains("show")) panel.classList.remove("show");
      else if (this.info.visible) this.info.close();
      else if (this.slideshow.running) this.slideshow.stop();
      else if (await chrome.isFullscreen()) await chrome.toggleFullscreen();
      else await chrome.closeWindow();
    })();
  }

  touch = (): void => this.hud.touch(this.settings.autohide_controls);
}
