# Adding features to TextPoppup

This file exists so that adding the features listed below stays a
small, localized change — a couple of files, not a tour of the whole
codebase. If you find yourself editing more files than the recipe
below says, something has drifted from the intended architecture; fix
the abstraction, don't just push through it. Read `ARCHITECTURE.md`
first if you haven't — it explains *why* the pieces below are split
the way they are.

The four extension points, and where they live:

| Concept | Lives in | Purpose |
|---|---|---|
| Command registry | `src/command.rs` | `:word` → behavior |
| Formatting/mark registry | `src/format.rs` (+ `MarkKind` in `src/entry.rs`) | mark type → command name + render style |
| Editor actions | `src/action.rs` | the actual state mutations; shared by keys and commands |
| Keybinding table | `src/keybind.rs` | key press → `Action` |

---

## Add a new command

Example: adding `:pin` to pin the selected entry to the top (hypothetical).

1. Open `src/command.rs`.
2. Add an entry to the `COMMANDS` array:

   ```rust
   CommandDef {
       name: "pin",
       aliases: &[],
       help: ":pin — pin the selected entry",
       run: |app, _args| {
           // your logic, or delegate to a new fn in action.rs
           Ok(())
       },
   },
   ```

That's it — the parser in `command::execute`, the `:` key handling in
`keybind.rs`, and the `:help` popup (which lists `COMMANDS`
automatically) all pick it up with no further changes.

If the command needs more than a couple of lines of logic, write it as
a `pub fn` in `src/action.rs` instead (see "Add a new editor action"
below) and have `run` call it — that keeps `command.rs` a thin table
and makes the logic reusable from a keybinding too.

**Applying a mark over a range?** Use `Formatting::set` / `clear` /
`covers` (`src/entry.rs`) rather than pushing onto `marks` directly —
they merge, trim and split overlapping runs, so repeating a command is
idempotent. `action::set_done` (`:done`/`:undone`) is the worked example;
`toggle_format` is the older exact-range toggle used by `:i` and friends.

**If the command only makes sense in one mode, guard it.** Commands can be
typed from anywhere, so check `app.mode` and return an `Err` rather than
silently acting on the wrong thing — the way `action::toggle_format`
requires `Mode::Editor` and `restore_version` requires `Mode::History`:

```rust
if !matches!(app.mode, Mode::Editor) {
    return Err("Only works while editing an entry".into());
}
```

## Add a command alias

One file, one line. In `src/command.rs`, add the alias string to the
existing command's `aliases` slice:

```rust
CommandDef {
    name: "history",
    aliases: &["hist", "log"],  // added "log"
    ...
```

## Add a new formatting type

**Worked example already in the tree:** `MarkKind::Heading` (`:head`) was
added exactly this way and touched only these two files — grep for
`Heading` to see the whole change. Here's the same recipe for a new one,
say `Strikethrough`:

1. **`src/entry.rs`** — add a variant to `MarkKind`:

   ```rust
   pub enum MarkKind {
       Italic,
       Bold,
       Underline,
       Code,
       Highlight,
       Strikethrough, // new
   }
   ```

2. **`src/format.rs`** — add one entry to `FORMATS`:

   ```rust
   FormatDef {
       kind: MarkKind::Strikethrough,
       command: "strike",
       name: "strikethrough",
       style: || Style::default().add_modifier(Modifier::CROSSED_OUT),
   },
   ```

Done. No other file changes:

- `command::execute` already resolves unknown command names against
  `format::find_by_command`, so `:strike` works immediately.
- `text::render_lines` / `render_lines_sel` — the single renderer behind
  all three views — looks up style via `format::find_by_kind`, so the new
  mark renders in the journal, the editor, and every history version.
- `:help` lists it automatically (it iterates `FORMATS`).
- `action::toggle_format` is generic over `MarkKind`, so selecting text
  and running `:strike` toggles it exactly like `:i` does.

## Add a new keyboard shortcut

All shortcuts are defined in one function: `keybind::resolve`. Find the
`match` arm for the relevant `ModeKind` (`Normal`, `EditorNormal`,
`EditorTyping`, `Command`, `History`, `Confirm`, `Help`) and add a line
mapping a `KeyCode` to an `Action`:

```rust
ModeKind::Normal => match key.code {
    ...
    KeyCode::Char('/') => Action::Search, // hypothetical new shortcut
    ...
```

If `Action::Search` doesn't exist yet, add it to the `Action` enum at
the top of `src/keybind.rs`, then handle it in `action::apply`'s
`match` in `src/action.rs` (see next section). Two files for a
shortcut that does something new; one file (`keybind.rs` only) if
you're just rebinding an existing `Action` to a different key.

### Multi-key chords (`gg`, `dd`)

`resolve` takes the in-progress chord as its `pending` argument, so chords
live in the same table as everything else. Emit `Action::SetPending('g')`
for the first key, then match the completion at the top of `resolve`:

```rust
(ModeKind::EditorNormal, 'g', KeyCode::Char('g')) => Action::EditorBufferStart,
```

`action::apply` stores `SetPending` on `App::pending_key` and clears it on
any other action, so an abandoned chord (`g` then `$`) just runs the second
key normally. Don't add a second pending-state mechanism elsewhere.

**Counts (`5j`, `d3w`) are deliberately not implemented.** Adding them
means an accumulator plus an operator-pending model threaded through every
motion — a real change, not a tweak. Plan for that rather than bolting a
count onto one or two keys.

## Add a new editor action

Actions are the layer both keybindings and commands funnel through, so
a behavior only needs to be written once.

1. Add a variant to `Action` in `src/keybind.rs`.
2. Handle it in the big `match` in `action::apply` (`src/action.rs`),
   either inline or by calling a new `fn` you add lower in that file.
3. Wire it up from wherever it should be triggered: a `keybind::resolve`
   arm (see above), a `CommandDef` in `command.rs`, or both — that's
   the point of the shared `Action` layer.

## Add a new UI popup

Follow the pattern `Mode::Confirm` and `Mode::Help` already use:

1. Add a variant to `Mode` in `src/app.rs` that carries whatever data
   the popup needs, plus a `return_to: Box<Mode>` so it knows what to
   restore when it closes.
2. Add a matching variant to `ModeKind` and a case in
   `App::mode_kind()` (same file).
3. Write the render function in `src/ui/popup.rs` (or a new module next
   to it, for something bigger than a message box) — follow
   `popup::confirm` for a small centered box using the `centered()`
   helper, or `popup::help` for a larger scrollable one.
4. Call it from the `match &app.mode` at the bottom of
   `ui::draw` in `src/ui/mod.rs`.
5. Handle its keys: add a `ModeKind::YourPopup` arm in
   `keybind::resolve`, and the `Action`s it produces in `action::apply`.

Five small edits, each in the module that already owns that concern —
no changes to the journal, editor, or history views.

## Add a new history-related feature

History-specific behavior (anything only meaningful while
`Mode::History { .. }` is active) goes in three places:

- **Data**, if needed: `Version` in `src/entry.rs` already carries
  `version_number`, `created_at`, `text`, `formatting`, and
  `restored_from`. Add fields there if a feature needs more per-version
  data (see "Add a new persistent data field" below).
- **Behavior**: a `pub fn` in `src/action.rs` near `open_history`,
  `history_goto`, and `restore_version` — these are the existing
  examples to copy. They all read/write `Mode::History { entry_id,
  selected, return_to }` and `app.store`. Preserve `return_to` when you
  rebuild the mode: it's what lets Esc put the user back in the draft
  they were editing when they opened history.
- **Keys/commands**: a `ModeKind::History` arm in `keybind.rs`, and/or a
  `CommandDef` in `command.rs` guarded the way `restore` is (it returns
  an `Err` if `app.mode` isn't `Mode::History`).
- **Rendering**: `src/ui/history.rs` — it's the only file that draws
  the tab bar and version content, including the mouse hit-test rects
  in `App::history_tab_cols`/`history_tab_row`.

## Add a new persistent data field

**Worked example already in the tree:** `Version::title` (set by `:title`).
Note where it lives — on `Version`, not `Entry` — because it's content that
changes over time, so it has to be versioned like the text: retitling makes
a new version and old versions keep their old title. Put a field on `Entry`
only if it describes the entry for all time (like `id`). Whatever you add,
extend the "unchanged" comparison in `action::save_editor`, or a change to
your field alone will be silently discarded on save.

Everything on disk is one JSON file (`src/store.rs`), serialized
straight from the structs in `src/entry.rs` via `serde`. To add a field
— say, a `tags: Vec<String>` on `Entry`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub created_at: DateTime<Local>,
    pub last_edited_at: DateTime<Local>,
    pub versions: Vec<Version>,
    #[serde(default)]
    pub tags: Vec<String>, // new
}
```

The `#[serde(default)]` matters: it's what lets `entries.json` files
written by an older build of TextPoppup keep loading after you add a
field — see `Store::load`, which falls back to `Store::default()` only
on a parse *failure*, not on a missing field, so `serde(default)` is
what actually protects existing data. Update `Entry::new` to initialize
it, then read/write it from wherever the feature needs it (a new
command in `command.rs`, a new action in `action.rs`, a render tweak in
`ui/`).

The same pattern applies to `Version` (e.g. per-version metadata) or
`Store` (e.g. app-wide settings) — add the field with `#[serde(default)]`,
update the constructor, wire up the behavior.

## Add a new editor mode

The spec's "editor mode" would be something like `Mode::Normal` or
`Mode::Editor` but with different keybindings/rendering for a
different purpose (e.g. a "search results" mode). Same shape as adding
a popup (above), but as a full-screen view rather than an overlay:

1. `Mode` variant in `src/app.rs`, `ModeKind` variant, `mode_kind()` case.
2. A new `src/ui/your_mode.rs` with a `draw(frame, app, area)` function,
   called from the `match base_mode(&app.mode).clone()` in
   `ui::draw` (`src/ui/mod.rs`).
3. A `ModeKind::YourMode` arm in `keybind::resolve` for its keys.
4. Whatever `Action`s it needs, handled in `action::apply`.
