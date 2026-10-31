---
schema: 1
id: smollm2-1-7b-gguf
repository: HuggingFaceTB/SmolLM2-1.7B-Instruct-GGUF
revision: '2d4a76a30b4af41ecd395c35725ac11688d4cfe4'
format: gguf
entry: 'smollm2-1.7b-instruct-q4_k_m.gguf'
parameters_millions: 1700
purposes: [chat, tools]
license: Apache-2.0
layers: 24
kv_heads: 32
head_dim: 64
trained_context: 8192
cache_element_bytes: 2
evidence: docs/harness.md
files:
  - path: 'smollm2-1.7b-instruct-q4_k_m.gguf'
    bytes: 1055609536
    sha256: 'decd2598bc2c8ed08c19adc3c8fdd461ee19ed5708679d1c54ef54a5a30d4f33'
---
Pinned from the [tool-gguf.json](../../../tests/fixtures/tool-gguf.json) fixture used by sefai's own tests.
