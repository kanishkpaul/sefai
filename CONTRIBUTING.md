# Contributing

Read [docs/style.md](docs/style.md), [ARCHITECTURE.md](ARCHITECTURE.md), and the relevant phase in [docs/build-plan.md](docs/build-plan.md) before editing.
Keep changes small and remove unnecessary machinery before adding a replacement.

## Setup

The current foundation needs stable Rust, Git and Python 3 for development guards.
Python is not a product runtime requirement.

```sh
git config core.hooksPath .githooks
cargo run --locked -- prompt hello
```

Engine phases add CMake, a native compiler and the SDK for the selected backend.
Document exact platform prerequisites with the backend when it is implemented.
Do not install every engine SDK for a base build.

## Checks

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings
cargo test --locked --workspace
python3 -m unittest discover -s tests -v
```

CI targets Windows, macOS and Linux.
The no-default-features check is currently equivalent to the base build; it becomes meaningful when optional engine/capability features exist.
Hardware execution and real-model tests require their own explicit jobs and evidence.

## Review and push

Use one topic per PR and one logical change per commit.
An imperative subject names the behavior; the body explains a non-obvious reason and actual validation.
New dependencies need justification; exactly pinned llama.cpp binding upgrades get a standalone commit and benchmark comparison.
Read `git diff` and staged content, check the target branch, and run the commit/pre-push guards before publishing.
Do not force-push main or touch the local old archive branch.
Agents commit only when the maintainer explicitly requests it.

## Attribution policy

Do not add coauthor trailers or coding-tool signatures to commits, PRs, code or documentation.
Tool-specific settings disable automatic signatures; repository instructions cover all contributors.
The commit and pre-push hooks use `scripts/check_attribution.py`; CI checks new commit messages, PR metadata and added signature lines.
Policy explanations, technical product names and required third-party license notices are allowed.
The guard's implementation and fixture files are excluded from the added-content scan so they can describe the patterns they test; changes to those files require maintainer inspection.
Hooks are bypassable; required CI checks and review are needed on GitHub.
A repository administrator remains able to alter protections; no tool configuration can provide an absolute guarantee against its own administrator.

## Publishing the foundation

Bootstrap publication requires maintainer authorization and local validation before the new workflow exists on main.
GitHub rejected commit-message metadata rules with HTTP 422; the proposed JSON is retained as a reference, not an active control.
After committing and publishing the foundation through an explicitly authorized bootstrap, require `attribution`, `check (ubuntu-latest)`, `check (macos-latest)` and `check (windows-latest)` for main.
The trusted-base attribution job assumes its checker already exists on the base branch; the first publication must run the checker locally and verify push CI before enabling the gate.
Require PRs for subsequent changes, disallow force pushes/deletion, and review changes to hooks/checkers/workflows as policy changes.
The prepared main ruleset is disabled in `.github/main-ruleset.json`; activate only after verifying those exact check contexts on GitHub.

Historical cleanup is a separate, verified candidate at `archive/clean-main-2026-10-19`.
Its publication needs an explicit exception to the no-main-rewrite rule and a fresh check of remote main against the audited tip.
Original local archives deliberately retain their original identities and history.
