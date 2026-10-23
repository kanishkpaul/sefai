"""Greedy ONNX Runtime reference for sefai's ONNX adapter parity check.

Runs the same Optimum decoder graph on the CPU provider with sefai's sampler
(greedy after a 0.1 frequency penalty over the last 64 tokens) and prints JSON.
"""
import json
import sys
from pathlib import Path

import numpy as np
import onnxruntime as ort
from tokenizers import Tokenizer

graph = Path(sys.argv[1])
prompt = sys.argv[2]
output_tokens = int(sys.argv[3])
root = graph.parent if (graph.parent / "config.json").exists() else graph.parent.parent
config = json.loads((root / "config.json").read_text())
generation = json.loads((root / "generation_config.json").read_text())
eos = generation.get("eos_token_id", config.get("eos_token_id"))
eos = set(eos if isinstance(eos, list) else [eos])
tokenizer = Tokenizer.from_file(str(root / "tokenizer.json"))
layers = config["num_hidden_layers"]
kv_heads = config.get("num_key_value_heads", config["num_attention_heads"])
head_dim = config.get("head_dim", config["hidden_size"] // config["num_attention_heads"])

options = ort.SessionOptions()
options.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
options.intra_op_num_threads = 2
session = ort.InferenceSession(str(graph), options, providers=["CPUExecutionProvider"])
names = {port.name for port in session.get_inputs()}
tokens = tokenizer.encode(prompt, add_special_tokens=False).ids
history = list(tokens)
past = [np.zeros((1, kv_heads, 0, head_dim), np.float32) for _ in range(layers * 2)]
step = tokens
generated = []
margins = []
while len(generated) < output_tokens:
    length = past[0].shape[2]
    feed = {
        "input_ids": np.array([step], np.int64),
        "attention_mask": np.ones((1, length + len(step)), np.int64),
    }
    if "position_ids" in names:
        feed["position_ids"] = np.arange(length, length + len(step), dtype=np.int64)[None]
    for layer in range(layers):
        feed[f"past_key_values.{layer}.key"] = past[layer * 2]
        feed[f"past_key_values.{layer}.value"] = past[layer * 2 + 1]
    outputs = session.run(None, feed)
    logits = outputs[0][0, -1].copy()
    past = outputs[1:]
    for token in history[-64:]:
        logits[token] -= 0.1
    order = np.argsort(logits)[::-1]
    margins.append(float(logits[order[0]] - logits[order[1]]))
    token = int(order[0])
    generated.append(token)
    if token in eos:
        break
    history.append(token)
    step = [token]
text = tokenizer.decode([t for t in generated if t not in eos], skip_special_tokens=True)
print(json.dumps({
    "onnxruntime": ort.__version__,
    "prompt_tokens": len(tokens),
    "generated": generated,
    "text": text,
    "smallest_top2_margin": min(margins),
}, ensure_ascii=False))
