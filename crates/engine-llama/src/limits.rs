use crate::Error;
pub use sefai_core::{PROMPT_BYTES_MAX, Settings};

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
