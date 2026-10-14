# Terminal

Every project ships a Rust binary, and in most of them the CLI arrives before any GUI. The
terminal is the first place each project's identity shows up, so it follows Hermes's TUI
grammar closely. `rust/theme.rs` implements all of it with no dependencies.

## Anatomy

```
███████╗███████╗███████╗ █████╗ ██╗        ← banner: 2 rows title, 2 accent, 2 border
...
sefai v0.3.0 · qwen3-8b · local · metal    ← dim: version, model, where it runs
──────────────────────────────────────────  ← input rule
❯ rainy day. talk?                          ← prompt symbol + user text
 ☾ sefai                                    ← response label, bold, response_border colour
  ┊ 🧠 recall    "rain", "walks" · 3 hits 0.2s   ← tool rows
  ⠧ ( ˘ω˘ ) remembering…                    ← spinner: frame, face, verb
  It's the kind of rain you said you like walking in…
 model qwen3-8b │ ctx 41% │ local │ 0 gates ← status bar
```

## Rules

**Banner.** Show it once per interactive session, never for one-shot commands (`casca doctor`,
`stockfih import`). Below `art width + 4` columns, collapse to one line: `{glyph} {agent}`.
Never print it when stdout is not a terminal.

**Metadata line.** `dim` colour, `·` separators, in this order: name and version, model, where
it runs (`local`, `your key: anthropic`, `none`), hardware or sync state. This line answers
"what will leave my machine" before the user types anything.

**Prompt.** `❯ ` in `prompt` colour with an `input_rule` line above it. Multi-line input
continues with `· ` aligned under the cursor.

**Response label.** ` {glyph} {agent} ` in bold `response_border`, on its own line, before each
agent turn.

**Tool rows.** `┊ {icon} {verb:9} {detail} {duration}`.
- The verb is a short present-tense word from a fixed list per project (`search`, `read`,
  `click`, `recall`, `probe`). Nine cells, left-aligned, so details line up.
- `detail` is the exact argument: the query in quotes, the path, the element name, the URL host.
- `duration` in `dim`. Failed rows colour only the verb with `error` and replace duration with
  the reason (`exit 101`, `denied`).
- Indent two spaces under the response label. Collapse a run of more than six into
  `┊ … 14 more (t to expand)` in interactive mode.

**Spinner.** Braille frame in `accent`, then a face in `label` (if the skin's kawaii setting
allows), then a verb in `dim` ending in `…`. Change the face and verb every 3 seconds, not every
frame. Frame interval 80–100ms. When stdout isn't a terminal, print `{verb}…` once and nothing
else until the result.

**Gates.** An approval in the terminal is a bordered block, the one place the TUI uses a box:

```
╭─ approve? ───────────────────────────────────────────────────╮
│ click  "Delete 14 files" in Finder                           │
│ cannot be undone                                             │
╰─ [y] approve once · [a] always for this task · [n/esc] deny ─╯
```
Border in `warn`. The action line in `text`, the reason in `dim`. Default is deny. Never accept
on Enter alone.

**Status bar.** One line, `status_bg` fill, segments separated by a `dim` `│`. Tones:
`strong` for the model, `good` / `warn` / `bad` / `critical` for context use (under 60 / 80 /
95 / over), pending gates and sync state. Show it in interactive sessions only.

**Errors.** `error: ` in `error`, then what failed and what to do, on one line. Then the cause
chain indented in `dim`. Then nothing else. No stack traces unless `-v`.

```
error: model file not found: ~/.sefai/models/qwen3-8b.gguf
  run `sefai model pull qwen3-8b`, or set model.path in ~/.sefai/config.toml
```

**Success.** One line starting with `ok ` in `ok` colour, or the project glyph. No celebration.

**Help.** `clap` help styled with the skin: headers in `title` bold, flags in `accent`,
placeholders in `label`. Map these to `clap::builder::Styles` from the generated palette.

## Colour detection

Implemented in `ColorLevel::detect` and `terminal_is_light`:

1. `NO_COLOR` set and non-empty: no colour at all.
2. `CLICOLOR_FORCE` set: colour even when piped.
3. stdout is not a terminal, or `TERM=dumb`: no colour.
4. `COLORTERM=truecolor|24bit`: 24-bit. Otherwise 256-colour, mapped to the nearest cube or grey.
5. Polarity: `<APP>_THEME=light|dark`, then `COLORFGBG`, then Apple Terminal's light default,
   else dark. Light terminals get the skin's tuned `light` palette, never an inversion.

Every project also accepts `--color=auto|always|never`, which overrides 1–3.

## Machine output

Anything a script might read gets `--json` (one object) or `--jsonl` (one per event). Machine
output never contains colour, spinners, banners or kawaii, whatever the flags. Human output is
not a stable interface; JSON is.

## Using `rust/theme.rs`

`kit` installs three files in `design/rust/`: `theme.rs` (this module), `palettes.rs` (every
skin) and `identity.rs` (this project's name, glyph, faces, verbs and banner). Include them as
one module:

```rust
// src/term/mod.rs (rustfmt keeps these in alphabetical order)
#[path = "../../design/rust/identity.rs"]
pub mod identity;
#[path = "../../design/rust/palettes.rs"]
pub mod palettes;
#[path = "../../design/rust/theme.rs"]
pub mod theme;
```

```rust
use term::{identity::SEFAI, palettes, theme::{Term, Tone, terminal_width}};

let t = Term::new(&palettes::CANDLE, &SEFAI, "SEFAI");
print!("{}", t.banner(terminal_width(), env!("CARGO_PKG_VERSION")));
println!("{}", t.tool_row("🧠", "recall", "\"rain\" · 3 hits", Some("0.2s"), false));
eprintln!("{}", t.status_bar(80, &[("qwen3-8b", Tone::Strong), ("ctx 41%", Tone::Good)]));
```

In a Cargo workspace, count the `../` from the crate's `src/term/` up to the repo root
(`crates/sefai/src/term/` needs `../../../../design/rust/theme.rs`).

`palettes::by_name("candle")` looks a skin up by the name in `design/kit.json`, so a config
setting can switch it. All three files carry `#![allow(dead_code)]`, so a CLI that uses a
subset still passes `clippy -D warnings`.

For a full-screen interface (reasontrace's `rt watch`, horizon's simulation monitor), use
`ratatui` and convert `Rgb` to `ratatui::style::Color::Rgb`. The layout rules above still hold:
banner in the empty state, status bar at the bottom, tool rows in the log pane.

## Previewing in Hermes

`kit` writes `design/hermes.yaml`: the project's identity wearing its chosen skin, as a
valid Hermes skin (the master keeps one per project in `hermes/`). If you have Hermes:

```sh
cp design/hermes.yaml ~/.hermes/skins/sefai-candle.yaml
hermes skin use sefai-candle
```

The file name must match the `name:` line at its top. The TUI, CLI and desktop app repaint
with your project's palette, banner, faces and verbs.
