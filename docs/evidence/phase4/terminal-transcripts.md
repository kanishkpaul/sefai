# Phase 4 terminal evidence

Five real macOS pseudo-terminal cases used the pinned SmolLM2-1.7B GGUF on CPU, context 2,048, output 256 and two threads.
Each exited with code 0 and restored the original terminal attributes.
The test driver sent actual key and cancellation events; no production fake-model mode was added.
Temporary workspace paths below are normalized to `<workspace>`.

## approve-read

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
Read me│server: local-memory                                                     │
       │tool: read_memory                                                        │
☾ sefai│workspace:                                                               │
<tool_c│"<workspace>"                                                            │
       │                                                                         │
       │arguments (Unicode escaped): {"id":"note","text":null}                   │
       │reads a memory file into model context; no network                       │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## approve-write

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
Call re│server: local-memory                                                     │result.
       │tool: remember_memory                                                    │
☾ sefai│workspace:                                                               │
<tool_c│"<workspace>"                                                            │answers
concise│                                                                         │
       │arguments (Unicode escaped): {"id":"rule","text":"Keep answers concise."}│
       │creates a new memory file; no network                                    │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## enter-deny

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
Call re│server: local-memory                                                     │sult.
       │tool: remember_memory                                                    │
☾ sefai│workspace:                                                               │
<tool_c│"<workspace>"                                                            │ "Do not
save"}}│                                                                         │
       │arguments (Unicode escaped): {"id":"denied-rule","text":"Do not save"}   │
       │creates a new memory file; no network                                    │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## escape-deny

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
Call re│server: local-memory                                                     │sult.
       │tool: remember_memory                                                    │
☾ sefai│workspace:                                                               │
<tool_c│"<workspace>"                                                            │ "Do not
save"}}│                                                                         │
       │arguments (Unicode escaped): {"id":"escape-rule","text":"Do not save"}   │
       │creates a new memory file; no network                                    │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## cancel-approval

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
Call re│server: local-memory                                                     │sult.
       │tool: remember_memory                                                    │
☾ sefai│workspace:                                                               │
<tool_c│"<workspace>"                                                            │ "Do not
save"}}│                                                                         │
       │arguments (Unicode escaped): {"id":"cancel-rule","text":"Do not save"}   │
       │creates a new memory file; no network                                    │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## Outcomes and limits

The approved read returned the actual marker `cobalt` and completed answer generation.
The approved create wrote only `memory/rule.md`; a repeated model proposal was refused rather than dispatched again.
Enter and Esc denied their pending writes; neither target memory was created.
Ctrl+C canceled a pending approval, dismissed its gate and created no target memory.
These are CPU/macOS terminal results; Windows/Linux terminal lifecycle and physical GPU execution remain unverified in this phase.
The cell snapshots use `NO_COLOR`; the unit cell backend separately checks the warning border against the generated palette.
