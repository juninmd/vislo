import { describe, expect, it } from "vitest";
import { titleFor } from "./hud";

describe("titleFor", () => {
  it("shows the pixel size once it is known", () => {
    expect(titleFor("foto.jpg", { w: 4000, h: 3000 })).toBe("foto.jpg (4000 × 3000) — Vislo");
  });

  it("omits the size while it is still unknown", () => {
    expect(titleFor("foto.jpg", null)).toBe("foto.jpg — Vislo");
  });

  it("keeps unicode file names intact", () => {
    expect(titleFor("写真 ☃.png", null)).toBe("写真 ☃.png — Vislo");
  });
});
