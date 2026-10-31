use super::{
    Workspace, at,
    schema::{self, invalid},
    session,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

impl Workspace {
    /// Creates an empty workspace without replacing existing user files.
    pub fn init(root: &Path) -> io::Result<Self> {
        fs::create_dir_all(root).map_err(|error| at(root, error))?;
        let workspace = Self::open(root)?;
        let _lock = workspace.lock()?;
        for directory in ["models", "skills", "memory", "sessions", "mcp", "cache"] {
            workspace.directory(Path::new(directory))?;
        }
        for name in ["config.md", "system.md", "preferences.md"] {
            let relative = Path::new(name);
            if !workspace.root.join(relative).exists() {
                workspace.stage(relative, None, "---\nschema: 1\n---\n".as_bytes())?;
            }
        }
        Ok(workspace)
    }

    pub fn put(
        &self,
        relative: &Path,
        expected: Option<&Path>,
        source: impl Read,
    ) -> io::Result<()> {
        let _lock = self.lock()?;
        self.stage(relative, expected, source)
    }

    pub fn remember(&self, id: &str, text: &str, expected: Option<&Path>) -> io::Result<()> {
        self.write_memory(id, text, expected, "")
    }

    /// Creates a memory from an approved model tool call, marked so recall labels it.
    #[cfg(feature = "inference")]
    pub fn remember_from_model(&self, id: &str, text: &str) -> io::Result<()> {
        self.write_memory(id, text, None, "source: model\n")
    }

    fn write_memory(
        &self,
        id: &str,
        text: &str,
        expected: Option<&Path>,
        source: &str,
    ) -> io::Result<()> {
        schema::id(id)?;
        if text.len() > schema::FILE_BYTES_MAX {
            return Err(invalid("memory.body: exceeds 1048576 bytes"));
        }
        let text = format!("---\nschema: 1\nid: {id}\n{source}---\n{text}");
        self.put(
            Path::new(&format!("memory/{id}.md")),
            expected,
            text.as_bytes(),
        )
    }

    pub fn forget(&self, id: &str, expected: &Path) -> io::Result<()> {
        schema::id(id)?;
        let relative = PathBuf::from(format!("memory/{id}.md"));
        let _lock = self.lock()?;
        self.compare(&relative, Some(expected))?;
        fs::remove_file(self.path(&relative)?).map_err(|error| at(&relative, error))
    }

    pub fn session(&self, id: &str, limit: usize) -> io::Result<Vec<sefai_core::Message>> {
        schema::id(id)?;
        let relative = PathBuf::from(format!("sessions/{id}.md"));
        let file = File::open(self.path(&relative)?)?;
        if file.metadata()?.len() > session::SESSION_BYTES_MAX {
            return Err(at(&relative, "session: exceeds 67108864 bytes"));
        }
        let (header, messages) = session::read(
            BufReader::new(file.take(session::SESSION_BYTES_MAX + 1)),
            limit,
        )
        .map_err(|error| at(&relative, error))?;
        self.identity(&relative, header.schema, &header.id)?;
        Ok(messages)
    }

    pub fn append(
        &self,
        id: &str,
        role: &str,
        text: &str,
        expected: Option<&Path>,
    ) -> io::Result<()> {
        schema::id(id)?;
        let record = session::encode(role, text)?;
        let relative = PathBuf::from(format!("sessions/{id}.md"));
        let _lock = self.lock()?;
        if self.root.join(&relative).exists() {
            let file = File::open(self.path(&relative)?)?;
            self.stage(
                &relative,
                expected,
                file.take(session::SESSION_BYTES_MAX + 1)
                    .chain(record.as_bytes()),
            )
        } else {
            let front = format!("---\nschema: 1\nid: {id}\noutcome: completed\n---\n");
            self.stage(
                &relative,
                expected,
                front.as_bytes().chain(record.as_bytes()),
            )
        }
    }

    fn stage(&self, relative: &Path, expected: Option<&Path>, source: impl Read) -> io::Result<()> {
        self.stage_checked(relative, source, || self.compare(relative, expected))
    }

    pub(super) fn stage_checked(
        &self,
        relative: &Path,
        mut source: impl Read,
        verify: impl Fn() -> io::Result<()>,
    ) -> io::Result<()> {
        super::kind(relative)?;
        let parent = relative
            .parent()
            .ok_or_else(|| at(relative, "path: missing parent"))?;
        self.directory(parent)?;
        let target = self.destination(relative)?;
        verify()?;
        let temporary = target.with_file_name(format!(
            ".sefai-{}-{}.tmp",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&temporary)?;
        let cleanup = Temporary(temporary);
        let limit = if relative.starts_with("sessions") {
            session::SESSION_BYTES_MAX
        } else {
            schema::FILE_BYTES_MAX as u64
        };
        let written = io::copy(&mut source.by_ref().take(limit + 1), &mut file)?;
        if written > limit {
            return Err(at(relative, format!("file: exceeds {limit} bytes")));
        }
        file.flush()?;
        file.sync_all()?;
        drop(file);
        self.validate_file(relative, &cleanup.0)?;
        verify()?;
        if self.destination(relative)? != target {
            return Err(at(relative, "path: changed during write"));
        }
        fs::rename(&cleanup.0, &target).map_err(|error| at(relative, error))?;
        #[cfg(unix)]
        File::open(
            target
                .parent()
                .ok_or_else(|| invalid("path: missing parent"))?,
        )?
        .sync_all()?;
        Ok(())
    }

    fn compare(&self, relative: &Path, expected: Option<&Path>) -> io::Result<()> {
        let target = self.destination(relative)?;
        match (target.exists(), expected) {
            (false, None) => Ok(()),
            (true, Some(expected)) => {
                if expected.canonicalize()? == target.canonicalize()? {
                    return Err(at(
                        relative,
                        "revision: snapshot must be a separate unchanged copy",
                    ));
                }
                let mut actual = File::open(self.path(relative)?)?;
                let mut expected = File::open(expected)?;
                let limit = if relative.starts_with("sessions") {
                    session::SESSION_BYTES_MAX
                } else {
                    schema::FILE_BYTES_MAX as u64
                };
                if actual.metadata()?.len() > limit || expected.metadata()?.len() > limit {
                    return Err(at(relative, "revision: exceeds file limit"));
                }
                let mut left = [0; 8192];
                let mut right = [0; 8192];
                for _ in 0..=limit / 8192 {
                    let length = read_chunk(&mut actual, &mut left)?;
                    let expected_length = read_chunk(&mut expected, &mut right)?;
                    if length != expected_length || left[..length] != right[..expected_length] {
                        return Err(at(relative, "conflict: changed since snapshot"));
                    }
                    if length == 0 {
                        return Ok(());
                    }
                }
                Err(at(relative, "revision: file grew beyond limit"))
            }
            _ => Err(at(
                relative,
                "conflict: create requires absent file; update requires exact --expected snapshot",
            )),
        }
    }

    pub(super) fn validate_file(&self, relative: &Path, source: &Path) -> io::Result<()> {
        if super::kind(relative)? == "sessions" {
            let file = File::open(source)?;
            if file.metadata()?.len() > session::SESSION_BYTES_MAX {
                return Err(at(relative, "session: exceeds 67108864 bytes"));
            }
            let (header, _) =
                session::read(BufReader::new(file.take(session::SESSION_BYTES_MAX + 1)), 1)
                    .map_err(|error| at(relative, error))?;
            let _outcome = header.outcome;
            return self.identity(relative, header.schema, &header.id);
        }
        let mut text = String::new();
        File::open(source)?
            .take(schema::FILE_BYTES_MAX as u64 + 1)
            .read_to_string(&mut text)?;
        if text.len() > schema::FILE_BYTES_MAX {
            return Err(at(relative, "file: exceeds 1048576 bytes"));
        }
        self.validate_document(relative, &text)
            .map_err(|error| at(relative, error))
    }

    fn destination(&self, relative: &Path) -> io::Result<PathBuf> {
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(at(relative, "path: expected a workspace-relative file"));
        }
        let mut path = self.root.clone();
        for component in relative.components() {
            path.push(component);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(at(relative, "path: symlinks are not writable"));
                }
                Ok(_) => (),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(at(relative, error)),
            }
        }
        Ok(path)
    }

    fn directory(&self, relative: &Path) -> io::Result<()> {
        let path = if relative.as_os_str().is_empty() {
            self.root.clone()
        } else {
            self.destination(relative)?
        };
        fs::create_dir_all(&path).map_err(|error| at(relative, error))
    }

    pub(super) fn lock(&self) -> io::Result<File> {
        let path = self.destination(Path::new(".sefai.lock"))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock()
            .map_err(|error| at(".sefai.lock", format!("workspace busy: {error}")))?;
        Ok(file)
    }
}

fn read_chunk(input: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    let mut count = 0;
    while count < buffer.len() {
        match input.read(&mut buffer[count..]) {
            Ok(0) => break,
            Ok(length) => count += length,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(count)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
