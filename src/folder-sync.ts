import { type Album, openPath } from "./api";

interface Synced {
  files(): readonly string[];
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
 * Only a file that is really gone moves the view. Any other failure (a share that is briefly
 * unreachable, a locked file) keeps the picture; the next focus simply tries again.
 */
export function neighbourIfGone(err: unknown, files: readonly string[], index: number): string | null {
  const gone = typeof err === "object" && err !== null && (err as { kind?: unknown }).kind === "not_found";
  return gone ? neighbour(files, index) : null;
}

/**
 * Re-lists the folder whenever the window regains focus, so files added or deleted in Explorer
 * show up without reopening. If the picture on screen was deleted, its neighbour is opened.
 */
export function bindFolderSync(viewer: Synced, open: (path: string) => Promise<void>): void {
  let busy = false;
  window.addEventListener("focus", async () => {
    const files = viewer.files();
    const shown = files[viewer.position()];
    if (!shown || busy) return;
    busy = true;
    try {
      const album = await openPath(shown);
      if (!sameList(album.files, files)) viewer.refresh(album);
    } catch (err) {
      const next = neighbourIfGone(err, files, viewer.position());
      if (next) await open(next);
    } finally {
      busy = false;
    }
  });
}
