import { describe, expect, it } from "vitest";
import { vi } from "vitest";
import type { Album } from "./api";
import { createFocusCheck, neighbour, neighbourIfGone, sameList } from "./folder-sync";

/** A folder the fake backend lists: `stamp` moves whenever its entries change. */
function fakeFolder(files: string[], stamp: string | null, fileStamps = files.map(() => "0-0")) {
  let current = { files, stamp, fileStamps };
  let listings = 0;
  const check = async (path: string, known: string | null): Promise<Album | null> => {
    if (known !== null && known === current.stamp) return null; // unchanged: nothing to re-list
    listings++;
    const index = current.files.indexOf(path);
    return {
      dir: "C:/pics",
      files: current.files,
      index,
      size: null,
      stamp: current.stamp,
      file_stamps: current.fileStamps,
    };
  };
  return {
    check,
    listings: () => listings,
    change: (files: string[], stamp: string | null, fileStamps = files.map(() => "0-0")) => {
      current = { files, stamp, fileStamps };
    },
  };
}

function fakeViewer(files: string[], stamps: string[] = files.map(() => "0-0")) {
  let list = files;
  let current = stamps;
  const refresh = vi.fn((album: Album) => {
    list = album.files;
    current = album.file_stamps ?? album.files.map(() => "0-0");
  });
  return { files: () => list, stamps: () => current, position: () => 0, refresh };
}

describe("sameList", () => {
  it("is true only for identical ordered lists", () => {
    expect(sameList(["a", "b"], ["a", "b"])).toBe(true);
    expect(sameList(["a", "b"], ["b", "a"])).toBe(false);
    expect(sameList(["a"], ["a", "b"])).toBe(false);
    expect(sameList([], [])).toBe(true);
  });
});

describe("neighbour", () => {
  it("prefers the next file, then the previous one", () => {
    expect(neighbour(["a", "b", "c"], 1)).toBe("c");
    expect(neighbour(["a", "b", "c"], 2)).toBe("b");
  });

  it("is null when nothing else is left", () => {
    expect(neighbour(["a"], 0)).toBeNull();
    expect(neighbour([], 0)).toBeNull();
  });
});

describe("neighbourIfGone", () => {
  // A transient I/O failure must not look like a deleted file: the user would lose their place.
  it("moves to the neighbour only when the shown file is gone", () => {
    expect(neighbourIfGone({ kind: "not_found", message: "gone" }, ["a", "b", "c"], 1)).toBe("c");
  });

  it("keeps the picture on I/O errors and on anything unrecognised", () => {
    expect(neighbourIfGone({ kind: "io", message: "busy" }, ["a", "b", "c"], 1)).toBeNull();
    expect(neighbourIfGone("cannot list", ["a", "b", "c"], 1)).toBeNull();
    expect(neighbourIfGone(null, ["a", "b", "c"], 1)).toBeNull();
  });
});

describe("createFocusCheck", () => {
  const pics = ["a.jpg", "b.jpg"];

  it("does not re-list a folder that has not changed", async () => {
    const folder = fakeFolder(pics, "1");
    const viewer = fakeViewer(pics);
    const focus = createFocusCheck(viewer, async () => {}, folder.check);
    await focus(); // learns the stamp: the one full listing
    await focus();
    await focus();
    expect(folder.listings()).toBe(1);
    expect(viewer.refresh).not.toHaveBeenCalled();
  });

  it("shows added files and then stops re-listing until the folder changes again", async () => {
    const folder = fakeFolder(pics, "1");
    const viewer = fakeViewer(pics);
    const focus = createFocusCheck(viewer, async () => {}, folder.check);
    await focus();
    folder.change([...pics, "c.jpg"], "2");
    await focus();
    expect(viewer.files()).toEqual([...pics, "c.jpg"]);
    await focus();
    expect(folder.listings()).toBe(2);
  });

  it("takes the new stamp even when the list is the same, so a non-image change is not re-listed forever", async () => {
    const folder = fakeFolder(pics, "1");
    const viewer = fakeViewer(pics);
    const focus = createFocusCheck(viewer, async () => {}, folder.check);
    await focus();
    folder.change(pics, "2"); // e.g. a .txt was added: the stamp moves, the picture list does not
    await focus();
    expect(viewer.refresh).not.toHaveBeenCalled();
    await focus();
    expect(folder.listings()).toBe(2);
  });

  it("re-lists a file that was rewritten in place, so its cached pixels are dropped", async () => {
    const folder = fakeFolder(pics, "1");
    const viewer = fakeViewer(pics);
    const focus = createFocusCheck(viewer, async () => {}, folder.check);
    await focus();
    // Same names and same folder stamp: only the pictured file's own stamp moved.
    folder.change(pics, "2", ["0-0", "7-7"]);
    await focus();
    expect(viewer.refresh).toHaveBeenCalledTimes(1);
  });

  it("keeps the picture on an I/O error and retries on the next focus", async () => {
    const viewer = fakeViewer(pics);
    const open = vi.fn(async () => {});
    const check = vi.fn(async () => {
      throw { kind: "io", message: "busy" };
    });
    const focus = createFocusCheck(viewer, open, check);
    await focus();
    await focus();
    expect(open).not.toHaveBeenCalled();
    expect(viewer.refresh).not.toHaveBeenCalled();
    expect(check).toHaveBeenCalledTimes(2);
  });

  it("moves to the neighbour when the shown file is gone", async () => {
    const viewer = fakeViewer(pics);
    const open = vi.fn(async () => {});
    const check = async () => {
      throw { kind: "not_found", message: "gone" };
    };
    await createFocusCheck(viewer, open, check)();
    expect(open).toHaveBeenCalledWith("b.jpg");
  });

  it("re-lists in full every time when the backend reports no stamp", async () => {
    const folder = fakeFolder(pics, null);
    const focus = createFocusCheck(fakeViewer(pics), async () => {}, folder.check);
    await focus();
    await focus();
    await focus();
    expect(folder.listings()).toBe(3);
  });

  it("ignores a focus that arrives while a check is still running", async () => {
    const viewer = fakeViewer(pics);
    let finish: (a: null) => void = () => {};
    const check = vi.fn(() => new Promise<null>((resolve) => (finish = resolve)));
    const focus = createFocusCheck(viewer, async () => {}, check);
    const first = focus();
    await focus();
    expect(check).toHaveBeenCalledTimes(1);
    finish(null);
    await first;
  });
});
