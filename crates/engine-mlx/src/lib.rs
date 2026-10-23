//! Local Llama and Qwen3 MLX, GPTQ and AWQ bundles; native objects stay on their owning thread.
//! No Python, downloads, checkpoint conversion or remote code execution.

mod bundle;
pub use bundle::{Bundle, Weights, validate};
#[cfg(any(
    test,
    all(feature = "runtime", target_os = "macos", target_arch = "aarch64")
))]
mod repack;

#[cfg(all(feature = "runtime", target_os = "macos", target_arch = "aarch64"))]
mod native;
#[cfg(all(feature = "runtime", target_os = "macos", target_arch = "aarch64"))]
mod quantized;
#[cfg(all(feature = "runtime", target_os = "macos", target_arch = "aarch64"))]
pub use native::{Cache, Error, Input, Model, memory};
