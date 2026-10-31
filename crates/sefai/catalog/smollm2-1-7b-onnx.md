---
schema: 1
id: smollm2-1-7b-onnx
repository: HuggingFaceTB/SmolLM2-1.7B-Instruct
revision: '31b70e2e869a7173562077fd711b654946d38674'
format: onnx
entry: 'onnx/model_fp16.onnx'
parameters_millions: 1700
purposes: [chat, tools]
license: Apache-2.0
layers: 24
kv_heads: 32
head_dim: 64
trained_context: 8192
cache_element_bytes: 2
evidence: docs/onnx.md
files:
  - path: 'config.json'
    bytes: 908
    sha256: '994f50b16abb4ae00880baefe03c10260b5bd608d2bf586f7056ca05a534feea'
  - path: 'generation_config.json'
    bytes: 132
    sha256: '87b916edaaab66b3899b9d0dd0752727dff6666686da0504d89ae0a6e055a013'
  - path: 'tokenizer.json'
    bytes: 2104556
    sha256: '9ca9acddb6525a194ec8ac7a87f24fbba7232a9a15ffa1af0c1224fcd888e47c'
  - path: 'tokenizer_config.json'
    bytes: 3764
    sha256: '4ec77d44f62efeb38d7e044a1db318f6a939438425312dfa333b8382dbad98df'
  - path: 'onnx/model_fp16.onnx'
    bytes: 1326807956
    sha256: '3d891d77661f6339f727b3188f1c21b1429a1e4e55c40d1f60f85048b00a4348'
  - path: 'onnx/model_fp16.onnx_data'
    bytes: 2097152000
    sha256: '5f48c05c14ed97738f8dc5854c20c229ddc8661f43fa914085843901a4ba8740'
---
Pinned from the [onnx-tool.json](../../../tests/fixtures/onnx-tool.json) fixture used by sefai's own tests.
