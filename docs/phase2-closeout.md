# Phase 2 acceptance

Phase 2 implementation and local acceptance are complete in four stacked PRs.
Review, cross-platform CI qualification and merge are still required before closing the phase gate.
No CI run has been polled or assumed green.

## Stack and merge order

| Order | PR | Base | Scope |
| --- | --- | --- | --- |
| 1 | [#8](https://github.com/kanishkpaul/sefai/pull/8) | `main` | Typed Markdown schema, workspace discovery and selected loading |
| 2 | [#9](https://github.com/kanishkpaul/sefai/pull/9) | `codex/phase2-schema` | Atomic writes, explicit memory/session persistence and bounded tail |
| 3 | [#10](https://github.com/kanishkpaul/sefai/pull/10) | `codex/phase2-files` | Schema fixtures, process/restart/filesystem qualification |
| 4 | `codex/phase2-closeout` PR | `codex/phase2-persistence` | Final consistency fixes, documentation and acceptance evidence |

Review and merge oldest first, using merge commits to retain stack ancestry.
After each parent merge, retarget the next PR to `main` and review its checks against that base before merging.
Keep stack branches until their children have been retargeted.
No force-push or history rewrite is needed.

## Acceptance matrix

| Requirement | Evidence | Remaining qualification |
| --- | --- | --- |
| Editor-owned customization | Schema fixture for every kind; real CLI restart observes system/preferences/skill/memory/session edits; selected profile and CLI precedence regressions | New three-OS CI results |
| Strict configuration | Unknown/duplicate keys, unsupported versions, invalid ranges, YAML aliases/tags and nonportable IDs rejected | None beyond CI |
| Inert body prose | Fences never execute; body cannot change config; MCP definitions never launch or connect | MCP execution is Phase 7 |
| Bounded loading | 1 MiB ordinary files/composed prompt, 16 KiB front matter, bounded selection/recall/directories | No throughput or RSS claim |
| Safe writes | Same-directory synced staging, OS lock, expected snapshots checked before rename, stale/during-staging edit tests, invalid candidate rollback | Windows sharing/rename tests in CI; hostile path races and arbitrary power-loss durability are outside this guarantee |
| Durable memory | Explicit remember/edit/forget survive restart; deterministic literal recall | Automatic extraction/embeddings intentionally absent |
| Sessions | Literal role-safe framing, every incomplete record prefix rejected, CRLF and Unicode round-trips, 64 MiB disk/1 MiB tail bounds, separate-process resume | Automatic generation/outcome persistence is later harness lifecycle work |
| Lock/process failures | Separate process holds the lock, concurrent writer fails visibly, process termination releases the lock; failed source removes its temporary file | Windows/Linux execution of new tests |
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

The final suite passes 84 Rust tests: 46 binary tests, 12 CLI integration tests, 4 core tests and 22 engine tests.
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

The remaining product work begins with Phase 3's persistent terminal TUI after this stack is accepted.
Automatic turn recording, live-state reload recovery, permission decisions and tool execution belong to the harness/TUI phases that own those lifecycles.
Phase 2 gives them validated file APIs and a bounded, role-preserving session representation.
The one-shot CLI reports and retains a malformed manual edit rather than silently restoring a backup or presenting it as valid.
`cache/` remains disposable and contains no authoritative customization data.
