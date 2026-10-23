//! Local Optimum-layout Llama and Qwen3 ONNX decoder exports on ONNX Runtime.
//! No Python, downloads, checkpoint conversion or arbitrary graph execution.

mod bundle;
mod signature;
pub use bundle::{Bundle, validate};
pub use signature::{Element, Layout, Port, check};

#[cfg(feature = "runtime")]
mod native;
#[cfg(feature = "runtime")]
pub use native::{Cache, Error, Input, Model};
