# Inference and engineering research

Research checked on 19 October 2026 for sefai's Rust runner/harness direction.
Primary sources below inform proposals, not claims that these backends have been built or benchmarked here.
Live upstream documentation may describe a newer implementation than the version eventually pinned; the engine spike must verify the exact dependency source.

## Format and runtime choices

| Target | What it actually contains | Plan and limits |
| --- | --- | --- |
| GGUF | Model metadata and tensors, including quantized tensors; tokenizer/template availability depends on the bundle | First embedded backend through llama.cpp; single and split files; validate architecture, tensor types and template |
| ONNX | Computation graph, tensors or external tensor-data references | Optional ONNX Runtime adapter for named decoder exports; tokenizer, generation config and KV input/output convention are also required |
| GPTQ | Quantization method and kernel/packing configuration, often tensors in safetensors | Optional qualified runtime adapter; support defined by architecture, packing and hardware, not suffix |
| AWQ | Activation-aware weight quantization method, also often safetensors plus configuration | Separate acceptance fixtures from GPTQ; do not assume their kernels or packed tensors are interchangeable |
| MLX | Framework-specific model bundle, often safetensors with tokenizer, architecture and quantization config | Apple Silicon adapter; Rust tensor bindings need actual model/generation code, or an explicitly optional worker |
| Safetensors | Tensor container, with possible sharding and an index | Recognize bundles for the target adapters; not an inference engine and not a promise that arbitrary checkpoints execute |
| PyTorch `.pt/.pth/.bin`, GGML legacy formats | Framework-specific checkpoints or superseded formats | No native execution in the initial plan; conversion/import is a separate, explicit action |

The [GGUF specification](https://github.com/ggml-org/ggml/blob/master/docs/gguf.md) defines the container; llama.cpp has [split-file tooling](https://github.com/ggml-org/llama.cpp/blob/master/tools/gguf-split/README.md).
[Safetensors](https://github.com/safetensors/safetensors) documents tensor storage and memory mapping; it does not carry a universal executable architecture.
[ONNX Runtime GenAI](https://onnxruntime.ai/docs/genai/) supplies generation building blocks, while [ort](https://github.com/pykeio/ort) supplies a Rust runtime interface; these are different abstraction levels.
[Transformers' GPTQ guide](https://huggingface.co/docs/transformers/quantization/gptq) and [AWQ guide](https://huggingface.co/docs/transformers/quantization/awq) describe quantized loading ecosystems.
[vLLM's quantization matrix](https://docs.vllm.ai/en/latest/features/quantization/) makes clear that hardware support depends on the implementation.
[MLX Rust bindings](https://github.com/oxiglade/mlx-rs) are unofficial tensor bindings; [MLX LM](https://github.com/ml-explore/mlx-lm) is a distinct generation/model implementation.

**Decision:** accept all five requested runtime targets, in separate optional packages/features.
GGUF is the portable baseline; MLX is Apple-specific; ONNX providers and GPTQ/AWQ kernels have their own qualified matrix.
External workers remain a compatibility option, with a measured dependency/footprint tradeoff, rather than an unannounced interpreter in the base product.
A conversion to GGUF never counts as direct support for the original format.

## Where local inference memory goes

| Consumer | Cost driver | First control |
| --- | --- | --- |
| Weights | Parameter count, tensor dtype/quantization blocks, scales, mixed tensors, alignment | Choose a fitting model/quantization; inspect actual tensor bytes |
| KV cache | Attention architecture, layers, KV heads, head dimensions, context capacity, sequences, cache dtype | One sequence; conservative context; supported cache quantization |
| Activations and scratch | Prefill microbatch, graph, attention implementation and backend kernels | Tune microbatch and supported Flash Attention |
| Runtime/driver | GPU context, kernel compilation, graph/allocator pools, libraries | Measure separately; avoid initializing unused backends |
| Host staging and repacking | Transfers, pinned buffers, dequantized/repacked copies | Inspect load peak and backend logs; avoid unnecessary copies |
| Tokenizer/templates/sampler | Vocabulary, prompt buffers, logits, sampling history | Reuse buffers and keep prompt construction off the token path |
| Harness/TUI | Transcript, Markdown, tools, queues, redraw buffers | Page history; bound results; on-demand skills and memory |
| Optional engines/servers | Second model, embedding model, worker runtime, MCP subprocesses | Load only what was selected; include child-process totals |
| Saved state | Serialized KV and indexes; temporary serialization copies | Bounded cache and explicit eviction; do not restore blindly |

A memory-mapped file consumes virtual address space; resident pages still use physical RAM.
The OS may reclaim clean file-backed pages, which does not make the working set free.
GPU offload may retain host mappings or staging buffers; do not assume VRAM displaces all RAM.
On Apple Silicon, CPU and GPU share a physical pool: adding reported GPU allocations to the same resident pages can double-count memory.
Report physical footprint/memory pressure alongside process and engine statistics, and clearly define each number.
The [llama.cpp CLI options](https://github.com/ggml-org/llama.cpp/blob/master/tools/cli/README.md) expose context, batch, microbatch, cache precision and placement controls.
The [backend build guide](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md) describes backend-specific dependencies and options; verify behavior against the pinned build.

### Arithmetic before optimization

For a simple dense transformer with equal key/value head dimensions and full-attention cache, estimate:

```text
weight_payload_bytes ≈ parameters × effective_bits_per_weight / 8
kv_bytes ≈ sequences × context_tokens × layers × kv_heads
           × (key_head_dimension × key_bytes + value_head_dimension × value_bytes)
peak_bytes ≈ resident_weights + allocated_cache + compute_buffers
             + staging/repacking + runtime + harness + child_processes
```

These are estimates, not allocator contracts.
Quantization blocks include scales/metadata, some tensors retain higher precision, and buffers have padding/alignment.
GQA uses KV heads, not query heads.
Sliding-window attention, mixed-layer architectures, MLA and recurrent/state-space models need their actual backend allocation model rather than this full-attention formula.

Worked example, calculated from those assumptions: 32 layers, 8 KV heads, 128 dimensions per K/V head, one sequence, 4,096 tokens and two-byte cache elements use 512 MiB of KV payload.
At 32,768 tokens the same estimate is 4 GiB.
Eight billion parameters at an ideal four bits each use about 3.73 GiB of weight payload before quantization metadata and mixed-precision tensors.
That is why "the model file fits in 4 GB" is insufficient evidence that inference fits.
Cache quantization can lower the payload but adds block overhead and may change quality/kernel compatibility; measure it.

### Optimization order

1. One loaded model and one sequence; no second embedding/draft model at startup.
2. Budget context and reserved output explicitly; load selected Markdown content, not an entire workspace.
3. Start from supported Q4/Q5-class GGUF candidates and compare output quality before recommending an aggressive quantization.
4. Use mmap where the pinned backend benefits, with mlock off by default; retain a documented opt-out for measured backend problems.
5. Sweep prefill microbatch before adding custom kernels; retain the smallest buffer setting meeting latency goals.
6. Enable supported Flash Attention; verify cache dtype support on that exact model/backend rather than assuming feature parity.
7. Compare F16 cache with supported Q8 and then Q4 candidates, testing long-context retrieval and tool arguments as well as tokens/second.
8. Reuse valid token prefixes and keep stable system text first; attention still reads growing cached context, so constant turn latency is not promised.
9. Page scrollback and tool results; keep bounded channels and stop redrawing at idle.
10. Only then evaluate disk KV restore, embeddings or speculative decoding against their added memory and complexity.

Do not automatically change the model, context or cache quality to hide an OOM.
Offer an explicit retry with the changed settings and explain the tradeoff.
The selected model's real context length, device capability and available memory constrain defaults.
An auto-fit estimate needs safety headroom and still cannot guarantee allocation success under concurrent application load.
[MLX LM's prompt/cache controls](https://github.com/ml-explore/mlx-lm#long-prompts-and-generations) are useful comparison points for the Apple adapter.
For ONNX, measure I/O binding, arena retention and cache input/output aliasing with the specific graph; a Rust wrapper alone does not establish that these are optimal.

### Measurement protocol

Record model revision/hash, quantization, template/tokenizer, engine revision, Rust/compiler flags, OS, CPU/GPU, drivers, context, output length, sequence count, batch/microbatch, Flash Attention and cache dtype.
Measure cold load peak separately from warm decode and long-conversation steady state.
Run a matrix with context 2K/4K/8K/16K, candidate microbatches 64/128/256/512, supported cache precisions and CPU/partial/full offload.
Start with a small one-variable sweep, then investigate interactions only where a result warrants it.
Keep prompts, seeds, quality checks and raw measurements in versioned evidence; repeat a fixed small number of runs and show variation.

Capture Windows process working set/private bytes and vendor GPU counters, Linux RSS/PSS/peak plus device allocations, and macOS physical footprint/pressure plus engine Metal allocation data.
Monitor the process tree when an optional runtime or MCP server is enabled.
A sampled peak may miss short spikes; include engine allocation logs and OS high-water metrics where available.
Compare GGUF to `llama-bench`, MLX to its selected reference and ONNX/GPTQ/AWQ to the selected runtime with equivalent settings.
A 5% throughput-gap target is provisional; do not call a wrapper faster from one short run.

## Engineering taste for Rust

Use the smallest implementation whose ownership, failure handling and invariants a reviewer can see.
Five readable lines are better than fifty doing the same job; five dense lines are not better than a straightforward function with explicit errors.
Prefer deletion to an unused extension seam and concrete functions/types to traits without a second actual implementation.
Use an enum for distinct outcomes instead of several booleans that permit impossible combinations.
Borrow where ownership is clear; do not clone to quiet the compiler or invent complicated lifetimes to avoid one measured allocation.

Use `Result` and contextual errors at boundaries, precise public contracts, private invariants, and bounded resources.
Keep synchronous FFI off the TUI/async executor; one owner and bounded channels make cancellation and cleanup legible.
No unsafe code in sefai; bindings may contain unsafe FFI and native code and remain part of the trust boundary.
Avoid new wrappers, managers, registries and generic builders until there is a real caller that needs them.

Comments record a reason, constraint, unit, ordering rule or failure contract that naming cannot convey.
Document public `Errors` and `Panics` where they apply; do not narrate loops or embed change history in code.
Let rustfmt decide layout and Clippy find routine issues; do not rewrite unrelated code to express a preference.
These decisions follow the [Rust API documentation guidelines](https://rust-lang.github.io/api-guidelines/documentation.html), [Rust style guide](https://doc.rust-lang.org/style-guide/) and [rust-analyzer style guide](https://rust-analyzer.github.io/book/contributing/style.html).
The more complete repository convention is [../style.md](../style.md).

A commit should expose one understandable change with its reason and evidence.
Use an imperative subject, body only when useful, and actual test results; keep dependency/binding upgrades isolated.
Before pushing, read the diff and target ref, run checks, inspect message text, and use a topic branch and PR.
Never manufacture dates or a perfect history, and never bypass hooks.
[Git's push documentation](https://git-scm.com/docs/git-push) explains ref updates and the limits of leases; a lease is not permission to rewrite `main`.

## MCP and essential security

Choose the [official Rust SDK](https://github.com/modelcontextprotocol/rust-sdk) rather than implementing the protocol from scratch.
The [transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports) distinguishes stdio and Streamable HTTP; negotiate the supported revision explicitly.
The [security guidance](https://modelcontextprotocol.io/docs/2025-11-25/tutorials/security/security_best_practices) informs endpoint trust, credential handling and confused-deputy tests.
Do not treat tool annotations, tool output or Markdown as authority to execute.
The client is not a sandbox for arbitrary third-party executables.
Test the concrete file/process/network and approval boundaries sefai owns.

## Ideas borrowed from nearby repositories

Inspected local source on this machine; these are patterns to adapt, not code copied wholesale or claims of remote repository status.

| Source inspected | Borrow | Leave behind |
| --- | --- | --- |
| `casca/casca/gates.py` and README | Decisions based on observed action/target and visible tool rows | Python/Textual stack and keyword heuristics as a universal authorization policy |
| `chromeclaw/packages/shared/src/permissions.ts` | Scope and budgets fixed before untrusted content; unknown/denied outcomes remain explicit | Browser/CDP abstractions and site-specific action code |
| `maiku/docs/CODING_STYLE.md` and README | Explicit ownership, cancellation and outcome contracts; honest foundation status | General integration platform, web UI and database requirements |
| `design-kit/system/TUI.md`, Rust palette and kit script | Terminal grammar, Candle skin and native generated theme | Browser styling as a runtime dependency |

Keep borrowing at the level of a demonstrated need.
If actual source is copied later, check its license and retain required legal notices; the no-tool-attribution rule is not a license to remove third-party notices.

## Repository enforcement limits

Local hooks are convenience guards; they can be omitted or bypassed.
CI must validate all new commit messages, pull request title/body, and added content with a trusted base-branch checker.
Required checks must block merging; changes to the checker/workflow need owner review.
Use minimal workflow permissions, immutable action SHAs and no execution of PR-controlled text in shell expressions, following [GitHub's secure-use guidance](https://docs.github.com/en/actions/reference/security/secure-use).
The [rules API](https://docs.github.com/en/rest/repos/rules#create-a-repository-ruleset) describes available rule types, but this repository rejected both broad and minimal `commit_message_pattern` requests with HTTP 422.
That rejection is recorded rather than interpreted as proof of an account-plan limitation.
No regex guarantees detection of every possible human-language signature, and an administrator retains control over repository policy.
