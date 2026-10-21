// Version planning for the release workflow: every commit that lands on main ships a release.

import { parseCommit } from "./commits.mjs";

export function parseVersion(text) {
  const m = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(String(text).trim());
  return m ? [Number(m[1]), Number(m[2]), Number(m[3])] : null;
}

export const formatVersion = ([a, b, c]) => `${a}.${b}.${c}`;

/**
 * major: a breaking change (while still 0.x it bumps the minor, as is customary)
 * minor: feat
 * patch: everything else, so each commit on main is deliverable
 */
export function bumpFor(commits, current) {
  const parsed = commits.map((c) => parseCommit(c.message));
  const breaking = parsed.some((p) => p?.breaking);
  if (breaking) return current[0] === 0 ? "minor" : "major";
  return parsed.some((p) => p?.type === "feat") ? "minor" : "patch";
}

export function bump(version, kind) {
  const [a, b, c] = version;
  if (kind === "major") return [a + 1, 0, 0];
  if (kind === "minor") return [a, b + 1, 0];
  return [a, b, c + 1];
}

/** Commits made by the release job itself, which must never trigger another release. */
export const isReleaseCommit = (message) => /^chore\(release\): /.test(message);

const SECTIONS = [
  ["breaking", "Breaking changes"],
  ["feat", "Features"],
  ["fix", "Bug fixes"],
  ["perf", "Performance"],
  ["refactor", "Refactoring"],
  ["docs", "Documentation"],
  ["build", "Build"],
  ["ci", "CI"],
  ["test", "Tests"],
  ["style", "Style"],
  ["chore", "Chores"],
  ["revert", "Reverts"],
];

/** Markdown release notes grouped by commit type. */
export function notes(commits, { version, previous, repo }) {
  const groups = new Map();
  for (const c of commits) {
    if (isReleaseCommit(c.message)) continue;
    const p = parseCommit(c.message);
    const key = p?.breaking ? "breaking" : (p?.type ?? "chore");
    const subject = p ? p.subject : c.message.split("\n")[0].trim();
    const line = `- ${p?.scope ? `**${p.scope}:** ` : ""}${subject} (${c.hash.slice(0, 7)})`;
    groups.set(key, [...(groups.get(key) ?? []), line]);
  }
  const out = [];
  for (const [key, title] of SECTIONS) {
    if (groups.has(key)) out.push(`## ${title}\n\n${groups.get(key).join("\n")}`);
  }
  if (previous) out.push(`**Full changelog:** https://github.com/${repo}/compare/v${previous}...v${version}`);
  return out.join("\n\n") + "\n";
}

/** Next release from the commits since the last tag; `null` when there is nothing to ship. */
export function plan(commits, last, { repo }) {
  const shippable = commits.filter((c) => !isReleaseCommit(c.message));
  if (shippable.length === 0) return null;
  const current = parseVersion(last) ?? [0, 0, 0];
  const kind = bumpFor(shippable, current);
  const next = formatVersion(bump(current, kind));
  return {
    version: next,
    bump: kind,
    notes: notes(shippable, { version: next, previous: parseVersion(last) ? formatVersion(current) : null, repo }),
  };
}

/** Rewrites the version in the files that carry it; returns the new text of each. */
export function stamp(files, version) {
  const out = {};
  for (const [name, text] of Object.entries(files)) {
    if (name.endsWith("package.json") || name.endsWith("tauri.conf.json")) {
      out[name] = text.replace(/("version"\s*:\s*")[^"]+(")/, `$1${version}$2`);
    } else if (name.endsWith("package-lock.json")) {
      const lock = JSON.parse(text);
      lock.version = version;
      if (lock.packages?.[""]) lock.packages[""].version = version;
      out[name] = JSON.stringify(lock, null, 2) + "\n";
    } else if (name.endsWith("Cargo.toml")) {
      out[name] = text.replace(/^(version\s*=\s*")[^"]+(")/m, `$1${version}$2`);
    } else if (name.endsWith("Cargo.lock")) {
      out[name] = text.replace(/(\[\[package\]\]\nname = "vislo"\nversion = ")[^"]+(")/, `$1${version}$2`);
    }
  }
  return out;
}
