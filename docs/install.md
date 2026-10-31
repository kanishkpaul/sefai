# Install, update and uninstall

Each release has one archive per platform plus `SHA256SUMS`; an archive is a directory you can put anywhere.
Nothing is installed system-wide, and sefai never downloads a model or runtime on its own.

## Choose an archive

| Archive | Engines | Notes |
| --- | --- | --- |
| `macos-arm64` | GGUF on CPU and Metal; MLX, GPTQ and AWQ; ONNX on CPU and WebGPU; MCP; model downloads | Apple Silicon; keep `mlx.metallib` and `libwebgpu_dawn.dylib` beside `sefai` |
| `macos-x86_64` | GGUF on CPU; MCP; model downloads | Intel Macs; ONNX Runtime publishes no Intel macOS build |
| `linux-x86_64`, `linux-arm64` | GGUF on CPU; ONNX on CPU; MCP; model downloads | glibc systems |
| `windows-x86_64` | GGUF on CPU; ONNX on CPU; MCP; model downloads | |

CUDA and Vulkan builds compile but have never run on NVIDIA or AMD hardware, so no archive includes them; build them from source and see [testing.md](testing.md#known-limits).

## Install

```sh
shasum -a 256 -c sefai-VERSION-PLATFORM.tar.gz.sha256
tar -xzf sefai-VERSION-PLATFORM.tar.gz
./sefai-VERSION-PLATFORM/sefai --version
```

On Windows, check the archive with `certutil -hashfile sefai-VERSION-windows-x86_64.zip SHA256`, compare it with the `.sha256` file, and extract it with Explorer.
Add the extracted directory to your `PATH` to run `sefai` from anywhere, and keep every file in it together.

Release archives are not signed or notarized, because the project has no signing certificates.
macOS therefore blocks a downloaded archive's executable on first launch; after checking the SHA-256, remove the quarantine flag with:

```sh
xattr -dr com.apple.quarantine sefai-VERSION-macos-arm64
```

Windows SmartScreen may warn for the same reason.

## First run

```sh
sefai chat
```

The first `chat` creates the Markdown workspace in your platform's configuration directory; `sefai workspace check` prints how many records it validated, and `--workspace DIR` chooses another location.
Type `/setup` for the hardware the build sees, `/setup recommend` for models that fit, and `/setup get ID` to preview a pinned download; `/setup get ID confirm` downloads and verifies it and writes its profile, then `/profile ID` selects it.
To use a model you already have, run `sefai setup import ID PATH --format gguf` (or `mlx`, `gptq`, `awq`, `onnx`).
[Model setup](setup.md) explains estimates, the catalog and downloads.

## Update

Download the newer archive, check it, and replace the old directory.
Workspace Markdown lives outside the archive and needs no migration within schema 1.

## Uninstall

Delete the extracted directory.
Your workspace, sessions, memories and downloaded models stay until you delete them yourself: by default the workspace is `~/Library/Application Support/sefai` on macOS, `$XDG_CONFIG_HOME/sefai` or `~/.config/sefai` on Linux, and `%APPDATA%\sefai` on Windows, with downloaded models under its `weights` directory.

## Build from source

Install stable Rust, CMake, a C/C++ compiler and libclang, then:

```sh
cargo build --locked --release -p sefai --features mcp-oauth,download
```

Add `metal` for GGUF on Apple GPUs, `mlx` for MLX, GPTQ and AWQ on Apple Silicon (with Xcode's Metal toolchain and `MLX_RS_METAL_PATH` set), `onnx` or `onnx-webgpu` for ONNX, and `cuda` or `vulkan` for those unqualified GPU backends.
`python3 scripts/package.py` turns a build into the same archive layout the release workflow produces, with third-party notices.
