//! Dedicated gptq lifecycle contract; the real fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_LIFECYCLE_GPTQ";
const FORMAT: &str = "gptq";
#[path = "support/lifecycle.rs"]
mod lifecycle;
