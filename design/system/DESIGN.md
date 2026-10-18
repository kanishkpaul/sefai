# Design system

The visual and interaction contract for maiku, horizon, chromeclaw, reasontrace, sefai,
stockfih, casca and parareport. It follows Hermes Agent's approach (see
[RESEARCH.md](RESEARCH.md)): the terminal is the oldest surface, every surface reads one skin,
and the GUI is flat, quiet and built from a small set of primitives.

Rule of thumb: **one skin per project, tokens over literals, flat over boxed.**

This file holds two kinds of content:

- **Principles** are durable. They outlast any component.
- **Named contracts** (tokens, primitive names, variants) are the current API. Change them in
  the same commit as the code, as you would a type.

When a rule and the code disagree, fix whichever is wrong. Don't fork a one-off at the call site.

| Read next | For |
|---|---|
| [COLOR.md](COLOR.md) | Skins, seeds, derived tokens, contrast floors |
| [TYPOGRAPHY.md](TYPOGRAPHY.md) | Faces, licences, sizes, loading fonts offline |
| [COMPONENTS.md](COMPONENTS.md) | Every primitive, its variants and when to use it |
| [TUI.md](TUI.md) | Terminal output: banner, prompt, tool rows, spinner, status bar |
| `design/README.md` | Each project's direction, layouts and specific components |

## Principles

1. **Terminal first.** Each project's CLI is a real interface, not a debug log. Design its
   output with the same care as the GUI, using the same skin.
2. **Flat, not boxed.** No card inside a card. No dividers inside a panel. Group with space
   and, when space is not enough, one `--ui-stroke-3` hairline.
3. **Borderless elevation.** Only floating things float: dialogs, popovers, approval cards,
   onboarding. They use `.float` (soft shadow plus a currentColor hairline). Nothing in the
   page flow has a shadow.
4. **One primitive per concern.** One button, one search field, one loader, one error state,
   one confirm dialog. Reuse; don't fork. Style lives in the primitive.
5. **Tokens, not literals.** Components use `--ui-*`, `--font-*`, `--radius-*`, `--shadow-*`,
   `--z-*`, `--dur-*`. Never a hex value, never `--seed-*`, never an ad-hoc rgba.
6. **Show the record.** Five of these projects are agents or analysis tools. What the system
   saw, proposed, was allowed to do, and sent off the machine is always one click away, in
   monospace, in order.
7. **Intent before automation.** Offer an action; don't take it. A finished background job
   updates a badge. It does not open a pane, move focus, scroll the transcript or switch views.
8. **Direct manipulation paints first.** The view updates in the current frame; persistence
   reconciles after and rolls back visibly on failure.
9. **Say what is unknown.** When a source can't provide something, show it as unavailable with
   the reason. Never fill the gap with a plausible-looking default.
10. **Local and offline by default.** Fonts, icons and scripts ship with the app. Nothing on a
    first paint needs the network.

## Information architecture

All eight apps share one layout grammar, scaled to the device.

```
┌ sidebar ─────┬ main ──────────────────────────────────┬ rail (optional) ┐
│ nouns:       │ the one primary surface for this app   │ working context │
│ sessions,    │ (transcript, case, board, trace, map)  │ (preview, trace,│
│ cases, games │                                        │  evidence, log) │
│              │ composer / action bar                  │                 │
├──────────────┴────────────────────────────────────────┴─────────────────┤
│ status bar: model · locality · context · gates · sync                   │
└─────────────────────────────────────────────────────────────────────────┘
```

- **One home surface.** Each app has a single primary view. Everything else complements it.
- **Pages are durable nouns** (Sessions, Cases, Games, Reports). Settings, pickers and short
  tasks open as overlays and return to where you were on close.
- **Rails are working context.** A rail belongs to the current task and keeps its state when
  hidden. Visibility is not lifecycle.
- **One action, one home.** A command can have a shortcut, a palette entry and a button, but
  they all call the same action.
- **The status bar is honest.** It names the model, where it runs (local / your key / none),
  and anything waiting on you. It is the GUI twin of the TUI status bar.
- **Narrow layouts collapse right to left.** The rail becomes a sheet, then the sidebar becomes
  a drawer. The main surface never shrinks below 28rem on desktop; below that, stack.

## Surfaces and elevation

| Surface | Token | Use |
|---|---|---|
| Chrome | `--ui-bg-chrome` | Window background, transcript, main view |
| Sidebar | `--ui-bg-sidebar` | Left navigation |
| Card | `--ui-bg-card` | Editors, the composer, inputs at rest |
| Elevated | `--ui-bg-elevated` / `--ui-bg-popover` | Menus, dialogs, approval cards |
| Widget | `--ui-bg-widget` | Inline panels inside a transcript (no border) |

Floating panels: `.float`. Menus and selects: `.menu`. Never invent a per-component shadow.

Layer order uses the `--z-*` ladder in `tokens/base.css`. Plain `z-index: 1` is fine inside one
component; anything that competes across the app picks a rung.

## Feedback states

| State | Primitive | Rule |
|---|---|---|
| Loading | `<curve-loader>` | Label it with what is happening ("Indexing 214 notes"). Never "Loading…". Don't show it for anything under 300ms |
| Empty | `.empty` | A display-serif title, one sentence on what goes here, the action that fills it |
| Error | `.error-state` | What went wrong and how to fix it. No apology. Details and logs behind a disclosure |
| Logs | `.log` | Monospace, hairline, no fill, capped height |
| Confirm | Confirm dialog | Focus lands on Confirm; Enter confirms, Esc cancels. Never `window.confirm` |
| Gate | `.approval` | The exact action, in monospace, before the buttons. See below |

**Red means failed.** Use `--ui-error` only for an explicit failure. An exit code of 1 with
useful output, a missing optional file, or an error described inside returned data is a
neutral notice.

### Gates and approvals

Four of these apps act on the user's behalf (chromeclaw, casca, maiku, horizon's execution
side). Approvals share one shape:

1. The **exact** action in monospace: the command, the button name, the URL, the file path.
2. Why it needs approval, in one line ("Cannot be undone", "Leaves this machine").
3. Choices as buttons. Deny is always available and always one keystroke (Esc).
4. Queued approvals stack behind the front card as one silhouette. They never auto-advance.
5. Gestures (swipe, drag) can dismiss a card. They never approve.

## Motion

- Controls: `--dur-fast` (100ms). Drawers and disclosure: `--dur-base` (180ms). Card promotion
  and panel slides: `--dur-slow` (220ms).
- Motion follows state; it never delays it. Selection, pressed states and cancel paint in the
  current frame.
- Animate named properties (`transform`, `opacity`, `background-color`). Never
  `transition: all` on anything hot.
- Tooltips: 200ms delay, instant within 300ms of the last one, no entrance animation, 100ms exit.
- `prefers-reduced-motion` sets every duration to zero. The loader draws one still frame.
- Pause animation in hidden windows and tabs.

## Keyboard

- The focused surface owns its keys. App shortcuts never steal from an editor, terminal or input.
- One Esc does one thing: cancel the active interaction, or close the topmost dismissable surface.
- Every app has a command palette on ⌘K / Ctrl+K that lists every action with its shortcut.
- Shortcut hints come from the binding table, never hardcoded strings.
- Focus is always visible (`:focus-visible`, accent outline, 1px offset).

## Icons

- **Tabler Icons** (MIT) for app chrome. **Codicons** (CC BY 4.0) for code, tool and status.
  No third set. Import through one module per app so the set can be swapped.
- 14px in controls, 12px at `xs`. Stroke 1.75.
- No decorative sparkles or stars. Brand moments use the project's glyph (see each project file).
- Emoji appear in two places only: TUI tool rows, and user content.

## Writing

- Name things by what people recognise ("Notifications", "Your key"), not by how they are built
  ("webhook config", "BYOK provider").
- Buttons say what happens: "Approve and run", "Save report". The result confirms it: "Saved".
- Errors say what went wrong and what to do next. No "Oops", no "Something went wrong".
- Numbers carry units. Time is relative under a day ("4 min ago"), absolute after.
- Sentence case everywhere except `.eyebrow` and `.section-heading`, which are uppercase by CSS.
- Kawaii faces live in the terminal spinner and empty states, and only in projects whose
  `kawaii` setting allows them. They never appear in errors, gates or anything about money,
  safety or another person.

## Accessibility

- Text meets 4.5:1; large text and UI marks meet 3:1. `node scripts/build.mjs` checks every
  skin and fails the build otherwise.
- Colour is never the only signal. Status dots pair with a word; diff lines pair with `+`/`−`.
- Every icon-only button has an `aria-label`. Tooltips add to labels; they don't replace them.
- Hit targets: 24px on desktop, 44px on touch (`--hit-touch`).
- Respect `prefers-reduced-motion`, `prefers-contrast` and the OS text size.

## Before you add something

- [ ] Reused a primitive from [COMPONENTS.md](COMPONENTS.md) instead of making a new one?
- [ ] Only `--ui-*` and other shared tokens; no hex, no `--seed-*`?
- [ ] No class overriding a primitive's padding, radius, colour or shadow?
- [ ] Flat: no card in a card, no unneeded dividers?
- [ ] Floating surfaces use `.float` or `.menu`, nothing else?
- [ ] Loading, empty and error states designed, with real copy?
- [ ] No navigation, focus change or pane opening triggered by a background event?
- [ ] Red only for explicit failures?
- [ ] Works with the keyboard alone, and Esc does exactly one thing?
- [ ] Looks right in both modes, and at the narrowest supported width?
- [ ] The CLI output for the same feature follows [TUI.md](TUI.md)?
- [ ] Changed a token or primitive? Its entry in these docs changed in the same commit.
