import { describe, expect, it, vi } from "vitest";
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

describe("Commands", () => {
  const setup = async () => {
    vi.resetModules();
    const calls: string[] = [];
    vi.doMock("./api", () => ({
      copyImage: vi.fn(),
      openWith: vi.fn(),
      pickImage: vi.fn(),
      revealInFolder: vi.fn(),
      srcFor: (p: string) => `src:${p}`,
      setWallpaper: async (p: string) => void calls.push(`wallpaper ${p}`),
      setLockScreen: async (p: string) => void calls.push(`lock ${p}`),
      sharePicture: async (p: string) => {
        calls.push(`share ${p}`);
        throw new Error("Sharing is Windows-only");
      },
    }));
    vi.doMock("./print", () => ({ printPicture: async (s: string) => void calls.push(`print ${s}`) }));
    const { Commands } = await import("./commands");
    const toasts: string[] = [];
    const commands = new Commands({ path: () => "C:/pics/a.png", notify: (t) => toasts.push(t) }, async () => {});
    return { commands, calls, toasts };
  };

  it("sends the open picture to the wallpaper, lock screen and printer, and confirms the first two", async () => {
    const { commands, calls, toasts } = await setup();
    for (const c of ["wallpaper", "lockScreen", "print"] as const) await commands.run(c);
    expect(calls).toEqual(["wallpaper C:/pics/a.png", "lock C:/pics/a.png", "print src:C:/pics/a.png"]);
    expect(toasts).toEqual(["Set as desktop wallpaper", "Set as lock screen"]);
  });

  it("reports a failed share instead of swallowing it", async () => {
    const { commands, toasts } = await setup();
    await commands.run("share");
    expect(toasts).toEqual(["Error: Sharing is Windows-only"]);
  });
});
