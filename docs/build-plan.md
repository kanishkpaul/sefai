# Build plan

Build a small Rust LLM runner and terminal harness, with Markdown-owned customization and optional inference backends.
GGUF, ONNX, GPTQ, AWQ, and MLX are release targets.
Phase 1 is complete: the published GGUF runner passes local CPU/Metal tests, Windows/macOS/Linux CPU-model CI and CUDA/Vulkan compile checks.
Phase 2 is merged through PRs #8–#11 with passing Windows, macOS, Linux and attribution checks.
Phases 3–8 are merged; Phase 6 native MLX is published through PR #15 with passing CI.
Phase 9 runtime unification is delivered by PRs #23–#25; the Phase 8 NVIDIA gate remains open.
See [phase2-closeout.md](phase2-closeout.md) for the acceptance matrix and stack order.
Complete runner correctness before adding harness features.
Every phase includes regression tests; the second-last phase is a complete product verification pass, and the last is repository organization, installation, and presentation.

## Scope

- Windows, macOS, and Linux, with CPU fallback chosen explicitly and reported.
- NVIDIA through CUDA, Apple Silicon through Metal, AMD through Vulkan on Windows and Linux; ROCm is an additional qualified route, not a requirement for AMD coverage.
- Persistent terminal TUI, with one-shot runner and diagnostic commands for scripts.
- System prompts, skills, preferences, memory, session records, model profiles, and MCP definitions owned by Markdown files.
- MCP client with local stdio and remote Streamable HTTP, bounded tool execution, explicit authority, and cancellation.
- One active model and one conversation generating at a time by default.
- Optional engine packages; choosing more formats must not install or load every runtime.

The requirement covers the product across those operating systems and GPU vendors, not every model format on every GPU.
MLX targets Apple Silicon; GPTQ/AWQ are qualified first on Apple Silicon through MLX, and NVIDIA kernels are a follow-up gate.
The format and hardware matrix is in [stack.md](stack.md).

## What to keep and remove first

| Source | Keep | Remove from the active tree |
| --- | --- | --- |
| Audited `main` at `c89958a` | Failure cases, explicit GPU controls, loading and streaming lessons | WPF app, Python service, named-pipe IPC, persona mood/refusal rules, scheduler, canned fallback replies, SQLite as canonical memory, brittle CLI-output parsing |
| Archived `new` at `0041f47` | Dependency-free message/prompt types, Cargo workspace, style, hooks/CI starting point, design kit | Companion ability registry and persona TOML prototype, desktop/server plans, mandatory embedding model and autonomous memory extraction |
| Design kit | Candle palette, terminal theme, metadata/status, tool rows, approval layout | Desktop/tray assumptions in this project's direction |

Legacy sources are moved intact to the ignored `.local-archive/legacy-main-c89958a/` locally and also recoverable from `.local-archive/before-cleanup.bundle`.
The exact `new` tip remains at local `archive/new-2026-10-19`; its GitHub branch was deleted after bundle verification.
The local `old` branch is untouched.
Removing old code retires that implementation; it does not prove its bugs have been fixed in a replacement engine.
Do not build a compatibility layer for the retired desktop app.

## Phases and gates

| Phase | Deliverable | Gate |
| --- | --- | --- |
| 0 | Audit, reversible pruning, Rust foundation, engineering and repository controls | Audit and fix backlog are reviewable; local checks pass; publication/protection status is explicit |
| 1 | Correct embedded GGUF runner | Every retained runner defect has a regression test and real-model evidence |
| 2 | Markdown workspace | Configuration, skills, memory and sessions round-trip safely through files |
| 3 | Persistent terminal TUI | Streaming, cancellation, recovery and terminal restoration work |
| 4 | Harness loop | Bounded turns, tool proposals, approvals and memory editing work without MCP |
| 5 | MCP client | stdio and remote transports pass conformance and failure cases |
| 6 | MLX adapter | A declared Apple Silicon model set runs through the same harness |
| 7 | ONNX adapter | A declared decoder-model set generates with explicit cache and provider control |
| 8 | GPTQ and AWQ adapter | Real quantized checkpoints generate through a qualified runtime |
| 9 | Runtime unification | All five format targets share explicit profile, capability, lifecycle and reporting contracts |
| 10 | Memory and platform qualification | OS/GPU matrix and footprint budgets have measured results |
| 11 | Hardware-aware model selection and setup dashboard | A user can choose, acquire/import, configure and load a suitable model through the TUI |
| 12 | Safety and permission hardening | Authority, privacy, isolation limits and recovery controls have implementation evidence |
| 13, second-last | Full testing and essential red teaming | All features, parts and integrations, including setup, have release evidence |
| 14, last | Repository structure, installation, README and release | Clean-machine install works and documentation matches the shipped product |

### Phase 0 — Foundation and audit

Record the original branch identities, main findings, test results, keep/delete decisions and research sources.
Keep the root build runnable without an inference SDK.
Keep only core prompt types and a diagnostic command; do not pretend this is an inference-capable release.
Add local commit/pre-push checks, CI on three operating systems, attribution regression tests, dependency updates, and a review template.
Disable tool-provided attribution in repository settings.
Prepare historical trailer cleanup without rewriting `old` or silently force-pushing `main`.
Publish controls only after the maintainer authorizes commits; enable required checks against the actual published workflow.
A regex and an agent instruction cannot guarantee that an administrator or credential holder will never bypass policy.

**Exit:** audited snapshot and verified local archives; all four required Rust checks; attribution tests; workflow validation; CI execution on Windows/macOS/Linux after publication; branch protections verified by API.
The foundation and controls are published; bootstrap CI passed on all three operating systems and main protection is active.
Detailed evidence belongs in [verification.md](verification.md).

### Phase 1 — Fix the runner before expanding it

**Complete:** the runner is published through merged PR #6; local CPU/Metal regressions, cross-platform CPU-model CI and CUDA/Vulkan compilation have passed.
Merged PR #7 retains those passing checks with updated CI actions and the repaired attribution workflow.
CUDA/Vulkan execution remains a Phase 10 hardware gate.
The [runner guide](runner.md) describes the implemented API and limits; [verification.md](verification.md) records results and failures.
The Phase 1 gate is closed; Phase 2 implementation is recorded below.

Work through [bug-fix-plan.md](bug-fix-plan.md) in order.
Create `engine-llama` only when the first real engine implementation lands.
Pin `llama-cpp-2` exactly in a standalone dependency commit, with defaults disabled and explicit backend features.
Determine the version from an API/build probe rather than assuming the current documentation matches the pinned vendored llama.cpp.
Do not copy the old binding's API calls without checking their semantics.

Implement model validation; metadata/tokenizer loading; model-native chat templating; correct BOS/EOG handling; chunked prefill; output reservation; incremental UTF-8; one sampler acceptance; streaming and checked positions.
CPU mode must disable GPU compute and cache/offload paths supported by the pinned API and report any unavoidable backend allocation accurately.
Validate device selection and fail clearly when an explicitly chosen backend is unavailable.
Implement explicit `run` and `bench` commands, a single engine owner, and bounded cancellation checks between decode/prefill steps.
Use the upstream template implementation when the binding exposes it; otherwise test the renderer against upstream fixtures, including tool templates.
A template-renderer dependency needs a demonstrated compatibility gap.

**Exit:** empty prompt policy, 511/512/513-token boundaries, a 3,000-token prompt, exact context boundary/overflow, output limit zero, Bengali/emoji split across tokens, all stop tokens, sampler state, CPU selection, invalid GPU index and repeated model load/unload are tested.
Run a tiny pinned, licensed GGUF in dedicated integration CI, with its SHA256 checked and a named test target; a missing model must fail that job rather than silently skip it.
Record actual CPU/Metal results and compile CUDA/Vulkan configurations; GPU execution remains pending until hardware is used.
No harness feature starts while a correctness defect in this gate remains unresolved.

### Phase 2 — Markdown owns customization (merged)

The implemented workspace uses this layout, rooted at the platform's config/data location or an explicit `--workspace` path:

```text
config.md
system.md
preferences.md
models/*.md
skills/<name>/SKILL.md
memory/*.md
sessions/<id>.md
mcp/*.md
cache/                 disposable engine state and indexes
```

Keep prose as ordinary Markdown and machine settings in a strictly typed YAML front matter block.
Only front matter changes configuration; a sentence in the body cannot execute a command, grant a permission, or change a setting.
Version the schema; reject unknown keys, invalid ranges, duplicate IDs and unsupported versions with a path and field name.
Use the documented precedence: explicit CLI override, selected model profile, workspace config, built-in defaults; user/global prose is composed before workspace prose in a documented order.
Do not execute Markdown code fences.
Skills are instructions and resource references, with metadata and input limits; do not build a new plugin programming language.
Read skills on demand and include only selected skills in the prompt.

Use one file per durable memory and session, bounded reads and recall, deterministic IDs and ordering.
Start with bounded text recall and explicit remember/edit/forget; defer embeddings until a measured recall failure warrants their memory cost.
Use atomic replace in the same directory, cross-platform locking, and conflict detection before overwriting externally edited content.
Keep large transcripts on disk; page a bounded tail into the TUI and token budget.
Define a versioned session grammar that safely encodes headings, delimiters, code fences and role markers supplied by users; roles come from parsed records, never inferred from arbitrary prose.
Test truncated files and interrupted writes; preserve the last good version on malformed edits.

Binary models, KV cache and other generated indexes are exceptions to the Markdown requirement because they are machine artifacts, not customization.
Indexes must be deletable and rebuilt from Markdown.
Secrets use environment or OS credential-store references in Markdown, never plaintext keys.
Do not replace the Markdown source of truth with a hidden database.

**Exit:** edit a system prompt, model profile, skill, preference, memory and session with a text editor; restart and observe exactly the intended change.
Test Unicode paths, CRLF, invalid front matter, oversize files, symlinks escaping the workspace, concurrent edits and Windows rename semantics.
Record schema fixtures and migration/rollback behavior.

### Phase 3 — Terminal home

**Merged:** PR #12 publishes the terminal implementation with passing CI; physical Windows/Linux terminal qualification remains a Phase 10 gate.
See [tui.md](tui.md) for implemented controls and explicit limits.

Use Ratatui with Crossterm, and `design/rust/theme.rs` as palette/identity authority.
Introduce these dependencies only when implementing this phase.
Keep one main transcript, one composer and one status line; use overlays for models, skills, memory, settings and MCP connections.
The TUI remains the permanent product surface; no tray app, webview or desktop shell is planned.

Keep model weights loaded across turns.
Own model/context on a dedicated worker thread with bounded command and token channels.
Coalesce streaming redraws and stop idle redraws; never block the input event loop on inference or disk I/O.
Support stop generation, reset, new/resume session, model selection, context overflow feedback, and clear failure recovery.
Prefix reuse must compare actual rendered tokens; changing Markdown or templates invalidates the divergent suffix.
Session/model switching must free the old cache before allocating the next large model unless the user explicitly chooses a measured higher-memory strategy.
Restore terminal mode on normal exit, errors, panics, cancellation and child-process failures.

**Exit:** terminal backend snapshots and real terminal transcripts for first run, load failure, streaming, cancellation, context full, resume, resize and quit.
Test narrow widths, long wrapped text, wide/combining characters, paste, `NO_COLOR`, `TERM=dumb`, and a screen-reader-friendly plain mode.
Diagnostics and one-shot output stay usable without a terminal and never contain an interactive banner.

### Phase 4 — Harness behavior

**Merged:** PR #13 publishes the bounded harness with passing CI.
See [harness.md](harness.md) for the explicit protocol, local tool scope and qualification limits.

Implement a small request → generation → optional tool proposal → permission decision → tool result → generation loop.
Use concrete types until the second backend needs a shared interface; the engine capability boundary then becomes a justified abstraction.
Capabilities include template/tool support, cache reuse, supported context sizes, cancellation and memory reporting; do not emulate unavailable capabilities silently.

Bound steps, wall time, generated tokens, tool calls, pending approvals, result bytes and transcript size with named limits.
Freeze the permitted workspace/server/tool scope before reading untrusted content.
Tool output and skill prose cannot widen it.
Keep authority in runtime code, with an exact preview for an external write and denial as the default.
Revalidate the approved server/tool/arguments immediately before execution.
Do not infer safety from a model's explanation or from an MCP tool annotation.
Cancellation of a dispatched write can leave the outcome unknown; show that state and do not blindly retry it.

Keep memory writes explicit initially.
Regenerate must not replay successful side effects.
Preserve errors and provenance in a bounded Markdown session record, with secrets redacted before serialization.

**Exit:** deterministic fake-engine tests cover no-tool replies, tool success/error, denied and stale approval, step/time/token limits, retry rules, cancellation and recovery.
One real tool-capable GGUF passes the same end-to-end turn path.
No keyword-based persona refusals or synthetic success fallbacks are carried forward.

### Phase 5 — MCP

**Merged:** the MCP implementation is published with passing CI.
See [mcp.md](mcp.md) for optional builds, first-launch trust, per-call authority, bounds and OAuth limits.
The maintainer selected local/dummy authenticated servers for this phase; production-service interoperability remains unqualified.

Use the official Rust SDK `rmcp` with only required client/transport features after a size/dependency review.
Negotiate a pinned supported protocol revision; test version rejection rather than declaring every future revision compatible.
Support initialization, capability discovery, paginated tool/resource listing, bounded resource reads, prompt retrieval, tool calls, structured results, errors and cancellation.
Prompts/resources selected by the user remain untrusted data.
Disable server-initiated sampling and elicitation until deliberately implemented and tested.

For stdio, launch an explicit executable and argument array, never a shell string assembled from model output.
Show first-launch trust, constrain environment inheritance, bound stdout/stderr, and reap the whole child process tree on Windows and Unix.
For remote servers, use Streamable HTTP with timeouts, explicit endpoint trust, proper OAuth when required, credential references, safe redirect handling and no token passthrough.
A root or advertised path is not a sandbox; enforce filesystem permissions locally wherever sefai owns access.
Display the server, tool, exact arguments and network destination before approval.
An MCP connection makes external traffic possible; the UI must stop claiming that every operation stays local.

**Exit:** fixture servers cover both transports, discovery pagination, duplicate names, malformed/oversize responses, disconnect, timeout, cancellation and write-outcome uncertainty.
A real local server and an authenticated remote server are tested with recorded software versions and redacted evidence.

### Phase 6 — MLX

Merged through PR #15 with passing platform, attribution and CUDA/Vulkan compilation checks.
See [the native MLX adapter, measurements and limits](mlx.md).

Support MLX-compatible model bundles on Apple Silicon as a committed target.
A model directory includes tokenizer/configuration, safetensors shards and MLX quantization metadata; a `.safetensors` suffix alone does not identify an MLX model.
Evaluate `mlx-rs` for a Rust-owned adapter and `mlx-lm` as a compatibility reference.
The binding exposes tensors, not an automatic implementation of every `mlx-lm` architecture.
Select a small named decoder architecture set and implement/borrow licensed model code only for that set.
If native coverage is inadequate, an optional persistent `mlx-lm` worker is an explicit packaging decision with its Python/runtime memory shown separately.
Never silently install Python or substitute a worker behind a claim of native execution.

**Exit:** actual bundled model fixtures generate Unicode and tool/non-tool turns, preserve cache correctly, cancel, unload and resume through the TUI.
Compare unified-memory peak, decode and prefill against the same-version MLX reference.
Document native versus worker support before this phase can close.

### Phase 7 — ONNX

**Complete:** named Optimum and GenAI share-buffer exports generate through the CLI and harness on the CPU and WebGPU providers; see [the ONNX adapter](onnx.md) for evidence and limits.

Support ONNX decoder model bundles including external tensor data, tokenizer/config and generation metadata.
Use the Rust `ort` interface to ONNX Runtime if its safe API covers the required execution-provider and I/O-binding controls.
ONNX Runtime GenAI is a generation reference, not assumed to have a drop-in Rust API.
A generic `.onnx` graph is not automatically an autoregressive language model.
Validate the graph's declared inputs, outputs, dynamic shapes, past/present cache layout, dtype and supported opset for the selected model family.
Keep KV tensors device-resident with I/O binding where supported; inspect allocator arena retention and unnecessary host copies.
Do not allocate a second full cache each token.

**Exit:** a named model/export set generates with CPU and one qualified accelerated provider; test past-state growth, output parity tolerances, memory plateau, missing external files and incompatible graph signatures.
Report providers by OS and hardware; never infer AMD or Apple coverage from the existence of an ONNX file.
Compare against ONNX Runtime/GenAI with the same graph/provider settings.

### Phase 8 — GPTQ and AWQ

**Complete on Apple Silicon:** named GPTQ and AWQ checkpoints are repacked onto MLX affine groups in memory and run through the CLI, TUI and harness; see [GPTQ and AWQ on MLX](gptq-awq.md) for evidence and limits.
NVIDIA kernels remain a follow-up gate for contributors with that hardware.

Support both quantized checkpoint families as committed targets.
They are quantization schemes, usually carried in safetensors plus a quantization config, not two universal file extensions.
Validate method, bit width, group size, packing/kernel variant, architecture and tokenizer before loading.
The qualification host is a Mac, so begin with an Apple Silicon model set on the existing MLX adapter; NVIDIA and other GPUs require their own kernel evidence.

Run a compatibility spike against an established runtime that actually supports the selected packing; for NVIDIA, use vLLM's published matrix as one reference.
Prefer an existing safe Rust integration if it meets that gate; do not write a quantized GEMM engine to satisfy an extension list.
If native coverage is inadequate, use an explicitly installed, persistent local worker behind a narrow adapter.
That adds an optional runtime dependency and RAM/VRAM overhead; show both, and keep the GGUF package independent of it.
Require a documented native/worker choice and measured idle footprint before committing the adapter implementation.
Conversion to GGUF does not count as GPTQ/AWQ runtime support.

**Exit:** one real checkpoint for each scheme loads without conversion, streams through the same TUI/harness, handles tools where the model template supports them, cancels, unloads and respects the configured memory ceiling.
Unsupported packing/hardware fails clearly.
Compare against the selected runtime with equal context, cache dtype and single-request concurrency.

### Phase 9 — Runtime unification

**Complete on Apple Silicon:** one load plan carries each artifact's declared format, every engine admits the artifact before native allocation, unused runner controls are rejected, and the same lifecycle contract passes for GGUF, MLX, GPTQ, AWQ and ONNX fixtures, including switching engines within one session; see [the runner guide](runner.md#lifecycle-contract) and [verification](verification.md#phase-9--runtime-unification).
CUDA, Vulkan and non-CPU ONNX providers inherit the contract but were not exercised; they remain Phase 10 hardware gates.

Consolidate demonstrated adapter differences after the GGUF, MLX, ONNX and GPTQ/AWQ implementations exist.
Keep baseline measurements during each adapter phase; run the full Phase 10 platform and memory matrix against the unified implementation.
Keep one Markdown profile workflow, explicit runtime/device/provider selection, capability-aware settings, bounded model lifecycle and comparable statistics through the existing binary dispatch.
Native formats/caches stay backend-owned; no implicit conversion, hidden fallback, mandatory runtimes or speculative plugin framework.
See [setup-plan.md](setup-plan.md) for researched constraints, ownership, delivery steps and acceptance cases.

**Exit:** the same terminal/harness lifecycle handles every qualified format, reports unsupported capabilities and missing requirements precisely, and passes switching/unload/recovery contracts on declared hardware.

### Phase 10 — Footprint and platform qualification

Qualify the unified Phase 9 implementation before using its results in the Phase 11 setup dashboard.
Apply the experiments in [research/2026-10-19-inference-and-engineering.md](research/2026-10-19-inference-and-engineering.md).
Use one active model, bounded context/output, microbatch tuning, supported Flash Attention, explicit KV precision, stable prompt ordering and bounded scrollback first.
Do not add speculative decoding, eager embeddings, multi-model residency or persistent KV snapshots until their total memory benefit is demonstrated.
Disk KV restore is optional and keyed by model identity, engine revision, template, tokens, cache format and placement; stale or oversized state is discarded safely.

Target harness-only idle overhead at 40 MiB or less for the GGUF build, and 60 MiB or less for an idle TUI/MCP client without child servers.
These are proposed targets, not current measurements.
Report backend context/weights, driver and optional worker cost separately and also report the total process/process-tree footprint.
Measure backend-specific binary and installed-package size; a 15 MiB target for the small Rust package is not a promise about CUDA/ONNX/MLX runtimes.
Start with context 4,096, batch 512, microbatch 128 and one sequence as benchmark candidates, not immutable defaults.
Set defaults from the measurements, with a conservative user-visible fit calculation and headroom for the OS/display/other apps.
No hidden CPU fallback, OOM retry loops, or automatic quality reduction.

**Exit:** CPU runs on Windows/macOS/Linux; Metal on Apple Silicon; CUDA on Windows/Linux NVIDIA; Vulkan on Windows/Linux AMD.
List exact devices, drivers and backend revisions.
ROCm, ONNX providers and GPTQ/AWQ kernels are separately labeled qualified/experimental/unavailable.
Hosted CI compilation is not proof that a GPU backend ran.

### Phase 11 — Hardware-aware model selection and setup dashboard

Build a focused terminal setup view on the unified contracts and measured Phase 10 data; the conversation remains the permanent home.
Inspect hardware/runtime availability, import local models or use a small offline Markdown recommendation catalog, explain compatibility and memory fit, preview acquisition/settings, and save/load a chosen Markdown profile.
Use explicit download actions, bounded optional calibration and honest measured/estimated/unknown labels; no invented compatibility percentage or universal best-model claim.
See [setup-plan.md](setup-plan.md) for the complete workflow, memory accounting, acquisition controls, proposed bounds and failure matrix.

**Exit:** a new user can choose a suitable model for their task/hardware, understand its exact runtime/settings, acquire or import it, and generate through the existing TUI with recoverable failures and no hidden fallback.
Re-measure footprint after unification and setup; Phase 12 hardens their safety boundaries and Phase 13 covers every added path before release.

### Phase 12 — Safety and permission hardening

Consolidate safety across the completed runtimes, harness, MCP client and setup flow before the full Phase 13 verification pass.
Safety controls and regression tests remain mandatory in every earlier phase; this phase closes cross-feature gaps rather than postponing basic protection.
Build on existing approval, redaction, file validation and process supervision code; do not introduce a separate policy framework or an additional model to decide authority.

Deliver this phase in three steps:

1. Map assets, trust boundaries and authority for local inference, Markdown, native runtimes, optional workers, MCP and model acquisition.
   Record each concrete abuse case, existing control, remaining gap, owning module and regression test in one safety checklist.
   Prioritize unauthorized actions, secret disclosure, data loss and resource exhaustion; document residual risks explicitly.
2. Implement the missing controls at their owning boundaries, using the smallest concrete changes.
   Keep permissions in validated runtime configuration and explicit user actions; Markdown prose, recalled memory, model output, tool descriptions and remote metadata cannot grant or persist authority.
   Bind each approval to the exact server/tool/arguments, destination and affected files; deny by default, revalidate immediately before dispatch and invalidate pending approvals after relevant configuration/catalog changes or disconnect.
   Provide an explicit way to stop further dispatch and revoke a connection's authority; revocation does not undo completed actions or guarantee interruption of an in-flight native call.
3. Qualify the controls with deterministic hostile fixtures and focused process/transport/file integration tests, record unsupported protections and hand the checklist to Phase 13.
   Re-measure the safety additions against Phase 10 footprint results; no always-running safety service or unbounded audit store.

Treat prompt injection and memory poisoning as attempts to cross these authority boundaries; a successful persuasion must still leave unauthorized actions blocked.
Cover these boundaries:

- **Privacy and outbound data:** show what prompt, tool arguments, selected files or other data can leave the machine and to which destination; an enabled connection is not blanket consent to export workspace content.
  Keep credential references separate from Markdown content; restrict environment inheritance, scope credentials to their intended service and redact known secrets before display or persistence.
  Do not promise detection of arbitrary or encoded secrets; raw native caches and crash dumps are not guaranteed to be scrubbed.
- **Files and durable state:** preserve workspace boundaries, role grammar, atomic writes and revision checks; reject traversal, unsafe destinations and poisoned memory attempts that would alter authority.
  Review path/symlink races where sefai owns writes; where a safe implementation cannot enforce containment, block the affected operation or disclose the remaining boundary before granting access.
- **Processes and network:** constrain executable/arguments, inherited environment, lifetime, output and child cleanup; validate endpoints, redirects and credential forwarding, including local/private destinations that require deliberate trust.
  Decide and record feasible OS isolation for optional workers and MCP servers on each supported platform; ship a tested narrow sandbox where practical, otherwise label the process as unsandboxed before launch.
  Process supervision and MCP roots are not filesystem/network sandboxes; third-party processes may retain the user's OS privileges.
- **Models, downloads and dependencies:** validate pinned artifact identity, manifest paths, sizes, hashes and supported format metadata before loading; stage incomplete downloads safely and never execute model-repository code or installers implicitly.
  Review optional runtime/native parser dependencies, license notices and vulnerability findings; hashes prove identity/integrity relative to their source, not that a model or library is safe.
  Keep native inference libraries an explicit trust boundary rather than claiming Rust makes malformed model files harmless.
- **Terminal, resources and recovery:** sanitize untrusted display text, bound allocation/input/output/queues/retries and preserve terminal cleanup.
  Denied, failed, cancelled and unknown-after-dispatch outcomes remain distinct; never replay a potentially completed write automatically.
  Keep bounded, redacted Markdown action records separate from model-visible conversation; document retention/deletion controls without claiming tamper-proof logging against the same OS user.

Use the [OWASP MCP security guidance](https://cheatsheetseries.owasp.org/cheatsheets/MCP_Security_Cheat_Sheet.html) and [least-privilege guidance for excessive agency](https://genai.owasp.org/llmrisk/llm062025-excessive-agency/) as review inputs, not as certification.
Do not add keyword refusals, a model-opinion policy or generic jailbreak research to this runner phase.

**Exit:** every checklist row has a verified control or an explicit blocked/unavailable capability; no known release-blocking unauthorized-action, secret-leak or data-loss gap is deferred to documentation alone.
Record evidence for stale/revoked approvals, malicious Markdown/tool/model metadata, secret-bearing output, hostile paths/redirects, process exhaustion and interrupted writes across the affected configurations.
Document isolation and native-runtime limits honestly; Phase 13 independently exercises these controls alongside every product feature.

### Phase 13 — Full testing and essential red teaming (second-last)

Freeze feature scope, exercise the Phase 12 safety checklist, and maintain a traceable table of every feature, module, backend, command, file schema and integration, its test, and its result.
Unit tests remain model/network independent; integration jobs use pinned local fixtures and explicit hardware/model requirements.
Run default/minimal feature combinations, supported backend builds, clean-session and long-session runs, replay, crash recovery, memory plateaus, dependency/license review and dependency vulnerability scanning.
Measure load peak, long prefill, repeated turns, tool-heavy turns, cancellation, reset, model switching, unload and restart.
Test all accepted format targets on their declared hardware; keep gaps visible and block the corresponding release claim.

Limit red teaming to actual trust boundaries:

- Instructions in skill text, memory, model replies or MCP output trying to grant authority or leak secrets.
- Tool/schema changes after approval, forged tool identity, response replay and hidden write retries.
- Workspace traversal, symlink escapes, session role spoofing, file corruption and concurrent edits.
- Terminal escape/control sequences, deceptive links and oversized Unicode/tool output.
- MCP child-process resource exhaustion, hostile redirect destinations, credential leaks and cancellation races.
- Invalid model/cache files through the selected dependencies, with clear acknowledgement that Rust does not sandbox native inference libraries.

Do not expand this into generic jailbreak research about the model's opinions.
Success means runtime permissions and file/process/transport limits hold even when the model is persuaded.

**Exit:** no open release-blocking correctness, data-loss, secret-leak or unauthorized-action issue; all coverage rows have evidence; failed cases remain in the report.
Use a deterministic core test suite plus separately recorded real-model/hardware results; model text is not a universal golden string across GPUs.

### Phase 14 — Repository structure, installation and presentation (last)

Review the actual modules and ownership boundaries; remove unused code and organize only around responsibilities that now exist.
Do not defer a buildable workspace until this phase: it is the final structural cleanup, not the first structure.
Keep `core` free of dependencies; concrete engine types stay inside engine crates; the binary composes the selected runtime.
A separate adapter for each backend is justified when it exists, not generated as an empty crate today.

Ship versioned per-platform/backend artifacts with checksums, third-party licenses, minimum OS/driver requirements and a documented update/uninstall path.
Keep optional runtime libraries/models separate and account for their disk footprint.
Test clean machines/VMs for install, first model selection, TUI start, updates, offline run and uninstall without deleting user Markdown.
Do not publish an installer that silently downloads an inference environment or model.
Sign/notarize where the distribution requires it and secrets are available; otherwise describe the limitation accurately.

Make the README useful to three readers: a researcher inspecting reproducibility and model/backend constraints, an engineer installing/contributing, and a recruiter scanning the purpose, architecture and a real demo.
Include a real TUI capture, concise quick start, tested support matrix, measured footprint/performance with reproduction commands, architecture links, limits, contribution guide and license.
Avoid decorative badges that imply unsupported hardware or unmeasured performance.
Keep release claims tied to Phase 13 evidence.
Any packaging fix that changes behavior reruns its affected tests before release.

**Exit:** a fresh reader can install, run a supported local model, customize Markdown, connect an authorized MCP server, inspect memory and uninstall from the README alone.
Publish the feature/hardware evidence and release notes with the artifact.
