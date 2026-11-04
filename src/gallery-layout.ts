/** Thumbnail edge range in CSS pixels; mirrors `GALLERY_SIZE` in src-tauri/src/settings.rs. Keep the two in step. */
export const GALLERY_MIN = 96;
export const GALLERY_MAX = 480;
export const GALLERY_STEP = 16;

export const clampSize = (px: number): number => Math.min(GALLERY_MAX, Math.max(GALLERY_MIN, Math.round(px)));

/**
 * Edge of the thumbnail to request: the cell in device pixels, rounded up to a multiple of 128 so that
 * dragging the slider reuses a handful of cached sizes instead of making one per pixel.
 */
export const thumbPx = (cell: number, dpr: number): number => Math.min(1024, Math.max(128, Math.ceil((cell * Math.max(1, dpr)) / 128) * 128));

export type GalleryKey = "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp" | "PageDown";

/** Index the keyboard moves the focus to, staying inside the grid. `rows` is how many rows a page covers. */
export function moveIndex(from: number, key: GalleryKey, columns: number, total: number, rows = 3): number {
  if (total <= 0) return 0;
  const last = total - 1;
  const to: Record<GalleryKey, number> = {
    ArrowLeft: from - 1,
    ArrowRight: from + 1,
    ArrowUp: from - columns,
    ArrowDown: from + columns,
    PageUp: from - columns * rows,
    PageDown: from + columns * rows,
    Home: 0,
    End: last,
  };
  const next = to[key];
  // Up/Down from the first or last row stay put instead of wrapping to somewhere surprising.
  return Math.min(last, Math.max(0, next));
}
