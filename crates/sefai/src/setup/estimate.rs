//! Memory estimates from weight sizes, attention shape and measured runtime overhead.
//!
//! The KV cache of a dense full-attention decoder holds keys and values for every layer:
//! `2 × layers × kv_heads × head_dim × context × bytes_per_element` per sequence. Runtime
//! overhead covers compute buffers, logits, allocator retention and libraries; it comes from
//! Phase 10 measurements on one machine, so the high bound is labeled as an estimate.

use crate::workspace::Format;

/// An inspectable estimate; the low bound is weights plus cache, the high bound adds overhead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Estimate {
    pub weight_bytes: u64,
    pub cache_bytes: u64,
    pub overhead_bytes: u64,
    pub low_bytes: u64,
    pub high_bytes: u64,
}

/// How an estimate compares with memory that is available now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// The high bound leaves at least a quarter of available memory free.
    Comfortable,
    /// The high bound fits, with less than a quarter free.
    Tight,
    /// The high bound exceeds available memory.
    Exceeds,
    /// Available memory could not be read.
    Unknown,
}

impl Fit {
    pub fn name(self) -> &'static str {
        match self {
            Self::Comfortable => "comfortable",
            Self::Tight => "tight",
            Self::Exceeds => "exceeds available memory",
            Self::Unknown => "unknown",
        }
    }
}

/// Runtime overhead above weights and cache, from Phase 10 bench peaks at context 4,096.
fn overhead_bytes(format: Format) -> u64 {
    const MIB: u64 = 1024 * 1024;
    match format {
        Format::Gguf => OVERHEAD_GGUF_MIB * MIB,
        Format::Mlx | Format::Gptq | Format::Awq => OVERHEAD_MLX_MIB * MIB,
        Format::Onnx => OVERHEAD_ONNX_MIB * MIB,
    }
}

// Apple M5 peaks of 41, 129 and 291 MiB above weights and cache, rounded up; see
// docs/footprint.md. Larger vocabularies and microbatches add more.
const OVERHEAD_GGUF_MIB: u64 = 45;
const OVERHEAD_MLX_MIB: u64 = 130;
const OVERHEAD_ONNX_MIB: u64 = 300;

/// Estimates one sequence at `context` tokens.
pub fn estimate(
    format: Format,
    weight_bytes: u64,
    layers: u64,
    kv_heads: u64,
    head_dim: u64,
    cache_element_bytes: u64,
    context: u64,
) -> Estimate {
    let cache_bytes = 2u64
        .saturating_mul(layers)
        .saturating_mul(kv_heads)
        .saturating_mul(head_dim)
        .saturating_mul(context)
        .saturating_mul(cache_element_bytes);
    let low_bytes = weight_bytes.saturating_add(cache_bytes);
    let overhead_bytes = overhead_bytes(format);
    Estimate {
        weight_bytes,
        cache_bytes,
        overhead_bytes,
        low_bytes,
        high_bytes: low_bytes.saturating_add(overhead_bytes),
    }
}

impl Estimate {
    pub fn fit(&self, available_bytes: u64) -> Fit {
        if available_bytes == 0 {
            Fit::Unknown
        } else if self.high_bytes.saturating_mul(4) <= available_bytes.saturating_mul(3) {
            Fit::Comfortable
        } else if self.high_bytes <= available_bytes {
            Fit::Tight
        } else {
            Fit::Exceeds
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_follows_the_dense_attention_formula() {
        // Qwen3-0.6B: 28 layers, 8 KV heads of 128 dimensions, 16-bit cache, 4,096 tokens.
        let estimate = estimate(Format::Mlx, 0, 28, 8, 128, 2, 4096);
        assert_eq!(estimate.cache_bytes, 448 * 1024 * 1024);
    }

    #[test]
    fn fit_bands_follow_available_memory() {
        let estimate = Estimate {
            weight_bytes: 0,
            cache_bytes: 0,
            overhead_bytes: 0,
            low_bytes: 0,
            high_bytes: 750,
        };
        assert_eq!(estimate.fit(1000), Fit::Comfortable);
        assert_eq!(estimate.fit(800), Fit::Tight);
        assert_eq!(estimate.fit(700), Fit::Exceeds);
        assert_eq!(estimate.fit(0), Fit::Unknown);
    }
}
