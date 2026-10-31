---
schema: 1
id: qwen3-0-6b-onnx-cpu-int4
repository: onnx-community/Qwen3-0.6B-ONNX
revision: '58ca03a5e7b354b480b1a82b34bde8dc7b854cc0'
format: onnx
entry: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/model.onnx'
parameters_millions: 600
purposes: [chat]
license: Apache-2.0
layers: 28
kv_heads: 8
head_dim: 128
trained_context: 40960
cache_element_bytes: 2
evidence: docs/onnx.md
files:
  - path: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/chat_template.jinja'
    bytes: 4168
    sha256: 'a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8'
  - path: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/config.json'
    bytes: 8957
    sha256: '136fdc6ed4513075b60c40cd6e2ed6b302c7baee595c6fa82c46b1fbf72d6cf2'
  - path: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/genai_config.json'
    bytes: 1520
    sha256: '67348085a5b994a6b6d94e56e395e675d8ef4732c3c46ff75de30a541489fd3f'
  - path: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/model.onnx'
    bytes: 524589133
    sha256: '35fe34321ea9b1eaa43dfa648dd199903d01d93f97dc2b24fe1d16079ebea957'
  - path: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/tokenizer.json'
    bytes: 11422648
    sha256: '979d160e081df25a1bf7f4e2e8f4c441b5dfdc9a8e84aec9f32e80445e1b59b8'
  - path: 'onnxruntime/cpu_and_mobile/cpu-int4-kld-block-128/tokenizer_config.json'
    bytes: 663
    sha256: 'bfd13b57b2e0c2cb582f311c0895f077d16a868c0dc61e4dbbba544a835341d6'
---
Pinned from the [onnx-genai.json](../../../tests/fixtures/onnx-genai.json) fixture used by sefai's own tests.
