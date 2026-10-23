"""Teacher-forced parity between a GPTQ or AWQ checkpoint's reference and its MLX-layout repack.

reference CHECKPOINT PROMPT TOKENS OUT.npz [BACKEND]
    Greedy continuation with Transformers and GPTQModel on CPU in float16, then the
    sequence's next-token logits in float16 and float32. BACKEND selects a GPTQModel kernel.
compare MLX_BUNDLE OUT.npz
    Next-token logits for the same sequence with Python mlx-lm on the bundle written by
    quantized_repack.py, compared position by position with the reference; prints JSON.
"""
import json
import sys

import numpy as np


def reference(path, prompt, output, out, backend=None):
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer, GPTQConfig

    tokenizer = AutoTokenizer.from_pretrained(path)
    ids = tokenizer(prompt, return_tensors="pt", add_special_tokens=False).input_ids
    result = {"prompt_ids": ids[0].numpy()}
    extra = {"quantization_config": GPTQConfig(bits=4, backend=backend)} if backend else {}
    for dtype in (torch.float16, torch.float32):
        model = AutoModelForCausalLM.from_pretrained(path, device_map="cpu", dtype=dtype, **extra)
        with torch.no_grad():
            if dtype == torch.float16:
                sequence = model.generate(ids, max_new_tokens=output, min_new_tokens=output, do_sample=False)
                result["sequence"] = sequence[0].numpy()
            logits = model(torch.from_numpy(result["sequence"])[None]).logits
        result[f"logits_{str(dtype)[6:]}"] = logits[0, ids.shape[1] - 1:-1].float().numpy()
    np.savez(out, **result)


def compare(bundle, reference):
    import mlx.core as mx
    from mlx_lm import load

    reference = np.load(reference)
    model, _ = load(bundle)
    sequence, start = reference["sequence"], len(reference["prompt_ids"])
    logits = np.array(model(mx.array(sequence[None]))[0, start - 1:-1].astype(mx.float32))
    fp16, fp32 = reference["logits_float16"], reference["logits_float32"]

    def agree(a, b):
        return float((a.argmax(-1) == b.argmax(-1)).mean())

    top = np.sort(fp32, -1)[:, -2:]
    differs = logits.argmax(-1) != fp32.argmax(-1)
    print(json.dumps({
        "positions": int(len(logits)),
        "mlx_vs_fp16_top1": agree(logits, fp16),
        "mlx_vs_fp32_top1": agree(logits, fp32),
        "fp16_vs_fp32_top1": agree(fp16, fp32),
        "mlx_vs_fp32_max_abs_logit": float(np.abs(logits - fp32).max()),
        "fp16_vs_fp32_max_abs_logit": float(np.abs(fp16 - fp32).max()),
        "largest_fp32_margin_where_mlx_differs": float((top[:, 1] - top[:, 0])[differs].max(initial=0)),
    }))


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    if command == "reference":
        reference(args[0], args[1], int(args[2]), args[3], *args[4:])
    elif command == "compare":
        compare(*args)
    else:
        raise SystemExit(__doc__)
