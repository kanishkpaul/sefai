//! Dedicated MLX harness gate; the real bundle fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_MLX_TEST_MODEL";
const FORMAT: &str = "mlx";
const BACKEND: &str = "mlx";
#[path = "support/harness.rs"]
mod harness;
