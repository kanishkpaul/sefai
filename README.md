# sefai

Small Rust CLI for running a local GGUF model with `llama.cpp` bindings.

## Requirements

- Rust toolchain (`rustup`, `cargo`, `rustc`)
- A C/C++ build toolchain
- `libclang` available during build
- A local `.gguf` model file

On Windows, the easiest route is usually Rust plus Visual Studio Build Tools with C++ support.

## Build

```powershell
cargo build --release
```

Optional acceleration features:

```powershell
cargo build --release --features cuda
cargo build --release --features vulkan
```

## Run

```powershell
cargo run --release -- "C:\models\TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf" --prompt "Write a haiku about Rust"
```

Generate more tokens:

```powershell
cargo run --release -- "C:\models\model.gguf" --prompt "Explain GGUF in one paragraph." --max-tokens 256
```

## Notes

- The CLI currently supports local GGUF files only.
- It uses the default `llama_cpp` session parameters to stay small and easy to understand.
- Release builds matter a lot for performance.
