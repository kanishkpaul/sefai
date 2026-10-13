# sefai — archived code

This branch keeps the code sefai had before the rewrite that began on `main` in October 2026.
Nothing here is developed further.
It exists so the history stays readable and the old ideas stay easy to find.

Sefai has had two lives, and both are kept here side by side:

| Directory | What it is | Period | Platform |
| --- | --- | --- | --- |
| [`companion/`](companion/) | Sefai Companion: a desktop AI companion with a persona, memory and proactive messages | May–June 2026 | Windows |
| [`runner/`](runner/) | A ~140-line Rust CLI that runs a GGUF model through llama.cpp with explicit GPU offload | June–September 2026 | macOS (Metal), Windows (CUDA, Vulkan) |

Each directory has its own README with build instructions as they were written at the time.
The full commit history of both is on this branch.

## companion/

A Windows desktop app that tried to be a friend rather than a chat box.
It had two halves that talked over a Windows named pipe:

- **`frontend/CompanionApp/`** — a WPF app on .NET 8: chat window, onboarding, settings, tray icon, start-with-Windows, and a supervisor that restarted the backend if it died.
- **`backend/`** — a Python service:
  - `persona/` loads a persona file (see `persona.sample.json`) with identity, values, voice and refusal rules.
  - `memory/` stores every exchange in SQLite and builds context from keyword overlap plus a rolling relationship summary.
  - `autonomy/` uses APScheduler to send unprompted messages on time- and state-based triggers, with quiet hours.
  - `inference/` runs the model.

Inference was the weak part.
Each reply started a fresh `llama-cli` process with GPU offload turned off, so every message paid for a full model load and a CPU-only pass over the whole persona prompt.
`llama-cpp-python` and a canned fallback responder were the backups.
That cost is the reason the project moved to an in-process engine.

To run it you need Windows, Python 3.12 and the .NET 8 SDK; see [`companion/README.md`](companion/README.md).

## runner/

The first step of the rewrite: one Rust file that loads a GGUF model, tokenizes a prompt, decodes greedily and streams the text.
Its point was the `--gpu-layers` flag (`0`, a count, or `all`) and builds for Metal, CUDA and Vulkan.
On an Apple M5 it showed Metal running a 0.5B model about seven times faster than the CPU path.

```bash
cd runner
cargo build --release --features metal      # or: cuda, vulkan
./target/release/sefai model.gguf -p "Hello" --gpu-layers all
```

Known problems, all fixed by design in the rewrite rather than here:

- Prompts over 512 tokens fail because the prompt goes into one 512-slot batch, and the 2,048-token context is never checked.
- Each token is decoded to UTF-8 on its own, so characters split across tokens print as `�`.
- No chat template, and greedy decoding only.
- The sampler accepts every token twice, which is harmless for greedy decoding and wrong for anything else.
- `--gpu-layers 0` is not strictly CPU-only, and `--main-gpu` does not choose the device under llama.cpp's default split mode.

## Where sefai went next

The `main` branch is a ground-up rewrite: a single Rust binary that keeps the model loaded, runs on Apple Silicon and NVIDIA GPUs, and brings the companion ideas back as abilities that can be added or removed independently.
See its README and `docs/` for the design.

## Credits

Both versions run models through [llama.cpp](https://github.com/ggml-org/llama.cpp) and [ggml](https://github.com/ggml-org/ggml) by Georgi Gerganov and contributors.

## License

MIT, see [LICENSE](LICENSE).
