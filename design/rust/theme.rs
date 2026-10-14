//! Terminal presentation shared by every project CLI: colour detection, the skin palette,
//! the banner, the prompt, tool rows and the spinner. No dependencies.
//!
//! Lives in a `term` module beside the generated `palettes.rs` (every skin) and
//! `identities.rs` (this project). Route every coloured write through `Term`. See TUI.md.

// Vendored whole; a CLI rarely uses every tone and helper.
#![allow(dead_code)]

use std::env;
use std::io::IsTerminal;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// One polarity of a skin. Field names follow the Hermes skin tokens they come from.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub title: Rgb,
    pub accent: Rgb,
    pub border: Rgb,
    pub dim: Rgb,
    pub text: Rgb,
    pub label: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub error: Rgb,
    pub prompt: Rgb,
    pub input_rule: Rgb,
    pub response_border: Rgb,
    pub status_bg: Rgb,
    pub status_text: Rgb,
    pub status_strong: Rgb,
    pub status_dim: Rgb,
    pub status_good: Rgb,
    pub status_warn: Rgb,
    pub status_bad: Rgb,
    pub status_critical: Rgb,
    pub selection: Rgb,
    pub shell: Rgb,
}

/// A colour skin (`palettes.rs`). Any project can wear any skin.
#[derive(Clone, Copy, Debug)]
pub struct Skin {
    pub name: &'static str,
    pub dark: Palette,
    pub light: Palette,
}

/// Who the project is in the terminal (`identities.rs`): name, glyph, faces, verbs, banner.
#[derive(Clone, Copy, Debug)]
pub struct Identity {
    pub name: &'static str,
    pub agent: &'static str,
    pub glyph: &'static str,
    pub kawaii: bool,
    pub waiting_faces: &'static [&'static str],
    pub thinking_faces: &'static [&'static str],
    pub thinking_verbs: &'static [&'static str],
    /// Six rows of ANSI Shadow block letters, or empty for the one-line wordmark.
    pub banner: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorLevel {
    None,
    Ansi256,
    TrueColor,
}

impl ColorLevel {
    /// `NO_COLOR` wins, then `CLICOLOR_FORCE`, then whether stdout is a terminal.
    pub fn detect() -> Self {
        if env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty()) {
            return Self::None;
        }
        let forced = env::var("CLICOLOR_FORCE").is_ok_and(|v| v != "0");
        if !forced
            && (!std::io::stdout().is_terminal() || env::var("TERM").is_ok_and(|t| t == "dumb"))
        {
            return Self::None;
        }
        match env::var("COLORTERM").as_deref() {
            Ok("truecolor") | Ok("24bit") => Self::TrueColor,
            _ if env::var("TERM_PROGRAM").is_ok_and(|p| p == "Apple_Terminal") => Self::Ansi256,
            _ if forced => Self::TrueColor,
            _ => Self::Ansi256,
        }
    }
}

/// Light or dark terminal. `<APP>_THEME=light|dark` overrides; otherwise `COLORFGBG`,
/// then Apple Terminal's light default. Anything undecided is dark, as in Hermes.
pub fn terminal_is_light(app_env_prefix: &str) -> bool {
    if let Ok(v) = env::var(format!("{app_env_prefix}_THEME")) {
        return v.eq_ignore_ascii_case("light");
    }
    if let Ok(v) = env::var("COLORFGBG") {
        if let Some(bg) = v.rsplit(';').next().and_then(|b| b.parse::<u8>().ok()) {
            return bg == 7 || bg == 15;
        }
    }
    env::var("TERM_PROGRAM").is_ok_and(|p| p == "Apple_Terminal")
}

/// Terminal width from `COLUMNS`, else 80. Pass a measured width where you have one.
pub fn terminal_width() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .unwrap_or(80)
}

pub struct Term {
    pub skin: &'static Skin,
    pub id: &'static Identity,
    pub palette: Palette,
    pub level: ColorLevel,
}

impl Term {
    /// `app_env_prefix` names the override variable: `"MAIKU"` reads `MAIKU_THEME`.
    pub fn new(skin: &'static Skin, id: &'static Identity, app_env_prefix: &str) -> Self {
        let light = terminal_is_light(app_env_prefix);
        Self {
            skin,
            id,
            palette: if light { skin.light } else { skin.dark },
            level: ColorLevel::detect(),
        }
    }

    fn fg(&self, c: Rgb) -> String {
        match self.level {
            ColorLevel::None => String::new(),
            ColorLevel::TrueColor => format!("\x1b[38;2;{};{};{}m", c.0, c.1, c.2),
            ColorLevel::Ansi256 => format!("\x1b[38;5;{}m", to_ansi256(c)),
        }
    }

    fn bg(&self, c: Rgb) -> String {
        match self.level {
            ColorLevel::None => String::new(),
            ColorLevel::TrueColor => format!("\x1b[48;2;{};{};{}m", c.0, c.1, c.2),
            ColorLevel::Ansi256 => format!("\x1b[48;5;{}m", to_ansi256(c)),
        }
    }

    fn reset(&self) -> &'static str {
        if self.level == ColorLevel::None {
            ""
        } else {
            "\x1b[0m"
        }
    }

    pub fn paint(&self, c: Rgb, text: &str) -> String {
        format!("{}{text}{}", self.fg(c), self.reset())
    }

    pub fn bold(&self, c: Rgb, text: &str) -> String {
        let b = if self.level == ColorLevel::None {
            ""
        } else {
            "\x1b[1m"
        };
        format!("{b}{}{text}{}", self.fg(c), self.reset())
    }

    /// Block-letter banner with the Hermes gradient (two rows title, two accent, two
    /// border). Narrower than the art plus a margin, or with no art, it is one line.
    pub fn banner(&self, width: usize, version: &str) -> String {
        let rows: Vec<&str> = self.id.banner.lines().collect();
        let art_width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let p = &self.palette;
        let mut out = String::new();
        if !rows.is_empty() && width >= art_width + 4 {
            for (i, row) in rows.iter().enumerate() {
                let c = match i {
                    0 | 1 => p.title,
                    2 | 3 => p.accent,
                    _ => p.border,
                };
                out += &if i < 2 {
                    self.bold(c, row)
                } else {
                    self.paint(c, row)
                };
                out.push('\n');
            }
        } else {
            out += &self.bold(p.title, &format!("{} {}", self.id.glyph, self.id.agent));
            out.push('\n');
        }
        out += &self.paint(p.dim, &format!("{} {version}", self.id.name));
        out.push('\n');
        out
    }

    /// `❯ ` in the prompt colour.
    pub fn prompt(&self) -> String {
        self.paint(self.palette.prompt, "❯ ")
    }

    /// The thin rule above the input line.
    pub fn input_rule(&self, width: usize) -> String {
        self.paint(self.palette.input_rule, &"─".repeat(width))
    }

    /// The label that opens an agent response: ` ✿ maiku `.
    pub fn response_label(&self) -> String {
        self.bold(
            self.palette.response_border,
            &format!(" {} {} ", self.id.glyph, self.id.agent),
        )
    }

    /// `┊ {icon} {verb:9} {detail} {duration}`. The verb column is nine cells wide so
    /// details line up down a run of tools.
    pub fn tool_row(
        &self,
        icon: &str,
        verb: &str,
        detail: &str,
        duration: Option<&str>,
        failed: bool,
    ) -> String {
        let p = &self.palette;
        let verb_color = if failed { p.error } else { p.label };
        let mut out = format!(
            "{} {icon} {} {}",
            self.paint(p.border, "┊"),
            self.paint(verb_color, &format!("{verb:<9}")),
            self.paint(p.text, detail)
        );
        if let Some(d) = duration {
            out += &format!(" {}", self.paint(p.dim, d));
        }
        out
    }

    pub fn ok(&self, text: &str) -> String {
        self.paint(self.palette.ok, text)
    }
    pub fn warn(&self, text: &str) -> String {
        self.paint(self.palette.warn, text)
    }
    pub fn error(&self, text: &str) -> String {
        self.paint(self.palette.error, text)
    }
    pub fn dim(&self, text: &str) -> String {
        self.paint(self.palette.dim, text)
    }

    /// One status-bar line, padded to `width`. Segments are (text, tone).
    pub fn status_bar(&self, width: usize, segments: &[(&str, Tone)]) -> String {
        let p = &self.palette;
        let sep = self.paint(p.status_dim, " │ ");
        let mut plain_len = 0;
        let body: Vec<String> = segments
            .iter()
            .map(|(text, tone)| {
                plain_len += text.chars().count();
                let c = match tone {
                    Tone::Text => p.status_text,
                    Tone::Strong => p.status_strong,
                    Tone::Dim => p.status_dim,
                    Tone::Good => p.status_good,
                    Tone::Warn => p.status_warn,
                    Tone::Bad => p.status_bad,
                    Tone::Critical => p.status_critical,
                };
                if matches!(tone, Tone::Strong) {
                    self.bold(c, text)
                } else {
                    self.paint(c, text)
                }
            })
            .collect();
        plain_len += 3 * segments.len().saturating_sub(1) + 1;
        let pad = " ".repeat(width.saturating_sub(plain_len));
        let bg = self.bg(p.status_bg);
        // Re-apply the background after each inner reset.
        let joined = body.join(&sep).replace("\x1b[0m", &format!("\x1b[0m{bg}"));
        format!("{bg} {joined}{pad}{}", self.reset())
    }

    /// One spinner frame. Call on a ~100ms tick; a non-terminal should print the label
    /// once instead (`ColorLevel::None` usually means that).
    pub fn spinner_frame(&self, tick: usize, thinking: bool) -> String {
        const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let faces = if thinking {
            self.id.thinking_faces
        } else {
            self.id.waiting_faces
        };
        let face = faces
            .get((tick / 30) % faces.len().max(1))
            .copied()
            .unwrap_or("");
        let verbs = self.id.thinking_verbs;
        let verb = verbs
            .get((tick / 30) % verbs.len().max(1))
            .copied()
            .unwrap_or("working");
        format!(
            "{} {} {}",
            self.paint(self.palette.accent, FRAMES[tick % FRAMES.len()]),
            self.paint(self.palette.label, face),
            self.dim(&format!("{verb}…"))
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Tone {
    Text,
    Strong,
    Dim,
    Good,
    Warn,
    Bad,
    Critical,
}

/// Nearest xterm-256 colour: the 6×6×6 cube or the grey ramp, whichever is closer.
pub fn to_ansi256(c: Rgb) -> u8 {
    const STEPS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let idx = |v: u8| {
        STEPS
            .iter()
            .enumerate()
            .min_by_key(|(_, s)| (**s as i32 - v as i32).abs())
            .map(|(i, _)| i)
            .unwrap_or(0)
    };
    let (r, g, b) = (idx(c.0), idx(c.1), idx(c.2));
    let cube = Rgb(STEPS[r], STEPS[g], STEPS[b]);
    let avg = (c.0 as u32 + c.1 as u32 + c.2 as u32) / 3;
    let grey_i = if avg < 8 { 0 } else { ((avg - 8) / 10).min(23) };
    let grey_v = (8 + grey_i * 10) as u8;
    let dist = |a: Rgb| {
        let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2);
        d(a.0, c.0) + d(a.1, c.1) + d(a.2, c.2)
    };
    if dist(Rgb(grey_v, grey_v, grey_v)) < dist(cube) {
        232 + grey_i as u8
    } else {
        16 + (36 * r + 6 * g + b) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi256_maps_extremes() {
        assert_eq!(to_ansi256(Rgb(0, 0, 0)), 16);
        assert_eq!(to_ansi256(Rgb(255, 255, 255)), 231);
        assert_eq!(to_ansi256(Rgb(255, 0, 0)), 196);
    }

    #[test]
    fn grey_prefers_ramp() {
        assert!((232..=255).contains(&to_ansi256(Rgb(0x80, 0x80, 0x80))));
    }
}
