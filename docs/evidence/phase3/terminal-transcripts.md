# Local terminal transcripts

Captured on an Apple Silicon Mac on 20 October 2026 with the CPU backend and the existing SmolLM2-135M-Instruct-Q4_K_M GGUF fixture.
These are normalized excerpts of actual PTY output and the resulting Markdown records; terminal positioning/color controls are omitted.
The fixture SHA-256 is `2e8040ceae7815abe0dcb3540b9995eaa1fa0d2ca9e797d0a635ae4433c68c2d`.
The model test target separately checks cached/fresh equality; these captures demonstrate interaction and cleanup rather than output quality.

| Scenario | Input and observed state | Exit | Original terminal attributes restored |
| --- | --- | --- | --- |
| First run | Empty-state model guidance; bracketed paste of `你好 é` and a second line; resize from 90×24 to 36×14; quit | 0 | Yes |
| Load failure | `/model /missing-model.gguf`, then `hello`; visible file-open error; quit | 0 | Yes |
| Streaming | `/new pty-chat`, `hello`, `again`, `/stats`; two generated turns and a verified prefix count | 0 | Yes |
| Resume | Restart with `--session pty-chat`; both previous turns visible | 0 | Yes |
| Context full | Paste 600 repetitions of ` word`; overflow rejected before a turn starts | 0 | Yes |
| Cancellation | Start counting with 8192 context/7000 output tokens; Esc after prefill becomes visible; an interrupted record is saved | 0 | Yes |
| Signal | SIGINT while idle; normal exit | 0 | Yes |

The first case set `NO_COLOR=1`; its raw capture contains no foreground/background color-setting sequences.
Raw captures and the temporary qualification workspace are local test artifacts under the system temporary directory, not required runtime assets.
Cell fixtures in `tests/fixtures/tui/` separately pin deterministic narrow-terminal output for first run, streaming, overflow and an overlay.
Those fixtures are test-authored states; they are not presented as real inference captures.

## First run

```text
☾ sefai
v0.1.0 · local inference · /help
Your local conversation

Choose /model PATH or /profile ID, then write a message.
Markdown owns your instructions and saved sessions.
```

Wide terminals also show the generated Candle banner above the empty-state guidance.
The physical terminal's font owns the glyph rendering.

## Two turns and restart

The 512-context profile used batch 64, microbatch 32, output 8 and two CPU threads.
The persisted session and resumed transcript contained:

```text
❯ you
hello

☾ sefai
Hello! I'm so glad you're

❯ you
again

☾ sefai
I'm so glad you're here.
```

The second request reported 30 prompt tokens, 8 generated tokens, `OutputLimit` and 10 reused prompt tokens.
The session header recorded `outcome: completed`, with four explicitly quoted role records.
No second weight allocation or model switch was requested between these turns.

## Context full

```text
prompt (609) plus reserved output (8) exceeds context (512);
history was preserved; use /new or edit the profile context/output limits
```

The full error is available with F2 when the status-width preview is clipped.
No conversation record was appended for this rejected request.
The final implementation restores a rejected question to the composer; a deterministic regression covers that recovery.

## Cancellation

```text
❯ you
Count from one to ten thousand, in order.

☾ sefai

SmolLM2-135M-Instruc · Cpu · ctx 19/8192 · local · 2 records · cancelled
```

Cancellation occurred before the first reply text in this capture.
The saved Markdown header recorded `outcome: interrupted` and retained the user record plus an empty assistant record.
The dedicated native target also checks cancellation after a sampled output token, without relying on PTY timing.

## Limits of this evidence

These PTYs ran locally on macOS; they do not prove Windows Console or Linux terminal lifecycle behavior.
SIGKILL, process abort and power loss cannot run terminal cleanup and were not claimed to do so.
Panic restoration uses the installed Ratatui panic hook and the scope guard; an injected production panic was not added for this capture.
MCP process failure is not applicable because Phase 3 launches no child processes or servers.
The initial capture harness needed terminal-cell matching and simultaneous I/O draining; that harness issue was corrected before the successful scenarios above.
No speedup, peak-RSS improvement or GPU runtime qualification is inferred from these transcripts.
