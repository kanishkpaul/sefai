# Verification

Evidence recorded on 19 October 2026 on this Apple Silicon Mac and GitHub-hosted Windows, macOS and Linux runners.
This is a published foundation and audit, not a tested inference release.

## Preservation and history cleanup

| Item | Verified state |
| --- | --- |
| Audited original main | `c89958a429053b62241b05fb5ec51e33c1937077` |
| Former GitHub new | `0041f47e6204edb38d322c92a17ad525701c23df`, preserved at local `archive/new-2026-10-19` |
| Remote new deletion | Succeeded after archive/bundle verification |
| Local main | Aligned with the published lineage after proving its former tip equals the preserved new archive |
| Local old | `ce769c1b675e59df2d4aa820c9d88f3a41eebb4c`, unchanged |
| Original bundle | `.local-archive/before-cleanup.bundle`, verification succeeded |
| Legacy folders | Moved intact to `.local-archive/legacy-main-c89958a/`; no legacy source erased |
| Clean-history candidate | `archive/clean-main-2026-10-19` at `1e772bb810851790eb8263036c3d38bae2fb0ef3` |
| Clean-history bundle | `.local-archive/clean-main.bundle`, verification succeeded |

The history verifier compared all 29 original commits with their candidate counterparts.
Exactly two message bodies lose coauthor lines; four commit IDs change due to ancestry.
All file trees, author/committer identities and timestamps are unchanged; unchanged ancestors retain their original objects/signatures.
The mapping is in `.local-archive/history-map.json`.
Original local archive refs/bundles deliberately retain original historical metadata.
The maintainer explicitly authorized the cleanup, which was pushed with an exact lease against the audited original main tip.
GitHub main was verified at the clean tip before the new foundation was published on top.
No further force push was used for foundation publication.

## Local checks

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
| `actionlint 1.7.12 -shellcheck= .github/workflows/ci.yml .github/workflows/attribution.yml` | Passed; official release checksum verified; ShellCheck integration disabled |
| `git diff --check` | Passed; new-file whitespace also checked |
| `node ../design-kit/scripts/kit.mjs status . --json` | 15 shared files current; only project-owned direction modified; no pending files |

The Candle selection was made through the master kit script.
No inference/runtime dependency has been added.
The no-default-features check currently has no optional features to disable.

## GitHub publication and enforcement

The foundation is commit `e6f7b23`; cross-platform CI/hooks are `3ab02ae`; trusted PR policy is `dce6967`.
The [bootstrap CI run](https://github.com/kanishkpaul/sefai/actions/runs/39161976869) passed all four jobs: Windows, macOS, Linux and attribution.
The three platform jobs ran formatting, both Clippy configurations, workspace/minimal tests, diagnostic prompt execution and the 13 attribution tests.
The [separate attribution-policy run](https://github.com/kanishkpaul/sefai/actions/runs/39162149468) passed after splitting the policy into a trusted workflow.
Final documentation/configuration synchronization uses the protected PR path.

[Main ruleset 25357015](https://github.com/kanishkpaul/sefai/rules/25357015) is active and was verified through GitHub's API.
It requires PRs, resolved review threads and `attribution`, `check (ubuntu-latest)`, `check (macos-latest)`, `check (windows-latest)` from GitHub Actions app 15368.
It blocks deletion and non-fast-forward updates and has no bypass actors.
No additional approving reviewer is required for ordinary PRs in this single-maintainer repository; enforcement-file changes still require maintainer inspection.

The attribution workflow uses read-only permissions and `pull_request_target` so its definition and checker come from the base branch.
It reads candidate messages/diffs and PR title/body without executing candidate files.
Local commit/pre-push hooks and disabled automatic tool signatures add earlier checks.
GitHub rejected both broad and minimal `commit_message_pattern` requests with HTTP 422 (`Invalid rule 'commit_message_pattern'`), so no metadata-pattern rule is claimed.
An administrator or someone controlling maintainer credentials can still change repository policy.
No regex detects every possible human-language signature.

## Failures and unverified work

Original runner formatting failed at the model-parameter chain and context-parameter wrapping.
Its test target compiled but reported zero tests.
The archived Python suite failed with `No module named pytest` and was not counted as tested.
A trailing blank line found by diff checking during workflow separation was removed before the policy commit.

No real model inference, GPU benchmark, MLX/ONNX/GPTQ/AWQ execution, MCP integration or TUI interaction has been performed.
Hosted CI verifies the Rust foundation on three operating systems; it does not establish GPU or model support.
The ordered implementation and hardware gates remain in [build-plan.md](build-plan.md).
