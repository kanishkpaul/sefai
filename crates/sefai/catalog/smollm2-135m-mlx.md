---
schema: 1
id: smollm2-135m-mlx
repository: mlx-community/SmolLM2-135M-Instruct-8bit
revision: '0f0d9b8218915bc34d401e1a340b8c049d300d5e'
format: mlx
entry: '.'
parameters_millions: 135
purposes: [chat]
license: Apache-2.0
layers: 30
kv_heads: 3
head_dim: 64
trained_context: 8192
cache_element_bytes: 2
evidence: docs/mlx.md
files:
  - path: 'config.json'
    bytes: 1097
    sha256: '5da91905efdfde905f566d76d4031e6ca7c4cb2a01dc4e3812a70b855188fc73'
  - path: 'tokenizer.json'
    bytes: 3522656
    sha256: '7d27c493c729a66ecefc837280b05d948b1ed50d130eebdbf911b1b36cf38ed7'
  - path: 'tokenizer_config.json'
    bytes: 3764
    sha256: 'a27f638bd2831f5c3dea654a75838930f2b11fbe550c4d4e1d5d7bd07157b2ee'
  - path: 'model.safetensors.index.json'
    bytes: 48979
    sha256: 'd016e7514e8bfed162eb9d0716c73244c1b73792db3143e92e54138c1772db44'
  - path: 'model.safetensors'
    bytes: 143030401
    sha256: 'ec569c771e56424636be99625aba9031abc525a68a5f4ab16d678eca96fef12c'
---
Pinned from the [mlx-llama.json](../../../tests/fixtures/mlx-llama.json) fixture used by sefai's own tests.
