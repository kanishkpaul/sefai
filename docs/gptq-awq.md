# GPTQ and AWQ on MLX

GPTQ and AWQ checkpoints run on Apple Silicon through the native [MLX adapter](mlx.md).
Their packed layers are repacked into MLX affine groups in memory when the model loads; no file is converted or written, and MLX's own quantized kernels run the result.
Phase 8 status and the open NVIDIA gate are at the end of this guide.

## Decision and scope

Phase 8 started Apple-first because the qualification host is a Mac.
A compatibility spike showed that the common GPTQ and AWQ layouts map exactly onto MLX's affine quantization, which sefai already runs natively:
both formats dequantize each weight as `scale × (q − zero)`, MLX computes `scale × q + bias`, so each group's bias is `−scale × zero` and the packed integers are reused unchanged.
That keeps the route native Rust with no worker, Python runtime or new kernel; the cost is a [vendored `mlx-lm` patch](../vendor/mlx-lm/SEFAI-PATCH.md) that lets the loader accept arrays built in memory alongside the checkpoint's own tensors.

A checkpoint is admitted only when its `quantization_config` maps exactly:

| Method | Admitted | Rejected with an error |
| --- | --- | --- |
| GPTQ | 4 or 8 bits; group size 32, 64 or 128; `checkpoint_format` `gptq` (zero points stored minus one) or `gptq_v2`; int32 packing | Act-order (`desc_act` or a reordering `g_idx`), other bit widths, per-channel or other group sizes, Marlin and other kernel formats, quantized `lm_head` |
| AWQ | AutoAWQ GEMM; 4 bits; group size 32, 64 or 128; zero points | GEMV, ExLlama, Marlin and other versions; symmetric checkpoints without zero points |

A bundle that also declares MLX `quantization` with a GPTQ or AWQ method is ambiguous and rejected.
The architectures are those of the MLX adapter: dense Llama and Qwen3.
The checkpoint's own `quantization_config` chooses the loader; a profile's `format: gptq` or `format: awq` selects the MLX backend but is not checked against that metadata.

Scales and biases are stored in the checkpoint's activation dtype, read from `model.embed_tokens.weight`.
MLX promotes a quantized product whose scales differ from the activations to `float32`; on bf16 Qwen3-0.6B AWQ that cost 3× decode throughput.
For bf16 models this rounds each `float16` scale to bf16, as MLX's own quantized models store them; [parity](#verification) is measured with that rounding.
Some exporters store an all-zero bias for bias-free layers; it is removed, which is exact, and any other bias is left for strict loading to accept only where the architecture has one.

## Build and selection

The `mlx` build includes GPTQ and AWQ; see [the MLX guide](mlx.md) for its Metal toolchain requirements.

```sh
export MLX_RS_METAL_PATH="$PWD/target/mlx-metal"
cargo build --locked --release --no-default-features --features mlx
target/release/sefai run --backend mlx --model /absolute/path/to/Qwen3-0.6B-GPTQ-Int8 'Say hello'
```

```markdown
---
schema: 1
id: smollm2-awq
path: /absolute/path/to/SmolLM2-1.7B-Instruct-AWQ-Int4
format: awq
---
```

`format: gptq` and `format: awq` imply `backend: mlx`; without an MLX build they fail with the engine-not-enabled error, and no other runtime is substituted.
Bundle layout, asset bounds, context limits, cancellation and memory reporting are the MLX adapter's.

Loading reads each packed layer's tensors directly from the safetensors files and builds its MLX replacements, so peak load memory is the repacked weights plus one source layer.
The repack added 0.3–1.9 s to load on these fixtures: 0.48 s for Qwen3-0.6B GPTQ Int8 against 0.18 s for the same weights in MLX layout, and 2.0 s against 0.13 s for SmolLM2-1.7B AWQ.

## Fixtures

| Fixture | Manifest | Use |
| --- | --- | --- |
| Qwen3-0.6B-GPTQ-Int8 from Qwen | [gptq.json](../tests/fixtures/gptq.json) | GPTQ, 8 bits, Qwen3 |
| Qwen3-0.6B-AWQ | [awq.json](../tests/fixtures/awq.json) | AWQ, 4 bits, Qwen3; GPL-3.0 weights |
| SmolLM2-1.7B-Instruct auto-round GPTQ Int4 | [gptq-tool.json](../tests/fixtures/gptq-tool.json) | GPTQ, 4 bits, Llama; tool harness |
| SmolLM2-1.7B-Instruct auto-round AWQ Int4 | [awq-tool.json](../tests/fixtures/awq-tool.json) | AWQ, 4 bits, Llama; tool harness |

Large-file hashes match the upstream LFS records and the other files match the upstream sizes.
JunHowie's Qwen3-0.6B-GPTQ-Int4 at revision `b9d87006067b0c0c2dea836370d6288e14f112ab` is used in the parity checks; its chat template is only in `chat_template.jinja`, which the tokenizer reads when `tokenizer_config.json` declares none, and all eleven real-bundle cases pass on it.

## Verification

Unit tests repack synthetic GPTQ (4 and 8 bits, both zero-point formats) and AWQ layers and check every dequantized weight, and cover the admission matrix above.
The eleven [real MLX bundle cases](mlx.md) pass on all four fixtures:

```sh
SEFAI_MLX_TEST_MODEL=/absolute/path/to/SmolLM2-1.7B-Instruct-GPTQ-Int4 \
  cargo test --locked --release -p sefai-engine-mlx --features model-tests --test mlx -- --test-threads=1
```

The seven tool harness cases pass through a `format: gptq` profile and a `format: awq` profile:

```sh
SEFAI_GPTQ_TEST_MODEL=/absolute/path/to/SmolLM2-1.7B-Instruct-GPTQ-Int4 \
SEFAI_AWQ_TEST_MODEL=/absolute/path/to/SmolLM2-1.7B-Instruct-AWQ-Int4 \
SEFAI_MCP_TEST_PYTHON="$(command -v python3)" \
  cargo test --locked --release -p sefai --no-default-features --features mlx-model-tests,mcp-oauth \
  --test harness_gptq --test harness_awq
```

Correctness is checked in three links, each against an independent implementation:

- [Layers](evidence/phase8/layers.json): applying each reference layer from Transformers with GPTQModel to an identity matrix reproduces [the Python repack's](../scripts/quantized_repack.py) dequantized weights exactly for GPTQ Int8, and within float16 kernel rounding for GPTQ and AWQ Int4: mean absolute difference about 3×10⁻⁵ against weights up to 1.6.
- [Parity](evidence/phase8/parity.json): teacher-forced over each reference's 64-token greedy continuation of three prompts, [the MLX-layout repack](../scripts/quantized_parity.py) chooses the float32 reference's next token at 764 of 768 positions across four checkpoints, and every disagreement is at a reference top-two margin of at most 0.024. GPTQModel cannot load the auto-round SmolLM2 GPTQ export, so that checkpoint is compared with the producer's 8-bit MLX model instead: 78–92% agreement, matching the AWQ export's 81–92%, and 0% with the zero-point convention deliberately flipped.
- [Loader](evidence/phase8/loader.json): sefai's native in-memory repack produces byte-identical 64-token output to the Python repack's bundle in all 15 runs, five checkpoints by three prompts.

Free-running text can still diverge from the reference after several tokens, because small numeric differences change near-tied choices.

[The comparison](evidence/phase8/comparison.json) runs the same workload as Phase 6: identical prompt IDs, greedy sampling, 128 output tokens, 128-token prefill chunks and three fresh-cache repetitions, reporting the median of repetitions 2 and 3.
The selected runtime is Python `mlx-lm` 0.31.3 on MLX 0.32.2 with the same weights in MLX layout, so the comparison isolates sefai's adapter; the host was not idle, so runners alternate per fixture.

| Fixture / input tokens | sefai on checkpoint, tokens/s | sefai on MLX copy | Python mlx-lm | sefai / Python max RSS MiB | Process peak MiB: checkpoint / copy / Python |
| --- | ---: | ---: | ---: | ---: | ---: |
| Qwen3-0.6B GPTQ Int8 / 23 | 96.9 | 98.3 | 138.0 | 920 / 1,060 | 965 / 975 / 1,082 |
| Qwen3-0.6B GPTQ Int8 / 535 | 80.4 | 63.1 | 128.3 | 918 / 1,058 | 1,292 / 1,292 / 1,282 |
| Qwen3-0.6B AWQ / 23 | 127.2 | 112.8 | 167.6 | 707 / 848 | 764 / 757 / 870 |
| Qwen3-0.6B AWQ / 535 | 96.3 | 81.2 | 147.9 | 708 / 850 | 1,084 / 1,082 / 1,074 |
| SmolLM2-1.7B GPTQ / 23 | 60.7 | 58.6 | 75.5 | 1,329 / 1,415 | 1,590 / 1,590 / 1,666 |
| SmolLM2-1.7B GPTQ / 567 | 38.8 | 34.3 | 69.9 | 1,331 / 1,413 | 2,631 / 2,631 / 2,222 |
| SmolLM2-1.7B AWQ / 23 | 51.0 | 55.7 | 67.0 | 1,329 / 1,415 | 1,602 / 1,581 / 1,664 |

The in-memory repack leaves steady-state memory and decode unchanged: sefai on each checkpoint and on its MLX-layout copy have the same peak footprint, and decode differs within run-to-run variation.
Against the reference, sefai decodes at 0.56–0.80× its throughput, the native adapter gap Phase 6 measured, with 6–17% lower host RSS; the long SmolLM2 case has a higher process peak.
All outputs match the reference after removing leading whitespace.
The SmolLM2 AWQ long case is excluded from the reference comparison because Python `mlx-lm` and the native tokenizer encode its prompt to different IDs with that export's tokenizer.
SmolLM2's checkpoints store `float32` embeddings, so they compute in `float32`.

No memory ceiling is configured beyond the context and output reservations, which the real-bundle cases check before streaming; the footprint above is the measured cost.

## Phase 8 status

Phase 8's exit gate is met on Apple M5 with macOS 27.0 for the four named fixtures: one real checkpoint of each scheme loads without conversion, streams through the TUI and harness, handles tools, cancels and unloads, rejects unmappable packings clearly, and is compared with its selected runtime.

These remain open and are not inferred from it:

- NVIDIA is the follow-up gate: CUDA GPTQ and AWQ kernels such as Marlin or ExLlama through a qualified runtime, compared against vLLM's published matrix, on contributors' hardware.
- Act-order GPTQ, 2-, 3- and other bit widths, per-channel groups, AWQ GEMV and quantized `lm_head` checkpoints are rejected rather than supported.
- A profile's GPTQ or AWQ format is not checked against the checkpoint's metadata.
- Decode trails Python `mlx-lm` by the same margin as the MLX adapter; that is a Phase 9 and 10 concern, not specific to these formats.
