use serde::{Deserialize, de::DeserializeOwned};
use std::{io, path::PathBuf};

pub const FILE_BYTES_MAX: usize = 1024 * 1024;
pub const FRONT_BYTES_MAX: usize = 16 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: u32,
    pub model: Option<String>,
    #[serde(default)]
    pub runner: Runner,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Runner {
    pub backend: Option<Backend>,
    pub provider: Option<Provider>,
    pub device: Option<usize>,
    pub context: Option<u32>,
    pub batch: Option<u32>,
    pub microbatch: Option<u32>,
    pub output: Option<u32>,
    pub threads: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Mlx,
    Onnx,
    Cpu,
    Metal,
    Cuda,
    Vulkan,
}

impl Backend {
    /// The lowercase name used in profiles and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mlx => "mlx",
            Self::Onnx => "onnx",
            Self::Cpu => "cpu",
            Self::Metal => "metal",
            Self::Cuda => "cuda",
            Self::Vulkan => "vulkan",
        }
    }
}

/// ONNX Runtime execution provider; valid only with backend onnx.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Cpu,
    Webgpu,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub schema: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    pub schema: u32,
    pub id: String,
    pub path: PathBuf,
    pub format: Format,
    #[serde(default)]
    pub runner: Runner,
}

/// A model artifact's declared format; GPTQ and AWQ run through the MLX engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Gguf,
    Onnx,
    Gptq,
    Awq,
    Mlx,
}

#[cfg(feature = "inference")]
impl Format {
    /// The lowercase name used in profiles and on the command line.
    pub fn name(self) -> &'static str {
        match self {
            Self::Gguf => "gguf",
            Self::Onnx => "onnx",
            Self::Gptq => "gptq",
            Self::Awq => "awq",
            Self::Mlx => "mlx",
        }
    }
}

/// One pinned model in the offline setup catalog; it describes an artifact and grants nothing.
/// Builds without inference validate catalog files in `workspace check` but never use them.
#[cfg_attr(not(feature = "inference"), allow(dead_code))]
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogEntry {
    pub schema: u32,
    pub id: String,
    /// Hugging Face `owner/name`.
    pub repository: String,
    /// The immutable 40-character commit the files are pinned to.
    pub revision: String,
    pub format: Format,
    /// The path to load, relative to the acquired directory; `.` for a bundle directory.
    pub entry: String,
    pub parameters_millions: u32,
    pub purposes: Vec<Purpose>,
    pub license: String,
    pub layers: u32,
    pub kv_heads: u32,
    pub head_dim: u32,
    pub trained_context: u32,
    pub cache_element_bytes: u32,
    /// A repository document recording this entry's qualification.
    pub evidence: String,
    pub files: Vec<CatalogFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

/// What a catalog model has been qualified for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    Chat,
    Tools,
}

impl CatalogEntry {
    /// Checks pinned identity, safe relative paths, hashes and bounds.
    pub fn validate(&self) -> io::Result<()> {
        version(self.schema)?;
        id(&self.id)?;
        let mut parts = self.repository.split('/');
        let portable = |part: &str| {
            !part.is_empty()
                && part.len() <= 96
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
                && !part.starts_with('.')
        };
        if !(parts.next().is_some_and(portable)
            && parts.next().is_some_and(portable)
            && parts.next().is_none())
        {
            return Err(invalid("repository: expected owner/name"));
        }
        if !hex(&self.revision, 40) {
            return Err(invalid("revision: expected a 40-character commit"));
        }
        if self.files.is_empty() || self.files.len() > 256 {
            return Err(invalid("files: expected 1..256 entries"));
        }
        for file in &self.files {
            relative(&file.path)?;
            if !hex(&file.sha256, 64) || file.bytes == 0 {
                return Err(invalid("files: each file needs bytes and a sha256"));
            }
        }
        if self.entry != "." {
            relative(&self.entry)?;
            if !self.files.iter().any(|file| file.path == self.entry) {
                return Err(invalid("entry: must be . or a listed file"));
            }
        }
        if self.purposes.is_empty()
            || [
                self.layers,
                self.kv_heads,
                self.head_dim,
                self.trained_context,
            ]
            .contains(&0)
            || !matches!(self.cache_element_bytes, 1 | 2 | 4)
        {
            return Err(invalid(
                "shape: layers, heads, context and cache element bytes are required",
            ));
        }
        Ok(())
    }

    #[cfg(feature = "inference")]
    pub fn weight_bytes(&self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// A forward-slash path of plain components that cannot leave its directory.
fn relative(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > 512
        || value.contains(['\\', ':', '\0'])
        || value
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid(
            "path: expected a relative path of plain components",
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Skill {
    pub schema: u32,
    pub id: String,
    #[serde(default)]
    pub resources: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memory {
    pub schema: u32,
    pub id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub schema: u32,
    pub id: String,
    pub outcome: Outcome,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Completed,
    Interrupted,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mcp {
    pub schema: u32,
    pub id: String,
    pub transport: Transport,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Transport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: std::collections::BTreeMap<String, String>,
    },
    Http {
        url: String,
        token_env: Option<String>,
        oauth: Option<OAuth>,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OAuth {
    pub issuer: String,
    pub client_id: String,
    pub redirect_uri: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub fn parse<T: DeserializeOwned>(text: &str) -> io::Result<(T, &str)> {
    let mut lines = text.split_inclusive('\n');
    if lines.next().map(str::trim_end) != Some("---") {
        return Err(invalid("front matter: expected opening ---"));
    }
    let start = text.find('\n').unwrap_or(text.len()) + 1;
    let mut end = start;
    for line in lines {
        if end > FRONT_BYTES_MAX {
            return Err(invalid("front matter: exceeds 16384 bytes"));
        }
        if line.trim_end_matches(['\r', '\n']) == "---" {
            let options = serde_saphyr::options! {
                reject_unsupported_tags: true,
                merge_keys: serde_saphyr::MergeKeyPolicy::Error,
                no_schema: true,
                budget: serde_saphyr::budget! {
                    max_depth: 8, max_nodes: 1024, max_events: 4096,
                    max_aliases: 0, max_anchors: 0, max_documents: 1,
                    max_total_scalar_bytes: FRONT_BYTES_MAX,
                },
            };
            let header = serde_saphyr::from_str_with_options(&text[start..end], options)
                .map_err(|error| invalid(format!("front matter: {error}")))?;
            return Ok((header, &text[end + line.len()..]));
        }
        end += line.len();
    }
    Err(invalid("front matter: missing closing ---"))
}

pub fn version(schema: u32) -> io::Result<()> {
    if schema != 1 {
        return Err(invalid("schema: unsupported version; expected 1"));
    }
    Ok(())
}

pub fn id(value: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || matches!(value, "con" | "prn" | "aux" | "nul")
        || (value.len() == 4
            && (value.starts_with("com") || value.starts_with("lpt"))
            && value.as_bytes()[3].is_ascii_digit())
    {
        return Err(invalid(
            "id: expected a portable lowercase name of 1..64 bytes",
        ));
    }
    Ok(())
}

impl Runner {
    pub fn overlay(&mut self, other: &Self) {
        self.backend = other.backend.or(self.backend);
        self.provider = other.provider.or(self.provider);
        if matches!(
            other.backend,
            Some(Backend::Cpu | Backend::Mlx | Backend::Onnx)
        ) {
            self.device = None;
        }
        self.device = other.device.or(self.device);
        self.context = other.context.or(self.context);
        self.batch = other.batch.or(self.batch);
        self.microbatch = other.microbatch.or(self.microbatch);
        self.output = other.output.or(self.output);
        self.threads = other.threads.or(self.threads);
    }

    pub fn validate(&self) -> io::Result<()> {
        for (field, value, min, max) in [
            ("context", self.context, 1, 131072),
            ("batch", self.batch, 1, 4096),
            ("microbatch", self.microbatch, 1, 4096),
            ("output", self.output, 0, 131072),
            ("threads", self.threads, 1, 256),
        ] {
            if value.is_some_and(|value| !(min..=max).contains(&value)) {
                return Err(invalid(format!("runner.{field}: expected {min}..={max}")));
            }
        }
        Ok(())
    }
}

pub fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
