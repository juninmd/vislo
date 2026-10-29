import { describe, expect, it } from "vitest";
import { formatBytes, formatDimensions } from "./info";

describe("formatBytes", () => {
  it("scales through the units", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.0 MB");
    expect(formatBytes(300 * 1024 * 1024)).toBe("300 MB");
    expect(formatBytes(3 * 1024 ** 3)).toBe("3.0 GB");
  });
});

describe("formatDimensions", () => {
  it("shows pixels and megapixels", () => {
    expect(formatDimensions(6000, 4000)).toBe("6000 × 4000 px (24.0 MP)");
  });
  it("is null when the size is unknown", () => {
    expect(formatDimensions(null, 10)).toBeNull();
    expect(formatDimensions(0, 0)).toBeNull();
  });
});
