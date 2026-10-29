import { describe, expect, it } from "vitest";
import { extOf, isNative, isStill, isVector } from "./formats";

describe("formats", () => {
  it("opens SVG, CUR and AVIFS through the webview", () => {
    for (const f of ["a.svg", "a.CUR", "a.avifs"]) expect(isNative(f)).toBe(true);
  });

  it("does not hand vectors or animations to the sharp downscaler", () => {
    for (const f of ["a.svg", "a.avifs", "a.gif"]) expect(isStill(f)).toBe(false);
    expect(isStill("a.png")).toBe(true);
  });

  it("only SVG counts as vector, and extensions ignore folder dots on both separators", () => {
    expect(isVector(String.raw`C:\icons.v2\logo.svg`)).toBe(true);
    expect(isVector("logo.png")).toBe(false);
    expect(extOf(String.raw`C:\icons.v2\noext`)).toBe("");
    expect(extOf("/icons.v2/noext")).toBe("");
  });
});
