# sefai

A small Rust LLM runner and terminal harness, built around files you can read and edit.
The goal is low RAM/VRAM use, a persistent TUI, Markdown-owned prompts/skills/preferences/memory, and explicit MCP connections.

## 🏗️ Under construction

sefai is in active development and is not ready for a stable release.
Phases 1–11 are implemented: the GGUF runner, Markdown workspace, terminal TUI, bounded harness, MCP client, optional native MLX and ONNX adapters, GPTQ/AWQ on Apple Silicon, one runtime lifecycle across those formats, measured footprint on Apple Silicon and hosted CPU runners, and hardware-aware model setup.
Safety hardening is recorded in the [safety checklist](docs/safety.md); full testing, packaging and execution on NVIDIA and AMD GPUs remain planned.
Follow the [build plan](docs/build-plan.md) for scope and acceptance gates.

Pinned-model CPU tests pass on Windows, macOS and Linux; local Metal tests and CUDA/Vulkan compilation also pass.
An idle terminal session uses 4–13 MiB across the measured platforms and builds, against 40 and 60 MiB targets; see [footprint](docs/footprint.md).
GPU execution coverage depends on the backend and named model; compilation does not establish CUDA/Vulkan runtime support.
The complete hardware matrix and full-product testing remain open gates; see [verification](docs/verification.md) for evidence and limits.
There is no published installer yet; use the source-build instructions below.

Models run locally, but explicitly enabled [MCP connections](docs/mcp.md) can launch third-party processes or send data to remote services.
Tool calls require exact per-call approval; third-party servers are not automatically sandboxed.
Keep credentials out of Markdown and review the documented limits before enabling connections.

## Direction

| Area | Planned support |
| --- | --- |
| Model runtimes | GGUF, optional native MLX with GPTQ and AWQ, and optional ONNX on CPU and WebGPU; GPTQ and AWQ on NVIDIA planned |
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

The default runner loads the chosen local GGUF and streams a reply; CPU is the explicit default.
The optional [native MLX adapter](docs/mlx.md) runs named Apple Silicon safetensors bundles through the same terminal harness.
On Apple Silicon the MLX build also runs [GPTQ and AWQ checkpoints](docs/gptq-awq.md) whose packing maps onto MLX affine groups.
The optional [ONNX adapter](docs/onnx.md) runs named decoder exports on the ONNX Runtime CPU provider, or WebGPU in an `onnx-webgpu` build, through the same harness.
See the [runner guide](docs/runner.md) for GPU builds, limits, benchmarks and model tests.
`cargo run --locked --no-default-features -- prompt hello` retains the diagnostic without native inference dependencies.

## Customize with Markdown

Initialize a workspace, edit its `system.md` or `preferences.md`, then inspect the composed prompt:

```sh
cargo run --locked -- --workspace /path/to/workspace workspace init
cargo run --locked -- --workspace /path/to/workspace prompt hello
```

Model profiles, explicitly selected skills, literal memory recall and session resume use the same workspace.
`sefai setup recommend` shortlists pinned models that fit this machine, and [model setup](docs/setup.md) covers checking, downloading and importing them.
The [workspace guide](docs/workspace.md) documents schema 1, precedence, limits, safe writes and session framing.
The [Phase 2 closeout](docs/phase2-closeout.md) records the merged workspace foundation.
The [terminal guide](docs/tui.md) documents the interactive controls, limits and plain mode.

## Read the plan

| Document | Purpose |
| --- | --- |
| [Build plan](docs/build-plan.md) | Phases, retained scope and acceptance gates, including Phase 12 safety hardening |
| [Runtime and setup plan](docs/setup-plan.md) | Shared backend workflow, hardware-aware model selection and acquisition controls |
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
