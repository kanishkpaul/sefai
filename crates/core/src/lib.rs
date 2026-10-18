//! Messages and prompt sections shared by the runner and harness.
//!
//! This crate is the bottom of the dependency graph. It has no dependencies,
//! does no I/O, and knows nothing about models, storage or user interfaces, so
//! other crates can depend on it without pulling anything else in.

mod message;
mod prompt;

pub use message::{Message, Role};
pub use prompt::{Prompt, Section};
