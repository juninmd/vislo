import { describe, expect, it } from "vitest";
import { ICO_SIZES, packIco, SMALL_MAX } from "./ico.mjs";

const png = (n) => Buffer.from(Array.from({ length: n }, (_, i) => i % 251));

describe("packIco", () => {
  const frames = [
    { size: 16, png: png(10) },
    { size: 256, png: png(30) },
  ];
  const ico = packIco(frames);

  it("writes an icon directory with one entry per frame", () => {
    expect(ico.readUInt16LE(0)).toBe(0);
    expect(ico.readUInt16LE(2)).toBe(1); // type: icon
    expect(ico.readUInt16LE(4)).toBe(2);
  });

  it("encodes 256 px as 0 and points each entry at its own payload", () => {
    expect([ico[6], ico[7]]).toEqual([16, 16]);
    expect([ico[22], ico[23]]).toEqual([0, 0]);
    for (const [i, f] of frames.entries()) {
      const e = 6 + 16 * i;
      const [len, off] = [ico.readUInt32LE(e + 8), ico.readUInt32LE(e + 12)];
      expect(len).toBe(f.png.length);
      expect(ico.subarray(off, off + len).equals(f.png)).toBe(true);
    }
  });
});

describe("icon sizes", () => {
  it("covers the Windows DPI steps, 20 and 40 px included", () => {
    expect(ICO_SIZES).toEqual(expect.arrayContaining([16, 20, 24, 32, 40, 48, 64, 256]));
  });

  it("switches to the simplified artwork only up to 32 px", () => {
    expect(SMALL_MAX).toBe(32);
    expect(ICO_SIZES.filter((s) => s <= SMALL_MAX)).toEqual([16, 20, 24, 32]);
  });
});
