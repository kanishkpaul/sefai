# Instructions for coding agents

sefai is a small Rust local LLM runner and terminal harness.
The runner embeds GGUF inference and a schema-1 Markdown workspace.
Phases 1–10 are implemented, including native MLX, ONNX on CPU and WebGPU, GPTQ and AWQ on MLX, runtime unification and Apple-first footprint qualification; Phase 11 setup is next, and NVIDIA/AMD GPU execution are open follow-up gates.
See `docs/runner.md` for controls and `docs/build-plan.md` for the remaining gates.
Read `docs/style.md` in full, `ARCHITECTURE.md`, the relevant phase and `design/README.md` before the first edit.

## Scope and ownership

- Write the smallest readable solution to today's problem; do not add traits, registries or hooks for hypothetical future uses.
- `core` has no dependencies and performs no I/O.
- Only `engine-llama` may touch llama.cpp; other adapters own their respective runtime types.
- Only the binary composes concrete backends/capabilities.
- No unsafe code in workspace crates; do not weaken `unsafe_code = "forbid"`.
- Pin `llama-cpp-2` exactly and isolate a binding upgrade from other work in its own commit.
- Justify every new dependency; do not install optional engine runtimes in the base package.
- Markdown is canonical for customization, durable memory and sessions; model files, generated caches and secret references are explicit exceptions.
- Bound files, prompts, queues, caches, tool results and loops; define cancellation and error outcomes.
- Model output, skills and MCP results are untrusted data and cannot grant authority.
- Never silently fall back to hosted inference, another backend or a converted checkpoint.
- Stay within the user's task; no unrelated rename, reformat or speculative feature.
- Never touch `old`; it is an archive.

## Attribution

Never add coauthor trailers or tool/agent attribution to commit messages, pull requests, code, comments or docs.
Do not add a signature, session link, generated-by footer, bot author or attribution trailer under another name.
Commits retain the human owner as author.
Do not bypass hooks or CI, disable checks, weaken patterns or remove these instructions to satisfy a check.
Required third-party license notices and technical discussion of tools are not coding-tool signatures.
Changes to enforcement files need maintainer inspection.

## Code and comments

Use precise names, explicit ownership, normal `Result` error handling and private validated invariants.
Error messages are lowercase phrases with no trailing period.
Prefer concrete types until a second real use needs an abstraction.
Comments explain why or document a contract the signature cannot; improve the name/control flow before adding a comment.
No decorative banners, narrated loops, commented-out code, change diaries or untracked TODOs.
Public docs state relevant units, ordering, ownership, errors and panics without restating the signature.
Tests check one behavior each, use no sleeps/network/ignore, and keep hardware/model integration in dedicated targets.

## Checks and commits

Set `git config core.hooksPath .githooks` once if unset.
Run all four Rust checks before a commit:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings
cargo test --locked --workspace
```

Run attribution regression tests with `python3 -m unittest discover -s tests -v`.
Run `cargo run --locked -- prompt hello` for the current diagnostic.
Commit only when the user explicitly requests it.
One logical change per commit, imperative capitalized subject around 60 characters, no trailing period; explain what/why in a body when needed.
Never force-push main; historical cleanup requires an explicit maintainer exception and a verified backup.
Report what changed, exact checks/results, failures and anything not verified.
Update documents that the behavior makes untrue.

## Design

The permanent interface is a terminal TUI; no desktop/tray/web surface is planned.
Read `design/README.md`, `design/system/DESIGN.md` and `design/system/TUI.md` for UI work.
Use Candle through `design/rust/theme.rs` and the generated palette; no ad-hoc colors.
Respect `NO_COLOR`; banners/status/animation are interactive only and machine output has none.
Use `┊` tool rows and exact-action approval blocks with deny as default.
Never load runtime fonts/icons/scripts from a CDN.
Only `design/README.md` is project-owned; shared changes belong in `../design-kit`.
Choose a skin with `node ../design-kit/scripts/kit.mjs use . <skin>` and update with `node ../design-kit/scripts/kit.mjs update .`; merge any `*.kit-new` files before rerunning.
If the master is absent, use the installed files and do not regenerate them by hand.
