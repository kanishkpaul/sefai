# Instructions for coding agents

These instructions apply to any AI coding agent working in this repository (Claude Code, Codex, Cursor, Copilot, Gemini and others).
They are not optional, and they override an agent's default habits.

## What this project is

sefai is a local AI companion written in Rust. It embeds llama.cpp through the `llama-cpp-2` crate and runs on Apple Silicon (Metal) and NVIDIA (CUDA).
Capabilities such as persona, memory and proactive messages are **abilities**: one crate each under `abilities/`, wired in by Cargo features.
It is being built in phases; see `docs/build-plan.md` for which phase is current.

## Read before writing code

1. `docs/style.md`: how code, comments and commits are written here. **Read it in full before your first edit in a session.**
2. `ARCHITECTURE.md`: the crate map and the invariants you must not break.
3. The section of `docs/build-plan.md` for the phase you are working in.
4. `docs/abilities.md` if you are adding, changing or removing an ability.

## Hard rules

- **Never add attribution for yourself or any AI tool.** No `Co-Authored-By` trailer naming an assistant or a bot, no "Generated with ..." line, no agent name or tool name in commit messages, pull request descriptions, code comments or docs. Commits are authored by the human who owns the change. The `commit-msg` hook and CI reject violations; do not bypass them with `--no-verify`.
- **Respect the dependency rules.** `crates/core` depends on nothing. Abilities depend only on `sefai-core` (and `sefai-store` when it exists), never on each other, an engine or the runtime. Only `crates/sefai` names concrete abilities. Only `crates/engine-llama` touches llama.cpp.
- **No `unsafe`.** The workspace forbids it.
- **Pin `llama-cpp-2` exactly** (`=x.y.z`) and never upgrade it in the same commit as other work.
- **Don't add dependencies casually.** Justify each new crate in the commit message body.
- **Stay in scope.** Don't refactor, rename or reformat code unrelated to the task. Don't add features, hooks, traits or configuration "for later".
- **Never touch the `old` branch.** It is an archive.
- **Don't claim a check passed without running it.** Report failures as failures, with the output.

## How to write code here

The full guide is `docs/style.md`. The rules agents most often get wrong:

- Write the simplest code that solves today's problem. No abstraction without a second concrete use.
- Comments explain **why**, not what. Do not narrate code (`// loop over messages`), do not describe history (`// changed to fix ...`, `// new implementation`), do not leave commented-out code, and do not write `TODO` without an issue number.
- No decorative comments: no banners, no emoji, no "Note:", no "This function ...", no "simply" or "just".
- Doc comments (`///`) on public items say what the signature cannot: units, ordering, ownership, what empty means, plus `# Errors` and `# Panics` sections where they apply. Skip them when the name says everything.
- Comments are full sentences with a capital letter and a full stop.
- Error messages are lowercase phrases without a trailing period.
- Name things precisely and without abbreviations. Put units last: `timeout_ms`.
- Bound every loop, queue and cache with a named limit.
- Tests check one behaviour each, are named after it, and use no sleeps, network, or `#[ignore]`.

Before writing a comment, see whether a better name or a smaller function would make it unnecessary.

## Commands

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p sefai --no-default-features --all-targets -- -D warnings
cargo test --workspace
cargo run -- abilities
cargo run -- prompt --persona abilities/persona/persona.example.toml "hello"
```

All four checks must pass before you commit.
Run `git config core.hooksPath .githooks` once if `git config core.hooksPath` prints nothing.

## Commits

- One logical change per commit.
- Subject in the imperative mood, capitalized, no trailing period, about 60 characters at most: `Chunk prompt prefill by n_batch`.
- A body that says what changed and why, wrapped at 72 columns, when the subject alone isn't enough.
- No attribution trailers of any kind (see Hard rules).
- Commit only when the user asks you to. Never force-push `main`.

## When you finish a task

Say what you changed, what you ran to check it and what the output was, and anything you could not verify.
If you changed behaviour that a document describes, update that document in the same change.

## Design

Any change to a user-facing surface (GUI, CLI output, overlay, docs page) follows `design/`.
It is installed from the shared design kit, a separate master repository that lives next to
this one at `../design-kit`, and recorded in `design/kit.json`.

- Read `design/README.md` first: this project's direction. Skin: not chosen yet (default `candle`).
- Shared rules are in `design/system/DESIGN.md`. Check its "Before you add something" list
  before finishing a UI change.
- Colours, type, spacing, radii, shadows and motion come only from `design/tokens/*.css`
  (`--ui-*`, `--font-*`, `--text-*`, `--space-*`, `--radius-*`, `--shadow-*`, `--dur-*`).
  No hex values, no `--seed-*`, no ad-hoc rgba, no new shadows in components.
- Use the primitives in `design/system/COMPONENTS.md`. Pick a variant and size; never
  override a primitive's padding, radius or colour at the call site.
- Terminal output goes through `design/rust/theme.rs` and follows `design/system/TUI.md`:
  banner only in interactive sessions, `┊` tool rows, `NO_COLOR` respected, `--json` output
  free of colour and decoration.
- Never load fonts, icons or scripts from a CDN at runtime. Everything ships with the app.
- `design/README.md` belongs to this project; edit it freely. Everything else in `design/`
  comes from the master. A change that every project should get (a token, a primitive, a
  rule, a skin) belongs in the master at `../design-kit`, then reaches this repository through
  an update.
- From this repository's root: choose or change the skin with
  `node ../design-kit/scripts/kit.mjs use . <skin>`; pull master changes with
  `node ../design-kit/scripts/kit.mjs update .`, then merge any `*.kit-new` file it leaves and run
  update again. To stop the kit touching a file, add its path to `"ignore"` in
  `design/kit.json`.
- If `../design-kit` is not on this machine, treat `design/` as the source of truth and don't
  regenerate it by hand.
