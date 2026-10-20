import type { WheelMode } from "./api";
import { WheelAccumulator } from "./nav";
import type { Slide } from "./slide";

/** Zoom per wheel pixel: one 100px mouse notch is about +22%. */
const WHEEL_ZOOM = 0.002;

export interface Controller {
  readonly stage: HTMLElement;
  wheelMode(): WheelMode;
  slide(): Slide | null;
  touch(): void;
  pointerDown(): void;
  go(step: number): void;
  jump(where: "first" | "last"): void;
  zoomBy(factor: number, cx?: number, cy?: number): void;
  zoomFit(): void;
  toggleActual(cx: number, cy: number): void;
  rotate(dir: 1 | -1): void;
  toggleStrip(): void;
  toggleInfo(): void;
  toggleSlideshow(): void;
  toggleSettings(): void;
  toggleFullscreen(): void;
  escape(): void;
}

/** Cursor position relative to the stage centre, the origin zoom math works in. */
function fromCentre(stage: HTMLElement, e: { clientX: number; clientY: number }): [number, number] {
  const r = stage.getBoundingClientRect();
  return [e.clientX - r.left - r.width / 2, e.clientY - r.top - r.height / 2];
}

/** On-screen buttons mirror the keyboard shortcuts. */
export function bindButtons(root: HTMLElement, ctl: Controller): void {
  const on = (sel: string, fn: () => void) => root.querySelector(sel)!.addEventListener("click", fn);
  on("#prev", () => ctl.go(-1));
  on("#next", () => ctl.go(1));
  on("#btn-strip", () => ctl.toggleStrip());
  on("#btn-full", () => ctl.toggleFullscreen());
  on("#btn-info", () => ctl.toggleInfo());
  on("#btn-show", () => ctl.toggleSlideshow());
  on("#btn-gear", () => ctl.toggleSettings());
  on("#btn-rot-l", () => ctl.rotate(-1));
  on("#btn-rot-r", () => ctl.rotate(1));
}

export function bindInput(ctl: Controller): void {
  const { stage } = ctl;
  const wheel = new WheelAccumulator();

  for (const ev of ["mousemove", "pointerdown", "keydown"]) addEventListener(ev, () => ctl.touch());

  stage.addEventListener(
    "wheel",
    (e) => {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? 40 : 1; // lines -> px
      const dx = e.deltaX * unit;
      const dy = e.deltaY * unit;
      const sideways = e.shiftKey || Math.abs(dx) > Math.abs(dy);
      // Ctrl+wheel is also what a trackpad pinch sends, so it always zooms.
      const zoom = e.ctrlKey || (ctl.wheelMode() === "zoom" && !sideways);
      if (zoom) {
        wheel.reset();
        ctl.zoomBy(Math.exp(-dy * WHEEL_ZOOM), ...fromCentre(stage, e));
        return;
      }
      const steps = wheel.push(sideways ? dx || dy : dy);
      for (let i = 0; i < Math.min(Math.abs(steps), 3); i++) ctl.go(Math.sign(steps));
    },
    { passive: false },
  );

  stage.addEventListener("dblclick", (e) => ctl.toggleActual(...fromCentre(stage, e)));

  let drag: { x: number; y: number; vx: number; vy: number } | null = null;
  stage.addEventListener("pointerdown", (e) => {
    ctl.pointerDown();
    const s = ctl.slide();
    if (e.button !== 0 || !s?.zoomed) return;
    drag = { x: e.clientX, y: e.clientY, vx: s.view.x, vy: s.view.y };
    stage.setPointerCapture(e.pointerId);
    s.el.classList.add("panning");
  });
  stage.addEventListener("pointermove", (e) => {
    if (drag) ctl.slide()?.panTo(drag.vx + e.clientX - drag.x, drag.vy + e.clientY - drag.y);
  });
  const end = () => {
    drag = null;
    ctl.slide()?.el.classList.remove("panning");
  };
  stage.addEventListener("pointerup", end);
  stage.addEventListener("pointercancel", end);

  addEventListener("keydown", (e) => {
    if (e.altKey || e.metaKey || e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
    const k = e.key;
    if (k === "ArrowRight" || k === "PageDown" || (k === " " && !e.shiftKey)) ctl.go(1);
    else if (k === "ArrowLeft" || k === "PageUp" || k === "Backspace" || (k === " " && e.shiftKey)) ctl.go(-1);
    else if (k === "Home") ctl.jump("first");
    else if (k === "End") ctl.jump("last");
    else if (k === "+" || k === "=") ctl.zoomBy(1.25);
    else if (k === "-" || k === "_") ctl.zoomBy(0.8);
    else if (k === "0") ctl.zoomFit();
    else if (k === "1") ctl.toggleActual(0, 0);
    else if (k === "r") ctl.rotate(1);
    else if (k === "R") ctl.rotate(-1);
    else if (k === "t" || k === "T") ctl.toggleStrip();
    else if (k === "i" || k === "I") ctl.toggleInfo();
    else if (k === "s" || k === "S" || k === "F5") ctl.toggleSlideshow();
    else if (k === "f" || k === "F" || k === "F11") ctl.toggleFullscreen();
    else if (k === "Escape") ctl.escape();
    else return;
    e.preventDefault();
  });
  stage.ownerDocument.addEventListener("contextmenu", (e) => e.preventDefault());
}
