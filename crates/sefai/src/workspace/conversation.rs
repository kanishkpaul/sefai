use super::{Workspace, schema, schema::invalid, session};
use std::{io, path::Path};

impl Workspace {
    /// Captures at most 1 MiB for exact revision checks in an interactive session.
    pub fn conversation_revision(&self, id: &str) -> io::Result<String> {
        schema::id(id)?;
        let path = format!("sessions/{id}.md");
        let text = self.read(Path::new(&path))?;
        let (header, _) = session::read(io::Cursor::new(text.as_bytes()), 1)?;
        self.identity(Path::new(&path), header.schema, &header.id)?;
        Ok(text)
    }

    /// Saves both records and the outcome atomically, rejecting concurrent edits.
    #[cfg(test)]
    pub fn save_turn(
        &self,
        id: &str,
        revision: Option<&str>,
        user: &str,
        assistant: &str,
        interrupted: bool,
    ) -> io::Result<String> {
        self.save_records(
            id,
            revision,
            &[("user", user), ("assistant", assistant)],
            if interrupted {
                "interrupted"
            } else {
                "completed"
            },
        )
    }

    /// Replaces a complete turn checkpoint; harness records never become model roles.
    #[cfg(test)]
    pub fn save_records(
        &self,
        id: &str,
        revision: Option<&str>,
        records: &[(&str, &str)],
        outcome: &str,
    ) -> io::Result<String> {
        self.save_checkpoint(id, revision, revision, records, outcome)
    }

    /// Replaces records following `base`, checking exact current bytes against `expected`.
    /// Callers retain the pre-turn base across checkpoints and dispatch only after success.
    pub fn save_checkpoint(
        &self,
        id: &str,
        base: Option<&str>,
        expected: Option<&str>,
        records: &[(&str, &str)],
        outcome: &str,
    ) -> io::Result<String> {
        schema::id(id)?;
        let body = match base {
            Some(text) => schema::parse::<schema::Session>(text)?.1,
            None => "",
        };
        if !matches!(outcome, "completed" | "interrupted") {
            return Err(invalid("session.outcome: unsupported outcome"));
        }
        let mut next = format!("---\nschema: 1\nid: {id}\noutcome: {outcome}\n---\n{body}");
        for (role, text) in records {
            next.push_str(&session::encode(role, text)?);
        }
        if next.len() > schema::FILE_BYTES_MAX {
            return Err(invalid(
                "session: interactive file exceeds 1048576 bytes; start a new session",
            ));
        }
        let relative = format!("sessions/{id}.md");
        let relative = Path::new(&relative);
        let _lock = self.lock()?;
        self.stage_checked(relative, next.as_bytes(), || {
            match (self.root.join(relative).exists(), expected) {
                (false, None) => Ok(()),
                (true, Some(expected)) if self.read(relative)? == expected => Ok(()),
                _ => Err(invalid(
                    "session: changed on disk; resume it or start a new session",
                )),
            }
        })?;
        Ok(next)
    }
}
