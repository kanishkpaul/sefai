# sefai

Small Rust CLI for running a local GGUF model with `llama.cpp` bindings.

The binary accepts a local `.gguf` path, a prompt, a token budget, and optional GPU offload parameters:

- `--gpu-layers 0` keeps the model on CPU.
- `--gpu-layers <N>` offloads exactly `N` layers.
- `--gpu-layers all` requests maximal offload.
- `--main-gpu <INDEX>` selects the backend device index when offload is enabled.

## Requirements

- Rust toolchain (`rustup`, `cargo`, `rustc`)
- A C/C++ build toolchain
- `libclang` available during build
- CMake
- A local `.gguf` model file

Windows-specific GPU builds also benefit from:

- Visual Studio Build Tools 2022 with MSVC
- Ninja
- LLVM with `libclang.dll`
- Vulkan SDK for `--features vulkan`
- CUDA toolkit plus NVIDIA drivers for `--features cuda`

## CLI Surface

```text
sefai <MODEL> --prompt <TEXT> [--max-tokens <N>] [--gpu-layers <COUNT|all>] [--main-gpu <INDEX>]
```

Example:

```powershell
.\target\release\sefai.exe "C:\models\TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf" --prompt "Write a haiku about Rust"
```

Generate more tokens:

```powershell
.\target\release\sefai.exe "C:\models\model.gguf" --prompt "Explain GGUF in one paragraph." --max-tokens 256
```

Run with explicit offload control:

```powershell
.\target\release\sefai.exe "C:\models\model.gguf" --prompt "Summarize this model." --gpu-layers 35 --main-gpu 0
```

## Build

CPU-only release build:

```powershell
cargo build --release
```

Optional acceleration features:

```powershell
cargo build --release --features cuda
cargo build --release --features vulkan
```

## Windows Toolchain Notes

The project was validated on Windows with the following native toolchain stack:

- Rust: `stable-x86_64-pc-windows-msvc`
- MSVC: Visual Studio Build Tools 2022
- LLVM: installed so `bindgen` can find `libclang.dll`
- CMake: invoked as the full path to `cmake.exe`
- Ninja: used as the CMake generator to avoid MSBuild argument translation issues

If your shell does not already expose those tools, a known-good PowerShell setup looks like:

```powershell
$ninjaDir = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\Ninja-build.Ninja_Microsoft.Winget.Source_8wekyb3d8bbwe"
$env:PATH = "C:\Users\HP\.cargo\bin;C:\Program Files\LLVM\bin;C:\Program Files\CMake\bin;$ninjaDir;$env:PATH"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:CMAKE = "C:\Program Files\CMake\bin\cmake.exe"
$env:CMAKE_GENERATOR = "Ninja"
$env:CMAKE_MAKE_PROGRAM = "$ninjaDir\ninja.exe"
```

## Vulkan Build And Run

Install the Vulkan SDK and export `VULKAN_SDK` if the current shell does not already contain it:

```powershell
$env:VULKAN_SDK = "C:\VulkanSDK\1.4.350.0"
$env:PATH = "$env:VULKAN_SDK\Bin;$env:PATH"
```

Build:

```powershell
cargo build --release --features vulkan
```

Run:

```powershell
.\target\release\sefai.exe "D:\models\model.gguf" --prompt "Hello" --gpu-layers all --main-gpu 0
```

Technical note: on hybrid-graphics Windows laptops, the Vulkan backend may enumerate only the integrated GPU. In one validated run, `ggml_vulkan` exposed `AMD Radeon(TM) Graphics` while `nvidia-smi` separately confirmed the presence of an `RTX 3050`. In that configuration, `sefai` still offloaded successfully, but the active Vulkan device was the AMD adapter rather than the NVIDIA adapter.

## CUDA Build And Run

If you specifically need NVIDIA execution, build the CUDA backend instead of relying on Vulkan device enumeration:

```powershell
cargo build --release --features cuda
```

Run:

```powershell
.\target\release\sefai.exe "C:\models\model.gguf" --prompt "Explain GGUF in one paragraph." --gpu-layers all --main-gpu 0
```

For laptops with both integrated and discrete GPUs, CUDA is the more reliable path when the target is explicitly the NVIDIA device.

## Validated Session

The following workflow was validated end-to-end on Windows:

1. Install Rust, LLVM, Ninja, CMake, and Vulkan SDK.
2. Build `sefai` with the Ninja-backed CMake configuration.
3. Build the `vulkan` feature successfully.
4. Load a 3.20 GiB GGUF model from `D:\models`.
5. Verify prompt execution with both CPU and Vulkan-enabled runs.

Observed runtime characteristics:

- CPU-only run loaded the model and generated output successfully.
- Vulkan run offloaded all 36 layers reported by `llama.cpp`.
- The validated Vulkan run used roughly `1437.86 MiB` of model buffer on the GPU device and `2152.50 MiB` of mapped CPU model buffer.
- The same run reported `515.00 MiB` Vulkan compute buffer and `27.52 MiB` host-visible Vulkan compute buffer.

## Notes

- The CLI currently supports local GGUF files only.
- It uses the default `llama_cpp` session parameters to stay small and easy to understand.
- Release builds matter a lot for performance.
- `--gpu-layers all` delegates maximal offload to `llama.cpp` by setting `n_gpu_layers` to `u32::MAX`.
- `--main-gpu` is forwarded into `LlamaModelParams::with_main_gpu(...)`.
