import { type Album, openPath } from "./api";

interface Synced {
  files(): readonly string[];
  stamps(): readonly string[];
  position(): number;
  refresh(album: Album): void;
}

export const sameList = (a: readonly string[], b: readonly string[]): boolean =>
  a.length === b.length && a.every((f, i) => f === b[i]);

/** When the shown file is gone, the closest remaining neighbour (next first), or null. */
export function neighbour(files: readonly string[], index: number): string | null {
  return files[index + 1] ?? files[index - 1] ?? null;
}

/**
 * Re-lists the folder whenever the window regains focus, so files added or deleted in Explorer
 * show up without reopening. If the picture on screen was deleted, its neighbour is opened.
 */
export function bindFolderSync(viewer: Synced, open: (path: string) => Promise<void>): void {
  let busy = false;
  window.addEventListener("focus", async () => {
    const files = viewer.files();
    const stamps = viewer.stamps();
    const shown = files[viewer.position()];
    if (!shown || busy) return;
    busy = true;
    try {
      const album = await openPath(shown);
      // a rewritten file keeps its name: the stamps catch it, so its cached pixels are dropped too
      if (!sameList(album.files, files) || !sameList(album.file_stamps ?? [], stamps)) viewer.refresh(album);
    } catch {
      const next = neighbour(files, viewer.position());
      if (next) await open(next);
    } finally {
      busy = false;
    }
  });
}
