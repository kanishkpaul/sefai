use super::App;
use crate::term::{
    identity::SEFAI,
    palettes::CANDLE,
    theme::{ColorLevel, Rgb, Term, to_ansi256},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;

pub fn draw(frame: &mut Frame<'_>, app: &App) -> u16 {
    let theme = Term::new(&CANDLE, &SEFAI, "SEFAI");
    draw_with_theme(frame, app, &theme)
}

fn draw_with_theme(frame: &mut Frame<'_>, app: &App, theme: &Term) -> u16 {
    let palette = theme.palette;
    let style = |rgb| Style::default().fg(color(theme.level, rgb));
    let area = frame.area();
    let [header, transcript, composer, status] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                format!(" {} sefai", SEFAI.glyph),
                style(palette.title).add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                format!(" v{} · local inference · /help", env!("CARGO_PKG_VERSION")),
                style(palette.dim),
            ),
        ]),
        header,
    );
    let mut lines = Vec::new();
    if app.transcript.is_empty() {
        if transcript.width >= 40 && area.height >= 24 {
            for (index, row) in SEFAI.banner.lines().enumerate() {
                lines.push(Line::styled(
                    row,
                    style(match index {
                        0 | 1 => palette.title,
                        2 | 3 => palette.accent,
                        _ => palette.border,
                    }),
                ));
            }
            lines.push(Line::from(""));
        }
        lines.push(Line::styled(
            "Your local conversation",
            style(palette.title).add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(
            "Choose /model PATH or /profile ID, then write a message.",
        ));
        lines.push(Line::styled(
            "Markdown owns your instructions and saved sessions.",
            style(palette.dim),
        ));
    }
    for message in &app.transcript {
        let user = message.role == sefai_core::Role::User;
        lines.push(Line::styled(
            if user {
                "❯ you".into()
            } else {
                format!("{} sefai", SEFAI.glyph)
            },
            style(if user {
                palette.prompt
            } else {
                palette.response_border
            })
            .add_modifier(Modifier::BOLD),
        ));
        lines.extend(
            message
                .content
                .lines()
                .map(|line| Line::from(line.to_owned())),
        );
        lines.push(Line::from(""));
    }
    let paragraph = Paragraph::new(Text::from(lines))
        .style(style(palette.text))
        .wrap(Wrap { trim: false });
    let bottom = paragraph
        .line_count(transcript.width)
        .saturating_sub(transcript.height as usize)
        .min(u16::MAX as usize) as u16;
    let scroll = app.scroll.unwrap_or(bottom).min(bottom);
    frame.render_widget(paragraph.scroll((scroll, 0)), transcript);
    let hint = app.error.as_deref().unwrap_or(if app.busy {
        "Esc stop · Ctrl+D quit"
    } else {
        "Enter send · Shift+Enter newline · /help"
    });
    let border = Block::new()
        .borders(Borders::TOP)
        .border_style(style(palette.input_rule))
        .title_bottom(Line::styled(
            hint,
            style(if app.error.is_some() {
                palette.error
            } else {
                palette.dim
            }),
        ));
    let input_area = border.inner(composer);
    frame.render_widget(border, composer);
    let (input_lines, cursor_column) = composer_lines(&app.input, input_area.width);
    let input_count = input_lines.len();
    frame.render_widget(
        Paragraph::new(input_lines.into_iter().map(Line::from).collect::<Vec<_>>())
            .style(style(palette.prompt))
            .scroll((
                input_count
                    .saturating_sub(input_area.height as usize)
                    .min(u16::MAX as usize) as u16,
                0,
            )),
        input_area,
    );
    if !app.busy && app.overlay.is_none() && input_area.width > 0 && input_area.height > 0 {
        frame.set_cursor_position((
            input_area.x + cursor_column,
            input_area.y + (input_count.saturating_sub(1) as u16).min(input_area.height - 1),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(&app.status, style(palette.status_strong)),
        ]))
        .style(Style::default().bg(color(theme.level, palette.status_bg))),
        status,
    );
    if let Some(text) = &app.overlay {
        let overlay = Rect {
            x: area.x + area.width / 12,
            y: area.y + area.height / 12,
            width: area.width.saturating_sub(area.width / 6),
            height: area.height.saturating_sub(area.height / 6),
        };
        frame.render_widget(Clear, overlay);
        frame.render_widget(
            Paragraph::new(text.as_str())
                .scroll((app.overlay_scroll, 0))
                .style(style(palette.text))
                .wrap(Wrap { trim: false })
                .block(
                    Block::bordered()
                        .title(" Esc close ")
                        .border_style(style(palette.border)),
                ),
            overlay,
        );
    }
    bottom
}

fn composer_lines(input: &str, width: u16) -> (Vec<String>, u16) {
    if width == 0 {
        return (vec![String::new()], 0);
    }
    let prefix = if width >= 3 { "· " } else { "" };
    let mut lines = vec![if width >= 3 {
        "❯ ".into()
    } else {
        String::new()
    }];
    let prefix_width = if width >= 3 { 2 } else { 0 };
    let mut column = prefix_width;
    for grapheme in input.graphemes(true) {
        let cells = Span::raw(grapheme).width() as u16;
        if grapheme == "\n" || column + cells > width {
            lines.push(prefix.into());
            column = prefix_width;
            if grapheme == "\n" {
                continue;
            }
        }
        lines
            .last_mut()
            .expect("composer always has a line")
            .push_str(grapheme);
        column = column.saturating_add(cells);
    }
    if column >= width {
        lines.push(prefix.into());
        column = prefix_width;
    }
    (lines, column)
}

fn color(level: ColorLevel, Rgb(red, green, blue): Rgb) -> Color {
    match level {
        ColorLevel::None => Color::Reset,
        ColorLevel::TrueColor => Color::Rgb(red, green, blue),
        ColorLevel::Ansi256 => Color::Indexed(to_ansi256(Rgb(red, green, blue))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    #[test]
    fn empty_narrow_terminal_explains_model_selection() {
        let mut terminal = Terminal::new(TestBackend::new(40, 18)).expect("terminal");
        terminal
            .draw(|frame| {
                draw_with_theme(frame, &App::default(), &Term::new(&CANDLE, &SEFAI, "SEFAI"));
            })
            .expect("draw");
        let text: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(text.contains("/model PATH"));
    }

    #[test]
    fn conversation_states_match_cell_snapshots() {
        for (name, app) in [
            (
                "first-run",
                App {
                    busy: false,
                    status: "no model selected · /model PATH".into(),
                    ..App::default()
                },
            ),
            (
                "streaming",
                App {
                    transcript: vec![
                        sefai_core::Message::user("Hello"),
                        sefai_core::Message::assistant("你好 e\u{301} · streaming reply"),
                    ],
                    status: "tiny · cpu · ctx 20/512 · local".into(),
                    ..App::default()
                },
            ),
            (
                "context-full",
                App {
                    busy: false,
                    error: Some("context full; /new preserves this session".into()),
                    status: "tiny · cpu · ctx 512/512 · local".into(),
                    ..App::default()
                },
            ),
            (
                "overlay",
                App {
                    overlay: Some(super::super::HELP.into()),
                    ..App::default()
                },
            ),
        ] {
            let mut terminal = Terminal::new(TestBackend::new(48, 18)).expect("terminal");
            let mut theme = Term::new(&CANDLE, &SEFAI, "SEFAI");
            theme.level = ColorLevel::None;
            terminal
                .draw(|frame| {
                    draw_with_theme(frame, &app, &theme);
                })
                .expect("draw");
            let buffer = terminal.backend().buffer();
            let snapshot = buffer
                .content
                .chunks(48)
                .map(|row| {
                    row.iter()
                        .map(|cell| cell.symbol())
                        .collect::<String>()
                        .trim_end()
                        .to_owned()
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(format!("../../tests/fixtures/tui/{name}.txt"));
            let expected = std::fs::read_to_string(path).expect("snapshot fixture");
            assert_eq!(snapshot, expected.replace("\r\n", "\n"), "{name}");
        }
    }

    #[test]
    fn generated_polarity_and_no_color_reach_the_cell_backend() {
        for palette in [CANDLE.light, CANDLE.dark] {
            for level in [ColorLevel::TrueColor, ColorLevel::None] {
                let mut theme = Term::new(&CANDLE, &SEFAI, "SEFAI");
                theme.palette = palette;
                theme.level = level;
                let mut terminal = Terminal::new(TestBackend::new(48, 18)).expect("terminal");
                terminal
                    .draw(|frame| {
                        draw_with_theme(frame, &App::default(), &theme);
                    })
                    .expect("draw");
                let title = &terminal.backend().buffer()[(1, 0)];
                assert_eq!(title.fg, color(level, palette.title));
                if level == ColorLevel::None {
                    assert!(
                        terminal
                            .backend()
                            .buffer()
                            .content
                            .iter()
                            .all(|cell| cell.fg == Color::Reset && cell.bg == Color::Reset)
                    );
                }
            }
        }
    }

    #[test]
    fn composer_cursor_tracks_wrapped_graphemes() {
        let (lines, column) = composer_lines("你好 e\u{301} abc", 8);
        assert_eq!(lines, ["❯ 你好 e\u{301}", "·  abc"]);
        assert_eq!(column, 6);
    }

    #[test]
    fn unicode_and_small_terminal_render_without_panicking() {
        for (width, height) in [(1, 1), (12, 5), (80, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
            let app = App {
                input: "你好 e\u{301} 🙂".into(),
                transcript: vec![sefai_core::Message::assistant("界".repeat(1024))],
                ..App::default()
            };
            terminal
                .draw(|frame| {
                    draw(frame, &app);
                })
                .expect("draw");
        }
    }
}
