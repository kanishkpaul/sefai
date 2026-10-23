"""Independent Python repack of a GPTQ or AWQ checkpoint for sefai's Phase 8 parity checks.

Writes an MLX-layout bundle, whose scales and biases use the checkpoint's activation dtype as
sefai's native loader does, and a float16 bundle of the dequantized weights.

usage: quantized_repack.py SOURCE_DIR MLX_OUT_DIR FLOAT_OUT_DIR
"""
import json, shutil, struct, sys
from pathlib import Path
import numpy as np

source, mlx_out, float_out = map(Path, sys.argv[1:4])
DTYPES = {"F16": np.float16, "F32": np.float32, "I32": np.int32, "U32": np.uint32, "I64": np.int64, "U8": np.uint8, "I8": np.int8}
AWQ_ORDER = [0, 2, 4, 6, 1, 3, 5, 7]

def read(path):
    data = path.read_bytes()
    n = struct.unpack("<Q", data[:8])[0]
    header = json.loads(data[8:8 + n])
    header.pop("__metadata__", None)
    base = 8 + n
    return {k: (v["dtype"], v["shape"], data[base + v["data_offsets"][0]: base + v["data_offsets"][1]]) for k, v in header.items()}

def array(entry):
    dtype, shape, raw = entry
    return np.frombuffer(raw, DTYPES[dtype]).reshape(shape)

def write(path, tensors):
    header, offset, blobs = {}, 0, []
    for k in sorted(tensors):
        dtype, shape, raw = tensors[k]
        header[k] = {"dtype": dtype, "shape": list(shape), "data_offsets": [offset, offset + len(raw)]}
        offset += len(raw)
        blobs.append(raw)
    h = json.dumps(header).encode()
    h += b" " * (-len(h) % 8)
    with open(path, "wb") as f:
        f.write(struct.pack("<Q", len(h)))
        f.write(h)
        for b in blobs:
            f.write(b)

def entry(a):
    name = {np.dtype(np.float16): "F16", np.dtype(np.float32): "F32", np.dtype(np.uint32): "U32"}[a.dtype]
    a = np.ascontiguousarray(a)
    return (name, a.shape, a.tobytes())

def grouped(a, dtype):
    """Stores scales or biases in the activation dtype; bf16 rounds to nearest even like half::bf16."""
    a = np.ascontiguousarray(a.astype(np.float32))
    if dtype == "F16":
        return entry(a.astype(np.float16))
    if dtype == "F32":
        return entry(a)
    bits = a.view(np.uint32)
    rounded = ((bits + 0x7FFF + ((bits >> 16) & 1)) >> 16).astype(np.uint16)
    return ("BF16", a.shape, rounded.tobytes())

def unpack_rows(packed, bits):
    """GPTQ packs consecutive rows of the first axis LSB-first into each int32."""
    per = 32 // bits
    p = packed.view(np.uint32)
    parts = [(p >> (bits * k)) & ((1 << bits) - 1) for k in range(per)]
    return np.stack(parts, axis=1).reshape(p.shape[0] * per, p.shape[1])

def unpack_cols(packed, bits, order=None):
    per = 32 // bits
    p = packed.view(np.uint32)
    parts = [(p >> (bits * k)) & ((1 << bits) - 1) for k in range(per)]
    out = np.empty((p.shape[0], p.shape[1] * per), np.uint32)
    for k in range(per):
        column = order[k] if order else k
        out[:, column::per] = parts[k]
    return out

def pack_mlx(q, bits):
    """MLX packs consecutive elements of the last axis LSB-first into each uint32."""
    per = 32 // bits
    rows, cols = q.shape
    q = q.astype(np.uint32).reshape(rows, cols // per, per)
    out = np.zeros((rows, cols // per), np.uint32)
    for k in range(per):
        out |= q[:, :, k] << np.uint32(bits * k)
    return out

config = json.loads((source / "config.json").read_text())
qc = config["quantization_config"]
method, bits, group = qc["quant_method"], qc["bits"], qc["group_size"]
tensors = read(source / "model.safetensors")
activation = tensors["model.embed_tokens.weight"][0]
prefixes = sorted(k[: -len(".qweight")] for k in tensors if k.endswith(".qweight"))
mlx_tensors, float_tensors = {}, {}
used = set()
for prefix in prefixes:
    names = {s: f"{prefix}.{s}" for s in ("qweight", "qzeros", "scales", "g_idx", "bias")}
    used.update(n for n in names.values() if n in tensors)
    scales = array(tensors[names["scales"]]).astype(np.float32)
    if method == "gptq":
        q = unpack_rows(array(tensors[names["qweight"]]), bits)        # [in, out]
        z = unpack_cols(array(tensors[names["qzeros"]]), bits)         # [groups, out]
        if qc.get("checkpoint_format", "gptq") == "gptq":
            z = z + 1                                                   # v1 stores zero - 1
        if names["g_idx"] in tensors:
            g = array(tensors[names["g_idx"]])
            if not np.array_equal(g, np.arange(q.shape[0]) // group):
                raise SystemExit(f"{prefix}: act-order g_idx cannot map to contiguous groups")
    elif method == "awq":
        q = unpack_cols(array(tensors[names["qweight"]]), bits, AWQ_ORDER)  # [in, out]
        z = unpack_cols(array(tensors[names["qzeros"]]), bits, AWQ_ORDER)
    else:
        raise SystemExit(f"unsupported method {method}")
    z = z.astype(np.float32)
    qt = q.T                                                           # [out, in]
    s_t, z_t = scales.T, z.T                                           # [out, groups]
    mlx_tensors[f"{prefix}.weight"] = entry(pack_mlx(qt, bits))
    mlx_tensors[f"{prefix}.scales"] = grouped(s_t, activation)
    mlx_tensors[f"{prefix}.biases"] = grouped(-s_t * z_t, activation)
    dequant = (qt.astype(np.float32) - np.repeat(z_t, group, axis=1)) * np.repeat(s_t, group, axis=1)
    float_tensors[f"{prefix}.weight"] = entry(dequant.astype(np.float16))
    if names["bias"] in tensors and np.any(array(tensors[names["bias"]])):
        mlx_tensors[f"{prefix}.bias"] = float_tensors[f"{prefix}.bias"] = tensors[names["bias"]]
for k, v in tensors.items():
    if k not in used:
        mlx_tensors[k] = float_tensors[k] = v
for out, tensors_out, quantized in ((mlx_out, mlx_tensors, True), (float_out, float_tensors, False)):
    out.mkdir(parents=True, exist_ok=True)
    write(out / "model.safetensors", tensors_out)
    c = dict(config)
    c.pop("quantization_config")
    c["torch_dtype"] = "float16"
    if quantized:
        c["quantization"] = {"group_size": group, "bits": bits}
    (out / "config.json").write_text(json.dumps(c, indent=1))
    for name in ("tokenizer.json", "tokenizer_config.json", "generation_config.json", "chat_template.jinja", "special_tokens_map.json"):
        if (source / name).exists():
            shutil.copy(source / name, out / name)
print(json.dumps({"method": method, "bits": bits, "group_size": group, "layers": len(prefixes), "passthrough": sorted({v[0] for k, v in tensors.items() if k not in used})}))
