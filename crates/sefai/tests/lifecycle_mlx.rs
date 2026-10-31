//! Dedicated mlx lifecycle contract; the real fixture is mandatory.
const MODEL_ENV: &str = "SEFAI_LIFECYCLE_MLX";
const FORMAT: &str = "mlx";
#[path = "support/lifecycle.rs"]
mod lifecycle;
