# sefai terminal direction

sefai is a terminal LLM runner and harness with one main conversation surface.
The existing Candle identity from the shared design kit is retained.
The TUI is permanent; desktop and tray plans are retired.
The Phase 3 TUI implements this direction; controls and current limits are in [the terminal guide](../docs/tui.md).

## Job and hierarchy

The user chooses a local model, reads and writes a conversation, selects Markdown instructions, and approves tool use when needed.
The transcript takes the main space, the composer stays available, and one status line shows the model, actual engine/device, context pressure, locality and pending approval.
Settings, skills, memory and MCP connections open as focused overlays and return to the conversation.
At narrow widths hide optional detail before shrinking the composer; retain readable model and error state.

Use Ratatui/Crossterm and convert the generated palette from `rust/theme.rs` and `rust/palettes.rs` into terminal styles.
All colors come from that palette; use the terminal's configured font.
Do not introduce a browser or load runtime assets from a CDN.

## Existing visual authority

Follow [system/TUI.md](system/TUI.md) for the banner, metadata, prompt, response labels, tool rows, approvals, color detection and machine output.
Follow [system/DESIGN.md](system/DESIGN.md) and its before-you-add checklist for reuse and state clarity.
The installed `kit.json` selects `candle` using the local master kit's script.
Only this project direction is edited locally; generated shared files come from `../design-kit`.

## States and interaction

- First run: explain that a model must be chosen; show local-file selection and the selected format/backend requirements.
- Loading/prefill: show the model and operation, keep cancel responsive, and never fabricate an estimated memory figure.
- Streaming: append bounded text without stealing scroll position from someone reading history.
- Context full: offer an explicit trim/new-session action; show what history would be excluded.
- Approval: show exact server/tool/arguments and consequence; deny by default, Enter alone does not approve, Esc denies.
- Failed or uncertain tool: distinguish denied, failed, canceled and unknown-after-dispatch outcomes.
- Bad Markdown edit: retain the last usable state and show the file/field error; never overwrite the user's edit silently.
- Model/engine unavailable: show the requirement and an explicit alternative; no hidden fallback.
- Idle: no periodic full redraw and no attention-seeking animation.

Memory entries show the source file and can be inspected, edited and forgotten.
Detailed cache, throughput and allocation information belongs in `/stats`; the ordinary status bar remains compact.
No faces or decoration in system errors or permission decisions.
Strip terminal control sequences from model and tool text while preserving ordinary Unicode.
Test wide characters, combining marks, long lines, small terminals, keyboard-only use, `NO_COLOR`, light/dark modes and terminal cleanup after interruption.
Plain output and future JSON modes contain no banner, color or animation.

## Prototype commands

The `prompt` diagnostic emits plain role-labeled text; `run` streams sanitized Unicode and reports locality/device/context on stderr.
`bench` emits plain timing records and `devices` lists ggml indices; one-shot commands have no banner.
The default command opens the interactive transcript/composer/status layout; focused overlays expose selection and inspection controls.
One-shot commands remain plain, and `chat --plain` provides a line-oriented conversation without screen controls.
This brief retains the kit identity without inventing a new visual world or additional UI surfaces.
