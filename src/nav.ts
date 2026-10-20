export type Dir = 1 | -1;

/** Index reached by moving `step` from `index`, or null when that runs off either end. */
export function stepIndex(index: number, step: number, len: number): number | null {
  const next = index + step;
  return next < 0 || next >= len ? null : next;
}

/**
 * Indices worth keeping decoded: the current one first, then neighbours ordered by
 * how soon they will be needed, biased towards the direction of travel.
 */
export function preloadWindow(index: number, len: number, dir: Dir, ahead = 4, behind = 2): number[] {
  const out = [index];
  for (let d = 1; d <= Math.max(ahead, behind); d++) {
    if (d <= ahead) out.push(index + dir * d);
    if (d <= behind) out.push(index - dir * d);
  }
  return out.filter((i) => i >= 0 && i < len);
}

/** Turns raw wheel deltas into whole navigation steps; trackpads emit many small deltas. */
export class WheelAccumulator {
  private sum = 0;
  constructor(private readonly threshold = 60) {}

  push(delta: number): number {
    this.sum += delta;
    const steps = Math.trunc(this.sum / this.threshold);
    this.sum -= steps * this.threshold;
    return steps;
  }

  reset(): void {
    this.sum = 0;
  }
}
