/** Where a picture sits on the stage: pan (CSS px), scale relative to fit, rotation (degrees). */
export interface Pose {
  x: number;
  y: number;
  scale: number;
  rot: number;
}

const EASE = "cubic-bezier(0.22, 0.8, 0.28, 1)";
const SAMPLES = 12;
const reduced = (): boolean => matchMedia("(prefers-reduced-motion: reduce)").matches;

export const poseCss = (p: Pose): string => `translate(${p.x}px, ${p.y}px) scale(${p.scale}) rotate(${p.rot}deg)`;

/**
 * Pose at progress `t` (0..1). The scale moves geometrically, so 1% -> 100% feels as even as
 * 100% -> 200%, and the pan follows the fixed point of the zoom so the spot under the cursor stays put.
 */
export function poseAt(a: Pose, b: Pose, t: number): Pose {
  const k = b.scale / a.scale;
  const p = k ** t;
  const rot = a.rot + (b.rot - a.rot) * t;
  if (Math.abs(k - 1) < 1e-6) return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, scale: a.scale, rot };
  const fx = (b.x - a.x * k) / (1 - k);
  const fy = (b.y - a.y * k) / (1 - k);
  return { x: fx + (a.x - fx) * p, y: fy + (a.y - fy) * p, scale: a.scale * p, rot };
}

/** Longer jumps take longer, within reason: a wheel notch ~120 ms, 1% -> 100% ~380 ms. */
export function durationMs(a: Pose, b: Pose): number {
  return Math.round(Math.min(380, 120 + 55 * Math.abs(Math.log(b.scale / a.scale))));
}

/** Pose an element is showing right now, read back from its (possibly animated) transform. */
function readPose(el: HTMLElement, near: Pose): Pose {
  const m = new DOMMatrix(getComputedStyle(el).transform);
  const deg = (Math.atan2(m.b, m.a) * 180) / Math.PI;
  return { x: m.e, y: m.f, scale: Math.hypot(m.a, m.b), rot: deg + 360 * Math.round((near.rot - deg) / 360) };
}

/** Eases transform changes of a group of elements, retargeting from wherever they currently are. */
export class PoseMotion {
  private running: Animation[] = [];

  /** Current pose if a motion is in flight, else null. */
  current(el: HTMLElement | null, near: Pose): Pose | null {
    return el && this.running.length ? readPose(el, near) : null;
  }

  stop(): void {
    for (const a of this.running) a.cancel();
    this.running = [];
  }

  run(els: HTMLElement[], from: Pose, to: Pose): void {
    this.stop();
    if (reduced()) return;
    const frames = Array.from({ length: SAMPLES + 1 }, (_, i) => ({ transform: poseCss(poseAt(from, to, i / SAMPLES)) }));
    this.running = els.map((el) => el.animate(frames, { duration: durationMs(from, to), easing: EASE }));
    for (const a of this.running) a.onfinish = () => (this.running = this.running.filter((r) => r !== a));
  }
}
