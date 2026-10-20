import type { Hud } from "./hud";
import type { Slide } from "./slide";
import { stepScale } from "./zoom";

/** Zoom and rotation of the picture on screen, plus the percentage tooltip that follows them. */
export class ZoomControls {
  constructor(
    private readonly slide: () => Slide | null,
    private readonly hud: Hud,
  ) {}

  zoomBy = (factor: number, cx = 0, cy = 0): void => {
    this.slide()?.zoomBy(factor, cx, cy);
    this.showZoom();
  };

  /** One `+` / `-` step; a held key snaps to fit and 100% (see `stepScale`). */
  step = (dir: 1 | -1, repeat: boolean): void => {
    const s = this.slide();
    const stops = s?.detents();
    if (!s || !stops) return;
    this.zoomBy(stepScale(s.view.scale, dir, repeat, stops) / s.view.scale);
  };

  fit = (): void => {
    const s = this.slide();
    if (s) this.zoomBy(1 / s.view.scale);
  };

  toggleActual = (cx: number, cy: number): void => {
    this.slide()?.toggleActual(cx, cy);
    this.showZoom();
  };

  rotate = (dir: 1 | -1): void => {
    this.slide()?.rotate(dir);
    this.showZoom();
  };

  private showZoom(): void {
    const pct = this.slide()?.percent();
    if (pct !== null && pct !== undefined) this.hud.showZoom(pct);
  }
}
