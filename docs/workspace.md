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
`--raw` rejects workspace instructions rather than dropping them silently.
The engine owns the final token budget and rejects an oversized rendered prompt.

## Schema 1

Every registered file starts with a YAML front matter block delimited by lines containing exactly `---`.
CRLF and LF line endings are accepted.
Only front matter has machine meaning; body prose, headings and code fences never change settings or grant authority.
Every schema rejects unknown fields, duplicate YAML fields and schema versions other than `1`.
YAML aliases and anchors are disabled; parsing is bounded to depth 8, 1,024 nodes and 4,096 events.
Errors identify the path and invalid field or YAML location.

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
| `sessions/ID.md` | `schema: 1`, `id`, `outcome: completed` or `interrupted` | None | Session records; persistence follows in step 2 |
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
Environment values are variable names to resolve when an MCP client exists, never inline secrets.
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
Safe write and persistence qualification are the next two PRs in this stack.
