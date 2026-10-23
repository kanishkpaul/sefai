//! Dedicated GPTQ harness gate on MLX; the real checkpoint fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_GPTQ_TEST_MODEL";
const FORMAT: &str = "gptq";
const BACKEND: &str = "mlx";
#[path = "support/harness.rs"]
mod harness;
