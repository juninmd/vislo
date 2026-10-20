/**
 * Filmstrip layout. The 3D look is a four-point depth (z) curve that drives how fast cells shrink away from the centre.
 */

export const PITCH = 104; // px between neighbouring cell centres at full size
export const REACH = 6; // cells away from the centre at which the curve ends

// z of the four curve control points: far (10) -> near (1.1)
const Z = [10, 4, 0.6, 1.1] as const;
const NEAR = 1.1;
const MIN_SCALE = 0.3;

function bezierZ(t: number): number {
  const u = 1 - t;
  return u * u * u * Z[0] + 3 * u * u * t * Z[1] + 3 * u * t * t * Z[2] + t * t * t * Z[3];
}

/** Scale of a cell `a` cells (>= 0) away from the centre. */
export function depthScale(a: number): number {
  const t = Math.max(0, 1 - a / REACH);
  return Math.min(1, Math.max(MIN_SCALE, NEAR / bezierZ(t)));
}

const STEP = 0.05;
const MAX_A = 40;
// cumulative ∫ depthScale: lets cells pack tighter the further they are, without overlapping
const TABLE: number[] = (() => {
  const t = [0];
  for (let i = 1; i <= MAX_A / STEP; i++) t.push(t[i - 1] + ((depthScale((i - 1) * STEP) + depthScale(i * STEP)) / 2) * STEP);
  return t;
})();

function advance(a: number): number {
  const x = Math.min(a, MAX_A) / STEP;
  const i = Math.min(Math.floor(x), TABLE.length - 2);
  return TABLE[i] + (TABLE[i + 1] - TABLE[i]) * (x - i);
}

const smooth = (lo: number, hi: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - lo) / (hi - lo)));
  return t * t * (3 - 2 * t);
};

export interface Lane {
  x: number;
  scale: number;
  rotateY: number;
  opacity: number;
}

/** Placement of a cell at signed distance `d` (cells) from the centre, in 3D. */
export function lane3d(d: number): Lane {
  const a = Math.abs(d);
  const sign = Math.sign(d);
  return {
    x: sign * advance(a) * PITCH,
    scale: depthScale(a),
    rotateY: sign === 0 ? 0 : -sign * 52 * smooth(0, 1.4, a),
    opacity: 1 - 0.6 * smooth(0, 5, a),
  };
}

/** Flat alternative: evenly spaced, only fading with distance. */
export function lane2d(d: number): Lane {
  const a = Math.abs(d);
  return { x: d * PITCH, scale: 1, rotateY: 0, opacity: 1 - 0.5 * smooth(0, 6, a) };
}

/** Whole cells that can be on screen on each side of the centre for a strip of `width` px. */
export function visibleRadius(width: number, lane: (d: number) => Lane): number {
  let r = 1;
  while (r < 200 && Math.abs(lane(r).x) < width / 2 + PITCH) r++;
  return r;
}
