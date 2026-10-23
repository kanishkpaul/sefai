# ONNX adapter

The optional ONNX adapter generates from named decoder exports through the CLI and terminal harness on the ONNX Runtime CPU provider and, in an `onnx-webgpu` build, the WebGPU provider.
Phase 7 status and remaining limits are at the end of this guide.

## Decision and scope

Use the Rust [`ort`](https://github.com/pykeio/ort) interface pinned to `=2.0.0-rc.15`, which binds ONNX Runtime 1.31.0.
Its safe API covers session construction, execution providers, I/O binding and device output binding without workspace `unsafe` code.
[ONNX Runtime GenAI](https://onnxruntime.ai/docs/genai/) is a generation and timing reference only; it has no Rust API used here.

Two decoder conventions are admitted:

- **Optimum**, the Hugging Face Optimum/Transformers.js export: `input_ids`, `attention_mask`, optional `position_ids` and `past_key_values.N.key/value` inputs; `logits` and `present.N.key/value` outputs, where each present tensor is the past concatenated with the new tokens.
- **GenAI share-buffer**, the ONNX Runtime GenAI model-builder export: the same names without `position_ids`, a `genai_config.json` beside the graph that names it as the decoder and declares `past_present_share_buffer`, and GroupQueryAttention that writes new keys and values into the past buffer in place.

Graphs are admitted only when every declared input is fed, every output is expected, and cache layer count, key-value heads, head size, vocabulary and element types match `config.json`.
The accepted architectures are `llama` and `qwen3`.
Cache tensors are all `float32` or all `float16`; logits are `float32` or `float16` and are widened to `float32` before sampling.
Encoder-decoder graphs, multimodal graphs, GenAI exports without a shared buffer and arbitrary `.onnx` files are rejected.

A profile or `--model` path names the `.onnx` graph file so the export variant is explicit.
`config.json`, `tokenizer.json` and `tokenizer_config.json` are read beside the graph, or from the parent of an `onnx` directory, which is the Hugging Face layout.
Optimum bundles also need `generation_config.json`; GenAI bundles take end-of-generation tokens and context length from `genai_config.json`.
A chat template comes from `tokenizer_config.json` or a `chat_template.jinja` beside it.
Metadata files are top-level regular files of at most 64 MiB, and symlinks cannot escape the bundle directory.
A self-contained graph is at most 2 GiB; larger weights use ONNX external data, which ONNX Runtime resolves beside the graph.

Prompts are rendered with the export's own Jinja chat template through `minijinja`, so a template's default system message is included where GGUF's built-in template would omit it.
Sampling matches the GGUF adapter: greedy selection after a 0.1 frequency penalty over the last 64 tokens, with ties going to the lowest token ID.
Generation stops at any declared end-of-generation token, falling back to `config.json`.

## Build and selection

The `onnx` feature is off by default and never enabled by the GGUF package.
Its build downloads the prebuilt ONNX Runtime archive that `ort-sys` pins by URL and SHA-256 from `cdn.pyke.io`, and links it statically; inference itself stays offline.
That archive includes CoreML on Apple Silicon and is not published for Intel macOS.
Archive download uses native TLS: Secure Transport on macOS, SChannel on Windows and OpenSSL on Linux.
On macOS the adapter's build script also links the toolchain's `clang_rt.osx`, because release LTO drops the standard library's `__isPlatformVersionAtLeast` that ONNX Runtime's CoreML objects reference; the build therefore requires `clang`.
GGUF, MLX and ONNX link and generate together in one binary.

`onnx-webgpu` adds the WebGPU execution provider from the matching pinned archive.
WebGPU runs through Dawn, which ONNX Runtime links dynamically: the build copies the Dawn library beside the executable and adds an `@executable_path` (macOS) or `$ORIGIN` (Linux) run path, so the library must ship next to the binary.

```sh
cargo build --locked --release --features onnx-webgpu
target/release/sefai run --backend onnx --provider webgpu \
  --model /absolute/path/to/webgpu-int4-kld-block-32/model.onnx 'Say hello'
```

Select ONNX with `--backend onnx` or a Markdown model profile, and a provider with `--provider` or `runner.provider`:

```markdown
---
schema: 1
id: qwen3-webgpu
path: /absolute/path/to/webgpu-int4-kld-block-32/model.onnx
format: onnx
runner:
  provider: webgpu
---
```

`format: onnx` implies `backend: onnx` and rejects a device index; other formats reject `backend: onnx` and `provider`.
The provider defaults to `cpu`.
Loading fails if the selected provider cannot initialize; ONNX Runtime may still place small shape nodes on the CPU, which [the placement evidence](evidence/phase7/webgpu.json) records per graph.
ONNX Runtime fixes the provider and intra-op threads when the session is created, so both are part of the selected device: changing either reloads the model, and a request with a different thread count fails.
ONNX Runtime telemetry is disabled and its native log output is limited to fatal messages, which would otherwise corrupt the terminal.

## Providers by platform

An ONNX file proves no AMD, NVIDIA or Apple coverage; a provider counts only where a named export has run on it.

| Provider | Build | Qualified exports and hardware |
| --- | --- | --- |
| CPU | `onnx` on macOS arm64, Linux x86_64 and Windows x86_64 | All fixtures on Apple M5, macOS 27.0; unit and signature tests in CI on all three systems |
| WebGPU | `onnx-webgpu` | The `float32` Optimum fixtures and the WebGPU-targeted GenAI export on Apple M5 through Dawn on Metal; Linux and Windows builds are untested |
| CoreML | Present in the macOS archive, not selectable | Not qualified: [it splits these graphs into many partitions with CPU fallback](evidence/phase7/coreml.json) |
| CUDA, DirectML, ROCm and others | Not built | None |

`sefai devices` lists the ONNX providers compiled into the binary.
The CPU-targeted GenAI export loads on WebGPU but is qualified for CPU only, and the Optimum `int8` export is not qualified on WebGPU because its quantized matrix products fall back to CPU.

## Cache and memory

Cache tensors stay on the provider's device through I/O binding; each step copies only the token IDs, positions and attention mask in and the logits out.
On the Optimum layout each layer's present tensors become the next step's past inputs.
On the GenAI layout one buffer per layer and part, sized to the configured `context`, is allocated from the session's device allocator on the first step and bound as both past input and present output.
That reservation is taken in full up front: Qwen3-0.6B's `float16` cache is 112 KiB per token, so a 4,096-token context reserves 448 MiB.
A different context reservation reallocates the buffers.

A new prompt reuses the cache only when the cached tokens are a strict prefix, so at least one new token produces logits.
Cancellation, a callback error or any runtime error clears the cache.
Prefill runs in `microbatch` chunks because these exports return logits for every input position: 4,096 positions of a 49,152-token vocabulary would be 768 MiB of `float32` logits.

[Memory evidence](evidence/phase7/memory.json) samples the process footprint, including GPU memory, once per second over three 1,536-token generations in one process.
The share-buffer exports stay flat after load: about 1,020 MB on WebGPU and 1,510 MB on CPU.
The Optimum export grows by about 850 MB during the first generation, because each step allocates a new present cache beside the previous one, and then stays at that high-water mark.
Optimum exports therefore remain admitted but do not meet the per-token allocation gate; the share-buffer exports do.
Memory reporting returns `onnx memory reporting unavailable`; ONNX Runtime exposes no allocator counters through the safe API used here.

## Fixtures and verification

| Fixture | Manifest | Use |
| --- | --- | --- |
| SmolLM2-135M-Instruct `onnx/model_int8.onnx` and `onnx/model.onnx` | [onnx.json](../tests/fixtures/onnx.json) | Optimum layout, `int8` and `float32` |
| Qwen3-0.6B `onnx/model.onnx` with `model.onnx_data` | [onnx-external.json](../tests/fixtures/onnx-external.json) | External data |
| Qwen3-0.6B GenAI `cpu-int4-kld-block-128` and `webgpu-int4-kld-block-32` | [onnx-genai.json](../tests/fixtures/onnx-genai.json) | Share-buffer layout; `float32` and `float16` cache |
| SmolLM2-1.7B-Instruct `onnx/model_fp16.onnx` with `model_fp16.onnx_data` | [onnx-tool.json](../tests/fixtures/onnx-tool.json) | Tool harness; `float16` cache and logits |

Large-file hashes match the upstream LFS records, and the other files match the upstream sizes.
Real-graph tests require the generation graph and the external-data fixture, and fail when either is absent; `SEFAI_ONNX_TEST_PROVIDER=webgpu` reruns them on WebGPU in an `onnx-webgpu` build:

```sh
SEFAI_ONNX_TEST_MODEL=/absolute/path/to/webgpu-int4-kld-block-32/model.onnx \
SEFAI_ONNX_TEST_EXTERNAL_MODEL=/absolute/path/to/Qwen3-0.6B-ONNX/onnx/model.onnx \
SEFAI_ONNX_TEST_PROVIDER=webgpu \
  cargo test --locked -p sefai-engine-onnx --features model-tests,webgpu --test onnx
```

The eleven cases cover bounded chat streaming, Unicode decoding, prefix reuse matching a cold rebuild, a changed context reservation matching a cold rebuild, prompt changes discarding the cache, cancellation and callback failure clearing the cache, context reservation before streaming, a changed thread count, an incompatible graph signature and a graph whose external data is missing.
All eleven pass for SmolLM2 `int8` on CPU, SmolLM2 `float32` on WebGPU, the CPU GenAI export on CPU and the WebGPU GenAI export on both providers.

[Output parity](evidence/phase7/parity.jsonl) compares `sefai run --raw` with [the reference script](../scripts/onnx_reference.py), which runs Python `onnxruntime` 1.31.0 on the same graph, CPU provider, two threads and sampler.
The reference grows its past tensors instead of sharing buffers, so GenAI rows also check sefai's in-place cache against an independent cache path.
All 24 runs on qualified paths produce identical 64-token text on English, Python and Japanese prompts: SmolLM2 `int8` and `float32`, Qwen3-0.6B with external data and both GenAI exports, on CPU and on each export's qualified WebGPU path.
The CPU-targeted GenAI export on WebGPU matches on two prompts and diverges on the Python prompt after 14 characters, which is why it is not qualified there.
The smallest nonzero reference top-two logit margin was 0.0025, and the `float16` Japanese run contained an exact tie, so the lowest-ID rule is exercised.
Agreement holds for these exact cases; it is not a numeric tolerance bound for every graph.

[Timing](evidence/phase7/timing.json) alternates sefai with a reference runner on the same graph and provider, one generation per process, five rounds per case.
The host was not idle, so these compare runners under the same load rather than establish absolute throughput.

| Provider | Reference | Case | sefai | Reference |
| --- | --- | --- | --- | --- |
| CPU | ONNX Runtime GenAI 0.18.0 on ONNX Runtime 1.31.0 | 512-token decode | 113.3 tokens/s | 112.1 tokens/s |
| CPU | ONNX Runtime GenAI 0.18.0 | 1,001-token prefill | 451.4 tokens/s | 452.6 tokens/s |
| WebGPU | onnxruntime-node 1.30.0 | 512-token decode | 120.3 tokens/s | 94.8 tokens/s |
| WebGPU | onnxruntime-node 1.30.0 | 1,001-token prefill | 1,296.9 tokens/s | 1,220.2 tokens/s |

CPU uses the CPU-targeted GenAI export with four threads; WebGPU uses the WebGPU-targeted export.
GenAI has no macOS WebGPU build, so the WebGPU reference is a greedy loop over onnxruntime-node that keeps present tensors on the GPU but grows them instead of sharing one buffer; it bounds sefai's overhead rather than isolating it.
Two CPU differences have known causes.
A 14-token prefill is slower because the first step reserves the whole 2,048-token share buffer while GenAI sizes its buffer to the requested length; with a 528-token context the two match at 25 ms.
Decode after the 1,001-token prompt is about 8% slower in a fresh process; later generations in one process match GenAI.

The tool harness gate runs through `harness_onnx` with the same seven cases as GGUF and MLX.
Its fixture is [SmolLM2-1.7B-Instruct `onnx/model_fp16.onnx` with external data](../tests/fixtures/onnx-tool.json), because the harness uses SmolLM2's tool protocol:

```sh
SEFAI_ONNX_TEST_MODEL=/absolute/path/to/SmolLM2-1.7B-Instruct/onnx/model_fp16.onnx \
SEFAI_MCP_TEST_PYTHON="$(command -v python3)" \
  cargo test --locked -p sefai --features onnx-model-tests,mcp-oauth --test harness_onnx
```

All seven pass on the CPU provider.
The producer's `int8` and `q4` exports of the same model do not: `int8` passes three cases and `q4` none, because their proposals contain several calls, which the harness rejects before approval.
The same prompt renders to the same token count as GGUF, so this is those exports' quantization rather than prompt rendering.
Qwen3-0.6B GenAI exports propose calls but truncate their arguments, and fail all seven.

## Phase 7 status

Phase 7's exit gate is met on Apple M5 with macOS 27.0: the named export set generates on CPU and on the qualified WebGPU provider, and past-state growth, output parity, memory plateau, missing external files, incompatible graph signatures, tool use and same-graph timing are tested as described above.

These limits remain and are not inferred from it:

- WebGPU is unqualified on Linux and Windows, and other hardware needs its own evidence; CUDA, DirectML and ROCm are not built.
- The tool harness has run on the CPU provider only.
- Memory reporting through `sefai bench` is unavailable; the plateau evidence comes from the operating system's footprint.
- Optimum exports allocate a new cache per token, so long generations should use a share-buffer export.
