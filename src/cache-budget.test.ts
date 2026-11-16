import { describe, expect, it } from "vitest";
import { DECODED_BUDGET, withinBudget } from "./cache-budget";
import { preloadWindow } from "./nav";

const mp = (n: number) => n * 1e6 * 4; // decoded RGBA bytes of an n-megapixel picture
const peak = (order: number[], bytes: (i: number) => number) => order.reduce((sum, i) => sum + bytes(i), 0);

describe("withinBudget", () => {
  const order = preloadWindow(10, 100, 1); // the current picture, then 4 ahead and 2 behind

  it("keeps the whole window for ordinary photos", () => {
    expect(withinBudget(order, () => mp(24))).toEqual(order);
    expect(withinBudget(order, () => mp(50))).toEqual(order);
  });

  it("drops the farthest neighbours first when huge pictures would not fit", () => {
    const kept = withinBudget(order, () => mp(100));
    expect(kept).toEqual(order.slice(0, kept.length));
    expect(peak(kept, () => mp(100))).toBeLessThanOrEqual(DECODED_BUDGET);
    expect(peak(order, () => mp(100))).toBeGreaterThan(2.5 * 1024 ** 3); // what the fixed window held before
    expect(kept).toEqual([10, 11, 9, 12]);
  });

  it("always keeps the current picture, even alone over budget", () => {
    expect(withinBudget(order, () => mp(500))).toEqual([10]);
  });

  it("guesses unknown sizes from the known ones", () => {
    const sizes = new Map([[10, mp(100)]]);
    expect(withinBudget(order, (i) => sizes.get(i))).toEqual([10, 11, 9, 12]);
    expect(withinBudget(order, () => undefined)).toEqual(order); // nothing known yet: no reason to hold back
  });
});
