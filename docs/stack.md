# Stack and compatibility

Rust owns the runner/harness, file semantics, permissions and terminal UI.
Inference libraries are isolated optional dependencies.
The GGUF runner is locally tested on Apple M5 CPU/Metal with one pinned SmolLM2 fixture.
Other runtime choices and the wider platform/model matrix still require qualification; see [runner.md](runner.md).

| Concern | Choice | Boundary |
| --- | --- | --- |
| Product | Rust, edition 2024, stable toolchain | No interpreter required by the base GGUF package |
| Shared vocabulary | Dependency-free `sefai-core` | No I/O, UI, native backend or async dependencies |
| GGUF | Embedded llama.cpp via exactly pinned `llama-cpp-2` | Only `engine-llama` touches llama.cpp; defaults off; API probe before version selection |
| MLX | Optional Apple adapter, evaluate `mlx-rs` and model code | A tensor binding does not implement every architecture; explicit worker alternative if needed |
| ONNX | Optional ONNX Runtime via `ort` | Model/export cache contract and provider compatibility tested separately |
| GPTQ/AWQ | In-memory repack onto MLX affine groups on Apple Silicon; NVIDIA runtime still to be selected | Only layouts that map exactly are admitted; actual checkpoint/kernel support required per GPU |
| TUI | Ratatui/Crossterm and `design/rust/theme.rs` | Event-driven, bounded scrollback and terminal restoration |
| Settings/data | Markdown with strict typed YAML front matter | YAML parser and Markdown parser introduced only when needed; no arbitrary code execution |
| Memory | Markdown files, bounded text recall | No mandatory database/vector index or embedding model |
| MCP | Official Rust SDK `rmcp` | Minimal client/transport features; subprocess and remote traffic explicitly visible |
| Concurrency | Single engine owner and bounded channels | Async only where transports/timers need it; cancellation owned by caller |
| Errors | Standard typed errors initially; `thiserror`/`anyhow` only for demonstrated need | No dependency required just to wrap one error |

## Hardware targets

| OS / hardware | GGUF baseline | Other planned routes | Release evidence required |
| --- | --- | --- | --- |
| macOS arm64 / Apple Silicon | Metal, CPU | MLX, GPTQ/AWQ on MLX; ONNX CPU and qualified providers | Actual Metal and MLX runs |
| macOS x86_64 / Intel | CPU | ONNX CPU where runtime build supports it | Separate artifact/test; no Apple-GPU promise |
| Windows x86_64 / NVIDIA | CUDA, CPU | Qualified ONNX and GPTQ/AWQ runtime | Windows GPU run and clean install |
| Linux x86_64 / NVIDIA | CUDA, CPU | Qualified ONNX and GPTQ/AWQ runtime | Linux GPU run and clean install |
| Windows x86_64 / AMD | Vulkan, CPU | ONNX provider only if qualified | Actual AMD Vulkan run |
| Linux x86_64 / AMD | Vulkan, CPU | Optional ROCm and qualified ONNX provider | Actual AMD Vulkan run; ROCm separately labeled |
| Linux arm64 | CPU where dependencies build | Additional acceleration only with evidence | Separate qualification; not inferred from x86 CI |

No supported-format row implies every architecture, quantization or execution provider works everywhere.
GPU vendors' kernels and drivers constrain individual combinations.
One binary per platform/backend is the initial packaging strategy; dynamic backend loading is considered only after dependency discovery and clean-install tests justify it.
No silently hosted inference and no mandatory local HTTP server.

## Memory targets

A base GGUF harness aims for at most 40 MiB of idle overhead; the TUI with idle MCP client aims for 60 MiB excluding the backend and child servers.
Report total process-tree RAM/VRAM as well as the breakdown, so exclusions never hide a large worker.
These are targets, not results.
The measurement method and arithmetic are in [the research note](research/2026-10-19-inference-and-engineering.md).
Treat installed size, cold-load peak, steady-state memory, quality and latency as separate measurements.
