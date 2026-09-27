# Architecture

This describes how TextPoppup's pieces fit together and why they're
split the way they are. Read `FEATURES.md` for the "I want to add X"
recipes that depend on understanding this.

## Data flow

```
Keyboard / Mouse (crossterm::event::read)
              │
              ▼
   keybind::resolve(ModeKind, KeyEvent) -> Action        [src/keybind.rs]
              │                                     (mouse clicks are
              ▼                                      hit-tested directly
     action::apply(&mut App, Action)                 in main.rs, see below)
              │                                            │
              ▼                                            │
      App state mutated: Store, EditorState, Mode  ◄───────┘
       [src/app.rs, src/entry.rs, src/store.rs]
              │
              ▼
        ui::draw(&mut Frame, &mut App)             [src/ui/*.rs]
              │
              ▼
           Terminal (ratatui)

  Store::save() ──► temp file ──► rename ──► entries.json on disk
       [src/store.rs]                        (only on explicit save,
                                               delete, or restore)
```

A `:command` line takes a side door into the same place: `Mode::Command`
captures keystrokes into `app.command_input` until Enter, then
`command::execute` (`src/command.rs`) parses it and either runs a
`CommandDef` from its table or — for formatting commands like `:i` —
looks it up in `format::FORMATS` and calls
`action::toggle_format`. Either way, commands end up calling the same
`action.rs` functions a keybinding would. **`action.rs` is the only
place `App` state actually changes** — this is deliberate, so a
command and a shortcut that do the same thing can't drift apart into
two slightly different behaviors.

## Module responsibilities

- **`src/main.rs`** — terminal setup/teardown (raw mode, alternate
  screen, mouse capture, and — where the terminal supports it — the
  Kitty keyboard protocol so `Ctrl+1..9` can be distinguished from
  plain digits) and the event loop. The only place that reads
  `crossterm::event::Event` directly; key events go to
  `keybind::resolve`, mouse events are hit-tested against
  `App::history_tab_cols` inline (the only current mouse interaction).

- **`src/app.rs`** — `App`, the single source of truth: the `Store`, the
  current `Mode`, the in-progress `EditorState`, status messages. Also
  defines `Mode` (what's currently on screen / being edited) and the
  coarser `ModeKind` that `keybind::resolve` switches on. The
  `Mode` → `ModeKind` split exists so that adding a `Mode` variant that
  *behaves* like an existing one (e.g. another overlay like `Confirm`)
  doesn't require new keybinding logic — see `App::mode_kind()`.

- **`src/entry.rs`** — the persisted data model: `Entry`, `Version`,
  `Mark`, `MarkKind`, `Formatting`. This is the one file with no
  dependency on the terminal, rendering, or input handling, so its
  logic (version history, restore) is covered by ordinary `#[test]`s —
  run `cargo test`.

- **`src/store.rs`** — loads/saves the whole journal as one JSON file
  via `serde_json`, atomically (write to `.tmp`, then `rename`). Knows
  nothing about `App`, `Mode`, or rendering — it only operates on
  `Vec<Entry>`.

- **`src/id.rs`** — random 6-hex-char ID generation; `Store::unique_id`
  is what actually guarantees no collision with existing entries.

- **`src/format.rs`** — the formatting registry: for each `MarkKind`,
  its `:command` name, display name, and ratatui `Style`. This table is
  read by three independent things (`command::execute`,
  `text::render_lines`, `ui/popup.rs`'s help listing), which is exactly
  why it's a flat data table rather than logic scattered across those
  three call sites — see `FEATURES.md`.

- **`src/text.rs`** — character-index text editing primitives
  (`insert_char`, `remove_before`, cursor row/col math), soft wrapping,
  and `render_lines` / `render_lines_sel`: the one function that turns
  `(text, marks, width)` into styled, wrapped `ratatui::text::Line`s.
  All three views (`ui/normal.rs`, `ui/history.rs`, `ui/editor.rs`) call
  it, so a new `MarkKind` renders correctly everywhere without any of
  them changing. The editor passes a selection range to the `_sel`
  variant; selection is handled *inside* the renderer rather than by
  post-processing its output, so a highlight can't land on the wrong
  characters when text wraps.

  **One wrap calculation, two consumers.** `wrap_positions` decides where
  soft breaks fall; `render_lines_sel` draws them and `visual_row_col`
  places the terminal cursor from the same function's output. Anything
  that needs to know "where does this character appear on screen" must go
  through `visual_row_col`, never re-derive it — the two drifting apart
  is exactly how a cursor ends up sitting on the wrong character. The
  test `cursor_position_agrees_with_rendered_wrapping` walks every
  character offset in a multi-line buffer and asserts the two agree.

  Note the deliberate pair: `row_col` is about **logical** lines (what
  `j`/`k` move between, what `0`/`$` act on), `visual_row_col` is about
  **visual** rows after wrapping (where the cursor is drawn). Don't
  substitute one for the other.

  **One character is one terminal column.** Everything above depends on it,
  so `render_lines` substitutes a space for any literal tab: left alone, the
  terminal would jump to its next tab stop and every column after it —
  wrap points, cursor, mark boundaries — would be off. `Tab` in insert mode
  inserts spaces for the same reason. The stored text keeps whatever it had,
  so nothing is rewritten behind the user's back.

  `trimmed_line_spans` is the other half of that: it turns a selected range
  into per-line, whitespace-trimmed ranges so a mark covers words rather
  than the blank margin around them. Mark-applying actions go through
  `action::target_spans`, which wraps it.

  Positions are **character offsets**, not byte offsets — chosen so
  cursor/selection/mark arithmetic can't produce a UTF-8 boundary panic.
  The one thing this doesn't handle is wide characters (CJK, emoji)
  rendering as more than one terminal column; documented in `text.rs`
  rather than hidden, since fixing it properly means teaching the
  editor about display width, not just character count.

- **`src/keybind.rs`** — `Action`, the enum naming every possible
  user-triggered effect, and
  `resolve(ModeKind, KeyEvent, pending) -> Action`, the single match
  statement mapping keys to them per mode. This is the entire keybinding
  table; see `FEATURES.md`.

  The editor is three `ModeKind`s, not one — `EditorNormal`,
  `EditorVisual`, `EditorTyping` — so "what does `d` mean" is answered by
  the table rather than by `if` statements inside the actions. Visual
  falls through to the normal arm for anything it doesn't override, which
  is how motions stay defined once. `pending` carries a half-typed chord
  (`gg`, `dd`, `cw`, `r<char>`); see FEATURES.md "Multi-key chords".

- **`src/browser.rs`** — hands a link to the desktop's opener for `gx`.
  Same shape as the clipboard bridge: shell out to `xdg-open`/`open`/`gio`,
  URL passed as a single `Command` argument so nothing in it can be read as
  a command. `normalize` is the gatekeeper — it https-prefixes a bare
  domain but refuses any scheme outside http/https/mailto, because a
  journal file is just JSON that could have come from anywhere.

- **`src/clipboard.rs`** — the yank register and the system-clipboard
  bridge. Yanks and deletes fill both; paste prefers the system clipboard
  when it holds something newer, so copy/paste works across applications
  (Vim's `clipboard=unnamedplus`). It shells out to `wl-copy`/`wl-paste`,
  `xclip`, `xsel` or `pbcopy`/`pbpaste` — whichever exists — instead of
  linking a clipboard crate, and degrades to an in-app-only register when
  none is installed. `Register::linewise` is what makes `dd`+`p` restore a
  whole line while `y`-over-a-selection pastes inline.

- **`src/action.rs`** — `apply(&mut App, Action)` plus the `pub fn`s it
  delegates to (`save_editor`, `open_history`, `restore_version`,
  `toggle_format`, …). This is where `App` state is actually mutated —
  see "Data flow" above for why that's centralized here.

  **Commands must check the mode they need.** A `:command` can be typed in
  any mode, so anything that only makes sense somewhere specific guards on
  `app.mode` and returns an `Err`: `toggle_format` requires `Mode::Editor`
  (otherwise it would mark a buffer nothing will save and report success),
  `restore_version` requires `Mode::History`, and deleting requires
  `Mode::Normal` (it targets the *journal* selection, which is not
  necessarily the entry open in the editor). Add the same guard to any new
  mode-specific command.

- **`src/command.rs`** — the `:command` registry (`COMMANDS`) and
  `execute`, the parser. Falls through to `format::find_by_command` for
  formatting commands, so those don't need a `CommandDef` each.

- **`src/ui/`** — rendering only; no module here mutates `App` (they
  take `&App`, except `ui/history.rs::draw`, which takes `&mut App`
  solely to record `history_tab_cols` for mouse hit-testing — a
  read-mostly exception, not a precedent for putting logic in `ui/`).
  - `mod.rs` — dispatches on `Mode` via `base_mode` (which unwraps
    overlay modes like `Command`/`Confirm`/`Help` to find what's
    underneath) and draws the status/hint line.
  - `normal.rs` — the journal (entry list) view.
  - `editor.rs` — the compose/edit view, cursor, and selection.
  - `history.rs` — the read-only version-tabs view.
  - `popup.rs` — floating overlays (`Confirm`, `Help`).

## Why history is a first-class model, not a diff log

`Entry` never stores "current text" — only `versions: Vec<Version>`,
each a complete, immutable snapshot (text *and* formatting together, so
a future formatting type is automatically covered by history without
extra work). `Entry::push_version` and `Entry::restore` are the only
two ways `versions` ever grows, and both only ever `push` — nothing in
this codebase mutates or removes an element of `versions`. That
invariant is what makes "restoring version 2 creates version 5 instead
of deleting versions 3–4" true by construction rather than by careful
bookkeeping at each call site. See the tests in `entry.rs` for the
behavior this guarantees.

## Why formatting is data, not markup in the text

A `Mark` is `{ start, end, kind }` over character offsets into a
`Version`'s plain `text` — the text itself never contains formatting
syntax. This is what lets `toggle_format` be generic over every
`MarkKind` (`action.rs`), what lets `text::render_lines` render any
mark type without knowing the specific set of types in advance beyond
looking them up in `format::FORMATS`, and what keeps `insert_char`/
`remove_before` simple: they only need to shift mark boundaries
(`action::shift_marks_for_insert`/`shift_marks_for_delete`), never parse
or re-escape anything.

## Testing

`cargo test` runs unit tests for the two modules with logic worth
testing without a terminal: `entry.rs` (version history, restore
semantics) and `text.rs` (char-index safety with multi-byte UTF-8,
mark-aware line splitting). There's no automated test for the
`ratatui`/`crossterm` rendering and input loop — that's exercised by
running the binary.
