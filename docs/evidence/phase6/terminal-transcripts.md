# Phase 6 terminal records

Actual 90 × 24 macOS PTYs, `NO_COLOR=1`, combined MLX/GGUF/MCP release build.
Each process exits zero and restores its initial terminal attributes.
Screen cells omit wide-glyph continuation placeholders; model text is retained.

## unicode-switch-stats

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you
Repeat exactly this greeting: こんにちは

☾ sefai
こんにちは。

❯ you
Reply with one short greeting.

☾ sefai
こんにちは？






──────────────────────────────────────────────────────────────────────────────────────────
❯

Enter send · Shift+Enter newline · /help
 SmolLM2-135M-Instruc · Mlx · ctx 84/4096 · local · 4 records
```

## cancel

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you
Count from 1 to 1000, one number per line, without stopping.

☾ sefai













──────────────────────────────────────────────────────────────────────────────────────────
❯

generation interrupted or output limit reached; no tool dispatched
 Qwen3-0.6B-4bit · Mlx · ctx 31/4096 · local · 2 records · cancelled
```

## resume

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you
Count from 1 to 1000, one number per line, without stopping.

☾ sefai

❯ you
Now say hello briefly.

☾ sefai
Hello! 😊







──────────────────────────────────────────────────────────────────────────────────────────
❯

Enter send · Shift+Enter newline · /help
 Qwen3-0.6B-4bit · Mlx · ctx 51/4096 · local · 4 records
```

## mlx-to-gguf

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you
Say hello briefly.

☾ sefai
Hello! How can I assist you today?

❯ you
Say hello briefly.

☾ sefai
Hello! How can I assist you today?






──────────────────────────────────────────────────────────────────────────────────────────
❯

Enter send · Shift+Enter newline · /help
 SmolLM2-135M-Instruc · Cpu · ctx 47/2048 · local · 4 records
```

## allocator statistics overlay

```text
 ☾ sefai
 v0.1.0 · local inference · /help
❯ you  ┌ Esc close ──────────────────────────────────────────────────────────────┐
Repeat │last generation step                                                │
       │prompt 19 tokens                                                         │
☾ sefai│generated 3 tokens                                                       │
こんにちprefill 0.052s                                                           │
       │generation 0.013s                                                        │
       │stop EndOfGeneration                                                     │
       │verified prefix reused 0 tokens; weights retained                        │
       │tools: disabled · context max 40960 · cancellation between native calls  │
       │mlx_active_bytes=394100736 mlx_cached_bytes=68155764                     │
       │mlx_peak_active_bytes=454552934; process-global allocator counters, host │






──────────────────────────────────────────────────────────────────────────────────────────
❯

Esc stop · Ctrl+D quit
 preparing request
```
