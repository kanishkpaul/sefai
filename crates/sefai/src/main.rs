//! The `sefai` binary: the command line and the composition root.
//!
//! It parses arguments, builds the enabled abilities and hands them to the
//! runtime. Until the engine lands in phase 1, `prompt` stops short of a
//! model and prints the prompt the abilities assembled.

mod abilities;

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sefai_core::{Message, Prompt};

#[derive(Parser)]
#[command(version, about = "A local AI companion runtime")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List the abilities compiled into this binary.
    Abilities,
    /// Print the prompt a message would produce, without running a model.
    Prompt {
        /// The user's message.
        message: String,
        /// A persona file to speak as.
        #[cfg(feature = "persona")]
        #[arg(long)]
        persona: Option<std::path::PathBuf>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Command::Abilities => {
            for name in abilities::compiled() {
                println!("{name}");
            }
        }
        Command::Prompt {
            message,
            #[cfg(feature = "persona")]
            persona,
        } => {
            let options = abilities::Options {
                #[cfg(feature = "persona")]
                persona,
            };
            let mut abilities = abilities::build(&options)?;
            let mut prompt = Prompt::new(vec![Message::user(message)]);
            for failure in abilities.prepare(&mut prompt) {
                eprintln!("warning: {} skipped: {}", failure.ability, failure.error);
            }
            for message in prompt.to_messages() {
                println!("[{}]\n{}\n", message.role.as_str(), message.content);
            }
        }
    }
    Ok(())
}
