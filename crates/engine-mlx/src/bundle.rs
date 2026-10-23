use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read},
    path::{Component, Path},
};

const METADATA_BYTES_MAX: u64 = 64 * 1024 * 1024;
const MODEL_BYTES_MAX: u64 = 1024 * 1024 * 1024 * 1024;

/// An admitted bundle's context limit and weight storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bundle {
    pub context: u32,
    pub weights: Weights,
}

/// How a bundle stores its quantized linear weights.
/// GPTQ and AWQ groups are repacked into MLX affine groups in memory at load; files are never rewritten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weights {
    /// MLX floating or affine-quantized safetensors.
    Mlx,
    /// GPTQ int32 packing along the input dimension.
    /// Version 1 checkpoints store each zero point minus one.
    Gptq {
        bits: u8,
        group_size: u32,
        zero_offset: bool,
    },
    /// AutoAWQ GEMM int32 packing along the output dimension, with zero points.
    Awq { bits: u8, group_size: u32 },
}

/// Validates bounded, local bundle assets before native loading.
/// Model files must remain unchanged until the model is unloaded.
///
/// # Errors
/// Rejects missing assets, unsupported architectures or quantization, escaping paths and limits.
pub fn validate(path: &Path) -> io::Result<Bundle> {
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
    let weights = weights(&config)?;
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
    Ok(Bundle { context, weights })
}

/// Admits only GPTQ and AWQ layouts that map exactly onto MLX affine groups.
/// MLX's own `quantization` metadata is validated by the native loader.
fn weights(config: &Value) -> io::Result<Weights> {
    let declared = &config["quantization_config"];
    let method = declared["quant_method"].as_str();
    // MLX bundles may repeat their affine settings here without a method.
    if declared.is_null() || method.is_none() && !config["quantization"].is_null() {
        return Ok(Weights::Mlx);
    }
    if !config["quantization"].is_null() {
        return Err(invalid(
            "mlx config declares both mlx quantization and a quantization_config method",
        ));
    }
    let group_size = match declared["group_size"].as_u64() {
        Some(size @ (32 | 64 | 128)) => size as u32,
        _ => {
            return Err(invalid(
                "quantization_config group_size must be 32, 64 or 128 to map onto mlx affine groups",
            ));
        }
    };
    let bits = declared["bits"].as_u64();
    match method {
        Some("gptq") => {
            let bits = match bits {
                Some(bits @ (4 | 8)) => bits as u8,
                _ => return Err(invalid("gptq bits must be 4 or 8")),
            };
            if declared["desc_act"].as_bool() == Some(true) {
                return Err(invalid(
                    "gptq act-order (desc_act) reorders groups and cannot map onto mlx affine groups",
                ));
            }
            if declared["lm_head"].as_bool() == Some(true) {
                return Err(invalid("gptq quantized lm_head is not supported"));
            }
            if !matches!(declared["pack_dtype"].as_str(), None | Some("int32")) {
                return Err(invalid("gptq pack_dtype must be int32"));
            }
            let zero_offset = match declared["checkpoint_format"].as_str() {
                None | Some("gptq") => true,
                Some("gptq_v2") => false,
                Some(other) => {
                    return Err(invalid(format!(
                        "gptq checkpoint_format {other} is not supported; expected gptq or gptq_v2"
                    )));
                }
            };
            Ok(Weights::Gptq {
                bits,
                group_size,
                zero_offset,
            })
        }
        Some("awq") => {
            if bits != Some(4) {
                return Err(invalid("awq bits must be 4"));
            }
            if !declared["version"]
                .as_str()
                .is_some_and(|version| version.eq_ignore_ascii_case("gemm"))
            {
                return Err(invalid("awq version must be gemm"));
            }
            if declared["zero_point"].as_bool() != Some(true) {
                return Err(invalid(
                    "awq checkpoints without zero points are not supported",
                ));
            }
            Ok(Weights::Awq {
                bits: 4,
                group_size,
            })
        }
        Some(method) => Err(invalid(format!(
            "quantization method {method} is not supported; expected gptq or awq"
        ))),
        None => Err(invalid("quantization_config requires quant_method")),
    }
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
        assert_eq!(
            validate(&fixture.0).expect("bundle"),
            Bundle {
                context: 2048,
                weights: Weights::Mlx
            }
        );
    }
    fn quantized(declared: serde_json::Value) -> io::Result<Weights> {
        weights(&serde_json::json!({ "quantization_config": declared }))
    }
    #[test]
    fn gptq_and_awq_layouts_that_map_onto_mlx_are_admitted() {
        assert_eq!(
            quantized(serde_json::json!({"quant_method": "gptq", "bits": 4, "group_size": 128, "desc_act": false, "sym": true})).expect("gptq"),
            Weights::Gptq { bits: 4, group_size: 128, zero_offset: true }
        );
        assert_eq!(
            quantized(serde_json::json!({"quant_method": "gptq", "bits": 8, "group_size": 64, "checkpoint_format": "gptq_v2"})).expect("gptq v2"),
            Weights::Gptq { bits: 8, group_size: 64, zero_offset: false }
        );
        assert_eq!(
            quantized(serde_json::json!({"quant_method": "awq", "bits": 4, "group_size": 32, "version": "GEMM", "zero_point": true})).expect("awq"),
            Weights::Awq { bits: 4, group_size: 32 }
        );
    }
    #[test]
    fn unmappable_quantization_is_rejected() {
        for declared in [
            serde_json::json!({"quant_method": "gptq", "bits": 4, "group_size": 128, "desc_act": true}),
            serde_json::json!({"quant_method": "gptq", "bits": 3, "group_size": 128}),
            serde_json::json!({"quant_method": "gptq", "bits": 4, "group_size": -1}),
            serde_json::json!({"quant_method": "gptq", "bits": 4, "group_size": 128, "checkpoint_format": "marlin"}),
            serde_json::json!({"quant_method": "gptq", "bits": 4, "group_size": 128, "lm_head": true}),
            serde_json::json!({"quant_method": "awq", "bits": 4, "group_size": 128, "version": "gemv", "zero_point": true}),
            serde_json::json!({"quant_method": "awq", "bits": 4, "group_size": 128, "version": "gemm", "zero_point": false}),
            serde_json::json!({"quant_method": "fp8", "bits": 8, "group_size": 128}),
            serde_json::json!({"bits": 4, "group_size": 128}),
        ] {
            assert!(quantized(declared.clone()).is_err(), "{declared}");
        }
        assert!(weights(&serde_json::json!({"quantization": {"bits": 4, "group_size": 64}, "quantization_config": {"quant_method": "gptq", "bits": 4, "group_size": 64}})).is_err());
    }
    #[test]
    fn mlx_affine_settings_repeated_without_a_method_stay_mlx() {
        let settings = serde_json::json!({"bits": 4, "group_size": 64});
        assert_eq!(
            weights(
                &serde_json::json!({"quantization": settings, "quantization_config": settings})
            )
            .expect("mlx"),
            Weights::Mlx
        );
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
