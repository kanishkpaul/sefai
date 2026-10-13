//! Shared vocabulary for sefai: messages, prompts, and the ability contract.
//!
//! This crate is the bottom of the dependency graph. It has no dependencies,
//! does no I/O, and knows nothing about models, storage or user interfaces, so
//! every other crate can depend on it without pulling anything else in.

mod ability;
mod message;
mod prompt;

pub use ability::{Abilities, Ability, AbilityError, Failure};
pub use message::{Message, Role};
pub use prompt::{Prompt, Section};
