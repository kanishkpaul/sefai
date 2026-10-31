# Footprint and platform qualification

Phase 10 measures what sefai costs to run and labels every platform route by the evidence behind it.
Both idle targets are met everywhere measured: an idle GGUF terminal session uses at most 8.5 MiB resident on the Apple M5 against the 40 MiB target, and the MCP build 9.0 MiB against 60 MiB.
GGUF now defaults to 512-token microbatches, which raised Metal prefill 47% for 5 MiB, and MLX evaluates each sampling step once, which raised small-model decode 12%.
CUDA and Vulkan were compiled, never run; they stay follow-up gates.

## Scope

The maintainer's only hardware is a MacBook Air with an Apple M5 and 16 GB of unified memory, so this phase qualifies:

- macOS arm64 fully on that Mac: CPU and Metal GGUF, MLX, GPTQ and AWQ on MLX, and ONNX on the CPU provider;
- CPU on hosted GitHub runners for Linux x86_64 and arm64, macOS arm64 and x86_64, and Windows x86_64;
- CUDA and Vulkan as compiled-only routes, because no NVIDIA or AMD hardware was used.

The build plan's original exit gate also asks for CUDA on NVIDIA and Vulkan on AMD; those rows stay open as follow-up gates for contributors with that hardware rather than being inferred from compilation.

## Method

[`scripts/measure_footprint.py`](../scripts/measure_footprint.py) reads memory from the operating system, never from sefai:

| System | Idle | Bench peak |
| --- | --- | --- |
| macOS | `ps` resident size and `footprint` physical footprint | `/usr/bin/time -l` peak physical footprint and maximum resident size |
| Linux | `/proc` `VmRSS` and `VmHWM` | `getrusage` maximum resident size of the child |
| Windows | `GetProcessMemoryInfo` working set and private bytes | Peak working set sampled every 50 ms |

macOS physical footprint includes Metal and MLX unified-memory allocations; resident size does not include all of them, so the two must not be added.
`idle` starts `chat --plain` in an empty workspace, waits for the ready line, settles for 2 s and samples; no model is loaded.
`bench` runs `sefai bench` with a 512-token prompt (` a` repeated), 64 output tokens, context 4,096 and three repetitions in one process.
Local runs alternate binaries with an 8 s pause and record the thermal state before each run; the MacBook Air has no fan, so long runs throttle.

## Apple M5 results

MacBook Air, Apple M5 with 4 performance and 6 efficiency cores, 16 GB unified memory, macOS 27.0.1, Xcode 27.2, Rust 1.96.1.
No thermal or performance warning was recorded before any run.
Raw records, with fixture paths and host details, are in [evidence/phase10/local.json](evidence/phase10/local.json).

### Idle and size

| Build | Features | Binary bytes | Idle resident MiB | Idle physical footprint MiB |
| --- | --- | --- | --- | --- |
| minimal | `--no-default-features` | 1,928,736 | no terminal | no terminal |
| GGUF | default | 11,811,392 | 8.5 | 2.4 |
| GGUF with Metal | `metal` | 11,811,424 | 8.5 | 2.5 |
| GGUF with MCP | `mcp-oauth` | 18,348,080 | 9.0 | 2.7 |
| MLX only | `--no-default-features --features mlx` | 22,280,864 | not measured | not measured |
| GGUF and MLX | `mlx` | 31,241,200 | 9.8 | 3.0 |
| ONNX only | `--no-default-features --features onnx` | 25,485,424 | 8.8 | 3.0 |
| ONNX with WebGPU | `--no-default-features --features onnx-webgpu` | 28,985,120 | 10.3 | 3.3 |
| everything | `metal,mlx,onnx-webgpu,mcp-oauth` | 60,586,288 | 12.5 | 4.0 |

Idle means no model is loaded; engines initialize on the first load.
The MLX builds also need the 1,473,912-byte `mlx.metallib` at their compiled `MLX_RS_METAL_PATH`, and WebGPU builds need the 9,144,808-byte `libwebgpu_dawn.dylib` beside the executable; a copied WebGPU binary without it aborts at launch, which Phase 14 packaging must handle.

### Loaded models

Each row is `bench --raw` with a 512-token prompt, 64 output tokens and context 4,096, averaged over the second and third repetitions in one process.
Weights are file bytes; the KV column is the full-context cache from the attention shape, which GGUF allocates up front and MLX grows as tokens arrive; overhead is peak footprint minus both.

| Engine and fixture | Prefill tok/s | Decode tok/s | Peak footprint MiB | Weights MiB | KV MiB | Overhead MiB |
| --- | --- | --- | --- | --- | --- | --- |
| GGUF CPU, 4 threads, SmolLM2-135M Q4_K_M | 1,020 | 358 | 226 | 101 | 90 | 35 |
| GGUF Metal, SmolLM2-135M Q4_K_M | 12,601 | 343 | 185 | 101 | 90 | below zero |
| MLX, SmolLM2-135M 8-bit | 8,583 | 247 | 356 | 136 | 90 | 129 |
| GPTQ on MLX, Qwen3-0.6B Int8 | 2,541 | 84 | 1,258 | 728 | 448 | 82 |
| AWQ on MLX, Qwen3-0.6B | 2,583 | 103 | 1,050 | 515 | 448 | 87 |
| ONNX CPU, 4 threads, SmolLM2-135M int8 | 2,216 | 91 | 599 | 131 | 180 | 288 |

Metal's footprint sits below weights plus cache most likely because Metal wraps the memory-mapped GGUF file without copying and macOS physical footprint excludes clean file-backed pages, whereas the CPU path repacks weights into anonymous memory.
ONNX's overhead is the Optimum export allocating a new cache per token, recorded in Phase 7; share-buffer GenAI exports avoid it.
These overheads seed `sefai setup` estimates as per-engine constants rounded up; they come from small models on one machine and are labeled as estimates.

### Microbatch, context and threads

| Microbatch | GGUF CPU prefill | GGUF Metal prefill | MLX prefill | GGUF Metal peak MiB | MLX peak MiB |
| --- | --- | --- | --- | --- | --- |
| 64 | 926 | 7,997 | 5,396 | 184 | 350 |
| 128 | 982 | 12,878 | 8,452 | 183 | 354 |
| 256 | 1,044 | 16,562 | 9,890 | 184 | 434 |
| 512 | 1,050 | 18,897 | 14,051 | 189 | 461 |

GGUF now defaults to 512-token microbatches, matching llama.cpp's own default; Metal prefill rises 47% and CPU prefill 7% for at most 9 MiB.
MLX and ONNX keep 128 because their prefill logits scale with vocabulary times microbatch: 512 positions of Qwen3's 151,936-token vocabulary would be 297 MiB of `float32` logits.

GGUF CPU at context 2,048, 4,096 and 8,192 peaks at 178, 226 and 313 MiB, tracking the 45, 90 and 180 MiB cache; MLX stays near 352 MiB because it grows its cache with the conversation.
Context 16,384 is rejected before allocation by both engines because SmolLM2's trained limit is 8,192.

| GGUF CPU threads | 2 | 4 | 6 | 8 | 10 |
| --- | --- | --- | --- | --- | --- |
| Prefill tok/s | 641 | 1,024 | 1,264 | 1,470 | 1,535 |
| Decode tok/s | 272 | 362 | 366 | 351 | 340 |

All ten cores give the best prefill and decode within 7% of the best, so the default stays every available core.
The first default-thread run measured 855 and 246 tokens/s while background App Store work was still settling; later runs did not reproduce it, which is why every comparison here alternates binaries or repeats under the same conditions.
ONNX CPU shows the same shape: 1,783, 2,431 and 2,948 prefill tokens/s at 2, 4 and 10 threads with decode between 82 and 94.

### MLX single evaluation

The vendored sampler forced a GPU synchronization for each checked logits row; evaluating the checks with the sampled token, alternated with the old binary over three rounds at 128 output tokens, gave:

| Fixture | Decode before | Decode after |
| --- | --- | --- |
| SmolLM2-135M 8-bit | 245.8 | 274.5 |
| Qwen3-0.6B GPTQ Int8 | 87.8 | 88.7 |
| Qwen3-0.6B AWQ | 104.6 | 106.2 |

Greedy output is byte-identical; the remaining gap to Python `mlx-lm` is plausibly its asynchronous pipelining of the next forward pass, which is not attempted.

## Hosted CPU results

The [footprint workflow](../.github/workflows/footprint.yml) builds release binaries on each GitHub-hosted runner, measures idle chat and a raw 512-token GGUF CPU benchmark with all available threads, runs the GGUF lifecycle contract and uploads the JSON records, summarized in [evidence/phase10/hosted.json](evidence/phase10/hosted.json).
Hosted runners are shared virtual machines, so their throughput is a smoke check rather than a benchmark.

| Runner | CPU | Minimal bytes | GGUF bytes | Idle GGUF MiB | Idle MCP MiB | Prefill tok/s | Decode tok/s | Peak MiB |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `ubuntu-latest` | AMD EPYC 9V74 80-Core Processor, 4 threads | 2,345,600 | 8,160,288 | 7.1 | 8.3 | 421 | 114 | 235 |
| `ubuntu-24.04-arm` | aarch64, 4 threads | 1,905,568 | 6,890,424 | 5.9 | 7.0 | 260 | 86 | 223 |
| `macos-latest` | Apple M1 virtual, 3 threads | 1,949,248 | 11,767,424 | 9.9 | 10.5 | 415 | 169 | 195 |
| `macos-15-intel` | Intel(R) Core(TM) i7-8700B CPU @ 3.20GHz, 4 threads | 2,162,904 | 12,891,256 | 3.6 | 3.8 | 523 | 106 | 150 |
| `windows-latest` | AMD64 Family 25 Model 1 Stepping 1, AuthenticAMD, 4 threads | 2,463,744 | 7,528,448 | 7.4 | 8.7 | 336 | 98 | 233 |

Linux idle is resident size, Windows is working set, and macOS is resident size; each bench peak is that platform's measure from the method table.
Every row met both idle targets and passed `lifecycle_gguf`.

## Platform matrix

The labeled matrix lives in [stack.md](stack.md#hardware-targets).
Phase 10's remaining gates are execution evidence that this hardware cannot produce: CUDA on NVIDIA, Vulkan on AMD, GPTQ and AWQ CUDA kernels, ONNX GPU providers other than WebGPU on Apple Silicon, physical Windows and Linux terminals, and multi-GPU or integrated-GPU selection.
Each stays labeled compiled or unavailable until a contributor with that hardware records it.
