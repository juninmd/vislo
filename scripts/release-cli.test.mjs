import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const script = resolve("scripts/release.mjs");
let repo;

const git = (...args) => execFileSync("git", args, { cwd: repo, encoding: "utf8" }).trim();
const commit = (message) => {
  writeFileSync(join(repo, "f.txt"), message);
  git("add", "f.txt");
  git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", message);
};
const plan = () => JSON.parse(execFileSync("node", [script, "plan"], { cwd: repo, encoding: "utf8" }));

beforeAll(() => {
  repo = mkdtempSync(join(tmpdir(), "vislo-release-"));
  mkdirSync(repo, { recursive: true });
  git("init", "-q");
  writeFileSync(join(repo, "package.json"), JSON.stringify({ version: "0.0.0" }));
  git("add", "package.json");
  git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "chore: start");
});

afterAll(() => rmSync(repo, { recursive: true, force: true }));

describe("release.mjs plan in a real repository", () => {
  it("ships the commits that no release contains yet", () => {
    commit("feat: first thing");
    expect(plan().version).toBe("0.1.0");
  });

  it("plans nothing for a commit that a release already contains", () => {
    // a queued run still has an older checkout than the release a previous run has just tagged
    const older = git("rev-parse", "HEAD");
    commit("chore(release): v0.1.0 [skip ci]");
    git("-c", "user.name=t", "-c", "user.email=t@t", "tag", "-a", "v0.1.0", "-m", "v0.1.0");
    git("checkout", "-q", "--detach", older);
    expect(plan()).toEqual({});
    git("checkout", "-q", "-");
  });

  it("plans from the new tag afterwards", () => {
    commit("fix: later fix");
    const p = plan();
    expect([p.version, p.previous]).toEqual(["0.1.1", "v0.1.0"]);
  });
});
