import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FocusReturn } from "./focus-return";

class FakeElement {
  isConnected = true;
  focus = vi.fn(() => {
    doc.activeElement = this;
  });
}
const doc: { activeElement: FakeElement | null } = { activeElement: null };

beforeEach(() => {
  vi.stubGlobal("HTMLElement", FakeElement);
  vi.stubGlobal("document", doc);
});
afterEach(() => vi.unstubAllGlobals());

describe("FocusReturn", () => {
  it("hands the focus back to the element that opened the popup", () => {
    const opener = new FakeElement();
    doc.activeElement = opener;
    const fr = new FocusReturn();
    fr.save();
    doc.activeElement = new FakeElement(); // the popup took the focus
    fr.restore(true);
    expect(opener.focus).toHaveBeenCalledOnce();
  });

  it("leaves the focus alone when it had already left the popup (a click elsewhere keeps its target)", () => {
    const opener = new FakeElement();
    doc.activeElement = opener;
    const fr = new FocusReturn();
    fr.save();
    fr.restore(false);
    expect(opener.focus).not.toHaveBeenCalled();
  });

  it("skips an opener that left the document and forgets it after one restore", () => {
    const opener = new FakeElement();
    doc.activeElement = opener;
    const fr = new FocusReturn();
    fr.save();
    opener.isConnected = false;
    fr.restore(true);
    opener.isConnected = true;
    fr.restore(true);
    expect(opener.focus).not.toHaveBeenCalled();
  });
});
