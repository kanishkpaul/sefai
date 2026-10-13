//! The persona ability: who the companion is.
//!
//! A persona is a small TOML file (see `persona.example.toml`). It is rendered
//! once, when the ability is built, and contributed as a system section on
//! every turn. Importing community Character Cards (V2/V3) is planned for
//! phase 4 of `docs/build-plan.md`.

use std::path::{Path, PathBuf};

use sefai_core::{Ability, AbilityError, Prompt};
use serde::Deserialize;

/// The name this ability is known by in configuration and logs.
pub const NAME: &str = "persona";

/// Who the companion is and how it speaks.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Persona {
    pub name: String,
    pub description: String,
    /// Short lines describing how the companion talks, one habit per line.
    #[serde(default)]
    pub voice: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("reading {}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("parsing {}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("{} has an empty `name`", path.display())]
    EmptyName { path: PathBuf },
}

impl Persona {
    /// Reads and validates a persona file.
    ///
    /// # Errors
    ///
    /// Fails if the file cannot be read, is not valid persona TOML, or has a
    /// blank `name`.
    pub fn load(path: &Path) -> Result<Persona, LoadError> {
        let text = std::fs::read_to_string(path).map_err(|source| LoadError::Read {
            path: path.to_owned(),
            source,
        })?;
        let persona: Persona = toml::from_str(&text).map_err(|source| LoadError::Parse {
            path: path.to_owned(),
            source,
        })?;
        if persona.name.trim().is_empty() {
            return Err(LoadError::EmptyName {
                path: path.to_owned(),
            });
        }
        Ok(persona)
    }

    /// Renders the persona as system-prompt text.
    pub fn render(&self) -> String {
        let mut text = format!("You are {}. {}", self.name.trim(), self.description.trim());
        if !self.voice.is_empty() {
            text.push_str("\n\nHow you speak:");
            for line in &self.voice {
                text.push_str("\n- ");
                text.push_str(line.trim());
            }
        }
        text
    }
}

/// Contributes the persona to every turn.
pub struct PersonaAbility {
    rendered: String,
}

impl PersonaAbility {
    pub fn new(persona: &Persona) -> PersonaAbility {
        PersonaAbility {
            rendered: persona.render(),
        }
    }
}

impl Ability for PersonaAbility {
    fn name(&self) -> &'static str {
        NAME
    }

    fn prepare(&mut self, prompt: &mut Prompt) -> Result<(), AbilityError> {
        prompt.add_section(NAME, self.rendered.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_file_parses() {
        let persona: Persona = toml::from_str(include_str!("../persona.example.toml")).unwrap();

        assert_eq!(persona.name, "Aria");
        assert_eq!(persona.voice.len(), 3);
    }

    #[test]
    fn render_lists_voice_lines() {
        let persona = Persona {
            name: "Aria".into(),
            description: "A friend.".into(),
            voice: vec!["Direct.".into(), "Warm.".into()],
        };

        assert_eq!(
            persona.render(),
            "You are Aria. A friend.\n\nHow you speak:\n- Direct.\n- Warm."
        );
    }

    #[test]
    fn render_without_voice_is_one_line() {
        let persona = Persona {
            name: "Aria".into(),
            description: "A friend.".into(),
            voice: Vec::new(),
        };

        assert_eq!(persona.render(), "You are Aria. A friend.");
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let result = toml::from_str::<Persona>("name = \"A\"\ndescription = \"B\"\nmood = \"C\"");

        assert!(result.is_err());
    }
}
