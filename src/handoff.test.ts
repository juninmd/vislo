import { describe, expect, it } from "vitest";
import { beginHandoff, type ClassHost, windowShown } from "./handoff";

function host(...initial: string[]): ClassHost & { names: Set<string> } {
  const names = new Set(initial);
  return {
    names,
    classList: {
      add: (...n) => n.forEach((x) => names.add(x)),
      remove: (...n) => n.forEach((x) => names.delete(x)),
      contains: (n) => names.has(n),
    },
  };
}

describe("hand-over from the native splash", () => {
  it("holds the page's dim and controls only when a splash is up", () => {
    const withSplash = host("booting");
    beginHandoff(withSplash, true);
    expect([...withSplash.names]).toEqual(["booting", "handoff"]);
    const without = host("booting");
    beginHandoff(without, false);
    expect([...without.names]).toEqual(["booting"]);
  });

  it("releases them as the window is shown, then drops the fade class", () => {
    const root = host("booting", "handoff");
    const pending: (() => void)[] = [];
    windowShown(root, (fn) => pending.push(fn), (fn) => fn(), () => {});
    expect([...root.names]).toEqual(["handoff-out"]);
    expect(pending).toHaveLength(1);
    pending[0]();
    expect(root.names.size).toBe(0);
  });

  it("tells the splash to fade only after the page's next frame, and only after a splash", () => {
    const events: string[] = [];
    const root = host("booting", "handoff");
    let frame: (() => void) | undefined;
    windowShown(root, () => {}, (fn) => (frame = fn), () => events.push("release"));
    expect(events, "not before the page has painted").toEqual([]);
    frame?.();
    expect(events).toEqual(["release"]);

    const plain = host("booting");
    windowShown(plain, () => {}, () => events.push("painted"), () => events.push("release"));
    expect(events, "no splash, nothing to release").toEqual(["release"]);
  });

  it("without a splash it only lets transitions start, with no fade class and no timer", () => {
    const root = host("booting");
    const pending: (() => void)[] = [];
    windowShown(root, (fn) => pending.push(fn), () => {}, () => pending.push(() => {}));
    expect(root.names.size).toBe(0);
    expect(pending).toHaveLength(0);
  });

  it("is safe to call again on later shows", () => {
    const root = host();
    windowShown(root, () => {});
    windowShown(root, () => {});
    expect(root.names.size).toBe(0);
  });
});
