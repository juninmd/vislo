import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("./api", () => ({ srcFor: (path: string, stamp?: string) => `src:${path}:${stamp ?? ""}` }));
vi.mock("./boot", () => ({ takePreloaded: vi.fn(() => null) }));

import { takePreloaded } from "./boot";
import { ImageCache } from "./cache";

class FakeImage {
  static all: FakeImage[] = [];
  decoding = "";
  draggable = true;
  src = "";
  naturalWidth = 0;
  naturalHeight = 0;
  static failDecode = false;
  constructor() {
    FakeImage.all.push(this);
  }
  decode() {
    return FakeImage.failDecode ? Promise.reject(new Error("bad image")) : Promise.resolve();
  }
}

beforeEach(() => {
  FakeImage.all = [];
  FakeImage.failDecode = false;
  vi.stubGlobal("Image", FakeImage);
  vi.mocked(takePreloaded).mockReturnValue(null);
});
afterEach(() => vi.unstubAllGlobals());

const make = () => new ImageCache((i) => `/pics/${i}.jpg`, (i) => `stamp${i}`);

describe("ImageCache", () => {
  it("decodes an index once and reuses the entry", async () => {
    const cache = make();
    const a = cache.get(3);
    expect(cache.get(3)).toBe(a);
    expect(FakeImage.all).toHaveLength(1);
    expect(a.img.src).toBe("src:/pics/3.jpg:stamp3");
    expect(a.img.decoding).toBe("async");
    expect(a.img.draggable).toBe(false);
    expect(await a.ready).toBe(true);
    expect(a.settled).toBe(true);
  });

  it("settles a failed decode as false instead of rejecting", async () => {
    const cache = make();
    FakeImage.failDecode = true;
    const entry = cache.get(0);
    expect(await entry.ready).toBe(false);
    expect(entry.settled).toBe(true);
  });

  it("takes over a picture already preloaded at boot instead of loading it again", async () => {
    const early = { img: new FakeImage() as unknown as HTMLImageElement, ready: Promise.resolve(true) };
    vi.mocked(takePreloaded).mockReturnValueOnce(early as never);
    const entry = make().get(1);
    expect(entry.img).toBe(early.img);
    expect(FakeImage.all).toHaveLength(1); // only the one built for `early`
    expect(await entry.ready).toBe(true);
  });

  it("reports decoded RGBA bytes only once the size is known", () => {
    const cache = make();
    expect(cache.bytes(5)).toBeUndefined();
    const { img } = cache.get(5);
    expect(cache.bytes(5)).toBeUndefined(); // naturalWidth still 0
    Object.assign(img, { naturalWidth: 4000, naturalHeight: 3000 });
    expect(cache.bytes(5)).toBe(4000 * 3000 * 4);
  });

  it("retain drops and releases every entry outside the kept window", () => {
    const cache = make();
    const kept = cache.get(1);
    const dropped = cache.get(2);
    cache.retain([1]);
    expect(dropped.img.src).toBe(""); // releases the decoded bitmap
    expect(kept.img.src).not.toBe("");
    expect(cache.get(1)).toBe(kept);
    expect(cache.get(2)).not.toBe(dropped); // re-created on demand
  });

  it("clear releases everything", () => {
    const cache = make();
    const a = cache.get(1);
    cache.clear();
    expect(a.img.src).toBe("");
    expect(cache.bytes(1)).toBeUndefined();
  });
});
