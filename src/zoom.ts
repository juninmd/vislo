export interface View {
  scale: number; // relative to the fitted size
  x: number; // pan offset in CSS px from the stage centre
  y: number;
}

export interface Box {
  w: number;
  h: number;
}

/** Zoom range as a multiple of the image's own pixels (0.01 = 1%, 100 = 10,000%). */
export const MIN_ZOOM = 0.01;
export const MAX_ZOOM = 100;
export const FIT: View = { scale: 1, x: 0, y: 0 };

/** Largest size the image takes without upscaling: natural CSS size, shrunk to fit the stage. */
export function fitSize(natural: Box, stage: Box): Box {
  const f = Math.min(1, stage.w / natural.w, stage.h / natural.h);
  return { w: natural.w * f, h: natural.h * f };
}

/**
 * Keeps the image on the stage: a larger one keeps covering it, a smaller one can be moved
 * around but not out of it.
 */
export function clampView(view: View, fit: Box, stage: Box): View {
  const maxX = Math.abs(fit.w * view.scale - stage.w) / 2;
  const maxY = Math.abs(fit.h * view.scale - stage.h) / 2;
  return {
    scale: view.scale,
    x: Math.min(maxX, Math.max(-maxX, view.x)),
    y: Math.min(maxY, Math.max(-maxY, view.y)),
  };
}

/**
 * Scales by `factor` keeping the point under (cx, cy) — given from the stage centre — fixed.
 * `max` / `min` bound the scale relative to the fitted size (see `maxScale`, `minScale`).
 */
export function zoomAt(view: View, factor: number, cx: number, cy: number, fit: Box, stage: Box, max: number, min = 1): View {
  const scale = Math.min(max, Math.max(min, view.scale * factor));
  const k = scale / view.scale;
  return clampView({ scale, x: cx - (cx - view.x) * k, y: cy - (cy - view.y) * k }, fit, stage);
}

/** Zoom change per `+` / `-` press or button click. */
export const ZOOM_STEP = 1.15;

const near = (a: number, b: number): boolean => Math.abs(a - b) <= 1e-6 * Math.max(a, b);

/**
 * Next scale for one zoom step. A fresh press moves freely; a held (repeating) key snaps to the
 * `detents` (fit, 100%) it would cross and stops there, so the key must be released to go on.
 */
export function stepScale(scale: number, dir: 1 | -1, repeat: boolean, detents: readonly number[]): number {
  const next = scale * ZOOM_STEP ** dir;
  if (!repeat) return next;
  if (detents.some((d) => near(scale, d))) return scale;
  return detents.find((d) => (scale < d && d < next) || (next < d && d < scale) || near(next, d)) ?? next;
}

/** Scale (relative to fit) at which one image pixel is one device pixel; never below fit. */
export function actualScale(naturalCss: Box, fit: Box): number {
  return Math.max(1, naturalCss.w / fit.w);
}

/** Zoom as a whole percentage of the image's own pixels (100 = one image pixel per device pixel). */
export function zoomPercent(view: View, naturalCss: Box, fit: Box): number {
  return Math.round(((view.scale * fit.w) / naturalCss.w) * 100);
}

/** Highest scale (relative to fit) that shows the image at `MAX_ZOOM` x its pixel size. */
export function maxScale(naturalCss: Box, fit: Box): number {
  return Math.max(1, (naturalCss.w * MAX_ZOOM) / fit.w);
}

/** Lowest scale (relative to fit) that shows the image at `MIN_ZOOM` x its pixel size; never above fit. */
export function minScale(naturalCss: Box, fit: Box): number {
  return Math.min(1, (naturalCss.w * MIN_ZOOM) / fit.w);
}

/** True when `rot` (degrees) leaves the image lying on its side. */
export function isQuarterTurn(rot: number): boolean {
  return Math.abs(Math.round(rot / 90)) % 2 === 1;
}

/** Upright size as the viewer sees it after rotating by `rot`. */
export function effectiveBox(natural: Box, rot: number): Box {
  return isQuarterTurn(rot) ? { w: natural.h, h: natural.w } : natural;
}

/** CSS size of the (unrotated) element whose rotated bounding box is `fit`. */
export function elementSize(fit: Box, rot: number): Box {
  return isQuarterTurn(rot) ? { w: fit.h, h: fit.w } : fit;
}

export const normaliseTurn = (rot: number): number => ((rot % 360) + 360) % 360;
