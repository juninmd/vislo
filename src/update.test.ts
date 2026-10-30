import { describe, expect, it } from "vitest";
import { progressText, updateText } from "./update";

describe("progressText", () => {
  it("shows the share of a known length, rounded down and never above 100", () => {
    expect(progressText({ received: 42, total: 100 })).toBe("Downloading… 42%");
    expect(progressText({ received: 1, total: 3 })).toBe("Downloading… 33%");
    expect(progressText({ received: 150, total: 100 })).toBe("Downloading… 100%");
  });

  it("falls back to megabytes when the server sent no length", () => {
    expect(progressText({ received: 3 * 1024 * 1024, total: null })).toBe("Downloading… 3.0 MB");
    expect(progressText({ received: 0, total: 0 })).toBe("Downloading… 0.0 MB");
  });
});

describe("updateText", () => {
  it("names the version and the first real line of the notes", () => {
    expect(updateText({ version: "1.2.0", notes: "## Features\n- sharper zoom\n- more" })).toBe(
      "Vislo 1.2.0 is available — sharper zoom",
    );
  });

  it("copes with empty or heading-only notes", () => {
    expect(updateText({ version: "1.2.0", notes: "" })).toBe("Vislo 1.2.0 is available");
    expect(updateText({ version: "1.2.0", notes: "## Only a heading\n\n" })).toBe("Vislo 1.2.0 is available");
  });
});
