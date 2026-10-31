use super::{Event, TEXT_BYTES_MAX, ToolProtocol};
use crate::engine::{Cache, Device, Engine, Model, Settings, Stop};
use crate::harness::{self, Action, Redactor, Turn};
use crate::{
    runner,
    workspace::{Format, Prepared, Runner, Selection, Workspace},
};
use sefai_core::{Message, Prompt};
use std::{
    io,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, SyncSender},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

// A new turn must fit its records within the 128-message resume tail; tool turns also reserve intermediate records.
const HISTORY_MESSAGES_MAX: usize = 125;
const TOOL_HISTORY_MESSAGES_MAX: usize = 110;
const MEMORY_PREVIEW_CHARS_MAX: usize = 8192;
const OVERLAY_ENTRIES_MAX: usize = 128;

struct Dispatch<'a> {
    job: &'a Job,
    base: Option<&'a str>,
    events: &'a SyncSender<Event>,
    cancelled: &'a AtomicBool,
    commands: &'a Receiver<String>,
    started: Instant,
}

struct Job {
    prepared: Prepared,
    path: PathBuf,
    format: Option<Format>,
    device: Device,
    settings: Settings,
    user: String,
    scope: Scope,
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
    tools: Option<ToolProtocol>,
    redactor: Redactor,
    uncertain: bool,
    #[cfg(feature = "mcp")]
    mcp: Option<crate::mcp::Connection>,
}

pub fn run(
    selection: Selection,
    model: Option<PathBuf>,
    tools: Option<ToolProtocol>,
    #[cfg(feature = "mcp")] mcp: Option<String>,
    commands: Receiver<String>,
    events: SyncSender<Event>,
    cancelled: Arc<AtomicBool>,
) {
    let result = work(
        selection,
        model,
        tools,
        #[cfg(feature = "mcp")]
        mcp,
        commands,
        &events,
        cancelled,
    );
    if let Err(error) = result {
        let _ = events.send(Event::Error(error.to_string()));
    }
}

fn work(
    selection: Selection,
    model: Option<PathBuf>,
    tools: Option<ToolProtocol>,
    #[cfg(feature = "mcp")] mcp: Option<String>,
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
        tools,
        redactor: Redactor::environment()?,
        uncertain: false,
        #[cfg(feature = "mcp")]
        mcp: None,
        statistics: "no generation yet; one model and context at most".into(),
    };
    #[cfg(feature = "mcp")]
    if let Some(id) = mcp {
        let definition = crate::mcp::Definition::load(&root, &id)?;
        state.redactor.capture(definition.credential_values()?)?;
        events.send(Event::Approval(definition.preview()))?;
        if approval(&commands, Instant::now(), &cancelled)? {
            let connected = crate::mcp::Connection::connect(definition, &cancelled, |text| {
                events
                    .send(Event::Status("authorizing MCP · external access".into()))
                    .map_err(io::Error::other)?;
                events
                    .send(Event::Overlay(text.into()))
                    .map_err(io::Error::other)
            });
            match connected {
                Ok(connection) => {
                    state.redactor.capture(connection.credential_values())?;
                    state.mcp = Some(connection);
                }
                Err(error) => {
                    events.send(Event::Error(format!(
                        "mcp connection failed: {error}; no server connected"
                    )))?;
                }
            }
        } else {
            events.send(Event::Error(
                "mcp connection denied; no server started".into(),
            ))?;
        }
    }
    if let Some(id) = state.selection.session.take() {
        state.resume(&id)?;
    }
    events.send(Event::History(state.history.clone()))?;
    if state.uncertain {
        events.send(Event::Error(
            "prior tool dispatch has an unknown outcome; inspect the session; no call was replayed"
                .into(),
        ))?;
    }
    events.send(Event::Ready(format!(
        "session {} · {} · /model PATH or /profile ID · /help",
        state.session,
        state.locality()
    )))?;
    let mut engine = Engine::new()?;
    let mut pending = None;
    loop {
        let job = match pending.take().or_else(|| state.next(&commands, events)) {
            Some(job) => job,
            None => return Ok(()),
        };
        events.send(Event::Status(format!(
            "loading {} · {}",
            job.path.display(),
            job.device
        )))?;
        let model = match engine.load(&job.path, job.format, job.device, cancelled.clone()) {
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
            if let Err(error) =
                state.generate(&model, &mut cache, &job, events, &cancelled, &commands)
            {
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
                "{} · {} · ctx {}/{} · {} · {} records{}",
                label,
                model.device(),
                state.context_used,
                job.settings.context_tokens,
                state.locality(),
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
            if next.path != job.path || next.format != job.format || next.device != job.device {
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
    fn locality(&self) -> &'static str {
        #[cfg(feature = "mcp")]
        if self.mcp.is_some() {
            return "local model · MCP external access";
        }
        "local"
    }

    fn resume(&mut self, id: &str) -> io::Result<()> {
        let workspace = Workspace::open(&self.selection.root()?)?;
        let revision = workspace.conversation_revision(id)?;
        let history = workspace.session(id, 32)?;
        self.uncertain = harness::uncertain_dispatch(&revision);
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
            self.run_command(command, events)?;
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
        let history_messages_max = if self.tools.is_some() {
            TOOL_HISTORY_MESSAGES_MAX
        } else {
            HISTORY_MESSAGES_MAX
        };
        if self.history.len() > history_messages_max {
            return Err(
                "conversation needs room for a bounded tool turn; use /new to preserve it and start again".into(),
            );
        }
        let scope = if self.tools.is_some() {
            Scope::freeze(&self.selection.root()?)?
        } else {
            Scope {
                root: self.selection.root()?.canonicalize()?,
                memories: Vec::new(),
            }
        };
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
            return Err(format!("conversation exceeds {TEXT_BYTES_MAX} bytes; use /new").into());
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
        // A profile's declared format describes its own path, not one chosen with /model.
        let format = prepared.format.filter(|_| self.model.is_none());
        let plan = runner::resolve(Runner::default(), prepared.runner.clone(), format)?;
        Ok(Some(Job {
            prepared,
            path,
            format: plan.format,
            device: plan.device,
            settings: plan.settings,
            user: text,
            scope,
        }))
    }

    fn run_command(
        &mut self,
        command: &str,
        events: &SyncSender<Event>,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
                    text.chars()
                        .take(MEMORY_PREVIEW_CHARS_MAX)
                        .collect::<String>()
                )))?;
            }
            "models" | "skills" | "memory" | "mcp" | "sessions" => {
                let mut lines = vec![format!("{name} · Markdown files")];
                for path in workspace
                    .entries(name)?
                    .into_iter()
                    .take(OVERLAY_ENTRIES_MAX)
                {
                    lines.push(path.display().to_string());
                }
                if name == "mcp" {
                    #[cfg(feature = "mcp")]
                    lines.push(self.mcp.as_ref().map_or_else(
                        || {
                            "no connection; restart with chat --mcp ID --tool-protocol smollm2"
                                .into()
                        },
                        |connection| {
                            format!(
                                "connected {} · {}",
                                connection.id(),
                                connection.destination()
                            )
                        },
                    ));
                    #[cfg(not(feature = "mcp"))]
                    lines.push("connections require the optional mcp feature".into());
                }
                events.send(Event::Overlay(lines.join("\n")))?;
            }
            #[cfg(feature = "mcp")]
            "disconnect" => {
                self.mcp = None;
                events.send(Event::Overlay(
                    "mcp disconnected; child process tree stopped".into(),
                ))?;
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
                if self.uncertain {
                    events.send(Event::Error("prior tool dispatch has an unknown outcome; inspect the session; no call was replayed".into()))?;
                }
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
        Ok(())
    }

    fn generate<'model>(
        &mut self,
        model: &'model Model<'_>,
        cache: &mut Cache<'model>,
        job: &Job,
        events: &SyncSender<Event>,
        cancelled: &AtomicBool,
        commands: &Receiver<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let started = Instant::now();
        let base = self.revision.clone();
        let mut turn = Turn::new(&job.prepared.prompt, self.tools.is_some());
        #[cfg(feature = "mcp")]
        if let Some(connection) = &self.mcp {
            connection.validate()?;
            turn.mcp_catalog(connection.id(), connection.tools())?;
        }
        let first = model.prepare(&turn.prompt(&job.prepared.prompt))?;
        Settings {
            output_tokens: job.settings.output_tokens.min(harness::TOKENS_MAX),
            ..job.settings
        }
        .validate(first.token_count(), model.context_tokens_max())
        .map_err(|error| {
            format!(
                "{error}; history was preserved; use /new or edit the profile context/output limits"
            )
        })?;
        let mut first = Some(first);
        events.send(Event::Begin(job.user.clone()))?;
        let mut context_used = self.context_used;
        let mut statistics_text =
            "generation did not finish; native step statistics unavailable".to_owned();
        let tools_enabled = self.tools.is_some();
        let mut continued = false;
        let locality = self.locality();
        let result = turn.drive(
            &job.prepared.prompt,
            || (started.elapsed(), cancelled.load(Ordering::Relaxed)),
            |prompt, remaining, answer_budget| {
                let input = first.take().map_or_else(|| model.prepare(prompt), Ok).map_err(|error| io::Error::other(error.to_string()))?;
                let settings = Settings {
                    output_tokens: job.settings.output_tokens.min(remaining),
                    ..job.settings
                };
                settings.validate(input.token_count(), model.context_tokens_max()).map_err(io::Error::other)?;
                events.send(Event::Status(format!(
                    "prefill · {} · ctx {}/{} · {}",
                    model.device(),
                    input.token_count(),
                    settings.context_tokens,
                    locality
                ))).map_err(io::Error::other)?;
                if continued { events.send(Event::Continue).map_err(io::Error::other)?; }
                continued = true;
                context_used = input.token_count();
                let mut answer = String::new();
                let generated = model.generate_cached(
                    cache,
                    &input,
                    settings,
                    || {
                        cancelled.load(Ordering::Relaxed)
                            || started.elapsed() >= harness::TURN_TIME_MAX
                    },
                    |text| {
                        if answer.len() + text.len() > answer_budget {
                            return Err(harness::fail("reply byte budget exceeded"));
                        }
                        answer.push_str(text);
                        events
                            .send(Event::Token(text.into()))
                            .map_err(io::Error::other)
                    },
                );
                let statistics = match generated {
                    Ok(stats) => stats,
                    Err(error) => {
                        return Ok(harness::Reply { text: answer, tokens: 0, complete: false, error: Some(error.to_string()) });
                    }
                };
                context_used = statistics.prompt_tokens + statistics.generated_tokens as usize;
                statistics_text = format!(
                    "prompt {} tokens\ngenerated {} tokens\nprefill {:.3}s\ngeneration {:.3}s\nstop {:?}\nverified prefix reused {} tokens; weights retained\ntools: {} · context max {} · cancellation between native calls\n{}",
                    statistics.prompt_tokens,
                    statistics.generated_tokens,
                    statistics.prefill.as_secs_f64(),
                    statistics.generation.as_secs_f64(),
                    statistics.stop,
                    model.reused_tokens(cache),
                    if tools_enabled {
                        "explicit SmolLM2 protocol"
                    } else {
                        "disabled"
                    },
                    model.context_tokens_max(),
                    model.memory()
                );
                let complete = statistics.stop != Stop::Cancelled
                    && (statistics.stop == Stop::EndOfGeneration
                        || !answer.contains("<tool_call>"));
                Ok(harness::Reply { text: answer, tokens: statistics.generated_tokens, complete, error: None })
            },
            |turn, action| self.dispatch(turn, action, &Dispatch { job, base: base.as_deref(), events, cancelled, commands, started }),
        );
        self.context_used = context_used;
        self.statistics = format!(
            "last generation step\n{statistics_text}\n{}",
            turn.summary()
        );
        if let Err(error) = &result {
            turn.outcome = "interrupted";
            turn.audit.push(
                serde_json::json!({"outcome":"interrupted","error":error.to_string()}).to_string(),
            );
            cache.clear();
        }
        self.history.push(Message::user(&job.user));
        self.history.extend(turn.messages.clone());
        self.unsaved = true;
        if let Err(error) = self.checkpoint(job, base.as_deref(), &turn) {
            events.send(Event::Error(format!("saving session: {error}; conversation remains in memory; resolve the file before sending again")))?;
        }
        result.map_err(Into::into)
    }

    fn dispatch(
        &mut self,
        turn: &mut Turn,
        action: &Action,
        dispatch: &Dispatch<'_>,
    ) -> io::Result<()> {
        #[cfg(feature = "mcp")]
        if action.name == "mcp_call" {
            return self.dispatch_mcp(turn, action, dispatch);
        }
        self.redactor.check_write(action)?;
        dispatch.job.scope.validate(action)?;
        dispatch
            .events
            .send(Event::Approval(action.preview(&dispatch.job.scope.root)))
            .map_err(io::Error::other)?;
        let approved = approval(dispatch.commands, dispatch.started, dispatch.cancelled)?;
        dispatch.job.scope.validate(action)?;
        if !turn.authorize(
            action,
            approved,
            dispatch.started.elapsed(),
            dispatch.cancelled.load(Ordering::Relaxed),
        )? {
            dispatch
                .events
                .send(Event::Tool(action.row("denied")))
                .map_err(io::Error::other)?;
            return Ok(());
        }
        // A durable dispatch intent precedes every side effect, including a crash.
        self.checkpoint(dispatch.job, dispatch.base, turn)?;
        turn.check(
            dispatch.started.elapsed(),
            dispatch.cancelled.load(Ordering::Relaxed),
        )?;
        dispatch.job.scope.validate(action)?;
        let executed = dispatch.job.scope.execute(action);
        let (outcome, text) = match executed {
            Ok(text) => ("completed", text),
            Err(error) => (
                if action.name == "remember_memory" {
                    "unknown"
                } else {
                    "failed"
                },
                error.to_string(),
            ),
        };
        let outcome =
            if dispatch.cancelled.load(Ordering::Relaxed) && action.name == "remember_memory" {
                "unknown"
            } else {
                outcome
            };
        turn.result(action, outcome, &text)?;
        self.checkpoint(dispatch.job, dispatch.base, turn)?;
        dispatch
            .events
            .send(Event::Tool(action.row(outcome)))
            .map_err(io::Error::other)?;

        Ok(())
    }

    #[cfg(feature = "mcp")]
    fn dispatch_mcp(
        &mut self,
        turn: &mut Turn,
        action: &Action,
        dispatch: &Dispatch<'_>,
    ) -> io::Result<()> {
        let connection = self
            .mcp
            .as_ref()
            .ok_or_else(|| harness::fail("no explicitly connected mcp server"))?;
        let call = action
            .arguments
            .mcp
            .as_ref()
            .ok_or_else(|| harness::fail("mcp call arguments missing"))?;
        if connection.id() != action.arguments.id || !connection.has_tool(&call.tool) {
            return Err(harness::fail(
                "mcp call is outside the frozen connection catalog",
            ));
        }
        connection.validate()?;
        let arguments = serde_json::Value::Object(call.arguments.clone())
            .to_string()
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    c.to_string()
                } else {
                    c.escape_unicode().to_string()
                }
            })
            .collect::<String>();
        dispatch.events.send(Event::Approval(format!("server: {}\ntool: {:?}\ndestination: {}\narguments (Unicode escaped): {arguments}\nmay read or modify external state; server annotations grant no authority\n[y] approve once · [n/Esc/Enter] deny", connection.id(), call.tool, connection.destination()))).map_err(io::Error::other)?;
        let approved = approval(dispatch.commands, dispatch.started, dispatch.cancelled)?;
        connection.validate()?;
        if !turn.authorize(
            action,
            approved,
            dispatch.started.elapsed(),
            dispatch.cancelled.load(Ordering::Relaxed),
        )? {
            dispatch
                .events
                .send(Event::Tool(action.row("denied")))
                .map_err(io::Error::other)?;
            return Ok(());
        }
        self.checkpoint(dispatch.job, dispatch.base, turn)?;
        turn.check(
            dispatch.started.elapsed(),
            dispatch.cancelled.load(Ordering::Relaxed),
        )?;
        let connection = self
            .mcp
            .as_ref()
            .ok_or_else(|| harness::fail("mcp disconnected"))?;
        connection.validate()?;
        let result = connection.tool(
            &call.tool,
            serde_json::Value::Object(call.arguments.clone()),
            dispatch.cancelled,
            harness::TURN_TIME_MAX.saturating_sub(dispatch.started.elapsed()),
        );
        let (outcome, text) = match result {
            Ok(text) => {
                let failed = serde_json::from_str::<serde_json::Value>(&text)
                    .is_ok_and(|value| value["isError"] == true);
                (if failed { "failed" } else { "completed" }, text)
            }
            Err(error) => {
                self.mcp = None;
                ("unknown", error.to_string())
            }
        };
        let outcome = if dispatch.cancelled.load(Ordering::Relaxed)
            || dispatch.started.elapsed() >= harness::TURN_TIME_MAX
        {
            "unknown"
        } else {
            outcome
        };
        turn.result(action, outcome, &text)?;
        self.checkpoint(dispatch.job, dispatch.base, turn)?;
        dispatch
            .events
            .send(Event::Tool(action.row(outcome)))
            .map_err(io::Error::other)?;
        Ok(())
    }

    fn checkpoint(&mut self, job: &Job, base: Option<&str>, turn: &Turn) -> io::Result<()> {
        let workspace = Workspace::open(&job.scope.root)?;
        let mut records = vec![("user", self.redactor.text(&job.user))];
        records.extend(
            turn.messages
                .iter()
                .map(|message| (message.role.as_str(), self.redactor.text(&message.content))),
        );
        records.extend(
            turn.audit
                .iter()
                .map(|text| ("harness", self.redactor.text(text))),
        );
        let borrowed = records
            .iter()
            .map(|(role, text)| (*role, text.as_str()))
            .collect::<Vec<_>>();
        self.revision = Some(workspace.save_checkpoint(
            &self.session,
            base,
            self.revision.as_deref(),
            &borrowed,
            turn.outcome,
        )?);
        self.unsaved = false;
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

struct Scope {
    root: PathBuf,
    memories: Vec<String>,
}
impl Scope {
    fn freeze(root: &std::path::Path) -> io::Result<Self> {
        let workspace = Workspace::open(root)?;
        let entries = workspace.entries("memory")?;
        if entries.len() > 128 {
            return Err(harness::fail("memory tool scope exceeds 128 files"));
        }
        Ok(Self {
            root: root.canonicalize()?,
            memories: entries
                .iter()
                .filter_map(|p| p.file_stem()?.to_str().map(str::to_owned))
                .collect(),
        })
    }
    fn validate(&self, action: &Action) -> io::Result<()> {
        action.validate()?;
        if self.root.canonicalize()? != self.root {
            return Err(harness::fail("workspace identity changed since approval"));
        }
        if action.name == "read_memory" && !self.memories.contains(&action.arguments.id) {
            return Err(harness::fail("memory is outside the frozen read scope"));
        }
        Ok(())
    }
    fn execute(&self, action: &Action) -> io::Result<String> {
        self.validate(action)?;
        let workspace = Workspace::open(&self.root)?;
        if action.name == "remember_memory" {
            workspace.remember(
                &action.arguments.id,
                action
                    .arguments
                    .text
                    .as_deref()
                    .ok_or_else(|| harness::fail("memory text missing"))?,
                None,
            )?;
            Ok(format!("created memory/{}.md", action.arguments.id))
        } else {
            let relative = PathBuf::from(format!("memory/{}.md", action.arguments.id));
            let text = workspace.read(&relative)?;
            workspace.validate_document(&relative, &text)?;
            if text.len() > harness::RESULT_BYTES_MAX {
                return Err(harness::fail("memory result exceeds 16384 bytes"));
            }
            Ok(text)
        }
    }
}

fn approval(
    commands: &Receiver<String>,
    started: Instant,
    cancelled: &AtomicBool,
) -> io::Result<bool> {
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(harness::fail("approval cancelled"));
        }
        if started.elapsed() >= harness::TURN_TIME_MAX {
            return Err(harness::fail("approval timed out; denied"));
        }
        match commands.recv_timeout(Duration::from_millis(50)) {
            Ok(text) => return Ok(text == "/approve"),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => return Err(harness::fail("approval input closed; denied")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::Arguments;
    use std::{fs, sync::atomic::AtomicUsize};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "sefai-scope-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Workspace::init(&path).unwrap();
            Self(path)
        }
        fn memory(&self) {
            Workspace::open(&self.0)
                .unwrap()
                .remember("note", "original", None)
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn read() -> Action {
        Action {
            name: "read_memory".into(),
            arguments: Arguments {
                #[cfg(feature = "mcp")]
                mcp: None,
                id: "note".into(),
                text: None,
            },
        }
    }

    #[test]
    fn frozen_scope_does_not_adopt_a_new_memory() {
        let fixture = Fixture::new();
        let scope = Scope::freeze(&fixture.0).unwrap();
        fixture.memory();
        assert!(scope.execute(&read()).is_err());
    }

    #[test]
    fn write_never_replaces_existing_memory() {
        let fixture = Fixture::new();
        fixture.memory();
        let scope = Scope::freeze(&fixture.0).unwrap();
        let mut action = read();
        action.name = "remember_memory".into();
        action.arguments.text = Some("replacement".into());
        assert!(scope.execute(&action).is_err());
        assert!(
            fs::read_to_string(fixture.0.join("memory/note.md"))
                .unwrap()
                .ends_with("original")
        );
    }

    #[test]
    fn malformed_memory_is_a_tool_error() {
        let fixture = Fixture::new();
        fixture.memory();
        let scope = Scope::freeze(&fixture.0).unwrap();
        fs::write(fixture.0.join("memory/note.md"), "not schema one").unwrap();
        assert!(scope.execute(&read()).is_err());
    }

    #[test]
    fn oversized_memory_is_not_returned_to_the_model() {
        let fixture = Fixture::new();
        Workspace::open(&fixture.0)
            .unwrap()
            .remember("note", &"x".repeat(harness::RESULT_BYTES_MAX), None)
            .unwrap();
        let scope = Scope::freeze(&fixture.0).unwrap();
        assert!(scope.execute(&read()).is_err());
    }

    #[test]
    fn expired_approval_denies_without_waiting() {
        let (_sender, incoming) = std::sync::mpsc::sync_channel(1);
        assert!(
            approval(
                &incoming,
                Instant::now() - harness::TURN_TIME_MAX,
                &AtomicBool::new(false)
            )
            .is_err()
        );
    }
    fn state_and_job(fixture: &Fixture) -> (State, Job) {
        use clap::Parser;
        let selection = crate::Cli::try_parse_from([
            "sefai",
            "--workspace",
            fixture.0.to_str().unwrap(),
            "prompt",
            "hi",
        ])
        .unwrap()
        .selection;
        let job = Job {
            prepared: selection.prepare("hi".into()).unwrap(),
            path: "unused.gguf".into(),
            format: None,
            device: Device::Cpu,
            settings: Settings::default(),
            user: "hi".into(),
            scope: Scope::freeze(&fixture.0).unwrap(),
        };
        let state = State {
            selection,
            model: None,
            session: "chat".into(),
            revision: None,
            history: Vec::new(),
            statistics: String::new(),
            unsaved: false,
            context_used: 0,
            tools: Some(ToolProtocol::Smollm2),
            redactor: Redactor::environment().unwrap(),
            uncertain: false,
            #[cfg(feature = "mcp")]
            mcp: None,
        };
        (state, job)
    }

    #[test]
    fn failed_dispatch_checkpoint_prevents_a_write() {
        let fixture = Fixture::new();
        let (mut state, job) = state_and_job(&fixture);
        Workspace::open(&fixture.0)
            .unwrap()
            .save_turn("chat", None, "editor", "keep", false)
            .unwrap();
        let before = fs::read(fixture.0.join("sessions/chat.md")).unwrap();
        let mut turn = Turn::new(&job.prepared.prompt, true);
        let action = turn.accept("<tool_call>[{\"name\":\"remember_memory\",\"arguments\":{\"id\":\"note\",\"text\":\"do not write\"}}]</tool_call>".into(), 1, true).unwrap().unwrap();
        let (sender, incoming) = std::sync::mpsc::sync_channel(1);
        sender.send("/approve".into()).unwrap();
        let (events, _received) = std::sync::mpsc::sync_channel(64);
        let cancelled = AtomicBool::new(false);
        assert!(
            state
                .dispatch(
                    &mut turn,
                    &action,
                    &Dispatch {
                        job: &job,
                        base: None,
                        events: &events,
                        commands: &incoming,
                        cancelled: &cancelled,
                        started: Instant::now()
                    }
                )
                .is_err()
        );
        assert!(!fixture.0.join("memory/note.md").exists());
        assert_eq!(
            fs::read(fixture.0.join("sessions/chat.md")).unwrap(),
            before
        );
    }

    #[test]
    fn production_dispatch_persists_the_actual_write_outcome() {
        let fixture = Fixture::new();
        let (mut state, job) = state_and_job(&fixture);
        let mut turn = Turn::new(&job.prepared.prompt, true);
        let action = turn.accept("<tool_call>[{\"name\":\"remember_memory\",\"arguments\":{\"id\":\"note\",\"text\":\"hello\"}}]</tool_call>".into(), 1, true).unwrap().unwrap();
        let (sender, incoming) = std::sync::mpsc::sync_channel(1);
        sender.send("/approve".into()).unwrap();
        let (events, _received) = std::sync::mpsc::sync_channel(64);
        let cancelled = AtomicBool::new(false);
        state
            .dispatch(
                &mut turn,
                &action,
                &Dispatch {
                    job: &job,
                    base: None,
                    events: &events,
                    commands: &incoming,
                    cancelled: &cancelled,
                    started: Instant::now(),
                },
            )
            .unwrap();
        assert!(
            fs::read_to_string(fixture.0.join("memory/note.md"))
                .unwrap()
                .ends_with("hello")
        );
        let saved = fs::read_to_string(fixture.0.join("sessions/chat.md")).unwrap();
        assert!(!harness::uncertain_dispatch(&saved));
        assert!(saved.contains("\"outcome\":\"completed\""));
    }
    #[test]
    fn dispatched_write_error_is_persisted_as_uncertain() {
        let fixture = Fixture::new();
        fixture.memory();
        let (mut state, job) = state_and_job(&fixture);
        let mut turn = Turn::new(&job.prepared.prompt, true);
        let action = turn.accept("<tool_call>[{\"name\":\"remember_memory\",\"arguments\":{\"id\":\"note\",\"text\":\"replacement\"}}]</tool_call>".into(), 1, true).unwrap().unwrap();
        let (sender, incoming) = std::sync::mpsc::sync_channel(1);
        sender.send("/approve".into()).unwrap();
        let (events, _received) = std::sync::mpsc::sync_channel(64);
        let cancelled = AtomicBool::new(false);
        state
            .dispatch(
                &mut turn,
                &action,
                &Dispatch {
                    job: &job,
                    base: None,
                    events: &events,
                    commands: &incoming,
                    cancelled: &cancelled,
                    started: Instant::now(),
                },
            )
            .unwrap();
        let saved = fs::read_to_string(fixture.0.join("sessions/chat.md")).unwrap();
        assert!(harness::uncertain_dispatch(&saved));
        assert!(
            fs::read_to_string(fixture.0.join("memory/note.md"))
                .unwrap()
                .ends_with("original")
        );
    }
    #[cfg(feature = "mcp")]
    fn dispatch_fixture(
        approved: bool,
        conflict: bool,
        failed: bool,
    ) -> (String, Vec<serde_json::Value>) {
        use crate::mcp::tests::{connection_at, initialized};
        let fixture = Fixture::new();
        let (mut state, job) = state_and_job(&fixture);
        let (connection, captured) = connection_at(
            &fixture.0,
            vec![
                initialized(),
                serde_json::json!({"content":[{"type":"text","text":"actual result"}],"isError":failed}),
            ],
        );
        state.mcp = Some(connection);
        if conflict {
            Workspace::open(&fixture.0)
                .unwrap()
                .save_turn("chat", None, "editor", "keep", false)
                .unwrap();
        }
        let mut turn = Turn::new(&job.prepared.prompt, true);
        let action = turn.accept("<tool_call>[{\"name\":\"mcp_call\",\"arguments\":{\"id\":\"fixture\",\"mcp\":{\"tool\":\"echo\",\"arguments\":{\"text\":\"exact request\"}}}}]</tool_call>".into(), 1, true).unwrap().unwrap();
        let (sender, incoming) = std::sync::mpsc::sync_channel(1);
        sender
            .send(if approved { "/approve" } else { "/deny" }.into())
            .unwrap();
        let (events, _received) = std::sync::mpsc::sync_channel(64);
        let cancelled = AtomicBool::new(false);
        let result = state.dispatch(
            &mut turn,
            &action,
            &Dispatch {
                job: &job,
                base: None,
                events: &events,
                commands: &incoming,
                cancelled: &cancelled,
                started: Instant::now(),
            },
        );
        assert_eq!(result.is_err(), conflict);
        let audit = turn.audit.join("\n");
        let requests = captured.lock().unwrap().clone();
        (audit, requests)
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn approved_mcp_dispatch_preserves_exact_arguments_and_outcome() {
        let (audit, requests) = dispatch_fixture(true, false, false);
        let call = requests
            .iter()
            .find(|request| request["method"] == "tools/call")
            .unwrap();
        assert_eq!(call["params"]["arguments"]["text"], "exact request");
        assert!(audit.contains("\"outcome\":\"completed\""));
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn denied_mcp_dispatch_sends_no_tool_request() {
        let (audit, requests) = dispatch_fixture(false, false, false);
        assert!(
            !requests
                .iter()
                .any(|request| request["method"] == "tools/call")
        );
        assert!(audit.contains("\"outcome\":\"denied\""));
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn checkpoint_conflict_prevents_mcp_dispatch() {
        let (_, requests) = dispatch_fixture(true, true, false);
        assert!(
            !requests
                .iter()
                .any(|request| request["method"] == "tools/call")
        );
    }

    #[cfg(feature = "mcp")]
    #[test]
    fn server_tool_error_is_a_failed_outcome() {
        let (audit, _) = dispatch_fixture(true, false, true);
        assert!(audit.contains("\"outcome\":\"failed\""));
    }
}
