# Contributing

## Setup

You need a stable Rust toolchain (`rust-toolchain.toml` selects it) and Git.
From phase 1 on, building the engine also needs CMake and a C++ compiler:

| Platform | Extra tools |
| --- | --- |
| macOS | Xcode command-line tools (`xcode-select --install`), `brew install cmake` |
| Linux with NVIDIA | CMake, a C++ compiler, the CUDA toolkit and driver |
| Windows with NVIDIA | Visual Studio Build Tools 2022, LLVM (for `libclang`), CMake, Ninja, the CUDA toolkit and driver |

Turn on the repository's Git hooks once per clone:

```bash
git config core.hooksPath .githooks
```

## Checks

Run these before every commit. CI runs the same ones on macOS and Linux.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p sefai --no-default-features --all-targets -- -D warnings
cargo test --workspace
```

The third line checks that the binary still builds with every ability removed.

## Making a change

1. Read [ARCHITECTURE.md](ARCHITECTURE.md) and the part of [docs/build-plan.md](docs/build-plan.md) your change belongs to.
2. Follow [docs/style.md](docs/style.md). It covers code, comments, docs and commits.
3. Keep one logical change per commit, and one topic per pull request.
4. Update the docs your change makes untrue in the same pull request.
5. For anything on the token path, include before-and-after numbers from `llama-bench` or `sefai bench`.

## Commits and attribution

Commit messages follow the format in [docs/style.md](docs/style.md#commits).

sefai's history credits people only. Commits and pull requests must not carry attribution for AI tools or coding agents: no `Co-Authored-By` lines naming an assistant, and no "Generated with" footers.
If you used a tool, you are still the author and you are responsible for the change.
The `commit-msg` hook rejects such lines, and CI checks every pushed commit with the same script.

## Adding a dependency

Say in the pull request why the standard library or an existing dependency won't do.
Prefer crates that are widely used and actively maintained, and pin the llama.cpp bindings with `=`.
