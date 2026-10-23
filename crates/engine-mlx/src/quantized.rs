//! Loads GPTQ and AWQ checkpoints by repacking each quantized layer into MLX affine groups in memory.
//! Only packed layers are read here; embeddings, norms and other tensors load from the files unchanged.

use crate::{
    Weights,
    native::{Error, Result},
    repack::{self, Affine, Shape},
};
use mlx_rs::Array;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const HEADER_BYTES_MAX: u64 = 100 * 1024 * 1024;

/// Checkpoint tensor names mapped to their MLX replacements; `None` removes a packed tensor.
pub(crate) type Replacements = BTreeMap<String, Option<Array>>;

/// Returns the MLX configuration and the tensors that replace each packed layer.
pub(crate) fn load(root: &Path, weights: Weights) -> Result<(Vec<u8>, Replacements)> {
    let (bits, group_size) = match weights {
        Weights::Gptq {
            bits, group_size, ..
        }
        | Weights::Awq { bits, group_size } => (u32::from(bits), group_size),
        Weights::Mlx => return Err("mlx bundle has no gptq or awq layers".into()),
    };
    let mut config: Value = serde_json::from_slice(&std::fs::read(root.join("config.json"))?)
        .map_err(|error| Error::Invalid(format!("mlx config.json: {error}")))?;
    let fields = config
        .as_object_mut()
        .ok_or("mlx config.json must be an object")?;
    fields.remove("quantization_config");
    fields.insert(
        "quantization".into(),
        serde_json::json!({ "group_size": group_size, "bits": bits }),
    );
    let config = serde_json::to_vec(&config).map_err(|error| Error::Invalid(error.to_string()))?;
    let tensors = index(root)?;
    // MLX computes in the activation dtype only when scales and biases share it; a mismatch
    // promotes every quantized matrix product to float32, which ran 3x slower on bf16 Qwen3.
    let activation = tensors
        .get("model.embed_tokens.weight")
        .ok_or("quantized bundle has no model.embed_tokens.weight")?
        .dtype
        .clone();
    let grouped = |values: Vec<f32>, shape: &[i32]| -> Result<Array> {
        Ok(match activation.as_str() {
            "BF16" => Array::from_slice(
                &values
                    .into_iter()
                    .map(half::bf16::from_f32)
                    .collect::<Vec<_>>(),
                shape,
            ),
            "F16" => Array::from_slice(
                &values
                    .into_iter()
                    .map(half::f16::from_f32)
                    .collect::<Vec<_>>(),
                shape,
            ),
            "F32" => Array::from_slice(&values, shape),
            other => {
                return Err(Error::Invalid(format!(
                    "embedding dtype {other} is not a floating activation dtype"
                )));
            }
        })
    };
    let mut replaced = BTreeMap::new();
    for prefix in tensors
        .keys()
        .filter_map(|name| name.strip_suffix(".qweight"))
    {
        let read = |suffix: &str| {
            tensors
                .get(&format!("{prefix}.{suffix}"))
                .ok_or_else(|| Error::Invalid(format!("{prefix} has qweight but no {suffix}")))
        };
        let (qweight, qzeros, scales) = (read("qweight")?, read("qzeros")?, read("scales")?);
        let [groups, outputs] = scales.shape[..] else {
            return Err(Error::Invalid(format!(
                "{prefix}.scales must be two-dimensional"
            )));
        };
        let shape = Shape {
            inputs: groups * group_size as usize,
            outputs,
            bits,
            group_size: group_size as usize,
        };
        let (qweight, qzeros, scales) = (qweight.words()?, qzeros.words()?, scales.floats()?);
        let affine = match weights {
            Weights::Gptq { zero_offset, .. } => {
                if let Ok(order) = read("g_idx") {
                    let order = order.words()?;
                    if order.len() != shape.inputs
                        || order
                            .iter()
                            .enumerate()
                            .any(|(input, group)| *group as usize != input / shape.group_size)
                    {
                        return Err(Error::Invalid(format!(
                            "{prefix}.g_idx reorders groups (act-order), which mlx affine groups cannot represent"
                        )));
                    }
                    replaced.insert(format!("{prefix}.g_idx"), None);
                }
                repack::gptq(&qweight, &qzeros, &scales, shape, zero_offset)
            }
            Weights::Awq { .. } => repack::awq(&qweight, &qzeros, &scales, shape),
            Weights::Mlx => unreachable!("rejected above"),
        }
        .map_err(|error| Error::Invalid(format!("{prefix}: {error}")))?;
        let Affine {
            weight,
            scales,
            biases,
        } = affine;
        let (rows, columns, groups) = (
            shape.outputs as i32,
            (shape.inputs * bits as usize / 32) as i32,
            (shape.inputs / shape.group_size) as i32,
        );
        replaced.insert(format!("{prefix}.qweight"), None);
        replaced.insert(format!("{prefix}.qzeros"), None);
        replaced.insert(
            format!("{prefix}.weight"),
            Some(Array::from_slice(&weight, &[rows, columns])),
        );
        replaced.insert(
            format!("{prefix}.scales"),
            Some(grouped(scales, &[rows, groups])?),
        );
        replaced.insert(
            format!("{prefix}.biases"),
            Some(grouped(biases, &[rows, groups])?),
        );
        // Some exporters store an all-zero bias for bias-free layers; removing it is exact.
        // Any other bias stays, and strict loading accepts it only where the architecture has one.
        if let Ok(bias) = read("bias")
            && bias.floats()?.iter().all(|value| *value == 0.0)
        {
            replaced.insert(format!("{prefix}.bias"), None);
        }
    }
    if replaced.is_empty() {
        return Err("quantized bundle has no qweight tensors".into());
    }
    Ok((config, replaced))
}

/// One tensor's location and declared layout in a safetensors shard.
struct Stored {
    path: PathBuf,
    dtype: String,
    shape: Vec<usize>,
    start: u64,
    bytes: usize,
}

impl Stored {
    fn read(&self) -> Result<Vec<u8>> {
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.start))?;
        let mut bytes = vec![0; self.bytes];
        file.read_exact(&mut bytes)?;
        Ok(bytes)
    }
    /// Reads an `I32` or `U32` tensor as little-endian words.
    fn words(&self) -> Result<Vec<u32>> {
        if !matches!(self.dtype.as_str(), "I32" | "U32") {
            return Err(Error::Invalid(format!(
                "packed tensor must be I32 or U32, not {}",
                self.dtype
            )));
        }
        Ok(self
            .read()?
            .chunks_exact(4)
            .map(|word| u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
            .collect())
    }
    /// Reads an `F16`, `BF16` or `F32` tensor as `f32`.
    fn floats(&self) -> Result<Vec<f32>> {
        let bytes = self.read()?;
        Ok(match self.dtype.as_str() {
            "F16" => bytes
                .chunks_exact(2)
                .map(|value| half::f16::from_le_bytes([value[0], value[1]]).to_f32())
                .collect(),
            "BF16" => bytes
                .chunks_exact(2)
                .map(|value| half::bf16::from_le_bytes([value[0], value[1]]).to_f32())
                .collect(),
            "F32" => bytes
                .chunks_exact(4)
                .map(|value| f32::from_le_bytes([value[0], value[1], value[2], value[3]]))
                .collect(),
            other => {
                return Err(Error::Invalid(format!(
                    "scales must be F16, BF16 or F32, not {other}"
                )));
            }
        })
    }
}

/// Maps every tensor name to its shard; bundle admission already bounded the shard set.
fn index(root: &Path) -> Result<BTreeMap<String, Stored>> {
    let index = root.join("model.safetensors.index.json");
    let shards = if index.exists() {
        let index: Value = serde_json::from_slice(&std::fs::read(index)?)
            .map_err(|error| Error::Invalid(format!("mlx index: {error}")))?;
        let mut shards = index["weight_map"]
            .as_object()
            .ok_or("mlx index requires weight_map")?
            .values()
            .filter_map(Value::as_str)
            .map(|name| root.join(name))
            .collect::<Vec<_>>();
        shards.sort();
        shards.dedup();
        shards
    } else {
        vec![root.join("model.safetensors")]
    };
    let mut tensors = BTreeMap::new();
    for path in shards {
        let mut file = File::open(&path)?;
        let length = file.metadata()?.len();
        let mut size = [0; 8];
        file.read_exact(&mut size)?;
        let size = u64::from_le_bytes(size);
        if size > HEADER_BYTES_MAX || size + 8 > length {
            return Err(Error::Invalid(format!(
                "{} has an invalid safetensors header",
                path.display()
            )));
        }
        let mut header = vec![0; size as usize];
        file.read_exact(&mut header)?;
        let header: BTreeMap<String, Value> = serde_json::from_slice(&header)
            .map_err(|error| Error::Invalid(format!("{}: {error}", path.display())))?;
        for (name, entry) in header {
            if name == "__metadata__" {
                continue;
            }
            let invalid = || Error::Invalid(format!("{name}: malformed safetensors entry"));
            let dtype = entry["dtype"].as_str().ok_or_else(invalid)?.to_owned();
            let shape = entry["shape"]
                .as_array()
                .ok_or_else(invalid)?
                .iter()
                .map(|dimension| dimension.as_u64().map(|d| d as usize))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(invalid)?;
            let [start, end] = [0, 1].map(|i| entry["data_offsets"][i].as_u64());
            let (Some(start), Some(end)) = (start, end) else {
                return Err(invalid());
            };
            let width = match dtype.as_str() {
                "BOOL" | "U8" | "I8" | "F8_E4M3" | "F8_E5M2" => 1,
                "U16" | "I16" | "F16" | "BF16" => 2,
                "U32" | "I32" | "F32" => 4,
                "U64" | "I64" | "F64" => 8,
                _ => return Err(Error::Invalid(format!("{name}: unsupported dtype {dtype}"))),
            };
            let elements = shape
                .iter()
                .try_fold(1usize, |total, dimension| total.checked_mul(*dimension));
            if end < start
                || 8 + size + end > length
                || elements.and_then(|n| n.checked_mul(width)) != Some((end - start) as usize)
            {
                return Err(invalid());
            }
            tensors.insert(
                name,
                Stored {
                    path: path.clone(),
                    dtype,
                    shape,
                    start: 8 + size + start,
                    bytes: (end - start) as usize,
                },
            );
        }
    }
    Ok(tensors)
}
