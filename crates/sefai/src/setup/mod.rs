//! Hardware-aware setup: an inspectable hardware snapshot, offline artifact checks and memory
//! estimates. Nothing here loads a model, downloads a file or changes a profile; those remain
//! explicit actions.

#[cfg(feature = "download")]
mod acquire;
mod catalog;
mod command;
mod estimate;
mod gguf;
mod hardware;

pub use command::{Action, report};
use hardware::snapshot;

use crate::workspace::Format;
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

const CONFIG_BYTES_MAX: u64 = 1024 * 1024;

/// The attention shape that decides KV cache size; weights come from file sizes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub architecture: String,
    pub layers: u64,
    /// The model's trained context, when the artifact declares one.
    pub context: Option<u64>,
    pub heads: u64,
    pub kv_heads: u64,
    pub head_dim: u64,
}

/// What an offline check learns about one artifact without loading it.
#[derive(Clone, Debug)]
pub struct Artifact {
    pub path: PathBuf,
    pub format: Format,
    pub shape: Shape,
    /// Bytes of weight files: the GGUF file, safetensors shards, or ONNX graph and external data.
    pub weight_bytes: u64,
    /// Bytes per KV cache element, from the declared activation dtype; GGUF uses a 16-bit cache.
    pub cache_element_bytes: u64,
}

/// Reads an artifact's shape and weight size from metadata alone.
///
/// # Errors
/// Fails for missing or oversized metadata, a shape without layer or head counts, and a
/// bundle with no weight files.
pub fn inspect(path: &Path, format: Format) -> io::Result<Artifact> {
    let (shape, weight_bytes, cache_element_bytes) = match format {
        Format::Gguf => (gguf::read(path)?, path.metadata()?.len(), 2),
        Format::Mlx | Format::Gptq | Format::Awq => {
            let config = config(&path.join("config.json"))?;
            let weights = sum_files(path, |name| name.ends_with(".safetensors"))?;
            (shape(&config)?, weights, dtype_bytes(&config))
        }
        Format::Onnx => {
            let directory = path.parent().unwrap_or(Path::new("."));
            // Optimum exports keep config.json beside the onnx directory; GenAI exports beside the graph.
            let config_path = [
                directory.join("config.json"),
                directory.join("../config.json"),
            ]
            .into_iter()
            .find(|candidate| candidate.is_file())
            .ok_or_else(|| invalid("onnx export: config.json not found beside the graph"))?;
            let config = config(&config_path)?;
            let stem = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let weights = sum_files(directory, |name| {
                name == stem
                    || name.starts_with(&format!("{stem}_data"))
                    || name.starts_with(&format!("{stem}.data"))
            })?;
            // The cache dtype lives in the graph, which setup does not parse; 32 bits bounds it.
            (shape(&config)?, weights, 4)
        }
    };
    if weight_bytes == 0 {
        return Err(invalid("artifact has no weight files"));
    }
    Ok(Artifact {
        path: path.to_owned(),
        format,
        shape,
        weight_bytes,
        cache_element_bytes,
    })
}

fn config(path: &Path) -> io::Result<serde_json::Value> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(CONFIG_BYTES_MAX + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > CONFIG_BYTES_MAX {
        return Err(invalid("config.json exceeds 1 mib"));
    }
    serde_json::from_slice(&bytes).map_err(|error| invalid(format!("config.json: {error}")))
}

fn shape(config: &serde_json::Value) -> io::Result<Shape> {
    let number = |name: &str| config[name].as_u64().filter(|value| *value > 0);
    let layers = number("num_hidden_layers")
        .ok_or_else(|| invalid("config.json: missing num_hidden_layers"))?;
    let heads = number("num_attention_heads")
        .ok_or_else(|| invalid("config.json: missing num_attention_heads"))?;
    let hidden =
        number("hidden_size").ok_or_else(|| invalid("config.json: missing hidden_size"))?;
    Ok(Shape {
        architecture: config["model_type"]
            .as_str()
            .unwrap_or("unknown")
            .to_owned(),
        layers,
        context: number("max_position_embeddings"),
        heads,
        kv_heads: number("num_key_value_heads").unwrap_or(heads),
        head_dim: number("head_dim").unwrap_or(hidden / heads),
    })
}

fn dtype_bytes(config: &serde_json::Value) -> u64 {
    match config["torch_dtype"].as_str().or(config["dtype"].as_str()) {
        Some("float32") => 4,
        _ => 2,
    }
}

fn sum_files(directory: &Path, matches: impl Fn(&str) -> bool) -> io::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(directory)?.take(4096) {
        let entry = entry?;
        let name = entry.file_name();
        // Follow symbolic links, as loaders do, so linked weights are counted.
        let metadata = std::fs::metadata(entry.path())?;
        if matches(&name.to_string_lossy()) && metadata.is_file() {
            total = total.saturating_add(metadata.len());
        }
    }
    Ok(total)
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}
