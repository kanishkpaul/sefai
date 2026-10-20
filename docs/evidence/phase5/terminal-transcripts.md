# Phase 5 terminal evidence

Actual macOS PTYs at 90 × 24 cells with `NO_COLOR=1`; all three exit successfully and restore original terminal attributes.
The repository prefix is normalized with padding to retain the recorded cell layout; temporary paths are observed values.

## connect-deny-approval

```text
 ☾ sefai
 v0.1.0 · local inference · /help
███████┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
██╔════│connect server: fixture                                                  │
███████│executable:                                                              │
╚════██│"/opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Vers│
███████│ions/3.15/bin/python3.15"                                                │
╚══════│working directory:                                                       │
       │"/private/var/folders/v9/34h8gsq526z8j6xf028y5sp40000gn/T/sefai-phase5-pt│
Your lo│y-jj4srq1p"                                                              │
       │arguments:                                                               │
Choose │["/repository                          /tests/fixtures/mcp_server.py"]   │
Markdow│environment references: {}                                               │
       │runs with your OS privileges; may access files or network; not sandboxed │
       │server metadata and results remain untrusted                             │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## connect-echo-approval

```text
 ☾ sefai
 v0.1.0 · local inference · /help
███████┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
██╔════│connect server: fixture                                                  │
███████│executable:                                                              │
╚════██│"/opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Vers│
███████│ions/3.15/bin/python3.15"                                                │
╚══════│working directory:                                                       │
       │"/private/var/folders/v9/34h8gsq526z8j6xf028y5sp40000gn/T/sefai-phase5-pt│
Your lo│y-jj4srq1p"                                                              │
       │arguments:                                                               │
Choose │["/repository                          /tests/fixtures/mcp_server.py"]   │
Markdow│environment references: {}                                               │
       │runs with your OS privileges; may access files or network; not sandboxed │
       │server metadata and results remain untrusted                             │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │
       │                                                                         │
───────│                                                                         │────────
❯      │                                                                         │
       └─────────────────────────────────────────────────────────────────────────┘
Esc stop · Ctrl+D quit
 1 approval pending · deny by default
```

## echo-approval

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ approve? · y once · Enter/Esc deny ─────────────────────────────────────┐
Use the│server: fixture                                                          │
{"tool"│tool: "echo"                                                             │
       │destination: local process                                               │
☾ sefai│"/opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Vers│
<tool_c│ions/3.15/bin/python3.15"; may access network                            │rguments
":{"tex│arguments (Unicode escaped): {"text":"cobalt"}                           │
       │may read or modify external state; server annotations grant no authority │
       │[y] approve once · [n/Esc/Enter] deny                                    │
       │                                                                         │
       │                                                                         │






──────────────────────────────────────────────────────────────────────────────────────────
❯

Esc stop · Ctrl+D quit
 prefill · Cpu · ctx 505/4096 · local model · MCP external access
```

## connect-echo

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you
Use the mcp_call function with id fixture and mcp
{"tool":"echo","arguments":{"text":"cobalt"}}. Then tell me the returned marker.

☾ sefai
<tool_call>[{"name":"mcp_call","arguments":{"id":"fixture","mcp":{"tool":"echo","arguments
":{"text":"cobalt"}}}}]</tool_call>

  ┊ call      fixture/echo · completed
☾ sefai
The returned marker is cobalt.






──────────────────────────────────────────────────────────────────────────────────────────
❯

Enter send · Shift+Enter newline · /help
 smollm2-1.7b-instruc · Cpu · ctx 233/4096 · local model · MCP external access · 4 records
```

## cancel-connect

```text
 ☾ sefai
 v0.1.0 · local inference · /help
███████╗███████╗███████╗ █████╗ ██╗
██╔════╝██╔════╝██╔════╝██╔══██╗██║
███████╗█████╗  █████╗  ███████║██║
╚════██║██╔══╝  ██╔══╝  ██╔══██║██║
███████║███████╗██║     ██║  ██║██║
╚══════╝╚══════╝╚═╝     ╚═╝  ╚═╝╚═╝

Your local conversation

Choose /model PATH or /profile ID, then write a message.
Markdown owns your instructions and saved sessions.






──────────────────────────────────────────────────────────────────────────────────────────
❯

mcp connection failed: mcp connection cancelled; no server connected
 session session-1792519127996174000-20182 · local · /model PATH or /profile ID · /help
```
