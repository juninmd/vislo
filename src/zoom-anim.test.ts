import { describe, expect, it } from "vitest";
import { durationMs, type Pose, poseAt } from "./zoom-anim";

const a: Pose = { x: 0, y: 0, scale: 1, rot: 0 };

describe("poseAt", () => {
  it("starts and ends exactly on the given poses", () => {
    const b: Pose = { x: -120, y: 40, scale: 8, rot: 90 };
    expect(poseAt(a, b, 0)).toEqual(a);
    const end = poseAt(a, b, 1);
    expect([end.x, end.y, end.scale, end.rot].map((n) => Math.round(n * 1e6) / 1e6)).toEqual([-120, 40, 8, 90]);
  });
  it("moves the scale geometrically, so the midpoint is the geometric mean", () => {
    expect(poseAt(a, { ...a, scale: 100 }, 0.5).scale).toBeCloseTo(10);
  });
  it("keeps the zoom anchor fixed along the way", () => {
    // zoom x4 about the point 100 px right of the centre: pan ends at 100 - 100*4 = -300
    const b: Pose = { x: -300, y: 0, scale: 4, rot: 0 };
    for (const t of [0.2, 0.5, 0.8]) {
      const p = poseAt(a, b, t);
      expect((100 - p.x) / p.scale).toBeCloseTo(100); // image-space point under the cursor
    }
  });
  it("lerps the pan when the scale does not change", () => {
    expect(poseAt(a, { ...a, x: 50 }, 0.5).x).toBeCloseTo(25);
  });
});

describe("durationMs", () => {
  it("keeps small steps snappy and caps long jumps", () => {
    expect(durationMs(a, { ...a, scale: 1.15 })).toBeLessThan(135);
    expect(durationMs({ ...a, scale: 0.01 }, { ...a, scale: 100 })).toBe(380);
  });
});
