import { convertFileSrc } from "@tauri-apps/api/core";

export interface Loaded {
  img: HTMLImageElement;
  ready: Promise<boolean>;
}

/** Decoded-image cache keyed by album index; decoding happens off the main thread. */
export class ImageCache {
  private entries = new Map<number, Loaded>();

  constructor(private readonly pathOf: (index: number) => string) {}

  get(index: number): Loaded {
    let entry = this.entries.get(index);
    if (!entry) {
      const img = new Image();
      img.decoding = "async";
      img.draggable = false;
      img.src = convertFileSrc(this.pathOf(index));
      entry = { img, ready: img.decode().then(() => true, () => false) };
      this.entries.set(index, entry);
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
