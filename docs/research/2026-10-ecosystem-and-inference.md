> **Research record, 13 October 2026.** Written before the rewrite on `main` began, about the code that is now on the [`old`](https://github.com/kanishkpaul/sefai/tree/old) branch under `runner/` and `companion/`. Its references to "sefai today" and `src/main.rs` mean that runner. Its roadmap fed [`../build-plan.md`](../build-plan.md), which supersedes it; [`../stack.md`](../stack.md) explains where the final choices differ, most notably choosing `llama-cpp-2` over `llama-cpp-4`. Claims are dated and sourced as of the date above.

# Sefai Should Grow Back Into a Companion

As of October 13, 2026, sefai is a correct-but-minimal Rust front end to llama.cpp: about 140 lines on `llama-cpp-4` 0.3.1 that load a GGUF, prefill a raw prompt, decode greedily and stream bytes. As a product it competes with tools that are much larger. Upstream llama.cpp now ships a unified `llama cli`/`llama serve`. Ollama has raised $65M and reports about 9M users. LM Studio, llamafile and mistral.rs all cover the same "run a GGUF" job. So **a standalone runner is not a defensible destination for sefai.** The project's history points to what it actually wanted to be. Until June 2026 the repo was "Sefai Companion", a single-user desktop AI friend with a persona, SQLite memory and proactive outreach. That niche is still thinly served. SillyTavern is a frontend that needs Node.js and a separate backend. The proactive companions are hobby Python or Electron projects. The one commercial local desktop companion, Backyard AI, reportedly deprecated its llama.cpp app (single source). The old companion spawned a fresh `llama-cli` process for every reply, with GPU offload hard-coded off. Read against that design, the Rust rewrite is the **engine layer the companion never had**: an in-process llama.cpp that keeps the model loaded, keeps the persona prompt in the KV cache, and runs on Metal. The roadmap below fixes five concrete bugs in `src/main.rs`, upgrades to `llama-cpp-4 =0.8.0` behind an internal engine module, adds chat templates, sampling and KV reuse, and then rebuilds the companion features on top. The rest of the report explains the ecosystem and the inference mechanics that justify each step.

## Sefai's two lives map onto two different niches

The repository has had two incarnations. The first, preserved on the `old` branch under `companion/` (last complete at `527a5da`, 2026-06-04), had four pieces:

| Piece | What it was |
|---|---|
| Frontend | WPF/.NET 8 Windows app |
| Backend | Python, over named pipes |
| Features | persona JSON, SQLite memory, an APScheduler-driven "autonomy engine" for time- and state-based outreach, refusal logic, tray notifications |
| Runtime provisioning | `LlamaRuntimeProvisioner` downloaded the llama.cpp Windows CLI |

Its inference path is the telling detail. `backend/inference/engine.py` preferred a subprocess call to `llama-cli` with `--n-gpu-layers 0`, `--single-turn`, a temp-file system prompt and a 240-second timeout. It fell back to llama-cpp-python, then to a canned "fallback responder" (`companion/backend/inference/engine.py` on the `old` branch). Every message therefore paid for a full model load and a CPU-only prefill of the whole persona prompt. The rewrite started on 2026-06-20 (`b1510f2`) and moved to static linking and a Metal-first README on 2026-09-24 (`a7defb6`, `13b90f6`). It fixes exactly that bottleneck, even though nothing yet keeps a context alive across turns.

The ecosystem research puts the original design in the "desktop AI friend" sub-niche, alongside MyAiFriend, Open-LLM-VTuber and alicization. That is a different place from the roleplay-frontend niche (SillyTavern) or the runner niche (Ollama, LM Studio) ([ecosystem notes, GitHub topic desktop-ai](https://github.com/topics/desktop-ai)). Proactive messaging, the old sefai's most distinctive feature, appears elsewhere only as an **"experimental" feature in Open-LLM-VTuber** ([Open-LLM-VTuber releases](https://github.com/Open-LLM-VTuber/Open-LLM-VTuber/releases)). Cloud companions such as Replika, Kindroid and Nomi offer it, but with no local mode found ([aitoolsatlas](https://aitoolsatlas.ai/compare/nomi-vs-kindroid)). The runner niche, by contrast, is saturated and well funded, which the next section shows. The verdict: keep the runner as sefai's engine and as a teaching artifact, but aim the project at a **single-binary, privacy-first local companion runtime**. That direction is an inference from market structure, not a measured opportunity. No market sizing for local companion users was found.

## Every popular local-LLM app is a product layer over ggml

### From a leaked-weights hack to a Hugging Face–backed standard

Georgi Gerganov began the ggml tensor library in **September 2022** ([sponsors.ecosyste.ms](https://sponsors.ecosyste.ms/accounts/ggerganov)). llama.cpp followed around **March 10, 2023**, with the goal of running 4-bit LLaMA on a MacBook. Simon Willison called it LLMs' "Stable Diffusion moment" a day later ([Simon Willison](https://simonwillison.net/tags/georgi-gerganov.atom)). The file format churned from GGML to GGMF to GGJT (which made weights mmap-able) to **GGUF on August 21, 2023**. GGUF fixed the old formats' inability to identify their architecture or carry typed hyperparameters ([GGUF spec](https://github.com/ggml-org/ggml/blob/master/docs/gguf.md); [TheBloke model card commit](https://huggingface.co/TheBloke/CodeLlama-13B-Python-GGUF/commit/ef38b45aba0cfbb8564fb38968147f15c60a64a3)).

The company ggml.ai, pre-seeded by Nat Friedman and Daniel Gross, **joined Hugging Face on February 20, 2026**. HF calls it joining; ggml.ai's site says "acquired". No terms were disclosed. Gerganov keeps technical leadership, the code stays MIT, and the stated plans are Transformers-as-source-of-truth model conversion and simpler packaging ([HF blog](https://huggingface.co/blog/ggml-joins-hf); [ggml.ai](https://ggml.ai)).

| Date | Milestone | Source |
|---|---|---|
| Sep 2022 | ggml started | [sponsors.ecosyste.ms](https://sponsors.ecosyste.ms/accounts/ggerganov) |
| ~Mar 10, 2023 | llama.cpp first release | [Willison](https://simonwillison.net/tags/georgi-gerganov.atom) |
| Jul 8, 2023 | Ollama first release | [Wikipedia](https://en.wikipedia.org/wiki/Ollama) |
| Aug 21, 2023 | GGUF replaces GGML | [model card commit](https://huggingface.co/TheBloke/CodeLlama-13B-Python-GGUF/commit/ef38b45aba0cfbb8564fb38968147f15c60a64a3) |
| Jan 2024 | i-quants (PR #4773) | [PR #4773](https://github.com/ggml-org/llama.cpp/pull/4773) |
| ~Jan 2025 | `llama_vocab` split out (b4464) | [API changelog #9289](https://github.com/ggml-org/llama.cpp/issues/9289) |
| May 15, 2025 | Ollama announces its own multimodal engine | [Ollama blog](https://ollama.com/blog/multimodal-models) |
| Jul 4, 2025 | Jan archives its Cortex fork | [cortex.cpp](https://github.com/janhq/cortex.cpp) |
| ~Aug 2025 | KV API fully moved to `llama_memory_*` (b6239) | [#9289](https://github.com/ggml-org/llama.cpp/issues/9289) |
| Feb 20, 2026 | ggml team joins Hugging Face | [HF blog](https://huggingface.co/blog/ggml-joins-hf) |
| Aug 17, 2026 | First semver release v0.1.0 (secondary source) | [zeli.app](https://zeli.app/es/story/49335017) |
| Oct 5, 2026 | v0.6.0: `llama_batch_ext`/`llama_process`, MTP speculative decoding | [v0.6.0 release](https://github.com/ggml-org/llama.cpp/releases/latest) |
| Oct 13, 2026 | Nightly b11580; ~131k stars, 24.4k forks | [GitHub](https://github.com/ggml-org/llama.cpp) |

The month-level dates for b4464 and b6239 are estimates from build numbers. The changelog itself carries no dates.

### What llama.cpp is in October 2026

Today llama.cpp has three layers:

| Layer | What it does |
|---|---|
| ggml | Compute graphs, executed by **17 pluggable backends**: Metal, CUDA, HIP, Vulkan, SYCL, OpenCL/Adreno, Hexagon, WebGPU, RPC, CPU BLAS variants and others |
| GGUF v3 | Single-file, mmap-able weight format. Typed metadata includes `tokenizer.chat_template` |
| libllama | C API: `llama_model`, `llama_context`, `llama_vocab`, a `llama_memory_*` KV API, `llama_sampler` chains and `llama_batch` (plus the new opaque `llama_batch_ext`) |

Sources: [README](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/README.md); [llama.h](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/include/llama.h).

`llama-server` has become a general local gateway. It offers OpenAI and Anthropic-compatible endpoints, continuous batching with parallel slots, prompt caching, grammar and JSON-schema output, Jinja tool calling on by default, multimodal input via `--mmproj`, multi-model **router mode**, MCP tools and Prometheus metrics ([server README](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/tools/server/README.md)).

The project cuts about **ten nightly builds a day**. It added semver `v0.x` tags in mid-2026, and those notes still say "Semantic versioning is still work in progress" ([releases](https://github.com/ggml-org/llama.cpp/releases); [SSD Nodes](https://www.ssdnodes.com/learn/pin-llama-cpp-releases-on-a-server)). The libllama API broke dozens of times between 2024 and 2026 ([#9289](https://github.com/ggml-org/llama.cpp/issues/9289)). That churn is the main tax on any binding, sefai's included. Gerganov himself said in March 2026 that many local-model quality problems come from harnesses and chat templates rather than the engine ([Willison](https://simonwillison.net/tags/georgi-gerganov.atom)). sefai today sends no chat template at all.

### The descendants, and the drift back to upstream

| Project | What it adds over llama.cpp | License | Oct 2026 status |
|---|---|---|---|
| **Ollama** | Go daemon, registry, REST on :11434, cloud models, agent integrations | MIT | $65M raised, ~9M users (Jul 2026) ([Wikipedia/TechCrunch](https://en.wikipedia.org/wiki/Ollama)). v0.40 (Oct 6) makes MLX default on Apple Silicon. v0.40.2 converts old models "for llama.cpp compatibility" (release aggregator only) ([Releasebot](https://releasebot.io/updates/ollama)) |
| **LM Studio** | Closed desktop app, MLX + llama.cpp engines, SDKs, agent "Bionic", iPhone remote | Proprietary | Active. Funding reports conflict ([LM Studio blog](https://lmstudio.ai/blog); [PitchBook](https://pitchbook.com/profiles/company/542298-61)) |
| **llamafile** | One executable for six OSes | Believed Apache-2.0 (unverified) | Rebuilt at v0.10.0 (Mar 2026) to track upstream. v0.10.6 on Sep 15 ([Mozilla.ai](https://blog.mozilla.ai/llamafile-reloaded-whats-new-in-v0-10-0/)) |
| **KoboldCpp** | Fork plus KoboldAI Lite UI, memory/world info, Tavern cards, image/TTS/STT, many API emulations | AGPL-3.0 | ~12k stars. Version reports conflict (1.118–1.122) ([GitHub](https://github.com/LostRuins/koboldcpp)) |
| **Jan** | Tauri desktop app | License unverified | Dropped Cortex in 2025. v0.8.0 (May 2026) adopts upstream router mode ([Jan changelog](https://www.jan.ai/changelog/2026-05-22-jan-v0.8.0)) |
| **GPT4All** | Desktop app | | Last tagged release Feb 2025, likely stagnant ([ML Journey](https://mljourney.com/gpt4all-review-2026-is-it-still-worth-using/)) |
| **Docker Model Runner / RamaLama / Lemonade** | Container or NPU packaging over llama.cpp (Docker also ships vLLM) | | Active ([Docker docs](https://docs.docker.com/ai/model-runner/); [Red Hat](https://www.redhat.com/en/blog/run-containerized-ai-models-locally-ramalama); [AMD](https://ryzenai.docs.amd.com/en/1.6/llm/overview.html)) |
| **SillyTavern** | Frontend only. Character cards V1–V3, lorebooks, vector memory via extensions | | ~33k stars (third-party count) ([promptquorum](https://www.promptquorum.com/local-llms/sillytavern-review)) |

The 2025–2026 story is **consolidation onto upstream**. Jan dropped its fork, llamafile was rebuilt to follow upstream, and Ollama is converting models back to llama.cpp compatibility. Ollama's 2025 custom-engine bet had drawn sustained criticism for reintroduced bugs and missing attribution ([sleepingrobots.com](https://sleepingrobots.com/dreams/stop-using-ollama/)). Differentiation has moved up the stack into registries, agents, cloud and UI.

**MLX** is the one serious second engine. On Apple Silicon it usually wins steady-state decode by roughly 1.2–1.8× over llama.cpp Metal for small and medium models, and the gap closes above about 27B. llama.cpp often keeps an edge on prompt processing before M5 and on memory-starved machines ([yage.ai](https://yage.ai/share/mlx-apple-silicon-en-20260331.html); [compute-market](https://www.compute-market.com/blog/mlx-vs-llama-cpp-apple-silicon-2026); [apple-silicon-arena](https://github.com/hoodini/apple-silicon-arena)). These are blog benchmarks, not controlled studies. At high concurrency, one September 2026 blog test found vLLM delivering **2.8–4.4× llama.cpp's aggregate throughput** at 64 requests ([insiderllm](https://insiderllm.com/guides/llamacpp-vs-ollama-vs-vllm/)). That matters for servers, not for a single-user companion.

## Inference is two different workloads sharing one cache

An LLM forward pass turns tokens into next-token logits. A runtime does that in two phases with opposite bottlenecks, glued together by the KV cache. The table traces one generation end to end and marks where sefai stands today.

| Stage | What happens | Bound by | sefai today ([src/main.rs](https://github.com/kanishkpaul/sefai/blob/old/runner/src/main.rs)) |
|---|---|---|---|
| Template | Wrap messages in the model's control tokens (`<\|im_start\|>`, `<start_of_turn>`) | Correctness | None. Raw prompt |
| Tokenize | Text to vocab IDs (byte-level BPE or SentencePiece). BOS per model metadata | Correctness | `AddBos::Always` |
| Prefill | All prompt tokens through every layer in matrix-matrix ops. Writes K/V for each position | **Compute** | One `LlamaBatch::new(512,1)`. Fails above 512 tokens |
| Decode | One token per step, matrix-vector ops. Reads every weight byte plus the KV cache | **Memory bandwidth** | Correct loop, `n_ctx` fixed at 2048, never checked |
| Sample | Logits to a token via a sampler chain | Quality | Greedy, plus a redundant `accept()` |
| Detokenize | Token to bytes. Bytes to UTF-8 across token boundaries | Correctness | Per-token `from_utf8_lossy` corrupts split characters |

### The forward pass and its arithmetic

A Llama-style decoder repeats `x ← x + Attn(RMSNorm(x)); x ← x + SwiGLU(RMSNorm(x))` across `n_layers`. Attention is `softmax(QKᵀ/√d)·V` with RoPE rotating Q and K by position ([Vaswani et al.](https://arxiv.org/abs/1706.03762); [RoFormer](https://arxiv.org/abs/2104.09864)). Grouped-query attention lets several query heads share one K/V head, shrinking the cache without much loss in quality ([Ainslie et al.](https://arxiv.org/abs/2305.13245)). A forward pass costs about **2 FLOPs per parameter per token** ([kipp.ly](https://kipp.ly/transformer-inference-arithmetic/)).

For Llama 3.1 8B the shape is 32 layers, 4096 hidden, 32 query heads and 8 KV heads ([config.json](https://huggingface.co/NousResearch/Meta-Llama-3.1-8B-Instruct/raw/main/config.json)). That gives about 8.03B parameters, or about 16 GFLOP per decoded token. Mixture-of-experts models break the link between size and speed. Mixtral 8x7B holds 47B parameters but touches 13B per token ([Jiang et al.](https://arxiv.org/abs/2401.04088)), so memory capacity scales with total parameters while decode speed scales with active ones.

### Decode speed is memory bandwidth divided by bytes read

At batch size 1, each weight byte supports only about 2 FLOPs. At Q4_0 that is about **3.6 FLOP/byte**, far below any accelerator's compute-to-bandwidth ratio. Decode therefore obeys a simple ceiling:

```
decode tokens/s  ≲  memory bandwidth  ÷  (weight bytes read per token + KV bytes read per token)
```

Measured llama.cpp results land at **45–80% of that ceiling**. Llama 2 7B Q4_0 is about 3.8 GB:

| Device | Bandwidth | Ceiling | Measured tg128 | Efficiency |
|---|---|---|---|---|
| M1 | 68 GB/s | 17.8 t/s | 14.2 | 80% |
| M4 Max | 546 GB/s | 143 t/s | 83.1 | 58% |
| **M5 base (sefai's machine class)** | 154 GB/s | 40.6 t/s | **31.9** | 78% |
| M5 Max | 614 GB/s | 162 t/s | 119.9 | 74% |
| RTX 4090 | 1008 GB/s | 264 t/s | 186–189 | ~71% |
| RTX 5090 | 1792 GB/s | 469 t/s | 290–300 | ~63% |

Sources: [discussion #4167](https://github.com/ggml-org/llama.cpp/discussions/4167); [discussion #15013](https://github.com/ggml-org/llama.cpp/discussions/15013); [TechRadar](https://www.techradar.com/computing/gpu/nvidia-rtx-5090-vs-rtx-4090). The M5 base row was measured on a 16 GB MacBook Air on 2026-08-30. The Apple numbers for older chips are pinned to a 2023 commit and understate current builds.

The practical rule is **decode t/s ≈ 0.5–0.75 × bandwidth ÷ model file size**. Quantization speeds up decode almost linearly for the same reason. On one GPU, Llama-3.1-8B tg128 went from 29 t/s at F16 to 51 at Q8_0 to 72 at Q4_K_M, while prompt processing stayed flat at about 750–920 t/s ([quantize README](https://github.com/ggml-org/llama.cpp/blob/master/tools/quantize/README.md)).

Prefill is the opposite case. N tokens share one weight read, so it is **compute-bound**. The M5 base runs 7B Q4_0 prefill at about 723 t/s, a large jump over the M4 (about 221) thanks to Metal 4 tensor ops, which ggml enables by default only on M5/M6/A19/A20 ([#4167](https://github.com/ggml-org/llama.cpp/discussions/4167); [ggml-metal-device.m](https://github.com/ggml-org/llama.cpp/blob/master/ggml/src/ggml-metal/ggml-metal-device.m)). Time to first token is therefore about prompt length ÷ prefill rate. A 2,000-token persona-plus-history prompt costs about 2.8 s on an M5 base at 7B (computed from the figures above).

For a companion this is the key number. **Avoiding re-prefill matters more than raw decode speed.** The old sefai paid it on every message, on CPU.

sefai's own README benchmark fits this picture. It reports 0.5B Q4_K_M, 64 tokens, 7.8 s on CPU versus 1.1 s on Metal, both wall time including load. The file is a few hundred MB, so the Metal decode ceiling is several hundred t/s, and the 1.1 s is mostly model load and Metal pipeline initialization. The 7.8 s CPU figure is suspiciously slow for a 0.5B model. Likely causes are thread count, build profile, or the fact that `--gpu-layers 0` is not truly CPU-only (see the GPU offload section below). This is an inference to verify with `llama-bench`, not a measurement.

### The KV cache is where context length turns into memory

For every layer and past position, the cache stores that position's post-RoPE K and V vectors ([kipp.ly](https://kipp.ly/transformer-inference-arithmetic/)):

```
KV bytes = 2 × n_layers × n_kv_heads × head_dim × n_ctx × bytes_per_element   (× parallel sequences)
```

For Llama 3.1 8B at F16 that is **128 KiB per token**. sefai's fixed 2,048 context costs 256 MiB, 8K costs 1 GiB, 32K costs 4 GiB, and the full 128K costs 16 GiB. Llama 2 7B, which lacks GQA, needs 4× as much per token. KV reads also add to the bytes read per decoded token. At 32K context the 8B model's cache roughly equals its Q4 weights, so decode speed roughly halves. Quantizing the cache to q8_0 halves both costs, and llama.cpp refuses a quantized V cache without Flash Attention ([llama-context.cpp](https://github.com/ggml-org/llama.cpp/blob/master/src/llama-context.cpp)).

In llama.cpp the cache is a fixed set of `n_ctx` cells tagged by position and sequence ID, allocated at context creation. Three batch sizes matter:

| Parameter | Library default | Meaning |
|---|---|---|
| `n_ctx` | 512 | Number of cache cells (the context window) |
| `n_batch` | 2048 | Maximum tokens accepted by one `llama_decode` call |
| `n_ubatch` | 512 | Tokens per physical compute pass |

Exceeding `n_batch` trips a `GGML_ASSERT` and aborts the process. A full cache makes `llama_decode` return 1 ("no KV slot") ([llama.h](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/include/llama.h); [llama-context.cpp](https://github.com/ggml-org/llama.cpp/blob/master/src/llama-context.cpp)). The `llama_memory_seq_rm/add/cp` API turns the cache into something a runtime can edit:

- **Multi-turn reuse:** keep the context alive, find the longest common token prefix with the new prompt, remove everything after it, and prefill only the suffix.
- **Shared prompts:** `seq_cp` a shared system prompt into several sequences.
- **Context shift:** discard the middle and re-rotate K by the position delta, which works because RoPE is relative.
- **Persistence:** `llama_state_seq_save_file` saves a sequence's KV state to disk.

A companion with a fixed persona prompt benefits directly from the last one: it can keep that prompt pre-computed on disk.

### Batching, sampling, templates and speculation

A `llama_batch` is a flat token list where each token carries its own position, sequence IDs and an output flag. One decode can therefore mix prefill chunks and decode steps from independent sequences. Rebuilding that mixed batch every step is continuous batching ([llama.h](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/include/llama.h); [Orca, OSDI '22](https://www.usenix.org/conference/osdi22/presentation/yu)). Because decode is bandwidth-bound, extra tokens per step are nearly free until arithmetic intensity reaches the device's compute-to-bandwidth balance. That point is about B ≈ 5 on an M1 Max and B ≈ 50 on an RTX 4090 (estimates from effective throughput).

Sampling is a chain. llama.cpp's `common` default order is penalties → DRY → top-n-σ → top-k → typical → top-p → min-p → XTC → temperature → `dist(seed)`. The defaults are top_k 40, top_p 0.95, min_p 0.05 and temp 0.8, with repetition penalty off ([common.h](https://github.com/ggml-org/llama.cpp/blob/master/common/common.h); [sampling.cpp](https://github.com/ggml-org/llama.cpp/blob/master/common/sampling.cpp)). min-p keeps tokens with p ≥ min_p × p_max, so the cutoff adapts to the model's confidence. It beat top-p across 1B–123B models at ICLR 2025 ([Nguyen et al.](https://arxiv.org/abs/2407.01082)). Pure greedy decoding, which sefai uses, tends toward repetitive, degenerate text ([Holtzman et al.](https://arxiv.org/abs/1904.09751)), and small quantized models are especially prone to loops.

**`llama_sampler_sample()` already calls `llama_sampler_accept()`** (verified against the bundled llama.cpp source in llama-cpp-sys-4 0.3.1). sefai's extra `sampler.accept(token)` therefore records each token twice. That is harmless for pure greedy and wrong the moment penalties, DRY or a grammar join the chain.

Chat templates are not optional for instruct models. HF's documentation says the wrong control tokens cause "drastically worse performance" ([HF chat templating](https://huggingface.co/docs/transformers/main/en/chat_templating)). GGUF embeds the Jinja template in `tokenizer.chat_template`. libllama's `llama_chat_apply_template` does not parse Jinja. It pattern-matches the template against a built-in list of formats such as ChatML, Llama 3 and Gemma ([llama.h](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/include/llama.h); [llama-chat.cpp](https://github.com/ggml-org/llama.cpp/blob/master/src/llama-chat.cpp)). Full Jinja with tool-call parsing lives in `common/chat.cpp`, which the server uses. Templated text must be tokenized with special-token parsing on. Generation must stop on any end-of-generation token: Llama 3.1 alone has three EOS IDs ([config.json](https://huggingface.co/NousResearch/Meta-Llama-3.1-8B-Instruct/raw/main/config.json)).

Detokenization has its own trap. Byte-level BPE and SentencePiece byte-fallback emit tokens that hold only part of a multi-byte UTF-8 character. llama-server buffers incomplete trailing bytes in `validate_utf8` ([server-common.cpp](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/server-common.cpp)). In Rust, `Utf8Error::error_len() == None` identifies exactly that incomplete-tail case ([Rust docs](https://doc.rust-lang.org/std/str/struct.Utf8Error.html)). sefai's README already reports the symptom: Bengali and Hindi characters printing as `�`.

**Speculative decoding** turns the bandwidth bound into speed. A cheap drafter proposes γ tokens and the target verifies them in one pass. That pass costs about one decode step while γ+1 is below the arithmetic-intensity ridge. The acceptance rule min(1, p/q) leaves the output distribution unchanged ([Leviathan et al.](https://arxiv.org/abs/2211.17192)). With acceptance rate α, expected tokens per target pass = (1 − α^(γ+1)) / (1 − α), about **3.4 at α = 0.8, γ = 4**. The low ridge on Apple Silicon makes long drafts less useful there than on discrete GPUs.

llama.cpp now supports several kinds of drafter: separate draft models, EAGLE-3, n-gram caches and **MTP heads**. MTP decoding headlined v0.6.0 ([common.h](https://github.com/ggml-org/llama.cpp/blob/master/common/common.h); [v0.6.0](https://github.com/ggml-org/llama.cpp/releases/latest)). Flash Attention is exact, IO-aware attention. On an RTX 4090 it raised 7B prefill by 23% and left decode flat ([Dao et al.](https://arxiv.org/abs/2205.14135); [#15013](https://github.com/ggml-org/llama.cpp/discussions/15013)). GBNF grammars and JSON-schema-to-grammar constrain sampling by masking invalid tokens ([sampling.cpp](https://github.com/ggml-org/llama.cpp/blob/master/common/sampling.cpp)).

## Quantization buys bandwidth, and offload decides where the bytes live

### The quant ladder: Q4_K_M by default, imatrix below Q5

GGUF quantization is block-wise. Legacy Q4_0 stores 32 weights plus one fp16 scale, which is 4.5 bits per weight. K-quants use 256-weight super-blocks whose sub-block scales are themselves quantized. I-quants encode groups of eight weights as indices into E8-lattice codebooks, and their author warned that at 2 bits "you will get total garbage without the importance matrix" ([ggml-common.h](https://github.com/ggml-org/llama.cpp/blob/master/ggml/src/ggml-common.h); [PR #4773](https://github.com/ggml-org/llama.cpp/pull/4773)). The `_S/_M/_L` suffixes are recipes. Q4_K_M bumps `attn_v` and `ffn_down` to Q6_K on about 3/8 of layers and keeps the output tensor at Q6_K, so a Llama-3.1-8B Q4_K_M file averages **4.89 bpw**, not 4.5 ([llama-quant.cpp](https://github.com/ggml-org/llama.cpp/blob/master/src/llama-quant.cpp); [quantize README](https://github.com/ggml-org/llama.cpp/blob/master/tools/quantize/README.md)). Formats added since 2024 include ternary TQ1_0/TQ2_0, MXFP4 (type 39), NVFP4 (40), Q1_0 (41) and Q2_0 (42). No published quality scoreboard was found for the last three.

| Quant (Llama-3-8B) | Size | KLD vs FP16 | Reading |
|---|---|---|---|
| Q8_0 | 7.96 GiB | 0.0014 | Effectively lossless |
| Q6_K | 6.14 GiB | 0.0055 | Near-lossless |
| Q5_K_M | 5.33 GiB | 0.011 | Excellent |
| **Q4_K_M** (+imatrix) | 4.58 GiB | 0.028–0.031 | Default sweet spot. ~92% same top token |
| IQ4_XS (+imatrix) | 4.14 GiB | 0.036 | 10% smaller, slightly worse |
| Q4_0 | 4.34 GiB | 0.072 | Kept for speed on repack/Adreno paths |
| Q3_K_M (+imatrix) | 3.74 GiB | 0.084 | Degradation noticeable |
| Q2_K (no imatrix → imatrix) | 2.96 GiB | 0.445 → 0.332 | Only for very large models |
| IQ1_S | 1.88 GiB | 2.21 | Broken for 8B |

Source: [perplexity README](https://github.com/ggml-org/llama.cpp/blob/master/tools/perplexity/README.md), measured on an RTX 4090 with wikitext-2.

Two caveats matter for sefai. First, Llama-3-8B is about **2.5× more quantization-sensitive than Llama-2-7B** at Q4_K_M (KLD 0.031 vs 0.013), so newer, denser-trained models lose more. Sub-1B models like sefai's Qwen2.5-0.5B test model are likely more sensitive still, and they are so small that Q8_0 costs almost nothing. Second, Unsloth's "Dynamic" UD- quants (now v3.0) improve KLD by only about 3–8% over a standard imatrix, at slightly larger sizes ([Unsloth docs](https://unsloth.ai/docs/basics/unsloth-dynamic-2.0-ggufs)).

Kernels almost never materialize dequantized matrices. The CPU quantizes activations on the fly to Q8_0/Q8_1/Q8_K and runs integer dot products. GPUs dequantize tiles inside the matmul ([ggml-cpu.c](https://github.com/ggml-org/llama.cpp/blob/master/ggml/src/ggml-cpu/ggml-cpu.c); [build.md](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md)).

### GPU offload semantics that sefai's flags currently get wrong

`n_gpu_layers` counts **n_layer + 1 slots**: the repeating blocks plus the output layer. They fill **from the output end backwards**, and token embeddings always stay on the CPU ([llama-model.cpp](https://github.com/ggml-org/llama.cpp/blob/master/src/llama-model.cpp)). The defaults differ by layer of the stack. libllama's default is −1, "all layers", with **no** auto-fit. The CLI tools treat −1 as `auto` and run `common/fit.cpp`, which shrinks the context toward 4096 and places layers and MoE experts to fit free memory with a 1 GiB margin per device ([common.h](https://github.com/ggml-org/llama.cpp/blob/master/common/common.h); [fit.cpp](https://github.com/ggml-org/llama.cpp/blob/master/common/fit.cpp)).

Three consequences hit sefai's current flags directly.

The first is about CPU mode. **`--gpu-layers 0` is not CPU-only.** The build docs say the GPU "may still be used … even when using the `-ngl 0` option" and that `--device none` disables it. Op offload sends large batches to the GPU, with a default threshold of 32 tokens on Metal ([build.md](https://github.com/ggml-org/llama.cpp/blob/master/docs/build.md); [ggml-metal-device.m](https://github.com/ggml-org/llama.cpp/blob/master/ggml/src/ggml-metal/ggml-metal-device.m)). sefai's README claims `0` "keeps everything on CPU". `llama-cpp-4` 0.3.1's `LlamaModelParams` exposes no device-list or split-mode setter (checked in the local crate source), so sefai cannot currently honour that claim.

The second is about `--main-gpu`. **The flag only matters with split mode `none`.** Under the default `layer` mode, layers spread across all visible devices in proportion to free memory ([llama.h](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/include/llama.h); [multi-gpu.md](https://github.com/ggml-org/llama.cpp/blob/master/docs/multi-gpu.md)). sefai sets `main_gpu` but cannot set the split mode, so on a multi-device machine the flag does not pick where the layers go.

The third is about the hybrid Windows laptop. When the RTX 3050 was missing from Vulkan, that was not a ggml choice. ggml's Vulkan backend includes every discrete and integrated GPU the loader returns ([ggml-vulkan.cpp](https://github.com/ggml-org/llama.cpp/blob/master/ggml/src/ggml-vulkan/ggml-vulkan.cpp)). The classic culprit is AMD's switchable-graphics implicit layer, and the community workaround is `DISABLE_LAYER_AMD_SWITCHABLE_GRAPHICS_1=1` ([Khronos forum](https://community.khronos.org/t/vkenumeratephysicaldevices-incorrect-results/107465); [Arch Wiki](https://wiki.archlinux.org/title/Vulkan)). Those are forum and wiki sources, not llama.cpp docs. The README's advice to build with `--features cuda` there is sound. CUDA on Ampere has historically been well ahead of Vulkan for prefill, and the two are now close for decode ([Vulkan scoreboard #10879](https://github.com/ggml-org/llama.cpp/discussions/10879)).

On the M5 16 GB, the GPU budget llama.cpp sees is Metal's `recommendedMaxWorkingSetSize`: **12.7 GB**. That is why 7B F16 (13.5 GiB) does not run there ([#4167](https://github.com/ggml-org/llama.cpp/discussions/4167); [ggml-metal-device.m](https://github.com/ggml-org/llama.cpp/blob/master/ggml/src/ggml-metal/ggml-metal-device.m)). After KV and compute buffers, the practical full-offload ceiling is roughly a 14B model at Q4_K_M (estimate). The `iogpu.wired_limit_mb` sysctl can raise the budget, but it is undocumented and unverified on the Darwin 27 the repo builds on ([llmconfigurator](https://llmconfigurator.com/en/guides/troubleshooting/increase-metal-vram-limit-apple-silicon)). For a companion that must coexist with the user's other apps, the sensible default is a 3–8B model at Q4_K_M with a modest context.

## Two Rust binding families exist, and sefai pins the riskier one

| Option | Version (date) | Adoption | Upstream lag (Oct 13, 2026) | Notes |
|---|---|---|---|---|
| `llama-cpp-4` 0.3.1 (sefai today) | 2026-06-04 | — | ~4 months (upstream commit of 2026-06-03) | No `load_mode`, `llama_batch_ext`, `load_mtp`, fit or split-mode/device setters |
| **`llama-cpp-4` 0.8.0** | 2026-10-11 | 30.5k total downloads, 51 stars, 1 owner | ~2 days (b11557, includes v0.6.0) | Wraps `common/chat.h` (Jinja, tool parsing), `fit_params`, `common_sampler`, MTP/EAGLE-3/n-gram speculation, `LlamaBatchExt`. Carries local patches into `llama_decode` |
| `llama-cpp-2` 0.1.159 | 2026-10-07 | ~1.59M downloads, 657 stars | ~3 weeks (2026-09-21 commit, pre-v0.6.0) | "does not follow semver meaningfully". Better packaging: `dynamic-backends`, `static-openmp`, `system-ggml`, llguidance. No `common/chat` wrapper |
| mistral.rs | crates.io 0.8.1 (Apr 2026); GitHub v0.9.4 (Sep 2026) | 7.7k stars | Own engine on candle | PagedAttention on CUDA/Metal, OpenAI/Anthropic servers, MCP. No Vulkan. Curated model list. Self-reported CUDA wins over llama.cpp |
| candle / burn / kalosm / shimmy | — | — | — | Frameworks or early engines. shimmy dropped llama.cpp in v2.0 for a WebGPU engine |

Sources: [crates.io llama-cpp-4](https://crates.io/crates/llama-cpp-4); [CHANGELOG](https://github.com/eugenehp/llama-cpp-rs/blob/main/CHANGELOG.md); [crates.io llama-cpp-2](https://crates.io/crates/llama-cpp-2); [llama-cpp-rs README](https://github.com/utilityai/llama-cpp-rs/blob/main/README.md); [mistral.rs](https://github.com/EricLBuehler/mistral.rs); [candle](https://github.com/huggingface/candle); [shimmy Cargo.toml](https://github.com/Michael-A-Kuykendall/shimmy/blob/main/Cargo.toml).

`llama-cpp-4` is almost certainly a hard fork of utilityai's crate. It reuses the same 0.1.8x numbering and repo name. It went **dormant for 13 months** (Feb 2025 to Mar 2026), then shipped as many as a dozen releases a day. Its bus factor is one. In its favour, its 2026 changelog documents every break, and it added CI gates.

The crate also explains a piece of sefai's own history. Its 0.3.1 defaults included `dynamic-link` with `CMAKE_SKIP_RPATH=ON`, and its README contradicted its own Cargo.toml about those defaults. The result was a macOS binary that could not find its dylibs. That is why commit `a7defb6` had to turn default features off. 0.6.0 later fixed the dynamic-link runtime bug ([llama-cpp-sys-4 0.3.1 build.rs](https://docs.rs/crate/llama-cpp-sys-4/0.3.1/source/build.rs); [CHANGELOG](https://github.com/eugenehp/llama-cpp-rs/blob/main/CHANGELOG.md)). Static linking remains the right call for a single binary.

The 0.3.1 → 0.8.0 upgrade trades risk for exactly the features sefai needs next. The cost is a set of breaking changes:

- MSRV rises to 1.88.
- Model params are passed pinned.
- `with_flash_attention` becomes `with_flash_attn_type`.
- The `penalties` and `dry` signatures change.
- `clear_kv_cache_seq` takes `Option` for the sequence ID.
- Session files from 0.7 no longer load.

None of these were compiled in this research, so treat the list as a starting checklist. The alternative, `llama-cpp-2`, offers about 50× the users and better packaging, but lags about three weeks and would make sefai bring its own Jinja engine (e.g. minijinja). The recommendation is to **pin `llama-cpp-4 = "=0.8.0"`, commit the lockfile, and put it behind a thin internal `engine` module.** Switching to `llama-cpp-2` then becomes a moderate port, because the two crates share names such as `LlamaModel`, `LlamaBatch` and `LlamaSampler`.

Pure-Rust engines are the wrong base for a "run any GGUF" tool in 2026. Upstream supports new architectures within days, and mistral.rs's published speed wins are self-reported and CUDA-only.

Larger Rust apps mostly don't embed llama.cpp at all. Tabby's `LlamaCppSupervisor` and Jan's `tauri-plugin-llamacpp` spawn and supervise `llama-server` ([tabby supervisor.rs](https://github.com/TabbyML/tabby/blob/main/crates/llama-cpp-server/src/supervisor.rs); [Jan plugin](https://github.com/janhq/jan/blob/main/src-tauri/plugins/tauri-plugin-llamacpp/src/commands.rs)). That isolates crashes and FFI churn. Embedding is justified when you need token-level control: KV surgery, persona state on disk, custom stopping, in-process embeddings. A companion with persistent persona state is that case. The old sefai's per-message `llama-cli` spawning was the worst of both approaches: subprocess overhead with no server-side cache.

## A five-stage roadmap from runner to companion runtime

The order below fixes correctness first, because every later feature depends on long prompts and clean text. It upgrades the crate once the fixes are small and testable. Only then does it add the companion layer. **Stage 0 is a prerequisite the repo currently lacks: a pinned test model and a smoke test.** No GGUF was available locally when the coordinator verified the build, so generation has not been exercised since the Metal-first changes. A tiny instruct GGUF (Qwen2.5-0.5B-Instruct at Q8_0, given small models' quantization sensitivity) plus a golden-output test with greedy sampling would catch regressions across the upgrade.

| # | Change | Where | Why | Effort |
|---|---|---|---|---|
| 0 | Test model + smoke test (non-Latin prompt, >512-token prompt) | new `tests/`, CI script | Nothing currently exercises generation | S |
| 1a | **Chunked prefill**: `LlamaBatch::new(n_batch,1)`, loop `tokens.chunks(n_batch)` with absolute positions, logits only on the final token | prompt loop | Prompts over 512 tokens fail today ("Insufficient Space of 512") | S |
| 1b | **Check `prompt + max_tokens ≤ n_ctx`**; add `--ctx-size` (default 4096–8192; warn above `n_ctx_train`) | context params | Decode returns "no KV slot" once 2048 fills | S |
| 1c | **Delete `sampler.accept(token)`** | decode loop | Double-accept is verified; breaks penalties, DRY and grammar | XS |
| 1d | **UTF-8 carry buffer** (`valid_up_to`/`error_len`) and `Special::Plaintext` | print path | Fixes the `�` bug; stops control tokens like `<think>` leaking | S |
| 1e | BOS per `llama_vocab_get_add_bos`, not `AddBos::Always` | tokenize | Avoids double BOS once templates arrive | XS |
| 1f | Correct README: `--gpu-layers 0` is not CPU-only; `--main-gpu` needs split mode `none` | README | Current docs are wrong about both | XS |
| 2 | **Upgrade to `llama-cpp-4 =0.8.0`** behind `mod engine` (`load`, `tokenize`, `prefill`, `step`, `embed`) | Cargo.toml, new module | Gains Jinja chat, fit, load_mode, MTP, device/split control (to confirm on upgrade) | M |
| 3a | **Chat template by default** (`chat::ChatTemplates` in ≥0.7), `--raw` escape hatch, `--system` | engine | Instruct models need control tokens; this is the "rambling completions" in the README | S–M |
| 3b | **Sampler flags**: `--temp --top-k --top-p --min-p --seed --repeat-penalty`; default min_p 0.05 + temp 0.8 + `dist(seed)`; `--greedy` | sampler chain | Greedy loops on small quants | S |
| 3c | `--gpu-layers auto` default via `fit_params`; `--device`/`none`; split-mode `none` when `--main-gpu` is given | model params | libllama's default of "all, no fit" OOMs on small GPUs; makes CPU mode real | M |
| 3d | Report TTFT and decode t/s separately; add a `bench` subcommand | output | They have different bottlenecks; the 7.8 s CPU number needs diagnosing | S |
| 4a | **Interactive REPL with KV prefix reuse** (longest common prefix → `seq_rm` → prefill suffix) | engine | Multi-turn without re-prefill: the core companion latency win | M |
| 4b | **Persist persona-prefix KV** with `state_seq_save_file`; reload on start | engine | Removes the ~seconds-long persona prefill on every launch | M |
| 4c | Context-shift or summarize-and-truncate when near `n_ctx` | engine | Long conversations without crashing | M |
| 5a | Port the companion core from the `old` branch into Rust: persona, SQLite memory (`rusqlite`), autonomy scheduler | new crates/modules | This is the niche; the Python version's logic already exists | L |
| 5b | **Character Card V2/V3 import** (PNG `chara`/`ccv3` chunks) and lorebooks | persona | Gives access to the community's existing content library | M |
| 5c | Memory retrieval: embeddings in-process (`with_embeddings`), SQLite + vector search, periodic LLM-extracted summaries | memory | Matches hobby-app state of the art (ChromaDB + SQLite + summaries) | M–L |
| 5d | Optional OpenAI-compatible local endpoint | server | Lets SillyTavern and other frontends use sefai's memory-backed persona | M |
| 5e | Thin UI (Tauri, as Jan uses) and tray notifications for proactive messages | app | Restores what WPF did, cross-platform | L |

Sources for the API patterns: [llama.h](https://raw.githubusercontent.com/ggml-org/llama.cpp/master/include/llama.h); [llama-cpp-4 0.8.0 README](https://crates.io/crates/llama-cpp-4/0.8.0); [Character Card V3 notes](https://docs.meganova.ai/character-studio/import-export/character-cards); [SillyTavern memory extensions](https://tavernsprite.com/blog/best-sillytavern-memory-extensions/).

Several ecosystem lessons constrain the strategy.

**Track upstream rather than forking.** Jan, llamafile and now Ollama all paid to un-fork. For sefai this means staying on a binding and keeping local patches at zero.

**Credit llama.cpp and ggml prominently.** Ollama's attribution backlash is a cautionary tale ([sleepingrobots.com](https://sleepingrobots.com/dreams/stop-using-ollama/)).

**Treat distribution as a product feature.** Single-file, zero-install is what KoboldCpp and llamafile sell ([KoboldCpp](https://github.com/LostRuins/koboldcpp)). sefai's static macOS binary already qualifies. On Windows and Linux, CUDA and Vulkan need either per-backend artifacts or a move to `GGML_BACKEND_DL` dynamic backends, which today only `llama-cpp-2` exposes as a first-class feature ([llama-cpp-sys-2 build.rs](https://github.com/utilityai/llama-cpp-rs/blob/main/llama-cpp-sys-2/build.rs)).

**Consider MLX on Apple Silicon.** Both major wrappers now ship it as a second engine ([Releasebot: Ollama](https://releasebot.io/updates/ollama); [LM Studio blog](https://lmstudio.ai/blog)). That is a reason to keep the `engine` trait backend-agnostic, but not a reason to build MLX support before the companion exists.

There is a case for stopping at the runner. As a 150-line, readable reference for how libllama works, sefai has real teaching value, and the README frames it that way ("meant to be read as well as run"). Stages 0–3 serve that goal without committing to the companion. The risks of the companion path are real too. Backyard AI's reported retreat to cloud suggests that GPU-support burden and monetization are hard for local companions, although its reasons were not found and the claim rests on one review ([aipartnerguides](https://aipartnerguides.com/reviews/backyard-ai/)). Companion apps also face emotional-dependence and safety scrutiny, which this research did not cover. The old codebase's refusal logic and quiet-mode toggle are a starting point there, not a solution.

## Conclusion

The research changes how the June 2026 rewrite should be read. It looked like a pivot from a companion app to a model runner. In fact it rebuilt the one layer the companion had outsourced badly. The old design spawned a CPU-only `llama-cli` per message and re-prefilled the persona every time. A Rust process that holds the model and its KV state turns the companion's dominant latency, prompt prefill, into a one-time cost. That advantage comes from embedding rather than wrapping a server. It also explains why sefai should stay on in-process bindings while the large apps (Tabby, Jan) spawn `llama-server`: they serve many conversations, and sefai serves one persistent relationship.

The second takeaway is that sefai's current defects are not exotic. Every one of them appears in the upstream headers or examples: 512-token batches, double-accept, lossy UTF-8, a missing template, and flags whose semantics differ from what the README promises. So the fastest route to credibility is fidelity to libllama's documented contract, not new features. Once stages 0–4 land, sefai will be a runner that does a few things llama-cli does not, notably a persistent persona cache. That is the right foundation for reviving the proactive companion that, as of October 2026, no polished local app offers.
