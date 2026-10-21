# Native MLX adapter

Native MLX is implemented, locally qualified on Apple Silicon and published through merged PR #15 with passing hosted CI.
The implementation uses Rust throughout the product runtime.

## Decision and scope

Use a native Rust adapter with `mlx-lm` and `mlx-rs` from the same immutable [revision](https://github.com/oxiglade/mlx-rs/tree/579199b4bb5e39b35f33fc6a59cb90e2c031a6a7).
This is the Rust `mlx-lm` library in that repository, distinct from Apple's [Python `mlx-lm`](https://github.com/ml-explore/mlx-lm), which is the qualification reference.
The Rust language-model crate is currently unpublished; its git pin and build requirements are an explicit maintenance cost.
No Python interpreter, worker, HTTP inference server or model download is used by sefai.
The standard GGUF build does not enable MLX dependencies.
An MLX-only build excludes llama.cpp.

The selected decoder families are dense Llama and Qwen3 with floating or MLX affine-quantized safetensors, including indexed shards.
These are upstream-supported candidates, not a promise that every architecture variant or checkpoint works.
The initial real fixture is Qwen3-0.6B-4bit; its revision, asset sizes and hashes are in [the fixture manifest](../tests/fixtures/mlx.json).
The Llama-family fixture is [SmolLM2-135M-Instruct-8bit](../tests/fixtures/mlx-llama.json).
The tool fixture is the model producer’s [SmolLM2-1.7B-Instruct-Q8-mlx](../tests/fixtures/mlx-tool.json).
Floating weights remain upstream-supported but locally unqualified; measured support is these named affine-quantized bundles.
MoE, multimodal, GPTQ/AWQ weights, speculative decoding, adapters and checkpoint conversion are outside this adapter.
A directory with safetensors is insufficient without compatible config, tokenizer, template and quantization metadata.
GGUF files stay with the llama.cpp adapter.
The [isolated binding build patch](../vendor/mlx-sys/SEFAI-PATCH.md) disables MLX’s unused GGUF reader because its `gguf_get_key` C symbol conflicts with llama.cpp’s incompatible signature.
Only build configuration and standalone Cargo metadata differ from the retained upstream source; FFI code is unchanged.
The patch removes the native crash found while qualifying a combined build.

The [upstream API documentation at the pin](https://github.com/oxiglade/mlx-rs/blob/579199b4bb5e39b35f33fc6a59cb90e2c031a6a7/mlx-lm/src/lib.rs) reports a 3.3× decode-throughput gap relative to its Python reference on one Qwen3 benchmark.
That is upstream evidence, not a measurement of sefai; native Rust alone does not establish better speed or memory use.
The measured comparison below is specific to sefai’s workload and hardware.

## Build and selection

Apple Silicon macOS, CMake, libclang and Apple's Metal toolchain are required.
The pinned native dependency builds MLX 0.32.2 and uses Metal JIT, which still requires the Metal compiler during its build.
Apple documents installation through [Xcode components](https://developer.apple.com/documentation/Xcode/downloading-and-installing-additional-xcode-components): `xcodebuild -downloadComponent MetalToolchain`.
The standalone Command Line Tools installation on the qualification Mac does not include `xcodebuild` or `metal`.
The native dependency downloads its pinned CMake sources during an initial build; inference itself stays offline.

```sh
xcrun --find metal
export MLX_RS_METAL_PATH="$PWD/target/mlx-metal"
cargo build --locked --release --no-default-features --features mlx
target/release/sefai run --model /absolute/path/to/bundle --backend mlx 'Say hello'
target/release/sefai bench --model /absolute/path/to/bundle --backend mlx --output 128 'Say hello'
```

The generated `mlx.metallib` must remain at the compiled path; copying the executable alone is not a relocatable installation.
Asset packaging and installation are Phase 14 gates.
For both engines, use `--features mlx` with the default GGUF feature.
Select MLX explicitly with `--backend mlx` or a Markdown model profile:

```markdown
---
schema: 1
id: qwen3
path: /absolute/path/to/Qwen3-0.6B-4bit
format: mlx
runner:
  backend: mlx
  context: 2048
  batch: 128
  microbatch: 128
  output: 128
---
```

`format: mlx` selects `backend: mlx` when no conflicting backend or device index is configured.
It never changes a GGUF profile into MLX or converts a model.
MLX chooses the Apple GPU; a ggml `--device` index is rejected.
Unsupported platforms and disabled engines return an explicit error.
The same `chat`, workspace, session and opt-in harness paths select the concrete engine in the binary.
The shared request settings and statistics live in dependency-free, I/O-free `core`; native model types remain private to their adapter.
The library request default uses one thread; the CLI retains its previous available-parallelism default for GGUF.
MLX controls its own native scheduling and does not apply GGUF CPU thread or batch flags.
`microbatch` is its prefill chunk size; the shared context/output reservation still bounds the request.

## Bounds and ownership

Bundle admission requires regular assets whose canonical paths stay directly within the bundle.
Each JSON asset is capped at 64 MiB; config dimensions have explicit limits; an index has at most 65,536 entries, 128 shards and 1 TiB total weight bytes.
The native loader then validates keys, shapes, dtypes and architecture details.
Model assets must remain immutable while loaded.
As with the GGUF native loader, this is not a sandbox for hostile checkpoint files or templates.
The upstream template interpreter has no execution-fuel limit; source-size and rendered-output limits do not bound a malicious template's intermediate work.
Use trusted bundles.

Prompt text is limited to 1 MiB and 1,024 messages/sections; encoded input and context are capped at 131,072 tokens.
The model's declared context limit and reserved output are checked before decoding.
The model-native text template is used with `enable_thinking: false`; raw mode skips it and adds no special tokens.
Tool results keep their distinct role; only the harness can validate or approve proposals.
The existing explicit SmolLM2 text protocol remains the only harness parser, including when an MLX model is asked to use it.
SmolLM2-1.7B-Q8 passed the seven end-to-end tool cases; Qwen3-0.6B failed six of them with malformed or missing call markup and is not qualified for reliable tools.
The harness refused malformed proposals; no parser or approval rule was relaxed.
Native Qwen tool parsing is not implicitly enabled.

One worker owns the selected model, native arrays and KV cache on the same OS thread.
Switching model/device drops old contexts and weights before loading the next model; exiting also unloads them.
The native allocator has a 64 MiB scratch-retention target while loaded.
Its measured cached-byte counter can exceed that target; it is not a hard total-RAM cap.
The target excludes active weights and KV tensors.
Model drop and failed loading clear retained buffers; one process-wide model lease prevents competing allocator owners.
The full KV cache is bounded by context/output reservation; native growth can round capacity upward and temporarily retain old storage.
No snapshots, rotating history truncation, cache serialization or quantized KV are enabled.

The public cache lifetime is tied to its model, including cache drop.
Cache reuse requires the exact complete native cache token sequence to prefix a longer newly rendered input, with unchanged request settings and model identity.
Identical or divergent prompts rebuild rather than approximate a suffix trim the selected upstream API does not expose.
Cancellation and errors discard the cache.
Cancellation is checked between iterator steps and around loading; it cannot interrupt an in-progress native loading, shader compilation or GPU evaluation call.
Accumulated output is capped at 1 MiB because the upstream streaming decoder retains emitted text.
Streaming uses the native incremental Unicode decoder and the existing terminal sanitizer and bounded event channels.

`/stats` and benchmark stderr expose MLX active, allocator-cached and peak-active byte counters.
They are process-global native allocator observations, exclude host RSS and are not a VRAM-plus-RAM total.
Unified CPU/GPU storage must not be counted twice.
The counters do not establish the Python reference's interpreter or process footprint.

## Local measurements

The [raw records and summary](evidence/phase6/comparison.json) use Apple M5, 16 GiB unified memory, macOS 27.0.1, MLX 0.32.2 and Python `mlx-lm` 0.31.3.
Each case uses the same local weights, exact prompt IDs, greedy sampling, 128 output tokens, 128-token prefill chunks, full attention and a 64 MiB allocator-retention target.
There are three fresh-cache repetitions per process; the table uses the median of repetitions 2 and 3.
First-token duration includes prefill and first sampling; decode throughput excludes the first sampled token.
RSS and macOS process peak footprint come from `/usr/bin/time -l` for the whole process.
MLX peak-active counters are process-global; they overlap OS accounting and must not be added to RSS or called separate VRAM.

| Fixture / input tokens | Native decode tokens/s | Reference decode tokens/s | Native / reference max RSS MiB | Native / reference process peak MiB |
| --- | ---: | ---: | ---: | ---: |
| Qwen3 4-bit / 23 | 195.1 | 284.2 | 508.4 / 658.3 | 556.2 / 677.1 |
| Qwen3 4-bit / 535 | 134.5 | 243.7 | 505.0 / 658.2 | 951.3 / 881.4 |
| SmolLM2 135M 8-bit / 23 | 297.7 | 499.4 | 223.4 / 314.6 | 239.2 / 325.5 |
| SmolLM2 135M 8-bit / 567 | 254.9 | 427.8 | 222.9 / 314.1 | 381.4 / 409.4 |

Native host RSS is lower, but decode is 1.46–1.81× slower here.
The longer Qwen case has about 8% higher process peak footprint and 29% higher MLX peak-active bytes than the reference.
This is not a consistent total-memory win; longer-context KV storage and temporary allocations remain a Phase 10 profiling gate.
The selected upstream cache doubles full-cache capacity and can retain old buffers during transactional updates; the measurements do not attribute every byte of the gap to one mechanism.
All 12 outputs match after removing a leading whitespace difference; raw output bytes are not identical because the incremental detokenizers handle initial whitespace differently.
This small fixed workload on one Mac does not establish performance for other models, contexts or GPUs.

Reproduce using the pinned fixtures and the isolated reference dependencies in [reference-requirements.txt](evidence/phase6/reference-requirements.txt):

```sh
cargo build --locked --release -p sefai-engine-mlx --features runtime --example measure
/usr/bin/time -l target/release/examples/measure /path/to/bundle > native.json
HF_HUB_OFFLINE=1 TRANSFORMERS_OFFLINE=1 /path/to/reference-python scripts/measure_mlx.py /path/to/bundle native.json
```

Pass `long` after the model path for the longer prompt.
The reference script checks every prompt ID before inference; it cannot substitute a checkpoint or download missing model assets.
Sefai itself never invokes this Python reference.

## Qualification controls

Dedicated real-bundle tests require their fixture and fail if it is missing:

```sh
SEFAI_MLX_TEST_MODEL=/absolute/path/to/bundle \
  cargo test --locked -p sefai-engine-mlx --features model-tests --test mlx -- --test-threads=1
SEFAI_MLX_TEST_MODEL=/absolute/path/to/tool-bundle \
SEFAI_MCP_TEST_PYTHON=/absolute/path/to/python \
  cargo test --locked -p sefai --no-default-features --features mlx-model-tests,mcp --test harness_mlx
```

Serial adapter tests avoid conflating native process-global allocator/device state across models.
Ordinary workspace tests require no model or GPU.
Hosted macOS CI compiles both engine combinations, checks unit contracts and executes GGUF on CPU with both native libraries linked.
It does not claim MLX GPU execution; local GPU and terminal evidence is recorded in [verification.md](verification.md).
The workflow changes require maintainer inspection before publication.
