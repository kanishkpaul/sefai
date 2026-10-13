//! Which abilities this binary contains, and how each one is built.
//!
//! This module is the only place in sefai that names concrete abilities. Each
//! one is a row in [`REGISTRY`] plus a build function, both behind the Cargo
//! feature of the same name. Adding an ability is one feature in `Cargo.toml`,
//! one row, one function and, if it needs settings, one field in [`Options`].
//! Removing one is the same edit in reverse. `docs/abilities.md` has the
//! walkthrough.

use anyhow::Context;
use sefai_core::{Abilities, Ability};

/// Settings that abilities are built from.
#[derive(Default)]
pub struct Options {
    #[cfg(feature = "persona")]
    pub persona: Option<std::path::PathBuf>,
}

/// Builds one ability, or returns `None` when `options` leave it switched off.
type Build = fn(&Options) -> anyhow::Result<Option<Box<dyn Ability>>>;

struct Entry {
    name: &'static str,
    build: Build,
}

/// Every ability compiled into this binary, in the order they contribute to
/// the prompt.
const REGISTRY: &[Entry] = &[
    #[cfg(feature = "persona")]
    Entry {
        name: sefai_ability_persona::NAME,
        build: persona,
    },
];

/// Returns the names of every compiled-in ability, enabled or not.
pub fn compiled() -> impl Iterator<Item = &'static str> {
    REGISTRY.iter().map(|entry| entry.name)
}

/// Builds the abilities that `options` switch on.
pub fn build(options: &Options) -> anyhow::Result<Abilities> {
    let mut abilities = Abilities::default();
    for entry in REGISTRY {
        let ability = (entry.build)(options).with_context(|| format!("building {}", entry.name))?;
        if let Some(ability) = ability {
            abilities.push(ability);
        }
    }
    Ok(abilities)
}

#[cfg(feature = "persona")]
fn persona(options: &Options) -> anyhow::Result<Option<Box<dyn Ability>>> {
    let Some(path) = &options.persona else {
        return Ok(None);
    };
    let persona = sefai_ability_persona::Persona::load(path)?;
    Ok(Some(Box::new(sefai_ability_persona::PersonaAbility::new(
        &persona,
    ))))
}
