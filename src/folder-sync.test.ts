import { describe, expect, it } from "vitest";
import { neighbour, sameList } from "./folder-sync";

describe("sameList", () => {
  it("is true only for identical ordered lists", () => {
    expect(sameList(["a", "b"], ["a", "b"])).toBe(true);
    expect(sameList(["a", "b"], ["b", "a"])).toBe(false);
    expect(sameList(["a"], ["a", "b"])).toBe(false);
    expect(sameList([], [])).toBe(true);
  });
});

describe("neighbour", () => {
  it("prefers the next file, then the previous one", () => {
    expect(neighbour(["a", "b", "c"], 1)).toBe("c");
    expect(neighbour(["a", "b", "c"], 2)).toBe("b");
  });

  it("is null when nothing else is left", () => {
    expect(neighbour(["a"], 0)).toBeNull();
    expect(neighbour([], 0)).toBeNull();
  });
});
