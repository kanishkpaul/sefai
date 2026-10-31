#[cfg(feature = "inference")]
mod engine;
#[cfg(feature = "inference")]
mod harness;
#[cfg(feature = "mcp")]
mod mcp;
#[cfg(feature = "inference")]
mod runner;
#[cfg(feature = "inference")]
mod setup;
mod term;
#[cfg(feature = "inference")]
mod tui;
mod workspace;

use clap::{Parser, Subcommand};
use std::{fs::File, io, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(styles = term::styles(), version, about = "A small local LLM runner and terminal harness")]
struct Cli {
    #[command(flatten)]
    selection: workspace::Selection,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the persistent terminal conversation (use --plain for line input).
    #[cfg(feature = "inference")]
    Chat(tui::Options),
    /// Validate Markdown workspace files without executing their contents.
    Workspace {
        #[command(subcommand)]
        action: WorkspaceCommand,
    },
    /// Inspect an explicitly approved MCP server without loading a model.
    #[cfg(feature = "mcp")]
    Mcp(mcp::Options),
    /// Print a prompt without loading a model.
    Prompt { message: String },
    /// List available local inference devices.
    #[cfg(feature = "inference")]
    Devices,
    /// Stream a reply from a local model.
    #[cfg(feature = "inference")]
    Run(runner::Options),
    /// Inspect hardware, check local models and shortlist catalog models without loading one.
    #[cfg(feature = "inference")]
    Setup {
        #[command(subcommand)]
        action: setup::Action,
    },
    /// Measure prefill and generation with one loaded model.
    #[cfg(feature = "inference")]
    Bench {
        #[command(flatten)]
        options: runner::Options,
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(1..=20))]
        repetitions: u32,
    },
}

#[derive(Subcommand)]
enum WorkspaceCommand {
    Check,
    /// Create missing workspace directories and starter Markdown files.
    Init,
    /// Validate and atomically install a Markdown file; updates require a snapshot.
    Put {
        path: PathBuf,
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        expected: Option<PathBuf>,
    },
    /// Create an explicitly requested durable memory.
    Remember {
        id: String,
        text: String,
    },
    /// Edit memory only if it still matches a saved snapshot.
    Edit {
        id: String,
        text: String,
        #[arg(long)]
        expected: PathBuf,
    },
    /// Delete memory only if it still matches a saved snapshot.
    Forget {
        id: String,
        #[arg(long)]
        expected: PathBuf,
    },
    /// Append one literal record; existing sessions require a saved snapshot.
    Append {
        id: String,
        #[arg(long, value_parser = ["user", "assistant"])]
        role: String,
        text: String,
        #[arg(long)]
        expected: Option<PathBuf>,
    },
    /// Read a bounded session tail without loading a model.
    Tail {
        id: String,
        #[arg(long, default_value_t = 32)]
        messages: usize,
    },
}

fn main() -> ExitCode {
    match execute(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let mut message = Vec::new();
            let _ = term::Output::default().write(&error.to_string(), &mut message);
            let message = String::from_utf8_lossy(&message).replace(['\n', '\t'], " ");
            let theme =
                term::theme::Term::new(&term::palettes::CANDLE, &term::identity::SEFAI, "SEFAI");
            eprintln!("{}", theme.error(&message));
            ExitCode::FAILURE
        }
    }
}

fn execute(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "inference")]
    let command = cli
        .command
        .unwrap_or(Command::Chat(tui::Options::default()));
    #[cfg(not(feature = "inference"))]
    let command = cli
        .command
        .ok_or("choose a command; chat requires an inference feature")?;
    match command {
        #[cfg(feature = "inference")]
        Command::Chat(options) => tui::run(cli.selection, options)?,
        Command::Workspace { action } => {
            let root = cli.selection.root()?;
            if matches!(action, WorkspaceCommand::Init) {
                workspace::Workspace::init(&root)?;
            } else {
                let workspace = workspace::Workspace::open(&root)?;
                match action {
                    WorkspaceCommand::Check => println!("validated {} records", workspace.check()?),
                    WorkspaceCommand::Put {
                        path,
                        from,
                        expected,
                    } => workspace.put(&path, expected.as_deref(), File::open(from)?)?,
                    WorkspaceCommand::Remember { id, text } => {
                        workspace.remember(&id, &text, None)?;
                    }
                    WorkspaceCommand::Edit { id, text, expected } => {
                        workspace.remember(&id, &text, Some(&expected))?
                    }
                    WorkspaceCommand::Forget { id, expected } => {
                        workspace.forget(&id, &expected)?
                    }
                    WorkspaceCommand::Append {
                        id,
                        role,
                        text,
                        expected,
                    } => workspace.append(&id, &role, &text, expected.as_deref())?,
                    WorkspaceCommand::Tail { id, messages } => {
                        print_messages(workspace.session(&id, messages)?)?
                    }
                    WorkspaceCommand::Init => unreachable!(),
                }
            }
        }
        #[cfg(feature = "mcp")]
        Command::Mcp(options) => mcp::run(&cli.selection.root()?, options)?,
        Command::Prompt { message } => {
            print_messages(cli.selection.prepare(message)?.prompt.to_messages())?;
        }
        #[cfg(feature = "inference")]
        Command::Devices => runner::devices()?,
        #[cfg(feature = "inference")]
        Command::Setup { action } => {
            let cancelled = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let signal = cancelled.clone();
            ctrlc::set_handler(move || signal.store(true, std::sync::atomic::Ordering::Relaxed))?;
            let report = setup::report(&cli.selection, &action, &cancelled)?;
            term::Output::default().write(&report, &mut io::stdout().lock())?;
        }
        #[cfg(feature = "inference")]
        Command::Run(options) => runner::run(options, runner::Mode::Reply, &cli.selection)?,
        #[cfg(feature = "inference")]
        Command::Bench {
            options,
            repetitions,
        } => runner::run(
            options,
            runner::Mode::Benchmark(repetitions),
            &cli.selection,
        )?,
    }
    Ok(())
}

fn print_messages(messages: Vec<sefai_core::Message>) -> io::Result<()> {
    let mut output = io::stdout().lock();
    let mut terminal = term::Output::default();
    for message in messages {
        terminal.write(
            &format!("[{}]\n{}\n\n", message.role.as_str(), message.content),
            &mut output,
        )?;
    }
    Ok(())
}
