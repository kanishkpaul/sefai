//! The contract every ability implements.
//!
//! An ability is one self-contained capability of the companion: its persona,
//! its memory, its habit of reaching out first. The runtime knows abilities
//! only through [`Ability`], and abilities never know about each other, so
//! adding or removing one touches its own crate and one block in the `sefai`
//! binary. `docs/abilities.md` walks through both.
//!
//! Hooks are added to the trait when the first ability needs them, not before.
//! Today there is one, [`Ability::prepare`], which runs before every turn.

use crate::prompt::Prompt;

/// The error an ability hook returns. Abilities are free to use any error type.
pub type AbilityError = Box<dyn std::error::Error + Send + Sync>;

/// One capability of the companion that can be compiled in or left out.
pub trait Ability: Send {
    /// A short, stable, lowercase identifier used in configuration and logs.
    fn name(&self) -> &'static str;

    /// Contributes system sections to the prompt before the model sees it.
    ///
    /// # Errors
    ///
    /// Returns an error when the ability cannot contribute this turn. The turn
    /// still runs, without anything this ability added to the prompt.
    fn prepare(&mut self, prompt: &mut Prompt) -> Result<(), AbilityError>;
}

/// The abilities enabled for this run, in the order they contribute.
#[derive(Default)]
pub struct Abilities {
    abilities: Vec<Box<dyn Ability>>,
}

/// An ability hook that failed, for the caller to report.
#[derive(Debug)]
pub struct Failure {
    pub ability: &'static str,
    pub error: AbilityError,
}

impl Abilities {
    /// Enables an ability after the ones already enabled.
    ///
    /// # Panics
    ///
    /// Panics if an ability with the same name is already enabled. That is a
    /// wiring mistake in the binary, not something a user can cause.
    pub fn push(&mut self, ability: Box<dyn Ability>) {
        let name = ability.name();
        assert!(
            self.names().all(|enabled| enabled != name),
            "ability `{name}` enabled twice"
        );
        self.abilities.push(ability);
    }

    /// Returns the names of the enabled abilities, in order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.abilities.iter().map(|ability| ability.name())
    }

    /// Runs every ability's [`Ability::prepare`] hook, in order.
    ///
    /// A failure never stops the turn. Whatever the failing ability added is
    /// removed so the model never sees a half-written section, the remaining
    /// abilities still run, and the failures are returned for logging.
    pub fn prepare(&mut self, prompt: &mut Prompt) -> Vec<Failure> {
        let mut failures = Vec::new();
        for ability in &mut self.abilities {
            let n_sections = prompt.sections.len();
            if let Err(error) = ability.prepare(prompt) {
                prompt.sections.truncate(n_sections);
                failures.push(Failure {
                    ability: ability.name(),
                    error,
                });
            }
        }
        failures
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Message;

    struct Fixed(&'static str);

    impl Ability for Fixed {
        fn name(&self) -> &'static str {
            self.0
        }

        fn prepare(&mut self, prompt: &mut Prompt) -> Result<(), AbilityError> {
            prompt.add_section(self.0, self.0);
            Ok(())
        }
    }

    struct HalfWritten;

    impl Ability for HalfWritten {
        fn name(&self) -> &'static str {
            "half-written"
        }

        fn prepare(&mut self, prompt: &mut Prompt) -> Result<(), AbilityError> {
            prompt.add_section("half-written", "partial");
            Err("store unavailable".into())
        }
    }

    #[test]
    fn abilities_contribute_in_order() {
        let mut abilities = Abilities::default();
        abilities.push(Box::new(Fixed("first")));
        abilities.push(Box::new(Fixed("second")));
        let mut prompt = Prompt::new(vec![Message::user("hi")]);

        let failures = abilities.prepare(&mut prompt);

        assert!(failures.is_empty());
        let sources: Vec<_> = prompt
            .sections()
            .iter()
            .map(|section| section.source)
            .collect();
        assert_eq!(sources, ["first", "second"]);
    }

    #[test]
    fn failing_ability_does_not_abort_turn() {
        let mut abilities = Abilities::default();
        abilities.push(Box::new(HalfWritten));
        abilities.push(Box::new(Fixed("after")));
        let mut prompt = Prompt::new(vec![Message::user("hi")]);

        let failures = abilities.prepare(&mut prompt);

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].ability, "half-written");
        let sources: Vec<_> = prompt
            .sections()
            .iter()
            .map(|section| section.source)
            .collect();
        assert_eq!(sources, ["after"]);
    }
}
