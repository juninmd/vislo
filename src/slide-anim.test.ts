import { describe, expect, it } from "vitest";
import { appearFrames, coverage, crossfadeFrames, incomingAlpha, outgoingAlpha, slideFrames, travel } from "./slide-anim";

/** Value of an animated property at progress `t`, interpolated the way the browser does: linearly between keyframes. */
function at(frames: Keyframe[], prop: "opacity" | "transform", t: number): number {
  const px = (v: string) => {
    const move = /translateX\((-?[\d.]+)px\)/.exec(v);
    if (move) return Number(move[1]);
    const scale = /scale\(([\d.]+)\)/.exec(v);
    return scale ? Number(scale[1]) : Number(v);
  };
  const points = frames.map((k, i) => ({
    offset: typeof k.offset === "number" ? k.offset : i / (frames.length - 1),
    value: px(String(k[prop])),
  }));
  for (let i = 1; i < points.length; i++) {
    const a = points[i - 1];
    const b = points[i];
    if (t <= b.offset) {
      const k = b.offset === a.offset ? 0 : (t - a.offset) / (b.offset - a.offset);
      return a.value + (b.value - a.value) * k;
    }
  }
  return points[points.length - 1].value;
}

// The two pictures are stacked on the stage: whatever the incoming one does not cover shows the
// outgoing one, and only what neither covers shows the background.
const samples = Array.from({ length: 41 }, (_, i) => i / 40);

describe("coverage", () => {
  it("never lets the background through during a step", () => {
    for (const t of samples) expect(coverage(t)).toBeCloseTo(1, 6);
  });

  it("matches the opacities the keyframes actually use", () => {
    const f = slideFrames(1, 1200);
    for (const t of samples) {
      expect(at(f.incomingO, "opacity", t)).toBeCloseTo(incomingAlpha(t), 6);
      expect(at(f.outgoingO, "opacity", t)).toBeCloseTo(outgoingAlpha(t), 6);
    }
  });

  it("keeps the outgoing picture whole until the incoming one is opaque", () => {
    for (const t of [0, 0.2, 0.55]) expect(outgoingAlpha(t)).toBe(1);
    expect(outgoingAlpha(0.8)).toBeLessThan(0.6);
    expect(incomingAlpha(1)).toBe(1);
  });
});

describe("slideFrames", () => {
  it("travels a fraction of the stage in the direction of the step", () => {
    expect(travel(1000)).toBe(90);
    expect(slideFrames(1, 1000).incomingX).toEqual([{ transform: "translateX(90px)" }, { transform: "translateX(0)" }]);
    expect(slideFrames(-1, 1000).incomingX[0]).toEqual({ transform: "translateX(-90px)" });
  });

  it("moves both pictures the same way, the outgoing one leaving where the incoming one came from", () => {
    const f = slideFrames(1, 1000);
    expect(f.outgoingX).toEqual([{ transform: "translateX(0)" }, { transform: "translateX(-90px)" }]);
  });

  it("starts and ends on whole pictures", () => {
    const f = slideFrames(1, 1000);
    expect(at(f.incomingO, "opacity", 0)).toBe(0);
    expect(at(f.outgoingO, "opacity", 0)).toBe(1);
    expect(at(f.incomingO, "opacity", 1)).toBe(1);
    expect(at(f.outgoingO, "opacity", 1)).toBe(0);
  });
});

describe("crossfadeFrames", () => {
  it("only raises the incoming picture, so the stage is never left half empty", () => {
    expect(crossfadeFrames()).toEqual([{ opacity: 0 }, { opacity: 1 }]);
  });
});

describe("appearFrames", () => {
  it("settles from slightly smaller without ever dimming the picture", () => {
    const frames = appearFrames();
    expect(frames.some((k) => "opacity" in k)).toBe(false);
    expect(at(frames, "transform", 0)).toBeLessThan(1);
    expect(at(frames, "transform", 0)).toBeGreaterThan(0.95); // a settle, not a zoom
    expect(at(frames, "transform", 1)).toBe(1);
  });
});
