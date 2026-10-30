import { describe, expect, it } from "vitest";
import { afterAll, beforeAll, vi } from "vitest";
import { query, route, srcFor } from "./api";

// The webview's URL builder; the tests run in Node, where there is no window.
beforeAll(() => {
  vi.stubGlobal("window", {
    __TAURI_INTERNALS__: {
      convertFileSrc: (path: string, protocol: string) => `http://${protocol}.localhost/${encodeURIComponent(path)}`,
    },
  });
});
afterAll(() => vi.unstubAllGlobals());

describe("query", () => {
  it("percent-encodes paths so separators and spaces survive the round trip", () => {
    const q = query({ p: "C:\\My Pics\\a&b#1.jpg", s: 256 });
    expect(q).toBe("p=C%3A%5CMy%20Pics%5Ca%26b%231.jpg&s=256");
    expect(decodeURIComponent(q.split("&")[0].slice(2))).toBe("C:\\My Pics\\a&b#1.jpg");
  });
});

describe("route", () => {
  it("puts the file stamp in the URL so a rewritten file is fetched again", () => {
    const before = route("o", { p: "C:\\pics\\a.png", v: "100-5" });
    const after = route("o", { p: "C:\\pics\\a.png", v: "200-6" });
    expect(before).not.toBe(after);
    expect(before.endsWith("?p=C%3A%5Cpics%5Ca.png&v=100-5")).toBe(true);
  });
});

describe("srcFor", () => {
  it("picks the original or the converted route and appends the stamp when known", () => {
    expect(srcFor("C:\\pics\\a.png", "1-2")).toContain("/o?p=");
    expect(srcFor("C:\\pics\\a.tif", "1-2")).toContain("/c?p=");
    expect(srcFor("C:\\pics\\a.png", "1-2").endsWith("&v=1-2")).toBe(true);
  });

  it("keeps the old URL shape when no stamp is known", () => {
    expect(srcFor("C:\\pics\\a.png")).not.toContain("v=");
  });
});
