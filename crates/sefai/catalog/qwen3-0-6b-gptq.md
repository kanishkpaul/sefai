---
schema: 1
id: qwen3-0-6b-gptq
repository: Qwen/Qwen3-0.6B-GPTQ-Int8
revision: 'd3f20e7e71825fe57ad32705b396e9e156279107'
format: gptq
entry: '.'
parameters_millions: 600
purposes: [chat]
license: Apache-2.0
layers: 28
kv_heads: 8
head_dim: 128
trained_context: 40960
cache_element_bytes: 2
evidence: docs/gptq-awq.md
files:
  - path: 'config.json'
    bytes: 1221
    sha256: '37ac3b589e76c7330b28c83dfa98a72f3c9fa731ba9e4f0b1294f292d350197e'
  - path: 'generation_config.json'
    bytes: 239
    sha256: '2325da0f15bb848e018c5ae071b7943332e9f871d6b60e2ed22ca97d4cb993d2'
  - path: 'model.safetensors'
    bytes: 763265296
    sha256: 'a8000e596caeb50b6e8e332e77e88a30f569a51ee6e45f00a7cbb40a36f3e85a'
  - path: 'tokenizer.json'
    bytes: 11422654
    sha256: 'aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4'
  - path: 'tokenizer_config.json'
    bytes: 9732
    sha256: 'd5d09f07b48c3086c508b30d1c9114bd1189145b74e982a265350c923acd8101'
---
Pinned from the [gptq.json](../../../tests/fixtures/gptq.json) fixture used by sefai's own tests.
