---
schema: 1
id: qwen3-0-6b-onnx-webgpu-int4
repository: onnx-community/Qwen3-0.6B-ONNX
revision: '58ca03a5e7b354b480b1a82b34bde8dc7b854cc0'
format: onnx
entry: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/model.onnx'
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
  - path: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/chat_template.jinja'
    bytes: 4168
    sha256: 'a55ee1b1660128b7098723e0abcd92caa0788061051c62d51cbe87d9cf1974d8'
  - path: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/config.json'
    bytes: 9033
    sha256: 'b7b2ac003e14aea8ea0e79c8f83a6033c9fb05e75baaf23661c7724afbf3e71b'
  - path: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/genai_config.json'
    bytes: 1752
    sha256: '5d6913e5bc4172ae1349e6f7fd36107fdee3bdded0e0ce48e85f90475f56cdb5'
  - path: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/model.onnx'
    bytes: 543321042
    sha256: '5e9fb386cb1a14009b02b43b0e0f0043e248ad5a6d3c3521c9a516062509909a'
  - path: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/tokenizer.json'
    bytes: 11422648
    sha256: '979d160e081df25a1bf7f4e2e8f4c441b5dfdc9a8e84aec9f32e80445e1b59b8'
  - path: 'onnxruntime/webgpu/webgpu-int4-kld-block-32/tokenizer_config.json'
    bytes: 663
    sha256: 'bfd13b57b2e0c2cb582f311c0895f077d16a868c0dc61e4dbbba544a835341d6'
---
Pinned from the [onnx-genai.json](../../../tests/fixtures/onnx-genai.json) fixture used by sefai's own tests.
