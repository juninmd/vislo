import { describe, expect, it } from "vitest";
import { updateText } from "./update";

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
