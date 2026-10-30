import { describe, expect, it } from "vitest";
import { CLIPBOARD_BUDGET_BYTES, CLIPBOARD_MAX_SIDE, copyPlan } from "./clipboard-plan";

describe("copyPlan", () => {
  it("leaves an empty picture alone, so the copy reports it", () => {
    expect(copyPlan(0, 0)).toEqual({ kind: "original" });
    expect(copyPlan(0, 5000)).toEqual({ kind: "original" });
  });

  it("copies a single pixel as it is", () => {
    expect(copyPlan(1, 1)).toEqual({ kind: "original" });
  });

  it("copies exactly the side cap and exactly the budget whole", () => {
    expect(copyPlan(8192, 8192)).toEqual({ kind: "original" }); // 256 MiB of RGBA exactly
    expect(copyPlan(8192, 4096)).toEqual({ kind: "original" });
    expect(copyPlan(1, 8192)).toEqual({ kind: "original" });
  });

  it("shrinks a picture one pixel over the side cap to the cap", () => {
    expect(copyPlan(8193, 100)).toEqual({ kind: "downscale", w: 8192, h: 100 });
  });

  it("brings a 150 MP picture under both limits", () => {
    const plan = copyPlan(15_000, 10_000);
    expect(plan).toEqual({ kind: "downscale", w: 8192, h: 5461 });
  });

  it("halves the largest square to the side cap", () => {
    expect(copyPlan(16_384, 16_384)).toEqual({ kind: "downscale", w: 8192, h: 8192 });
  });

  it("keeps at least one pixel per side for extreme aspect ratios", () => {
    expect(copyPlan(150_000_000, 1)).toEqual({ kind: "downscale", w: 8192, h: 1 });
    expect(copyPlan(1, 150_000_000)).toEqual({ kind: "downscale", w: 1, h: 8192 });
  });

  it("keeps every shrunk picture inside both limits", () => {
    const sizes: [number, number][] = [
      [9000, 9000],
      [12_000, 10_000],
      [16_384, 16_384],
      [20_000, 7_500],
      [3, 60_000],
      [65_535, 2_300],
    ];
    for (const [w, h] of sizes) {
      const plan = copyPlan(w, h);
      if (plan.kind !== "downscale") continue;
      expect(plan.w).toBeLessThanOrEqual(CLIPBOARD_MAX_SIDE);
      expect(plan.h).toBeLessThanOrEqual(CLIPBOARD_MAX_SIDE);
      expect(plan.w * plan.h * 4).toBeLessThanOrEqual(CLIPBOARD_BUDGET_BYTES);
      expect(Math.min(plan.w, plan.h)).toBeGreaterThanOrEqual(1);
    }
  });
});
