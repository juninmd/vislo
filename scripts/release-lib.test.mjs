import { describe, expect, it } from "vitest";
import { parseCommit, problems } from "./commits.mjs";
import { bump, bumpFor, notes, parseVersion, plan, stamp } from "./release-lib.mjs";

const c = (message, hash = "abcdef0123") => ({ message, hash });

describe("semantic commit parsing", () => {
  it("reads type, scope, breaking marker and subject", () => {
    expect(parseCommit("feat(zoom)!: allow 20,000%")).toMatchObject({ type: "feat", scope: "zoom", breaking: true, subject: "allow 20,000%" });
    expect(parseCommit("fix: crash\n\nBREAKING CHANGE: settings moved").breaking).toBe(true);
    expect(parseCommit("updated stuff")).toBeNull();
    expect(parseCommit("Feat: capital")).toBeNull();
  });

  it("explains what is wrong with a message", () => {
    expect(problems("feat(ui): add a banner")).toEqual([]);
    expect(problems("Merge pull request #7 from a/b")).toEqual([]);
    expect(problems("did things")[0]).toMatch(/not a semantic commit/);
    expect(problems("fix: Ends with a period.")).toHaveLength(2);
    expect(problems(`feat: ${"x".repeat(120)}`)[0]).toMatch(/characters/);
  });
});

describe("version planning", () => {
  it("bumps minor for features and patch for everything else", () => {
    expect(bumpFor([c("feat: a"), c("fix: b")], [1, 2, 3])).toBe("minor");
    expect(bumpFor([c("fix: b")], [1, 2, 3])).toBe("patch");
    expect(bumpFor([c("docs: only docs")], [1, 2, 3])).toBe("patch");
    expect(bumpFor([c("not semantic")], [1, 2, 3])).toBe("patch");
  });

  it("keeps breaking changes below 1.0 as a minor bump", () => {
    expect(bumpFor([c("feat!: new format")], [0, 4, 1])).toBe("minor");
    expect(bumpFor([c("feat!: new format")], [1, 4, 1])).toBe("major");
    expect(bump([1, 4, 1], "major")).toEqual([2, 0, 0]);
    expect(bump([1, 4, 1], "minor")).toEqual([1, 5, 0]);
    expect(bump([1, 4, 1], "patch")).toEqual([1, 4, 2]);
  });

  it("plans the next release and ignores the release commit itself", () => {
    const repo = "o/r";
    expect(plan([c("chore(release): v1.0.1 [skip ci]")], "v1.0.1", { repo })).toBeNull();
    const p = plan([c("feat(ui): update banner"), c("fix: toast over filmstrip", "1234567890")], "v0.3.0", { repo });
    expect(p.version).toBe("0.4.0");
    expect(p.notes).toContain("## Features\n\n- **ui:** update banner (abcdef0)");
    expect(p.notes).toContain("## Bug fixes");
    expect(p.notes).toContain("https://github.com/o/r/compare/v0.3.0...v0.4.0");
  });

  it("starts from the file version when nothing was released yet", () => {
    expect(plan([c("feat: first")], "0.1.0", { repo: "o/r" }).version).toBe("0.2.0");
    expect(parseVersion("v1.2.3")).toEqual([1, 2, 3]);
    expect(parseVersion("1.2")).toBeNull();
  });

  it("lists breaking changes first", () => {
    const text = notes([c("feat: a"), c("fix!: b")], { version: "1.0.0", previous: null, repo: "o/r" });
    expect(text.indexOf("Breaking changes")).toBeLessThan(text.indexOf("Features"));
  });
});

describe("stamping versions", () => {
  it("rewrites every file that carries the version", () => {
    const out = stamp(
      {
        "package.json": '{\n  "name": "vislo",\n  "version": "0.1.0"\n}\n',
        "package-lock.json": JSON.stringify({ name: "vislo", version: "0.1.0", packages: { "": { version: "0.1.0" } } }),
        "src-tauri/Cargo.toml": '[package]\nname = "vislo"\nversion = "0.1.0"\n\n[dependencies]\nserde = { version = "1" }\n',
        "src-tauri/Cargo.lock": '[[package]]\nname = "other"\nversion = "9.9.9"\n\n[[package]]\nname = "vislo"\nversion = "0.1.0"\n',
        "src-tauri/tauri.conf.json": '{ "productName": "Vislo",\n  "version": "0.1.0" }',
      },
      "0.4.0",
    );
    expect(out["package.json"]).toContain('"version": "0.4.0"');
    expect(JSON.parse(out["package-lock.json"]).packages[""].version).toBe("0.4.0");
    expect(out["src-tauri/Cargo.toml"]).toContain('version = "0.4.0"');
    expect(out["src-tauri/Cargo.toml"]).toContain('serde = { version = "1" }');
    expect(out["src-tauri/Cargo.lock"]).toContain('name = "other"\nversion = "9.9.9"');
    expect(out["src-tauri/Cargo.lock"]).toContain('name = "vislo"\nversion = "0.4.0"');
    expect(out["src-tauri/tauri.conf.json"]).toContain('"version": "0.4.0"');
  });
});
