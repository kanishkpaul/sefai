---
schema: 1
id: qwen3-0-6b-mlx
repository: mlx-community/Qwen3-0.6B-4bit
revision: '73e3e38d981303bc594367cd910ea6eb48349da8'
format: mlx
entry: '.'
parameters_millions: 600
purposes: [chat]
license: Apache-2.0
layers: 28
kv_heads: 8
head_dim: 128
trained_context: 40960
cache_element_bytes: 2
evidence: docs/mlx.md
files:
  - path: 'config.json'
    bytes: 937
    sha256: '15d3ac26c043ae477273ed5802ee0f0b33bb14f18c9d3dd70910c02d906e3f1f'
  - path: 'tokenizer.json'
    bytes: 11422654
    sha256: 'aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4'
  - path: 'tokenizer_config.json'
    bytes: 9706
    sha256: '253153d0738ceb4c668d2eff957714dd2bea0b56de772a9fdccd96cbf517e6a0'
  - path: 'model.safetensors.index.json'
    bytes: 49731
    sha256: '7b294141456f6904936db03c00bca50fb5f6198f652fe8483f9cd2a1018accfb'
  - path: 'model.safetensors'
    bytes: 335450584
    sha256: '392e8d466d56100ada00eb82031fb854297fc9e389b7d303eba3af114e87bce2'
---
Pinned from the [mlx.json](../../../tests/fixtures/mlx.json) fixture used by sefai's own tests.
