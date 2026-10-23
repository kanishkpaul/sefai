//! Dedicated AWQ harness gate on MLX; the real checkpoint fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_AWQ_TEST_MODEL";
const FORMAT: &str = "awq";
const BACKEND: &str = "mlx";
#[path = "support/harness.rs"]
mod harness;
