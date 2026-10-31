# GGUF runner

This guide covers the default GGUF backend.
The optional [native MLX adapter](mlx.md) and [ONNX adapter](onnx.md) use the same commands and terminal harness with their own model, device and cache constraints.

Phase 1 adds local GGUF inference with explicit CPU or GPU selection.
Phase 1 passed local CPU/Metal model tests, Windows/macOS/Linux CPU-model CI and CUDA/Vulkan compilation.
Phase 2 added Markdown profiles and instruction/session composition and merged after passing the required cross-platform checks.
The [terminal conversation](tui.md) adds a persistent model worker; the [bounded harness](harness.md) adds opt-in approved local memory tools; optional [MCP connections](mcp.md) add explicitly approved external tools.
See [workspace.md](workspace.md) for editable model settings, prompts, skills, memory and session resume.

## Build and run

The default build enables GGUF with CPU selected at runtime.
Install stable Rust, CMake, a C/C++ compiler and libclang for bindgen.
On macOS, install the Xcode command-line tools and CMake; on Linux use the distribution's CMake, Clang and libclang development packages; on Windows use Visual Studio C++ build tools, CMake and LLVM, with `LIBCLANG_PATH` pointing at LLVM's `bin` directory when discovery fails.

```sh
cargo run --locked -- devices
cargo run --locked -- run --model /path/to/model.gguf "hello"
cargo run --locked -- run --model /path/to/model.gguf --raw "complete this text"
cargo run --locked --release -- bench --model /path/to/model.gguf --threads 4 --output 32 "hello"
```

Explicit flags override a selected Markdown model profile, which overrides workspace config and built-in defaults.
Explicit CPU selection clears an inherited GPU device; passing CPU and an explicit device together remains an error.
`--raw` rejects nonempty workspace instructions or resumed history before loading weights.

`--format gguf|mlx|gptq|awq|onnx`, or a profile's `format`, declares the artifact's format; a profile's format applies only to its own path, not to one given with `--model` or `/model`.
A declared format without a backend selects its engine, and a format paired with another engine's backend is rejected before loading; with neither, a bare path loads as GGUF on the CPU.
Every engine admits the artifact before native allocation: GGUF needs a regular file with the GGUF magic, and a declared MLX, GPTQ or AWQ format must match the bundle's own quantization metadata.
Load failures name their category: `missing runtime` for an engine absent from the build or platform, `unsupported artifact` for a file that fails admission, and `load failed` for a native runtime error.

Pass `-` as the prompt to read at most 1 MiB of UTF-8 from stdin.
The ordinary `run` command streams text to stdout and reports locality/device/context on stderr.
It emits no interactive banner or color; terminal controls are filtered across stream chunks.
Benchmark output is a human-readable diagnostic of load, prefill and generation duration, token counts and stop reason; it is not a stable machine API.
Generation time includes sampling and the output callback; the final emitted token is not decoded again when no further logits are needed.
Sampled EOG tokens count toward generated tokens and are never printed.
Each benchmark repetition gets a fresh context and sampler while sharing one loaded model; the first repetition can include GPU pipeline warm-up.

GPU builds are explicit:

```sh
cargo run --locked --features metal -- run --model /path/to/model.gguf --backend metal --device 0 "hello"
cargo build --locked --no-default-features --features cuda
cargo build --locked --no-default-features --features vulkan
```

Use the ggml index reported by `devices`; it is not necessarily a GPU-only ordinal.
CUDA needs NVIDIA's CUDA compiler/SDK; Vulkan needs Vulkan headers/loader, `glslc` and SPIR-V headers with their CMake package configuration.
A requested backend/index that is absent fails before model loading; it never switches to another backend.
The Metal backend is registered as `MTL` by this pinned llama.cpp version.
Single-device GPU mode requests all model layers on that device, with CPU kernels still possible for unsupported operations and host-side work.
Native allocation evidence, rather than the selected-device label alone, determines actual placement.

`llama-cpp-2` and the resolved native crate are locked to 0.1.160.
Defaults are disabled; this upstream release nevertheless enables Metal unconditionally on Apple Silicon.
CPU mode passes an empty GPU device list, zero GPU layers, no KV offload and no operation offload.
Backend registration/driver initialization may still retain memory; CPU execution is not a promise of zero graphics-driver allocation.
The minimal diagnostic build, `cargo run --locked --no-default-features -- prompt hello`, has no inference or signal-handler dependency.

## Input and generation policy

Only GGUF v3 regular files up to 1 TiB are accepted.
The first decoder architecture set is `llama`, `qwen2`, `qwen3`, `gemma`, `gemma2`, `gemma3`, `phi3`, `gpt2` and `starcoder2`; only the pinned SmolLM2 `llama` fixture is currently qualified.
This allowlist is a decoder guard, not a claim that every checkpoint in those families has been tested.
Native model parsers are not sandboxed; load model files from a trusted source.

Chat mode uses the GGUF's template through upstream `llama_chat_apply_template` and fails on missing or unsupported templates.
The upstream renderer recognizes a finite set of template forms; this is not a complete Jinja/tool-template implementation.
An unsupported tool renderer is explicitly tested as an error; tool-template support remains a harness gate.
Raw mode is an explicit alternative that treats role/control-marker spellings as literal input.
Chat body text remains untrusted model input and cannot grant runtime permissions.
System sections and existing system messages merge into one leading system message, with sections first and conversation system text following in order; blank system text is omitted.
BOS follows tokenizer metadata without duplicating a BOS already emitted by the template.
Chat rendering owns turn delimiters; raw mode also follows tokenizer EOS insertion metadata.
The vocabulary's EOG predicate handles every declared stop token rather than checking one EOS ID.

Blank text and nul bytes fail; total input and rendered text are bounded to 1 MiB and 1,024 messages/sections.
The context ceiling is 131,072 tokens and never exceeds the model's training context.
`--context` defaults to 4,096, `--batch` to 512, `--microbatch` to 512 for GGUF and 128 for MLX and ONNX, and `--output` to 128; [Phase 10 measurements](footprint.md) chose the GGUF microbatch.
A batch cannot exceed context or 4,096 tokens; microbatch must be within the batch; threads must be within 1–256.
Prompt tokens plus the full reserved output must fit before context allocation or decoding, including checked integer positions.
Output zero validates the request but allocates no context and performs no decode.
Prefill chunks keep absolute monotonic positions and request logits only for the final prompt token.
Sampling is greedy with a 64-token frequency-penalty window of 0.1; the prompt seeds the sampler, and sampling accepts each generated token exactly once.

Only one model can borrow an engine at a time, and reentrant generation is rejected.
In `bench`, the model stays loaded and each sample uses a fresh context.
The interactive TUI instead retains one model-bound context and reuses only equal rendered prompt-token prefixes.
UTF-8 decoding retains at most three unfinished bytes between pieces; invalid bytes or an unfinished final character become U+FFFD.
Ctrl-C requests cancellation during loading and between prefill/decode calls.
A native call or a caller's blocking output callback cannot be interrupted midway by this API; no hard cancellation-latency guarantee is claimed.
Interrupted output is kept and the command exits unsuccessfully.

Ordinary builds suppress native logs because model metadata may contain terminal controls and native debug logging can be enormous.
For placement investigations, explicitly build with `--features sefai-engine-llama/native-logs` and redirect stderr to a file for inspection; this diagnostic stream is unsanitized.

## Regression fixture

[Fixture metadata](../tests/fixtures/gguf.json) records the publisher, immutable revision, size, SHA256 and Apache-2.0 model-card declaration.
The 105,454,432-byte Q4_K_M fixture is downloaded deliberately by CI, never by the product.
The main three-OS CI matrix verifies size/hash before executing these dedicated targets:

```sh
SEFAI_TEST_MODEL=/path/to/SmolLM2-135M-Instruct-Q4_K_M.gguf cargo test --locked -p sefai-engine-llama --features model-tests --test gguf
SEFAI_TEST_MODEL=/path/to/SmolLM2-135M-Instruct-Q4_K_M.gguf cargo test --locked -p sefai-engine-llama --features model-tests --lib gguf_
```

Both commands fail if their fixture is missing; ordinary workspace tests require no model or network.
For Metal generation tests, also set `SEFAI_TEST_BACKEND=metal` and `SEFAI_TEST_DEVICE` to its ggml index.
Tokenizer metadata overrides exercise BOS-enabled and BOS-disabled behavior with the same licensed weights.
Forced logit biases exercise termination at every fixture EOG ID independently of reply text and CPU/GPU numerical differences.
CUDA/Vulkan workflow jobs compile their respective builds without claiming GPU execution.
Current measurements, failed probes and remaining gates are in [verification.md](verification.md).

## Lifecycle contract

Every qualified format runs the same six end-to-end scenarios through the release binary:
a profile streams a Unicode prompt through the terminal worker, an overflowing prompt is rejected before streaming, a failed admission leaves the session usable for the next profile, a saved session resumes after restart, `bench` reports statistics and an engine memory line, and `run` reports the selected engine's device.
Engine-specific behavior such as cache reuse, cancellation inside native calls and tool parsing stays in each adapter's own tests.
`lifecycle_switch` additionally switches one session from GGUF to MLX to ONNX and back, unloading each model before the next loads.

[`scripts/fetch_fixture.py`](../scripts/fetch_fixture.py) downloads a pinned manifest and verifies every file's size and SHA-256:

```sh
python3 scripts/fetch_fixture.py tests/fixtures/gguf.json fixtures/smollm2-135m.gguf
python3 scripts/fetch_fixture.py tests/fixtures/mlx-llama.json fixtures/mlx-llama
python3 scripts/fetch_fixture.py tests/fixtures/gptq.json fixtures/gptq
python3 scripts/fetch_fixture.py tests/fixtures/awq.json fixtures/awq
python3 scripts/fetch_fixture.py tests/fixtures/onnx.json fixtures/onnx
SEFAI_LIFECYCLE_GGUF=$PWD/fixtures/smollm2-135m.gguf cargo test --locked --release -p sefai --features harness-model-tests --test lifecycle_gguf
SEFAI_LIFECYCLE_MLX=$PWD/fixtures/mlx-llama cargo test --locked --release -p sefai --features mlx-model-tests --test lifecycle_mlx
SEFAI_LIFECYCLE_GPTQ=$PWD/fixtures/gptq cargo test --locked --release -p sefai --features mlx-model-tests --test lifecycle_gptq
SEFAI_LIFECYCLE_AWQ=$PWD/fixtures/awq cargo test --locked --release -p sefai --features mlx-model-tests --test lifecycle_awq
SEFAI_LIFECYCLE_ONNX=$PWD/fixtures/onnx/onnx/model_int8.onnx cargo test --locked --release -p sefai --features onnx-model-tests --test lifecycle_onnx
```

`lifecycle_switch` needs the GGUF, MLX and ONNX variables together and `--features harness-model-tests,mlx-model-tests,onnx-model-tests`.
Each target fails when its fixture variable is unset or the path is missing.

## Dependency reasons

`llama-cpp-2` supplies the safe native inference API, tokenizer, template renderer and sampler; implementing those in the standard library would duplicate llama.cpp.
`ctrlc` supplies portable safe signal registration; the standard library has no equivalent API and workspace code forbids unsafe signal FFI.
It is optional with GGUF and is absent from the minimal diagnostic build.
No additional async runtime, template engine, UTF-8 decoder crate or error wrapper dependency was introduced.
The exact binding pin is isolated in dependency commit `8753694`; its dependency-only patch is also preserved locally.
