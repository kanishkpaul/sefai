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
    pub device: Option<usize>,
    pub context: Option<u32>,
    pub batch: Option<u32>,
    pub microbatch: Option<u32>,
    pub output: Option<u32>,
    pub threads: Option<u32>,
}

#[derive(Clone, Copy, Debug, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Cpu,
    Metal,
    Cuda,
    Vulkan,
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Gguf,
    Onnx,
    Gptq,
    Awq,
    Mlx,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mcp {
    pub schema: u32,
    pub id: String,
    pub transport: Transport,
}

#[derive(Debug, Deserialize)]
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
    },
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
