# sefai

A small Rust CLI that runs a local GGUF model through `llama.cpp` and lets you
choose exactly how many layers go to the GPU.

```bash
sefai model.gguf --prompt "Write a haiku about Rust" --gpu-layers all
```

The whole program is one file ([`src/main.rs`](src/main.rs), about 150 lines):
load the model, tokenize the prompt, decode greedily, and stream tokens to
stdout. It's meant to be read as well as run.

## Options

```text
sefai <MODEL> --prompt <TEXT> [--max-tokens <N>] [--gpu-layers <COUNT|all>] [--main-gpu <INDEX>]
```

| Flag | Default | Meaning |
| --- | --- | --- |
| `--prompt`, `-p` | required | Text fed to the model as-is (no chat template) |
| `--max-tokens`, `-n` | `128` | Stop after this many generated tokens |
| `--gpu-layers` | `0` | `0` keeps everything on CPU, `N` offloads exactly N layers, `all` offloads as many as llama.cpp can |
| `--main-gpu` | `0` | Backend device index, used only when offload is on |

## macOS (Apple Silicon)

You need the Xcode command-line tools, a Rust toolchain, and CMake
(`brew install cmake`).

```bash
xcode-select --install          # once, if you haven't already
cargo build --release --features metal
./target/release/sefai ~/models/model.gguf -p "Explain GGUF in one paragraph." --gpu-layers all
```

Without `--features metal`, `--gpu-layers` has no GPU to offload to and
everything runs on CPU.

Measured on an Apple M5 (16 GB) with `Qwen2.5-Coder-0.5B-Instruct Q4_K_M`,
generating 64 tokens, wall time including model load:

| Mode | Time |
| --- | ---: |
| `--gpu-layers 0` (CPU) | 7.8 s |
| `--gpu-layers all` (Metal) | 1.1 s |

llama.cpp prints its load and device logs to stderr. Add `2>/dev/null` to see
only the generated text.

## Windows

This is where the project started, and the CUDA and Vulkan paths were validated
there. You need Visual Studio Build Tools 2022 (MSVC), LLVM (so `bindgen` can
find `libclang.dll`), CMake, and Ninja. Add the Vulkan SDK for
`--features vulkan`, or the CUDA toolkit plus NVIDIA drivers for
`--features cuda`.

If those tools aren't on your `PATH`, this PowerShell setup worked:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;C:\Program Files\LLVM\bin;C:\Program Files\CMake\bin;<ninja-dir>;$env:PATH"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:CMAKE_GENERATOR = "Ninja"
$env:VULKAN_SDK = "C:\VulkanSDK\<version>"   # only for --features vulkan

cargo build --release --features cuda        # or: --features vulkan
.\target\release\sefai.exe "D:\models\model.gguf" -p "Hello" --gpu-layers all --main-gpu 0
```

Ninja is the CMake generator because MSBuild mangled some of the arguments the
build passes through.

**Hybrid-graphics laptops.** On a laptop with an AMD iGPU and an RTX 3050,
the Vulkan backend enumerated only the AMD adapter, so offload succeeded but ran
on the integrated GPU. Build with `--features cuda` when you specifically want
the NVIDIA card.

In the validated Vulkan run, a 3.20 GiB model offloaded all 36 layers: about
1.44 GiB of model buffer on the GPU, 2.10 GiB mapped on the CPU, and a 515 MiB
Vulkan compute buffer.

Linux should work with the same features but hasn't been tested.

## Known limitations

These are real, reproducible, and next on the list to fix:

- **Prompts over 512 tokens fail** with `Insufficient Space of 512`. The prompt
  goes into a single 512-slot batch, and the context is fixed at 2,048 tokens.
- **Non-Latin output can come out garbled.** Each token is decoded to UTF-8 on
  its own, so a Bengali or Hindi character split across two tokens prints as
  `�`.
- **No chat template.** Instruction-tuned models get the raw prompt, which is
  why short prompts sometimes produce rambling completions.
- **Greedy decoding only.** No temperature, top-p, or seed options yet.

## Build notes

`llama-cpp-4` enables `dynamic-link` and `openmp` by default. On macOS that
produces a binary that can't find its `libggml*.dylib` files at runtime and
also depends on Homebrew's `libomp`. `Cargo.toml` turns the defaults off, so
llama.cpp is linked statically and the release binary depends only on system
frameworks. You can copy it anywhere.

## License

MIT
