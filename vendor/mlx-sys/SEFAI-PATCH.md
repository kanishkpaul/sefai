# Build isolation patch

This is the `mlx-sys` package from `oxiglade/mlx-rs` revision `579199b4bb5e39b35f33fc6a59cb90e2c031a6a7`, including its pinned `mlx-c` source.
Only build-required source files and upstream license notices are retained.
Cargo workspace metadata is made explicit for standalone dependency use.
The build disables `MLX_BUILD_GGUF`, removes the `gguflib` link and disables unused C examples.
No FFI implementation is changed.

MLX's optional GGUF reader and llama.cpp both define `gguf_get_key` with incompatible C signatures.
Linking both unpatched libraries causes a native crash during GGUF metadata reading.
Sefai uses MLX only for safetensors bundles; GGUF belongs exclusively to `engine-llama`.
The native MLX `no_gguf.cpp` implementation provides explicit unsupported-operation errors for disabled entry points.

This third-party binding is outside the workspace's own safe Rust crates.
Its unsafe FFI remains a native dependency boundary, like the unmodified registry binding.
Keep this patch at the selected upstream revision until a compatible upstream build option replaces it.
