import { describe, expect, it } from "vitest";
import { CopyError, copyFromPage, type PageClipboard, toCopyError } from "./copy-page";

/** A fake WebView: the picture is `width` x `height`; drawing records the canvas size it was given. */
function fakeClip(width: number, height: number, opts: { canDraw?: boolean; write?: () => Promise<void> } = {}) {
  const drawn: [number, number][] = [];
  const written: string[] = [];
  const clip: PageClipboard = {
    async decode() {
      return { width, height, source: "decoded" };
    },
    canvas(w, h) {
      return {
        draw() {
          if (opts.canDraw === false) return false;
          drawn.push([w, h]);
          return true;
        },
        png: async () => new Blob([`${w}x${h}`], { type: "image/png" }),
      };
    },
    async write(png) {
      await png;
      written.push("image/png");
      await opts.write?.();
    },
  };
  return { clip, drawn, written };
}

describe("copyFromPage", () => {
  it("copies a picture within the limits at its own size", async () => {
    const { clip, drawn, written } = fakeClip(1200, 800);
    expect(await copyFromPage("p", clip)).toBeNull();
    expect(drawn).toEqual([[1200, 800]]);
    expect(written).toEqual(["image/png"]);
  });

  it("draws a 16384 x 16384 picture at the budget size and reports it", async () => {
    const { clip, drawn } = fakeClip(16384, 16384);
    expect(await copyFromPage("p", clip)).toEqual({ w: 8192, h: 8192 });
    expect(drawn).toEqual([[8192, 8192]]);
  });

  it("refuses an empty picture before drawing anything", async () => {
    const { clip, drawn, written } = fakeClip(0, 0);
    await expect(copyFromPage("p", clip)).rejects.toMatchObject({ key: "msg.copyFailed" });
    expect(drawn).toEqual([]);
    expect(written).toEqual([]);
  });

  it("says the canvas is too large when the WebView refuses to draw it", async () => {
    const { clip, written } = fakeClip(8000, 8000, { canDraw: false });
    await expect(copyFromPage("p", clip)).rejects.toMatchObject({ key: "msg.copyTooLarge" });
    expect(written).toEqual([]);
  });

  it("passes a denied clipboard write through as the denied message", async () => {
    const denied = () => Promise.reject(new CopyError("msg.copyDenied"));
    const { clip } = fakeClip(10, 10, { write: denied });
    await expect(copyFromPage("p", clip)).rejects.toMatchObject({ key: "msg.copyDenied" });
  });
});

describe("toCopyError", () => {
  it("maps NotAllowedError to the denied message", () => {
    expect(toCopyError({ name: "NotAllowedError" }).key).toBe("msg.copyDenied");
  });

  it("maps any other failure to the generic message", () => {
    expect(toCopyError(new Error("boom")).key).toBe("msg.copyFailed");
    expect(toCopyError(undefined).key).toBe("msg.copyFailed");
  });

  it("keeps an existing CopyError as it is", () => {
    const err = new CopyError("msg.copyTooLarge");
    expect(toCopyError(err)).toBe(err);
  });
});
