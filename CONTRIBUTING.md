# Contributing

Read [docs/style.md](docs/style.md), [ARCHITECTURE.md](ARCHITECTURE.md), and the relevant phase in [docs/build-plan.md](docs/build-plan.md) before editing.
Keep changes small and remove unnecessary machinery before adding a replacement.

## Setup

The GGUF build needs stable Rust, CMake, a C/C++ compiler and libclang; development guards also use Git and Python 3.
Python is not a product runtime requirement.

```sh
git config core.hooksPath .githooks
cargo run --locked -- prompt hello
```

The [runner guide](docs/runner.md) documents native prerequisites and backend SDK requirements.
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
The no-default-features binary excludes both inference and the signal handler.
CI downloads and verifies a pinned GGUF separately; ordinary workspace tests need no model or network.
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
If a push's previous commit is unavailable after a rebase, CI checks all reachable commit messages and the full current tree.
Policy explanations, technical product names and required third-party license notices are allowed.
The guard's implementation and fixture files are excluded from the added-content scan so they can describe the patterns they test; changes to those files require maintainer inspection.
Hooks are bypassable; required CI checks and review are needed on GitHub.
A repository administrator remains able to alter protections; no tool configuration can provide an absolute guarantee against its own administrator.

## Publishing the foundation

Bootstrap publication requires maintainer authorization and local validation before the new workflow exists on main.
GitHub rejected commit-message metadata rules with HTTP 422; trusted PR checks enforce the policy instead.
Main requires `attribution`, `check (ubuntu-latest)`, `check (macos-latest)` and `check (windows-latest)` from GitHub Actions.
The read-only `pull_request_target` attribution workflow and checker come from the trusted base branch; it never executes candidate code.
The bootstrap was validated locally and passed CI before protection was activated.
Require PRs for subsequent changes, disallow force pushes/deletion, and review changes to hooks/checkers/workflows as policy changes.
The active ruleset is recorded in `.github/main-ruleset.json` and [GitHub rules](https://github.com/kanishkpaul/sefai/rules/25357015).
It requires PRs, all four checks and resolved review threads, blocks deletion/force pushes, and has no bypass actors.

Historical cleanup is a separate, verified candidate at `archive/clean-main-2026-10-19`.
It was published with a maintainer-authorized one-time exception and an exact lease against the audited main tip.
Future main rewrites remain prohibited.
Original local archives deliberately retain their original identities and history.
