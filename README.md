# sefai

A local AI companion that lives on your own machine: one small program that keeps a language model loaded, remembers you, and can speak first.
It runs on Apple Silicon (Metal) and NVIDIA GPUs (CUDA), with the CPU as a fallback, and nothing leaves your computer.

**Status: early rewrite.** Phase 0 of the [build plan](docs/build-plan.md) is done: the workspace, the ability system and the persona.
The model engine is phase 1, so sefai cannot talk yet.
The earlier versions (a Windows companion app, then a minimal Rust model runner) are kept on the [`old`](https://github.com/kanishkpaul/sefai/tree/old) branch.

## Why it's built this way

Most local-LLM tools are model runners: they answer a request and forget it.
A companion is the opposite workload. It holds one conversation for hours, so the cost that matters is re-reading the persona and history on every turn.
sefai embeds llama.cpp in-process so it can keep that work cached: each turn pays only for its new words, and the persona's state is saved to disk between runs.

Everything the companion can do is an **ability** (persona, memory, proactive messages) in its own crate, compiled in by a Cargo feature.
Adding or removing one is a small, local change; see [docs/abilities.md](docs/abilities.md).

## Try what exists today

You need a stable Rust toolchain.

```bash
cargo run -- abilities
cargo run -- prompt --persona abilities/persona/persona.example.toml "Rainy day. Talk?"
```

The second command prints the prompt the persona ability assembles. Once the engine lands, the same prompt goes to the model.

## Documentation

| Document | What it covers |
| --- | --- |
| [ARCHITECTURE.md](ARCHITECTURE.md) | The parts of sefai, where they live, and the rules between them |
| [docs/stack.md](docs/stack.md) | Language, engine, bindings and libraries, and why each was chosen |
| [docs/build-plan.md](docs/build-plan.md) | The phases from here to a released companion, with exit criteria |
| [docs/abilities.md](docs/abilities.md) | How to add or remove an ability |
| [docs/style.md](docs/style.md) | How code, comments and commits are written here |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Setting up, checks to run, and commit rules |
| [AGENTS.md](AGENTS.md) | Instructions for coding agents working in this repository |
| [docs/research/](docs/research/) | The ecosystem and inference research behind the design |

## Credits

sefai runs models through [llama.cpp](https://github.com/ggml-org/llama.cpp) and [ggml](https://github.com/ggml-org/ggml), by Georgi Gerganov and contributors.

## License

MIT, see [LICENSE](LICENSE).
