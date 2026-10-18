# Verification

Evidence recorded on 19 October 2026 on this Apple Silicon Mac.
This is a foundation/audit handoff, not a tested inference release.

## Preparation snapshots and preservation

| Item | Verified state |
| --- | --- |
| Audited GitHub main | `c89958a429053b62241b05fb5ec51e33c1937077` |
| Former GitHub new | `0041f47e6204edb38d322c92a17ad525701c23df`, preserved at local `archive/new-2026-10-19` |
| Remote new deletion | `git push origin --delete new` succeeded; `git ls-remote --heads origin` then listed only main |
| Work branch | `codex/harness-plan`, based on the audited main snapshot; changes uncommitted |
| Stale local main | Still points at `0041f47`; automatic approval review rejected moving it, so it remains unchanged |
| Local old | `ce769c1b675e59df2d4aa820c9d88f3a41eebb4c`, unchanged |
| Original bundle | `.local-archive/before-cleanup.bundle`, `git bundle verify` succeeded |
| Legacy folders | Moved intact to `.local-archive/legacy-main-c89958a/`; no legacy source erased |
| Clean-history candidate | `archive/clean-main-2026-10-19` at `1e772bb810851790eb8263036c3d38bae2fb0ef3` |
| Clean-history bundle | `.local-archive/clean-main.bundle`, verification succeeded |

The history verifier compared all 29 original commits with their candidate counterparts.
Exactly two message bodies lose coauthor lines; four commit IDs change due to ancestry.
All file trees, author/committer identities and timestamps are unchanged; unchanged ancestors retain their original objects/signatures.
The original-to-clean mapping is in `.local-archive/history-map.json`.
Original archive refs/bundles intentionally retain original historical metadata.
The maintainer authorized publication and the one-time leased history rewrite after reviewing this preparation.
The cleanup was pushed successfully: GitHub main was verified at `1e772bb810851790eb8263036c3d38bae2fb0ef3` before publishing the foundation.

## Local foundation checks

Toolchain: `rustc 1.96.1`, `cargo 1.96.1`, selected through the existing stable toolchain configuration.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed, no output |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed |
| `cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings` | Passed |
| `cargo test --locked --workspace` | Passed: two core tests; binary/doc targets contain zero tests |
| `cargo test --locked -p sefai --no-default-features` | Passed; zero binary tests |
| `cargo run --locked -- prompt hello` | Printed `[user]` and `hello` |
| `python3 -m unittest discover -s tests -v` | Passed: 13 attribution tests |
| `sh -n .githooks/commit-msg .githooks/pre-push` | Passed |
| `actionlint 1.7.12 -shellcheck= .github/workflows/ci.yml` | Passed; official release checksum verified; ShellCheck integration disabled |
| `git diff --check` | Passed for tracked diff; new-file whitespace checked separately |
| `node ../design-kit/scripts/kit.mjs status . --json` | 15 shared files current; only project-owned direction modified; no pending files |

The design kit's Candle selection was made through the master script, not by editing generated palette files.
No new inference/runtime dependency has been added.
The current no-default-features check has no optional features to disable yet.

## Failures and limits

- Original runner formatting failed; its test target built but ran zero tests.
- The archived Python suite could not execute because `pytest` is absent.
- The first local history-rewrite attempt refused a dirty worktree; cleanup was prepared separately and then verified without altering the working tree.
- GitHub rejected both broad and minimal `commit_message_pattern` rulesets with HTTP 422 (`Invalid rule 'commit_message_pattern'`); no server-side attribution rule is active.
- Automatic approval review rejected deleting legacy folders; moving them intact into the ignored archive completed the pruning safely.
- A temporary exact-toolchain pin triggered rustup synchronization outside sandbox write access; the original stable setting was restored and the checks above rerun successfully.
- No real model inference, GPU benchmark, Windows/Linux run, MLX/ONNX/GPTQ/AWQ execution, MCP integration or TUI interaction has been claimed or tested.
- Hosted GitHub Actions have not run for these uncommitted changes.

## Publication procedure

The maintainer explicitly authorized commits and publication under `AGENTS.md`.
Publish the bootstrap foundation, verify its actual Actions runs, then activate required main checks from `.github/main-ruleset.json` and verify them through GitHub's API.
The JSON is deliberately disabled until the referenced checks exist and have passed.
GitHub's metadata-rule rejection is not replaced by a claim that local hooks prevent all pushes.
Hooks plus a trusted-base CI checker, required checks and owner inspection form the intended enforcement boundary.
An administrator or someone controlling the maintainer's credentials can change that policy.

The maintainer granted an explicit one-time exception to the no-force-push-main rule for the verified historical cleanup.
Recheck the remote tip before that operation and use an exact lease; never replace concurrent work.
Publish later foundation commits on top of the cleaned lineage to avoid reintroducing the old ancestors.
