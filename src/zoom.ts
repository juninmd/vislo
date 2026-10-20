export interface View {
  scale: number; // relative to the fitted size
  x: number; // pan offset in CSS px from the stage centre
  y: number;
}

export interface Box {
  w: number;
  h: number;
}

/** Zoom ceiling as a multiple of the image's own pixels (10 = 1000%). */
export const MAX_ZOOM = 10;
export const FIT: View = { scale: 1, x: 0, y: 0 };

/** Largest size the image takes without upscaling: natural CSS size, shrunk to fit the stage. */
export function fitSize(natural: Box, stage: Box): Box {
  const f = Math.min(1, stage.w / natural.w, stage.h / natural.h);
  return { w: natural.w * f, h: natural.h * f };
}

/** Keeps the zoomed image covering the stage; an axis smaller than the stage stays centred. */
export function clampView(view: View, fit: Box, stage: Box): View {
  const maxX = Math.max(0, (fit.w * view.scale - stage.w) / 2);
  const maxY = Math.max(0, (fit.h * view.scale - stage.h) / 2);
  return {
    scale: view.scale,
    x: Math.min(maxX, Math.max(-maxX, view.x)),
    y: Math.min(maxY, Math.max(-maxY, view.y)),
  };
}

/**
 * Scales by `factor` keeping the point under (cx, cy) — given from the stage centre — fixed.
 * `max` is the highest allowed scale relative to the fitted size (see `maxScale`).
 */
export function zoomAt(view: View, factor: number, cx: number, cy: number, fit: Box, stage: Box, max: number): View {
  const scale = Math.min(max, Math.max(1, view.scale * factor));
  const k = scale / view.scale;
  return clampView({ scale, x: cx - (cx - view.x) * k, y: cy - (cy - view.y) * k }, fit, stage);
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
