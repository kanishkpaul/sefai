use crate::Error;

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
            threads: std::thread::available_parallelism()
                .map_or(1, |count| count.get().min(THREADS_MAX as usize) as u32),
        }
    }
}

impl Settings {
    /// Rejects invalid sizes before allocating a context or decoding tokens.
    ///
    /// # Errors
    /// Returns an error for empty input, arithmetic overflow, or exceeded limits.
    pub fn validate(self, prompt_tokens: usize, model_context_tokens: u32) -> Result<(), Error> {
        if !(1..=CONTEXT_TOKENS_MAX).contains(&self.context_tokens)
            || self.context_tokens > model_context_tokens
        {
            return Err(Error::Invalid(format!(
                "context must be 1..={} tokens and within the model limit of {model_context_tokens}",
                CONTEXT_TOKENS_MAX
            )));
        }
        if !(1..=BATCH_TOKENS_MAX).contains(&self.batch_tokens)
            || self.batch_tokens > self.context_tokens
            || self.microbatch_tokens == 0
            || self.microbatch_tokens > self.batch_tokens
        {
            return Err(Error::Invalid(format!(
                "batch must be 1..={BATCH_TOKENS_MAX} tokens within context; microbatch must be 1..=batch"
            )));
        }
        if !(1..=THREADS_MAX).contains(&self.threads) {
            return Err(Error::Invalid(format!("threads must be 1..={THREADS_MAX}")));
        }
        if prompt_tokens == 0 {
            return Err(Error::Invalid(
                "prompt must contain at least one token".into(),
            ));
        }
        let total = prompt_tokens.checked_add(self.output_tokens as usize);
        if total.is_none_or(|total| total > self.context_tokens as usize) {
            return Err(Error::Invalid(format!(
                "prompt ({prompt_tokens}) plus reserved output ({}) exceeds context ({})",
                self.output_tokens, self.context_tokens
            )));
        }
        Ok(())
    }
}

pub(crate) fn validate_text(text: &str) -> Result<(), Error> {
    if text.trim().is_empty() {
        return Err(Error::Invalid("prompt must not be blank".into()));
    }
    if text.len() > PROMPT_BYTES_MAX || text.contains('\0') {
        return Err(Error::Invalid(format!(
            "prompt must be at most {PROMPT_BYTES_MAX} bytes without nul characters"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_context_boundary_is_accepted() {
        assert!(Settings::default().validate(3968, 8192).is_ok());
    }

    #[test]
    fn output_is_reserved_before_decode() {
        assert!(Settings::default().validate(3969, 8192).is_err());
    }

    #[test]
    fn overflow_is_rejected_before_decode() {
        assert!(Settings::default().validate(usize::MAX, 8192).is_err());
    }

    #[test]
    fn zero_output_accepts_a_full_prompt() {
        let settings = Settings {
            output_tokens: 0,
            ..Settings::default()
        };
        assert!(settings.validate(4096, 8192).is_ok());
    }

    #[test]
    fn empty_tokens_are_rejected() {
        assert!(Settings::default().validate(0, 8192).is_err());
    }

    #[test]
    fn model_context_limit_is_enforced() {
        assert!(Settings::default().validate(1, 2048).is_err());
    }

    #[test]
    fn zero_batch_is_rejected() {
        assert!(
            Settings {
                batch_tokens: 0,
                ..Settings::default()
            }
            .validate(1, 8192)
            .is_err()
        );
    }

    #[test]
    fn oversized_microbatch_is_rejected() {
        assert!(
            Settings {
                microbatch_tokens: 513,
                ..Settings::default()
            }
            .validate(1, 8192)
            .is_err()
        );
    }

    #[test]
    fn huge_context_is_rejected() {
        assert!(
            Settings {
                context_tokens: u32::MAX,
                ..Settings::default()
            }
            .validate(1, u32::MAX)
            .is_err()
        );
    }

    #[test]
    fn oversize_text_is_rejected() {
        assert!(validate_text(&"x".repeat(PROMPT_BYTES_MAX + 1)).is_err());
    }

    #[test]
    fn blank_text_is_rejected() {
        assert!(validate_text(" \n\t").is_err());
    }

    #[test]
    fn nul_text_is_rejected() {
        assert!(validate_text("a\0b").is_err());
    }
}
