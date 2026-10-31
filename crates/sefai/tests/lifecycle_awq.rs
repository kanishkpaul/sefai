//! Dedicated awq lifecycle contract; the real fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_LIFECYCLE_AWQ";
const FORMAT: &str = "awq";
#[path = "support/lifecycle.rs"]
mod lifecycle;
