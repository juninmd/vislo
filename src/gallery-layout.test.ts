import { describe, expect, it } from "vitest";
import { clampSize, GALLERY_MAX, GALLERY_MIN, moveIndex, thumbPx } from "./gallery-layout";

describe("clampSize", () => {
  it("keeps the slider range", () => {
    expect(clampSize(10)).toBe(GALLERY_MIN);
    expect(clampSize(9999)).toBe(GALLERY_MAX);
    expect(clampSize(200.4)).toBe(200);
  });
});

describe("thumbPx", () => {
  it("rounds the device-pixel size up to a cacheable bucket", () => {
    expect(thumbPx(96, 1)).toBe(128);
    expect(thumbPx(200, 1)).toBe(256);
    expect(thumbPx(200, 1.5)).toBe(384);
    expect(thumbPx(480, 2)).toBe(1024);
    expect(thumbPx(480, 3)).toBe(1024);
  });
  it("treats a nonsense pixel ratio as 1", () => {
    expect(thumbPx(200, 0)).toBe(256);
  });
});

describe("moveIndex", () => {
  it("moves by one and by a row", () => {
    expect(moveIndex(5, "ArrowRight", 4, 20)).toBe(6);
    expect(moveIndex(5, "ArrowLeft", 4, 20)).toBe(4);
    expect(moveIndex(5, "ArrowDown", 4, 20)).toBe(9);
    expect(moveIndex(5, "ArrowUp", 4, 20)).toBe(1);
  });
  it("stops at the edges instead of wrapping", () => {
    expect(moveIndex(0, "ArrowLeft", 4, 20)).toBe(0);
    expect(moveIndex(2, "ArrowUp", 4, 20)).toBe(0);
    expect(moveIndex(19, "ArrowRight", 4, 20)).toBe(19);
    expect(moveIndex(18, "ArrowDown", 4, 20)).toBe(19);
  });
  it("jumps to the ends and by pages", () => {
    expect(moveIndex(7, "Home", 4, 20)).toBe(0);
    expect(moveIndex(7, "End", 4, 20)).toBe(19);
    expect(moveIndex(1, "PageDown", 4, 20, 3)).toBe(13);
    expect(moveIndex(15, "PageUp", 4, 20, 3)).toBe(3);
  });
  it("is safe on an empty folder", () => {
    expect(moveIndex(0, "End", 4, 0)).toBe(0);
  });
});
