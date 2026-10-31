# Testing and release evidence

Phase 13 freezes the feature set and traces every feature, module, command, file schema and integration to the tests that exercise it, with the result and the hardware it ran on.
No release-blocking correctness, data-loss, secret-leak or unauthorized-action issue is open; the [known limits](#known-limits) block only the release claims they name.
The suite has 347 tests: model- and network-independent unit tests in every crate, plus dedicated integration targets that need a pinned fixture or a fixture MCP server and fail when it is missing.

## How to run it

| Layer | Command | Needs |
| --- | --- | --- |
| Required checks | `cargo fmt --all --check`; both Clippy runs and `cargo test --locked --workspace` from [AGENTS.md](../AGENTS.md) | Nothing |
| Attribution | `python3 -m unittest discover -s tests` | Python |
| Dependencies | `cargo deny check` with [deny.toml](../deny.toml) | `cargo-deny`; CI runs it on every pull request and weekly |
| Engine model tests | `--test gguf`, `--test mlx`, `--test onnx` and the GGUF `--lib` model tests, each with its `SEFAI_*_TEST_MODEL` | The fixtures in [runner.md](runner.md), [mlx.md](mlx.md), [onnx.md](onnx.md) |
| Lifecycle contract | `lifecycle_{gguf,mlx,gptq,awq,onnx,switch}` | `SEFAI_LIFECYCLE_*` fixtures; see [runner.md](runner.md#lifecycle-contract) |
| Harness with tools | `harness_{gguf,mlx,gptq,awq,onnx}` | The tool-capable fixtures in [harness.md](harness.md) |
| MCP | `mcp_transport` with `SEFAI_MCP_TEST_PYTHON` | Python for [the fixture server](../tests/fixtures/mcp_server.py) |
| Recovery | `recovery` with `SEFAI_LIFECYCLE_GGUF` | GGUF fixture |
| Setup | `setup`, with and without `--features download` | Nothing; no network |
| Footprint | [scripts/measure_footprint.py](../scripts/measure_footprint.py) and the [footprint workflow](../.github/workflows/footprint.yml) | Release builds |

## Coverage

Local means the Apple M5 MacBook Air; CI means GitHub-hosted runners, which run CPU only.

### Commands

| Command | Tests | Where |
| --- | --- | --- |
| `run`, `bench`, `devices` | engine model tests; `lifecycle_*` `run_reports_the_selected_engine_device`, `bench_reports_statistics_and_engine_memory`; runner unit tests | Local all engines; CI GGUF and ONNX CPU |
| `chat` and `chat --plain` | `tui` unit and target tests, `lifecycle_*`, `harness_*`, `recovery` | Local; CI GGUF and ONNX lifecycle |
| `prompt`, `workspace check/init/put/remember/edit/forget/append/tail` | `workspace` unit tests and target | Local and CI |
| `mcp inspect/read/prompt` | `mcp_transport` | Local and CI |
| `setup hardware/check/recommend/catalog/get/import` | `setup` target, setup unit tests, real downloads recorded in [verification.md](verification.md#phase-11--hardware-aware-model-setup) | Local; CI `setup` target |

### Modules and file schemas

| Part | Tests |
| --- | --- |
| `core` prompt, settings, special-token neutralization | `core` unit tests |
| `engine-llama` | 32 library and 13 `gguf` model tests, including hostile GGUF files |
| `engine-mlx` | 16 unit and 13 `mlx` model tests, including repack, admission, forged roles and hostile safetensors |
| `engine-onnx` | 21 unit and 13 `onnx` model tests, including graph signatures, template fuel, forged roles and hostile graphs |
| Binary engine dispatch and admission | `engine` and `runner` unit tests, `lifecycle_switch` |
| Workspace schema 1: config, profiles, skills, memory, sessions, MCP definitions, catalog | `workspace` unit tests and target, `setup` target |
| Harness: approvals, budgets, redaction, recovery | `harness` unit tests, `harness_*` targets |
| Terminal: sanitization, approvals, rendering | `term` and `tui` unit tests, `tui` target |
| MCP: stdio, Streamable HTTP, OAuth | `mcp` unit tests, `mcp_transport` |
| Setup: hardware, estimates, catalog, acquisition | setup unit tests, `setup` target |

### Integrations by format and hardware

| Integration | Qualified on | Evidence |
| --- | --- | --- |
| GGUF CPU | Apple M5; CI Linux x86_64 and arm64, macOS arm64 and Intel, Windows | engine tests, `lifecycle_gguf`, footprint workflow |
| GGUF Metal | Apple M5 | engine tests with `SEFAI_TEST_BACKEND=metal`, Phase 10 benches |
| MLX, GPTQ, AWQ on MLX | Apple M5 | `mlx` tests, `lifecycle_{mlx,gptq,awq}`, `harness_{mlx,gptq,awq}` |
| ONNX CPU | Apple M5; CI Linux, macOS, Windows | `onnx` tests, `lifecycle_onnx` |
| ONNX WebGPU | Apple M5 | Phase 7 evidence in [onnx.md](onnx.md) |
| GGUF CUDA and Vulkan | compiled only | backend-builds workflow |
| MCP stdio, HTTP, OAuth | local fixture servers on macOS; CI on three systems | `mcp_transport` |

## Red team

Each case attacks a real trust boundary; success means runtime controls hold even when the model or a file is hostile.
The full control list is in the [safety checklist](safety.md).

| Attack | Result | Test or evidence |
| --- | --- | --- |
| Memory, skill, tool or MCP text forges `<\|im_start\|>system` | Blocked on GGUF, MLX and ONNX | `*message_text_cannot_forge_a_role_token` |
| Model persuades the user's paste or type-ahead to approve | Blocked by the approval code and arming delay | `approval_codes_are_four_digits`, `y_typed_as_the_preview_appears_is_ignored` |
| Tool or schema changes after approval, forged tool identity, replay, hidden retry | Blocked | `changed_arguments_invalidate_approval`, `tool_outside_catalog_is_never_sent`, `approval_cannot_be_replayed`, `fake_engine_cannot_retry_a_failed_write` |
| Secret exfiltration through MCP arguments or memory | Refused before approval | `mcp_arguments_carrying_a_known_credential_are_refused`, `known_credentials_cannot_be_written_as_memory_ids` |
| Server asks for roots, sampling or elicitation | Refused | `server_requests_for_roots_sampling_and_elicitation_are_refused` |
| Workspace traversal, symlink and junction escapes, role spoofing, concurrent edits | Blocked | `symlink_escape_is_rejected_for_reading`, `windows_junction_cannot_escape_the_workspace`, `session_body_cannot_inject_a_role`, `stale_snapshot_cannot_overwrite_an_external_edit` |
| Terminal escapes, OSC 52 clipboard writes, bidi overrides, oversized output | Stripped or bounded | `terminal_controls_can_cross_stream_chunks`, `bidi_overrides_are_removed`, `result_budget_is_enforced` |
| MCP child floods, oversized or malformed messages, redirects | Bounded and refused | `exhausted_message_budget_stops_before_the_next_sdk_dispatch`, `stdio_rejects_oversized_messages`, `http_does_not_follow_redirects` |
| Hostile weight files: truncated, absurd counts or headers, flipped bytes | Every case rejected with an error; no crash | `gguf_hostile_files_fail_cleanly`, `hostile_weights_fail_cleanly`, `hostile_graphs_fail_cleanly` |
| Runaway chat template | Stops at the fuel limit | `a_runaway_template_stops_at_the_fuel_limit` |
| Download redirect to another host, planted links, stalled server | Refused or bounded | `redirects_stay_on_hugging_face_hosts_over_https`, `a_linked_staging_directory_is_refused`, `body_deadlines_grow_with_file_size` |
| sefai killed mid-reply | Workspace validates and the session resumes | `recovery` `a_killed_reply_leaves_a_valid_resumable_workspace` |

Generic jailbreaks of the model's opinions are out of scope; the runtime does not depend on the model refusing anything.

## Long sessions and switching

On the Apple M5 with the release build of every engine:

- Sixty GGUF turns, starting a new session every ten, held the physical footprint between 199 and 220 MiB with no growth.
- Alternating GGUF and MLX twenty-four times held MLX at 296–321 MiB and let GGUF creep from 220 to 240 MiB, about 1.6 MiB per reload.
- Cycling GGUF, MLX and ONNX left about 560 MiB in the process after the ONNX Optimum export unloaded, rising slowly to 821 MiB for GGUF by the eighth cycle while ONNX itself held at 671 MiB.
  Disabling ONNX Runtime's CPU arena and memory pattern did not release it and made ONNX creep, so the change was not kept; the memory is freed per-token cache allocations that the system allocator keeps, which only restarting sefai returns.

Raw samples for the sixty turns and the three-engine cycle are in [evidence/phase13/plateau.json](evidence/phase13/plateau.json).

## Dependencies and licenses

`cargo deny check` passes for every feature: no unpatched advisory, every license on the permissive allow list, and only crates.io plus the pinned `oxiglade/mlx-rs` git revision as sources.
It found `tokio` 1.54.0 affected by RUSTSEC-2026-0384 and yanked; Phase 13 updated it to 1.54.1.
`paste` (RUSTSEC-2024-0436) is unmaintained, not vulnerable, and runs only at build time inside `mlx-rs` and `tokenizers`; it is ignored with that reason.
All 404 third-party crates are permissively licensed; the Mozilla root-certificate bundles use CDLA-Permissive-2.0, and `mach-sys` declares `GPL-3.0 OR Apache-2.0`, used under Apache-2.0.
llama.cpp, MLX and ONNX Runtime are native dependencies with their own MIT notices, which Phase 14 ships with each artifact.

## Known limits

These block only the claims they name:

- CUDA, Vulkan, ROCm, GPTQ and AWQ CUDA kernels and ONNX GPU providers other than WebGPU on Apple Silicon were never executed; no release may claim them.
- Physical Windows and Linux terminals were not used; CI exercises the plain interface and the lifecycle on those systems.
- After an ONNX Optimum export unloads, about half a gigabyte stays in the process until sefai exits; share-buffer GenAI exports avoid per-token allocations.
- The ONNX external-data model test needs its own large fixture, which was not re-run after Phase 12.
- The [safety checklist's residual risks](safety.md#residual-risks) stand.
