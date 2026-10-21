//! Shared bounded request settings and observations; no runtime or I/O.

use std::fmt;

#[derive(Debug)]
pub struct LimitsError(String);

impl fmt::Display for LimitsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for LimitsError {}

pub const PROMPT_BYTES_MAX: usize = 1024 * 1024;
pub const CONTEXT_TOKENS_MAX: u32 = 131_072;
pub const BATCH_TOKENS_MAX: u32 = 4096;
pub const THREADS_MAX: u32 = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub context_tokens: u32,
    pub batch_tokens: u32,
    pub microbatch_tokens: u32,
    pub output_tokens: u32,
    pub threads: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            context_tokens: 4096,
            batch_tokens: 512,
            microbatch_tokens: 128,
            output_tokens: 128,
            threads: 1,
        }
    }
}

impl Settings {
    /// Rejects invalid sizes before allocating a context or decoding tokens.
    ///
    /// # Errors
    /// Returns an error for empty input, arithmetic overflow, or exceeded limits.
    pub fn validate(
        self,
        prompt_tokens: usize,
        model_context_tokens: u32,
    ) -> Result<(), LimitsError> {
        if !(1..=CONTEXT_TOKENS_MAX).contains(&self.context_tokens)
            || self.context_tokens > model_context_tokens
        {
            return Err(LimitsError(format!(
                "context must be 1..={} tokens and within the model limit of {model_context_tokens}",
                CONTEXT_TOKENS_MAX
            )));
        }
        if !(1..=BATCH_TOKENS_MAX).contains(&self.batch_tokens)
            || self.batch_tokens > self.context_tokens
            || self.microbatch_tokens == 0
            || self.microbatch_tokens > self.batch_tokens
        {
            return Err(LimitsError(format!(
                "batch must be 1..={BATCH_TOKENS_MAX} tokens within context; microbatch must be 1..=batch"
            )));
        }
        if !(1..=THREADS_MAX).contains(&self.threads) {
            return Err(LimitsError(format!("threads must be 1..={THREADS_MAX}")));
        }
        if prompt_tokens == 0 {
            return Err(LimitsError("prompt must contain at least one token".into()));
        }
        let total = prompt_tokens.checked_add(self.output_tokens as usize);
        if total.is_none_or(|total| total > self.context_tokens as usize) {
            return Err(LimitsError(format!(
                "prompt ({prompt_tokens}) plus reserved output ({}) exceeds context ({})",
                self.output_tokens, self.context_tokens
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    EndOfGeneration,
    OutputLimit,
    Cancelled,
}

#[derive(Debug)]
pub struct Statistics {
    pub prompt_tokens: usize,
    /// Includes the sampled stop token when generation ends with one.
    pub generated_tokens: u32,
    pub prefill: std::time::Duration,
    /// Includes sampling and the caller's output callback, excluding prefill.
    pub generation: std::time::Duration,
    pub stop: Stop,
}
