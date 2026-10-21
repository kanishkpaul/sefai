//! Dedicated GGUF harness gate; the real model fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_TOOL_TEST_MODEL";
const FORMAT: &str = "gguf";
const BACKEND: &str = "cpu";
#[path = "support/harness.rs"]
mod harness;
