# Runtime unification and model setup plan

Planning only: implement runtime unification in Phase 9, qualify memory and platforms in Phase 10, then build the hardware-aware terminal setup dashboard in Phase 11.
Phase 12 hardens safety and permissions before Phase 13 full testing and essential red teaming.
GGUF, MLX, ONNX, GPTQ and AWQ remain supported-format targets; this plan does not add training, checkpoint conversion or arbitrary ONNX graph execution.
Phase 14 remains repository organization, installation and presentation.

## Decisions from research

Unify the user's workflow and the binary's adapter contract, not the underlying weight layouts or native caches.
The same model family can have several separately published exports; they are distinct artifacts with distinct identities, compatibility and measurements.
A safetensors suffix alone does not distinguish MLX, GPTQ, AWQ or ordinary floating weights.

ONNX Runtime assigns supported graph nodes to execution providers, so a detected GPU or an available provider does not prove the selected decoder graph executes there.
Preflight must check the selected graph/provider and report actual placement, including any explicitly allowed CPU partitioning; no hidden fallback.
See [ONNX execution providers](https://onnxruntime.ai/docs/execution-providers/) and [device I/O binding](https://onnxruntime.ai/docs/performance/tune-performance/iobinding.html).

GPTQ/AWQ compatibility depends on the selected runtime, packing, quantization settings and device generation; a vendor name is insufficient.
The [vLLM quantization matrix](https://docs.vllm.ai/en/latest/features/quantization/) is evidence for that runtime, not a support promise for sefai.
Phase 8 selects and qualifies the actual implementation before the dashboard recommends it.

Memory probes expose different things: [NVML](https://docs.nvidia.com/deploy/nvml-api/latest/api/group__nvmlDeviceQueries.html) reports device memory fields, while [Vulkan memory budgets](https://docs.vulkan.org/refpages/latest/refpages/source/VK_EXT_memory_budget.html) are dynamic implementation-dependent guidance.
Apple's [Metal working-set guidance](https://developer.apple.com/videos/play/tech-talks/10580/) describes a working-set recommendation rather than independent dedicated VRAM.
Keep source, timestamp, memory pool and uncertainty with every observation; missing information stays unavailable.

Pinned-revision manifests and download previews are practical: the [Hub download documentation](https://huggingface.co/docs/huggingface_hub/guides/download) describes revision selection and dry runs reporting file sizes and cache state.
Use those semantics as a reference; do not add a Python dependency to the product for setup.

## Phase 9 — Runtime unification

### Reuse the existing implementation

Phase 6 already shares bounded settings/statistics in `core`, selects concrete engines in `crates/sefai/src/engine.rs` and uses the same worker, harness and Markdown profiles.
Phases 7 and 8 must join that path while implementing their actual adapters.
This phase closes demonstrated inconsistencies after all adapters exist; it is not a second engine rewrite.
Retain baseline measurements in each adapter phase, then run the full memory/platform matrix in Phase 10 against the unified implementation.
Keep I/O and hardware discovery out of `core`, native types inside their adapters and composition inside the binary.
Do not create an engine plugin framework, registry or mandatory dependency on all runtimes.

### Common operations and explicit differences

| Concern | Common contract | Backend-owned detail |
| --- | --- | --- |
| Identity | Versioned Markdown profile, immutable asset identity, selected runtime/device | GGUF metadata; MLX config; ONNX graph/export signature; GPTQ/AWQ packing |
| Availability | Compiled, installed, detected, qualified and load-tested are separate states | Native libraries, drivers, provider, kernel and optional worker |
| Admission | Validate assets and settings; return actionable failure before generation | Architecture, tokenizer/template, quantization and cache layout |
| Lifecycle | Validate, load, generate, cancel, reset, unload; one active model | Native cancellation boundaries and process supervision |
| Capabilities | State support or its absence explicitly | Tool protocol, context limit, prefix reuse, KV precision, offload controls |
| Statistics | Token counts, timing definitions and tagged memory observations | Allocator/process/worker/device counters and unavailable fields |
| Errors | Distinguish missing runtime, unsupported artifact, insufficient budget and failed load | Preserve the native cause without exposing secrets or terminal controls |

Use small concrete structs/enums in the existing binary module when multiple implemented adapters need them.
A proposed load plan names the exact artifact, engine build, device/provider, context/output limits, cache dtype, supported batch controls and optional worker cost.
Backend-specific settings remain typed, optional and validated; an unsupported control is not silently ignored or presented as adjustable.
Capability metadata is descriptive and never authorizes a tool, process, network request or file write.
A model that generates text is not automatically qualified for tools.

Preserve each adapter's actual cache semantics; do not translate or share native KV tensors across engines.
Switching runtime/model/settings invalidates the appropriate cache and releases the old model before allocating the next.
Profile/session compatibility requires explicit migrations if schema changes are necessary; never overwrite hand-edited Markdown to repair it.
Keep timing and memory definitions inspectable so unlike counters are not combined under one misleading total.

### Delivery and exit gate

1. Inventory current adapter contracts and reproduce each inconsistent behavior before changing it.
2. Consolidate profile resolution, preflight results, capabilities, error categories and lifecycle through the existing binary dispatch.
3. Run the same lifecycle contract scenarios for each qualified format, with explicit capability-specific expectations and real fixture targets.

Exit when one Markdown profile and the same terminal/harness controls can select each declared format without conversion, hidden fallback or installing unrelated engines.
Verify load, Unicode streaming, supported tools, context rejection, cancellation, errors, unload, switching and session resume on the appropriate qualified hardware.
Missing runtime/provider/kernel, incompatible exports and unsupported tools must explain the specific blocker.
Retain engine-specific tests; shared tests must not erase real differences.

## Phase 11 — Hardware-aware model selection and setup

### One terminal workflow

Expose a focused setup view from first run and a planned `/setup` action; keep the conversation as the permanent home.
Reuse `/models`, profile selection, `/settings` and `/stats` rather than creating competing model managers.
Use the installed Candle palette and keyboard interaction rules; support narrow terminals, `NO_COLOR`, `TERM=dumb` and a plain equivalent.
The setup flow must work before any LLM is loaded and must not depend on an LLM to recommend one.

| Step | User sees | Result |
| --- | --- | --- |
| Hardware | CPU/OS, memory pools, devices, available engines and missing requirements | Inspectable hardware/runtime snapshot |
| Purpose | Coding/chat, required languages/tools, context and memory/speed preference | Explicit selection constraints |
| Models | Local artifacts and a small offline recommendation catalog | Shortlist with reasons, provenance and limitations |
| Load plan | Artifact, runtime/device, context, estimated peak range and headroom | Editable settings and clear blockers |
| Acquire | Existing path, or an explicit download/install plan | Verified local assets; no inference environment installed silently |
| Check | Optional bounded load/generation benchmark | Measured placement, latency, throughput and memory observations |
| Use | Preview the Markdown profile changes and selected model | Atomic profile save and load into the existing conversation workflow |

Inspecting a plan does not switch the active model.
Saving a profile and replacing the active model are distinct actions; switching preserves session records and releases old residency first.
A cancelled or failed acquisition/load leaves recoverable files/state and an actionable retry or explicit alternative, not a synthetic reply or automatic CPU/model substitution.

### Hardware discovery

Collect OS/architecture, CPU capabilities, total/available host memory, destination free disk space, device identity, dedicated/shared/unified topology, and selected engine/runtime/provider/driver versions where available.
Do not collect serial numbers or upload inventory.
Display integrated and unified memory as shared pools; do not sum them with host RAM or sum multiple GPUs as if the current adapter supports splitting.

Start with existing adapter device enumeration and Phase 10 measurement helpers; current GGUF device listings expose names/indices but not a full fit budget.
Evaluate a narrow safe OS-information wrapper such as [sysinfo](https://docs.rs/sysinfo/latest/sysinfo/struct.System.html) for host RAM/CPU/disk if existing dependencies cannot supply it.
Only enable the refreshes/features actually used; no background process inventory or daemon.
Query optional GPU budget APIs through safe dependencies owned by the relevant adapters; keep `unsafe_code = "forbid"` in workspace crates.
Do not upgrade the pinned llama binding merely to make the dashboard prettier; unavailable safe telemetry is an explicit unknown until separately justified.
Refresh on opening setup, user request and immediately before loading; do not poll hardware continuously while idle.
Probe external utilities/workers only through bounded executable/argument calls with time/output limits, never shell commands from model metadata.

### Compatibility, fit and recommendation

Use separate compatibility, memory-fit and measured-performance fields rather than an unexplained percentage score.
Compatibility states distinguish blocked, unqualified candidate and qualified configuration; a successful local smoke test does not establish every feature or context length.
Fit states are estimated comfortable, tight, exceeds budget and unknown, with bytes/GiB, context assumptions, evidence date and reasons.
Performance is measured locally, matched reference evidence or unmeasured; never invent tokens/s from GPU branding.

Apply hard filters first: artifact completeness, license/access requirements, supported architecture/export/packing, compiled/installed runtime, device/provider and required capabilities.
Rank remaining candidates by the user's task/context/language requirements and memory/latency preference, using named evaluation evidence when available.
Popularity, parameter count and a model card's claims are not quality measurements.
Offer at most three explained choices: lower memory, balanced and stronger task evidence when a candidate exists; do not fabricate all three categories.
Permit manual local profiles outside the catalog, with the same validation and honest qualification status.

The offline catalog and user preferences are schema-validated Markdown, with pinned model repository/revision, required asset list, checksums when available, license, tokenizer/template identity, architecture, quantization/export variant and evidence references.
Treat model cards and remote metadata as untrusted data; they cannot provide executable hooks, change authority or grant tool support.
No hosted recommendation service, telemetry, vector database or automatic online catalog refresh is required.
A catalog recommendation is maintained evidence, not a permanent declaration of the best model.

### Estimate memory without pretending it is exact

Combine weight residency, KV cache at the selected context/cache dtype, transient prefill/load allocations, runtime/driver/worker overhead and explicit OS/display/other-app headroom.
Use metadata and the Phase 10 measurements for that engine/version/placement; show ranges and identify unsupported extrapolation.
For conventional dense full-attention decoders, KV payload is approximately `2 × layers × kv_heads × head_dim × context × bytes_per_element` per sequence; capacity rounding, alignment and alternative cache layouts need backend-specific accounting.
An ONNX graph's cache contract or GPTQ packing can require a different calculation; unknown is preferable to substituting the formula blindly.
Disk size is not peak RAM/VRAM; mmap residency, shared allocations and OS/process/device counters must not be double-counted.
For discrete GPUs assess host and device budgets separately; for unified memory assess the shared pool and working-set guidance together without adding them.
Available memory changes, so recheck before loading and retain an inspectable estimate even if admission later fails.
Change context, quantization or offload only through an explicit revised plan supported by that adapter; no OOM retry loop or silent quality reduction.

### Acquisition, calibration and persistence

Local files work fully offline.
For downloads, preview the repository, immutable revision, exact file allowlist, destination, license/gating, network hosts, uncached bytes and temporary disk requirement before explicit approval.
Use a bounded optional Rust download path; select/reuse a dependency during implementation rather than adding network dependencies to the base package now.
Keep partial assets in a staging location, verify expected sizes/hashes, then promote the complete bundle atomically where supported; interruptions must not publish a half-valid profile.
Record whether checksums come from a trusted catalog or the publisher; matching an untrusted checksum proves transfer integrity, not safety or authenticity.
Never execute repository Python, install packages/drivers, follow arbitrary download scripts, convert weights or accept license terms silently.
Use existing credential references for gated models; never store tokens in Markdown or terminal/session output.
Remote endpoints/redirects, partial-file resume and storage races need explicit bounds and failure rules.

A user-triggered calibration loads only the selected model, runs one fixed prompt with bounded context/output/time, records load/first-token/decode and available memory observations, then explicitly retains or unloads it.
Do not load several candidates concurrently or claim a short smoke test measures intelligence or maximum-context fit.
Write the chosen configuration, user constraints and durable calibration summary as versioned Markdown; model weights, temporary downloads, secrets and rebuildable caches retain their existing explicit exceptions.
Invalidate stale recommendations/calibration when artifact, engine/provider, driver, settings or hardware identity changes.
Use current atomic writes and revision-conflict checks; preserve manual edits and redact device identifiers not needed for reproduction.

### Delivery and exit gate

1. Implement hardware/runtime inventory and offline import/preflight with synthetic snapshots and no model residency.
2. Add the bounded Markdown catalog, explainable shortlist and estimates calibrated against Phase 10 evidence.
3. Add acquisition, profile preview/save and the real load/benchmark handoff, then finish terminal/error recovery states.

Candidate implementation bounds: 128 catalog entries, 1 MiB catalog/manifest input, 256 files per acquisition, one active download/load/benchmark, 64 KiB probe output, 5-second hardware probes and a 60-second generation benchmark budget after load.
Keep existing stricter adapter/file limits; declare a user-visible byte/disk budget and bounded network/load deadlines before starting work.
Cancellation during non-interruptible native work retains the adapter's documented boundary; time budgets do not falsely promise immediate preemption.
Finalize these bounds against Phase 10 fixtures before implementation, with dedicated integration targets for real processes, downloads and models.

Exit when a new user can inspect hardware, compare a supported shortlist, acquire or import assets, understand the exact runtime/memory plan, save Markdown and generate through the existing terminal harness.
Verify all five format targets on their declared hardware, plus CPU-only, multiple/integrated GPUs, unavailable telemetry, unsupported packing/export, missing runtime, low RAM/disk, stale estimates, offline/gated downloads, interrupted acquisition, hash mismatch, profile conflict, failed load and switch/unload recovery.
Test missing/stale metadata, malicious paths/redirects, deceptive model-card text, secret redaction, terminal sanitization, keyboard-only/narrow/plain modes and deny-default approvals.
Track coverage by configuration, not a blanket cross-platform badge.
Re-measure idle and active footprint after these phases; Phase 12 hardens their trust boundaries, and final Phase 13 testing includes unification, setup and safety controls and blocks unsupported release claims.

## Work deliberately outside these phases

No new inference format, fine-tuning, quantization service, model conversion, general model marketplace, all-disk model crawl, multi-model scheduler, driver manager or arbitrary native-runtime plugin loader.
Optional runtimes remain optional; the dashboard reports required packages rather than installing them behind the user's back.
Phase 14 still owns final distributable artifacts and clean-machine installation, including optional runtime assets.
