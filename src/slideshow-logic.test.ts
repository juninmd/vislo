import { describe, expect, it } from "vitest";
import { canSlideshow, nextSlide } from "./slideshow-logic";

describe("nextSlide", () => {
  it("advances and wraps to the first image", () => {
    expect(nextSlide(0, 3)).toBe(1);
    expect(nextSlide(2, 3)).toBe(0);
  });
  it("does nothing for a single image or empty album", () => {
    expect(nextSlide(0, 1)).toBeNull();
    expect(nextSlide(0, 0)).toBeNull();
  });
});

describe("canSlideshow", () => {
  it("needs at least two pictures", () => {
    expect(canSlideshow(2)).toBe(true);
    expect(canSlideshow(1)).toBe(false);
    expect(canSlideshow(0)).toBe(false);
  });
});
