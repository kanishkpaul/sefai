# Bug-fix plan

Fix the retained inference failure cases before building the harness.
The audited code is the immutable `c89958a429053b62241b05fb5ec51e33c1937077` snapshot, not the stale local branch previously called `main`.
Findings below come from source inspection unless a runtime check is explicitly recorded.
The old implementation is archived locally; its removal does not constitute a tested replacement.

## Ordered work

| Order | Defect and evidence | Smallest repair in the new engine | Regression evidence |
| --- | --- | --- | --- |
| B1 | `runner/src/main.rs:97–105` puts every prompt into a 512-token batch | Prefill `chunks(batch_tokens)` with checked monotonic positions and logits only where needed | Empty/511/512/513/3,000-token cases; compare prompt accounting |
| B2 | Lines 87–95 and 128–133 allocate a 2,048-token context without reserving generation capacity; unchecked index casts | Validate checked prompt+output arithmetic and configured/model context limits before decoding; stop or report context full | Exact limit, over limit, zero output, large numeric values, cancellation during prefill |
| B3 | Lines 121–125 decode each token independently with `from_utf8_lossy` | Keep only an incomplete UTF-8 suffix across tokens; explicitly handle invalid final bytes and hide control tokens | Bengali, emoji, combining marks, malformed tail and split-at-every-byte fixtures |
| B4 | Lines 93–95 force BOS and skip chat templating | Render the model's verified template from role-preserving messages; use tokenizer BOS/EOG policy | Models with/without BOS, multiple EOG IDs, tool template and raw mode |
| B5 | Lines 114–115 call `sample` and then `accept` | Accept once according to the pinned binding's actual API | A stateful sampler fixture catches duplicate acceptance; greedy alone would not |
| B6 | Lines 79–82 set layer count/main GPU without explicit device/split policy | Set CPU/device/cache-placement options together and expose actual backend placement | CPU-only run logs, invalid index, one selected GPU and unavailable-backend behavior |
| B7 | `runner/Cargo.toml` uses a non-exact `llama-cpp-4` version; old runner has no tests | Introduce exactly pinned `llama-cpp-2` only in its engine crate, then add engine regressions | Dedicated dependency commit; all four required checks; tiny-model integration |
| B8 | New-branch `Prompt::to_messages` prepends sections but can retain an existing conversation system message | Define exactly one supported system-message composition policy at the template boundary | Existing system message plus sections, blank sections, user text containing role markers |
| B9 | New-branch prompt and persona prototypes have unbounded vectors/file reads; new runtime will ingest Markdown and tools | Bound at file, session, prompt and transport boundaries; avoid adding an arbitrary low-level cap that silently drops text | Oversize input fails with an actionable limit; no partial state mutation |
| B10 | Old CLI prints user/model text straight to stdout; future TUI must not execute terminal control sequences | Sanitize terminal controls at the renderer boundary, preserving legitimate Unicode and ordinary whitespace | OSC/CSI, carriage return, malformed escape sequences and long strings |

B1–B7 gate Phase 1.
B8 is required before model-native chat composition; B9 and B10 gate the relevant Markdown/TUI/MCP phase as well as any new code path that consumes external bytes.
Do not mark these complete because the archived source no longer compiles in the active tree.

## Retired defects: record, do not repair dead paths

| Finding | Evidence | Disposition |
| --- | --- | --- |
| Full model reload for every CLI reply | `companion/backend/inference/engine.py:202–250` starts `subprocess.run` per generation | Delete this execution path; persistent engine owner replaces it |
| Blocking generation on async request path | `backend/app.py:150` invokes synchronous generation inside `async process_user_message` | Use the bounded engine worker; no Python backend |
| Silent canned replies after engine failure | `backend/inference/engine.py:135–150,186–204` | Explicit inference errors; no fake success |
| Current message repeated in system context | `backend/app.py` persists input before recall; `memory/manager.py:19–33` includes recent input plus a current-input section | Keep role-separated session records; test the current user turn appears once |
| Summary can include the preceding summary and has no explicit stored-size limit | `memory/manager.py:37–54` | Markdown memories with explicit writes and bounded recall; no recursive summary pipeline |
| State triggers bypass quiet hours | `autonomy/engine.py:66–84` checks enabled/quiet mode but does not use the quiet-hours check in `evaluate_trigger` | Retire autonomy entirely |
| Pipe handler can hang or die on malformed input | `ipc/named_pipe_server.py:91–94` parses without a request-level recovery boundary and waits on `future.result()` without a timeout | Retire named pipes; test new engine/MCP timeouts |
| README claims main is an archive and points to a rewrite no longer present there | Root README at audited main | Replace with honest foundation status |

These are source-level findings, not a claim that the Windows app was executed on this Mac.
The phase plan deliberately drops mood scoring, autonomy, tray settings and WPF recovery work because they do not serve the requested runner/harness.

## Baseline checks

On the original runner, `cargo fmt --manifest-path runner/Cargo.toml -- --check` failed at the model-parameter chain and context-parameter line wrapping.
`cargo test --manifest-path runner/Cargo.toml --locked --offline` compiled successfully and reported `0 passed; 0 failed` because there are no Rust tests.
`PYTHONPATH=companion python3 -m pytest companion/tests -q` failed with `No module named pytest`.
The earlier unittest discovery attempt also failed import for all four pytest modules; it is not a valid test pass.
No real GGUF inference, Windows execution or GPU validation was performed for this audit.

## Review discipline

Each repair is a small logical commit, with one behavioral regression first, no unrelated reformatting and an updated document when its behavior changes.
Use the fixed engine through a real model before advancing the phase gate.
Record failed commands alongside successful ones; do not trade missing evidence for a broad claim such as "fully tested".
