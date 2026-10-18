# What Hermes Agent does

Read from the source of [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent)
(MIT) on 2026-10-14, plus `@nous-research/ui` 1.5.2 from npm. Paths below are in that repo.
This kit borrows Hermes's method and a few token values. It does not copy Nous branding.

## Three dialects, one skin

Hermes has three visual surfaces that look different but share one contract.

| Surface | Look | Source |
|---|---|---|
| TUI / CLI | Gold, amber and bronze on navy-black. Block-letter logo, braille caduceus, kawaii spinner faces, `┊` tool rows, a navy status bar | `ui-tui/src/theme.ts`, `hermes_cli/skin_engine.py`, `hermes_cli/banner.py`, `agent/display.py` |
| Desktop app (Electron) | Quiet and flat. Near-neutral surfaces, one accent, hairlines, near-square corners, system UI font with JetBrains Mono | `apps/desktop/DESIGN.md`, `apps/desktop/src/styles.css` |
| Web dashboard and brand | Deep teal `#041c1c` with cream `#ffe6cb`. Sigurd serif display at weight 300, Rules sans, Courier Prime | `web/src/index.css`, `@nous-research/ui` |

The contract is the **skin**: one YAML file in `~/.hermes/skins/<name>.yaml`, resolved by the
Python skin engine and pushed to every surface. From `apps/shared/src/skin.ts`:

> Tokens are terminal-first (the CLI is the oldest surface); GUIs derive their fuller palettes
> from the load-bearing few.

That is the idea this kit adopts. Each of your projects gets a skin. The terminal uses its
colours directly; the GUI derives surfaces, text, fills and strokes from seven seeds.

## TUI details worth keeping

- **Default palette.** Title `#FFD700`, accent `#FFBF00`, border `#CD7F32`, dim `#B8860B`,
  text `#FFF8DC`, status bar `#1a1a2e`, completion highlight `#333355`, selection `#3a3a55`,
  shell `$` `#4dabf7`. Other built-ins: ares (crimson/bronze), mono, slate, daylight,
  warm-lightmode, poseidon, sisyphus, charizard.
- **Paired polarities.** Each skin has `colors` plus optional `light_colors` / `dark_colors`.
  The TUI detects terminal polarity (`COLORFGBG`, then a `TERM_PROGRAM` allow-list with Apple
  Terminal as light-by-default) and picks the matching set. Undecided means dark.
- **Light mode is tuned, not inverted.** Vivid gold on white is 1.4:1 and reads as glare;
  WCAG-darkened mustard reads as mud. Hermes uses a goldenrod ladder (`#C8961E`, `#D89B04`,
  `#A97E10`) that keeps the hue and drops saturation.
- **Banner.** Six rows of ANSI Shadow block letters, coloured top to bottom: two rows title,
  two rows accent, two rows border. A braille-art caduceus sits beside it.
- **Branding strings.** `agent_name`, `welcome`, `goodbye`, `response_label` (` ☤ Hermes `),
  `prompt_symbol` (`❯`), `help_header` (`(^_^)? Available Commands`).
- **Tool rows.** `┊ {emoji} {verb:9} {detail}` with the duration appended. The verb column is
  nine cells so a run of tools lines up.
- **Spinner.** Braille frames plus a kawaii face (`(｡◕‿◕｡)` while waiting, `(◔_◔)` while
  thinking) plus a rotating verb (`pondering`, `cogitating`, `synthesizing`…). Skins override
  faces, verbs and `⟪ ⟫` wings. Non-TTY output logs once instead of animating.

## Desktop details worth keeping

From `apps/desktop/DESIGN.md`, the rule of thumb is "one source per concern, tokens over
literals, flat over boxed."

- **Flat, not boxed.** No card-in-card. Group with whitespace and one hairline.
- **Borderless elevation.** Floating panels use a soft downward shadow (`--shadow-nous`) plus a
  3% `currentColor` hairline (`--stroke-nous`).
- **One primitive per concern.** One Button, one SearchField, one Loader, one ErrorState,
  one ConfirmDialog. Call sites pass a variant and size, never padding or colour overrides.
- **Text buttons are square.** Only icon buttons have a radius (4px). The app renders at a
  radius scalar of 0.2, so panels are about 2–3px.
- **Type.** UI uses the platform font stack (Segoe UI, SF Pro). Code and terminal use bundled
  JetBrains Mono. Conversation text is 13px with 18px leading; tool rows 11px; a separate
  110% "chat text size" multiplier scales the transcript without touching chrome.
- **Colour derivation.** `--theme-*` seeds feed `color-mix()` ladders: text is ink at 94 / 74 /
  54 / 36% alpha; strokes are an accent tint over ink at 10% alpha; fills and hovers likewise.
  Light default: ink `#17171a`, primary `#0053fd`, warm `#cf806d`. Dark neutrals: chrome
  `#0d0d0e`, sidebar `#0a0a0b`, card `#161618`.
- **Selection is yellow.** `#ffd24a` at 55% (light) or 38% (dark), in every skin.
- **Loader.** Animated parametric curves (lemniscate, rose, lissajous, cardioid) with a comet of
  particles. The rule: never ship the literal text "Loading…".
- **Red is for real failures.** Ambiguous exit codes and missing paths get neutral notices.
- **Intent before automation.** A tool result may offer to open a preview; it never opens a pane,
  moves focus or navigates on its own. Background events update badges, not the foreground.
- **Motion.** About 100ms on controls. Tooltips wait 200ms, then stay warm for 300ms. Approval
  cards stack as one 96% silhouette 7px above, promote in 220ms, clear in 180ms.
  `prefers-reduced-motion` removes everything beyond a fade.
- **Icons.** Tabler for chrome, Codicons for editor/tool/status. No third set; no decorative
  sparkles.
- **Wordmark.** The empty chat is titled in Collapse Bold, uppercase, 0.08em tracking,
  fitted to the width.

## Fonts and licences

| Face | Where Hermes uses it | Licence | This kit |
|---|---|---|---|
| JetBrains Mono | Desktop code, terminal, docs | OFL 1.1 | Use as is |
| Platform UI stack | Desktop chrome | System | Use as is |
| Inter | Docs site and landing | OFL 1.1 | Fallback in the sans stack, and docs |
| Sigurd Variable | Nous DS display, H1–H4 at weight 300 | Commercial, not shipped in the npm package | Replaced by Newsreader (OFL) |
| Rules Variable | Nous DS body | Commercial, not shipped | Not used; the platform stack covers it |
| Collapse Bold | Desktop wordmark | Nous asset | Replaced by Archivo at 125% width, weight 800 (OFL) |
| Courier Prime | Nous DS mono | OFL 1.1 | Optional for typewriter moments only |

## What not to take

- The Hermes name, the caduceus, the `nous-girl` mark, or Nous's licensed fonts.
- Mythological skin names. Each project gets its own name and glyph instead.
- Electron-specific machinery (window glass, tray). Your desktop apps are Tauri 2.
