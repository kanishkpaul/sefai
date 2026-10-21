"""Compare the native measure example with pinned Python MLX on local weights."""
import argparse
import importlib.metadata
import json
from pathlib import Path

import mlx.core as mx
from mlx_lm import load, stream_generate
from mlx_lm.sample_utils import make_sampler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("model", type=Path)
    parser.add_argument("native", type=Path)
    args = parser.parse_args()
    if not args.model.is_dir():
        parser.error("model must be a local bundle directory")
    versions = {name: importlib.metadata.version(name) for name in ["mlx", "mlx-lm"]}
    if versions != {"mlx": "0.32.2", "mlx-lm": "0.31.3"}:
        parser.error("reference requires mlx 0.32.2 and mlx-lm 0.31.3")
    native = json.loads(args.native.read_text())
    mx.set_cache_limit(64 * 1024 * 1024)
    mx.reset_peak_memory()
    model, tokenizer = load(args.model, tokenizer_config={"trust_remote_code": False})
    ids = tokenizer.encode(native["prompt"], add_special_tokens=False)
    if ids != native["prompt_ids"]:
        raise ValueError("reference prompt token ids differ from native ids")
    samples = []
    for _ in range(3):
        text = ""
        for response in stream_generate(
            model, tokenizer, ids,
            max_tokens=native["output_tokens"],
            prefill_step_size=native["prefill_chunk_tokens"],
            sampler=make_sampler(temp=0.0),
        ):
            text += response.text
        samples.append({
            "prompt_tokens": response.prompt_tokens,
            "generated_tokens": response.generation_tokens,
            "prefill_s": response.prompt_tokens / response.prompt_tps,
            "generation_s": response.generation_tokens / response.generation_tps,
            "stop": response.finish_reason,
            "text": text,
            "memory_bytes": [mx.get_active_memory(), mx.get_cache_memory(), mx.get_peak_memory()],
        })
    print(json.dumps({"versions": versions, "prompt_ids_match": True, "samples": samples}, indent=2))


if __name__ == "__main__":
    main()
