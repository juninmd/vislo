import { describe, expect, it } from "vitest";
import { copyPicture } from "./commands";
import { isPageCopy } from "./formats";

describe("copyPicture", () => {
  it("sends AVIF and SVG to the page, never to the backend that cannot decode them", async () => {
    const seen: string[] = [];
    const backend = async (p: string) => (seen.push(`backend ${p}`), null);
    const page = async (p: string) => (seen.push(`page ${p}`), { w: 8192, h: 4000 });
    for (const path of ["C:/pics/a.avif", "C:/pics/b.AVIFS", "C:/pics/c.svg"]) {
      expect(await copyPicture(path, backend, page, isPageCopy)).toEqual({ w: 8192, h: 4000 });
    }
    expect(seen).toEqual(["page C:/pics/a.avif", "page C:/pics/b.AVIFS", "page C:/pics/c.svg"]);
  });

  it("keeps a PNG on the backend, and reports its failure instead of retrying from the page", async () => {
    const seen: string[] = [];
    const backend = async () => {
      throw new Error("Clipboard is busy");
    };
    const page = async (p: string) => (seen.push(p), null);
    await expect(copyPicture("C:/pics/a.png", backend, page, isPageCopy)).rejects.toThrow("Clipboard is busy");
    expect(seen).toEqual([]);
  });

  it("returns the backend's shrunk size for a normal format", async () => {
    const backend = async () => ({ w: 8192, h: 5461 });
    expect(await copyPicture("C:/pics/huge.jpg", backend, async () => null, isPageCopy)).toEqual({ w: 8192, h: 5461 });
  });
});
