import { describe, expect, it } from "vitest";
import { titleFor } from "./hud";
import { type MenuActions, menuItems } from "./menu-items";

const noop = () => {};
const actions: MenuActions = { command: noop, toggleInfo: noop, toggleSlideshow: noop, toggleStrip: noop, toggleFullscreen: noop, toggleSettings: noop };
const labelled = (hasPicture: boolean) => menuItems(actions, hasPicture).flatMap((e) => (e === "separator" ? [] : [e]));

describe("menuItems", () => {
  it("greys out file actions until a picture is open", () => {
    const off = labelled(false);
    for (const label of ["Copy image", "Show in folder", "Open with…", "Information"]) {
      expect(off.find((i) => i.label === label)?.disabled).toBe(true);
    }
    expect(off.find((i) => i.label === "Open image…")?.disabled).toBeFalsy();
  });

  it("enables everything once a picture is open", () => {
    expect(labelled(true).every((i) => !i.disabled)).toBe(true);
  });

  it("runs the matching command", () => {
    const seen: string[] = [];
    const items = menuItems({ ...actions, command: (c) => void seen.push(c) }, true).flatMap((e) => (e === "separator" ? [] : [e]));
    for (const i of items.filter((x) => ["Copy image", "Open image…", "Show in folder", "Open with…"].includes(x.label))) i.run();
    expect(seen).toEqual(["copy", "open", "reveal", "openWith"]);
  });
});

describe("titleFor", () => {
  it("adds the pixel size once known", () => {
    expect(titleFor("a.jpg", { w: 6000, h: 4000 })).toBe("a.jpg (6000 × 4000) — Vislo");
    expect(titleFor("a.jpg", null)).toBe("a.jpg — Vislo");
  });
});
