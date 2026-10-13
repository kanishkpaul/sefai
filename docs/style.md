# Style

This is how code in sefai is written, commented and committed.
It applies to every contributor, human or agent.
When this guide and a habit disagree, this guide wins; when this guide is silent, match the code around you.

The rules come from places that have earned their opinions:
antirez's taxonomy of comments in Redis,
the rust-analyzer style guide,
TigerBeetle's TigerStyle,
the Rust API Guidelines,
and Ousterhout's *A Philosophy of Software Design*.
Links are at the end.

## What good taste means here

Taste is mostly subtraction.
Code in good taste solves the problem in front of it with the fewest moving parts a careful reader needs to hold in their head.
It does not anticipate problems nobody has, it does not decorate itself, and it explains only what the code cannot say on its own.

Three priorities, in order:

1. **Correct.** Handle every error. Bound every loop and queue. Never panic on user input or model output.
2. **Fast and small.** sefai runs beside the user's other apps on a laptop. Memory, binary size and latency are features. Do the back-of-the-envelope math before writing the code, not after profiling it.
3. **Easy to change.** Someone should be able to delete an ability, or swap the engine, by touching the places the docs say to touch and nothing else.

## Code

### Shape

- **Write the obvious version first.** Add a trait, generic, builder or macro only when a second concrete use exists. Two similar blocks are cheaper than one wrong abstraction.
- **Keep control flow in the caller.** Helpers compute; the parent decides. Don't hide an early return or a branch inside a helper whose name doesn't say so.
- **Split instead of flagging.** A function that takes a `bool` its callers always pass as a literal is two functions.
- **Keep functions short enough to see whole.** Around 70 lines is a hard look, not a hard limit. Long `match` blocks over an enum are fine.
- **Order a file for a first-time reader.** Public items first, the main type before its impls, helpers after the code that uses them.
- **Make illegal states unrepresentable.** Encode preconditions in types (`NonZeroU32`, newtypes, enums) so the caller proves them once, instead of every callee checking them.
- **Keep invariants private.** A field is `pub` only if any value is valid. Otherwise construct through a function that checks it.
- **Put a limit on everything.** Context sizes, queue depths, retries, memory recall counts and loop iterations all have explicit, named maximums.

### Names

- Use precise nouns for types and data, and verbs for functions. `Prompt`, `prepare`, `recall`; not `Manager`, `handle`, `process`, `do_stuff`.
- Don't abbreviate. `context`, `message`, `token_count`. Established short names are fine: `ctx`, `id`, `db`, `io`.
- Put units and qualifiers last: `timeout_ms`, `context_tokens_max`, `n_gpu_layers` only where it mirrors llama.cpp.
- Name counts `n_*` or `*_count`, indices `*_index`. Never mix them up; an index plus one is a count.
- Name things after what they are in the domain, not after the pattern used to build them. No `PersonaFactory`, no `AbilityManagerImpl`.

### Errors and panics

- Libraries return typed errors with `thiserror`. The binary uses `anyhow` and adds context at each boundary: `.with_context(|| format!("loading persona from {}", path.display()))`.
- Error messages are lowercase phrases with no trailing period, so they chain cleanly: `loading model: file not found`.
- `unwrap` and `expect` are for invariants the code itself guarantees. Write the invariant in the `expect` message: `.expect("prompt always ends with the user message")`.
- A failing ability degrades a turn; it never aborts one. The runtime logs the failure and carries on.
- Assert the things that would be catastrophic and surprising: positions are monotonic, batch sizes stay under `n_batch`, a cache prefix really matches. `debug_assert!` for hot loops, `assert!` elsewhere.

### Dependencies and `unsafe`

- Every new dependency needs a sentence in the pull request saying why the standard library or an existing dependency won't do. Prefer crates that are widely used and actively maintained.
- Pin the llama.cpp bindings to an exact version (`=x.y.z`). They do not follow semver.
- No `unsafe` in sefai's own crates. FFI belongs to the bindings crate. If that ever has to change, the `unsafe` block gets a `// SAFETY:` comment that proves every invariant.

### Performance

- Know the cost model. Decode speed is memory bandwidth divided by bytes read per token; prefill is compute-bound; the KV cache costs `2 × layers × kv_heads × head_dim × context × bytes`. A change that touches the hot path says what it does to those numbers.
- Don't allocate in the token loop. Reuse buffers; let the caller own allocations.
- Measure against `llama-bench` on the same model and build before claiming a speedup. `sefai bench` arrives in phase 1 for exactly this.

### Tests

- Test behaviour through the public API of a crate. Each test checks one thing and is named after it: `failing_ability_does_not_abort_turn`.
- Keep fixtures minimal. A persona in a test has the two fields the test needs.
- No `#[ignore]`, no sleeps, no network. Time comes from an injected clock.
- Tests that need a real model are gated behind the `SEFAI_TEST_MODEL` environment variable and run in CI with a tiny cached GGUF.

### Formatting

`cargo fmt` with default settings, and `cargo clippy` with the workspace lints, both clean. Don't argue with either in review.

## Comments

Code says *what* and *how*. Comments exist for what the code cannot say: *why*, the shape of the design, and the knowledge a reader needs from outside the file.
A comment that restates the code is noise that will eventually lie.

### Comments worth writing

- **Doc comments on public items** (`///`), written for the caller, not the implementer. First line is a one-sentence summary in the third person: `/// Adds a system section that renders before the conversation.` Then anything the signature can't say: units, ownership, ordering, what "empty" means. Add `# Errors`, `# Panics` and `# Safety` sections where they apply. Skip the doc comment when the name and types already say everything.
- **Module and crate docs** (`//!`) that explain what the module is for, what it deliberately does not do, and how it fits the architecture. This is where design rationale lives.
- **Why comments** next to a choice that would otherwise look wrong: `// llama_sampler_sample already accepts the token, so accepting it again would double-count it for penalties.`
- **Teacher comments** that bring in outside knowledge the code depends on: how RoPE positions work, why a UTF-8 tail can be incomplete, what a GGUF field means.
- **Checklist comments** where a change here forces a change elsewhere and the two cannot be unified: `// Keep in sync with the feature list in crates/sefai/Cargo.toml.`
- **Guide comments**, sparingly, to mark the phases of a long function the reader would otherwise have to discover: `// Prefill: feed the prompt in n_batch-sized chunks.`

### Comments not to write

- **Narration of the code.** `// Increment the counter`, `// Loop over the messages`, `// Return the result`.
- **History.** `// Changed to fix the 512-token bug`, `// Previously used llama-cpp-4`, `// New implementation`. History belongs in commit messages, where it can't go stale.
- **Commented-out code.** Delete it. Git remembers.
- **Untracked debt.** A `TODO` or `FIXME` is allowed only with an issue link: `// TODO(#12): evict by recency once memories exceed the recall budget.` Otherwise fix it, or describe the limitation as a fact in the module docs.
- **Attribution or provenance.** No author names, no "generated by", no agent names, no dates. Git has all of that.
- **Decoration.** No banner lines of `====`, no emoji, no ASCII boxes, no ALL-CAPS shouting except `SAFETY:` and `TODO(#n):`.
- **Hedging and filler.** No "basically", "simply", "just", "note that", "it's important to", "this function". Say the thing.
- **Apologies and speculation.** If a comment says "might", "should work" or "hopefully", either verify the claim or write down exactly what is unknown.

### Form

- Full sentences, capital letter, full stop, one space after `//`. A short trailing comment on a line may be a phrase.
- Put a comment directly above the code it explains, never at the end of a long line.
- Write the comment for someone who knows Rust but has never seen this file.
- When a comment needs more than a paragraph, the explanation probably belongs in module docs or `ARCHITECTURE.md`.
- Before writing a comment, see whether a better name, a smaller function or a type would make it unnecessary. Rename first, comment second.

### A small example

```rust
// Not this:

// This function decodes tokens.
// Loop through each token and decode it, then return the string.
// Updated to fix unicode issues!!
fn decode(tokens: &[Token]) -> String { ... }

// This:

/// Appends the bytes for `token` and returns the text that is now complete.
///
/// A token can end partway through a multi-byte UTF-8 character, so trailing
/// bytes of an unfinished character are held back until the next token
/// completes them.
fn push(&mut self, token: Token) -> &str { ... }
```

## Documentation files

- One sentence per line in Markdown, no hard wrapping. Diffs stay readable and editing stays cheap.
- Lead with the answer. A reader who stops after the first paragraph should still know what the document says.
- Prefer a table when comparing things along the same columns, prose when explaining reasoning, and a list only for genuinely parallel items.
- Name files, types and commands so they can be found with search; don't paste long code that will go stale.
- `ARCHITECTURE.md` changes rarely and describes what is unlikely to change. Fast-moving detail lives in module docs next to the code.

## Commits

- One logical change per commit. Formatting-only or move-only changes get their own commit.
- Subject line: imperative mood, capitalized, no trailing period, under about 60 characters. `Chunk prompt prefill by n_batch`, not `fixed stuff` or `feat: Added chunking.`.
- Body, when the subject isn't enough: what changed and why, wrapped at 72 columns. Mention the alternative you rejected if a reviewer would ask.
- **No attribution trailers of any kind for tools or agents.** No `Co-Authored-By` naming Claude, Copilot, Codex, Cursor, Gemini or any other assistant, and no "Generated with" lines. Commits are authored by the person responsible for them. The `commit-msg` hook in `.githooks/` rejects them, and CI checks again.
- Don't mention people with `@` in commit messages.

## Review checklist

Before a change is done, its author can answer yes to each:

- Does every error path either recover or carry enough context to debug from the message alone?
- Is every new loop, buffer and queue bounded?
- Could any comment be replaced by a better name? Does any remaining comment narrate, apologize, or describe history?
- Does the change respect the invariants in `ARCHITECTURE.md`, especially the dependency rules between crates?
- If it touches the token loop, what does it do to decode and prefill speed, and was that measured?
- Do `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass?
- Is the commit message free of tool or agent attribution?

## Sources

- antirez, [Writing system software: code comments](http://antirez.com/news/124) (2018)
- rust-analyzer, [Style](https://github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/style.md)
- TigerBeetle, [TigerStyle](https://github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md)
- [Rust API Guidelines: Documentation](https://rust-lang.github.io/api-guidelines/documentation.html)
- Alex Kladov, [ARCHITECTURE.md](https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html)
- John Ousterhout, *A Philosophy of Software Design*, chapters on comments
