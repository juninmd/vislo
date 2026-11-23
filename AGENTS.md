# AGENTS.md

Vislo is a desktop image viewer: a Tauri 2 app with a TypeScript frontend (no framework) and a Rust backend. Read this before changing anything.

## Stack
- Frontend: TypeScript, Vite, no UI framework; tests with vitest.
- Backend: Rust in `src-tauri/` (Tauri 2), workspace with `vislo-core`; Windows-first, OS calls behind `#[cfg(windows)]` with a stub elsewhere.
- Packaging: NSIS installer and portable exe (Windows).

## Commands
- Frontend: `npm ci`, `npm test` (vitest), `npm run build` (`tsc --noEmit && vite build`).
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml --workspace`
- Lint: `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace -- -D warnings` (on Windows, as in CI)
- Audit: `cargo audit --file src-tauri/Cargo.lock` and `npm audit --omit=dev --audit-level=high` (the `audit` job; cargo-audit 0.22.2)
- All checks locally: `pwsh scripts/ci-local.ps1` (runs the `check` steps)
- Screenshots: `npm run screenshots`

## Structure
- `src/`: frontend; `src/i18n/` holds all user-visible text (one file per language)
- `src-tauri/src/`: backend; `update/` (self-updater), `settings.rs`, `splash/`, `scheme.rs`, and more
- `src-tauri/vislo-core/`: image decoding core, with test data in `src/test_data/`
- `scripts/`: release planner, commit checker, screenshots, perf
- `.githooks/`: `commit-msg` hook (installed by `npm install`)
- `.github/workflows/`: `ci`, `release`, `bench`, `sast`, `security`, `validate`
- `assets/screenshots/`: README screenshots

## Conventions
- **Semantic commits are mandatory** for every commit message and every PR title (Conventional Commits):
  - Types: `feat` (minor), `fix`, `perf`, `refactor`, `style`, `docs`, `test`, `build`, `ci`, `chore`, `revert` (patch). A `!` or a `BREAKING CHANGE:` footer means major (minor while `0.x`).
  - Type and scope are lower case (`feat(zoom): ...`); the subject is lower case, imperative, no trailing period, first line under 100 characters.
  - One logical change per commit.
  - Enforced by `scripts/check-commits.mjs` in the `commit-msg` hook, the `commits` CI job, and the release planner. Never bypass with `--no-verify`.
  - Good: `feat(update): offer a one-click update from the release feed`. Bad: `Update stuff`, `Fixed bug.`, `feat: Add Zoom.`
- **Every commit on `main` is released.** `release.yml` plans the version from commits since the last `v*` tag, stamps it into the manifests, builds, and publishes. Never edit the version by hand, never create tags by hand, and never push to `main` a commit you would not want shipped. Choose the type honestly; it sets the bump.
- Updater contract: running copies read `releases/latest` of `juninmd/vislo`, download `Vislo_<version>_x64-setup.exe`, verify it against `SHA256SUMS.txt`, and run it silently. Keep those asset names stable, or update the updater (`src-tauri/src/update/`) and its tests with them.
- Workflow: work on a branch, open a PR, keep it green. Do not rewrite shared history.
- Do not mention or copy third-party products' internals in code, comments, docs or commits. Describe what Vislo does and why, in its own words.
- Code: match the surrounding style. Comments explain *why*, not *what*. Pure logic goes in its own module with unit tests (`*.test.ts`, `*.rs` helpers).
- i18n: add each user-visible string to `en.ts` and to **every** language file; `i18n.test.ts` fails on a missing key or a lost `{placeholder}`. Static markup uses `data-i18n*`. Rust error text and the tray menu stay in English.
- Settings are a contract: a new field in `src-tauri/src/settings.rs` needs `#[serde(default)]`, a matching field in `src/api.ts`, and a test that old settings files still load.
- Security posture is documented in the README; keep it intact (asset scope, `vislo://` canonicalisation, strict CSP, HTTPS-only downloads from this repo's releases, checksum before running anything).
- UI changes: when the look changes, regenerate with `npm run screenshots` and commit the PNGs in `assets/screenshots/`.

## Gotchas
- CI gates on PRs: `ci` (`commits`, `check`, `audit`, `actionlint`) and `Validate`. `Security Scan` runs on `main` and weekly. `Validate` and `Security Scan` are reusable workflows from `juninmd/base-actions`.
- `release.yml` calls `ci`, so the `audit` gate blocks releases too. A new advisory stops the next release until the dependency is upgraded, or until a reviewed ignore with a reason and expiry is added to `.cargo/audit.toml`. Never delete the gate to ship.
- Third-party actions are pinned by commit SHA with a `# vX.Y.Z` comment. `dtolnay/rust-toolchain` is pinned to a branch-head SHA that automation does not track: refresh it by hand. `actionlint` is pinned to 1.7.12.
- The README says Dependabot (`.github/dependabot.yml`) bumps the action SHAs, but that file is absent from this tree. Until it is restored, bump SHAs by hand.
- Every job sets `timeout-minutes`. Jobs that call a reusable workflow cannot set it (GitHub rejects the key); the limit lives in the called workflow.

<!-- base-actions-main-policy -->
## GitHub Actions — padrão obrigatório (@main)

- Referencie as actions internas sempre pelo branch, nunca por SHA:
  - `uses: juninmd/base-actions/<action>@main`
  - `uses: juninmd/base-actions/.github/workflows/<arquivo>.yml@main`
- Não fixe `@<sha>` em `juninmd/base-actions`: a atualização é centralizada no
  `main` e passa a valer no próximo run.
- PRs cujos checks não iniciam por billing/spending limit são mesclados pelo bot central
  `juninmd/base-actions/.github/workflows/auto-merge-billing-blocked.yml`.
  Para impedir o merge automático de um PR, aplique a label `do-not-merge`.
