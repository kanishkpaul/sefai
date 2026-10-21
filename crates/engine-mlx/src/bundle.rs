use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read},
    path::{Component, Path},
};

const METADATA_BYTES_MAX: u64 = 64 * 1024 * 1024;
const MODEL_BYTES_MAX: u64 = 1024 * 1024 * 1024 * 1024;

/// Validates bounded, local bundle assets before native loading.
/// Model files must remain unchanged until the model is unloaded.
///
/// # Errors
/// Rejects missing assets, unsupported architectures, escaping paths and limits.
pub fn validate(path: &Path) -> io::Result<u32> {
    let root = path.canonicalize()?;
    if !root.is_dir() {
        return Err(invalid("mlx model must be a bundle directory"));
    }
    let config = json(&root, "config.json")?;
    if !matches!(config["model_type"].as_str(), Some("llama" | "qwen3")) {
        return Err(invalid("mlx supports only llama and qwen3 decoder bundles"));
    }
    for (name, limit) in [
        ("num_hidden_layers", 128),
        ("hidden_size", 32768),
        ("intermediate_size", 131072),
        ("vocab_size", 262144),
        ("num_attention_heads", 256),
    ] {
        if config[name].as_u64().is_none_or(|n| n == 0 || n > limit) {
            return Err(invalid(format!(
                "mlx config {name} is missing or exceeds {limit}"
            )));
        }
    }
    let context = config["max_position_embeddings"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or_else(|| invalid("mlx config requires max_position_embeddings"))?
        .min(131072) as u32;
    json(&root, "tokenizer.json")?;
    json(&root, "tokenizer_config.json")?;
    if root.join("generation_config.json").exists() {
        json(&root, "generation_config.json")?;
    }
    let mut shards = BTreeSet::new();
    if root.join("model.safetensors.index.json").exists() {
        let index = json(&root, "model.safetensors.index.json")?;
        let map = index["weight_map"]
            .as_object()
            .ok_or_else(|| invalid("mlx index requires weight_map"))?;
        if map.is_empty() || map.len() > 65536 {
            return Err(invalid("mlx weight map must contain 1..65536 entries"));
        }
        for value in map.values() {
            shards.insert(
                value
                    .as_str()
                    .ok_or_else(|| invalid("mlx shard must be a filename"))?
                    .to_owned(),
            );
        }
    } else {
        shards.insert("model.safetensors".into());
    }
    if shards.len() > 128 {
        return Err(invalid("mlx bundle exceeds 128 shards"));
    }
    let mut total = 0u64;
    for name in shards {
        if !name.ends_with(".safetensors") {
            return Err(invalid("mlx shard must use safetensors"));
        }
        let file = asset(&root, &name)?;
        let bytes = file.metadata()?.len();
        total = total
            .checked_add(bytes)
            .ok_or_else(|| invalid("mlx bundle size overflow"))?;
        if bytes < 8 || total > MODEL_BYTES_MAX {
            return Err(invalid("mlx weights must be 8 bytes..1 tib total"));
        }
    }
    Ok(context)
}

fn asset(root: &Path, name: &str) -> io::Result<File> {
    let path = Path::new(name);
    if name.contains(['\\', ':'])
        || path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return Err(invalid("mlx asset must be a top-level filename"));
    }
    let path = root.join(path).canonicalize()?;
    if path.parent() != Some(root) {
        return Err(invalid("mlx asset escapes bundle"));
    }
    if !path.metadata()?.is_file() {
        return Err(invalid("mlx asset must be a regular file"));
    }
    File::open(path)
}

fn json(root: &Path, name: &str) -> io::Result<Value> {
    let mut bytes = Vec::new();
    asset(root, name)?
        .take(METADATA_BYTES_MAX + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > METADATA_BYTES_MAX {
        return Err(invalid("mlx metadata exceeds 64 mib"));
    }
    serde_json::from_slice(&bytes).map_err(|error| invalid(format!("mlx {name}: {error}")))
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
    struct Fixture(std::path::PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "sefai-mlx-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("unique fixture");
            fs::write(path.join("config.json"), r#"{"model_type":"llama","num_hidden_layers":1,"hidden_size":64,"intermediate_size":128,"vocab_size":256,"num_attention_heads":2,"max_position_embeddings":2048}"#).expect("config");
            for name in ["tokenizer.json", "tokenizer_config.json"] {
                fs::write(path.join(name), "{}").expect("metadata");
            }
            fs::write(path.join("model.safetensors"), [0; 8]).expect("weights");
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove fixture");
        }
    }
    #[test]
    fn bundle_paths_cannot_escape() {
        for name in [
            "../config.json",
            "/config.json",
            "a/b.safetensors",
            "a\\b",
            "C:weights",
        ] {
            assert!(asset(Path::new("."), name).is_err());
        }
    }
    #[test]
    fn missing_bundle_is_rejected() {
        assert!(validate(Path::new("/sefai-missing-model-bundle")).is_err());
    }
    #[test]
    fn complete_local_bundle_passes_admission() {
        let fixture = Fixture::new();
        assert_eq!(validate(&fixture.0).expect("bundle"), 2048);
    }
    #[test]
    fn unsupported_architecture_is_rejected() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("config.json"),
            r#"{"model_type":"qwen3_moe"}"#,
        )
        .expect("config");
        assert!(validate(&fixture.0).is_err());
    }
    #[test]
    fn missing_shard_is_rejected() {
        let fixture = Fixture::new();
        fs::write(
            fixture.0.join("model.safetensors.index.json"),
            r#"{"weight_map":{"weight":"missing.safetensors"}}"#,
        )
        .expect("index");
        assert!(validate(&fixture.0).is_err());
    }
    #[test]
    fn oversized_metadata_is_rejected_before_parsing() {
        let fixture = Fixture::new();
        File::create(fixture.0.join("tokenizer.json"))
            .expect("tokenizer")
            .set_len(METADATA_BYTES_MAX + 1)
            .expect("sparse file");
        assert!(validate(&fixture.0).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_metadata_cannot_escape() {
        let fixture = Fixture::new();
        fs::remove_file(fixture.0.join("tokenizer.json")).expect("remove tokenizer");
        std::os::unix::fs::symlink("/dev/null", fixture.0.join("tokenizer.json")).expect("symlink");
        assert!(validate(&fixture.0).is_err());
    }
}
