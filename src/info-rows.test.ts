import { describe, expect, it } from "vitest";
import type { Info } from "./api";
import { buildGroups, enumKey, formatAspect, formatBytes, formatDimensions } from "./info-rows";

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

describe("formatAspect", () => {
  it("reduces common shapes and falls back to a decimal ratio", () => {
    expect(formatAspect(6000, 4000)).toBe("3:2");
    expect(formatAspect(1920, 1080)).toBe("16:9");
    expect(formatAspect(1080, 1920)).toBe("9:16");
    expect(formatAspect(4001, 3000)).toBe("1.33:1");
    expect(formatAspect(3000, 4001)).toBe("1:1.33");
  });
  it("is empty without a size", () => {
    expect(formatAspect(null, 10)).toBeNull();
    expect(formatAspect(10, 0)).toBeNull();
  });
});

describe("enumKey", () => {
  it("maps rotations and mirrored orientations", () => {
    expect(enumKey("orientation", 3)).toBe("info.val.rot180");
    expect(enumKey("orientation", 6)).toBe("info.val.rotCW");
    expect(enumKey("orientation", 8)).toBe("info.val.rotCCW");
    expect(enumKey("orientation", 5)).toBe("info.val.mirrored");
  });
  it("only translates codes it knows, so an unknown code keeps the backend's wording", () => {
    expect(enumKey("program", 8)).toBe("info.val.program.8");
    expect(enumKey("program", 9)).toBeNull();
    expect(enumKey("metering", 255)).toBe("info.val.metering.255");
    expect(enumKey("metering", 7)).toBeNull();
    expect(enumKey("colorSpace", 1)).toBeNull();
    expect(enumKey("colorSpace", 0xffff)).toBe("info.val.uncalibrated");
    expect(enumKey("iso", 100)).toBeNull();
  });
});

describe("buildGroups", () => {
  const base: Info = {
    name: "a.jpg", folder: "C:/p", size: 2048, modified_ms: 2000, created_ms: 1000,
    width: 6000, height: 4000, format: "JPEG", color: "RGB · 8 bit", rows: [],
  };
  const tr = (k: string) => k;
  const fmt = (ms: number) => `@${ms}`;

  it("always yields the file and image groups, in display order", () => {
    const g = buildGroups(base, tr, fmt);
    expect(g.map((x) => x.id)).toEqual(["file", "image"]);
    expect(g[0].lines.map((l) => l.label)).toEqual(["info.file", "info.folder", "info.size", "info.format", "info.created", "info.modified"]);
    expect(g[1].lines.map((l) => l.value)).toEqual(["6000 × 4000 px (24.0 MP)", "3:2", "RGB · 8 bit"]);
  });

  it("drops missing facts instead of printing blanks", () => {
    const g = buildGroups({ ...base, created_ms: null, format: "", color: null, width: null, height: null }, tr, fmt);
    expect(g[0].lines.map((l) => l.label)).not.toContain("info.created");
    expect(g[0].lines.map((l) => l.label)).not.toContain("info.format");
    expect(g.map((x) => x.id)).toEqual(["file"]);
  });

  it("places EXIF rows in their group, translates codes and keeps text values", () => {
    const rows = [
      { group: "location", key: "gps", value: "1, 2", code: null },
      { group: "exposure", key: "program", value: "Aperture priority", code: 3 },
      { group: "exposure", key: "iso", value: "200", code: null },
      { group: "camera", key: "camera", value: "Canon", code: null },
      { group: "image", key: "colorSpace", value: "sRGB", code: 1 },
      { group: "image", key: "orientation", value: "Rotate 90 CW", code: 6 },
    ];
    const g = buildGroups({ ...base, rows }, tr, fmt);
    expect(g.map((x) => x.id)).toEqual(["file", "image", "camera", "exposure", "location"]);
    const exposure = g.find((x) => x.id === "exposure")!;
    expect(exposure.lines).toEqual([
      { label: "info.program", value: "info.val.program.3" },
      { label: "info.iso", value: "200" },
    ]);
    const image = g.find((x) => x.id === "image")!.lines;
    expect(image.find((l) => l.label === "info.colorSpace")!.value).toBe("sRGB");
    expect(image.find((l) => l.label === "info.orientation")!.value).toBe("info.val.rotCW");
  });

  it("shows a row id it has no label for rather than hiding the data", () => {
    const g = buildGroups({ ...base, rows: [{ group: "camera", key: "newThing", value: "x", code: null }] }, tr, fmt);
    expect(g.find((x) => x.id === "camera")!.lines).toEqual([{ label: "newThing", value: "x" }]);
  });
});
