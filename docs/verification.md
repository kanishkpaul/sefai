# Verification

Evidence recorded on 19 October 2026 on this Apple Silicon Mac and GitHub-hosted Windows, macOS and Linux runners.
Phases 0 and 1 are complete and published.
Phase 1 has local CPU/Metal evidence, passing Windows/macOS/Linux CPU-model CI and successful CUDA/Vulkan compilation.
Phase 2 merged through PRs #8–#11 as `33f1a8d2172db93eb0b8cb1759fa63c84d62d6ee` after the three platform checks and attribution passed.
Phase 3 is merged through PR #12 with passing CI; its local evidence and the subsequent Phase 4 acceptance records are below.
Physical cross-platform terminal and GPU execution qualification remain Phase 10 gates.
CUDA/Vulkan execution remains unverified.
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
At Phase 0 close, published CI verified only the foundation; the Phase 1 model/backend jobs subsequently passed as recorded below.
At Phase 0 close, MLX/ONNX/GPTQ/AWQ execution, MCP integration and TUI interaction were unimplemented.
The ordered implementation and hardware gates remain in [build-plan.md](build-plan.md).


## Phase 1 local runner evidence

The Phase 1 runner was developed on `codex/gguf-runner` from published main `f0a274e` and merged through [PR #6](https://github.com/kanishkpaul/sefai/pull/6) as `6e75a08`.
[PR #7](https://github.com/kanishkpaul/sefai/pull/7) subsequently updated pinned CI actions and repaired attribution checks, merging as `9112427`.
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
Disabling CPU repacking is a concrete Phase 10 memory/speed experiment, not an optimization claimed here.

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
The first macOS build/lint/unit checks passed, but model tests failed because the relative fixture path resolved from the crate directory.
CI now passes the absolute workspace fixture path; the corrected platform and backend jobs subsequently passed.
No CUDA, Vulkan, NVIDIA or AMD execution has been tested on this Mac; no SDK for those backends was silently installed.
The maintainer authorized publication and merged both PRs after their checks passed; the Phase 1 publication and CI gates are closed.
The shared master kit has an uncommitted one-conditional Rust lint fix; its generated local counterpart is included here, without committing unrelated master-repository work.
At the Phase 1 closeout, no Phase 2 feature had been started.


## Phase 1 closeout

Phase 1 closed on 19 October 2026.
The [runner platform run](https://github.com/kanishkpaul/sefai/actions/runs/39167302843) and [runner backend run](https://github.com/kanishkpaul/sefai/actions/runs/39165462628) passed before PR #6 merged.
The following checks also passed on exact PR #7 head `750702d0ffb57b2b16bb0a194e0e53a24197c7e4`; the merged tree at `9112427` was verified identical to that tested head.

| Check | Result and evidence |
| --- | --- |
| Windows, macOS and Linux | [All platform jobs passed](https://github.com/kanishkpaul/sefai/actions/runs/39245923416), including Rust checks, 30 workspace unit tests, minimal-build checks, 16 attribution regressions and both pinned-model CPU test targets |
| CUDA and Vulkan | [Both compile jobs passed](https://github.com/kanishkpaul/sefai/actions/runs/39245923461); no GPU execution is implied |
| Push attribution policy | [Passed](https://github.com/kanishkpaul/sefai/actions/runs/39245893523) |
| Trusted PR attribution policy | [Passed](https://github.com/kanishkpaul/sefai/actions/runs/39245923411) |
| Local CI maintenance validation | Formatting, both Clippy configurations, 30 Rust tests, 16 attribution tests, prompt diagnostic, actionlint 1.7.12 with ShellCheck disabled and diff whitespace checks passed |

PR #7 pins checkout 7.0.1 and cache 6.1.0 and checks all reachable messages plus the current tree when a rebased push's previous commit is unavailable.
PRs #3 and #4 were closed as superseded after the replacement passed.
The results above describe pre-merge PR validation; post-merge workflow runs were not monitored.
Phase 2 may begin with the versioned Markdown workspace; no Phase 2 implementation is included in this closeout.
Full CUDA/Vulkan device execution, footprint budgets and the remaining format adapters retain their later-phase gates.

## Phase 2 step 1 — schema and loading

Schema 1 rejects unknown/duplicate fields, unsupported versions and invalid ranges without modifying Markdown.
Ten model-free workspace tests cover body/config separation, ordered composition, profile precedence, selected-only skills, bounded ordered recall and identity/version errors.
All four required Rust checks, the no-default-features test suite, 16 attribution tests and the `prompt hello` diagnostic pass locally on macOS.
The first validation attempt found a no-default-features unused import; it was feature-gated and both Clippy configurations were rerun successfully.
The two new dependencies are `serde` (typed field validation) and deserialize-only `serde-saphyr` (bounded YAML parsing); the standard library has no YAML parser.
No engine runtime dependency or native binding version changes.
At the step-1 checkpoint, cross-platform CI had not been observed; the Phase 2 stack subsequently passed the required platform checks and merged.

## Phase 2 step 2 — safe persistence

Same-directory temporary files, sync-before-rename, nonblocking standard-library OS locks and exact expected-file comparisons protect explicit writes.
Seven new model-free tests cover invalid replacement rollback, stale snapshots, literal session roles, bounded tail count, truncated records, non-destructive initialization and memory edit/forget.
Session reads and writes are bounded to 64 MiB on disk with a 1 MiB decoded tail, without a hidden database.
The CLI offers explicit memory operations, validated file import and session append/tail/resume.
Automatic generation persistence remains later harness work.
All four Rust checks, the no-engine tests, attribution regression tests and the prompt diagnostic are required before this step's commit.
Windows rename and process-lock behavior will be exercised by the existing three-OS CI matrix; local execution is macOS only.

## Phase 2 step 3 — persistence qualification

Local macOS passes 72 Rust tests: 36 binary tests, 10 CLI integration tests, 4 core tests and 22 engine tests.
The no-engine configuration passes the same 46 workspace/terminal/CLI tests without loading an engine.
Sixteen attribution regression tests and all four required Rust checks pass; `prompt hello` emits the expected user message.
The schema fixture covers config, system, preferences, a GGUF profile, a selected skill, memory, an interrupted session and an inert MCP definition.

The expanded suite covers Unicode workspace paths, CRLF, restart visibility, portable IDs, unknown nested keys, alias rejection, front matter/file/record bounds, every incomplete prefix of a session record, injected read failure, edits during staging, last-good-byte preservation, crash-leftover files and lock release after terminating a separate process.
Unix tests reject escaped symlink reads and symlink-parent writes.
Windows-specific tests exercise a junction escape and an exclusive sharing violation; they have not been executed locally.
A portable integration test replaces an existing file through the real CLI on all three operating systems.
The existing CI matrix automatically runs these tests with both default and no-engine configurations; remote results are deliberately not polled.
A malformed manual edit remains on disk with a visible error; callers cannot silently overwrite it or promote a partial temporary file.
YAML tags and merge keys are rejected, and ambiguous scalar-to-string coercion is disabled.

## Phase 2 step 4 — closeout and consistency

The fourth PR closes documentation gaps and adds small runner/composition regressions: explicit CPU selection clears inherited GPU placement, CLI flags override Markdown values, raw mode cannot silently discard instructions or history, expected snapshots must be separate copies, and prompt sections are budgeted before copying.
Exact-size CRLF records have the same decoded byte limit as LF records.
Runner defaults are computed once per request, and invalid raw-mode requests fail before loading weights.
No native adapter or binding upgrade is included.

Final local checks and model evidence are summarized in [phase2-closeout.md](phase2-closeout.md).
The local GGUF smoke used the hash-verified `tests/fixtures/gguf.json` model on CPU with context 512, batch 64, microbatch 32, output 4 and two threads.
Markdown system text and the model profile produced an 18-token prompt and a successful reply.
A second benchmark probe explicitly overrode output to zero and returned `generated_tokens=0` with `OutputLimit`; this is an override diagnostic, not a throughput measurement.
No new RAM/VRAM claim or broader architecture qualification follows from these smoke tests.
Remote CI has not been polled or declared green.

## Phase 3 — terminal home, local acceptance

Recorded on 20 October 2026 on `codex/phase3-terminal`, based on merged main `33f1a8d2172db93eb0b8cb1759fa63c84d62d6ee`.
The default command now opens the terminal conversation; `chat --plain` provides line-oriented input without terminal controls.
One bounded worker owns inference, context/prefix reuse and Markdown I/O; model weights remain loaded across turns.
The [terminal guide](tui.md) records controls, dependency justification, byte/count bounds and explicit recovery behavior.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed |
| `cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings` | Passed |
| `cargo test --locked --workspace` | Passed: 105 tests — 61 binary, 3 chat CLI, 12 workspace CLI, 4 core and 25 engine |
| `cargo test --locked -p sefai --no-default-features` | Passed: 53 tests; no TUI/native engine dependency is activated by this build |
| `python3 -m unittest discover -s tests -v` | Passed: 16 attribution regressions |
| `cargo run --locked -- prompt hello` | Printed one `[user]` message containing `hello` |
| Dedicated GGUF target with `SEFAI_TEST_MODEL` and CPU backend | Passed: 13 tests, including cached/fresh equality after prompt edits and cancellation invalidation |
| Local PTYs | Seven scenarios passed; each exited 0 and restored the original terminal attributes |
| Cell snapshots and layout regressions | First run, streaming, context full and overlay snapshots; narrow/tiny terminals, long Unicode text, grapheme editing/cursor placement, palette polarity and no-color cells passed |
| Recovery regressions | Rejected requests restore the composer; concurrent session edits preserve disk bytes; an interrupted turn persists both records and outcome |
| `git diff --check` | Passed |

The real-model fixture was rehashed and matched `2e8040ceae7815abe0dcb3540b9995eaa1fa0d2ca9e797d0a635ae4433c68c2d`.
The two-turn PTY session resumed its exact saved records; the second request reused 10 verified prompt tokens.
Cancellation during prefill left an explicit interrupted session, and native tests separately cover cancellation after output.
See [normalized terminal transcripts](evidence/phase3/terminal-transcripts.md), [scenario results](evidence/phase3/terminal-results.json) and the cell fixtures under `tests/fixtures/tui/`.

Phase 3 has not been committed, pushed or qualified by remote CI at this checkpoint.
Windows/Linux real terminal lifecycle behavior, GPU execution and a full hardware matrix remain unverified for these changes.
The existing three-OS CI automatically runs the new ordinary tests and the dedicated pinned CPU fixture target once published.
No checks or workflow definitions were weakened or bypassed; the llama.cpp binding stays exactly `0.1.160`, and core stays dependency-free.
The AGENTS.md edit changes only the phase-status sentence and remains subject to maintainer inspection before publication.
No optional MLX/ONNX/GPTQ/AWQ runtime, MCP process, tool permission authority or automatic memory extraction was added.
This work claims tested behavior and verified prefix identity, not a throughput or RAM/VRAM improvement.
At that historical checkpoint the Phase 3 gate remained open and Phase 4 had not started.
PR #12 subsequently merged; the Phase 4 checkpoint below supersedes that publication status.

## Phase 3 CI correction — feature-gated stop-token tests

The initial [PR #12 CPU run](https://github.com/kanishkpaul/sefai/actions/runs/39340977976) failed on macOS, Linux and Windows when compiling `--features model-tests --lib gguf_`.
The 13-test public GGUF integration target passed on macOS and Linux before that compilation failure.
The cache refactor had removed `generate_with_sampler`, but the feature-gated every-stop-token regression still called it.
The initial local qualification omitted this separate six-test library target; passing the public integration target did not establish that coverage.

The cache-aware generation loop now accepts the existing sampler factory, and the stop-token regression supplies a fresh bounded cache to that loop.
The production sampler and its initialization order are unchanged; all three fixture stop tokens still must halt generation before emitting text.
No test, feature gate, workflow or binding pin was removed or weakened.

Local correction checks include the four required Rust checks, feature-enabled engine Clippy, attribution regressions, the diagnostic prompt and both CPU model-test targets:

```sh
cargo clippy --locked -p sefai-engine-llama --features model-tests --all-targets -- -D warnings
cargo test --locked -p sefai-engine-llama --features model-tests --test gguf
cargo test --locked -p sefai-engine-llama --features model-tests --lib gguf_
```

The model commands require `SEFAI_TEST_MODEL` pointing to the pinned fixture and `SEFAI_TEST_BACKEND=cpu`.
The library target passes six tests, including forced end-of-generation tokens, tokenizer BOS behavior, sampler acceptance and unsupported-template rejection.
The public integration target passes 13 tests; the workspace passes 105 tests, and attribution passes 16 tests.
PR #12 was closed during correction; new remote results remain unverified until the reopened PR completes CI.

At the correction checkpoint, the original Vulkan build passed and CUDA was still running against the previous commit.
Reopening PR #12 triggers fresh three-OS CPU checks and CUDA/Vulkan compilation against the correction; those remote results are not yet claimed.
CUDA compilation requires the Linux CUDA toolchain from the existing workflow and cannot be qualified on this macOS host.


## Phase 4 — bounded harness, local acceptance

Implemented on `codex/phase4-harness`, based on the Phase 3 merge `9767732a9db9009d53098397dd57155669b9adb9`.
Publication, PR review and GitHub Actions for this branch remain pending; no CI polling was performed.
The [harness guide](harness.md) records the implemented contract and its limits.

The binary now drives bounded generations, strict opt-in SmolLM2 proposals, one exact-action approval at a time, local memory read/create, untrusted tool results, and a generation follow-up.
Scope is captured before composing untrusted instructions; model output cannot add a server/tool/path capability.
Before dispatch, the worker revalidates the exact proposal and scope and atomically journals dispatch intent.
Failed checkpoints block execution; dispatched write errors/cancellation remain uncertain and are never automatically retried.
Session audit records remain separate from conversation roles, and known credential values are redacted before record serialization.
MCP transport, arbitrary file tools, shell execution, implicit memory extraction and regenerate are absent.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed |
| `cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings` | Passed |
| `cargo test --locked --workspace` | 151 passed: 107 binary unit, 3 chat CLI, 12 workspace CLI, 4 core, 25 engine |
| `cargo test --locked -p sefai --no-default-features` | 53 passed: 43 binary unit and 10 workspace CLI |
| `cargo clippy --locked -p sefai --features harness-model-tests --all-targets -- -D warnings` | Passed |
| `python3 -m unittest discover -s tests -v` | 16 passed |
| `cargo run --locked -- prompt hello` | Plain `[user]` / `hello`, no banner |
| Native GGUF `--features model-tests --test gguf`, CPU | 13 passed |
| Native GGUF `--features model-tests --lib gguf_`, CPU | 6 passed, including all forced fixture stop tokens |
| `--features harness-model-tests --test harness_gguf`, CPU | 3 passed, required tool fixture present |
| Real macOS terminal approval/cancellation cases | 5 passed; exit 0 and original terminal attributes restored |
| Minimal dependency graph | No engine, TUI or JSON runtime dependency |
| `git diff --check` | Passed |

The native regressions use the unchanged 135M fixture and SHA256 from `tests/fixtures/gguf.json`.
The new tool fixture is publisher-owned SmolLM2-1.7B-Instruct Q4_K_M, revision `2d4a76a30b4af41ecd395c35725ac11688d4cfe4`, 1,055,609,536 bytes, SHA256 `decd2598bc2c8ed08c19adc3c8fdd461ee19ed5708679d1c54ef54a5a30d4f33`.
Both size and digest were verified against the downloaded file and the immutable Hugging Face LFS metadata.
Its manifest includes the Apache-2.0 license references.
Model weights stay in the ignored local archive; no weights are committed.
The Linux CPU CI job now fetches, size/hash-verifies and runs this dedicated fixture target in addition to the existing runner gates.
The JSON decoder is optional under `gguf`; it is the only new direct dependency, with its formatter dependency recorded in `Cargo.lock`.
The native binding pin remains exactly `=0.1.160`.

Deterministic fake generation uses the production turn driver with an injected clock.
It covers ordinary replies, approved results, read errors, denied/stale/replayed approvals, repeated writes, step/time/token/tool/transcript/result/argument limits, cancellation and fresh-turn recovery.
Production dispatcher tests additionally prove that a failed journal checkpoint performs no memory write, successful dispatch records the real outcome, and a dispatched write error is persisted as uncertain.
Filesystem tests cover frozen read scope, create-only writes, malformed/oversize memory and exact checkpoint replacement.
Session tests preserve quoted role framing and exclude audit records from model context.
Terminal tests cover approval Enter-denial, paste isolation, one-shot approval consumption, timeout dismissal and the generated warning border.
Known literal and JSON-escaped credential values are tested, including refusal of credentials in a proposed memory ID.
Malformed audit JSON conservatively preserves uncertainty on recovery.

### Real-model results and retained failures

With CPU, 2,048 context tokens, 256 output tokens per step and two threads, the read case proposed `read_memory` with ID `note`, waited for approval, read the actual file and answered “The project marker is cobalt.”
The source memory contained that marker; no answer was inserted by the harness.
The approved write created only `memory/rule.md` with `Keep answers concise.`; the denied write created no memory.
The write and denial cases then requested the same tool again; the runtime emitted `tool call already attempted; automatic retry refused` and recorded the incomplete turn instead of dispatching it.
This model behavior remains a documented limitation, not a successful follow-up claim.
The read case completes the full real-model tool/result/answer path; permission tests assert actual files and dispatch counts rather than trusting confirmation prose.

Initial experiments framed results as ordinary user data and the model repeated the read.
A distinct tool-result role plus an explicit answer-stage instruction completed the read path; write repetition still occurs and remains bounded.
A dedicated-model Clippy pass also caught undiagnosed `unwrap` calls in fixture helpers; those now use operation-specific `expect` messages and the target passes strict Clippy.
A final reporting review corrected stale statistics after a native error: unavailable failed-step token counts are labeled unknown rather than reported as exact zero.
No performance or memory reduction is claimed from these acceptance runs.

The [terminal transcripts](evidence/phase4/terminal-transcripts.md) and [results](evidence/phase4/terminal-results.json) record approved read/create, Enter denial, Esc denial and Ctrl+C during approval.
All five cases kept the UI usable and restored terminal state on exit.
Physical Windows/Linux terminal lifecycle, CUDA/Vulkan/Metal execution for this phase, crash/power-loss durability and the complete hardware/format matrix remain later qualification gates.
Plain-mode stdin is line-blocking: runtime authority expires or cancels promptly, but consuming that status may require a newline/EOF.
There is no sandbox claim against hostile local processes racing filesystem paths.
Phase 4 is locally qualified; its publication/CI gate remains open and Phase 5 has not started.


## Phase 4 CI correction — approved content versus model punctuation

PR #13's Linux job failed only in `real_gguf_creates_only_the_approved_memory`.
All earlier Linux checks, both native-model targets, and the macOS/Windows/CUDA/Vulkan jobs passed at head `bccc6798f8f8afe05f537ac482a32759c9ca6b8d`.
The model proposed and the user approved `Keep answers concise` without a final period; the file and dispatch audit preserved those exact bytes.
The test incorrectly required the macOS model output's final period.
The corrected test compares the complete Markdown file against the actual approval preview, checks the audit arguments match that preview, retains a narrow wording check permitting the observed optional period, and still requires one created file and one dispatch.
It retains the duplicate-call refusal checks and does not change runtime permissions, parsing or persistence.
The [failed Linux job](https://github.com/kanishkpaul/sefai/actions/runs/39347063505/job/118382098742) preserves the original output.
Local verification after the correction passed all three real-tool cases, dedicated-target Clippy, the four required Rust checks, 16 attribution tests and the plain diagnostic; the fresh remote CI result is pending.


## Phase 5 — MCP (2026-10-20)

Implementation and local qualification are complete on `codex/phase5-mcp`.
Publication, PR review and hosted CI remain pending; no Phase 5 run has been monitored.
Phases 1–4 are merged, including the PR #13 CI correction.
The maintainer selected local/dummy authenticated servers for Phase 5 qualification.
This evidence does not qualify an unrelated production endpoint or claim physical Windows/Linux testing on this Mac.

### Changes

The optional client uses exactly pinned `rmcp 3.6.0` with defaults disabled and a current-thread Tokio runtime.
Supervised stdio runs an explicit canonical executable/argv, selected environment references, bounded stdout and discarded stderr.
Unix process groups and Windows Job Objects own cleanup; no OS sandbox is claimed.
Streamable HTTP uses bounded JSON/SSE POST reads, HTTPS or literal loopback HTTP, one ordinary POST at a time, no redirects/proxies and no expired-session replay or SSE resumption.
The optional OAuth flow uses the SDK for pre-registered public-client authorization-code login, PKCE, resource binding and state/issuer verification; callbacks are bounded and tokens remain in memory.
There is no automatic refresh, scope upgrade, dynamic registration or credential persistence.
The client pins protocol `2025-11-25`, disables sampling/elicitation authority, freezes paginated discovery and revalidates exact Markdown definitions before each request.
The harness retains one exact approval at a time, writes dispatch intent before sending, preserves structured/error results and marks failed/unknown external outcomes without replay.
A later successful operation cannot erase an earlier unknown outcome.
The TUI reports external MCP access, exposes disconnect and recovers visibly from a cancelled connection attempt.
See [mcp.md](mcp.md) for commands, supported subsets and all limits.

### Local checks

All checks below passed after the production changes:

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | pass |
| `cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings` | pass |
| `cargo test --locked --workspace` | 152 tests: 108 binary, 3 chat CLI, 12 workspace CLI, 4 core, 25 engine |
| `cargo test --locked -p sefai --no-default-features` | 53 tests |
| `cargo test --locked -p sefai --features mcp-oauth --bin sefai` | 128 tests |
| `cargo test --locked -p sefai --no-default-features --features mcp-oauth` | 68 tests |
| `cargo clippy --locked -p sefai --features mcp-fixture-tests,harness-model-tests --all-targets -- -D warnings` | pass |
| `cargo clippy --locked -p sefai --no-default-features --features mcp-oauth --all-targets -- -D warnings` | pass |
| `cargo clippy --locked -p sefai --no-default-features --features mcp --all-targets -- -D warnings` | pass |
| `python3 -m unittest discover -s tests -v` | 16 tests |
| `cargo run --locked -- prompt hello` | plain `[user]` / `hello`, no banner |
| `git diff --check` and attribution scan of changed/new files | pass |

Ordinary unit tests use in-memory duplex protocol fixtures and injected request budgets; no model, child server or network is needed.
Production dispatch tests verify exact arguments, denied calls, checkpoint conflicts and server-declared errors through the actual worker dispatch function.
Transport-wrapper tests enforce incoming-message exhaustion and reject oversized outgoing messages.

### Dedicated process and network qualification

```sh
export SEFAI_MCP_TEST_PYTHON="$(python3 -c 'import sys; print(sys.executable)')"
cargo test --locked -p sefai --features mcp-fixture-tests --test mcp_transport
```

All 20 tests passed on macOS using Python 3.15.0 and the committed fixture `tests/fixtures/mcp_server.py`.
The fixture is required, starts real stdio processes and authenticated loopback HTTP/OAuth servers, and does not contact production services.
The suite covers pagination, duplicate identities, protocol rejection, malformed/oversize responses, denied startup, selected resource/prompt retrieval, JSON/SSE HTTP, missing/wrong credentials, redirect refusal, initial disconnect and owned-descendant cleanup on Unix.
OAuth tests verify S256 PKCE and the resource parameter at the dummy token endpoint, reject forged callback state before token exchange, and reject changed issuer/unapproved metadata origins.
Fixture startup must produce an actual loopback endpoint, so an unavailable bind cannot make negative cases pass spuriously.
No test silently skips a missing Python executable.

### Actual model and terminal paths

```sh
SEFAI_TOOL_TEST_MODEL=/absolute/path/to/smollm2-1.7b-instruct-q4_k_m.gguf \
cargo test --locked -p sefai --features harness-model-tests,mcp --test harness_gguf
```

All seven real-GGUF tests passed on CPU with two threads.
The pinned model identity/hash is the same [tool fixture](../tests/fixtures/tool-gguf.json) qualified in Phase 4; MCP cases use a 4,096-token context and 256-token per-step limit.
The three original local-memory cases still pass.
New cases verify an approved MCP echo used in the final answer, an approved write equal to its actual displayed arguments, a denied write with no dispatch, and a server that writes then disconnects.
The latter leaves a durable unknown result, closes the connection and dispatches exactly once.
No golden punctuation or synthetic success reply is used to mask model variation.
This qualifies the explicit SmolLM2 protocol, not arbitrary models or general autonomous planning quality.

Three actual 90 × 24 macOS PTYs with `NO_COLOR=1` cover Enter-denied connection, separately approved connection/tool/real-model continuation, and Ctrl+C cancellation during initialization.
All exit zero and restore the original terminal attributes; cancellation returns a visible no-server state.
The recorded [results](evidence/phase5/terminal-results.json) and [terminal transcripts](evidence/phase5/terminal-transcripts.md) retain the approval controls and observed output.

### Dependency and binary cost

All original locked package/version pairs are preserved; neither llama.cpp binding changes from `0.1.160`.
The lockfile adds 159 platform-inclusive package/version pairs across optional features; they are not all installed into every build.
Unique normal dependency package counts on this Mac, including sefai, are 35 minimal, 74 stdio MCP, 136 HTTP and 156 OAuth.
The default GGUF dependency graph does not enable the MCP SDK, Tokio, HTTP or OAuth dependencies.
The [footprint record](evidence/phase5/footprint.json) stores exploratory headless release measurements: 1,912,224 bytes minimal; 3,990,336 stdio; 8,135,696 HTTP; 8,523,792 OAuth.
These use `--no-default-features`, the workspace release profile and aarch64 macOS; they exclude GGUF/TUI/native inference and model/server residency.
They are not idle, peak process-tree or cross-platform memory measurements; those remain Phase 10 gates.

### CI and limits

The existing three-OS CI now adds optional-feature Clippy/unit checks and the dedicated authenticated loopback fixture suite.
The Linux pinned real-model job adds the four MCP cases alongside the three memory cases.
CUDA/Vulkan compilation gates are unchanged; no additional GPU execution is claimed.
The only `AGENTS.md` change is the phase-status sentence; that and the added workflow gates require maintainer inspection before publication.
Production-server interoperability, physical Windows/Linux terminal/process-tree behavior and the full RAM/VRAM matrix remain unqualified here.

## Phase 6 — Native MLX

Published through merged PR #15 (`f41e395090a696aea3108d6481b7f21721ab0442`) after all checks passed, including macOS with its 60-minute cold-build allowance.
The local qualification record below predates publication; its pending-status statements describe that earlier checkpoint.

Local qualification uses Apple M5 with 16 GiB unified memory, macOS 27.0.1, Rust 1.96.1, Xcode 27.1 (27A9275), and Metal toolchain 27A266a (compiler 32023.921).
The public cache borrows its model until cache drop; a compile-fail contract rejects dropping the model first.
The optional Rust MLX libraries are pinned to `579199b4bb5e39b35f33fc6a59cb90e2c031a6a7`; native MLX is 0.32.2.
The product has no Python inference worker or model-download fallback.
The isolated Python 3.14.8 reference uses MLX 0.32.2 and `mlx-lm` 0.31.3; its [complete dependency versions](evidence/phase6/reference-requirements.txt) are retained.
PR review and hosted CI remain pending.
The preceding Phase 5 publication-pending text records its historical local checkpoint; Phase 5 has since merged with passing CI.

### Native-library coexistence

The first combined build compiled but crashed while loading GGUF, including before any MLX model was loaded.
Both native archives exported `gguf_get_key` with incompatible C signatures.
The [small binding build patch](../vendor/mlx-sys/SEFAI-PATCH.md) disables MLX's unused GGUF reader and removes its `gguflib` link; safetensors loading remains enabled.
The patched combined release loads real GGUF successfully and switches from real MLX generation to GGUF within one TUI session.
The hosted macOS workflow now runs that combined-link GGUF smoke test on CPU.
The [source comparison](evidence/phase6/binding-provenance.json) verifies all 78 retained upstream files outside the explicit build/metadata/documentation exceptions byte-for-byte; native FFI implementations are unchanged.
The third-party binding remains outside the workspace's safe Rust crates; `unsafe_code = "forbid"` is unchanged.

### Real-model and terminal evidence

The generation fixtures are [Qwen3-0.6B 4-bit](../tests/fixtures/mlx.json) and [SmolLM2-135M 8-bit](../tests/fixtures/mlx-llama.json).
The tool fixture is the producer-published [SmolLM2-1.7B Q8 MLX bundle](../tests/fixtures/mlx-tool.json).
Their manifests retain immutable revisions, asset sizes and SHA256 hashes; weights are ignored local assets.
The seven real MLX harness cases pass: three local memory cases plus approved MCP echo, exact approved write, denied write without dispatch, and a dispatched write followed by disconnect with an unknown outcome and no retry.
They use the same reusable harness scenarios as GGUF, the existing explicit SmolLM2 protocol and the local/dummy MCP server requested by the maintainer.
Qwen3-0.6B failed six of those seven tool scenarios because markup was absent or malformed; it remains qualified for generation, not reliable tool use.
The parser and approval rules were not weakened to admit it.

Four actual 90 × 24 macOS PTYs with `NO_COLOR=1` cover Unicode generation, native allocator statistics, switching MLX bundles, Ctrl+C cancellation, saved-session resume and MLX-to-GGUF profile switching.
Every process exits zero and restores its original terminal attributes.
The [terminal results](evidence/phase6/terminal-results.json) and [screen transcripts](evidence/phase6/terminal-transcripts.md) retain the observed behavior.
These release-binary terminal runs preceded the final help-label shortening and model-bound cache lifetime restriction; the final Rust checks cover those changes.
Physical terminal qualification across operating systems remains a Phase 10 gate.

### Matched reference and footprint

[Raw measurements and comparison](evidence/phase6/comparison.json) cover both small fixtures with short and longer prompts, three fresh-cache repetitions per process, identical full prompt token IDs, greedy 128-token output and 128-token prefill chunks.
The native and reference processes use the same MLX version and 64 MiB allocator-retention target, with no concurrent sefai builds or inference during the recorded measurements.
All 12 generated outputs match after stripping a leading-whitespace difference between the native and Python incremental detokenizers; raw bytes are not identical.
The native path has lower host RSS but decodes 1.46–1.81× more slowly here.
The longer Qwen case has about 8% higher macOS process peak footprint and 29% higher MLX peak-active allocation than the reference.
This does not establish a consistent total-memory improvement.
Full-cache capacity growth, transient allocation and decode throughput need profiling during Phase 10 before broader performance claims.
Native allocator counters overlap OS accounting and must not be added to RSS or described as separate VRAM.
The 64 MiB cached-allocation setting is a retention target, not a hard peak-memory cap; the measured cached counter can exceed it.

The [exploratory footprint record](evidence/phase6/footprint.json) stores aarch64 macOS release sizes and hashes: 1,912,224 bytes minimal, 11,778,128 GGUF, 22,181,264 MLX-only, and 33,309,024 combined GGUF/MLX/OAuth.
The generated MLX Metal library adds 1,473,912 bytes and must remain at the compiled `MLX_RS_METAL_PATH`; the executable alone is not relocatable.
These builds preceded the final help/cache-lifetime edits and are not release artifacts.
Normal dependency graphs including the binary contain 35, 104, 166 and 272 packages respectively on this target.
The MLX-only graph excludes llama.cpp, and the default GGUF graph excludes MLX.
All previous locked package/version pairs are preserved, including `llama-cpp-2 = 0.1.160`; 64 new platform-inclusive pairs belong to the optional adapter and are not all installed in the base build.
The new direct dependencies are `serde_json` for bounded bundle metadata, optional pinned native `mlx-lm`/`mlx-rs` for model execution and allocator control, and optional existing-version `thiserror` for typed native/tokenizer/template/cache errors.
The unpublished native language-model crate and maintained binding build patch are explicit dependency risks.

### Final local checks

Native commands set `MLX_RS_METAL_PATH="$PWD/target/mlx-metal"`.
Model targets use the pinned local paths in the linked manifests; harness targets also set `SEFAI_MCP_TEST_PYTHON` to the local Python executable.

| Command | Result |
| --- | --- |
| `cargo fmt --all --check` | pass |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | pass |
| `cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings` | pass |
| `cargo test --locked --workspace` | 160 tests: 109 binary, 3 chat CLI, 12 workspace CLI, 4 core, 25 GGUF adapter, 7 MLX bundle admission |
| `cargo clippy --locked -p sefai --features mlx,mlx-model-tests,mcp-oauth --all-targets -- -D warnings` | pass |
| `cargo clippy --locked -p sefai --no-default-features --features mlx,mlx-model-tests,mcp-oauth --all-targets -- -D warnings` | pass |
| `cargo clippy --locked -p sefai-engine-mlx --features model-tests --all-targets -- -D warnings` | pass |
| `cargo test --locked -p sefai --features mlx,mcp-oauth --bin sefai` | 130 tests |
| `cargo test --locked -p sefai --no-default-features --features mlx,mcp-oauth` | 145 tests: 130 binary, 3 chat CLI, 12 workspace CLI |
| `cargo test --locked -p sefai-engine-mlx --features runtime --lib` | 7 bundle admission tests |
| `cargo test --locked -p sefai-engine-mlx --features runtime --doc` | 1 compile-fail cache lifetime contract |
| `cargo test --locked -p sefai-engine-mlx --features model-tests --test mlx -- --test-threads=1` | 11 tests each on Qwen3-0.6B and SmolLM2-135M |
| `cargo test --locked -p sefai --features harness-model-tests,mcp --test harness_gguf` | 7 actual GGUF harness tests |
| `cargo test --locked -p sefai --no-default-features --features mlx-model-tests,mcp --test harness_mlx` | 7 actual MLX harness tests |
| `cargo clippy --locked -p sefai --target x86_64-unknown-linux-gnu --no-default-features --features mlx --all-targets -- -D warnings` | pass; compile-only unsupported-platform path |
| `python3 -m unittest discover -s tests -v` | 16 attribution tests |
| `cargo run --locked -- prompt hello` | plain `[user]` / `hello`, no banner |
| Actionlint 1.7.12 on both workflows, ShellCheck integration disabled | pass |
| `git diff --check` and full-content attribution scan of changed/new files | pass |

The Linux cross-Clippy attempt with `mlx,mcp-oauth` cannot build `aws-lc-sys` because `x86_64-linux-gnu-gcc` is absent on this Mac.
The MLX-only Linux cross-check passes; Linux OAuth runtime checks remain the existing hosted CI gate.
No hosted run was started or monitored during local qualification.

### Remaining limits

Floating weights and architectures beyond the named quantized fixtures are not locally qualified.
Untrusted model templates are not sandboxed: upstream rendering has no execution-fuel limit, even though input/output sizes are bounded.
Cancellation cannot interrupt a native load, shader compilation or GPU operation already in progress.
The full KV cache has no quantization, rotation, snapshotting or serialization in this adapter.
Hosted CI compiles native MLX combinations and tests pure contracts plus combined-library GGUF CPU inference; it does not claim hosted MLX GPU execution.
Windows/Linux MLX selection is an explicit unsupported-platform error; GPU vendor coverage remains provided by the selected GGUF backends and requires the planned physical matrix.
Under the expanded build plan, asset packaging and clean-machine installation remain Phase 14 gates, following runtime unification in Phase 9, setup in Phase 11, safety and permission hardening in Phase 12, and complete testing and essential red teaming in Phase 13.
The changed workflow and status-only `AGENTS.md` edit require maintainer inspection before publication.

## Phase 9 — Runtime unification

Phase 9 ships as three stacked PRs: #23 admits artifacts against their declared format, #24 rejects runner controls an engine never applies, and #25 adds the shared lifecycle contract.
Local qualification ran on Apple M5, 16 GB, macOS 27.0.1, Rust 1.96.1.

### Inventory

Reproduced before changing behavior:

- A profile's `format` was reduced to a backend during resolution; `--model` and `/model` had no format, and loading never saw one.
- A `format: gptq` profile could load an AWQ checkpoint, and a CLI `--backend cpu` could send a GPTQ profile's bundle directory to llama.cpp.
- MLX silently ignored `batch` and `threads`, and ONNX ignored `batch`, while profiles presented them as adjustable.
- Device labels in `run` and terminal status were Rust debug output, and every load failure was an unclassified string.

### Lifecycle contract

Each fixture was fetched with `scripts/fetch_fixture.py`, which verified every file's size and SHA-256 against its manifest, and run with the release profile and `--test-threads=1`:

| Target | Fixture | Result |
| --- | --- | --- |
| `lifecycle_gguf` | [gguf.json](../tests/fixtures/gguf.json), SmolLM2-135M Q4_K_M, CPU | 6 passed |
| `lifecycle_mlx` | [mlx-llama.json](../tests/fixtures/mlx-llama.json), SmolLM2-135M 8-bit MLX | 6 passed |
| `lifecycle_gptq` | [gptq.json](../tests/fixtures/gptq.json), Qwen3-0.6B GPTQ Int8 | 6 passed |
| `lifecycle_awq` | [awq.json](../tests/fixtures/awq.json), Qwen3-0.6B AWQ | 6 passed |
| `lifecycle_onnx` | [onnx.json](../tests/fixtures/onnx.json), SmolLM2-135M int8, CPU provider | 6 passed |
| `lifecycle_switch` | GGUF, MLX and ONNX fixtures above | 1 passed: one session switched GGUF → MLX → ONNX → GGUF and reported `cpu`, `mlx apple gpu`, `onnx cpu · 10 threads` and `cpu` |

The first `lifecycle_switch` run failed in the test's own parsing: plain mode prints the ready status directly after the streamed reply, so a line-start match missed it; the corrected test passes.
Hosted CI now runs `lifecycle_gguf` in the three-OS check job and `lifecycle_onnx` in the three-OS ONNX job.

Before the contract, JunHowie's Qwen3-0.6B-GPTQ-Int4 confirmed admission through the CLI: `--format awq` fails with `unsupported artifact: declared format awq but the checkpoint stores gptq weights`, `--format gguf` fails with the directory guidance, and `--format gptq`, `--backend mlx` and `--microbatch 1024` each generate; `--threads 4` and `--batch 64` are rejected with the field named.

### Remaining limits

CUDA, Vulkan, ROCm and ONNX providers other than CPU and WebGPU inherit the same load plan and admission but were not executed; they remain Phase 10 hardware gates.
The lifecycle contract checks behavior shared by every engine; cancellation inside native calls, cache reuse and tool parsing remain covered by each adapter's own targets.
Admission validates bundle metadata twice for MLX and ONNX, once in the binary and once in the adapter's loader; the second read is bounded by the same limits and costs milliseconds against model load.

## Phase 10 — Footprint and platform qualification

Phase 10 ships as PRs #26–#28 and is scoped to one MacBook Air (Apple M5, 16 GB) plus GitHub-hosted CPU runners; results and method are in [footprint.md](footprint.md), raw records in [evidence/phase10](evidence/phase10).

### Local measurement

Nine release variants were built and measured idle; six engine and fixture pairs were benchmarked with raw 512-token prompts, and microbatch, context and thread sweeps followed, each run separated by 8 s with the thermal state recorded.
No thermal or performance warning was recorded.
The first pass used chat-templated prompts, on which SmolLM2-135M ends after one token; it measured no decode, so every bench was rerun raw and the hosted workflow was corrected the same way.
A copied WebGPU binary aborted at launch because `libwebgpu_dawn.dylib` must sit beside it; placing the library there fixed it and the requirement is recorded for packaging.
Xcode updated itself to 27.2 during the session and removed the Metal toolchain; `xcodebuild -downloadComponent MetalToolchain` restored it before the MLX builds were measured.
An early default-thread result suggested all ten cores slowed decode by a third; a controlled sweep showed 340 against a best of 366 tokens/s, so the default was not changed.

### Changes from measurement

- GGUF defaults to 512-token microbatches: Metal prefill 12,878 → 18,897 tokens/s and CPU 982 → 1,050 for at most 9 MiB more.
- MLX evaluates each sampling step once: SmolLM2-135M decode 245.8 → 274.5 tokens/s with byte-identical greedy output; `sefai-engine-mlx --test mlx` passes 11/11 on the MLX, GPTQ and AWQ fixtures.
- `sefai setup` estimates use the measured per-engine overheads.

### Remaining limits

CUDA, Vulkan, ROCm, GPTQ and AWQ CUDA kernels, ONNX GPU providers other than WebGPU on Apple Silicon, physical Windows and Linux terminals, and multi-GPU selection were not executed.
Hosted runner throughput comes from shared virtual machines and is a smoke check.
MLX decode still trails Python `mlx-lm` on small models; asynchronous pipelining of the next forward pass is the likely remaining difference and was not attempted.

## Phase 11 — Hardware-aware model setup

Phase 11 ships as PR #29 on Apple M5, 16 GB, macOS 27.0.1.
`sefai setup` and `/setup` report hardware and compiled engines, check local models from metadata, shortlist the 14 pinned catalog entries, list blockers, preview and perform verified acquisitions in a `download` build, and import local models.

### Evidence

- Unit tests cover the bounded GGUF header reader, the KV-cache formula against Qwen3-0.6B's 448 MiB at 4,096 tokens, fit bands, catalog validation and duplicates, recommendations limited to compiled engines, memory and trained context, feature blockers, SHA-256 mismatch and a cancelled acquisition that publishes nothing.
- The `setup` target runs the binary for the catalog listing, a metadata check, trained-context rejection, a create-once import, a rejected non-GGUF import, workspace catalog entries, duplicate and escaping entries, and an unconfirmed `get` that downloads nothing; it passes 9/9 with and without `download`.
- Estimates against Phase 10 peaks: SmolLM2-135M GGUF 0.19–0.23 GiB against 0.22 GiB measured; Qwen3-0.6B GPTQ 1.16–1.29 GiB against 1.23 GiB; SmolLM2-135M ONNX 0.30–0.60 GiB against 0.59 GiB.
- A real `sefai setup get smollm2-135m-gguf --yes` downloaded 105,454,432 bytes from Hugging Face in 30 s, verified its SHA-256, promoted the staging directory, wrote a workspace-relative profile and generated through `chat --plain --profile`; a second run re-verified and succeeded without rewriting the profile.
- `setup import` of the GPTQ fixture wrote a profile that generated on MLX through the terminal.
- `/setup`, `/setup recommend`, `/setup check`, `/setup get ID` and an invalid subcommand behave the same in plain terminal mode.
- Lifecycle targets for GGUF, MLX, ONNX and engine switching still pass with the new first-run hint.

### Remaining limits

GPU memory budgets, gated downloads, cancelling a terminal download mid-file and automatic calibration are not implemented; `sefai bench` remains the calibration step.
The recommendation catalog holds only fixtures sefai's own tests run; it is evidence-backed, not a survey of good models.
