//! Local Llama and Qwen3 MLX bundles; native objects stay on their owning thread.
//! No Python, downloads, checkpoint conversion or remote code execution.

mod bundle;
pub use bundle::validate;

#[cfg(all(feature = "runtime", target_os = "macos", target_arch = "aarch64"))]
mod native;
#[cfg(all(feature = "runtime", target_os = "macos", target_arch = "aarch64"))]
pub use native::{Cache, Error, Input, Model, memory};
