// Conventional Commits parsing shared by the commit checker and the release tooling.

export const TYPES = ["feat", "fix", "perf", "refactor", "docs", "style", "test", "build", "ci", "chore", "revert"];

const HEADER = new RegExp(`^(${TYPES.join("|")})(?:\\(([a-z0-9][a-z0-9._/-]*)\\))?(!)?: (\\S.*)$`);
const MAX_HEADER = 100;

/** Merge commits and git's own `Revert "..."` titles are produced by tools, not typed by people. */
const generated = (header) => /^Merge (pull request|branch|remote-tracking branch) /.test(header) || /^Revert ".+"$/.test(header);

/** Returns `{ type, scope, breaking, subject, body }`, or `null` when the message is not a semantic commit. */
export function parseCommit(message) {
  const [header = "", ...rest] = message.replace(/\r/g, "").split("\n");
  const m = HEADER.exec(header.trim());
  if (!m) return null;
  const body = rest.join("\n").trim();
  return {
    type: m[1],
    scope: m[2] ?? null,
    breaking: m[3] === "!" || /^BREAKING[ -]CHANGE: /m.test(body),
    subject: m[4],
    body,
  };
}

/** A list of human-readable problems; empty when the message is acceptable. */
export function problems(message) {
  const header = message.replace(/\r/g, "").split("\n")[0].trim();
  if (generated(header)) return [];
  const errors = [];
  const parsed = parseCommit(message);
  if (!parsed) {
    errors.push(`"${header}" is not a semantic commit: use <type>(<scope>): <subject>, type one of ${TYPES.join(", ")}`);
    return errors;
  }
  if (header.length > MAX_HEADER) errors.push(`the first line is ${header.length} characters; keep it under ${MAX_HEADER}`);
  if (/\.$/.test(parsed.subject)) errors.push("the subject must not end with a period");
  if (/^[A-Z]/.test(parsed.subject) && !/^[A-Z]{2,}/.test(parsed.subject)) errors.push("start the subject in lower case");
  return errors;
}
