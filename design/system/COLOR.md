# Color

A **skin** is colour only: a terminal palette for dark and light terminals and seven GUI
seeds for dark and light mode. Everything else is derived. Any project can wear any skin; a
project's name, glyph, faces and banner are its **identity**, kept separately.

```
src/skins.mjs ──┐                      ┌▶ tokens/skins.css    GUI seeds per [data-skin]
src/identities.mjs ─node scripts/build.mjs─┼▶ tokens/tokens.json  everything, for other tools
banners/*.txt ──┘                      ├▶ rust/palettes.rs    every skin as Rust constants
                                       ├▶ rust/identities.rs  every identity as Rust constants
                                       └▶ hermes/<project>.yaml  identity + default skin, for Hermes
tokens/base.css: seeds ──color-mix()──▶ --ui-* surfaces, text, fills, strokes
```

Edit `src/skins.mjs` and run the build. It refuses to write if any pair misses its contrast
floor. A project's choice of skin is recorded in its `design/kit.json` (`kit use`).

## The eight skins

| Skin | `data-skin` | Dark accent | Light accent | Character | Default for |
|---|---|---|---|---|---|
| Sakura | `sakura` | `#FF8FC7` | `#C2307A` | Pink and mauve on plum-black | maiku |
| First light | `first-light` | `#F5A65B` apricot, `#8B7CF6` violet actions | `#B0581A`, `#5B4BD6` | Dawn over deep indigo | horizon |
| Ember | `ember` | `#FF7A5A` | `#C2461F` | Coral on neutral greys (matches Chrome's panels) | chromeclaw |
| Lens | `lens` | `#FFE6CB` cream, `#2FBFAE` teal | `#0F7068` | Hermes dashboard teal | reasontrace |
| Candle | `candle` | `#E8A87C` | `#A9552E` | Rose-gold on warm black | sefai |
| Trophy | `trophy` | `#F5C242` | `#8F6400` | Classic Hermes gold and bronze | stockfih |
| Shell | `shell` | `#EE6A52` crimson, `#E8C88A` bronze | `#B5301F` | Ares-adjacent, safety first | casca |
| Kolkata | `kolkata` | `#F2C94C` | `#7A5B00` text, `#0E6B4F` actions | Taxi yellow and deep green on paper | parareport |

"Default for" is only a starting suggestion. Before a project chooses, `tokens/base.css`
renders with Trophy's light seeds.

Family traits every skin keeps:

- Near-black dark surfaces with a slight hue toward the accent (never pure `#000`).
- Light surfaces that are warm or cool off-white, never flat `#fff` chrome (cards may be white).
- Selection in Hermes yellow (`#ffd24a`).
- The same semantic palette for success, warning, error and info.
- The same derivation ladders, so every app's hierarchy reads the same.

## Seeds

| Seed | Role |
|---|---|
| `ink` | Text and the base of every translucent fill and stroke |
| `accent` | Brand hue. Selected rows, links, focus, the live dot, tints in fills and strokes |
| `primary` | The primary button and the one action per view that matters most |
| `warm` | Secondary warmth: highlights, the warm glow, memory and history moments |
| `bg` | Window chrome and the main surface |
| `sidebar` | Left navigation, a step darker than `bg` |
| `card` | Editors, the composer and inputs |

Only `tokens/base.css` reads seeds. Components read `--ui-*`.

## Derived tokens

| Token | Derivation | Use |
|---|---|---|
| `--ui-text-1…4` | ink at 94 / 74 / 54 / 36% | Primary, secondary, meta, disabled |
| `--ui-stroke-1…4` | accent 24 / 16 / 10 / 6% over ink at 10 / 8 / 5 / 3% | Strong rule, input border, **default divider (3)**, faintest |
| `--ui-fill-1…5` | accent 16 / 11 / 8 / 5 / 3% over ink at 8…3% | Toggles, soft buttons, badges, chips |
| `--ui-row-hover` / `-active` | accent 4% / 8% over ink | List rows |
| `--ui-control-hover` / `-active` | accent 6% / 8% over ink | Quiet controls |
| `--ui-bg-bubble` | accent 6% (8% dark) into card | The user's message bubble |
| `--ui-selection` | `#ffd24a` at 55% (38% dark) | Text selection, everywhere |

## Semantic colour

Separate from the accent. Never use the accent to mean success or danger.

| Token | Light | Dark | Meaning |
|---|---|---|---|
| `--ui-ok` | `#1f8a65` | `#55a583` | Done, verified, synced |
| `--ui-warn` | `#c08532` | `#d9a24a` | Needs attention, not broken |
| `--ui-error` | `#cf2d56` | `#e75e78` | Explicitly failed |
| `--ui-info` | `#4c7f8c` | `#6f9ba6` | Neutral notice |
| `--ui-purple` | `#7a6fc0` | `#9e94d5` | Model-derived or inferred (see below) |

**Provenance colours.** Several projects distinguish where a fact came from. Use the same
mapping everywhere so it reads the same across apps:

| Provenance | Colour | Mark |
|---|---|---|
| Observed directly (page text, screen element, engine output, a resident's report) | `--ui-text-1` | solid |
| Stated by a model about itself, or claimed by a person | `--ui-warm` | dashed underline |
| Inferred or decoded, with a passing control | `--ui-info` | solid dot |
| Inferred or decoded, without a control | `--ui-purple` | hollow dot |
| Unavailable | `--ui-text-4` | the reason, in words |

## Contrast floors

`scripts/build.mjs` checks 20 pairs per skin and mode (320 in total):

- Terminal body text and prompt on the status background: **7:1**.
- Accent, label, ok, warn, error and status text: **4.5:1**.
- Banner title, status strong and status dim: **3:1** (large or bold, or UI marks).
- GUI ink on bg, card and sidebar: **12:1**. Accent on bg and card: **4.5:1**, so the accent can
  be used for link text. Primary on bg: **3:1**, since it is a filled control.

Light terminal palettes are tuned by hand, as Hermes does: bright accents on white become glare,
and naive darkening becomes mud. Keep the hue, lower the saturation, then check.

## Mode

- Apps follow the OS (`prefers-color-scheme`) unless the user picks a mode, which sets
  `data-theme="light"` or `"dark"` on `<html>`.
- **Light-first projects** (parareport: outdoors, on phones) set `data-theme="light"` at
  startup and switch to dark only when the user asks. This is the identity's `lightFirst`
  flag, not part of any skin, so it holds whichever skin the project wears.
- The terminal follows the terminal: `<APP>_THEME=light|dark`, then `COLORFGBG`, then Apple
  Terminal's light default, else dark.

## Adding a skin

1. Copy an existing entry in `src/skins.mjs`, rename it, and change the hues.
2. Keep the roles: the title is the brightest brand colour, the border is the deepest, `dim`
   is for the version line and metadata.
3. Run `node scripts/build.mjs`. Fix every failing pair it reports.
4. Check it in `preview.html` against several identities, in both modes.
5. Bump `VERSION` (minor) and add a `CHANGELOG.md` entry. Projects see it on their next
   `kit status` and can switch with `kit use <repo> <name>`.
