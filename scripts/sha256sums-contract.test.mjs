// The release workflow writes SHA256SUMS.txt with `sha256sum *` in the out dir, and the updater reads it
// with `expected_hash` (src-tauri/src/update/release.rs). These tests pin the format both sides rely on.
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

// Mirrors `expected_hash`: the first whitespace splits the hash, a leading `*` (binary mode) is dropped.
function expectedHash(sums, file) {
  for (const line of sums.split(/\r?\n/)) {
    const hash = line.match(/^\S*/)[0];
    const name = line.slice(hash.length).trim().replace(/^\*+/, "");
    if (name === file && /^[0-9a-f]{64}$/i.test(hash)) return hash.toLowerCase();
  }
  return null;
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

describe("SHA256SUMS.txt contract", () => {
  it("is read back by the updater for every file the release ships", () => {
    const dir = mkdtempSync(join(tmpdir(), "vislo-sums-"));
    const files = {
      "Vislo_0.2.0_x64-setup.exe": Buffer.from("synthetic installer"),
      "Vislo_0.2.0_x64-portable.exe": Buffer.from("synthetic portable"),
    };
    for (const [name, bytes] of Object.entries(files)) writeFileSync(join(dir, name), bytes);
    // `sha256sum *` prints `<hex>  <name>` per file, in glob (sorted) order
    const sums = Object.keys(files)
      .sort()
      .map((name) => `${sha256(files[name])}  ${name}\n`)
      .join("");

    for (const line of sums.trimEnd().split("\n")) expect(line).toMatch(/^[0-9a-f]{64}  [^\s*]+$/);
    for (const [name, bytes] of Object.entries(files)) expect(expectedHash(sums, name)).toBe(sha256(bytes));
  });

  it("also accepts the binary-mode spelling `<hex> *<name>`", () => {
    const bytes = Buffer.from("synthetic installer");
    const sums = `${sha256(bytes)} *Vislo_0.2.0_x64-setup.exe\n`;
    expect(expectedHash(sums, "Vislo_0.2.0_x64-setup.exe")).toBe(sha256(bytes));
  });

  it("keeps the workflow command and installer name the updater depends on", () => {
    const workflow = readFileSync(new URL("../.github/workflows/release.yml", import.meta.url), "utf8");
    expect(workflow).toContain("(cd out && sha256sum * > ../SHA256SUMS.txt)");
    expect(workflow).toContain('"out/Vislo_${V}_x64-setup.exe"');
  });
});
