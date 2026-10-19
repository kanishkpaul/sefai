# Architecture

sefai is a small local LLM runner and terminal harness.
It keeps one selected model loaded, composes prompts from Markdown-owned instructions and session records, and executes approved tools through a bounded loop.
The TUI is its permanent home.

## What exists

`crates/core` contains dependency-free `Message`, `Role`, `Prompt` and `Section` types.
`crates/engine-llama` owns the exactly pinned llama.cpp binding, GGUF model loading, native templates/tokenizer, bounded prefill, sampling and streaming.
`crates/sefai` composes the `run`, `bench`, `devices` and minimal `prompt` commands, cancellation and terminal sanitization.
An engine lends one model at a time; requests allocate one temporary context while retaining the model weights.
The binary workspace module validates typed Markdown and composes selected instructions, model profiles and bounded literal memory recall.
There is no TUI, MCP client or persistence writer yet.
The [workspace guide](docs/workspace.md) defines schema 1 and its limits.
The [runner guide](docs/runner.md) defines current limits and local qualification.
The [build plan](docs/build-plan.md) separates implementation phases and acceptance gates.
The [audit](docs/bug-fix-plan.md) records the legacy runner defects that a replacement must test.

## Planned ownership

| Part | Owns | Does not own |
| --- | --- | --- |
| `core` | Shared messages and minimal contracts needed by multiple parts | I/O, backend types, UI or permissions derived from model prose |
| `sefai` binary | Composition, user configuration, commands and terminal lifecycle | Native inference implementation |
| `engine-llama` | GGUF loading, tokenization/template, sampling and cache for llama.cpp | MCP or filesystem policy |
| Other optional engine adapters | MLX, ONNX and GPTQ/AWQ runtime-specific code | Unrelated formats or mandatory base-package dependencies |
| Harness module, extracted only when needed | Turn lifecycle, budgets, cache invalidation, permission decisions and cancellation | Trust in tool/model instructions |
| Workspace module | Versioned Markdown schema, bounded reads, atomic writes and conflict handling | A hidden canonical database |
| MCP module | Protocol/transport, server lifecycle, credential references and result limits | Authority to approve its own tool calls |
| TUI module | Input, transcript, status, approvals and rendering | Model execution on its event thread |

Modules become crates only when a real dependency/lifecycle boundary warrants it.
Do not create empty crates for future phases.
An engine interface becomes useful with a second concrete backend; avoid an exhaustive abstraction before that.
Only the binary composes concrete engines and optional capabilities.

## Invariants

- `core` depends on nothing and does no I/O.
- Only `engine-llama` touches llama.cpp, and only each engine adapter touches its native runtime.
- No unsafe code in sefai's crates; native dependencies remain an explicit trust boundary.
- One owner controls the active model/context; all cross-thread queues have named bounds.
- Markdown is canonical for customization, durable memories and sessions; binary weights/caches and secret references are explicit exceptions.
- Model output, Markdown body text and MCP responses cannot grant permissions.
- Every external write is checked against current user authority and the exact approved action.
- Context, output, tools, file reads, caches and loops have explicit bounds and failure behavior.
- Switching engines never silently selects a cloud service, changes quantization, or converts a checkpoint.
- Cancellation reclaims owned work; dispatched side effects may require an unknown outcome rather than an automatic retry.
- No telemetry; outbound traffic occurs only for deliberately enabled downloads/connections/tools.

## Turn path

The TUI sends user intent to the harness.
The workspace loader supplies selected system text, preferences, skills and memory under a token budget.
The engine renders the model-native template and validates context/output capacity.
The engine owner reuses only the valid token prefix and streams sanitized display events.
A tool proposal returns to the harness for validation and permission before the MCP client dispatches it.
The bounded result becomes untrusted input for the next generation step.
Completed and interrupted turns are persisted as versioned Markdown records with explicit outcome states.

## Cache and trust

Caches are disposable derivatives, never the authoritative copy of editable user data.
Model identity, engine revision, template, actual rendered tokens and cache format/placement decide reuse.
Changing a Markdown instruction invalidates the divergent suffix.
A cached model response cannot authorize an action.
MCP stdio processes have the user's OS privileges unless separately sandboxed; connection approval is not a claim that sefai has sandboxed third-party code.
