//! Bounded GGUF header reading for setup estimates; tensor data is never read.
//!
//! A GGUF v2 or v3 file starts with the magic, a version, tensor and key-value counts, then
//! typed key-value pairs. Only the architecture and attention shape are kept; every other
//! value, including large tokenizer arrays, is skipped by seeking past it.

use super::Shape;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

const KEY_COUNT_MAX: u64 = 65_536;
const KEY_BYTES_MAX: u64 = 65_536;
const STRING_BYTES_MAX: u64 = 1024 * 1024;
const ARRAY_LENGTH_MAX: u64 = 16 * 1024 * 1024;

/// Reads the architecture and attention shape from a GGUF header.
///
/// # Errors
/// Rejects a missing magic, an unsupported version, oversized keys or arrays, and a header
/// without the architecture's layer, head and embedding counts.
pub fn read(path: &Path) -> io::Result<Shape> {
    let file = File::open(path)?;
    let length = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if &magic != b"GGUF" {
        return Err(invalid("gguf header: missing magic"));
    }
    if !matches!(u32_le(&mut reader)?, 2 | 3) {
        return Err(invalid("gguf header: unsupported version"));
    }
    let _tensors = u64_le(&mut reader)?;
    let keys = u64_le(&mut reader)?;
    if keys > KEY_COUNT_MAX {
        return Err(invalid("gguf header: too many keys"));
    }
    let mut strings = BTreeMap::new();
    let mut numbers = BTreeMap::new();
    for _ in 0..keys {
        let key = string(&mut reader, KEY_BYTES_MAX, length)?;
        let kind = u32_le(&mut reader)?;
        match kind {
            8 if key == "general.architecture" => {
                strings.insert(key, string(&mut reader, STRING_BYTES_MAX, length)?);
            }
            0..=7 | 10..=12 => {
                if let Some(number) = scalar(&mut reader, kind)? {
                    numbers.insert(key, number);
                }
            }
            _ => skip(&mut reader, kind, length)?,
        }
    }
    let architecture = strings
        .remove("general.architecture")
        .ok_or_else(|| invalid("gguf header: missing general.architecture"))?;
    let field = |name: &str| numbers.get(&format!("{architecture}.{name}")).copied();
    let required = |name: &str| {
        field(name)
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid(format!("gguf header: missing {architecture}.{name}")))
    };
    let heads = required("attention.head_count")?;
    let embedding = required("embedding_length")?;
    let layers = required("block_count")?;
    let context = field("context_length");
    let kv_heads = field("attention.head_count_kv").unwrap_or(heads);
    let head_dim = field("attention.key_length").unwrap_or(embedding / heads);
    Ok(Shape {
        architecture,
        layers,
        context,
        heads,
        kv_heads,
        head_dim,
    })
}

fn scalar(reader: &mut impl Read, kind: u32) -> io::Result<Option<u64>> {
    let mut bytes = [0; 8];
    let size = scalar_bytes(kind).expect("caller passes scalar kinds");
    reader.read_exact(&mut bytes[..size])?;
    Ok(match kind {
        0 | 2 | 4 | 10 => Some(u64::from_le_bytes(bytes)),
        // Signed values are kept only when non-negative; floats and booleans are never shapes.
        1 | 3 | 5 | 11 => {
            let shift = 64 - 8 * size as u32;
            let signed = (i64::from_le_bytes(bytes) << shift) >> shift;
            u64::try_from(signed).ok()
        }
        _ => None,
    })
}

fn scalar_bytes(kind: u32) -> Option<usize> {
    match kind {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn skip(reader: &mut (impl Read + Seek), kind: u32, length: u64) -> io::Result<()> {
    match kind {
        8 => {
            let bytes = u64_le(reader)?;
            seek(reader, bytes, length)
        }
        9 => {
            let element = u32_le(reader)?;
            let count = u64_le(reader)?;
            if count > ARRAY_LENGTH_MAX {
                return Err(invalid("gguf header: array too long"));
            }
            if let Some(size) = scalar_bytes(element) {
                return seek(reader, count * size as u64, length);
            }
            if element != 8 {
                return Err(invalid("gguf header: unsupported nested array"));
            }
            for _ in 0..count {
                let bytes = u64_le(reader)?;
                seek(reader, bytes, length)?;
            }
            Ok(())
        }
        _ => Err(invalid("gguf header: unknown value type")),
    }
}

fn seek(reader: &mut impl Seek, bytes: u64, length: u64) -> io::Result<()> {
    let position = reader.stream_position()?;
    if position.checked_add(bytes).is_none_or(|end| end > length) {
        return Err(invalid("gguf header: value extends past the file"));
    }
    reader.seek(SeekFrom::Current(bytes as i64))?;
    Ok(())
}

fn string(reader: &mut (impl Read + Seek), limit: u64, length: u64) -> io::Result<String> {
    let bytes = u64_le(reader)?;
    if bytes > limit || reader.stream_position()? + bytes > length {
        return Err(invalid("gguf header: string too long"));
    }
    let mut text = vec![0; bytes as usize];
    reader.read_exact(&mut text)?;
    String::from_utf8(text).map_err(|_| invalid("gguf header: string is not utf-8"))
}

fn u32_le(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn u64_le(reader: &mut impl Read) -> io::Result<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(header: &mut Vec<u8>, name: &str) {
        header.extend((name.len() as u64).to_le_bytes());
        header.extend(name.as_bytes());
    }

    fn fixture(entries: &[(&str, u32)], architecture: &str) -> std::path::PathBuf {
        let mut header = b"GGUF".to_vec();
        header.extend(3u32.to_le_bytes());
        header.extend(0u64.to_le_bytes());
        header.extend((entries.len() as u64 + 2).to_le_bytes());
        key(&mut header, "general.architecture");
        header.extend(8u32.to_le_bytes());
        key(&mut header, architecture);
        key(&mut header, "tokenizer.ggml.tokens");
        header.extend(9u32.to_le_bytes());
        header.extend(8u32.to_le_bytes());
        header.extend(2u64.to_le_bytes());
        key(&mut header, "<s>");
        key(&mut header, "</s>");
        for (name, value) in entries {
            key(&mut header, name);
            header.extend(4u32.to_le_bytes());
            header.extend(value.to_le_bytes());
        }
        let path = std::env::temp_dir().join(format!(
            "sefai-gguf-header-{}-{}",
            std::process::id(),
            entries.len()
        ));
        std::fs::write(&path, header).expect("write fixture");
        path
    }

    #[test]
    fn attention_shape_is_read_past_tokenizer_arrays() {
        let path = fixture(
            &[
                ("llama.block_count", 30),
                ("llama.context_length", 8192),
                ("llama.embedding_length", 576),
                ("llama.attention.head_count", 9),
                ("llama.attention.head_count_kv", 3),
            ],
            "llama",
        );
        let shape = read(&path).expect("shape");
        std::fs::remove_file(&path).expect("remove fixture");
        assert_eq!(
            (shape.layers, shape.kv_heads, shape.head_dim, shape.context),
            (30, 3, 64, Some(8192))
        );
    }

    #[test]
    fn missing_layer_count_is_rejected() {
        let path = fixture(
            &[
                ("llama.embedding_length", 576),
                ("llama.attention.head_count", 9),
            ],
            "llama",
        );
        let result = read(&path);
        std::fs::remove_file(&path).expect("remove fixture");
        assert!(result.is_err());
    }
}
