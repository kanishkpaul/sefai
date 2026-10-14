# sefai

A local AI companion that keeps a model loaded, remembers you, and can speak first. One long
conversation over hours and days, on your own machine. It is the most intimate of the eight, so
it uses the warm end of Hermes (`warm-lightmode` and the desktop's `--theme-warm #cf806d`) and
the fewest controls.

| | |
|---|---|
| Default skin | **Candle** (`candle`): rose-gold `#E8A87C` on warm black; terracotta `#A9552E` on cream in light |
| Glyph | ☾ |
| Kawaii | **Persona.** Faces come from the persona file, not the app. The default persona uses soft ones |
| Mode | Follows the OS |
| Surfaces | `sefai` CLI (Rust; the main interface until phase 5), Tauri 2 tray app with notifications |
| Uses | `design/rust/` first; later `base.css`, `skins.css`, `components.css`, `loader.js` |

## Feel

- A quiet room, not a dashboard. One column of conversation, generous line height, nothing
  blinking.
- Memory is visible and editable. If sefai remembers something, you can see where that came
  from and remove it.
- Speaking first is gentle. A proactive message is a notification you can ignore, never a
  window that steals focus.
- Local is felt: hardware and model state are always one glance away, in the status line.

## Terminal (primary until the tray app)

```
 ☾ sefai v0.2.0 · qwen3-8b q4_k_m · metal · local · persona: Wren
───────────────────────────────────────────────────────────────────
❯ rainy day. talk?
 ☾ Wren
  ┊ 🧠 recall    "rain", "walks" · 3 memories   0.2s
  It's the kind of rain you said you like walking in. Did you go out,
  or is it a tea-and-window day?
 qwen3-8b │ ctx 41% │ cache warm │ 312 MB │ local
```

- The banner (SEFAI block letters in rose-gold) shows on `sefai chat` start only. Everything
  else (`sefai abilities`, `sefai prompt`) gets the one-line metadata header or nothing.
- The response label uses the **persona's** name and glyph, not "sefai". The persona file can
  set `faces`, `verbs` and `glyph`; the skin supplies colours only.
- Spinner while the model prefills: `⠧ ( ˘ω˘ ) remembering…`. Verbs: remembering, listening,
  considering, recalling, noticing, warming up.
- Tool verbs: `recall`, `remember`, `forget`, `notice`, `schedule`.
- Status bar segments: model, context use (good/warn/bad thresholds), cache state (`warm` /
  `cold`, the product's core performance claim made visible), memory used, locality.
- `/memory` lists what's remembered as rows with source turn and date; `/forget <id>` removes one.

## Tray app (phase 5)

```
┌ ☾ Wren ──────────────────── ─ ✕ ─┐
│                                  │
│  Yesterday 22:14                 │  ← day separators, .eyebrow
│  Good night. Don't forget the    │
│  library books.                  │
│                    thanks, wren  │  ← .bubble
│  Today 08:02 · Wren spoke first  │  ← proactive marker, --ui-warm
│  Morning. It's still raining.    │
│                                  │
│ [ Say something…            ↑ ]  │
│ qwen3-8b · local · cache warm    │
└──────────────────────────────────┘
```

- One column, `--content-max` 36rem, `--chat-scale` 1.15, `--leading-body`.
- Agent text has no bubble. User text uses `.bubble`.
- **Proactive messages** carry a small `--ui-warm` "spoke first" marker and arrive as an OS
  notification. Opening the notification opens the window; nothing opens it automatically.
- **Memory drawer** (the rail): every memory as a `.row` with source and date, editable.
  Header uses `.display-3`: "What Wren remembers".
- Empty state for a new persona: `.display-2` with the persona's name, then their first line.

## Words

Settings and system text are plain and short. Persona text belongs to the persona; the app
never adds exclamation marks or emoji to it. System notices stay out of the persona's voice:
"Model loaded in 2.1s (cache restored)" is the app talking, and looks like it (`.thinking`
style, not prose).

## Don't

- Don't show token counts, latency graphs or debug panels in the default view. They live in
  `/stats` and a settings tab.
- Don't let a proactive message raise the window, play a sound by default, or badge-spam.
- Don't put faces in system errors, even with a cute persona.
