#[rustfmt::skip]
#[path = "../../../../design/rust/identity.rs"]
pub mod identity;
#[rustfmt::skip]
#[path = "../../../../design/rust/palettes.rs"]
pub mod palettes;
#[rustfmt::skip]
#[path = "../../../../design/rust/theme.rs"]
pub mod theme;

use std::io::{self, Write};

#[derive(Default)]
enum State {
    #[default]
    Text,
    Escape,
    Csi,
    String,
    StringEscape,
}

/// Terminal controls can span token pieces; this state persists across writes.
#[derive(Default)]
pub struct Output {
    state: State,
}

impl Output {
    pub fn write(&mut self, text: &str, output: &mut impl Write) -> io::Result<()> {
        use State::*;
        for character in text.chars() {
            match self.state {
                Text => match character {
                    '\x1b' => self.state = Escape,
                    '\u{9b}' => self.state = Csi,
                    '\u{90}' | '\u{98}' | '\u{9d}' | '\u{9e}' | '\u{9f}' => self.state = String,
                    '\n' | '\t' => output.write_all(&[character as u8])?,
                    // Bidi embeddings, overrides and isolates can make displayed text read
                    // differently from its bytes; the marks U+200E and U+200F stay.
                    '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => {}
                    character if !character.is_control() => {
                        let mut bytes = [0; 4];
                        output.write_all(character.encode_utf8(&mut bytes).as_bytes())?;
                    }
                    _ => {}
                },
                Escape => {
                    self.state = match character {
                        '[' => Csi,
                        ']' | 'P' | 'X' | '^' | '_' => String,
                        '\x1b' => Escape,
                        ' '..='/' => Escape,
                        _ => Text,
                    }
                }
                Csi => match character {
                    '\x1b' => self.state = Escape,
                    '@'..='~' => self.state = Text,
                    _ => {}
                },
                String => match character {
                    '\x07' | '\u{9c}' => self.state = Text,
                    '\x1b' => self.state = StringEscape,
                    _ => {}
                },
                StringEscape => {
                    self.state = match character {
                        '\\' | '\x07' | '\u{9c}' => Text,
                        '\x1b' => StringEscape,
                        _ => String,
                    }
                }
            }
        }
        output.flush()
    }
}

pub fn styles() -> clap::builder::Styles {
    use clap::builder::styling::{Color, RgbColor, Style};
    let palette = theme::Term::new(&palettes::CANDLE, &identity::SEFAI, "SEFAI").palette;
    let color = |theme::Rgb(red, green, blue)| Some(Color::Rgb(RgbColor(red, green, blue)));
    clap::builder::Styles::styled()
        .header(Style::new().bold().fg_color(color(palette.title)))
        .usage(Style::new().bold().fg_color(color(palette.title)))
        .literal(Style::new().fg_color(color(palette.accent)))
        .placeholder(Style::new().fg_color(color(palette.label)))
        .error(Style::new().fg_color(color(palette.error)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_controls_can_cross_stream_chunks() {
        let mut output = Output::default();
        let mut bytes = Vec::new();
        for piece in [
            "safe\x1b]52;c;",
            "secret\x07",
            "\x1b[",
            "2Jtext\r\x08",
            "\u{9b}31mবাংলা🙂\n",
        ] {
            output.write(piece, &mut bytes).expect("output");
        }
        assert_eq!(String::from_utf8(bytes).expect("utf8"), "safetextবাংলা🙂\n");
    }

    #[test]
    fn bidi_overrides_are_removed() {
        let mut output = Output::default();
        let mut bytes = Vec::new();
        output
            .write(
                "approve \u{202e}evil\u{202c} \u{2066}x\u{2069} שלום\u{200f}",
                &mut bytes,
            )
            .expect("output");
        assert_eq!(
            String::from_utf8(bytes).expect("utf8"),
            "approve evil x שלום\u{200f}"
        );
    }

    #[test]
    fn unfinished_escape_does_not_reach_the_terminal() {
        let mut output = Output::default();
        let mut bytes = Vec::new();
        output
            .write("hello\x1b]unterminated", &mut bytes)
            .expect("output");
        assert_eq!(bytes, b"hello");
    }
}
