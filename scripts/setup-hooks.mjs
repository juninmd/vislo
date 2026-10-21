// `npm install` points git at .githooks so every commit message is checked locally (skipped outside a checkout).
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";

if (existsSync(".git")) {
  try {
    execFileSync("git", ["config", "core.hooksPath", ".githooks"]);
  } catch {
    /* git unavailable: CI still enforces the rule */
  }
}
