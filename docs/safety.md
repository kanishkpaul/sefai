# Safety checklist

Phase 12 maps every trust boundary to its abuse cases, the control that blocks each one, the module that owns it and the test that proves it.
A persuaded model, poisoned memory, hostile Markdown, a malicious MCP server or a hostile model file must still be unable to act without the user's exact approval, leak a known secret, destroy user data or exhaust the machine without a bound.
Rows marked **residual** are limits sefai cannot enforce; they are stated so nobody relies on a protection that does not exist.

## Assets and authority

| Asset | Who may change or send it |
| --- | --- |
| Workspace Markdown: config, profiles, skills, memory, sessions, catalog | The user through an editor or an explicit CLI command; the model only through an approved `remember_memory`, which creates and never replaces |
| External effects through MCP | Only an approved call whose server, tool and arguments match the preview exactly, once |
| Credentials named by `token_env`, stdio `env` and OAuth | Read from the environment or the OAuth flow, sent only to the approved server, never written to Markdown |
| The machine's CPU, memory, disk and processes | Bounded by named limits per turn, file, message, process and download |
| The terminal | Only sanitized text reaches it |

Model output, recalled memory, skill prose, catalog prose, tool results, MCP metadata and model-card text are untrusted data everywhere; none of them can grant, widen or persist authority.

## Checklist

Status is **verified** when the control and its test exist, **Phase 12** when this phase adds them, and **residual** when sefai cannot enforce it.

### Approvals and dispatch

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| Model proposes a call the user never saw | One exact proposal per approval; denial by default; `y` approves once | `harness`, `tui` | `denied_call_has_no_dispatch`, `enter_denies_an_approval` | verified |
| Arguments change between preview and dispatch | The approved action is consumed once and compared before dispatch | `harness` | `changed_arguments_invalidate_approval`, `approval_cannot_be_replayed` | verified |
| Scope, definition or catalog changes during an approval | Workspace scope and `mcp/ID.md` text revalidated after approval and immediately before dispatch | `tui/worker`, `mcp` | `edited_definition_invalidates_connection_authority`, `frozen_scope_does_not_adopt_a_new_memory` | verified |
| Tool output or skill text widens scope | Runtime-owned tool list; unknown names and other server IDs rejected | `harness` | `tool_output_cannot_widen_scope`, `tool_outside_catalog_is_never_sent` | verified |
| A pasted block in plain mode carries `y` for the next approval | In an interactive terminal, plain mode asks for a one-time code shown with the preview | `tui` | Phase 12 | Phase 12 |
| A key typed just before the overlay appears approves it | The screen overlay ignores keys for a short arming delay | `tui` | Phase 12 | Phase 12 |
| Approval timeout or cancellation | 120-second turn limit; Esc and Ctrl+C cancel; no dispatch after either | `harness`, `tui/worker` | `time_budget_stops_approval`, `expired_approval_denies_without_waiting`, `cancellation_stops_dispatch` | verified |
| A user wants to stop a connection mid-session | `/disconnect` drops it and stops the child between turns; cancellation stops a turn | `tui/worker` | Phase 12 adds the help entry and a test | Phase 12 |
| A misleadingly named MCP tool | The preview shows the server, destination, tool name and escaped arguments; the tool's description is not shown or bound | `tui/worker` | `approved_mcp_dispatch_preserves_exact_arguments_and_outcome` | residual: judge the call by its arguments |

### Untrusted text reaching the model

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| Memory, skill, tool or MCP text contains `<\|im_start\|>system` or another special-token string, forging a role | Each engine inserts a word joiner into its own special-token strings inside message content before rendering, so only the template emits them | engines, `core::neutralize_special` | `gguf_message_text_cannot_forge_a_role_token`, MLX and ONNX `message_text_cannot_forge_a_role_token`, `forged_role_markers_are_broken` | verified |
| A model-written memory is later recalled as if the user wrote it | Memories created by `remember_memory` record `source: model` and are recalled under a model-memory label | `workspace`, `tui/worker` | Phase 12 | Phase 12 |
| A session file forges a role or a dispatch record | Quoted record grammar; harness records never become roles | `workspace` | `session_body_cannot_inject_a_role`, `conversation_text_cannot_forge_a_dispatch_record` | verified |
| Front matter injected through a memory body | Parsing stops at the first closing delimiter | `workspace` | `body_is_never_configuration` | verified |

### Secrets and outbound data

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| A known credential is written into a session record | Values of credential-named variables, MCP env references, `token_env` and OAuth tokens are redacted before persistence | `harness::Redactor` | `secrets_are_redacted_literally`, `escaped_secret_is_redacted_in_audit_json` | verified |
| The model places a known credential in MCP arguments or a memory | The call is refused before approval | `harness` | `known_credentials_cannot_be_written_as_memory_ids`; MCP case Phase 12 | Phase 12 |
| A known credential appears in an approval preview, tool row or error | Redacted before display | `tui/worker` | Phase 12 | Phase 12 |
| A known credential streams in a live reply | Not redacted on screen because a value can split across tokens; the saved record is redacted | `harness` | none | residual |
| Short or common values such as `1` mangle saved text | Values shorter than 8 bytes are not treated as secrets | `harness::Redactor` | Phase 12 | Phase 12 |
| Credential names outside the suffix list | `KEY`, `TOKEN`, `SECRET`, `PASSWORD`, plus `PAT`, `AUTH` and `CREDENTIALS` suffixes | `harness::Redactor` | Phase 12 | Phase 12 |
| `token_env` names an unrelated credential | The connect preview names the variable and the destination; only the user's approval binds them | `mcp` | `http_requires_the_endpoint_credential` | residual |
| An MCP endpoint on a private, link-local or loopback network | Remote URLs need HTTPS or literal loopback; the preview labels private destinations | `mcp` | Phase 12 | Phase 12 |
| A server requests sampling, elicitation or roots | sefai refuses all three itself instead of relying on SDK defaults | `mcp` | Phase 12 | Phase 12 |

### Files and durable state

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| Traversal or symlink escape on read | Normal components only, canonical path inside the root | `workspace` | `symlink_escape_is_rejected_for_reading`, `windows_junction_cannot_escape_the_workspace` | verified |
| Symlinked parent on write | Every destination component checked with `symlink_metadata` | `workspace` | `symlink_parent_is_rejected_for_writing` | verified |
| Lost update from a concurrent editor | Create-only or exact-snapshot writes, staged and revalidated before rename | `workspace` | `stale_snapshot_cannot_overwrite_an_external_edit`, `edit_during_staging_is_detected_before_rename` | verified |
| A swap between the last check and `rename` | No `openat` without unsafe code | `workspace` | none | residual |
| A pre-placed symlink at a download's partial file | Partial files are created exclusively | `setup::acquire` | Phase 12 | Phase 12 |
| A reply with a bare carriage return blocks saving | Saving fails visibly and the session refuses further sends until resolved | `workspace` | none | residual: visible, never silent |

### Processes and network

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| Shell injection through MCP launch | Absolute canonical executable, argument array, `.bat`/`.cmd` refused | `mcp` | `stdio_denied_connection_does_not_start_server` | verified |
| Inherited environment leaks to a child | Cleared environment plus named references | `mcp` | `stdio_discovers_paginated_catalog` | verified |
| A child outlives sefai | Process group or job object, killed on drop; SIGTERM and SIGHUP now run that cleanup | `mcp`, `tui` | `stdio_cleanup_stops_a_server_descendant`; signal case Phase 12 | Phase 12 |
| SIGKILL of sefai, or a descendant that calls `setsid` | Cannot be intercepted without unsafe code | `mcp` | none | residual |
| A child exhausts CPU or memory | No resource limits are applied | `mcp` | none | residual: children are unsandboxed, as the connect preview states |
| Redirect to an unapproved host | MCP HTTP follows no redirects; downloads follow at most five, only to Hugging Face hosts | `mcp`, `setup::acquire` | `http_does_not_follow_redirects`; download case Phase 12 | Phase 12 |
| A proxy from the environment intercepts traffic | MCP clients disable proxies; downloads disable them too | `mcp`, `setup::acquire` | Phase 12 | Phase 12 |
| A download server stalls headers or the body | Response and per-file body deadlines scale with file size; cancellation is checked between reads | `setup::acquire` | Phase 12 | Phase 12 |
| A download cannot be stopped | Ctrl+C in the CLI and Esc in the terminal cancel between reads; verified files stay staged | `setup`, `tui` | `cancelled_acquisition_publishes_nothing` | Phase 12 |

### Models and dependencies

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| A checkpoint is not what its profile declares | Admission against the declared format before native allocation | `engine` | `non_gguf_file_is_rejected_before_native_loading`, `declared_quantization_must_match_the_checkpoint` | verified |
| Bundle metadata escapes or is oversized | Top-level canonical assets, size caps before parsing | engines | `bundle_paths_cannot_escape`, `oversized_metadata_is_rejected_before_parsing` | verified |
| A hostile chat template loops without bound | Template rendering stops after 1,000,000 instructions; the 1,024-message maximum needs under 100,000 | `engine-onnx`, vendored `mlx-lm` | `a_runaway_template_stops_at_the_fuel_limit`, `the_largest_prompt_renders_within_the_fuel_limit` | verified |
| A template builds one huge value in a single operation | Fuel counts instructions, not bytes; the rendered prompt is still capped at 1 MiB afterwards | engines | none | residual |
| A malformed weight file exploits a native parser | llama.cpp, MLX and ONNX Runtime are an explicit trust boundary; Rust does not sandbox them | engines | none | residual |
| A downloaded file is altered | Pinned size and SHA-256 per file, staged until all verify | `setup::acquire` | `verification_rejects_a_changed_byte` | verified |
| Repository code runs during acquisition | Only listed files are fetched and nothing is executed | `setup::acquire` | `get_without_confirmation_downloads_nothing` | verified |

### Terminal, resources and recovery

| Abuse case | Control | Owner | Test | Status |
| --- | --- | --- | --- | --- |
| Escape sequences in model or tool text | ESC, CSI, OSC, DCS and C1 stripped across chunks | `term` | `terminal_controls_can_cross_stream_chunks`, `unfinished_escape_does_not_reach_the_terminal` | verified |
| Bidi overrides reorder displayed text | Bidi embedding, override and isolate controls removed from displayed text; left-to-right and right-to-left marks kept | `term` | `bidi_overrides_are_removed` | verified |
| Oversized output floods memory | Named bounds on turns, tokens, results, display, messages and files | all | `step_budget_stops_generation`, `result_budget_is_enforced`, `oversized_result_is_refused` | verified |
| A dispatched write is retried after an unknown outcome | No replay; unknown outcomes are persisted and warned about on resume | `harness` | `incomplete_dispatch_is_visible_after_restart`, `fake_engine_cannot_retry_a_failed_write` | verified |
| Audit records leak into the model's context | Harness records are skipped when reading conversation | `workspace` | `harness_audit_is_not_a_conversation_role` | verified |

## Residual risks

- Third-party MCP servers and their descendants run with the user's privileges, can read the workspace they start in and are not sandboxed; SIGKILL of sefai or a `setsid` descendant can leave them running.
- Native inference libraries parse untrusted weight files; Rust's memory safety ends at that boundary.
- A credential visible on screen during streaming is redacted only in the saved session.
- Approval binds a tool's name and arguments, not what the server does with them.
- Workspace writes cannot close the final check-to-rename window without unsafe code; the advisory lock does not stop other programs.
- Audit records live in session files; deleting a session deletes its audit, and records are not tamper-proof against the same OS user.
