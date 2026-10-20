import { describe, expect, it } from "vitest";
import { query } from "./api";

describe("query", () => {
  it("percent-encodes paths so separators and spaces survive the round trip", () => {
    const q = query({ p: "C:\\My Pics\\a&b#1.jpg", s: 256 });
    expect(q).toBe("p=C%3A%5CMy%20Pics%5Ca%26b%231.jpg&s=256");
    expect(decodeURIComponent(q.split("&")[0].slice(2))).toBe("C:\\My Pics\\a&b#1.jpg");
  });
});
