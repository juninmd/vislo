/** Decoded pictures kept at once may take this many bytes (RGBA in memory). Folders of ordinary photos never
 *  reach it: seven 50 MP pictures fit. Beyond that the farthest neighbours are dropped, never the current one. */
export const DECODED_BUDGET = 1.5 * 1024 ** 3;

/**
 * Trims a preload window, ordered by priority (the current picture first), to what fits `budget`.
 * `bytesOf` is the decoded size when known; an unknown one is guessed from the known ones, since the pictures
 * of one folder are usually alike. The first index is always kept.
 */
export function withinBudget(order: number[], bytesOf: (index: number) => number | undefined, budget = DECODED_BUDGET): number[] {
  const known = order.map(bytesOf).filter((b): b is number => b !== undefined && b > 0);
  const guess = known.length ? Math.max(...known) : 0;
  const out: number[] = [];
  let used = 0;
  for (const index of order) {
    const bytes = bytesOf(index) ?? guess;
    if (out.length > 0 && used + bytes > budget) break;
    out.push(index);
    used += bytes;
  }
  return out;
}
