import { describe, expect, it } from "vitest";
import { depthScale, lane2d, lane3d, PITCH, visibleRadius } from "./filmstrip-geometry";

describe("lane3d", () => {
  it("leaves the centre cell untouched", () => {
    expect(lane3d(0)).toEqual({ x: 0, scale: 1, rotateY: 0, opacity: 1 });
  });
  it("is mirror-symmetric around the centre", () => {
    for (const d of [0.3, 1, 2.5, 7]) {
      const l = lane3d(-d);
      const r = lane3d(d);
      expect(l.x).toBeCloseTo(-r.x);
      expect(l.scale).toBeCloseTo(r.scale);
      expect(l.rotateY).toBeCloseTo(-r.rotateY);
      expect(l.opacity).toBeCloseTo(r.opacity);
    }
  });
  it("never lets cells shrink-then-grow with distance", () => {
    let prev = depthScale(0);
    for (let a = 0.1; a < 20; a += 0.1) {
      const s = depthScale(a);
      expect(s).toBeLessThanOrEqual(prev + 1e-9);
      prev = s;
    }
    expect(depthScale(100)).toBeGreaterThanOrEqual(0.3);
  });
  it("keeps cells ordered left to right with no negative spacing", () => {
    let prev = lane3d(0).x;
    for (let d = 0.1; d < 15; d += 0.1) {
      const x = lane3d(d).x;
      expect(x).toBeGreaterThan(prev);
      prev = x;
    }
  });
  it("packs distant cells tighter than the centre pitch", () => {
    expect(lane3d(6).x - lane3d(5).x).toBeLessThan(PITCH);
  });
});

describe("lane2d", () => {
  it("is evenly spaced", () => {
    expect(lane2d(3).x).toBe(3 * PITCH);
    expect(lane2d(-2).x).toBe(-2 * PITCH);
    expect(lane2d(0).opacity).toBe(1);
  });
});

describe("visibleRadius", () => {
  it("covers the strip width plus a margin cell", () => {
    expect(visibleRadius(1000, lane2d)).toBeGreaterThanOrEqual(5);
    expect(visibleRadius(1000, lane3d)).toBeGreaterThan(visibleRadius(1000, lane2d));
  });
  it("is bounded", () => {
    expect(visibleRadius(1e9, lane2d)).toBeLessThanOrEqual(200);
  });
});
