# Bounded local harness

Phase 4 adds a bounded generation/tool loop to the persistent terminal worker.
Tools are disabled by default.
Enable the explicit SmolLM2 text-call protocol for a compatible GGUF:

```sh
cargo run --locked -- --workspace /path/to/workspace --profile tools chat --tool-protocol smollm2
```

The qualified model is SmolLM2-1.7B-Instruct Q4_K_M from the publisher's GGUF repository.
Its immutable revision, byte count and SHA256 are in [the fixture manifest](../tests/fixtures/tool-gguf.json).
The [publisher's function-calling example](https://huggingface.co/HuggingFaceTB/SmolLM2-1.7B-Instruct#function-calling) uses a prompted `<tool_call>` JSON array.
This mode explicitly uses that text protocol, the model's native chat template, and a distinct `tool` result role.
The optional [MLX adapter](mlx.md) qualifies the same harness with the producer's SmolLM2-1.7B Q8 bundle; the small Qwen fixture is qualified for generation only.
It does not claim a native structured-tool API, arbitrary model compatibility or guaranteed tool reliability.
Unsupported templates and malformed proposals fail visibly; there is no conversion, hosted fallback or synthetic answer.
The 135M regression fixture is not qualified for tools.

## Authority

The runtime exposes only `local-memory/read_memory` and `local-memory/remember_memory`.
Local memory tools provide no shell, arbitrary file or network operation.
The optional [MCP client](mcp.md) adds a separately selected external connection and exact approval for every proposed MCP call.
The two tool definitions are runtime-owned; Markdown prose, tool results and model replies cannot add one.
Before composing instructions, the worker freezes the canonical workspace path and at most 128 existing memory IDs for reads.
A memory added after that snapshot is unavailable until a new user turn.
Creation accepts a portable new ID and at most 8,192 UTF-8 text bytes; existing files are never replaced.
Memory edit and forget remain explicit CLI operations with revision checks.

A proposal must be the entire response: one complete `<tool_call>[{"name":"…","arguments":{…}}]</tool_call>` block.
Only one call is accepted at a time; unknown fields, duplicate keys, unsupported names, invalid IDs, batches and truncated proposals fail.
There is at most one pending approval.
Both reads and writes require approval, even though they remain local.
The preview shows the exact workspace, server, tool, arguments and consequence.
Non-ASCII argument characters use explicit Unicode escapes to make deceptive controls visible.
`y` approves once; `n`, Esc and Enter deny.
Pasting `y` cannot approve; no always-allow option exists.
The plain interface reads one bounded approval line and accepts only `y`.
Its stdin reader remains line-blocking; a deadline or signal closes runtime authority immediately, while the plain input display may wait for a newline or EOF before consuming that status.
Plain piped input is an explicit source of user decisions, never model output.

After approval, the worker consumes the exact proposal once, validates scope again, persists a dispatch intent, checks cancellation/deadline, and revalidates immediately before dispatch.
An attempted exact call cannot repeat during that turn, including a denied, failed or uncertain call.
There are no automatic retries, recovery dispatches or regenerate command.
A new user turn requires fresh approval for every proposal.
Tools cannot grant permissions through their result text.
Results use a distinct `tool` role with runtime provenance, and are passed to the next generation as untrusted data.
After a result the SmolLM2 prompt explicitly requests a normal answer; general multi-tool planning quality is not qualified.
The runtime still enforces all bounds if the model proposes another call.

## Budgets and outcomes

| Resource | Limit |
| --- | --- |
| Generation steps | 8 per turn |
| Generated tokens | 4,096 total, further capped by the profile per step |
| Attempted tools | 4 per turn |
| Pending approvals | 1 |
| Proposal text | 8,448 bytes |
| New memory text | 8,192 bytes |
| Tool result | 16,384 bytes; oversize is a visible tool failure |
| Instructions and conversation | 1 MiB before template rendering |
| Interactive session file | 1 MiB including quoting and audit records |
| Turn time | 120 seconds, starting after model loading and including approvals |

The deadline and cancellation are cooperative between native calls and filesystem operations; neither can preempt an in-progress call.
Loading remains separately cancellable through the existing engine controls.
No background timeout thread, unbounded queue or retry loop is introduced.
The existing one-slot command and 64-slot event channels remain in use.
With tools enabled, a new turn needs room for its bounded intermediate records: start a new session after 110 existing records.
Ordinary chat retains its previous 126-record limit.
`/stats` separates the last generation's measurements from aggregate turn steps, tokens and tool attempts.
It reports the context limit, prefix reuse, cancellation granularity and unavailable memory reporting without inventing measurements.
If native generation fails without returning statistics, the current step count is unavailable and aggregate generated tokens are reported as a lower bound.

A tool read error is `failed`.
A dispatched write error or cancellation is conservatively `unknown`: an atomic rename or directory sync may already have happened.
The file must be inspected before the user chooses another action.
A crash after dispatch intent but before the terminal outcome also remains uncertain.
Resume displays an unknown-outcome warning and never replays a call.
Previously saved outcomes and errors stay available in the Markdown file.
The existing filesystem checks protect cooperating writers and ordinary edits, not a hostile process racing the final path check; they are not an OS sandbox.

## Persistence and secrets

Schema 1 now also accepts literal `## tool` and `## harness` records under the existing quoted-line grammar.
Tool records enter the bounded conversation tail; harness audit records are validated but never become model roles or instructions.
User text cannot forge a role by containing a heading or record terminator.
A current turn checkpoint replaces its own records against the latest exact revision, preserving the original preceding transcript without duplicating the current user record.
The dispatch intent is saved before execution; result/outcome is saved afterward; the final reply replaces the turn checkpoint again.
A failed checkpoint prevents subsequent dispatch and retains in-memory text while blocking new generation until explicit recovery.
There is no exactly-once claim across a process crash.
Old files still read unchanged; older binaries reject the new record headings, so back up sessions before rolling back binaries.

Before serializing new session record bodies and audit text, sefai replaces captured nonempty environment values whose names end with `KEY`, `TOKEN`, `SECRET` or `PASSWORD`, including their JSON-escaped spellings.
Capture is bounded at 128 values and 4,096 bytes per value; unsupported credential capture fails startup.
A memory-write proposal containing a captured credential is refused before approval or file creation.
This covers known environment values, not arbitrary secrets typed into prose, encoded forms, edited files, or credentials outside that set.
Keep credentials out of Markdown; the optional MCP client resolves explicitly named environment references and keeps OAuth credentials in memory.

## Verification

The optional `serde_json` dependency provides strict typed JSON decoding and escaped audit serialization; the standard library has neither, and the existing YAML decoder intentionally accepts a broader language.
It adds no engine runtime and is excluded from the no-default-features package.

Deterministic fake generation drives the same loop used by real inference.
Tests cover ordinary replies, approved results, failures, denial, stale/replayed approvals, repeated writes, cancellation, recovery and every named loop budget.
Concrete filesystem tests cover frozen scope, create-only writes, malformed/oversize memory, checkpoint replacement and role-safe audit loading.
The dedicated `harness_gguf` target requires `SEFAI_TOOL_TEST_MODEL` and fails when it is missing.
It verifies approved reads and answer continuation, an approved create, and a denied create.
The model can still request a repeated write; the test checks that only one dispatch occurred and that the runtime visibly refuses it.
This is a tested permission boundary, not a claim that the model always follows instructions.
Linux CI downloads and verifies the separate 1.06 GB tool fixture; the ordinary 105 MB runner fixture and both native regression targets remain intact.
See [verification.md](verification.md) for results and limitations.
