//! Dedicated gguf lifecycle contract; the real fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_LIFECYCLE_GGUF";
const FORMAT: &str = "gguf";
#[path = "support/lifecycle.rs"]
mod lifecycle;
