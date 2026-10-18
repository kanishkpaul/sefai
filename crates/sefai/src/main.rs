use clap::{Parser, Subcommand};
use sefai_core::{Message, Prompt};

#[derive(Parser)]
#[command(version, about = "A small local LLM runner and terminal harness")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print a prompt without loading a model.
    Prompt { message: String },
}

fn main() {
    match Cli::parse().command {
        Command::Prompt { message } => {
            for message in Prompt::new(vec![Message::user(message)]).to_messages() {
                println!("[{}]\n{}\n", message.role.as_str(), message.content);
            }
        }
    }
}
