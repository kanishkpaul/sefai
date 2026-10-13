# Build plan

sefai is built in seven phases.
Each phase ends with something a person can run, and each has exit criteria that are checked, not assumed.
Phases are ordered by dependency: correctness of the engine first, then the conversation loop that makes it fast, then the companion abilities on top, then the ways to reach it.

The stack behind these choices is in [`stack.md`](stack.md); the structure is in [`../ARCHITECTURE.md`](../ARCHITECTURE.md).

| Phase | Delivers | You can |
| --- | --- | --- |
| 0. Foundations | Workspace, core contracts, persona ability, docs, CI | Print the prompt a persona produces |
| 1. Engine | llama.cpp on Metal and CUDA, correctly | Run a model once from the command line, and benchmark it |
| 2. Conversation | Engine thread, KV reuse, saved persona cache, chat REPL | Talk, with each turn paying only for new tokens |
| 3. Memory | SQLite store, embeddings, recall, summaries | Be remembered across restarts |
| 4. Autonomy and character | Proactive messages, quiet hours, Character Card import | Hear from the companion first |
| 5. Interfaces | Local OpenAI-compatible API, Tauri desktop shell | Use it from a window, a tray, or another app |
| 6. Distribution | Release builds for each platform and backend | Download one file and run it |

## Phase 0 — Foundations (done)

- Cargo workspace with `crates/core`, `crates/sefai` and `abilities/persona`.
- `Ability` trait, `Abilities` collection with failure isolation, `Prompt` with section merging.
- Ability registry in the binary, with each ability behind its own Cargo feature.
- `sefai abilities` and `sefai prompt --persona <file> <message>`.
- Style guide, architecture, stack, this plan, agent instructions, CI, and a commit hook that blocks tool and agent attribution.

**Exit criteria:** `cargo fmt --check`, `cargo clippy -- -D warnings` and `cargo test` pass on macOS and Linux, and the binary builds with and without the persona feature.

## Phase 1 — Engine

New crate `crates/engine-llama`, depending on `llama-cpp-2` pinned exactly, with default features off and `metal` / `cuda` / `vulkan` passed through.
`core` gains the engine trait the runtime will call.

Work, in order:

1. **Load.** Open a GGUF file, read its metadata (architecture, context length, chat template, special tokens), log the device list.
2. **Place.** `--gpu-layers auto|N|all` where `auto` uses llama.cpp's memory-fit parameters and is the default. `--device <index>|cpu`, where `cpu` passes an empty device list so nothing runs on the GPU, and choosing a single device sets split mode `none`.
3. **Prefill correctly.** Feed the prompt in chunks of `n_batch` tokens with continuous positions, requesting logits only for the last token. Check that prompt plus requested output fits in the context before starting, and fail with a clear message if not. `--ctx-size` with a default of 8192, capped at the model's training length.
4. **Template.** Render the GGUF's Jinja chat template with `minijinja`, tokenize with special-token parsing on, and add BOS only when the vocabulary asks for it. `--raw` bypasses the template.
5. **Sample.** A sampler chain in llama.cpp's order: penalties, top-k, top-p, min-p, temperature, then a seeded distribution. Defaults follow llama.cpp (top-k 40, top-p 0.95, min-p 0.05, temperature 0.8). `--greedy` for deterministic output. Never call `accept` after `sample`, which already accepts.
6. **Stream.** A UTF-8 decoder that holds back an incomplete trailing character until the next token completes it. Control tokens are never printed. Stop on every end-of-generation token the vocabulary defines.
7. **Measure.** `sefai bench` reports load time, time to first token, prefill tokens per second and decode tokens per second separately.

Commands: `sefai run <model> -p <text>` and `sefai bench <model>`.

**Exit criteria:**

- A prompt of 3,000 tokens works.
- Bengali and emoji output round-trips without `�` in a test.
- `--device cpu` shows zero GPU buffers in llama.cpp's load log.
- Decode and prefill within 5% of `llama-bench` on the same model and build, on an Apple Silicon Mac and an NVIDIA GPU.
- A smoke test in CI runs a tiny instruct GGUF (cached between runs) on CPU.
- A CUDA build compiles in CI.

## Phase 2 — Conversation

New crate `crates/runtime`.

1. **Engine thread.** The model and context live on one OS thread. The runtime sends requests over a bounded channel and receives tokens over another.
2. **KV prefix reuse.** The runtime keeps the token sequence that mirrors the KV cache. Each turn, it finds the longest common prefix with the new prompt, removes cached tokens after it, and prefills only the rest.
3. **Persona cache.** The tokens for the persona section are prefilled once and their KV state saved to disk, keyed by model file, persona text and settings. On start, the state is restored instead of recomputed.
4. **Context pressure.** When the next turn would not fit, drop the oldest exchanges after the persona and re-prefill, or summarize them once phase 3 exists. Never crash on a full context.
5. **Configuration.** One TOML file in the platform's config directory, with a table per ability. Abilities can be compiled in but switched off here.
6. **REPL.** `sefai chat` with streaming output, `/reset`, `/persona`, and Ctrl-C to stop a reply without exiting.

**Exit criteria:**

- The tenth turn of a conversation starts replying in time proportional to its own length, measured.
- Restarting with an unchanged persona skips the persona prefill, measured.
- Idle memory of `sefai chat` beyond the model and KV cache is under 40 MB.

## Phase 3 — Memory

New crates `crates/store` and `abilities/memory`. The `Ability` trait gains an `observe` hook, called after each exchange.

1. **Store.** SQLite with numbered SQL migrations: exchanges, memories, summaries, per-ability key-value state.
2. **Embeddings.** A small GGUF embedding model loaded by the engine; vectors stored as `f32` blobs.
3. **Recall.** Before a turn, embed the user's message and add the closest memories within a token budget, by cosine similarity computed in Rust.
4. **Writing memories.** After an exchange, store it; periodically ask the model to extract durable facts and to summarize older conversation.
5. **Inspect and forget.** `sefai memory list`, `sefai memory forget <id>`, and an export to plain text.

**Exit criteria:**

- A fact mentioned once is recalled after a restart in a scripted test.
- Recall over 10,000 memories takes under 20 ms on a laptop.
- Removing the memory ability leaves every other test passing.

## Phase 4 — Autonomy and character

New crate `abilities/autonomy`. The `Ability` trait gains a `tick` hook that may propose an unprompted message.

1. **Scheduler.** The runtime calls `tick` on a fixed interval with an injected clock.
2. **Triggers.** Time of day, time since the last exchange, and things memory says are worth following up.
3. **Restraint.** Quiet hours, a daily limit, a cooldown after an ignored message, and a global switch.
4. **Delivery.** Proactive messages go to whatever interface is attached: the REPL, the desktop notification, or the API's event stream.
5. **Character Cards.** The persona ability imports Character Card V2 and V3 files (JSON, or embedded in PNG), including their lorebooks.

**Exit criteria:**

- Scheduler tests run against a fake clock with no sleeps.
- Quiet hours and the daily limit hold in tests.
- A community V3 card imports and renders.

## Phase 5 — Interfaces

1. **Local API.** `crates/server` with `axum`: OpenAI-compatible chat completions with streaming, plus an event stream for proactive messages. Localhost only unless configured otherwise. Optional feature `server`.
2. **Desktop shell.** `apps/desktop` with Tauri 2: a chat window, tray icon, notifications for proactive messages, start at login. It embeds the runtime in-process rather than talking to a separate server.

**Exit criteria:**

- SillyTavern can use sefai as its backend through the API.
- The desktop app's idle memory, excluding the model, is under 150 MB with the window closed to the tray.

## Phase 6 — Distribution

1. Release workflow building macOS arm64 with Metal, Linux x86_64 with CUDA and CPU, and Windows x86_64 with CUDA and CPU.
2. Evaluate `llama-cpp-2`'s `dynamic-backends` so one Linux or Windows download chooses CUDA or CPU at runtime.
3. Checksums and signed macOS builds. A Homebrew tap once there are users to serve.

**Exit criteria:**

- A clean machine of each platform runs the downloaded binary with no other installs beyond the GPU driver.
- Binary size budgets in `stack.md` hold.

## Later, and only with a reason

These are deliberately not scheduled. Each needs a concrete user need before work starts.

- An MLX engine for Apple Silicon, behind the same engine trait, if measurements show the decode gain is worth a second engine.
- Speculative decoding with a draft model or the model's own multi-token-prediction heads.
- Images and audio through llama.cpp's multimodal library.
- Tool use, with grammar-constrained output.
- Vulkan as a supported backend for AMD and Intel GPUs.

## How a phase is done

A phase is done when its exit criteria are demonstrated with commands and numbers in the pull request that closes it, the docs it touched are updated, and `ARCHITECTURE.md` still describes the code.
