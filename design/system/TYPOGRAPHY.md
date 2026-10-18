# Typography

Four roles, all free to ship. The pairing follows Hermes: the platform face for chrome,
JetBrains Mono for anything machine-shaped, a light serif for rare editorial moments, and a
wide bold wordmark.

| Role | Token | Face | Licence | Use |
|---|---|---|---|---|
| UI | `--font-sans` | Platform stack: Segoe UI, SF Pro, then Inter, Noto Sans | System / OFL | All chrome, labels, prose, buttons |
| Mono | `--font-mono` | **JetBrains Mono** 400, 700, 400 italic | OFL 1.1 | Code, traces, logs, tool rows, IDs, paths, aligned numbers, the terminal |
| Display | `--font-display` | **Newsreader** variable, weight 300 | OFL 1.1 | Empty-state titles, onboarding, report headings, docs heroes |
| Wordmark | `--font-wordmark` | **Archivo** variable at `wdth 125`, weight 800, uppercase | OFL 1.1 | The app name on the empty home view and the About screen. Nowhere else |

Hermes's own display faces (Sigurd, Rules, Collapse) are licensed to Nous and not
redistributable. Newsreader and Archivo are the closest free equivalents in role, not clones.

## Why the platform face for UI

Hermes desktop uses the OS font for chrome so it reads as a native tool, and keeps a bundled
mono so code renders the same everywhere. These are Tauri apps on three OSes and a Chrome
extension; the same reasoning holds, and it costs zero bytes. Inter is listed in the stack so
Linux machines without a good default still get a consistent face.

## Scale

Root stays at 16px so browser and OS zoom work. Dense app views size down from it.

| Token | Size / leading | Face | Use |
|---|---|---|---|
| `--text-2xs` | 11 / 18 | sans or mono | Tool rows, table meta, status bar, badges |
| `--text-xs` | 12 / 16 | sans | Captions, labels, hints, tooltips |
| `--text-sm` | 13 / 18 | sans | **Default app text**, transcript (× `--chat-scale`) |
| `--text-md` | 14 / 20 | sans | Forms, settings rows, side panels, chromeclaw body |
| `--text-lg` | 16 / 24 | sans | Section titles; **body on phones** (parareport) |
| `--text-xl` | 20 / 26 | sans 600 or display 300 | Page titles; display H4 |
| `--text-2xl` | 28 / 34 | display 300 | Display H3, empty-state titles |
| `--text-3xl` | 42 / 48 | display 300 | Display H2, onboarding |
| `--text-4xl` | 52 / 56 | display 300 | Display H1. Docs and landing pages only |

- `--chat-scale` (default 1.1) scales conversation text and the composer. It never scales
  chrome, sidebars or toolbars. Expose it in settings as "Text size".
- Display headings use weight 300 and `0.03em` tracking at H1–H2, matching the Nous DS.
- Uppercase labels (`.eyebrow`, `.section-heading`) use `0.05em` tracking.
- The wordmark uses `0.08em` tracking and `line-height: 0.9`, and fits the column width.
- Any column of numbers uses `font-variant-numeric: tabular-nums`.
- Reading columns cap at `--content-max` (44rem, about 75 characters at 13px).
- Headings get `text-wrap: balance`.

## Scripts beyond Latin

parareport is Kolkata-first, and maiku and sefai may hold any language.

- Add **Noto Sans Bengali** and **Noto Sans Devanagari** to `--font-sans` in parareport (after
  the platform faces, before `sans-serif`). For Bengali body text set `--leading-body: 1.7`;
  conjuncts and vowel signs need the room.
- JetBrains Mono has no Bengali or Devanagari. The mono stack falls back to Noto Sans Mono, then
  the UI face; don't force monospace on non-Latin user text.
- CJK falls back per locale, as Hermes does: PingFang / Microsoft YaHei / Noto Sans CJK.

## Loading fonts

These apps are local-first. **Ship fonts with the app; never load them from a CDN at runtime.**

```sh
npm i @fontsource/jetbrains-mono @fontsource-variable/newsreader @fontsource-variable/archivo
```

```ts
// main.ts, before any CSS that uses them
import '@fontsource/jetbrains-mono/400.css'
import '@fontsource/jetbrains-mono/700.css'
import '@fontsource/jetbrains-mono/400-italic.css'
import '@fontsource-variable/newsreader/wght.css'
import '@fontsource-variable/archivo/wdth.css'
```

Fontsource registers `'Newsreader Variable'` and `'Archivo Variable'`; `tokens/base.css`
already lists those names first. Archivo's `wdth.css` covers `font-stretch: 62%–125%`, which the
`.wordmark` class uses.

- Set `font-display: swap` for mono and `optional` for the display serif, so a slow first paint
  never shifts the layout.
- Subset Newsreader to Latin unless the app shows display headings in another script.
- chromeclaw: bundle the woff2 files in the extension and reference them with
  `chrome.runtime.getURL`; extension pages can't rely on remote fonts under MV3's CSP.
- Docs sites and the preview page may use Google Fonts.

## Terminal type

The terminal's font belongs to the user. Design for any monospace:

- Use only glyphs that render in common terminal fonts: box drawing (`─ │ ┊ ╭ ╮`), braille
  (`⠋⠙⠹`), blocks (`█ ▓ ░`), and the project glyph. Check each project glyph in JetBrains Mono,
  SF Mono, Cascadia and the default Windows Terminal font before shipping.
- Emoji in tool rows are optional and must degrade: when `TERM=dumb` or colour is off, replace
  them with a one-cell ASCII mark.
- Width-sensitive layout (the nine-cell verb column, the banner) assumes 1 cell per character.
  Kawaii faces contain wide characters; keep them at the end of a line.
