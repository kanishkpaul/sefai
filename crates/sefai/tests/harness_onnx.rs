//! Dedicated ONNX harness gate; the real graph fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_ONNX_TEST_MODEL";
const FORMAT: &str = "onnx";
const BACKEND: &str = "onnx";
#[path = "support/harness.rs"]
mod harness;
