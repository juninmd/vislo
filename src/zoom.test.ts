import { describe, expect, it } from "vitest";
import { actualScale, clampView, effectiveBox, elementSize, fitSize, maxScale, minScale, normaliseTurn, stepScale, ZOOM_STEP, zoomAt, zoomPercent } from "./zoom";

const stage = { w: 1000, h: 600 };

describe("fitSize", () => {
  it("shrinks big images to the stage keeping aspect", () => {
    expect(fitSize({ w: 4000, h: 3000 }, stage)).toEqual({ w: 800, h: 600 });
  });
  it("never upscales small images", () => {
    expect(fitSize({ w: 200, h: 100 }, stage)).toEqual({ w: 200, h: 100 });
  });
});

describe("zoomAt", () => {
  const fit = { w: 800, h: 600 };
  it("keeps the point under the cursor fixed", () => {
    const v = zoomAt({ scale: 1, x: 0, y: 0 }, 2, 100, 50, fit, stage, 24);
    // the image-space point under the cursor is (cursor - pan) / scale, and must not move
    const before = (100 - 0) / 1;
    const after = (100 - v.x) / v.scale;
    expect(after).toBeCloseTo(before);
  });
  it("cannot zoom out below the fitted size unless a lower minimum is given", () => {
    expect(zoomAt({ scale: 1, x: 0, y: 0 }, 0.5, 0, 0, fit, stage, 24).scale).toBe(1);
    expect(zoomAt({ scale: 1, x: 0, y: 0 }, 0.5, 0, 0, fit, stage, 24, 0.05).scale).toBe(0.5);
    expect(zoomAt({ scale: 0.06, x: 0, y: 0 }, 0.1, 0, 0, fit, stage, 24, 0.05).scale).toBe(0.05);
  });
  it("stays centred while smaller than the stage", () => {
    const v = zoomAt({ scale: 1, x: 40, y: 40 }, 0.1, 300, 200, fit, stage, 24, 0.05);
    expect([v.x, v.y]).toEqual([0, 0]);
  });
  it("caps at the given maximum scale", () => {
    expect(zoomAt({ scale: 20, x: 0, y: 0 }, 10, 0, 0, fit, stage, 24).scale).toBe(24);
  });
});

describe("clampView", () => {
  const fit = { w: 800, h: 600 };
  it("centres an axis that is smaller than the stage", () => {
    expect(clampView({ scale: 1, x: 300, y: 300 }, fit, stage)).toEqual({ scale: 1, x: 0, y: 0 });
  });
  it("limits panning to the overflow", () => {
    // 800*2=1600 wide vs 1000 stage -> max pan 300; 600*2=1200 tall vs 600 -> 300
    expect(clampView({ scale: 2, x: 999, y: -999 }, fit, stage)).toEqual({ scale: 2, x: 300, y: -300 });
  });
});

describe("actualScale", () => {
  it("is 1:1 device pixels when natural CSS size is 4000 over an 800 fit", () => {
    expect(actualScale({ w: 4000, h: 3000 }, { w: 800, h: 600 })).toBe(5);
  });
  it("stays at fit when the image is already smaller than 100%", () => {
    expect(actualScale({ w: 200, h: 100 }, { w: 200, h: 100 })).toBe(1);
  });
});

describe("zoomPercent", () => {
  const natural = { w: 4000, h: 3000 };
  const fit = { w: 800, h: 600 };
  it("reports the fitted size as a share of the image pixels", () => {
    expect(zoomPercent({ scale: 1, x: 0, y: 0 }, natural, fit)).toBe(20);
  });
  it("reads 100% at actual size and 10,000% at the ceiling", () => {
    expect(zoomPercent({ scale: actualScale(natural, fit), x: 0, y: 0 }, natural, fit)).toBe(100);
    expect(zoomPercent({ scale: maxScale(natural, fit), x: 0, y: 0 }, natural, fit)).toBe(10000);
  });
  it("is relative to device pixels on a HiDPI display (natural given in CSS px)", () => {
    // 4000px image at dpr 2 -> 2000 CSS px; fitted to 800 CSS px = 40 device-px share of 4000
    expect(zoomPercent({ scale: 1, x: 0, y: 0 }, { w: 2000, h: 1500 }, fit)).toBe(40);
  });
});

describe("maxScale (10,000% of image pixels)", () => {
  it("is 100x actual size for a large image", () => {
    expect(maxScale({ w: 4000, h: 3000 }, { w: 800, h: 600 })).toBe(500);
  });
  it("is 100 for an image that already fits at 100%", () => {
    expect(maxScale({ w: 200, h: 100 }, { w: 200, h: 100 })).toBe(100);
  });
  it("lets a HiDPI display reach 10,000% of image pixels (CSS size = px / dpr)", () => {
    // 4000px image on a 2x display: natural CSS width 2000, fit 800 -> 100 * 2000 / 800
    expect(maxScale({ w: 2000, h: 1500 }, { w: 800, h: 600 })).toBe(250);
  });
});

describe("minScale (1% of image pixels)", () => {
  it("is 1% of the pixels expressed relative to the fitted size", () => {
    expect(minScale({ w: 4000, h: 3000 }, { w: 800, h: 600 })).toBeCloseTo(0.05);
    expect(minScale({ w: 200, h: 100 }, { w: 200, h: 100 })).toBeCloseTo(0.01);
  });
  it("reads 1% in the tooltip and never exceeds fit", () => {
    const natural = { w: 4000, h: 3000 };
    const fit = { w: 800, h: 600 };
    expect(zoomPercent({ scale: minScale(natural, fit), x: 0, y: 0 }, natural, fit)).toBe(1);
    expect(minScale({ w: 100000, h: 100 }, { w: 500, h: 1 })).toBe(1);
  });
});

describe("rotation helpers", () => {
  it("swaps width and height on quarter turns only", () => {
    const nat = { w: 400, h: 200 };
    expect(effectiveBox(nat, 0)).toEqual(nat);
    expect(effectiveBox(nat, 90)).toEqual({ w: 200, h: 400 });
    expect(effectiveBox(nat, -90)).toEqual({ w: 200, h: 400 });
    expect(effectiveBox(nat, 180)).toEqual(nat);
    expect(effectiveBox(nat, 270)).toEqual({ w: 200, h: 400 });
    expect(effectiveBox(nat, 450)).toEqual({ w: 200, h: 400 });
  });
  it("sizes the element so its rotated box is the fitted box", () => {
    expect(elementSize({ w: 300, h: 600 }, 90)).toEqual({ w: 600, h: 300 });
    expect(elementSize({ w: 300, h: 600 }, 0)).toEqual({ w: 300, h: 600 });
  });
  it("normalises turns into 0..359", () => {
    expect(normaliseTurn(-90)).toBe(270);
    expect(normaliseTurn(450)).toBe(90);
    expect(normaliseTurn(0)).toBe(0);
  });
  it("fits a landscape photo rotated upright into a portrait stage", () => {
    const stage = { w: 600, h: 1000 };
    const eff = effectiveBox({ w: 1200, h: 800 }, 90); // 800 x 1200 on its side
    expect(fitSize(eff, stage)).toEqual({ w: 600, h: 900 });
  });
});

describe("stepScale", () => {
  const detents = [1, 2];
  it("multiplies by the 1.15 step in either direction", () => {
    expect(stepScale(1, 1, false, detents)).toBeCloseTo(ZOOM_STEP);
    expect(stepScale(2, -1, false, detents)).toBeCloseTo(2 / ZOOM_STEP);
  });
  it("a fresh press passes straight over a detent", () => {
    expect(stepScale(1.9, 1, false, detents)).toBeCloseTo(1.9 * ZOOM_STEP);
  });
  it("a held key lands exactly on the detent it would cross", () => {
    expect(stepScale(1.9, 1, true, detents)).toBe(2);
    expect(stepScale(2.1, -1, true, detents)).toBe(2);
  });
  it("a held key stops while it sits on a detent", () => {
    expect(stepScale(2, 1, true, detents)).toBe(2);
    expect(stepScale(1, 1, true, detents)).toBe(1);
  });
  it("away from detents a held key steps normally", () => {
    expect(stepScale(1.2, 1, true, detents)).toBeCloseTo(1.2 * ZOOM_STEP);
  });
});
