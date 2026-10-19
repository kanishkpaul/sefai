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
    pub fn save_turn(
        &self,
        id: &str,
        revision: Option<&str>,
        user: &str,
        assistant: &str,
        interrupted: bool,
    ) -> io::Result<String> {
        schema::id(id)?;
        let body = match revision {
            Some(text) => schema::parse::<schema::Session>(text)?.1,
            None => "",
        };
        let outcome = if interrupted {
            "interrupted"
        } else {
            "completed"
        };
        let mut next = format!("---\nschema: 1\nid: {id}\noutcome: {outcome}\n---\n{body}");
        next.push_str(&session::encode("user", user)?);
        next.push_str(&session::encode("assistant", assistant)?);
        if next.len() > schema::FILE_BYTES_MAX {
            return Err(invalid(
                "session: interactive file exceeds 1048576 bytes; start a new session",
            ));
        }
        let relative = format!("sessions/{id}.md");
        let relative = Path::new(&relative);
        let _lock = self.lock()?;
        self.stage_checked(relative, next.as_bytes(), || {
            match (self.root.join(relative).exists(), revision) {
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
