# Phase 2 acceptance

Phase 2 was merged through PRs #8–#11 on 19 October 2026.
The Windows, macOS, Linux and attribution checks passed before the final protected merge.
CUDA/Vulkan runtime qualification remains a separate hardware gate.

## Published stack

| Order | PR | Original base | Scope |
| --- | --- | --- | --- |
| 1 | [#8](https://github.com/kanishkpaul/sefai/pull/8) | `main` | Typed Markdown schema, workspace discovery and selected loading |
| 2 | [#9](https://github.com/kanishkpaul/sefai/pull/9) | `codex/phase2-schema` | Atomic writes, explicit memory/session persistence and bounded tail |
| 3 | [#10](https://github.com/kanishkpaul/sefai/pull/10) | `codex/phase2-files` | Schema fixtures, process/restart/filesystem qualification |
| 4 | [#11](https://github.com/kanishkpaul/sefai/pull/11) | `codex/phase2-persistence` | Final consistency fixes, documentation and acceptance evidence |

PR #8 merged first.
Strict up-to-date protection required synchronizing the remaining stack with main.
PR #11 merged the remaining ancestry as `33f1a8d2172db93eb0b8cb1759fa63c84d62d6ee`, which also marked #9 and #10 merged.
No force push or protection bypass was used.

## Acceptance matrix

| Requirement | Evidence | Remaining qualification |
| --- | --- | --- |
| Editor-owned customization | Schema fixture for every kind; real CLI restart observes system/preferences/skill/memory/session edits; selected profile and CLI precedence regressions | Required three-OS CI passed |
| Strict configuration | Unknown/duplicate keys, unsupported versions, invalid ranges, YAML aliases/tags and nonportable IDs rejected | None |
| Inert body prose | Fences never execute; body cannot change config; MCP definitions never launch or connect | MCP execution is Phase 5 |
| Bounded loading | 1 MiB ordinary files/composed prompt, 16 KiB front matter, bounded selection/recall/directories | No throughput or RSS claim |
| Safe writes | Same-directory synced staging, OS lock, expected snapshots checked before rename, stale/during-staging edit tests, invalid candidate rollback | Windows sharing/rename CI passed; hostile path races and arbitrary power-loss durability are outside this guarantee |
| Durable memory | Explicit remember/edit/forget survive restart; deterministic literal recall | Automatic extraction/embeddings intentionally absent |
| Sessions | Literal role-safe framing, every incomplete record prefix rejected, CRLF and Unicode round-trips, 64 MiB disk/1 MiB tail bounds, separate-process resume | Automatic turn/outcome persistence is implemented in Phase 3 |
| Lock/process failures | Separate process holds the lock, concurrent writer fails visibly, process termination releases the lock; failed source removes its temporary file | Windows/Linux CI passed |
| Workspace containment | Unix escaped-read/symlink-write tests; Windows junction test; resource traversal rejected | Advisory checks are not an OS sandbox |
| Migration/rollback | Unsupported versions fail without rewriting; malformed candidates preserve existing bytes; explicit matching-version backup restoration documented | Schema 1 is the initial version; no automatic migration |

## Local checks

On macOS, all four required Rust checks pass:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo clippy --locked -p sefai --no-default-features --all-targets -- -D warnings
cargo test --locked --workspace
```

The Phase 2 final suite passed 84 Rust tests: 46 binary tests, 12 CLI integration tests, 4 core tests and 22 engine tests.
`cargo test --locked -p sefai --no-default-features` passes 53 tests.
`python3 -m unittest discover -s tests -v` passes all 16 attribution regressions.
`cargo run --locked -- prompt hello` prints one user message.
The existing three-OS matrix runs the new default/no-engine tests; Windows-only sharing/junction tests replace the Unix-only symlink checks in that environment.

A local real-model run used the existing hash-verified SmolLM2 fixture through a Markdown profile, system instruction and runner settings; CPU inference returned a reply with an 18-token prompt.
A zero-output benchmark probe confirmed CLI override of the profile/config output budget.
No performance or memory reduction is claimed from these smoke tests.
No new model download or runtime dependency was needed for qualification.
The only new production dependencies are typed Serde decoding and deserialize-only YAML parsing in the binary; `core` remains dependency-free.

## Phase boundary

Phase 3 builds the persistent terminal TUI on this merged foundation.
Automatic turn recording, live-state reload recovery, permission decisions and tool execution belong to the harness/TUI phases that own those lifecycles.
Phase 2 gives them validated file APIs and a bounded, role-preserving session representation.
The one-shot CLI reports and retains a malformed manual edit rather than silently restoring a backup or presenting it as valid.
`cache/` remains disposable and contains no authoritative customization data.
