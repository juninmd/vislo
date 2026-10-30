/**
 * Size a picture is copied at. Mirrors `copy_plan` in src-tauri/src/clipboard.rs: the same budget and
 * side cap, so the backend and the page path shrink alike. Keep the two in step.
 */
export const CLIPBOARD_BUDGET_BYTES = 256 * 1024 * 1024;
export const CLIPBOARD_MAX_SIDE = 8192;

export type CopyPlan = { readonly kind: "original" } | { readonly kind: "downscale"; readonly w: number; readonly h: number };

const ORIGINAL: CopyPlan = { kind: "original" };

export function copyPlan(width: number, height: number): CopyPlan {
  // An empty picture has nothing to shrink; the copy itself reports it.
  if (width <= 0 || height <= 0) return ORIGINAL;
  const bytes = width * height * 4;
  if (Math.max(width, height) <= CLIPBOARD_MAX_SIDE && bytes <= CLIPBOARD_BUDGET_BYTES) return ORIGINAL;
  // Area scales with the square of the factor, so the byte budget gives a square root.
  const k = Math.min(CLIPBOARD_MAX_SIDE / Math.max(width, height), Math.sqrt(CLIPBOARD_BUDGET_BYTES / bytes));
  let w = Math.min(width, Math.max(1, Math.round(width * k)));
  let h = Math.min(height, Math.max(1, Math.round(height * k)));
  // Rounding can leave one pixel too many on a side; trim until the budget holds exactly.
  while (w * h * 4 > CLIPBOARD_BUDGET_BYTES && (w > 1 || h > 1)) {
    if (w >= h) w -= 1;
    else h -= 1;
  }
  return { kind: "downscale", w, h };
}
