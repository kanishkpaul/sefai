use serde_json::Value;
use std::{
    fs::File,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

const METADATA_BYTES_MAX: u64 = 64 * 1024 * 1024;
// Protobuf caps a self-contained graph at 2 GiB; larger weights use external data.
const GRAPH_BYTES_MAX: u64 = 2 * 1024 * 1024 * 1024;

/// Decoder dimensions and assets admitted before ONNX Runtime opens the graph.
#[derive(Debug)]
pub struct Bundle {
    /// Canonical graph path; ONNX Runtime resolves external data beside it.
    pub graph: PathBuf,
    /// Canonical directory holding the configuration and tokenizer.
    pub root: PathBuf,
    pub context: u32,
    pub layers: usize,
    pub kv_heads: usize,
    pub head_dim: usize,
    pub vocab: usize,
    /// Sorted, deduplicated end-of-generation token IDs.
    pub eos: Vec<u32>,
    /// A GenAI export whose present outputs are written in place into the past inputs.
    pub shared: bool,
}

/// Validates a local decoder graph and its bounded metadata.
/// Metadata is read beside the graph, or from the parent of an `onnx` directory.
/// A `genai_config.json` beside the graph selects the ONNX Runtime GenAI share-buffer layout.
/// Model files must remain unchanged until the model is unloaded.
///
/// # Errors
/// Rejects missing assets, unsupported architectures, escaping paths and limits.
pub fn validate(path: &Path) -> io::Result<Bundle> {
    let graph = path.canonicalize()?;
    let metadata = graph.metadata()?;
    if !metadata.is_file()
        || graph
            .extension()
            .is_none_or(|extension| extension != "onnx")
    {
        return Err(invalid("onnx model must be a .onnx graph file"));
    }
    if metadata.len() < 8 || metadata.len() > GRAPH_BYTES_MAX {
        return Err(invalid("onnx graph must be 8 bytes..2 gib"));
    }
    let directory = graph
        .parent()
        .ok_or_else(|| invalid("onnx graph has no directory"))?;
    let root = if directory.join("config.json").exists() {
        directory.to_path_buf()
    } else if directory.file_name().is_some_and(|name| name == "onnx") {
        directory
            .parent()
            .ok_or_else(|| invalid("onnx graph has no bundle directory"))?
            .to_path_buf()
    } else {
        return Err(invalid(
            "onnx bundle requires config.json beside the graph or its onnx directory",
        ));
    };
    let config = json(&root, "config.json")?;
    if !matches!(config["model_type"].as_str(), Some("llama" | "qwen3")) {
        return Err(invalid(
            "onnx supports only llama and qwen3 decoder exports",
        ));
    }
    let dimension = |name: &str, limit: u64| {
        config[name]
            .as_u64()
            .filter(|n| (1..=limit).contains(n))
            .map(|n| n as usize)
            .ok_or_else(|| invalid(format!("onnx config {name} is missing or exceeds {limit}")))
    };
    let layers = dimension("num_hidden_layers", 128)?;
    let heads = dimension("num_attention_heads", 256)?;
    let hidden = dimension("hidden_size", 32768)?;
    let vocab = dimension("vocab_size", 262144)?;
    let kv_heads = if config["num_key_value_heads"].is_null() {
        heads
    } else {
        dimension("num_key_value_heads", 256)?
    };
    let head_dim = if config["head_dim"].is_null() {
        if hidden % heads != 0 {
            return Err(invalid(
                "onnx hidden_size must divide evenly across attention heads",
            ));
        }
        hidden / heads
    } else {
        dimension("head_dim", 1024)?
    };
    if heads % kv_heads != 0 {
        return Err(invalid(
            "onnx attention heads must be a multiple of key-value heads",
        ));
    }
    let context = config["max_position_embeddings"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("onnx config requires max_position_embeddings"))?
        .min(sefai_core::CONTEXT_TOKENS_MAX.into()) as u32;
    let shared = root.join("genai_config.json").exists();
    let (context, mut eos) = if shared {
        let genai = json(&root, "genai_config.json")?;
        let decoder = &genai["model"]["decoder"];
        if decoder["filename"].as_str() != graph.file_name().and_then(|name| name.to_str()) {
            return Err(invalid(
                "onnx genai_config.json names a different decoder graph",
            ));
        }
        for (field, name) in [
            ("input_ids", "input_ids"),
            ("attention_mask", "attention_mask"),
            ("past_key_names", "past_key_values.%d.key"),
            ("past_value_names", "past_key_values.%d.value"),
        ] {
            if decoder["inputs"][field].as_str() != Some(name) {
                return Err(invalid(format!("onnx genai input {field} must be {name}")));
            }
        }
        for (field, name) in [
            ("logits", "logits"),
            ("present_key_names", "present.%d.key"),
            ("present_value_names", "present.%d.value"),
        ] {
            if decoder["outputs"][field].as_str() != Some(name) {
                return Err(invalid(format!("onnx genai output {field} must be {name}")));
            }
        }
        if genai["search"]["past_present_share_buffer"].as_bool() != Some(true) {
            return Err(invalid(
                "onnx genai exports must declare past_present_share_buffer",
            ));
        }
        let length = genai["model"]["context_length"]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or_else(|| invalid("onnx genai_config.json requires context_length"))?;
        (
            context.min(length.min(u64::from(u32::MAX)) as u32),
            token_ids(&genai["model"]["eos_token_id"])?,
        )
    } else {
        let generation = json(&root, "generation_config.json")?;
        (context, token_ids(&generation["eos_token_id"])?)
    };
    if eos.is_empty() {
        eos = token_ids(&config["eos_token_id"])?;
    }
    if eos.is_empty() || eos.iter().any(|id| *id as usize >= vocab) {
        return Err(invalid(
            "onnx bundle requires eos_token_id within the vocabulary",
        ));
    }
    json(&root, "tokenizer.json")?;
    json(&root, "tokenizer_config.json")?;
    Ok(Bundle {
        graph,
        root,
        context,
        layers,
        kv_heads,
        head_dim,
        vocab,
        eos,
        shared,
    })
}

fn token_ids(value: &Value) -> io::Result<Vec<u32>> {
    let values = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Array(values) => values.as_slice(),
        _ => std::slice::from_ref(value),
    };
    if values.len() > 64 {
        return Err(invalid("onnx eos_token_id exceeds 64 entries"));
    }
    let mut ids = values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|id| u32::try_from(id).ok())
                .ok_or_else(|| invalid("onnx eos_token_id must be token ids"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
}

pub(crate) fn asset(root: &Path, name: &str) -> io::Result<File> {
    let path = Path::new(name);
    if name.contains(['\\', ':'])
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(invalid("onnx asset must be a top-level filename"));
    }
    let path = root.join(path).canonicalize()?;
    if path.parent() != Some(root) {
        return Err(invalid("onnx asset escapes bundle"));
    }
    if !path.metadata()?.is_file() {
        return Err(invalid("onnx asset must be a regular file"));
    }
    File::open(path)
}

/// Reads one bounded metadata file from the bundle directory.
pub(crate) fn read(root: &Path, name: &str) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    asset(root, name)?
        .take(METADATA_BYTES_MAX + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_BYTES_MAX {
        return Err(invalid("onnx metadata exceeds 64 mib"));
    }
    Ok(bytes)
}

pub(crate) fn json(root: &Path, name: &str) -> io::Result<Value> {
    serde_json::from_slice(&read(root, name)?)
        .map_err(|error| invalid(format!("onnx {name}: {error}")))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "sefai-onnx-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("onnx")).expect("unique fixture");
            fs::write(path.join("config.json"), r#"{"model_type":"llama","num_hidden_layers":2,"hidden_size":64,"vocab_size":256,"num_attention_heads":4,"num_key_value_heads":2,"max_position_embeddings":2048}"#).expect("config");
            fs::write(
                path.join("generation_config.json"),
                r#"{"eos_token_id":[2,1,2]}"#,
            )
            .expect("generation config");
            for name in ["tokenizer.json", "tokenizer_config.json"] {
                fs::write(path.join(name), "{}").expect("metadata");
            }
            fs::write(path.join("onnx/model.onnx"), [0; 8]).expect("graph");
            Self(path)
        }
        fn graph(&self) -> PathBuf {
            self.0.join("onnx/model.onnx")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove fixture");
        }
    }
    #[test]
    fn optimum_layout_passes_admission() {
        let fixture = Fixture::new();
        let bundle = validate(&fixture.graph()).expect("bundle");
        assert_eq!(
            (
                bundle.context,
                bundle.layers,
                bundle.kv_heads,
                bundle.head_dim,
                bundle.eos
            ),
            (2048, 2, 2, 16, vec![1, 2])
        );
        assert!(!bundle.shared);
    }
    #[test]
    fn graph_beside_metadata_passes_admission() {
        let fixture = Fixture::new();
        fs::rename(fixture.graph(), fixture.0.join("model.onnx")).expect("move graph");
        assert!(validate(&fixture.0.join("model.onnx")).is_ok());
    }
    fn genai(fixture: &Fixture, share: bool) -> PathBuf {
        let graph = fixture.0.join("model.onnx");
        fs::rename(fixture.graph(), &graph).expect("move graph");
        fs::remove_file(fixture.0.join("generation_config.json"))
            .expect("remove generation config");
        fs::write(
            fixture.0.join("genai_config.json"),
            format!(r#"{{"model":{{"context_length":1024,"eos_token_id":[3],"decoder":{{"filename":"model.onnx","inputs":{{"input_ids":"input_ids","attention_mask":"attention_mask","past_key_names":"past_key_values.%d.key","past_value_names":"past_key_values.%d.value"}},"outputs":{{"logits":"logits","present_key_names":"present.%d.key","present_value_names":"present.%d.value"}}}}}},"search":{{"past_present_share_buffer":{share}}}}}"#),
        )
        .expect("genai config");
        graph
    }
    #[test]
    fn genai_share_buffer_export_passes_admission() {
        let fixture = Fixture::new();
        let bundle = validate(&genai(&fixture, true)).expect("bundle");
        assert_eq!(
            (bundle.shared, bundle.context, bundle.eos),
            (true, 1024, vec![3])
        );
    }
    #[test]
    fn genai_export_without_shared_buffer_is_rejected() {
        let fixture = Fixture::new();
        assert!(validate(&genai(&fixture, false)).is_err());
    }
    #[test]
    fn bundle_paths_cannot_escape() {
        for name in [
            "../config.json",
            "/config.json",
            "a/b.json",
            "a\\b",
            "C:config",
        ] {
            assert!(asset(Path::new("."), name).is_err());
        }
    }
    #[test]
    fn directory_is_not_a_graph() {
        let fixture = Fixture::new();
        assert!(validate(&fixture.0).is_err());
    }
    #[test]
    fn unsupported_architecture_is_rejected() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("config.json"), r#"{"model_type":"whisper"}"#).expect("config");
        assert!(validate(&fixture.graph()).is_err());
    }
    #[test]
    fn missing_eos_is_rejected() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("generation_config.json"), "{}").expect("generation config");
        assert!(validate(&fixture.graph()).is_err());
    }
    #[test]
    fn indivisible_key_value_heads_are_rejected() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("config.json"), r#"{"model_type":"llama","num_hidden_layers":2,"hidden_size":64,"vocab_size":256,"num_attention_heads":4,"num_key_value_heads":3,"max_position_embeddings":2048}"#).expect("config");
        assert!(validate(&fixture.graph()).is_err());
    }
    #[test]
    fn oversized_metadata_is_rejected_before_parsing() {
        let fixture = Fixture::new();
        File::create(fixture.0.join("tokenizer.json"))
            .expect("tokenizer")
            .set_len(METADATA_BYTES_MAX + 1)
            .expect("sparse file");
        assert!(validate(&fixture.graph()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_metadata_cannot_escape() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.0.join("tokenizer.json")).expect("remove tokenizer");
        std::os::unix::fs::symlink("/dev/null", fixture.0.join("tokenizer.json")).expect("symlink");
        assert!(validate(&fixture.graph()).is_err());
    }
}
