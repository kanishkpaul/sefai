# ONNX adapter

Phase 7 is in progress on the `phase7-onnx` branch.
A CPU adapter generates from a named export through the CLI and terminal harness; the accelerated-provider and measurement gates remain open.

## Decision and scope

Use the Rust [`ort`](https://github.com/pykeio/ort) interface pinned to `=2.0.0-rc.15`, which binds ONNX Runtime 1.31.0.
Its safe API covers session construction, execution providers, I/O binding and device output binding without workspace `unsafe` code.
[ONNX Runtime GenAI](https://onnxruntime.ai/docs/genai/) remains a generation reference only; it has no Rust API used here.

The admitted convention is the Hugging Face Optimum/Transformers.js decoder export:
`input_ids`, `attention_mask`, optional `position_ids` and `past_key_values.N.key/value` inputs; `logits` and `present.N.key/value` outputs.
Graphs are admitted only when every declared input is fed, every output is expected, and cache layer count, key-value heads, head size, vocabulary and element types match `config.json`.
The accepted architectures are `llama` and `qwen3` with `float32` cache tensors.
Half-precision cache exports, GroupQueryAttention share-buffer exports from the GenAI model builder, encoder-decoder graphs, multimodal graphs and arbitrary `.onnx` files are rejected.

A profile or `--model` path names the `.onnx` graph file so the export variant is explicit.
`config.json`, `generation_config.json`, `tokenizer.json` and `tokenizer_config.json` are read beside the graph, or from the parent of an `onnx` directory, which is the Hugging Face layout.
Metadata files are top-level regular files of at most 64 MiB, and symlinks cannot escape the bundle directory.
A self-contained graph is at most 2 GiB; larger weights use ONNX external data, which ONNX Runtime resolves beside the graph.

Prompts are rendered with the export's own Jinja chat template through `minijinja`, so a template's default system message is included where GGUF's built-in template would omit it.
Sampling matches the GGUF adapter: greedy selection after a 0.1 frequency penalty over the last 64 tokens.
Generation stops at any `eos_token_id` from `generation_config.json`, falling back to `config.json`.

## Build and selection

The `onnx` feature is off by default and never enabled by the GGUF package.
Its build downloads the prebuilt ONNX Runtime archive that `ort-sys` pins by URL and SHA-256 from `cdn.pyke.io`, and links it statically; inference itself stays offline.
That archive includes CoreML on Apple Silicon and is not published for Intel macOS.
Archive download uses native TLS: Secure Transport on macOS, SChannel on Windows and OpenSSL on Linux.
On macOS the adapter's build script also links the toolchain's `clang_rt.osx`, because release LTO drops the standard library's `__isPlatformVersionAtLeast` that ONNX Runtime's CoreML objects reference; the build therefore requires `clang`.
GGUF, MLX and ONNX link and generate together in one binary.

```sh
cargo build --locked --release --features onnx
target/release/sefai run --model /absolute/path/to/bundle/onnx/model_int8.onnx --backend onnx 'Say hello'
```

Select ONNX with `--backend onnx` or a Markdown model profile:

```markdown
---
schema: 1
id: smollm2-onnx
path: /absolute/path/to/SmolLM2-135M-Instruct/onnx/model_int8.onnx
format: onnx
---
```

`format: onnx` implies `backend: onnx` and rejects a device index; other formats cannot select `backend: onnx`.
ONNX Runtime fixes intra-op threads when the session is created, so `threads` is part of the selected device: changing it reloads the model, and a request with a different count fails.
ONNX Runtime telemetry is disabled and its native log output is limited to fatal messages, which would otherwise corrupt the terminal.

## Cache and memory

Each layer's present tensors become the next step's past inputs without a host copy on the CPU provider.
A new prompt reuses the cache only when the cached tokens are a strict prefix, so at least one new token produces logits.
Cancellation, a callback error or any runtime error clears the cache.
Prefill runs in `microbatch` chunks because these exports return logits for every input position: 4,096 positions of a 49,152-token vocabulary would be 768 MiB of `float32` logits.

The Optimum export concatenates each step's present tensors, so ONNX Runtime allocates a new full cache per token while the previous one is released.
The Phase 7 gate forbids that, so it is not closed by this convention.
Closing it requires either a share-buffer graph with bound, preallocated cache outputs or measured evidence that the arena reuses the released block.
Memory reporting returns `onnx memory reporting unavailable`; ONNX Runtime exposes no allocator counters through the safe API used here.

## Fixtures and verification

The generation fixture is [SmolLM2-135M-Instruct `onnx/model_int8.onnx`](../tests/fixtures/onnx.json): 30 layers, 3 key-value heads, head size 64 and an `int8` weight export with a `float32` cache.
The external-data fixture is [Qwen3-0.6B `onnx/model.onnx` with `model.onnx_data`](../tests/fixtures/onnx-external.json).
Their hashes match the upstream LFS records.
Real-graph tests require both fixtures and fail when either is absent:

```sh
SEFAI_ONNX_TEST_MODEL=/absolute/path/to/SmolLM2-135M-Instruct/onnx/model_int8.onnx \
SEFAI_ONNX_TEST_EXTERNAL_MODEL=/absolute/path/to/Qwen3-0.6B-ONNX/onnx/model.onnx \
  cargo test --locked -p sefai-engine-onnx --features model-tests --test onnx
```

The ten cases cover bounded chat streaming, Unicode decoding, prefix reuse matching a cold rebuild, prompt changes discarding the cache, cancellation and callback failure clearing the cache, context reservation before streaming, a changed thread count, an incompatible graph signature and a graph whose external data is missing.

[Output parity](evidence/phase7/parity.jsonl) compares `sefai run --raw` with [the reference script](../scripts/onnx_reference.py), which runs Python `onnxruntime` 1.31.0 on the same graph, CPU provider, two threads and sampler.
All nine runs produce identical 64-token text: SmolLM2-135M `int8` and `float32` and Qwen3-0.6B with external data, each on English, Python and Japanese prompts.
The smallest reference top-two logit margin was 0.0025, so the agreement is not explained by wide margins alone.
Agreement holds for these exact cases; it is not a numeric tolerance bound for every graph.

The tool harness gate runs through `harness_onnx` with the same seven cases as GGUF and MLX:

```sh
SEFAI_ONNX_TEST_MODEL=/absolute/path/to/SmolLM2-1.7B-Instruct/onnx/model_int8.onnx \
SEFAI_MCP_TEST_PYTHON="$(command -v python3)" \
  cargo test --locked -p sefai --features onnx-model-tests,mcp-oauth --test harness_onnx
```

It does not pass with the producer's SmolLM2-1.7B-Instruct exports at revision `31b70e2e869a7173562077fd711b654946d38674`.
The `int8` export passes the three MCP denial, exact-argument and disconnect cases and fails the four others; the `q4` export fails all seven.
Each failing case stops at its first expected outcome: no valid proposal reached approval, and the reproduced cases proposed several calls, which the harness rejects before approval, so nothing was dispatched.
The same prompt renders to the same token count as GGUF, and the Q4_K_M GGUF fixture proposes one call, so this is the exported weights' behaviour rather than prompt rendering.
A tool-capable ONNX fixture remains open.

## Remaining Phase 7 gate

- One qualified accelerated provider; CUDA requires NVIDIA hardware.
  [CoreML evidence](evidence/phase7/coreml.json) shows it is not qualified for these exports: it takes 308 of 459 `float32` nodes in 92 partitions and 727 of 1,212 `int8` nodes in 121 partitions, with CPU fallback between them.
- Device-resident cache tensors through I/O binding for that provider, with host-copy and arena-retention inspection.
- Per-token cache allocation: a share-buffer export or measured arena reuse.
- Decode/prefill comparison against ONNX Runtime with the same graph and provider settings; timing was deferred because the qualification host was not idle.
- Memory plateau across a long generation.
- A tool-capable fixture that passes `harness_onnx`.
- Provider reporting by OS and hardware; an ONNX file proves no AMD, NVIDIA or Apple coverage.
