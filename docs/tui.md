# Terminal conversation

`sefai` opens the permanent terminal interface; `sefai chat` is its explicit equivalent.
The default build embeds GGUF inference, and CPU remains the explicit default unless a Markdown profile selects another available backend/device.
No hosted inference, model conversion, MCP connection or tool execution happens implicitly.

```sh
cargo run --locked -- --workspace /path/to/workspace chat --model /path/to/model.gguf
cargo run --locked -- --workspace /path/to/workspace --profile tiny
cargo run --locked -- --workspace /path/to/workspace chat --plain
```

The workspace is initialized if its directory does not exist.
Existing files are never replaced during initialization.
First run explains model selection; a selected profile must identify GGUF weights and valid runner limits.
The file path may contain spaces: everything following `/model ` is the path, without shell quoting or expansion.
`~`, environment variables and shell commands are not expanded.
A missing or unsupported model reports an error and leaves the conversation available for retry.
Model files are trusted native inputs, not sandboxed data.

## Controls

| Input | Action |
| --- | --- |
| Enter | Send a message or execute a user-entered slash command |
| Shift+Enter | Add a newline when the terminal reports the modifier |
| Bracketed paste | Insert bounded multiline text without submitting it |
| Backspace | Remove the last Unicode grapheme |
| Esc | Close the overlay, otherwise stop generation |
| PageUp / PageDown | Read transcript history without following new tokens |
| End | Follow the latest text |
| Ctrl+K | Show commands |
| F2 | Inspect the full current error in a scrollable overlay |
| Ctrl+C | Stop active work; quit when idle |
| Ctrl+D or `/quit` | Quit |

The editor appends at the end; arbitrary cursor movement and selection are not implemented.
Basic terminals may encode Shift+Enter as Enter; bracketed paste is the portable multiline alternative.
Input is disabled while a request or workspace command is active.
Stopping waits for the current native call to return; it is not a claim of preempting an in-progress decode.
The interface does not periodically redraw when idle.
Streaming redraws coalesce events with a 33 ms input-poll interval.
Idle input polling uses 250 ms without repainting unchanged frames.

## Commands and overlays

| Command | Action |
| --- | --- |
| `/model PATH` | Choose a local weights file for the next request |
| `/models` | List model profile files |
| `/profile ID` | Validate and select a Markdown profile, replacing a direct weights override |
| `/new [ID]` | Start a create-only session without deleting the previous file |
| `/reset` | Start another uniquely named session |
| `/sessions` | List saved sessions |
| `/resume ID` | Load an exact revision and at most 32 recent records |
| `/skills` | List skill files |
| `/skill ID` | Toggle a validated selected skill |
| `/memory` | List memory files |
| `/memory ID` | Inspect the source file and a bounded excerpt |
| `/recall TEXT` | Select literal memory recall; an empty argument disables it |
| `/settings` | Inspect workspace and instruction selections |
| `/mcp` | List connection files; metadata only, no client or process launch |
| `/stats` | Show token counts, timings, stop reason and reused prompt tokens |
| `/help` | Show commands and shortcuts |

Overlays close with Esc or Enter and scroll with arrow keys or PageUp/PageDown.
Lists show at most 128 paths; the underlying workspace still enforces its 1024-entry directory bound.
Edit instructions, profiles, preferences, skills and memory with a text editor.
Explicit memory creation/edit/deletion remains available through `sefai workspace` commands with revision checks.
Invalid edits block the next request visibly while retaining the loaded model and previous conversation; they never replace the user's file.
When context fills, the request fails before generation, preserves history and restores the rejected question to the composer.
Use `/new` to start another session or deliberately adjust the profile's context/output limits.
There is no automatic trimming or hidden summarization.

## Ownership and limits

One named worker owns the engine, active model, context, Markdown reads and writes.
The input/render thread never performs inference or workspace disk I/O.
A one-slot command channel and a 64-slot streaming/event channel provide bounded backpressure.
The composer accepts at most 16 KiB; an oversize paste is rejected without changing its previous contents.
Composed instructions/history and each reply stay within 1 MiB; an interactive session file is also capped at 1 MiB, including quoted Markdown framing.
The display retains at most 128 records and 128 KiB, dropping its oldest content when necessary; the Markdown file retains saved records.
The larger 64 MiB CLI session format remains valid, but an interactive resume rejects files above its smaller bound.
After 126 conversation records, start a new session rather than silently discarding model context.

Weights remain loaded across requests with the same selected path and device.
The engine retains one context and the preceding prompt tokens, comparing actual model-native rendered tokens before reuse.
It clears the divergent suffix and replays the last prompt token for fresh logits; sampled output is not assumed to match the next rendered prompt.
Settings/model changes, session changes, cancellation and errors invalidate the relevant context before another allocation.
Model selection is lazy: the previous model is released before the next model is loaded, not while its replacement is being selected.
There is no second model, serialized KV cache or speculative higher-memory strategy.
Reuse is a correctness mechanism with measured token counts; no speedup or RAM/VRAM reduction is claimed here.

Each started generation saves both the user record and complete or partial assistant text in one atomic replacement.
The session header records `completed` or `interrupted`; native/callback failures count as interrupted.
Exact-byte revision checks preserve concurrent editor changes.
A failed save keeps the turn in memory and blocks subsequent generation until an explicit `/new` or `/resume` resolves the state.
Those commands can discard unsaved in-memory text, so resolve the reported filesystem conflict first when that text is needed.
A process killed during generation can lose the active, not-yet-saved turn; previous saved files retain the Phase 2 atomic-write guarantees.
Native model-loading failures do not append a turn.

## Terminal behavior

Candle colors come from the generated design palette, including its tuned light mode.
`NO_COLOR` disables colors; `SEFAI_THEME=light|dark` selects terminal polarity.
Model text, resumed records, errors and filenames are sanitized before display, including escape sequences split across tokens.
`TERM=dumb` selects the line-oriented mode automatically.
`--plain` uses no raw mode, alternate screen, banners, colors or cursor controls, and works with piped line input.
The existing `prompt`, `run`, `bench`, `devices` and workspace commands remain one-shot commands.
Full-screen chat rejects redirected input/output unless plain mode is selected.

The terminal guard restores bracketed-paste mode, raw mode, cursor visibility and the alternate screen on return or unwinding.
Ratatui's panic hook restores terminal state before reporting a panic.
As with other terminal applications, SIGKILL, process abort and power loss cannot execute cleanup.
No child process is launched in this phase.

## Dependencies and evidence

Ratatui supplies cell layout, wrapping, diff rendering and a test backend; the standard library has no terminal widget system.
Crossterm supplies Windows/macOS/Linux raw mode, key/resize events and bracketed paste.
Unicode segmentation supplies grapheme deletion instead of splitting combining marks and emoji sequences.
All three dependencies belong to the binary's optional `gguf` feature; the minimal diagnostic build does not install the TUI or an engine runtime.
Ratatui's rendered-line-count feature is used for wrapped transcript/composer scrolling and is checked by tests against the locked version.
See [verification.md](verification.md) for local checks and qualification boundaries.
