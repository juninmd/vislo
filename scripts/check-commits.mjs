// Enforces semantic commits (see AGENTS.md).
//   node scripts/check-commits.mjs --file .git/COMMIT_EDITMSG   (commit-msg hook)
//   node scripts/check-commits.mjs --message "feat: add x"      (e.g. a pull request title)
//   node scripts/check-commits.mjs --range origin/main..HEAD    (CI)
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { problems } from "./commits.mjs";

const args = process.argv.slice(2);
const value = (flag) => args[args.indexOf(flag) + 1];

let messages;
if (args.includes("--file")) {
  // `#` lines are git's template comments
  messages = [readFileSync(value("--file"), "utf8").split("\n").filter((l) => !l.startsWith("#")).join("\n").trim()];
} else if (args.includes("--message")) {
  messages = [value("--message")];
} else if (args.includes("--range")) {
  const raw = execFileSync("git", ["log", "--no-merges", "--format=%B%x00", value("--range")], { encoding: "utf8" });
  messages = raw.split("\0").map((m) => m.trim()).filter(Boolean);
} else {
  console.error("usage: check-commits.mjs --file <path> | --message <text> | --range <a..b>");
  process.exit(2);
}

let failed = false;
for (const message of messages) {
  const errors = problems(message);
  if (errors.length === 0) continue;
  failed = true;
  console.error(`✗ ${message.split("\n")[0]}`);
  for (const e of errors) console.error(`    - ${e}`);
}
if (failed) {
  console.error("\nCommits follow Conventional Commits: feat(scope): add a thing | fix: correct a bug | docs: ... (see AGENTS.md)");
  process.exit(1);
}
console.log(`✓ ${messages.length} commit message(s) follow Conventional Commits`);
