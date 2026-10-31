# In-memory weight patch

This is the `mlx-lm` package from `oxiglade/mlx-rs` revision `579199b4bb5e39b35f33fc6a59cb90e2c031a6a7`.
Only `src` (including its unit-test modules), the package README and the upstream license notices are retained; the integration tests, examples and fixtures are omitted.
The manifest is standalone: workspace fields are explicit, `mlx-rs` comes from the same git revision, and the unused `hf-hub` and `oracle-hooks` features are removed while their `cfg` gates stay unchanged.

The source changes add one loading entry point, one template source and one evaluation change, and nothing else:

- `WeightSource::Memory` holds a caller-owned array in place of a checkpoint tensor.
- `WeightManifest::replace` removes tensors mapped to `None` and inserts or replaces the rest.
- `Model::from_dir_with_tensors` loads a local bundle with a caller-supplied configuration and those replacements; `Model::from_dir` now calls it with the file's configuration and no replacements.
- Sampling builds its NaN, infinity and empty-support checks lazily and evaluates them with the sampled token in one evaluation, rather than forcing a separate GPU synchronization for each checked row; an invalid row still returns the same error and its sample is discarded.
- `Tokenizer::from_dir` reads `chat_template.jinja` when `tokenizer_config.json` declares no `chat_template`, as Hugging Face tokenizers do; sefai's bundle admission checks that file's size, location and UTF-8 first.

Replacement arrays pass the same strict name, shape and dtype matching as file-backed tensors.
Sefai uses it to load GPTQ and AWQ checkpoints: their packed layers are repacked into MLX affine weights, scales and biases in memory, and every other tensor still loads from the checkpoint's own safetensors files.
No file is written and no FFI code changes.

Keep this patch at the selected upstream revision until upstream offers an equivalent in-memory loading API and template file support.
