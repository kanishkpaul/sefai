---
schema: 1
id: smollm2-1-7b-mlx
repository: HuggingFaceTB/SmolLM2-1.7B-Instruct-Q8-mlx
revision: '2bb0b4cebf6f2e93b23779a44f8b93e185609046'
format: mlx
entry: '.'
parameters_millions: 1700
purposes: [chat, tools]
license: Apache-2.0
layers: 24
kv_heads: 32
head_dim: 64
trained_context: 8192
cache_element_bytes: 2
evidence: docs/mlx.md
files:
  - path: 'config.json'
    bytes: 1024
    sha256: '727ae411d8d5657a3edebf6fb0b64c71fcf5ca65af01bc70434d89a82124b2a3'
  - path: 'tokenizer.json'
    bytes: 3522656
    sha256: '7d27c493c729a66ecefc837280b05d948b1ed50d130eebdbf911b1b36cf38ed7'
  - path: 'tokenizer_config.json'
    bytes: 3764
    sha256: 'a27f638bd2831f5c3dea654a75838930f2b11fbe550c4d4e1d5d7bd07157b2ee'
  - path: 'model.safetensors.index.json'
    bytes: 39200
    sha256: 'a93cf855a70c2821af876414dd0436b2fab2d152064c51f377004fd3ad331582'
  - path: 'model.safetensors'
    bytes: 1818493993
    sha256: '2349d5384ac91889b42979e21e62fe5c7955cfe6083663552cf275ad6a678463'
---
Pinned from the [mlx-tool.json](../../../tests/fixtures/mlx-tool.json) fixture used by sefai's own tests.
