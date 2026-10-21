# AGENTS.md

Instructions for AI agents and human contributors working on Vislo. Read this before changing anything.

## Semantic commits are mandatory

Every commit message **and every pull request title** follows [Conventional Commits](https://www.conventionalcommits.org):

```
<type>(<optional scope>)<!>: <subject>

<optional body: what and why, wrapped at ~100 columns>

<optional footer, e.g. BREAKING CHANGE: ...>
```

| Type | Use for | Release effect |
|---|---|---|
| `feat` | a user-visible capability | **minor** |
| `fix` | a bug fix | patch |
| `perf` | a speed or memory improvement | patch |
| `refactor`, `style` | code changes with no behaviour change | patch |
| `docs`, `test`, `build`, `ci`, `chore` | everything else | patch |
| any type + `!`, or a `BREAKING CHANGE:` footer | an incompatible change | **major** (minor while the version is `0.x`) |
| `revert` | undoing an earlier commit | patch |

Rules, enforced by `scripts/check-commits.mjs`:

* the type is lower case and one of the types above; the scope (optional) is lower case: `feat(zoom): …`;
* the subject starts in lower case, is imperative (`add`, not `added`), has no trailing period, and the whole
  first line stays under 100 characters;
* one logical change per commit.

Enforcement happens in three places: the `commit-msg` hook in `.githooks/` (installed by `npm install`, which
sets `core.hooksPath`), the `commits` job of the CI workflow (PR title and each commit), and the release
planner, which reads the same messages. Never bypass the hook with `--no-verify`.

Good: `feat(update): offer a one-click update from the release feed` · `fix(ui): keep the toast above the filmstrip`
Bad: `Update stuff`, `Fixed bug.`, `feat: Add Zoom.`

## Every commit on `main` is released

`.github/workflows/release.yml` runs on each push to `main`:

1. runs the full CI (frontend tests, build, `cargo test`, clippy);
2. plans the next version from the commits since the last `v*` tag (`scripts/release.mjs plan`);
3. stamps it into `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `Cargo.lock` and `tauri.conf.json`;
4. builds the NSIS installer and the portable exe, and writes `SHA256SUMS.txt`;
5. commits `chore(release): vX.Y.Z [skip ci]`, tags `vX.Y.Z` and publishes a GitHub release with grouped notes.

So: **never edit the version by hand**, never create tags by hand, and never push to `main` a commit you would not
want shipped. The commit type decides the bump, so choose it honestly.

Running copies of Vislo find the release through the built-in updater (`src-tauri/src/update.rs`): it reads
`releases/latest` of `juninmd/vislo`, downloads `Vislo_<version>_x64-setup.exe`, verifies it against
`SHA256SUMS.txt` and runs it silently. Keep these asset names stable, or update the updater and its tests with them.

## Workflow

* Work on a branch, open a pull request, keep it green. Do not rewrite shared history.
* Do not mention or copy third-party products' internals in code, comments, docs or commit messages. Describe
  what Vislo does and why, in its own words.
* Before pushing, run what CI runs:

  ```sh
  npm ci && npm test && npm run build
  cargo test --manifest-path src-tauri/Cargo.toml
  cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings   # on Windows, as in CI
  ```

* UI changes: regenerate the README screenshots with `npm run screenshots` and commit the PNGs in
  `assets/screenshots/` when the look changes.

## Code conventions

* Match the surrounding code: comment density, naming, idiom. Comments explain *why*, not *what*.
* Frontend: TypeScript, no framework, one small module per concern, pure logic split out and unit-tested
  (`*.test.ts`, vitest). Backend: Rust, `#[cfg(windows)]` for OS calls with a stub elsewhere, pure helpers tested.
* Settings are a contract: a new field in `src-tauri/src/settings.rs` needs a default (`#[serde(default)]`), a
  matching field in `src/api.ts`, and a test that old settings files still load.
* Security posture is in the README; keep it intact (asset scope, `vislo://` canonicalisation, strict CSP,
  HTTPS-only downloads from this repository's releases, checksum before running anything).
