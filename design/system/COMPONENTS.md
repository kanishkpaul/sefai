# Components

The shared primitives, implemented as plain CSS in `tokens/components.css` plus one web
component, `<curve-loader>`, in `tokens/loader.js`. Svelte projects use the classes directly.
horizon (React) wraps each in a component that takes `variant` and `size` props and emits the
same classes. Every project gets the same set; projects add their own components in their own
folders, built from these.

**Call sites pick a variant and size. They never pass padding, radius, colour or shadow.** If a
primitive can't do what you need, change the primitive and this file in the same commit.

## Button

`.btn` plus at most one variant and one size.

| Variant | Class | Use |
|---|---|---|
| Primary | `.btn` | The one action per view that matters most. At most one visible per region |
| Secondary | `.btn--secondary` | Soft fill. The default for everything else |
| Outline | `.btn--outline` | A second option beside primary when secondary is too quiet |
| Ghost | `.btn--ghost` | Toolbars and dense rows |
| Destructive | `.btn--destructive` | Delete, revoke, reset. Always behind a confirm |
| Text | `.btn--text` | "Cancel", "Clear", "Skip". Boxless |
| Text strong | `.btn--text-strong` | Inline affordance inside a sentence: "Change", "Open logs" |
| Floating | `.btn--floating` | A control loose on top of content (jump to latest, map controls) |

| Size | Class | Use |
|---|---|---|
| xs | `.btn--xs` | Table footers, status stack |
| sm | `.btn--sm` | Dense rows, chromeclaw |
| default | (none) | Desktop |
| lg | `.btn--lg` | Onboarding, empty-state actions |
| touch | `.btn--touch` | parareport and any phone layout |
| icon | `.btn--icon` (+ `--sm`, `--lg`) | Icon-only. Needs `aria-label` |

Text buttons are square. Icon buttons have a 4px radius. Never put a raw `title=` on a button;
use a tooltip, and only when hover teaches something the screen doesn't already say (not on
menu triggers, not on close buttons).

## Badge

`.badge` with `--muted`, `--ok`, `--warn`, `--destructive`, `--outline`, `--solid`, and
`--mono` for IDs and counts. Badges label state; they are not buttons.

## Form controls

| Primitive | Class | Rule |
|---|---|---|
| Text field | `.field` on `input`, `textarea`, `select` | 2px radius, recessed inset, border at 70% of stroke-1 |
| Label / hint | `.label`, `.hint`, `.hint--error` | Label above, hint below. Errors replace the hint |
| Search | `.search` (wraps an `input`) | Borderless, underline on focus. The only search input. Hidden when its list is empty |
| Segmented | `.seg` with `button[aria-pressed]` | Two to five mutually exclusive options. Replaces radio piles |
| Switch | `input[type=checkbox].switch` | Bare, with an `aria-label` or a `<label>`. Applies immediately |
| Key cap | `.kbd` | Shortcut hints. Uses the native UI face |

Forms validate on blur and on submit, never on each keystroke. Required fields say "Required"
in the hint, not with an asterisk alone.

## Lists

`.row` with `.row__title`, `.row__desc`, `.row__meta`. Flat and flush left. Hover and active
fills come from the row tokens. Add `.list--divided` to the parent only when rows have no other
visual separation. `.section-heading` labels a group.

## Surfaces

| Primitive | Class | Use |
|---|---|---|
| Float | `.float` | Dialogs, overlays, onboarding cards, approval cards |
| Menu | `.menu` + `.menu__item` | Dropdowns, selects, context menus, command palette results |
| Widget | `.widget` | An inline panel inside a transcript. Actions go below it, outside |
| Tooltip | `.tip` | Short labels; descriptions wrap at 24rem |

## Transcript

Used by maiku, chromeclaw, sefai, casca and the tutor in stockfih.

| Primitive | Class | Rule |
|---|---|---|
| User message | `.bubble` | Right-aligned, 8px radius, accent-tinted card, hairline |
| Agent prose | `.prose` | No bubble. Full column width, capped at 44rem |
| Tool rows | `.tool-row` in a `.tool-stack` | The TUI row in HTML: `┊`, icon, nine-ch verb, detail, time. `data-state="running|failed"` |
| Thinking | `.thinking` | Collapsed by default to one line; expands in place |
| Status dot | `.dot` (`--ok`, `--warn`, `--error`, `--live`) | Always beside a word |
| Approval | `.approval.float` | See [DESIGN.md](DESIGN.md#gates-and-approvals). `data-queued` draws the stack silhouette |

Rhythm: `--turn-gap` between a reply's prose, tool groups and thinking; `--scaffold-gap` between
adjacent tool rows; `--paragraph-gap` between paragraphs.

The transcript never scrolls itself while the user is reading. It follows new content only
when already at the bottom, and shows a floating "Jump to latest" button otherwise.

## Feedback

| Primitive | Class / element | Rule |
|---|---|---|
| Loader | `<curve-loader type="lemniscate" label="…">` | `lemniscate` for long work, `thinking` for model turns, `rose`, `lissajous`, `cardioid` for variety. Inherits colour. Not shown under 300ms |
| Empty | `.empty` + `.empty__title` | Display-serif title, one line of help, one action |
| Error | `.error-state` + `.error-state__title` | Cause, fix, and a "Show details" disclosure holding a `.log` |
| Log | `.log` | Raw output: mono, hairline, no fill, 16rem cap, hands scroll back to the page at the edges |
| Terminal | `.term` with `.t-*` spans | Real CLI output inside a GUI, in the skin's terminal palette |

## Brand

| Primitive | Class | Rule |
|---|---|---|
| Wordmark | `.wordmark` | App name on the home empty state and About. Once per screen at most |
| Display | `.display-1`, `.display-2`, `.display-3` | Newsreader 300. Empty states, onboarding, reports, docs |
| Eyebrow | `.eyebrow` | Mono uppercase label above a display heading |
| Glyph | text | The project glyph (✿ ⌓ ⋔ ∴ ☾ ♞ ⬡ ⌂) in response labels, the tray icon, and favicons |

## Icons

Tabler for chrome, Codicons for code, tool and status. One import module per app
(`src/lib/icons.ts`) re-exports the curated set; feature code imports from there.

## Adding a primitive

1. Check that no existing primitive plus a variant covers it.
2. Add it to `tokens/components.css` using only tokens.
3. Document it here: class, variants, when to use, when not to.
4. Add it to `preview/index.html` so it can be seen in every skin and both modes.
