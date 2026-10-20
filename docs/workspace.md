# Markdown workspace

`sefai --workspace /path/to/workspace prompt hello` composes instructions without loading a model.
The same selections apply to `run` and `bench`.
`sefai --workspace /path/to/workspace workspace check` validates all registered Markdown files without loading models or connecting to MCP servers.
The workspace is local editable data, never executable policy.

## Location and precedence

An explicit `--workspace` selects the root.
Otherwise use `%APPDATA%/sefai` on Windows, `~/Library/Application Support/sefai` on macOS, or `$XDG_CONFIG_HOME/sefai` (`~/.config/sefai` when unset) on Linux.
An absent default workspace preserves the standalone runner; an explicitly selected missing directory fails.
No default directory is created by reading a prompt.

Runner settings resolve as explicit CLI flags, selected model profile, workspace config, then built-in runner defaults.
`--profile ID` overrides the config's `model` selection.
`--model PATH` overrides the selected profile's weights path, retaining its runner settings.
A relative weights path resolves against the workspace; an absolute path may point outside it.
The supported format names are `gguf`, `onnx`, `gptq`, `awq` and `mlx`; this runner can execute only `gguf` and rejects other selections without conversion or fallback.
Profiles are metadata; weights are not read by `workspace check`.

Prompt order is optional `--global-workspace` system and preferences, workspace system and preferences, selected `--skill ID` entries in CLI order, recalled memory in filename order, then the user message.
Only prose from the global workspace is inherited; its model settings are not inherited.
Up to 16 skills are selected explicitly and read on demand.
Skill resources are bounded workspace-relative references, validated but neither executed nor added to the prompt automatically.
`--recall TEXT` performs case-sensitive literal matching over memory bodies and includes at most eight matching files.
No embeddings or implicit memory extraction runs.
`--raw` rejects nonempty workspace instructions and resumed history before loading weights rather than dropping them silently.
The engine owns the final token budget and rejects an oversized rendered prompt.

## Schema 1

Every registered file starts with a YAML front matter block delimited by lines containing exactly `---`.
CRLF and LF line endings are accepted.
Only front matter has machine meaning; body prose, headings and code fences never change settings or grant authority.
Every schema rejects unknown fields, duplicate YAML fields and schema versions other than `1`.
Unsupported YAML tags, merge keys, aliases and anchors are rejected; parsing is bounded to depth 8, 1,024 nodes and 4,096 events.
Errors identify the path and invalid field or YAML location.
Fields must have their declared YAML type; quote numeric-only string IDs.

IDs are portable lowercase ASCII names of 1–64 bytes, starting with a letter or digit and containing letters, digits or hyphens.
Windows device names are rejected.
The `id` must match its filename stem, or the containing directory for `skills/ID/SKILL.md`; IDs therefore cannot be aliased across multiple files of one kind.
Different kinds have separate namespaces.

| File | Required front matter | Optional front matter | Body |
| --- | --- | --- | --- |
| `config.md` | `schema: 1` | `model: ID`, `runner` | Notes |
| `system.md`, `preferences.md` | `schema: 1` | None | Instructions |
| `models/ID.md` | `schema: 1`, `id`, `path`, `format` | `runner` | Notes |
| `skills/ID/SKILL.md` | `schema: 1`, `id` | `resources: [relative-path]` | Selected instructions |
| `memory/ID.md` | `schema: 1`, `id` | None | Recallable text |
| `sessions/ID.md` | `schema: 1`, `id`, `outcome: completed` or `interrupted` | None | Quoted session records |
| `mcp/ID.md` | `schema: 1`, `id`, `transport` | None | Notes; never approval |

`runner` accepts optional `backend` (`cpu`, `metal`, `cuda`, `vulkan`), `device` (nonnegative ggml index), `context` (1–131072 tokens), `batch`/`microbatch` (1–4096 tokens), `output` (0–131072 tokens), and `threads` (1–256).
The runner additionally validates device compatibility, batch/context relationships and reserved output before inference.
GPU selection still requires an explicit device; no automatic CPU fallback occurs.

A minimal config is:

```markdown
---
schema: 1
---
```

A model profile can select weights and default controls:

```markdown
---
schema: 1
id: small
path: /models/small.gguf
format: gguf
runner:
  backend: cpu
  context: 4096
  threads: 4
---
Local model notes.
```

MCP `transport` is either `{kind: stdio, command: PROGRAM, args: [], env: {SERVER_VARIABLE: LOCAL_VARIABLE}}` or `{kind: http, url: https://host/mcp, token_env: LOCAL_VARIABLE}`.
Environment values are variable names resolved only by an explicitly approved optional MCP connection, never inline secrets.
HTTP definitions can select `oauth` instead of `token_env`; see [mcp.md](mcp.md) for the pre-registered client schema and explicit loopback exception.
HTTP URLs cannot contain credentials, query strings or fragments.
At most 64 arguments/environment references and 4,096 bytes per argument/URL/command are allowed.
Plaintext credentials must not be placed in notes, commands or arguments either; syntactic validation cannot identify every secret-looking string.
These definitions do not launch a process or make a connection in Phase 2.

## Limits and failure behavior

Ordinary files are at most 1 MiB, front matter 16 KiB, and each registered directory at most 1,024 entries.
Composed text is at most 1 MiB before native tokenization.
Resources and registered files must resolve inside the workspace; escaped paths fail.
An invalid selected file stops the operation without modifying it.
Unselected skills are not read during prompt composition; `workspace check` deliberately inspects them all.
`cache/` is disposable and never authoritative.
No hidden database or canonical binary customization store exists.

Schema 1 is the initial format, so no migration is applied automatically.
Unsupported versions are rejected without rewriting; rollback means restoring the user's backed-up Markdown files and the matching application version.
Schema fixtures live in `tests/fixtures/workspace`; model-free restart and filesystem qualification lives in `crates/sefai/tests/workspace.rs`.
The existing three-OS CI matrix runs it automatically; local evidence is macOS and remote qualification remains pending.
The acceptance matrix is in [phase2-closeout.md](phase2-closeout.md).

## Editing safely

`workspace init` creates missing starter files and directories and never replaces existing files.
For normal editing, use a text editor directly and run `workspace check` afterward.
For writes through sefai, `workspace put system.md --from candidate.md` creates a missing file.
Updating it requires `--expected snapshot.md`, where the snapshot is a separate unchanged byte-for-byte copy of the old file.
The snapshot is read again immediately before replacement; keep it unchanged throughout the operation.
Use an independent copy, not a hard link to a file being edited.
A mismatched, missing or unexpected current file is a conflict, never a request to overwrite it.
The target itself cannot serve as its expected snapshot.
A malformed candidate is rejected before replacing the last good file.
An invalid manual edit is reported and left untouched; the CLI does not invent or silently restore lost content.
Keep a backup when editing manually; a future long-running TUI must retain its loaded valid state when reload fails.

`workspace remember ID TEXT` creates one memory, `workspace edit ID TEXT --expected snapshot.md` updates it, and `workspace forget ID --expected snapshot.md` deletes it.
No model or skill can authorize these actions.
The CLI user explicitly requests each write.

Every writer takes a nonblocking OS lock on `.sefai.lock`; contention returns a visible busy error without sleeping or retrying.
The lock file stays in place and its OS lock is released on handle close or process exit.
Candidates are bounded, validated, flushed and synced in a new same-directory temporary file before rename.
A rejected or failed candidate leaves the target unchanged and removes its temporary file during normal error unwinding.
A killed process can leave an unused `.sefai-*.tmp` file; it is not registered Markdown and is never loaded or promoted automatically.
On Unix the containing directory is synced after replacement; a directory-sync error occurs after replacement and may mean the new file is already visible.
No crash/power-loss durability guarantee is made for every filesystem.
Cancellation before rename preserves the old target; interruption during or after rename requires inspecting the target before retrying.
Workspace commands use normal process cancellation and do not automatically retry side effects.
Windows rename/share errors are returned; there is no delete-then-rename fallback that could lose the old file.
Writes reject symlinks in any relative path component.
Read paths must resolve within the workspace.
These checks and advisory locks protect cooperating writers and ordinary external edits; they are not an OS sandbox or an atomic compare-and-swap against a hostile process swapping paths during the final check/rename interval.

## Session grammar and tail

`workspace append ID --role user TEXT` creates a session; subsequent appends require `--expected snapshot.md`.
The CLI append role is explicitly `user` or `assistant` and never inferred from message text.
Phase 4 also persists `tool` results and `harness` audit records using the same quoted grammar; audit records are excluded from model conversation tails.
New files carry `outcome: completed`; import an explicitly edited `outcome: interrupted` file with `workspace put` when recording an interrupted transcript.
The TUI records turns and cancellation outcomes automatically; the [harness](harness.md) additionally checkpoints dispatch intent and tool outcomes.

After schema-1 front matter, each record has a heading, quoted body lines and an explicit terminator:

```markdown
## user
> hello
> ## assistant is literal user text here
<!-- end -->
```

Every logical content line, including an empty one, starts with `> `.
Headings, fences, front matter and terminators inside quoted content stay literal text.
A record without its newline-terminated `<!-- end -->` is incomplete and rejected.
Nonquoted prose outside records is rejected; blank separator lines are allowed.
CRLF is normalized to LF; bare carriage returns and nul bytes are not accepted by record encoding.
An empty string is encoded as one empty quoted line; trailing newlines are preserved as empty quoted lines.

Sessions may be at most 64 MiB on disk, with at most 65,536 records and 1 MiB of UTF-8 text per record.
`workspace tail ID --messages N` streams and validates the grammar while retaining at most N records (1–128) and 1 MiB of decoded content.
Older records are discarded from the in-memory tail rather than loaded as one transcript string.
`--session ID` adds the last 32 records under the same byte limit before the new user message in `prompt` and `run`.
Prompt composition can still reject the combined text budget or the engine's rendered token budget.
Tail output sanitizes terminal controls; source Markdown remains unchanged.

## Implementation references

[Serde](https://docs.rs/serde/latest/serde/) provides typed decoding and unknown-field rejection.
[Deserialize-only serde-saphyr](https://docs.rs/serde-saphyr/2.0.0/serde_saphyr/) supplies YAML budgets without filesystem includes or a serializer.
[Standard-library file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock) and [rename](https://doc.rust-lang.org/std/fs/fn.rename.html) provide the OS primitives; sefai adds bounded staging and expected-revision checks around them.
