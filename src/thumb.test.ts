import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: (name: string) => `vislo://localhost/${name}`, invoke: vi.fn() }));
import { fallbackSrc, firstSrc, ThumbQueue } from "./thumb";

describe("thumbnail sources", () => {
  it("skips the thumbnailer for native formats it cannot decode", () => {
    expect(firstSrc("C:/a.avif", 192)).toContain("/o?");
    expect(firstSrc("C:/a.svg", 192)).toContain("/o?");
    expect(firstSrc("C:/a.jpg", 192)).toContain("/t?");
    expect(firstSrc("C:/a.cr2", 192)).toContain("/t?");
  });

  it("falls back to the file only where the webview can decode it", () => {
    expect(fallbackSrc("C:/a.jpg")).toContain("/o?");
    // A RAW, PSD or HEIC that failed the thumbnailer fails the converter too: no doomed full-size decode.
    for (const f of ["C:/a.cr2", "C:/a.psd", "C:/a.heic", "C:/a.tif"]) expect(fallbackSrc(f)).toBeNull();
  });
});

describe("ThumbQueue", () => {
  it("never has more than `limit` requests in flight and starts the rest in arrival order", () => {
    const q = new ThumbQueue<number>(2);
    const started: number[] = [];
    for (let i = 0; i < 5; i++) q.want(i, () => started.push(i));
    expect(started).toEqual([0, 1]);
    expect([q.inFlight, q.pending]).toEqual([2, 3]);
    q.done(1);
    expect(started).toEqual([0, 1, 2]);
    q.done(0);
    q.done(2);
    expect(started).toEqual([0, 1, 2, 3, 4]);
  });

  it("drops a key that left the screen before its turn", () => {
    const q = new ThumbQueue<number>(1);
    const started: number[] = [];
    for (let i = 0; i < 3; i++) q.want(i, () => started.push(i));
    q.drop(1);
    q.done(0);
    expect(started).toEqual([0, 2]);
  });

  it("ignores repeated wants, stray dones and dones after a clear", () => {
    const q = new ThumbQueue<number>(1);
    const started: number[] = [];
    q.want(7, () => started.push(7));
    q.want(7, () => started.push(-1));
    q.done(99);
    q.want(8, () => started.push(8));
    expect(started).toEqual([7]);
    q.clear();
    q.done(7);
    expect([q.inFlight, q.pending, started.length]).toEqual([0, 0, 1]);
    q.want(9, () => started.push(9));
    expect(started).toEqual([7, 9]);
  });
});
