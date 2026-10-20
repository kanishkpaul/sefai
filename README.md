# sefai

A small Rust LLM runner and terminal harness, built around files you can read and edit.
The goal is low RAM/VRAM use, a persistent TUI, Markdown-owned prompts/skills/preferences/memory, and explicit MCP connections.

**Status: Phases 1–3 merged; Phase 4 harness is locally qualified; PR review and CI are pending.**
Pinned-model CPU tests pass on Windows, macOS and Linux; local Metal tests and CUDA/Vulkan compilation also pass.
CUDA/Vulkan execution and the full hardware matrix remain later qualification gates.
Markdown schema validation and selected instruction/profile loading are implemented.
Safe writes, explicit memory/session persistence and bounded session reads are implemented.
The terminal conversation, model worker and automatic Markdown turn persistence are implemented locally.
MCP connections remain planned.
See [the workspace guide](docs/workspace.md) for schema and selection controls.

## Direction

| Area | Planned support |
| --- | --- |
| Model runtimes | GGUF first; optional MLX, ONNX, GPTQ and AWQ adapters |
| Systems | Windows, macOS and Linux |
| GPUs | NVIDIA/CUDA, Apple/Metal, AMD/Vulkan; CPU fallback |
| Customization | Markdown source of truth; binary models/caches and credential references are explicit exceptions |
| Interface | Terminal TUI throughout the product's life |

Format support depends on the model architecture, quantization and backend.
The [compatibility plan](docs/stack.md) distinguishes planned coverage from measured support.

## Run locally

Install stable Rust, CMake, a C/C++ compiler and libclang, then:

```sh
cargo run --locked -- chat --model /path/to/model.gguf
cargo run --locked -- devices
cargo run --locked -- run --model /path/to/model.gguf "hello"
```

The runner loads the chosen local GGUF and streams a reply; CPU is the explicit default.
See the [runner guide](docs/runner.md) for GPU builds, limits, benchmarks and model tests.
`cargo run --locked --no-default-features -- prompt hello` retains the diagnostic without native inference dependencies.
There is no published installer yet.

## Customize with Markdown

Initialize a workspace, edit its `system.md` or `preferences.md`, then inspect the composed prompt:

```sh
cargo run --locked -- --workspace /path/to/workspace workspace init
cargo run --locked -- --workspace /path/to/workspace prompt hello
```

Model profiles, explicitly selected skills, literal memory recall and session resume use the same workspace.
The [workspace guide](docs/workspace.md) documents schema 1, precedence, limits, safe writes and session framing.
The [Phase 2 closeout](docs/phase2-closeout.md) records the merged workspace foundation.
The [terminal guide](docs/tui.md) documents the interactive controls, limits and plain mode.

## Read the plan

| Document | Purpose |
| --- | --- |
| [Build plan](docs/build-plan.md) | Phases, retained scope and acceptance gates |
| [Bug-fix plan](docs/bug-fix-plan.md) | Main-branch audit and ordered regression work |
| [Research](docs/research/2026-10-19-inference-and-engineering.md) | Formats, memory arithmetic, Rust practice and sources |
| [Bounded harness](docs/harness.md) | Tool protocol, exact approvals, budgets and recovery |
| [Architecture](ARCHITECTURE.md) | Current code and planned ownership boundaries |
| [Verification](docs/verification.md) | Checks performed, failures and outstanding work |
| [Contributing](CONTRIBUTING.md) | Setup, checks and publication policy |

The old Windows companion and runner are preserved in local archives rather than carried into the new product.
The former `new` branch is preserved locally as `archive/new-2026-10-19`.
The full README, real demo and installation story are final-phase work after product verification.

## License

MIT; see [LICENSE](LICENSE).
Third-party runtimes retain their own licenses and notices.
