use std::io;

/// Holds at most an unfinished UTF-8 character between token pieces.
pub(crate) struct Text {
    buffer: Vec<u8>,
}

impl Text {
    pub(crate) fn new(piece_bytes_max: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(piece_bytes_max + 3),
        }
    }

    pub(crate) fn push(
        &mut self,
        bytes: &[u8],
        emit: &mut impl FnMut(&str) -> io::Result<()>,
    ) -> io::Result<()> {
        self.buffer.extend_from_slice(bytes);
        let mut consumed = 0;
        for _ in 0..=self.buffer.len() {
            match str::from_utf8(&self.buffer[consumed..]) {
                Ok(text) => {
                    if !text.is_empty() {
                        emit(text)?;
                    }
                    consumed = self.buffer.len();
                    break;
                }
                Err(error) => {
                    let valid_end = consumed + error.valid_up_to();
                    let text = str::from_utf8(&self.buffer[consumed..valid_end])
                        .expect("valid_up_to identifies a valid UTF-8 prefix");
                    if !text.is_empty() {
                        emit(text)?;
                    }
                    consumed = valid_end;
                    match error.error_len() {
                        Some(length) => {
                            emit("\u{fffd}")?;
                            consumed += length;
                        }
                        None => break,
                    }
                }
            }
        }
        self.buffer.drain(..consumed);
        debug_assert!(self.buffer.len() <= 3);
        Ok(())
    }

    pub(crate) fn finish(self, emit: &mut impl FnMut(&str) -> io::Result<()>) -> io::Result<()> {
        if !self.buffer.is_empty() {
            emit("\u{fffd}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_survives_every_byte_split() {
        let expected = "বাংলা 🙂 e\u{301}";
        for split in 0..=expected.len() {
            let mut text = Text::new(expected.len());
            let mut actual = String::new();
            let mut emit = |part: &str| {
                actual.push_str(part);
                Ok(())
            };
            text.push(&expected.as_bytes()[..split], &mut emit)
                .expect("first piece");
            text.push(&expected.as_bytes()[split..], &mut emit)
                .expect("second piece");
            text.finish(&mut emit).expect("complete text");
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn partial_character_does_not_emit_empty_text() {
        let mut text = Text::new(4);
        text.push(&[0xf0, 0x9f], &mut |_| panic!("incomplete text emitted"))
            .expect("partial character");
    }

    #[test]
    fn invalid_bytes_are_replaced_once() {
        let mut text = Text::new(4);
        let mut actual = String::new();
        text.push(b"a\xffb", &mut |part| {
            actual.push_str(part);
            Ok(())
        })
        .expect("output");
        assert_eq!(actual, "a\u{fffd}b");
    }

    #[test]
    fn unfinished_final_character_is_visible() {
        let mut text = Text::new(4);
        let mut actual = String::new();
        let mut emit = |part: &str| {
            actual.push_str(part);
            Ok(())
        };
        text.push(&[0xf0, 0x9f], &mut emit)
            .expect("partial character");
        text.finish(&mut emit).expect("finish");
        assert_eq!(actual, "\u{fffd}");
    }
}
