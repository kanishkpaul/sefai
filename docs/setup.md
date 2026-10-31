# Model setup

`sefai setup` and the terminal's `/setup` help choose a model that fits this machine, acquire or import it, and save a Markdown profile.
They never load a model, run repository code or switch the active model; generation still starts only from `/profile ID` or `--profile ID`.

## Commands

| Command | Terminal | What it does |
| --- | --- | --- |
| `sefai setup hardware` | `/setup` | Shows the OS, CPU, total and available memory, whether memory is shared with the GPU, free workspace disk, and which engines this build can run |
| `sefai setup check PATH --format F [--context N]` | `/setup check F PATH` | Reads a local model's metadata, estimates weights, KV cache and runtime overhead at the context, and compares the high bound with available memory |
| `sefai setup recommend [--purpose chat\|tools] [--context N]` | `/setup recommend` | Shortlists at most three catalog models that this build can run and that fit: lower memory, balanced and largest that still fits, when distinct candidates exist |
| `sefai setup catalog` | `/setup catalog` | Lists every catalog model, its download size, pinned revision and what blocks it here |
| `sefai setup get ID [--to DIR] [--yes]` | `/setup get ID [confirm]` | Previews a download; with confirmation downloads, verifies, promotes and writes `models/ID.md` |
| `sefai setup import ID PATH --format F` | none | Checks a local model's metadata and writes `models/ID.md` pointing at it without copying files |

## Estimates

The low bound is weight bytes plus the KV cache, `2 × layers × kv_heads × head_dim × context × bytes per element`; the high bound adds runtime overhead measured per engine in [Phase 10](footprint.md).
Fit is comfortable when the high bound leaves a quarter of available memory free, tight when it still fits, and exceeds otherwise; unreadable memory is unknown.
Available memory changes, so the estimate is a plan to check, not a promise; measure with `sefai bench` before relying on a model.
On Apple Silicon CPU and GPU share one pool, so available memory is that pool; discrete GPU memory is not probed, and CUDA and Vulkan routes are compiled-only in Phase 10 evidence.
GGUF headers and `config.json` supply the shape; an ONNX export's cache dtype is not read from the graph, so `check` assumes 32-bit cache elements as an upper bound.
MLX grows its cache with the conversation, so its estimate at full context is an upper bound.

## Catalog

The built-in catalog in `crates/sefai/catalog` lists the fixtures sefai's own tests run, each pinned to an immutable revision with every file's size and SHA-256.
A workspace may add entries in `catalog/ID.md` with the same schema; an entry that reuses a built-in ID is rejected.
Entries describe artifacts; their text never grants tool support, runs code or changes settings, and a `tools` purpose records only that sefai's harness tests pass with that model.

| Field | Meaning |
| --- | --- |
| `repository`, `revision` | Hugging Face `owner/name` and a 40-character commit |
| `format`, `entry` | The engine format and the path to load inside the acquired directory, `.` for a bundle |
| `purposes` | `chat`, `tools` or both |
| `layers`, `kv_heads`, `head_dim`, `trained_context`, `cache_element_bytes` | The attention shape used for estimates |
| `files` | Every file with its `path`, `bytes` and `sha256`; at most 256 |

## Acquisition

Downloading needs a build with the `download` feature; without it `get` prints the pinned repository and the `import` command to use after fetching the files yourself.
The preview lists each file, its size, whether a matching copy is present, the destination and free disk space; nothing is downloaded until it is confirmed.
Files come only from `https://huggingface.co`; redirects are followed by hand, at most five, and only over HTTPS to `huggingface.co`, `hf.co` or their subdomains, without any proxy from the environment.
Connecting and the response headers each have 30 seconds, and each file's body gets 60 seconds plus its size at 256 KiB/s.
Each file is written to `DIR/.staging-ID`, must match its pinned size and SHA-256, and the staging directory is renamed to `DIR/ID` only when every file verifies.
An interrupted or failed acquisition keeps verified files in staging for the next attempt and never leaves a half-valid destination; an existing destination is re-verified rather than overwritten.
A hash match proves the bytes are the pinned ones, not that the model is safe; gated repositories and credentials are not supported.
The profile is created only if `models/ID.md` does not already exist, through the workspace's atomic write.
Ctrl+C during `sefai setup get` and Esc during `/setup get ID confirm` stop the download between reads; the partial file is removed and verified files stay staged for the next attempt.

## Calibration and limits

After acquiring or importing, `sefai --profile ID bench "hello"` measures load, prefill, decode and peak memory on this machine; setup does not run it automatically or store its results.
GPU memory budgets are not probed, so discrete-GPU fit is not estimated; on Apple Silicon the shared pool is used.
Gated repositories, credentials and mirrors other than Hugging Face are not supported, and a read already blocked on a stalled server waits for its deadline before cancellation is seen.
Estimates are recomputed each time they are shown, so they follow current available memory but are never cached or invalidated.
