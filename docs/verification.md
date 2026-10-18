# Verification

Evidence recorded on 19 October 2026 on this Apple Silicon Mac and GitHub-hosted Windows, macOS and Linux runners.
Phase 0 is published; Phase 1 now has local CPU/Metal evidence in the working branch and remains gated on publication and CI.
See the current Phase 1 results below; historical foundation results are retained separately.

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

## Phase 0 local checks

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
At Phase 0 close, no inference/runtime dependency had been added and there were no optional features to disable.

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

At Phase 0 close, no real-model inference or GPU benchmark had been performed.
The existing published CI verifies the foundation; the new model/backend jobs have not yet run remotely.
MLX/ONNX/GPTQ/AWQ execution, MCP integration and TUI interaction remain unimplemented.
The ordered implementation and hardware gates remain in [build-plan.md](build-plan.md).


## Phase 1 local runner evidence

The Phase 1 candidate is on `codex/gguf-runner`, based on published main `f0a274e`; remote CI qualification is pending.
The isolated exact-binding change passed all four Rust checks before runner work and is committed separately as `8753694`.
Its apply-checked patch remains in `.local-archive/phase1-dependency.patch`.
`llama-cpp-2` 0.1.160 was selected by inspecting its actual downloaded source and compiling an isolated probe, rather than copying the retired binding's calls.
The default dependency tree includes GGUF and optional `ctrlc`; the no-default-features binary contains neither, and `sefai-core` remains dependency-free.

| Check | Result |
| --- | --- |
| Four required Rust checks | Passed: formatting, workspace Clippy, minimal Clippy, workspace tests |
| Workspace unit tests | 30 passed: four binary/design, four core, 22 engine; zero failed/ignored |
| Model-feature Clippy with all targets | Passed |
| Dedicated `gguf` target, CPU | 11 passed, zero failed/ignored |
| Dedicated `gguf` target, Metal | 11 passed, zero failed/ignored |
| `gguf_` tokenizer/sampler suite, CPU | Six passed, zero failed/ignored |
| `gguf_` tokenizer/sampler suite, Metal | Six passed, zero failed/ignored; tokenizer metadata override cases remain CPU-only |
| Missing-model environment probe | Failed with exit 101 and `SEFAI_TEST_MODEL is required`, as required; no silent skip |
| Minimal diagnostic/tests/dependency tree | Passed; `[user]` and `hello`; no inference/signal dependency |
| Attribution regressions | 13 passed |
| `actionlint 1.7.12 -shellcheck= .github/workflows/*.yml` | Passed; ShellCheck integration disabled |
| Shared kit | 15 shared files current, project direction modified, no pending files |
| Optimized GGUF build | Passed on macOS arm64 |

Boundary tests cover 511/512/513/3,000 prompt tokens, exact context fit and overflow, zero output, empty/oversize/nul input, incremental Bengali/emoji/combining marks, CPU placement flags, invalid GPU index, cancellation within prefill and after output, write failure, malformed model header and repeated load/unload.
The sampler test observes one frequency-penalty acceptance after the production sampling call; a duplicate acceptance would change the expected logit.
The native vocabulary declares three EOG spellings for this fixture: `<|endoftext|>`, `<|im_end|>` and `<reponame>`.
A forced logit bias makes each stop token terminate the actual generation path without printing it.
Native template rendering is tested with model chat messages and an unsupported tool-renderer failure; full tool-template compatibility remains a harness gate.

The test model is the Apache-2.0 SmolLM2-135M-Instruct Q4_K_M fixture recorded in [fixture metadata](../tests/fixtures/gguf.json).
Its 105,454,432-byte size and SHA256 `2e8040ceae7815abe0dcb3540b9995eaa1fa0d2ca9e797d0a635ae4433c68c2d` were verified before inference.
The product downloads no model; CI deliberately downloads this immutable fixture and verifies it before both named test targets.

### Placement and measurement limits

Host: Apple M5, 16 GB unified memory, macOS 27.0.1, Rust/Cargo 1.96.1; the optimized binary is approximately 9.5 MiB, excluding OS frameworks/models/toolchains.
Native allocation logs confirm CPU mode offloads 0/31 layers, allocates a 90 MiB CPU KV buffer and a 26.69 MiB CPU compute buffer.
The native CPU runtime also allocates a 47.54 MiB repack buffer alongside a 98.87 MiB mapped model buffer; mmap does not make these resident allocations free.
Metal mode offloads 31/31 layers, allocates a 90 MiB Metal KV buffer, a 26.13 MiB Metal compute buffer and a 2.13 MiB CPU compute buffer.
Linked Metal still initializes in CPU mode; zero graphics-driver allocation is not claimed.
GPU/CPU mapped buffers can alias unified memory and must not be added as independent physical allocations.
Disabling CPU repacking is a concrete Phase 9 memory/speed experiment, not an optimization claimed here.

The raw three-repetition measurements are in [the benchmark record](benchmarks/2026-10-19-smollm2.json).
Both runs use the same model, literal `" a"` repeated 512 times, context 4,096, batch 512, microbatch 128, 32 output tokens and four threads.
Weights stay loaded across repetitions and contexts are recreated; filesystem caches are not purged, so these are not cold-disk measurements.
These are observations, not a speedup claim or a comparison with upstream `llama-bench`.
`/usr/bin/time -l` supplies process maximum RSS and peak physical footprint; neither is an isolated harness-overhead measurement or a complete GPU/process-tree accounting.
The later 40/60 MiB overhead targets remain unverified.

### Failed probes and remaining gates

The first model repository URL returned HTTP 401; the public quantization publisher was then resolved and verified.
An assumed upstream LICENSE-file URL returned HTTP 404; the pinned original publisher model card declares Apache-2.0 and is linked in the fixture manifest instead.
The first compile found the pinned penalty sampler requires vocabulary size; the call was corrected to the inspected API.
The first shared-theme Clippy run rejected a nested conditional; it was fixed in the master design kit and imported through its update command.
A sampler-test compile found an engine borrow conflict; it now accesses the backend through the borrowing model.
An EOG fixture expectation initially omitted `<reponame>`; enumeration identified the third native stop token and the corrected test covers it explicitly.
A text-dependent termination test passed on CPU but reached the output limit on Metal; it was replaced by deterministic forced-stop tests, rather than assuming identical model replies across backends.
An initial hardware metadata query timed out, and a later sandboxed `sysctl` read was denied; the host description comes from the successful hardware overview and OS-version query.

PR #6 started the Windows/Linux CPU/model jobs and CUDA/Vulkan compile jobs.
The first Vulkan job failed with `Could not find a package configuration file provided by "SPIRV-Headers"`; its SDK package was added before rerunning.
Push runs were restricted to main to avoid duplicate native builds alongside PR events.
The remaining remote results are still pending.
No CUDA, Vulkan, NVIDIA or AMD execution has been tested on this Mac; no SDK for those backends was silently installed.
The maintainer authorized commits, branch publication and a PR; the required checks plus backend-build jobs must pass before Phase 1 closes.
The shared master kit has an uncommitted one-conditional Rust lint fix; its generated local counterpart is included here, without committing unrelated master-repository work.
No Phase 2 feature has been started.
