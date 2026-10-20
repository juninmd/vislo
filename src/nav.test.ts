import { describe, expect, it } from "vitest";
import { jumpIndex, preloadWindow, stepIndex, WheelAccumulator } from "./nav";

describe("stepIndex", () => {
  it("moves within bounds", () => expect(stepIndex(3, 1, 10)).toBe(4));
  it("returns null past either end instead of wrapping", () => {
    expect(stepIndex(0, -1, 10)).toBeNull();
    expect(stepIndex(9, 1, 10)).toBeNull();
  });
});

describe("preloadWindow", () => {
  it("lists the current image first, then forward neighbours before backward ones", () => {
    expect(preloadWindow(10, 100, 1)).toEqual([10, 11, 9, 12, 8, 13, 14]);
  });
  it("flips the bias when travelling backwards", () => {
    expect(preloadWindow(10, 100, -1)).toEqual([10, 9, 11, 8, 12, 7, 6]);
  });
  it("clips at both ends of the album", () => {
    expect(preloadWindow(0, 3, 1)).toEqual([0, 1, 2]);
    expect(preloadWindow(0, 1, -1)).toEqual([0]);
  });
});

describe("WheelAccumulator", () => {
  it("fires once per full notch and carries the remainder", () => {
    const w = new WheelAccumulator(100);
    expect(w.push(60)).toBe(0);
    expect(w.push(60)).toBe(1);
    expect(w.push(90)).toBe(1); // 20 left + 90 = 110
  });
  it("counts direction changes against the carried remainder", () => {
    const w = new WheelAccumulator(100);
    expect(w.push(-250)).toBe(-2);
    expect(w.push(50)).toBe(0);
  });
});

describe("jumpIndex", () => {
  it("moves a full step when there is room", () => {
    expect(jumpIndex(20, 10, 100)).toBe(30);
    expect(jumpIndex(20, -10, 100)).toBe(10);
  });
  it("stops at the ends instead of failing", () => {
    expect(jumpIndex(95, 10, 100)).toBe(99);
    expect(jumpIndex(3, -10, 100)).toBe(0);
  });
  it("is null when already at the end", () => {
    expect(jumpIndex(99, 10, 100)).toBeNull();
    expect(jumpIndex(0, -10, 100)).toBeNull();
  });
});
