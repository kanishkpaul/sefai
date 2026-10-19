#[cfg(feature = "gguf")]
mod runner;
mod term;
mod workspace;

use clap::{Parser, Subcommand};
use std::{io, process::ExitCode};

#[derive(Parser)]
#[command(styles = term::styles(), version, about = "A small local LLM runner and terminal harness")]
struct Cli {
    #[command(flatten)]
    selection: workspace::Selection,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate Markdown workspace files without executing their contents.
    Workspace {
        #[command(subcommand)]
        action: WorkspaceCommand,
    },
    /// Print a prompt without loading a model.
    Prompt { message: String },
    /// List available local inference devices.
    #[cfg(feature = "gguf")]
    Devices,
    /// Stream a reply from a local GGUF model.
    #[cfg(feature = "gguf")]
    Run(runner::Options),
    /// Measure prefill and generation with one loaded model.
    #[cfg(feature = "gguf")]
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
    match cli.command {
        Command::Workspace {
            action: WorkspaceCommand::Check,
        } => {
            let workspace = workspace::Workspace::open(&cli.selection.root()?)?;
            println!("validated {} records", workspace.check()?);
        }
        Command::Prompt { message } => {
            let mut output = io::stdout().lock();
            let mut terminal = term::Output::default();
            for message in cli.selection.prepare(message)?.prompt.to_messages() {
                terminal.write(
                    &format!("[{}]\n{}\n\n", message.role.as_str(), message.content),
                    &mut output,
                )?;
            }
        }
        #[cfg(feature = "gguf")]
        Command::Devices => runner::devices()?,
        #[cfg(feature = "gguf")]
        Command::Run(options) => runner::run(options, runner::Mode::Reply, &cli.selection)?,
        #[cfg(feature = "gguf")]
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
