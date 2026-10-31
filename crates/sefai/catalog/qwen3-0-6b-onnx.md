---
schema: 1
id: qwen3-0-6b-onnx
repository: onnx-community/Qwen3-0.6B-ONNX
revision: '58ca03a5e7b354b480b1a82b34bde8dc7b854cc0'
format: onnx
entry: 'onnx/model.onnx'
parameters_millions: 600
purposes: [chat]
license: Apache-2.0
layers: 28
kv_heads: 8
head_dim: 128
trained_context: 40960
cache_element_bytes: 4
evidence: docs/onnx.md
files:
  - path: 'config.json'
    bytes: 912
    sha256: '8a04114ba59cc42b47d804d35d1d5c61d746ae4634f41f796768c6e302d39b9e'
  - path: 'generation_config.json'
    bytes: 219
    sha256: '9e9e031ae8bca36eefcdcdd0b35d83c61baa01e31d68d5f5a961c9ca1a4b95bf'
  - path: 'tokenizer.json'
    bytes: 9117040
    sha256: 'e7a95fce95bf5b0946d0ddb3f9d7caa030b7e850bbe92b0edb26bcf563e9f3d5'
  - path: 'tokenizer_config.json'
    bytes: 10369
    sha256: '7c5194eecfd95a4dcbd6b4b502af785d2541fd65a388d1cae7ad43693c67b802'
  - path: 'onnx/model.onnx'
    bytes: 315088888
    sha256: '0a8f1cfe531fc7b15d26db55090f0667863f0c62c952dff128cf23cd421e3fbc'
  - path: 'onnx/model.onnx_data'
    bytes: 2090336256
    sha256: 'e0927a7de2fe7c2a9b50a121905cf2edda78fb2900c85c1790db98188bbfe4be'
---
Pinned from the [onnx-external.json](../../../tests/fixtures/onnx-external.json) fixture used by sefai's own tests.
