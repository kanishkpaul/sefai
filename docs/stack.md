# Language and stack

Decided October 2026. This records what sefai is built with and why, so later changes argue with reasons instead of habits.

**Short version:** sefai is a single Rust binary that embeds llama.cpp through the `llama-cpp-2` bindings, with Metal on Apple Silicon and CUDA on NVIDIA.
State lives in one SQLite file. Everything beyond the core (server, desktop shell, individual abilities) is an optional crate behind a Cargo feature.

## What the stack has to deliver

| Requirement | What it rules in or out |
| --- | --- |
| Runs on Apple Silicon and NVIDIA GPUs at full speed | Needs first-class Metal and CUDA kernels for quantized models. Rules out anything that treats either as an afterthought. |
| Fast | Decode speed is set by memory bandwidth, so the engine must be as close to the hardware ceiling as llama.cpp itself. Turn latency is dominated by prompt prefill, so the process must keep the model and its KV cache alive between turns. |
| Low footprint | Sits beside the user's other apps for hours. No interpreter, no bundled browser, no background services. Small idle memory and a small binary. |
| Companion-shaped | One long-lived relationship, not many anonymous requests. Needs token-level control: KV-cache reuse, persona state on disk, custom stopping, in-process embeddings. |
| Easy to grow and prune | Abilities must be addable and removable without touching unrelated code. |

## Language: Rust

| Option | Verdict |
| --- | --- |
| **Rust** | **Chosen.** Native speed with no garbage collector, so idle memory stays small and latency stays predictable. Memory safety for the parts that aren't the engine. Static single-binary output on macOS, Linux and Windows. Mature bindings to llama.cpp and good crates for everything else the companion needs. Cargo features give compile-time removal of abilities for free. |
| C++ | llama.cpp's own language, so zero binding cost. Rejected because the companion layer (scheduling, storage, networking, configuration) is where most bugs will live, and C++ makes those bugs memory-safety bugs. |
| Zig | Excellent for systems code and C interop. Rejected for a young ecosystem: no mature SQLite, HTTP or desktop story, and a language still breaking between versions. |
| Go | What Ollama uses. Rejected because cgo calls add overhead and complexity on the hot path, the garbage collector adds memory and pauses, and Ollama's own history shows how hard it is to stay close to upstream llama.cpp from there. |
| Swift | Superb on Apple Silicon, but CUDA support would be a second codebase. |
| Python | What the first sefai used. Its interpreter, dependency weight and process model are the footprint and latency problems this rewrite exists to fix. |

## Inference engine: llama.cpp

| Option | Verdict |
| --- | --- |
| **llama.cpp (via bindings)** | **Chosen.** Metal and CUDA are both first-class and tuned by the people who wrote the quantized kernels. GGUF is the format local models ship in, with the widest range of quantizations. New model architectures usually work within days. MIT-licensed, and since February 2026 backed by Hugging Face. |
| mistral.rs | The strongest pure-Rust engine, with GGUF loading, CUDA and Metal. Rejected as the primary engine because it supports a curated model list, its performance claims are self-reported and CUDA-only, and it trails llama.cpp on new architectures. It stays the first candidate if a second engine is ever needed. |
| candle, burn | Frameworks for building an engine, not engines. |
| MLX | Often 1.2–1.8× faster at decode than llama.cpp on Apple Silicon for small and medium models, but Apple-only. A possible second engine for macOS later, which is why the engine sits behind its own crate. |
| vLLM, SGLang | Built for serving many concurrent users on datacenter GPUs. sefai serves one person. |

### Why embed instead of running `llama-server`

Large apps such as Tabby and Jan supervise a `llama-server` child process and talk HTTP to it.
That isolates crashes and FFI churn, and it suits apps serving many conversations.
sefai keeps one relationship alive for hours, so the win is in what only an embedded engine can do: reuse the KV cache across turns without re-sending history, save the persona's KV state to disk, stop on custom conditions, and embed memories in the same process.
The old sefai spawned a fresh `llama-cli` for every reply and paid a full model load and prefill each time; embedding removes that entirely.

## Bindings: `llama-cpp-2`, pinned exactly

| Option | Verdict |
| --- | --- |
| **`llama-cpp-2`** (utilityai/llama-cpp-rs) | **Chosen.** About 1.6M downloads and steady releases every one to three weeks; it was 0.1.159 on 2026-10-07. Exposes everything the plan needs: memory-fit parameters, device lists and split modes (so CPU-only and single-GPU modes are real), KV-cache sequence editing and state save/restore, embeddings, multimodal (`mtmd`), speculative decoding, grammar sampling, and `dynamic-backends` for shipping one binary that loads CUDA at runtime. |
| `llama-cpp-4` | Tracks upstream within days and wraps llama.cpp's Jinja chat-template engine. Rejected as the default because it has one maintainer and was dormant for thirteen months in 2025, and it patches llama.cpp locally. It shares most type names with `llama-cpp-2`, so it remains the fallback. |
| Our own `-sys` crate | The escape hatch if both bindings stall: bindgen over `llama.h`, built with the `cmake` crate, pinned to a llama.cpp release tag. Not worth owning until it has to be. |

`llama-cpp-2` does not follow semver, so it is pinned with `=` and upgraded deliberately, one commit per upgrade, with `sefai bench` numbers before and after.
Chat templates are rendered with **`minijinja`** (plus `minijinja-contrib` for Python-style string methods) from the Jinja template stored in each GGUF file, because llama.cpp's C-level `llama_chat_apply_template` only recognises a fixed list of formats.

## The rest of the stack

| Concern | Choice | Reason |
| --- | --- | --- |
| Storage | SQLite through `rusqlite` with the `bundled` feature | One file, transactional, zero setup, no server. Migrations are plain SQL files run at startup. |
| Memory recall | Brute-force cosine similarity over `f32` blobs in SQLite; `sqlite-vec` if recall needs outgrow it | 10,000 memories at 768 dimensions is about 30 MB scanned in a few milliseconds. An index is complexity sefai doesn't need until it has far more memories than one relationship produces. |
| Embeddings | A small GGUF embedding model loaded by the same engine | No second runtime, and recall stays local. |
| Concurrency | One dedicated OS thread owns the model and context; everything else talks to it over bounded channels. `tokio` only in crates that do network I/O or timers. | A llama.cpp context is single-threaded by nature. Keeping async out of the core keeps it small and easy to test. |
| Local API | `axum`, OpenAI-compatible, bound to localhost by default | Lets other frontends (SillyTavern, editors) use sefai's persona and memory. Optional feature. |
| Desktop shell | Tauri 2 | Uses the system web view, so the shell adds megabytes rather than the hundreds an Electron app costs. Tray icon, notifications and start-at-login on all three platforms. Optional crate, built last. |
| Command line | `clap` | Standard, derive-based, good help output. |
| Configuration | TOML through `serde` | Human-editable, comments allowed, strict parsing with `deny_unknown_fields`. |
| Errors | `thiserror` in libraries, `anyhow` in the binary | Typed errors where callers match on them, context chains where humans read them. |
| Logging | `tracing` | Structured, and llama.cpp's own log output can be routed into it. |

## Hardware targets

| Platform | Backend | Cargo feature | Status |
| --- | --- | --- | --- |
| macOS on Apple Silicon | Metal | `metal` | Primary |
| Linux x86_64, NVIDIA | CUDA | `cuda` | Primary |
| Windows x86_64, NVIDIA | CUDA | `cuda` | Primary |
| Any | CPU | none | Always available as a fallback |
| AMD and Intel GPUs | Vulkan | `vulkan` | Best effort, not part of the guarantee |

CUDA builds limit compiled GPU architectures to what is supported (Turing through Blackwell at the time of writing) to keep the binary size sane.
Distribution starts with one binary per backend; switching to `dynamic-backends`, so one download picks CUDA or CPU at runtime, is planned in phase 6 of the build plan.

## Footprint and speed budgets

These are targets the build plan holds each phase to. A change that breaks one needs a reason in its commit message.

| Measure | Budget |
| --- | --- |
| Release binary, CPU or Metal build, no server or UI | ≤ 15 MB |
| Idle memory beyond the model's own mapped weights and KV cache | ≤ 40 MB for the CLI, ≤ 60 MB with the local API |
| Decode and prefill speed | Within 5% of `llama-bench` for the same model, build and settings |
| Second-turn time to first token | Proportional to the new tokens only, never to the whole history |
| Restart with a saved persona cache | No persona prefill: load the model, restore state, answer |

## Research behind this

The ecosystem survey, inference arithmetic and binding comparison that informed these choices are in [`research/2026-10-ecosystem-and-inference.md`](research/2026-10-ecosystem-and-inference.md).
It recommended `llama-cpp-4`; this document deliberately chooses `llama-cpp-2` for maintenance risk and packaging, for the reasons above.
