# Architecture

This is the map of sefai: what the parts are, where they live, and the rules that keep them separate.
It describes things that change rarely. Details that move fast live in module docs next to the code.

## Bird's-eye view

sefai is a local AI companion: a single program that keeps a language model loaded on the user's own machine and holds one long-running relationship with them.
It answers when spoken to, remembers what matters, and can speak first.

Every turn follows the same path:

1. The runtime takes the user's message and the conversation so far and starts a `Prompt`.
2. Each enabled **ability** gets to add system text to the prompt: the persona says who the companion is, memory adds what it recalls, and so on.
3. The prompt is rendered through the model's chat template and tokenized.
4. The **engine** reuses the cached prefix of the previous turn, prefills only the new tokens, and streams the reply.
5. Abilities observe the finished exchange (to store memories, update mood) and the reply is shown.

Between turns, a scheduler gives abilities a periodic tick, which is how the companion decides to reach out on its own.

Steps 1 and 2 exist today; the rest arrive in the phases of [`docs/build-plan.md`](docs/build-plan.md).

## Code map

```text
crates/
  core/            sefai-core: messages, prompts, the Ability trait
  sefai/           the `sefai` binary: command line and composition root
  engine-llama/    sefai-engine-llama: llama.cpp behind sefai's engine interface   (phase 1)
  runtime/         sefai-runtime: turn loop, engine thread, KV reuse, scheduler     (phase 2)
  store/           sefai-store: SQLite schema, migrations, queries                   (phase 3)
  server/          sefai-server: local OpenAI-compatible API                         (phase 5)
abilities/
  persona/         who the companion is
  memory/          what it remembers                                                  (phase 3)
  autonomy/        when it speaks first                                               (phase 4)
apps/
  desktop/         Tauri shell: window, tray, notifications                           (phase 5)
docs/              stack, build plan, style, abilities guide, research
```

### `crates/core`

The shared vocabulary: `Message`, `Role`, `Prompt`, `Section`, the `Ability` trait and the `Abilities` collection that runs them in order.
It has no dependencies and does no I/O.
When a concept is needed by both an ability and the runtime, it belongs here.

### `crates/sefai`

The binary. It parses the command line, reads configuration, and builds the enabled abilities from a registry table in `src/abilities.rs`.
This is the **composition root**: the only place that names concrete abilities and concrete engines.

### `crates/engine-llama`

Everything that knows llama.cpp exists: loading GGUF files, fitting layers to GPU memory, device selection, batching the prefill, the sampler chain, chat-template rendering with `minijinja`, UTF-8-safe detokenization, embeddings, and saving and restoring KV state.
It implements an engine interface defined in `core`, so the rest of sefai never sees a llama.cpp type.

### `crates/runtime`

Owns a conversation. It runs the engine on a dedicated thread, keeps the token history that mirrors the KV cache so each turn prefills only what changed, decides what to do when the context fills up, and drives ability hooks and the autonomy scheduler.

### `crates/store`

The single SQLite database: exchanges, memories with their embeddings, summaries, ability state. Schema changes are numbered SQL migrations.

### `abilities/*`

One crate per ability. Each implements `sefai_core::Ability` and exports a `NAME` constant.
See [`docs/abilities.md`](docs/abilities.md).

## Invariants

These are the rules that make the structure worth having. Breaking one needs a design discussion first, not a quick fix.

- **`core` depends on nothing.** No model, storage, async or UI crates, ever.
- **Abilities depend on `core`, and on `store` if they persist data. Never on each other, never on an engine or the runtime.** They reach outside services only through what the hooks hand them.
- **Only the `sefai` binary names concrete abilities and engines.** Adding or removing an ability never touches `core`, `runtime` or another ability.
- **Only `engine-llama` knows about llama.cpp.** No llama.cpp type appears in another crate's public API.
- **One thread owns the model and its context.** Other code sends it requests over bounded channels. Nothing else touches the engine.
- **No `unsafe` in sefai's crates.** FFI lives in the bindings crate. The workspace forbids `unsafe_code`.
- **A failing ability degrades a turn; it never aborts one.** Whatever a failing ability added to the prompt is removed, the other abilities still run, and the failure is logged.
- **Every queue, cache and loop has a named upper bound.**
- **Nothing leaves the machine unless the user turned it on.** No telemetry. The local API binds to localhost by default.

## Boundaries

| Boundary | Crossed by | Notes |
| --- | --- | --- |
| Abilities and runtime | The `Ability` trait | Hooks are added only when an ability needs one. Today: `prepare`. Planned: `observe` after each exchange (phase 3), `tick` for proactive messages (phase 4). |
| Runtime and engine | An engine trait in `core` plus a request channel | Requests are "prefill these tokens at these positions", "sample with this chain", "embed this text", "save or restore sequence state". |
| sefai and llama.cpp | The `llama-cpp-2` crate, pinned exactly | Upgraded deliberately, one commit per upgrade, benchmarked before and after. |
| sefai and the outside world | The command line, the optional local API, the optional desktop shell | All three drive the same runtime. |

## Cross-cutting concerns

**Errors.** Libraries return typed errors; the binary adds context and prints the whole chain. Ability failures are reported, not fatal.

**Performance.** Decode speed is memory bandwidth divided by bytes read per token, and the engine should stay within 5% of `llama-bench`. Turn latency is dominated by prefill, which is why KV-prefix reuse and the saved persona cache matter more than any micro-optimization. Budgets are in [`docs/stack.md`](docs/stack.md).

**Footprint.** Optional parts (server, desktop shell, each ability) are Cargo features, so a build only contains what it uses.

**Testing.** Unit tests live beside the code. Tests that need a real model read the `SEFAI_TEST_MODEL` environment variable and run in CI with a tiny cached GGUF. Time is injected, never read from the system clock, in anything the scheduler touches.

**Configuration.** One TOML file, parsed strictly. Each ability reads its own table, named after the ability.

**Privacy.** Conversations, memories and models stay on disk on the user's machine.
