//! Dedicated onnx lifecycle contract; the real fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_LIFECYCLE_ONNX";
const FORMAT: &str = "onnx";
#[path = "support/lifecycle.rs"]
mod lifecycle;
