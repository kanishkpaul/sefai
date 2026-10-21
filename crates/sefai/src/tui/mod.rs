//! One input thread and one bounded model/workspace worker; exact-action local tool approvals.
mod view;
mod worker;

use crate::{term, workspace::Selection};
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event as InputEvent, KeyCode,
        KeyEventKind, KeyModifiers,
    },
    execute,
};
use sefai_core::Message;
use std::{
    io::{self, BufRead, IsTerminal, Read, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation;

pub const TEXT_BYTES_MAX: usize = 1024 * 1024;
const COMPOSER_BYTES_MAX: usize = 16 * 1024;
const DISPLAY_BYTES_MAX: usize = 128 * 1024;
const EVENTS_MAX: usize = 64;
const COMMANDS_MAX: usize = 1;
pub const HELP: &str = "Conversation\nEnter send · Shift+Enter newline · Esc stop/close\nPageUp/PageDown scroll · End follow · F2 error details
Ctrl+K commands · Ctrl+C stop/quit · Ctrl+D quit\n/model PATH  choose a local model\n/profile ID  choose a Markdown model profile\n/models  list profiles\n/new [ID]  preserve this conversation and start another\n/reset  start a fresh conversation\n/resume ID  open a saved session\n/sessions  list sessions\n/skills · /skill ID toggle selected skill\n/memory · /recall TEXT select literal memory recall\n/settings · /mcp · /stats · /help\n/quit  exit\nEdit Markdown with your text editor; it reloads before each turn";

#[derive(clap::Args, Default)]
pub struct Options {
    #[arg(long)]
    model: Option<PathBuf>,
    /// Line-oriented conversation without raw mode, screen controls or colour.
    #[arg(long)]
    plain: bool,
    /// Enable the documented SmolLM2 protocol and explicitly approved tools.
    #[arg(long, value_enum)]
    tool_protocol: Option<ToolProtocol>,
    /// Connect one Markdown MCP definition after exact launch approval.
    #[cfg(feature = "mcp")]
    #[arg(long, requires = "tool_protocol")]
    mcp: Option<String>,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ToolProtocol {
    Smollm2,
}

enum Event {
    Approval(String),
    Continue,
    Tool(String),
    History(Vec<Message>),
    Ready(String),
    Status(String),
    Begin(String),
    Token(String),
    Error(String),
    Overlay(String),
}

struct App {
    input: String,
    submitted: Option<String>,
    transcript: Vec<Message>,
    status: String,
    error: Option<String>,
    overlay: Option<String>,
    approval: bool,
    overlay_scroll: u16,
    busy: bool,
    scroll: Option<u16>,
    bottom: u16,
    sanitizer: term::Output,
}

impl Default for App {
    fn default() -> Self {
        Self {
            input: String::new(),
            submitted: None,
            transcript: Vec::new(),
            status: "opening workspace".into(),
            error: None,
            overlay: None,
            approval: false,
            overlay_scroll: 0,
            busy: true,
            scroll: None,
            bottom: 0,
            sanitizer: term::Output::default(),
        }
    }
}

pub fn run(selection: Selection, options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let plain = options.plain || std::env::var("TERM").is_ok_and(|value| value == "dumb");
    if !plain && (!io::stdin().is_terminal() || !io::stdout().is_terminal()) {
        return Err("chat requires a terminal; use `sefai chat --plain` for line input".into());
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let signal = cancelled.clone();
    let interrupted = Arc::new(AtomicBool::new(false));
    let signal_requested = interrupted.clone();
    ctrlc::set_handler(move || {
        signal.store(true, Ordering::Relaxed);
        signal_requested.store(true, Ordering::Relaxed);
    })?;
    let (commands, incoming) = mpsc::sync_channel(COMMANDS_MAX);
    let (outgoing, events) = mpsc::sync_channel(EVENTS_MAX);
    let flag = cancelled.clone();
    let worker = std::thread::Builder::new()
        .name("model".into())
        .spawn(move || {
            worker::run(
                selection,
                options.model,
                options.tool_protocol,
                #[cfg(feature = "mcp")]
                options.mcp,
                incoming,
                outgoing,
                flag,
            )
        })?;
    let result = if plain {
        plain_loop(&commands, &events, &cancelled)
    } else {
        screen_loop(&commands, &events, &cancelled, &interrupted)
    };
    cancelled.store(true, Ordering::Relaxed);
    drop(commands);
    drop(events);
    let joined = worker.join();
    result?;
    joined.map_err(|_| "model worker panicked")?;
    Ok(())
}

fn screen_loop(
    commands: &SyncSender<String>,
    events: &Receiver<Event>,
    cancelled: &AtomicBool,
    interrupted: &AtomicBool,
) -> io::Result<()> {
    let _restore = Restore;
    let mut terminal = ratatui::try_init()?;
    execute!(io::stdout(), EnableBracketedPaste)?;
    let mut app = App::default();
    let mut dirty = true;
    loop {
        if interrupted.swap(false, Ordering::Relaxed) && !app.busy {
            break;
        }
        for _ in 0..EVENTS_MAX {
            match events.try_recv() {
                Ok(event) => {
                    app.apply(event);
                    dirty = true;
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err(io::Error::other(
                        app.error.unwrap_or_else(|| "model worker stopped".into()),
                    ));
                }
            }
        }
        if dirty {
            terminal.draw(|frame| {
                app.bottom = view::draw(frame, &app);
            })?;
            dirty = false;
        }
        if event::poll(Duration::from_millis(if app.busy { 33 } else { 250 }))? {
            match event::read()? {
                InputEvent::Key(key) if key.kind != KeyEventKind::Release => {
                    if key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break;
                    }
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        if app.busy {
                            cancelled.store(true, Ordering::Relaxed);
                        } else {
                            break;
                        }
                    } else if !app.key(key.code, key.modifiers, commands, cancelled)? {
                        break;
                    }
                }
                InputEvent::Paste(text) if !app.busy && app.overlay.is_none() => app.paste(&text),
                InputEvent::Resize(_, _) => (),
                _ => continue,
            }
            dirty = true;
        }
    }
    Ok(())
}

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}

impl App {
    fn apply(&mut self, event: Event) {
        match event {
            Event::Continue => {
                self.sanitizer = term::Output::default();
                self.transcript.push(Message::assistant(""));
            }
            Event::Approval(text) => {
                self.overlay = Some(clean(&text));
                self.overlay_scroll = 0;
                self.approval = true;
                self.status = "1 approval pending · deny by default".into();
            }
            Event::Tool(text) => {
                self.transcript
                    .push(Message::tool(format!("┊ {}", clean(&text))));
            }
            Event::History(messages) => {
                self.transcript = messages
                    .into_iter()
                    .map(|mut m| {
                        m.content = clean(&m.content);
                        m
                    })
                    .collect();
                self.scroll = None;
            }
            Event::Ready(status) => {
                self.status = clean(&status);
                self.busy = false;
                if self.approval {
                    self.overlay = None;
                    self.approval = false;
                }
                self.submitted = None;
            }
            Event::Status(status) => self.status = clean(&status),
            Event::Error(error) => {
                if let Some(text) = self.submitted.take() {
                    self.input = text;
                }
                self.error = Some(clean(&error).chars().take(4096).collect());
            }
            Event::Overlay(text) => {
                self.overlay = Some(clean(&text));
                self.overlay_scroll = 0;
            }
            Event::Begin(user) => {
                self.submitted = None;
                self.error = None;
                self.sanitizer = term::Output::default();
                self.transcript.push(Message::user(clean(&user)));
                self.transcript.push(Message::assistant(""));
            }
            Event::Token(text) => {
                let mut bytes = Vec::new();
                let _ = self.sanitizer.write(&text, &mut bytes);
                if let Some(message) = self.transcript.last_mut() {
                    message.content.push_str(&String::from_utf8_lossy(&bytes));
                }
            }
        }
        while self.transcript.len() > 128
            || self
                .transcript
                .iter()
                .map(|m| m.content.len())
                .sum::<usize>()
                > DISPLAY_BYTES_MAX
        {
            if self.transcript.len() <= 1 {
                if let Some(message) = self.transcript.last_mut() {
                    let mut boundary = message.content.len().saturating_sub(DISPLAY_BYTES_MAX);
                    while !message.content.is_char_boundary(boundary) {
                        boundary += 1;
                    }
                    message.content.drain(..boundary);
                }
                break;
            }
            self.transcript.remove(0);
        }
    }

    fn paste(&mut self, text: &str) {
        let text = clean(text);
        if self.input.len() + text.len() <= COMPOSER_BYTES_MAX {
            self.input.push_str(&text);
        } else {
            self.error = Some("input exceeds 16384 bytes; paste was rejected".into());
        }
    }

    fn key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        commands: &SyncSender<String>,
        cancelled: &AtomicBool,
    ) -> io::Result<bool> {
        if self.overlay.is_some() {
            if self.approval
                && matches!(
                    code,
                    KeyCode::Char('y' | 'n') | KeyCode::Esc | KeyCode::Enter
                )
            {
                let decision = if code == KeyCode::Char('y') && modifiers.is_empty() {
                    "/approve"
                } else {
                    "/deny"
                };
                commands
                    .try_send(decision.into())
                    .map_err(io::Error::other)?;
                self.approval = false;
                self.overlay = None;
                return Ok(true);
            }
            match code {
                KeyCode::Esc | KeyCode::Enter => self.overlay = None,
                KeyCode::PageUp | KeyCode::Up => {
                    self.overlay_scroll = self.overlay_scroll.saturating_sub(5)
                }
                KeyCode::PageDown | KeyCode::Down => {
                    self.overlay_scroll = self.overlay_scroll.saturating_add(5)
                }
                _ => (),
            }
            return Ok(true);
        }
        if code == KeyCode::Char('k') && modifiers.contains(KeyModifiers::CONTROL) {
            self.overlay = Some(HELP.into());
            self.overlay_scroll = 0;
            return Ok(true);
        }
        match code {
            KeyCode::F(2) => {
                self.overlay = self.error.clone();
                self.overlay_scroll = 0;
            }
            KeyCode::Esc => {
                if self.busy {
                    cancelled.store(true, Ordering::Relaxed);
                    self.status = "stopping after the current native call".into();
                }
            }
            KeyCode::PageUp => {
                self.scroll = Some(self.scroll.unwrap_or(self.bottom).saturating_sub(10))
            }
            KeyCode::PageDown => {
                self.scroll = Some(self.scroll.unwrap_or(self.bottom).saturating_add(10))
            }
            KeyCode::End => self.scroll = None,
            KeyCode::Enter if !self.busy && !modifiers.contains(KeyModifiers::SHIFT) => {
                if self.input.trim() == "/quit" {
                    return Ok(false);
                }
                if !self.input.trim().is_empty() {
                    cancelled.store(false, Ordering::Relaxed);
                    self.submitted = Some(self.input.clone());
                    commands
                        .try_send(std::mem::take(&mut self.input))
                        .map_err(io::Error::other)?;
                    self.busy = true;
                    self.error = None;
                    self.status = "preparing request".into();
                }
            }
            KeyCode::Enter if !self.busy => self.paste("\n"),
            KeyCode::Backspace if !self.busy => {
                if let Some((index, _)) = self.input.grapheme_indices(true).next_back() {
                    self.input.truncate(index);
                }
            }
            KeyCode::Char(character)
                if !self.busy
                    && !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.paste(&character.to_string())
            }
            _ => (),
        }
        Ok(true)
    }
}

fn plain_loop(
    commands: &SyncSender<String>,
    events: &Receiver<Event>,
    cancelled: &AtomicBool,
) -> io::Result<()> {
    let mut input = io::stdin().lock();
    let mut output = io::stdout().lock();
    let mut sanitizer = term::Output::default();
    loop {
        loop {
            match events.recv().map_err(io::Error::other)? {
                Event::Continue => {
                    sanitizer = term::Output::default();
                    writeln!(output, "\n[assistant]")?;
                }
                Event::Approval(text) => {
                    writeln!(output, "\n[approve?]\n{}", clean(&text))?;
                    output.flush()?;
                    let mut decision = String::new();
                    (&mut input)
                        .take(COMPOSER_BYTES_MAX as u64 + 1)
                        .read_line(&mut decision)?;
                    commands
                        .send(
                            if decision.trim() == "y" {
                                "/approve"
                            } else {
                                "/deny"
                            }
                            .into(),
                        )
                        .map_err(io::Error::other)?;
                }
                Event::Tool(text) => writeln!(output, "\n┊ {}", clean(&text))?,
                Event::Ready(status) => {
                    writeln!(output, "[ready] {}", clean(&status))?;
                    break;
                }
                Event::Token(text) => sanitizer.write(&text, &mut output)?,
                Event::Begin(user) => {
                    sanitizer = term::Output::default();
                    writeln!(output, "[user]\n{}\n[assistant]", clean(&user))?;
                }
                Event::Error(text) => writeln!(output, "\nerror: {}", clean(&text))?,
                Event::Overlay(text) => writeln!(output, "{}", clean(&text))?,
                Event::History(messages) => {
                    for message in messages {
                        writeln!(
                            output,
                            "[{}]\n{}",
                            message.role.as_str(),
                            clean(&message.content)
                        )?;
                    }
                }
                Event::Status(_) => (),
            }
        }
        writeln!(output)?;
        output.flush()?;
        let mut line = String::new();
        if (&mut input)
            .take(COMPOSER_BYTES_MAX as u64 + 1)
            .read_line(&mut line)?
            == 0
        {
            return Ok(());
        }
        if line.len() > COMPOSER_BYTES_MAX {
            return Err(io::Error::other("input exceeds 16384 bytes"));
        }
        if line.trim() == "/quit" {
            return Ok(());
        }
        cancelled.store(false, Ordering::Relaxed);
        commands
            .send(line.trim_end_matches(['\r', '\n']).into())
            .map_err(io::Error::other)?;
    }
}

fn clean(text: &str) -> String {
    let mut bytes = Vec::new();
    let _ = term::Output::default().write(text, &mut bytes);
    String::from_utf8_lossy(&bytes).into_owned()
}

fn valid_session_id(id: &str) -> io::Result<()> {
    Selection::validate_id(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backspace_removes_one_combining_grapheme() {
        let (commands, _) = mpsc::sync_channel(1);
        let mut app = App {
            input: "one e\u{301}".into(),
            busy: false,
            ..App::default()
        };
        app.key(
            KeyCode::Backspace,
            KeyModifiers::NONE,
            &commands,
            &AtomicBool::new(false),
        )
        .expect("key");
        assert_eq!(app.input, "one ");
    }

    #[test]
    fn paste_never_submits_a_command() {
        let mut app = App {
            busy: false,
            ..App::default()
        };
        app.paste("/model file.gguf\n/quit\n");
        assert_eq!(app.input, "/model file.gguf\n/quit\n");
        assert!(!app.busy);
    }

    #[test]
    fn oversize_paste_preserves_the_composer() {
        let mut app = App {
            input: "keep".into(),
            ..App::default()
        };
        app.paste(&"x".repeat(COMPOSER_BYTES_MAX));
        assert_eq!(app.input, "keep");
        assert!(app.error.is_some());
    }

    #[test]
    fn stream_controls_are_removed_across_events() {
        let mut app = App::default();
        app.apply(Event::Begin("hi".into()));
        app.apply(Event::Token("visible\x1b]52;c;".into()));
        app.apply(Event::Token("secret\x07 answer".into()));
        assert_eq!(
            app.transcript.last().expect("reply").content,
            "visible answer"
        );
    }

    #[test]
    fn scrolling_history_stays_anchored_during_streaming() {
        let mut app = App {
            bottom: 80,
            ..App::default()
        };
        let (commands, _) = mpsc::sync_channel(1);
        app.key(
            KeyCode::PageUp,
            KeyModifiers::NONE,
            &commands,
            &AtomicBool::new(false),
        )
        .expect("scroll");
        app.apply(Event::Begin("hi".into()));
        app.apply(Event::Token("reply\n".repeat(20)));
        assert_eq!(app.scroll, Some(70));
    }

    #[test]
    fn rejected_request_restores_the_unsent_composer() {
        let mut app = App {
            submitted: Some("keep this question".into()),
            ..App::default()
        };
        app.apply(Event::Error("context full".into()));
        app.apply(Event::Ready("idle".into()));
        assert_eq!(app.input, "keep this question");
        assert!(app.transcript.is_empty());
    }

    #[test]
    fn escape_cancels_without_queueing_a_second_request() {
        let mut app = App::default();
        let (commands, incoming) = mpsc::sync_channel(1);
        let cancelled = AtomicBool::new(false);
        app.key(KeyCode::Esc, KeyModifiers::NONE, &commands, &cancelled)
            .expect("stop");
        assert!(cancelled.load(Ordering::Relaxed));
        assert!(incoming.try_recv().is_err());
    }
}

#[cfg(test)]
mod approval_tests {
    use super::*;

    #[test]
    fn enter_denies_an_approval() {
        let (commands, incoming) = mpsc::sync_channel(1);
        let mut app = App::default();
        app.apply(Event::Approval("exact action".into()));
        app.key(
            KeyCode::Enter,
            KeyModifiers::NONE,
            &commands,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(incoming.try_recv().unwrap(), "/deny");
    }

    #[test]
    fn paste_cannot_approve_a_tool() {
        let (commands, incoming) = mpsc::sync_channel(1);
        let mut app = App::default();
        app.apply(Event::Approval("exact action".into()));
        app.paste("y\n");
        assert!(incoming.try_recv().is_err());
        app.key(
            KeyCode::Char('y'),
            KeyModifiers::NONE,
            &commands,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(incoming.try_recv().unwrap(), "/approve");
        app.key(
            KeyCode::Char('y'),
            KeyModifiers::NONE,
            &commands,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(incoming.try_recv().is_err());
    }

    #[test]
    fn timeout_dismisses_pending_approval() {
        let mut app = App::default();
        app.apply(Event::Approval("exact action".into()));
        app.apply(Event::Ready("timed out".into()));
        assert!(!app.approval);
        assert!(app.overlay.is_none());
    }
}
