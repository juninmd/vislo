import { describe, expect, it } from "vitest";
import { actualScale, clampView, effectiveBox, elementSize, fitSize, maxScale, normaliseTurn, zoomAt, zoomPercent } from "./zoom";

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
  it("cannot zoom out below the fitted size", () => {
    expect(zoomAt({ scale: 1, x: 0, y: 0 }, 0.5, 0, 0, fit, stage, 24).scale).toBe(1);
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
  it("reads 100% at actual size and 1000% at the ceiling", () => {
    expect(zoomPercent({ scale: actualScale(natural, fit), x: 0, y: 0 }, natural, fit)).toBe(100);
    expect(zoomPercent({ scale: maxScale(natural, fit), x: 0, y: 0 }, natural, fit)).toBe(1000);
  });
  it("is relative to device pixels on a HiDPI display (natural given in CSS px)", () => {
    // 4000px image at dpr 2 -> 2000 CSS px; fitted to 800 CSS px = 40 device-px share of 4000
    expect(zoomPercent({ scale: 1, x: 0, y: 0 }, { w: 2000, h: 1500 }, fit)).toBe(40);
  });
});

describe("maxScale (1000% of image pixels)", () => {
  it("is ten times actual size for a large image", () => {
    expect(maxScale({ w: 4000, h: 3000 }, { w: 800, h: 600 })).toBe(50);
  });
  it("is 10 for an image that already fits at 100%", () => {
    expect(maxScale({ w: 200, h: 100 }, { w: 200, h: 100 })).toBe(10);
  });
  it("lets a HiDPI display reach 1000% of image pixels (CSS size = px / dpr)", () => {
    // 4000px image on a 2x display: natural CSS width 2000, fit 800 -> 10 * 2000 / 800
    expect(maxScale({ w: 2000, h: 1500 }, { w: 800, h: 600 })).toBe(25);
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
