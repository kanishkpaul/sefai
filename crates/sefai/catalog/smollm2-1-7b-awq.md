---
schema: 1
id: smollm2-1-7b-awq
repository: fbaldassarri/HuggingFaceTB_SmolLM2-1.7B-Instruct-auto_awq-int4-gs128-asym
revision: '899203c2d6b260cdc3f4f862a3b99a30dc362014'
format: awq
entry: '.'
parameters_millions: 1700
purposes: [chat, tools]
license: Apache-2.0
layers: 24
kv_heads: 32
head_dim: 64
trained_context: 8192
cache_element_bytes: 4
evidence: docs/gptq-awq.md
files:
  - path: 'config.json'
    bytes: 1560
    sha256: '213df528f2bfee038a7ae9b3e0c411cbfd80cfdb52528e003d2c856c41757905'
  - path: 'generation_config.json'
    bytes: 132
    sha256: 'ae455741bbec6a49f72f39f61222c52ec89d2e033354fec396c7ddfb2b66c31b'
  - path: 'model.safetensors'
    bytes: 1239880632
    sha256: 'f67e5d0e1fdd3cc558be20f8c8ada03bea0e9624cbd06459c77c9cccf0f4caf5'
  - path: 'special_tokens_map.json'
    bytes: 655
    sha256: '2b7379f3ae813529281a5c602bc5a11c1d4e0a99107aaa597fe936c1e813ca52'
  - path: 'tokenizer.json'
    bytes: 3522754
    sha256: 'a242410029b5cd624e7ea3df8d5f254b066721e96170d8f068a2cab44f95bcf3'
  - path: 'tokenizer_config.json'
    bytes: 3794
    sha256: '1ca52cbd5eb9e8e6156216e3952427fd3c2a63a119dfae97a2e8c8215dd393e6'
---
Pinned from the [awq-tool.json](../../../tests/fixtures/awq-tool.json) fixture used by sefai's own tests.
