import { describe, expect, it } from "vitest";
import { neighbour, neighbourIfGone, sameList } from "./folder-sync";

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

describe("neighbourIfGone", () => {
  // A transient I/O failure must not look like a deleted file: the user would lose their place.
  it("moves to the neighbour only when the shown file is gone", () => {
    expect(neighbourIfGone({ kind: "not_found", message: "gone" }, ["a", "b", "c"], 1)).toBe("c");
  });

  it("keeps the picture on I/O errors and on anything unrecognised", () => {
    expect(neighbourIfGone({ kind: "io", message: "busy" }, ["a", "b", "c"], 1)).toBeNull();
    expect(neighbourIfGone("cannot list", ["a", "b", "c"], 1)).toBeNull();
    expect(neighbourIfGone(null, ["a", "b", "c"], 1)).toBeNull();
  });
});
