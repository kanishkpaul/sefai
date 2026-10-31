# MCP connections

The optional MCP client connects one explicitly selected server to the terminal harness.
The default GGUF package has no MCP runtime or HTTP client dependency.
Local inference stays local; a connected MCP process or endpoint can access external state and network services.

## Build and connect

| Feature | Adds |
| --- | --- |
| `mcp` | Official `rmcp` client, Tokio current-thread runtime and supervised stdio processes |
| `mcp-http` | Streamable HTTP, HTTPS, bounded JSON/SSE bodies and bearer credential references |
| `mcp-oauth` | Authorization-code login through the SDK, PKCE, callback state and issuer validation |
| `mcp-fixture-tests` | Dedicated process/network qualification target; includes OAuth |

```sh
cargo build --locked --release -p sefai --features mcp-oauth
sefai --workspace ROOT --profile tools chat --tool-protocol smollm2 --mcp files
```

The startup approval shows the server ID, executable or network endpoint, arguments, working directory, environment references and any OAuth issuer/client/scopes.
Enter and Esc deny; `y` approves this connection once.
No server starts and no endpoint is contacted before approval.
A connection does not approve a model-proposed tool call.
A failed/cancelled startup closes its connection and returns the TUI to a visible no-server state.
`/mcp` shows the selected connection and `/disconnect` closes it.
The connect preview marks a loopback, private, link-local or single-label host so a local-network service is approved deliberately.
sefai declares no roots, sampling or elicitation capability and answers a server that requests any of them with method-not-found.
Restart with `--mcp ID` to connect again or to apply a changed definition.
There is no persistent always-allow grant.

## Markdown definitions

A local definition at `mcp/files.md`:

```yaml
---
schema: 1
id: files
transport:
  kind: stdio
  command: /absolute/canonical/path/to/server
  args: ["--directory", "/explicit/path"]
  env:
    SERVER_TOKEN: MY_SERVER_TOKEN
---
Notes describe this connection; they cannot approve it.
```

The executable must be an absolute canonical regular file.
No shell command is assembled, and Windows `.cmd`/`.bat` shims are refused.
Use an actual executable and explicit arguments.
The child starts in the workspace directory with an empty environment, plus the named references and Windows `SystemRoot` when available.
Each supplied environment value is at most 4,096 bytes.
PATH, home directories, proxy settings and credentials are not inherited automatically.
Add references deliberately when a particular server needs them.
Stderr goes to the null device, so untrusted logs cannot flood the terminal or a captured buffer.

A remote definition at `mcp/service.md`:

```yaml
---
schema: 1
id: service
transport:
  kind: http
  url: https://service.example/mcp
  token_env: SERVICE_MCP_TOKEN
---
```

Supply a credential intended for this resource, never a token copied from an unrelated provider.
Only the named endpoint receives the bearer header.
Endpoints require HTTPS, except literal `127.0.0.1` or `[::1]` HTTP for local servers and fixtures.
Credentials in URLs, query strings and fragments are rejected.
The connection preview uses the parsed canonical endpoint.
Redirects, ambient proxies, automatic expired-session recovery and SSE resumption are disabled.
JSON POST responses and SSE POST streams are supported; optional standalone GET notifications are not enabled.
Failure does not trigger a request replay or transport fallback.

For OAuth, replace `token_env` with:

```yaml
  oauth:
    issuer: https://auth.example
    client_id: your-preregistered-public-client
    redirect_uri: http://127.0.0.1:8421/callback
    scopes: [tools]
```

Use a pre-registered public client with that exact loopback redirect URI.
The callback port is bound before the authorization URL is shown.
Open the URL manually in your browser and complete the provider's consent flow.
Only the selected resource origin and explicitly approved issuer origin may receive SDK discovery/token HTTP requests.
Issuer metadata must match the configured issuer, and the browser authorization URL must use its origin.
The SDK owns PKCE, resource binding and callback state/issuer verification.
The callback request is limited to 8 KiB; the full flow has a 120-second deadline and Ctrl+C cancellation.
Access and refresh credentials remain in memory and are discarded on disconnect/exit.
Automatic refresh, dynamic client registration, scope upgrades and credential-store persistence are not implemented.
An expired/rejected token ends the failed tool connection without replay; explicitly reconnect and authorize again.
A fixed callback port can be occupied; the connection fails rather than selecting an unregistered redirect silently.

## Discovery and reads

The client advertises no sampling, elicitation or filesystem roots.
Server instructions are not imported into the system prompt.
The supported protocol is exactly `2025-11-25`; a different selected version stops the connection before discovery.
No claim is made that all revisions implemented by the SDK are compatible.
Tool, static-resource and prompt catalogs are paginated and frozen when connecting.
Duplicate names/URIs, repeated cursors and catalog overflow fail visibly.
Tool/prompt names use ASCII letters, digits and `_-./:`; unsupported names fail discovery instead of entering ambiguous approval rows.
Resource templates, subscriptions and long-running MCP tasks are not enabled.

Inspection does not require a model or GGUF support:

```sh
sefai --workspace ROOT mcp files inspect
sefai --workspace ROOT mcp files read 'file:///explicit/discovered/path'
sefai --workspace ROOT mcp files prompt explain --arguments '{"topic":"cache"}'
```

Each command displays its requested operation with the connection preview and requires a literal `y` line.
Read and prompt operations must name entries in the discovered catalog.
Returned content is sanitized for terminal controls and printed as JSON.
These commands do not install returned prompts as instructions or automatically feed resources into a conversation.
Select and inspect this untrusted content deliberately.

## Harness authority

Tools remain opt-in through `--tool-protocol smollm2`.
The added function is `mcp_call` with arguments `{"id":"server-id","mcp":{"tool":"exact-name","arguments":{}}}`.
The model sees a bounded catalog labeled untrusted; server descriptions and annotations never grant permissions.
The selected connection, exact Markdown definition and discovered tool names are fixed before a user turn is composed.
A model cannot add servers, executables, endpoints or tools to that scope.

Every call displays server, tool, exact JSON arguments, destination and the possibility of external reads/writes.
Unicode is escaped in the approval arguments; ordinary ASCII JSON stays readable.
The existing deny-default approval, turn/time/token/call budgets, one pending decision, durable dispatch intent and post-approval revalidation apply.
A changed definition, failed checkpoint or cancellation before dispatch prevents the call.
Tool results enter the distinct untrusted tool role; structured results and `isError` are preserved.
A server-declared tool error is recorded as failed.
Transport errors, timeout, cancellation after dispatch and oversize results are conservatively unknown because external side effects may already have happened.
A failed tool connection is closed; no automatic reconnect/retry occurs.
An unknown outcome stays uncertain even if a later operation succeeds on the same server.
Known environment credentials and the current OAuth access token are redacted from new durable conversation/audit records.
This is literal-value redaction, not a guarantee of detecting arbitrary encoded secrets.

## Bounds and lifecycle

| Item | Limit |
| --- | --- |
| Connected servers | 1 |
| Concurrent ordinary HTTP POSTs | 1, plus the SDK control slot |
| HTTP worker channel | 4 |
| Incoming SDK messages per connection | 512; explicitly reconnect when exhausted |
| Stdio bytes per connection | 4 MiB |
| One stdio line / HTTP JSON body / entire SSE POST body | 64 KiB |
| Catalog per kind | 128 entries, 16 pages, 64 KiB |
| Model-visible tool catalog | 16 KiB before instructions/framing |
| Resource URI / tool identity | 256 bytes |
| Discovery cursor | 1,024 bytes |
| Request arguments | 8 KiB including the method-specific parameter envelope |
| Tool/read/prompt result | 16 KiB; reject rather than silently truncate |
| Request / connect | 30 seconds; a tool request also honors the remaining turn time |
| OAuth HTTP response / callback | 64 KiB / 8 KiB |
| OAuth flow | 120 seconds |
| Shutdown waits | service 2 seconds, child termination/reaping 3 seconds |

The runtime advances on the worker's connection/request boundaries rather than creating another thread pool beside inference.
Stdio servers belong to a Unix process group or Windows Job Object; teardown kills the owned group/job and waits for the direct child.
These are lifecycle controls, not an OS filesystem/network sandbox.
A hostile Unix descendant that deliberately leaves its process group can escape that ownership boundary.
An advertised root is not a permission grant or a sandbox.
Plain-mode stdin remains line-blocking as documented in [tui.md](tui.md).

## Dependency decision and evidence

`rmcp = 3.6.0` is exactly pinned with defaults disabled: only client and bounded async I/O are enabled for stdio.
Tokio supplies the SDK runtime, timers, pipes and process I/O; `process-wrap` supplies safe group/job supervision that the standard library cannot provide across platforms.
Optional HTTP adds `reqwest` for TLS/HTTP, `http` for the SDK header boundary, `futures-util` for bounded stream adaptation and `sse-stream` for incremental SSE parsing.
The SDK's auth feature supplies the OAuth state machine rather than implementing PKCE or token exchange ad hoc.
No new dependency enters `core`; no optional model runtime is added.
The stock SDK HTTP JSON path buffers an entire body, so sefai provides a bounded `StreamableHttpClient` implementation instead.
The adapter also prevents response bodies and authentication challenges from being copied into error logs.

Local process, authenticated HTTP/OAuth fixture, real-GGUF and terminal evidence is recorded in [verification.md](verification.md).
The user's selected qualification target is local/dummy servers; this is not evidence of compatibility with an unrelated production service.
Physical Windows/Linux process-tree behavior and the full footprint/hardware matrix retain their Phase 10 gates.

Sources: [official Rust SDK](https://docs.rs/crate/rmcp/3.6.0), [pinned transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle), [authorization](https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization).
