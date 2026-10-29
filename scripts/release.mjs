// Release helper used by .github/workflows/release.yml.
//   node scripts/release.mjs plan    -> JSON { version, bump, notes, previous } for the commits since the last tag, or {} if none
//   node scripts/release.mjs stamp X -> writes version X into package.json, package-lock.json, Cargo.toml/lock, tauri.conf.json
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { latestTag, plan, stamp } from "./release-lib.mjs";

const REPO = process.env.GITHUB_REPOSITORY ?? "juninmd/vislo";
const git = (...args) => execFileSync("git", args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();

function lastTag() {
  return latestTag(git("tag", "--merged", "HEAD", "--list", "v[0-9]*").split(/\s+/)); // null: nothing released yet
}

function commitsSince(tag) {
  const SEP = "\u0001";
  const raw = git("log", "--no-merges", `--format=%H${SEP}%B${SEP}%x00`, tag ? `${tag}..HEAD` : "HEAD");
  return raw
    .split("\0")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [hash, message] = entry.split(SEP);
      return { hash, message: message.trim() };
    });
}

const FILES = ["package.json", "package-lock.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock", "src-tauri/tauri.conf.json"];

const [command, arg] = process.argv.slice(2);
if (command === "plan") {
  const tag = lastTag();
  const base = tag ?? JSON.parse(readFileSync("package.json", "utf8")).version;
  const result = plan(commitsSince(tag), base, { repo: REPO });
  console.log(JSON.stringify(result ? { ...result, previous: tag } : {}));
} else if (command === "stamp" && /^\d+\.\d+\.\d+$/.test(arg ?? "")) {
  const files = Object.fromEntries(FILES.map((f) => [f, readFileSync(f, "utf8")]));
  for (const [name, text] of Object.entries(stamp(files, arg))) writeFileSync(name, text);
  console.log(`stamped ${arg}`);
} else {
  console.error("usage: release.mjs plan | stamp <x.y.z>");
  process.exit(2);
}
