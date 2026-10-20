//! Markdown is authoritative; loading instructions never grants tool authority.

#[cfg(feature = "gguf")]
mod conversation;
mod files;
mod schema;
mod session;

#[cfg(feature = "gguf")]
pub use schema::Backend;
#[cfg(feature = "mcp-oauth")]
pub use schema::OAuth;
pub use schema::Runner;
use schema::{Config, Format, Memory, Model, Skill, Text, invalid};
#[cfg(feature = "mcp")]
pub use schema::{Mcp, Transport};
use sefai_core::{Message, Prompt};
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

const ENTRY_COUNT_MAX: usize = 1024;
const PROMPT_BYTES_MAX: usize = 1024 * 1024;

#[derive(clap::Args, Clone, Default)]
pub struct Selection {
    /// Markdown workspace; defaults to the platform configuration directory.
    #[arg(long, global = true)]
    workspace: Option<PathBuf>,
    /// Optional user instructions, composed before workspace instructions.
    #[arg(long, global = true)]
    global_workspace: Option<PathBuf>,
    #[arg(long, global = true)]
    pub(crate) profile: Option<String>,
    /// Selected skill IDs, in prompt order (at most 16).
    #[arg(long, global = true)]
    pub(crate) skill: Vec<String>,
    /// Recall at most eight memories containing this literal text.
    #[arg(long, global = true)]
    pub(crate) recall: Option<String>,
    /// Resume at most 32 records and 1 MiB from this session.
    #[arg(long, global = true)]
    pub(crate) session: Option<String>,
}

pub struct Workspace {
    root: PathBuf,
}

pub struct Prepared {
    pub prompt: Prompt,
    pub runner: Runner,
    pub model: Option<PathBuf>,
}

impl Selection {
    #[cfg(feature = "gguf")]
    pub fn validate_id(id: &str) -> io::Result<()> {
        schema::id(id)
    }

    pub fn root(&self) -> io::Result<PathBuf> {
        self.workspace.clone().or_else(default_root).ok_or_else(|| {
            invalid("workspace: specify --workspace; platform config directory is unavailable")
        })
    }

    pub fn prepare(&self, message: String) -> io::Result<Prepared> {
        if message.len() > PROMPT_BYTES_MAX {
            return Err(invalid("prompt: exceeds 1048576 bytes"));
        }
        if self.skill.len() > 16 {
            return Err(invalid("skill: at most 16 selections"));
        }
        let mut result = Prepared {
            prompt: Prompt::new(vec![Message::user(message)]),
            runner: Runner::default(),
            model: None,
        };
        if let Some(root) = &self.global_workspace {
            Workspace::open(root)?.instructions(&mut result.prompt)?;
        }
        let root = self.root()?;
        if self.workspace.is_none() && !root.exists() {
            if self.profile.is_some()
                || !self.skill.is_empty()
                || self.recall.is_some()
                || self.session.is_some()
            {
                return Err(invalid("workspace: initialize it before selecting content"));
            }
            return Ok(result);
        }
        let workspace = Workspace::open(&root)?;
        if let Some(id) = &self.session {
            let mut messages = workspace.session(id, 32)?;
            messages.extend_from_slice(result.prompt.conversation());
            let additional = messages
                .iter()
                .map(|message| message.content.len())
                .sum::<usize>();
            let sections_bytes = result
                .prompt
                .sections()
                .iter()
                .map(|section| section.text.len())
                .sum::<usize>();
            if additional + sections_bytes > PROMPT_BYTES_MAX {
                return Err(invalid("prompt: resumed text exceeds 1048576 bytes"));
            }
            let sections = result.prompt.sections().to_vec();
            result.prompt = Prompt::new(messages);
            for section in sections {
                result.prompt.add_section(section.source, section.text);
            }
        }
        let text = workspace.read(Path::new("config.md"))?;
        let (config, _) = schema::parse::<Config>(&text).map_err(|error| at("config.md", error))?;
        schema::version(config.schema).map_err(|error| at("config.md", error))?;
        config
            .runner
            .validate()
            .map_err(|error| at("config.md", error))?;
        result.runner = config.runner;
        if let Some(profile) = self.profile.as_ref().or(config.model.as_ref()) {
            schema::id(profile)?;
            let path = PathBuf::from(format!("models/{profile}.md"));
            let text = workspace.read(&path)?;
            let (model, _) = schema::parse::<Model>(&text).map_err(|error| at(&path, error))?;
            workspace.validate_model(&path, &model)?;
            if !matches!(model.format, Format::Gguf) {
                return Err(at(
                    &path,
                    "format: this build runs only gguf; no automatic conversion",
                ));
            }
            result.model = Some(if model.path.is_absolute() {
                model.path
            } else {
                workspace.root.join(model.path)
            });
            result.runner.overlay(&model.runner);
        }
        workspace.instructions(&mut result.prompt)?;
        for id in &self.skill {
            schema::id(id)?;
            let path = PathBuf::from(format!("skills/{id}/SKILL.md"));
            let text = workspace.read(&path)?;
            let (header, body) = schema::parse::<Skill>(&text).map_err(|error| at(&path, error))?;
            workspace.validate_skill(&path, &header)?;
            add_section(&mut result.prompt, "skill", body)?;
        }
        if let Some(query) = &self.recall {
            if query.is_empty() || query.len() > 1024 {
                return Err(invalid("recall: expected 1..1024 bytes"));
            }
            let mut count = 0;
            for path in workspace.entries("memory")? {
                let text = workspace.read(&path)?;
                let (header, body) =
                    schema::parse::<Memory>(&text).map_err(|error| at(&path, error))?;
                workspace.identity(&path, header.schema, &header.id)?;
                if body.contains(query) {
                    add_section(&mut result.prompt, "memory", body)?;
                    count += 1;
                    if count == 8 {
                        break;
                    }
                }
            }
        }
        prompt_budget(&result.prompt, 0)?;
        Ok(result)
    }
}

impl Workspace {
    pub fn open(root: &Path) -> io::Result<Self> {
        let root = root.canonicalize().map_err(|error| at(root, error))?;
        if !root.is_dir() {
            return Err(at(&root, "workspace: expected a directory"));
        }
        Ok(Self { root })
    }

    fn path(&self, relative: &Path) -> io::Result<PathBuf> {
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(at(relative, "path: expected a workspace-relative path"));
        }
        let path = self.root.join(relative);
        let canonical = path.canonicalize().map_err(|error| at(relative, error))?;
        if !canonical.starts_with(&self.root) {
            return Err(at(relative, "path: escapes workspace"));
        }
        if !canonical.is_file() {
            return Err(at(relative, "path: expected a regular file"));
        }
        Ok(canonical)
    }

    pub(crate) fn read(&self, relative: &Path) -> io::Result<String> {
        let path = self.path(relative)?;
        let mut text = String::new();
        fs::File::open(path)?
            .take(schema::FILE_BYTES_MAX as u64 + 1)
            .read_to_string(&mut text)
            .map_err(|error| at(relative, error))?;
        if text.len() > schema::FILE_BYTES_MAX {
            return Err(at(relative, "file: exceeds 1048576 bytes"));
        }
        Ok(text)
    }

    pub(crate) fn entries(&self, directory: &str) -> io::Result<Vec<PathBuf>> {
        let relative = Path::new(directory);
        let directory_path = self.root.join(relative);
        if !directory_path.exists() {
            return Ok(Vec::new());
        }
        let canonical = directory_path.canonicalize()?;
        if !canonical.starts_with(&self.root) {
            return Err(at(relative, "path: escapes workspace"));
        }
        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(canonical)?.enumerate() {
            if index == ENTRY_COUNT_MAX {
                return Err(at(relative, "directory: exceeds 1024 entries"));
            }
            let entry = entry?;
            if directory == "skills" {
                let path = relative.join(entry.file_name()).join("SKILL.md");
                entries.push(path);
            } else if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "md")
            {
                entries.push(relative.join(entry.file_name()));
            }
        }
        entries.sort();
        Ok(entries)
    }

    fn instructions(&self, prompt: &mut Prompt) -> io::Result<()> {
        for (name, source) in [("system.md", "system"), ("preferences.md", "preferences")] {
            let path = Path::new(name);
            if self.root.join(path).exists() {
                let text = self.read(path)?;
                let (header, body) =
                    schema::parse::<Text>(&text).map_err(|error| at(path, error))?;
                schema::version(header.schema).map_err(|error| at(path, error))?;
                add_section(prompt, source, body)?;
            }
        }
        Ok(())
    }

    fn identity(&self, path: &Path, version: u32, id: &str) -> io::Result<()> {
        schema::version(version).map_err(|error| at(path, error))?;
        schema::id(id).map_err(|error| at(path, error))?;
        let expected = if path.file_name().is_some_and(|name| name == "SKILL.md") {
            path.parent().and_then(Path::file_name)
        } else {
            path.file_stem()
        };
        if expected.and_then(|name| name.to_str()) != Some(id) {
            return Err(at(
                path,
                "id: must match filename (skill directory for SKILL.md)",
            ));
        }
        Ok(())
    }

    fn validate_model(&self, path: &Path, header: &Model) -> io::Result<()> {
        self.identity(path, header.schema, &header.id)?;
        if header.path.as_os_str().is_empty() {
            return Err(at(path, "path: model path is empty"));
        }
        header.runner.validate().map_err(|error| at(path, error))
    }

    fn validate_skill(&self, path: &Path, header: &Skill) -> io::Result<()> {
        self.identity(path, header.schema, &header.id)?;
        if header.resources.len() > 16 {
            return Err(at(path, "resources: at most 16 paths"));
        }
        for resource in &header.resources {
            self.path(&path.parent().unwrap_or(Path::new("")).join(resource))?;
        }
        Ok(())
    }

    #[cfg(feature = "mcp")]
    pub fn mcp(&self, id: &str) -> io::Result<(schema::Mcp, String)> {
        schema::id(id)?;
        let path = PathBuf::from(format!("mcp/{id}.md"));
        let text = self.read(&path)?;
        self.validate_document(&path, &text)?;
        let (config, _) = schema::parse(&text)?;
        Ok((config, text))
    }

    pub fn check(&self) -> io::Result<usize> {
        let text = self.read(Path::new("config.md"))?;
        let (config, _) = schema::parse::<Config>(&text).map_err(|error| at("config.md", error))?;
        schema::version(config.schema).map_err(|error| at("config.md", error))?;
        config
            .runner
            .validate()
            .map_err(|error| at("config.md", error))?;
        if let Some(id) = config.model {
            schema::id(&id).map_err(|error| at("config.md", error))?;
            self.read(Path::new(&format!("models/{id}.md")))?;
        }
        self.instructions(&mut Prompt::default())?;
        let mut count = 1 + ["system.md", "preferences.md"]
            .iter()
            .filter(|name| self.root.join(name).exists())
            .count();
        for directory in ["models", "skills", "memory", "sessions", "mcp"] {
            for path in self.entries(directory)? {
                self.validate_file(&path, &self.path(&path)?)?;
                count += 1;
            }
        }
        Ok(count)
    }

    pub(crate) fn validate_document(&self, path: &Path, text: &str) -> io::Result<()> {
        match kind(path)? {
            "config" => {
                let (header, _) = schema::parse::<Config>(text)?;
                schema::version(header.schema)?;
                if let Some(id) = header.model {
                    schema::id(&id)?;
                }
                header.runner.validate()
            }
            "text" => {
                let (header, _) = schema::parse::<Text>(text)?;
                schema::version(header.schema)
            }
            "models" => {
                let (header, _) = schema::parse::<Model>(text)?;
                self.validate_model(path, &header)
            }
            "skills" => {
                let (header, _) = schema::parse::<Skill>(text)?;
                self.validate_skill(path, &header)
            }
            "memory" => {
                let (header, _) = schema::parse::<Memory>(text)?;
                self.identity(path, header.schema, &header.id)
            }
            "mcp" => {
                let (header, _) = schema::parse::<schema::Mcp>(text)?;
                self.identity(path, header.schema, &header.id)?;
                validate_transport(&header.transport)
            }
            _ => Err(at(path, "path: unsupported document kind")),
        }
    }
}

fn prompt_budget(prompt: &Prompt, additional: usize) -> io::Result<()> {
    let bytes = prompt
        .sections()
        .iter()
        .map(|section| section.text.len())
        .sum::<usize>()
        + prompt
            .conversation()
            .iter()
            .map(|message| message.content.len())
            .sum::<usize>()
        + additional;
    if bytes > PROMPT_BYTES_MAX {
        return Err(invalid("prompt: composed text exceeds 1048576 bytes"));
    }
    Ok(())
}

fn add_section(prompt: &mut Prompt, source: &'static str, body: &str) -> io::Result<()> {
    prompt_budget(prompt, body.len())?;
    prompt.add_section(source, body);
    Ok(())
}

fn kind(path: &Path) -> io::Result<&str> {
    let parts: Vec<_> = path.iter().collect();
    match parts.as_slice() {
        [name] if *name == "config.md" => Ok("config"),
        [name] if *name == "system.md" || *name == "preferences.md" => Ok("text"),
        [directory, _] if path.extension().is_some_and(|extension| extension == "md") => {
            match directory.to_str() {
                Some(kind @ ("models" | "memory" | "mcp" | "sessions")) => Ok(kind),
                _ => Err(at(path, "path: unsupported document kind")),
            }
        }
        [directory, _, name] if *directory == "skills" && *name == "SKILL.md" => Ok("skills"),
        _ => Err(at(path, "path: unsupported document kind")),
    }
}

fn validate_transport(transport: &schema::Transport) -> io::Result<()> {
    match transport {
        schema::Transport::Stdio { command, args, env } => {
            if command.is_empty()
                || command.len() > 4096
                || args.len() > 64
                || args.iter().any(|arg| arg.len() > 4096)
                || env.len() > 64
            {
                return Err(invalid(
                    "transport: command/arguments/environment exceed limits",
                ));
            }
            for (name, reference) in env {
                env_name(name)?;
                env_name(reference)?;
            }
        }
        schema::Transport::Http {
            url,
            token_env,
            oauth,
        } => {
            if url.len() > 4096
                || !(url.starts_with("https://")
                    || url.starts_with("http://127.0.0.1:")
                    || url.starts_with("http://[::1]:"))
                || url.contains('@')
                || url.contains(['?', '#'])
                || url.len() <= 8
            {
                return Err(invalid(
                    "transport.url: expected https URL without credentials, query or fragment",
                ));
            }
            if let Some(oauth) = oauth
                && (token_env.is_some()
                    || oauth.issuer.len() > 4096
                    || oauth.client_id.is_empty()
                    || oauth.client_id.len() > 256
                    || oauth.redirect_uri.len() > 4096
                    || oauth.scopes.len() > 16
                    || oauth.scopes.iter().any(|scope| {
                        scope.len() > 256
                            || scope.is_empty()
                            || scope.bytes().any(|b| !b.is_ascii_graphic())
                    }))
            {
                return Err(invalid(
                    "transport.oauth: invalid identity, scopes or conflicting token reference",
                ));
            }
            if let Some(name) = token_env {
                env_name(name)?;
            }
        }
    }
    Ok(())
}

fn env_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > 128
        || !name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || index > 0 && byte.is_ascii_digit()
        })
    {
        return Err(invalid(
            "transport.env: expected an environment variable name, never a secret value",
        ));
    }
    Ok(())
}

fn default_root() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|root| PathBuf::from(root).join("sefai"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|root| PathBuf::from(root).join("Library/Application Support/sefai"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|root| PathBuf::from(root).join(".config")))
            .map(|root| root.join("sefai"))
    }
}

fn at(path: impl AsRef<Path>, error: impl std::fmt::Display) -> io::Error {
    invalid(format!("{}: {error}", path.as_ref().display()))
}

#[cfg(test)]
mod tests;
