import { srcFor } from "./api";
import { takePreloaded } from "./boot";

export interface Loaded {
  img: HTMLImageElement;
  ready: Promise<boolean>;
  /** True once `ready` settled, so a caller can tell a decoded picture from one still in flight. */
  settled: boolean;
}

/** Decoded-image cache keyed by album index; decoding happens off the main thread. */
export class ImageCache {
  private entries = new Map<number, Loaded>();

  constructor(
    private readonly pathOf: (index: number) => string,
    private readonly stampOf: (index: number) => string | undefined = () => undefined,
  ) {}

  get(index: number): Loaded {
    let entry = this.entries.get(index);
    if (!entry) {
      const src = srcFor(this.pathOf(index), this.stampOf(index));
      const early = takePreloaded(src); // started before the page even asked for this folder
      const img = early?.img ?? new Image();
      if (!early) {
        img.decoding = "async";
        img.draggable = false;
        img.src = src;
      }
      const loaded: Loaded = {
        img,
        ready: early?.ready ?? img.decode().then(() => true, () => false),
        settled: false,
      };
      void loaded.ready.then(() => (loaded.settled = true));
      entry = loaded;
      this.entries.set(index, loaded);
    }
    return entry;
  }

  /** Starts decoding in the background without waiting. */
  prefetch(index: number): void {
    void this.get(index).ready;
  }

  /** Drops every entry outside `keep` so memory stays bounded on huge folders. */
  retain(keep: Iterable<number>): void {
    const set = new Set(keep);
    for (const [index, { img }] of this.entries) {
      if (!set.has(index)) {
        img.src = "";
        this.entries.delete(index);
      }
    }
  }

  clear(): void {
    this.retain([]);
  }
}
