# sefai

A small local LLM runner and terminal harness in Rust.
One model stays loaded beside your other apps, a persistent terminal conversation drives it, prompts, skills, preferences, memory and sessions live in Markdown files you can edit, and every tool call waits for your exact approval.
It runs GGUF on CPU or Metal, MLX, GPTQ and AWQ on Apple Silicon, and ONNX on CPU or WebGPU; an idle session uses 4–13 MiB.

sefai 0.1.0 is pre-release: all planned phases are built and tested, and the [testing record](docs/testing.md) lists what each claim rests on.
NVIDIA and AMD GPU backends compile but have never run on that hardware, so no release claims them.

## What it looks like

A real `chat --plain` session with the 135M-parameter SmolLM2 fixture on an Apple M5, using an all-engine build:

```text
[ready] session session-… · local · /setup, /model PATH or /profile ID · /help

3 candidates for chat at context 4096 · available 8.9 GiB
smollm2-135m-gguf · gguf · lower memory · 0.19–0.23 GiB estimated · comfortable · Apache-2.0
qwen3-0-6b-gptq · gptq · balanced · 1.16–1.29 GiB estimated · comfortable · Apache-2.0
smollm2-1-7b-onnx · onnx · largest that still fits · 3.94–4.23 GiB estimated · comfortable · Apache-2.0
estimates use Phase 10 measurements from one Apple M5; nothing was downloaded or loaded

[ready] selected small · session session-… · /help

[user]
In one sentence, what is a river delta?
[assistant]
A river delta is a region characterized by a combination of rivers, lakes, and wetlands that are formed by the movement of water and sediment over time.
[ready] smollm2-135m.gguf · cpu · ctx 50/4096 · local · 2 records
```

Without `--plain`, the same conversation runs full-screen with a transcript, composer, status line and approval overlays; this frame is drawn by the interface's own rendering code in its [snapshot test](tests/fixtures/tui/streaming.txt), and [the terminal guide](docs/tui.md) lists the controls:

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you
Hello

☾ sefai
你 好  é · streaming reply






────────────────────────────────────────────────
❯

Esc stop · Ctrl+D quit
 tiny · cpu · ctx 20/512 · local
```

## Quick start

Each tagged release drafts one archive per platform with `SHA256SUMS`; download yours, check it and run it, and see [install.md](docs/install.md) for every platform, unsigned-binary warnings, updating and uninstalling.
Until the first release is published, build from source as shown below.

```sh
shasum -a 256 -c sefai-0.1.0-macos-arm64.tar.gz.sha256
tar -xzf sefai-0.1.0-macos-arm64.tar.gz
./sefai-0.1.0-macos-arm64/sefai chat
```

Inside the conversation, `/setup recommend` lists pinned models that fit this machine, `/setup get ID confirm` downloads and verifies one, and `/profile ID` loads it; `sefai setup import ID PATH --format gguf` uses a model you already have.
To build from source instead, install Rust, CMake, a C/C++ compiler and libclang, then run `cargo run --locked --release -- chat`.

## What runs where

| Platform | Qualified engines | Evidence |
| --- | --- | --- |
| macOS on Apple Silicon | GGUF on CPU and Metal, MLX, GPTQ and AWQ on MLX, ONNX on CPU and WebGPU | Local Apple M5 tests, lifecycle contract and measurements |
| Linux x86_64 and arm64, macOS on Intel, Windows x86_64 | GGUF on CPU; ONNX on CPU except Intel macOS | Hosted CI: lifecycle contract, footprint, clean install |
| NVIDIA and AMD GPUs | None; CUDA and Vulkan compile only | Follow-up gate for contributors with the hardware |

Support depends on the architecture, quantization and export, not just the file format; the [stack](docs/stack.md) and each engine's guide name the qualified model sets.

## Measured

| Measurement on an Apple M5 MacBook Air | Result |
| --- | --- |
| Idle terminal session, GGUF build | 8.5 MiB resident (target 40 MiB) |
| Idle terminal session, MCP build | 9.0 MiB resident (target 60 MiB) |
| SmolLM2-135M Q4_K_M, CPU, 4 threads | 1,020 tok/s prefill, 358 tok/s decode, 226 MiB peak |
| SmolLM2-135M Q4_K_M, Metal | 18,897 tok/s prefill, 348 tok/s decode, 189 MiB peak |
| Qwen3-0.6B GPTQ Int8 on MLX | 2,541 tok/s prefill, 84 tok/s decode, 1,258 MiB peak |

Reproduce any row with [`scripts/measure_footprint.py`](scripts/measure_footprint.py) and the commands in [footprint.md](docs/footprint.md), which also holds the hosted-runner results, sweeps and limits.

## Customize with Markdown

```sh
sefai --workspace ~/notes/sefai workspace init
sefai --workspace ~/notes/sefai prompt hello
```

`system.md`, `preferences.md`, model profiles, skills, memories, sessions and MCP connections are Markdown with strictly typed front matter; prose never changes a setting or grants a permission.
`/memory` lists memories and `/memory ID` shows one in the conversation; `sefai workspace remember`, `edit` and `forget` change them with revision checks.
The [workspace guide](docs/workspace.md) documents schema 1, precedence, limits and atomic writes.

## Safety model

Models run locally and nothing leaves the machine unless you enable an [MCP connection](docs/mcp.md) or confirm a model download.
Every tool call shows the exact server, tool, destination and arguments and runs only after you approve that one call; model output, memories, skills and server responses are untrusted data.
MCP servers you launch run with your privileges and are not sandboxed.
The [safety checklist](docs/safety.md) maps every trust boundary to its control and test, and states the residual risks.

## Read more

| Document | Purpose |
| --- | --- |
| [Install](docs/install.md) | Archives, first run, update, uninstall, building from source |
| [Model setup](docs/setup.md) | Hardware check, estimates, catalog, downloads and imports |
| [Runner](docs/runner.md) | Commands, limits, engine selection and the lifecycle contract |
| [Harness](docs/harness.md) | Tool protocol, approvals, budgets and recovery |
| [Architecture](ARCHITECTURE.md) | Crates, ownership boundaries and invariants |
| [Footprint](docs/footprint.md) | Memory, throughput and the platform matrix |
| [Testing](docs/testing.md) | Coverage, red team, dependency audit and known limits |
| [Build plan](docs/build-plan.md) | Phases, scope and acceptance gates |
| [Verification](docs/verification.md) | Phase-by-phase evidence, including failures |
| [Contributing](CONTRIBUTING.md) | Setup, checks and publication policy |

## License

MIT; see [LICENSE](LICENSE).
Release archives include `THIRD_PARTY_NOTICES.txt` with the licenses of every bundled dependency, including llama.cpp, MLX, ONNX Runtime and Dawn.
