import { bump } from "./anim";
import type { Album, Settings, WheelMode } from "./api";
import { ImageCache, type Loaded } from "./cache";
import { type Command, Commands } from "./commands";
import { ContextMenu } from "./context-menu";
import * as chrome from "./chrome";
import { Filmstrip } from "./filmstrip";
import { isJpeg, isNative, isStill } from "./formats";
import { HoverPreview } from "./hover";
import { InfoPanel } from "./info";
import { Hud, titleFor } from "./hud";
import { bindButtons, bindInput, type Controller } from "./input";
import { menuItems } from "./menu-items";
import { type Dir, jumpIndex, preloadWindow, stepIndex } from "./nav";
import { Sharpener } from "./sharp";
import { Slide } from "./slide";
import { Slideshow } from "./slideshow";
import { nextSlide } from "./slideshow-logic";
import { loadPreview, slideshowFullscreen, type Transition, Transitioner } from "./transitions";
import { ZoomControls } from "./zoom-controls";
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
  private zoom: ZoomControls;
  private commands: Commands;
  private menu: ContextMenu;

  constructor(
    private readonly app: HTMLElement,
    private settings: Settings,
    open: (path: string) => Promise<void>,
  ) {
    this.stage = this.q("#stage");
    this.hud = new Hud(app);
    this.zoom = new ZoomControls(() => this.current, this.hud);
    this.commands = new Commands({ path: () => this.album?.files[this.index] ?? null, notify: (t) => this.notify(t) }, open);
    this.menu = new ContextMenu(this.q("#ctxmenu"));
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

  private q = <T extends HTMLElement>(sel: string): T => this.app.querySelector<T>(sel)!;

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
    this.warm(album.index);
  }

  /** Called when the window is hidden to the tray: drop the pictures, reveal afresh on the next open. */
  park(): void {
    this.seq++; // a decode still in flight must not repaint or reopen the window
    this.slideshow.stop();
    this.cache.clear();
    this.stage.replaceChildren();
    this.current = this.album = null;
    this.firstShow = true;
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
    const next = (Math.abs(step) > 1 ? jumpIndex : stepIndex)(this.target, step, this.album.files.length);
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
    const quick = await Promise.race([loaded.ready, sleep(this.firstShow ? 0 : PREVIEW_DELAY_MS).then(() => false)]);
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
    if (slide === this.current) {
      this.updateHud(this.album!, this.index); // the size is only known now
      this.scheduleSharp();
    }
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
    this.cache.retain(preloadWindow(index, album.files.length, this.dir));
    if (!this.firstShow) this.warm(index); // the very first window is shown before neighbours compete for decoders
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
    this.warm(this.index);
  }

  private warm(index: number): void {
    for (const i of preloadWindow(index, this.album?.files.length ?? 0, this.dir)) this.cache.prefetch(i);
  }

  private updateHud(album: Album, index: number): void {
    const name = basename(album.files[index]);
    const px = this.current?.pixels() ?? null;
    this.hud.setPicture(name, index, album.files.length, px);
    void chrome.setTitle(titleFor(name, px));
  }

  private stageBox(): Box {
    return { w: this.stage.clientWidth, h: this.stage.clientHeight };
  }

  zoomBy = (factor: number, cx = 0, cy = 0): void => this.zoom.zoomBy(factor, cx, cy);
  zoomStep = (dir: 1 | -1, repeat: boolean): void => this.zoom.step(dir, repeat);
  zoomFit = (): void => this.zoom.fit();
  toggleActual = (cx: number, cy: number): void => this.zoom.toggleActual(cx, cy);
  rotate = (dir: 1 | -1): void => this.zoom.rotate(dir);
  command = (command: Command): void => void this.commands.run(command);
  showMenu = (x: number, y: number): void => this.menu.show(x, y, menuItems(this, this.album !== null));

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

  toggleInfo = (): void => this.info.toggle(this.album?.files[this.index] ?? null);
  toggleSlideshow = (): void => this.slideshow.toggle();
  toggleSettings = (): void => void this.q("#panel").classList.toggle("show");
  toggleFullscreen = (): void => void chrome.toggleFullscreen();

  escape(): void {
    if (this.menu.open) return this.menu.hide();
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
