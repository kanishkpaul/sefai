use super::{Event, TEXT_BYTES_MAX};
use crate::{
    runner,
    workspace::{Prepared, Runner, Selection, Workspace},
};
use sefai_core::{Message, Prompt};
use sefai_engine_llama::{Cache, Device, Engine, Model, Settings, Stop};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender},
    },
    time::{SystemTime, UNIX_EPOCH},
};

struct Job {
    prepared: Prepared,
    path: PathBuf,
    device: Device,
    settings: Settings,
    user: String,
}

struct State {
    selection: Selection,
    model: Option<PathBuf>,
    session: String,
    revision: Option<String>,
    history: Vec<Message>,
    statistics: String,
    unsaved: bool,
    context_used: usize,
}

pub fn run(
    selection: Selection,
    model: Option<PathBuf>,
    commands: Receiver<String>,
    events: SyncSender<Event>,
    cancelled: Arc<AtomicBool>,
) {
    let result = work(selection, model, commands, &events, cancelled);
    if let Err(error) = result {
        let _ = events.send(Event::Error(error.to_string()));
    }
}

fn work(
    selection: Selection,
    model: Option<PathBuf>,
    commands: Receiver<String>,
    events: &SyncSender<Event>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let root = selection.root()?;
    if !root.exists() {
        Workspace::init(&root)?;
    }
    let mut state = State {
        session: fresh_id(),
        selection,
        model,
        revision: None,
        history: Vec::new(),
        unsaved: false,
        context_used: 0,
        statistics: "no generation yet; one model and context at most".into(),
    };
    if let Some(id) = state.selection.session.take() {
        state.resume(&id)?;
    }
    events.send(Event::History(state.history.clone()))?;
    events.send(Event::Ready(format!(
        "session {} · /model PATH or /profile ID · /help",
        state.session
    )))?;
    let mut engine = Engine::new()?;
    let mut pending = None;
    loop {
        let job = match pending.take().or_else(|| state.next(&commands, events)) {
            Some(job) => job,
            None => return Ok(()),
        };
        events.send(Event::Status(format!(
            "loading {} · {:?}",
            job.path.display(),
            job.device
        )))?;
        let model = match engine.load(&job.path, job.device, cancelled.clone()) {
            Ok(model) => model,
            Err(error) => {
                events.send(Event::Error(error.to_string()))?;
                events.send(Event::Ready("choose a model or retry; /help".into()))?;
                continue;
            }
        };
        let mut job = job;
        let mut cache = Cache::default();
        let mut cache_session = state.session.clone();
        loop {
            if let Err(error) = state.generate(&model, &mut cache, &job, events, &cancelled) {
                events.send(Event::Error(error.to_string()))?;
            }
            let label = job
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(20)
                .collect::<String>();
            events.send(Event::Ready(format!(
                "{} · {:?} · ctx {}/{} · local · {} records{}",
                label,
                model.device(),
                state.context_used,
                job.settings.context_tokens,
                state.history.len(),
                if cancelled.load(Ordering::Relaxed) {
                    " · cancelled"
                } else {
                    ""
                }
            )))?;
            let Some(next) = state.next(&commands, events) else {
                return Ok(());
            };
            if next.path != job.path || next.device != job.device {
                pending = Some(next);
                break;
            }
            if cache_session != state.session {
                cache.clear();
                cache_session.clone_from(&state.session);
            }
            job = next;
        }
        // The old weights and context are dropped before loading the next model.
    }
}

impl State {
    fn resume(&mut self, id: &str) -> io::Result<()> {
        let workspace = Workspace::open(&self.selection.root()?)?;
        let revision = workspace.conversation_revision(id)?;
        let history = workspace.session(id, 32)?;
        self.session = id.into();
        self.revision = Some(revision);
        self.history = history;
        self.unsaved = false;
        Ok(())
    }

    fn next(&mut self, commands: &Receiver<String>, events: &SyncSender<Event>) -> Option<Job> {
        while let Ok(text) = commands.recv() {
            match self.prepare(text, events) {
                Ok(Some(job)) => return Some(job),
                Ok(None) => (),
                Err(error) => {
                    events.send(Event::Error(error.to_string())).ok()?;
                }
            }
            events
                .send(Event::Ready(format!(
                    "selected {} · session {} · /help",
                    self.model
                        .as_ref()
                        .map(|path| path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned())
                        .or_else(|| self.selection.profile.clone())
                        .unwrap_or_else(|| "workspace profile".into()),
                    self.session
                )))
                .ok()?;
        }
        None
    }

    fn prepare(
        &mut self,
        text: String,
        events: &SyncSender<Event>,
    ) -> Result<Option<Job>, Box<dyn std::error::Error>> {
        if let Some(command) = text.strip_prefix('/') {
            let (name, argument) = command.split_once(' ').unwrap_or((command, ""));
            let argument = argument.trim();
            let workspace = Workspace::open(&self.selection.root()?)?;
            match name {
                "help" => {
                    events.send(Event::Overlay(super::HELP.into()))?;
                }
                "memory" if !argument.is_empty() => {
                    Selection::validate_id(argument)?;
                    let path = format!("memory/{argument}.md");
                    let text = workspace.read(std::path::Path::new(&path))?;
                    events.send(Event::Overlay(format!(
                        "{path}\n{}",
                        text.chars().take(8192).collect::<String>()
                    )))?;
                }
                "models" | "skills" | "memory" | "mcp" | "sessions" => {
                    let mut lines = vec![format!("{name} · Markdown files")];
                    for path in workspace.entries(name)?.into_iter().take(128) {
                        lines.push(path.display().to_string());
                    }
                    if name == "mcp" {
                        lines.push("metadata only; connections arrive in Phase 5".into());
                    }
                    events.send(Event::Overlay(lines.join("\n")))?;
                }
                "model" if !argument.is_empty() => {
                    self.model = Some(argument.into());
                }
                "profile" if !argument.is_empty() => {
                    let mut candidate = self.selection.clone();
                    candidate.profile = Some(argument.into());
                    candidate.prepare("validate selection".into())?;
                    self.selection = candidate;
                    self.model = None;
                }
                "skill" if !argument.is_empty() => {
                    let mut candidate = self.selection.clone();
                    if candidate.skill.iter().any(|id| id == argument) {
                        candidate.skill.retain(|id| id != argument);
                    } else {
                        candidate.skill.push(argument.into());
                    }
                    candidate.prepare("validate selection".into())?;
                    self.selection = candidate;
                }
                "recall" => {
                    self.selection.recall = (!argument.is_empty()).then(|| argument.into());
                }
                "settings" => {
                    events.send(Event::Overlay(format!("workspace {}\nprofile {:?}\nskills {:?}\nrecall {:?}\nmodel {:?}\nedit config.md and profiles with your text editor; changes reload before each turn", self.selection.root()?.display(), self.selection.profile, self.selection.skill, self.selection.recall, self.model)))?;
                }
                "stats" => {
                    events.send(Event::Overlay(self.statistics.clone()))?;
                }
                "resume" if !argument.is_empty() => {
                    self.resume(argument)?;
                    events.send(Event::History(self.history.clone()))?;
                }
                "new" | "reset" => {
                    let id = if argument.is_empty() {
                        fresh_id()
                    } else {
                        argument.into()
                    };
                    // Create-only validation prevents accidentally overwriting a saved session.
                    super::valid_session_id(&id)?;
                    if self
                        .selection
                        .root()?
                        .join(format!("sessions/{id}.md"))
                        .exists()
                    {
                        return Err("session exists; use /resume ID".into());
                    }
                    self.session = id;
                    self.revision = None;
                    self.history.clear();
                    self.unsaved = false;
                    events.send(Event::History(Vec::new()))?;
                }
                _ => return Err("unknown command or missing argument; use /help".into()),
            }
            return Ok(None);
        }
        if self.unsaved {
            return Err(
                "session has unsaved records; resolve the error and /resume or /new before sending"
                    .into(),
            );
        }
        if text.trim().is_empty() {
            return Ok(None);
        }
        if self.history.len() > 125 {
            return Err(
                "conversation has 126 records; use /new to preserve it and start again".into(),
            );
        }
        let mut prepared = self.selection.prepare(text.clone())?;
        let bytes = self
            .history
            .iter()
            .map(|message| message.content.len())
            .sum::<usize>()
            + prepared
                .prompt
                .conversation()
                .iter()
                .map(|message| message.content.len())
                .sum::<usize>()
            + prepared
                .prompt
                .sections()
                .iter()
                .map(|section| section.text.len())
                .sum::<usize>();
        if bytes > TEXT_BYTES_MAX {
            return Err("conversation exceeds 1048576 bytes; use /new".into());
        }
        let mut history = self.history.clone();
        history.push(Message::user(&text));
        let mut prompt = Prompt::new(history);
        for section in prepared.prompt.sections() {
            prompt.add_section(section.source, &section.text);
        }
        prepared.prompt = prompt;
        let path = self
            .model
            .clone()
            .or_else(|| prepared.model.clone())
            .ok_or("choose /model PATH or /profile ID before sending")?;
        let (device, settings) = runner::resolve(Runner::default(), prepared.runner.clone())?;
        Ok(Some(Job {
            prepared,
            path,
            device,
            settings,
            user: text,
        }))
    }

    fn generate<'model>(
        &mut self,
        model: &'model Model<'_>,
        cache: &mut Cache<'model>,
        job: &Job,
        events: &SyncSender<Event>,
        cancelled: &AtomicBool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let input = model.prepare(&job.prepared.prompt)?;
        job.settings.validate(input.token_count(), model.context_tokens_max())
            .map_err(|error| format!("{error}; history was preserved; use /new or edit the profile context/output limits"))?;
        self.context_used = input.token_count();
        events.send(Event::Begin(job.user.clone()))?;
        events.send(Event::Status(format!(
            "prefill · {:?} · ctx {}/{} · local",
            model.device(),
            input.token_count(),
            job.settings.context_tokens
        )))?;
        let answer_budget = TEXT_BYTES_MAX.saturating_sub(
            self.history.iter().map(|m| m.content.len()).sum::<usize>() + job.user.len(),
        );
        let mut answer = String::new();
        let result = model.generate_cached(
            cache,
            &input,
            job.settings,
            || cancelled.load(Ordering::Relaxed),
            |text| {
                if answer.len() + text.len() > answer_budget {
                    return Err(io::Error::other(
                        "reply exceeds remaining conversation byte budget; use /new",
                    ));
                }
                answer.push_str(text);
                events
                    .send(Event::Token(text.into()))
                    .map_err(io::Error::other)
            },
        );
        let interrupted = result
            .as_ref()
            .ok()
            .is_none_or(|statistics| statistics.stop == Stop::Cancelled);
        self.history.push(Message::user(&job.user));
        self.history.push(Message::assistant(&answer));
        self.unsaved = true;
        let workspace = Workspace::open(&self.selection.root()?)?;
        match workspace.save_turn(
            &self.session,
            self.revision.as_deref(),
            &job.user,
            &answer,
            interrupted,
        ) {
            Ok(revision) => {
                self.revision = Some(revision);
                self.unsaved = false;
            }
            Err(error) => {
                self.unsaved = true;
                events.send(Event::Error(format!("saving session: {error}; conversation remains in memory; resolve the file before sending again")))?;
            }
        }
        let statistics = result?;
        self.context_used = statistics.prompt_tokens + statistics.generated_tokens as usize;
        self.statistics = format!(
            "prompt {} tokens\ngenerated {} tokens\nprefill {:.3}s\ngeneration {:.3}s\nstop {:?}\nverified prefix reused {} tokens; weights retained",
            statistics.prompt_tokens,
            statistics.generated_tokens,
            statistics.prefill.as_secs_f64(),
            statistics.generation.as_secs_f64(),
            statistics.stop,
            cache.reused_tokens()
        );
        if interrupted {
            events.send(Event::Status(
                "generation cancelled; partial reply saved if no file conflict".into(),
            ))?;
        }
        Ok(())
    }
}

fn fresh_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("session-{nanos}-{}", std::process::id())
}
