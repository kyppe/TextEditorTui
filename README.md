# TextPoppup

A fast-starting, Vim-flavored terminal journal. Hit a key, jot a
thought, get out of the way — every entry keeps its full edit history,
and you can format text (italic, bold, …) inline without leaving the
keyboard.

Written in Rust for startup speed: the release binary launches in a
few milliseconds and does one disk read on start, one disk write per
save. There is no background daemon and no network access.

## Installation

You need the Rust toolchain (`rustc`/`cargo`) — install it with
[rustup](https://rustup.rs) or your OS package manager.

```bash
cargo build --release
```

The binary is at `target/release/textpoppup`. Put it on your `PATH`,
e.g.:

```bash
install -Dm755 target/release/textpoppup ~/.local/bin/textpoppup
```

## Running

```bash
textpoppup
```

That's the whole interface — no subcommands or flags. Press `?` inside
the app for a keybinding/command cheat sheet at any time.

### Wiring it to a hotkey

TextPoppup is a terminal app, so a system-wide hotkey has to be bound
by your window manager or desktop environment to *open a terminal
running textpoppup*, not by the app itself. Examples:

- **sway / i3**: in your config, `bindsym $mod+t exec alacritty -e textpoppup`
  (swap `alacritty` for your terminal of choice).
- **GNOME / KDE**: create a custom keyboard shortcut whose command is
  your terminal emulator with `-e textpoppup` (flag name varies by
  terminal, e.g. `--` for gnome-terminal, `-e` for kitty/alacritty/foot).
- **macOS**: use an Automator "Run Shell Script" quick action bound to
  a hotkey via System Settings, or a launcher like Raycast/Alfred
  pointed at your terminal with the same `-e textpoppup` pattern.

## Basic usage

TextPoppup opens into the **journal view**: every entry you've written,
oldest first, each showing its ID, creation timestamp, and current
text.

The bottom-left corner always shows which mode you're in, and therefore
which keys are live: `JOURNAL` (browsing the list), `NORMAL` / `INSERT` /
`VISUAL` (in the editor), plus `COMMAND`, `HISTORY`, `TITLE`, `HELP` and
`CONFIRM`. The rest of that line is a hint for the current mode, or the
last status message.

- Press `n` to write a new entry. Type, then `Ctrl+S` to save.
- Press `j`/`k` (or arrows) to move between entries, `i`/`Enter` to
  edit the selected one — editing never overwrites the old text, it
  creates a new version (see below).
- Press `d` to delete the selected entry (asks for confirmation).
- Press `H` to view an entry's full version history.
- Press `:` at any time for a command line.

### Titling an entry

From the journal list, select an entry and press **`t`**. A small popup
opens: if the entry already has a title it's shown there ready to edit,
otherwise you get an empty field to name it. `Enter` saves, `Esc`
cancels, and submitting an empty field removes the title.

While *editing* an entry you can also use `:title Groceries` — that sets
the draft's title, saved along with the text on `Ctrl+S`.

Either way the title shows up in the journal list next to the ID and
date, and in the editor's border. Titles are **versioned**: retitling
creates a new version and old versions keep the title they had, so
`:history` shows you what each version was called.

For a heading *inside* an entry's text — as opposed to the entry's own
title — select the text and use `:head` (see below).

### Crossing things off

Put the cursor on a line and run `:done` to cross it out; `:undone`
removes the cross-out. Neither needs a selection — with none, they act on
the cursor's line, which is the usual case for a todo list. If you *do*
have a selection (say from `V`), they act on that instead. `:done` twice
is harmless; it says "Already done" rather than stacking marks.

```
buy milk
walk the dog
c̶a̶l̶l̶ ̶t̶h̶e̶ ̶b̶a̶n̶k̶      ← :done
fix the sink
```

Like all formatting, this lives on the draft until `Ctrl+S` saves it as a
new version.

### Formatting text

While editing an entry, press `Esc` to leave typing mode, move the
cursor to where you want the selection to start, press `v` (or `V` to
grab the whole line), move the cursor to extend the selection, then run
a format command:

```
:i     italic the selection
:b     bold
:u     underline
:code   code style
:hl     highlight
:head   heading (a title for a run of text inside the entry)
:strike crossed out (`:done`/`:undone` are the line-aware version)
```

Running the same command again on the same exact selection removes the
mark. Formatting is stored as data (start/end + type) alongside the
text, not baked into it — see `FEATURES.md` if you want to add more
formatting types.

### Entry history

Every timestamped entry has a unique ID (shown as `[ID: xxxxxx]`).
Editing an entry never destroys older content — it appends a new
version. `Ctrl+S` mid-edit only creates a new version if the text or
formatting actually changed; if you Ctrl+S with nothing changed, you'll
see "No changes" and no version is added.

Open an entry's history with `H` (from the journal view) or
`:history <id>` from anywhere. You'll see one tab per version:

```
┌ History — f71d92   READ ONLY ──────────────────────────┐
│ [1] 17:15:19  [2] 18:10:39  [3] 19:42:12  [4] 09:15:44 │
│Version 4 / 4   13/04/2026 09:15:44                     │
│                                                         │
│ ...content of the selected version...                  │
└─────────────────────────────────────────────────────────┘
```

Switch versions with `h`/`l`, `←`/`→`, `Tab`/`Shift+Tab`, a mouse click
on a tab, or `Ctrl+1`–`Ctrl+9` to jump straight to version 1–9 (works
on terminals that support the
[Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) —
kitty, wezterm, foot, recent alacritty; on other terminals there's no
byte sequence to distinguish `Ctrl+1` from a plain `1`, so use `h`/`l`
or `Tab` there — they always work).

Restore an old version with `:restore <n>` while its history is open.
This does **not** delete anything — it appends a brand-new version
whose content matches version `n`, so the full history (including the
version you were on before restoring) is preserved.

Press `Esc` or `q` to close the history view — it's read-only, there's
no way to accidentally edit an old version.

## Keyboard shortcuts

**Journal view (Normal mode)**

| Key | Action |
|---|---|
| `j`/`k`, `↑`/`↓` | Move selection |
| `gg` / `G` | Jump to first / last entry |
| `n`, `o` | New entry |
| `i`, `Enter` | Edit selected entry |
| `t` | Title / retitle selected entry (popup) |
| `d` | Delete selected entry (confirms) |
| `H` | Open history for selected entry |
| `:` | Command line |
| `?` | Help |
| `q` | Quit |

**Editor**

Line numbers are shown in a gutter; soft-wrapped continuation rows are
left unnumbered, and the cursor's line number is highlighted.

| Key | Action |
|---|---|
| `i`, `a` | Start typing (insert / append) |
| `I` / `A` | Start typing at line start / line end |
| `o` / `O` | Open a new line below / above and start typing |
| `Esc` | Stop typing → clear selection → exit editor (unsaved changes discarded) |
| `h`/`l`/`j`/`k`, arrows | Move cursor (arrows work while typing too) |
| `w` / `b` / `e` | Next word / previous word / end of word |
| `gg` / `G` | Start / last line of the entry |
| `0` / `^` / `$` | Line start / first non-blank / line end |
| `x` | Delete the character under the cursor |
| `dd` / `D` | Delete the line / delete to end of line |
| `v` | Toggle selection at cursor |
| `V` | Select the whole current line |
| `H` | Open history for the entry being edited |
| `Ctrl+S` | Save (new entry, or new version of an existing one) and exit |
| `:` | Command line (selection is preserved) |

Words are whitespace-delimited (Vim's `W`/`B`/`E` behaviour). **Counts and
operator-pending combos are not implemented** — `5j`, `d3w`, `ciw` and
friends do nothing; `dd`/`D`/`x` are the available deletions.

**History view**

| Key | Action |
|---|---|
| `h`/`l`, `←`/`→`, `Tab`/`Shift+Tab` | Switch version |
| `gg` / `G` | Jump to first / latest version |
| `Ctrl+1`–`Ctrl+9` | Jump to version 1–9 (terminal-dependent, see above) |
| mouse click on a tab | Jump to that version |
| `:` | Command line (for `:restore`) |
| `Esc`, `q` | Close (returns to your draft if you opened history mid-edit) |

With more versions than fit across the window, the tab bar scrolls to keep
the selected version visible; `‹` / `›` mark versions hidden off each edge.

**Help popup** (`?`): `j`/`k` scroll it when it's taller than the window,
any other key closes it.

## Commands

| Command | Aliases | Effect |
|---|---|---|
| `:new` | `:n` | Start composing a new entry |
| `:title <text>` | `:name` | Title the entry being edited (no text clears it) |
| `:done` / `:undone` | | Cross out / un-cross the selection, or the current line |
| `:history [id]` | `:hist` | Open version history (current entry if `id` omitted) |
| `:restore <n>` | | Restore version `n` as a new version |
| `:delete` | `:d` | Delete the selected entry |
| `:quit` | `:q` | Exit |
| `:help` | `:h` | Show the help popup |
| `:i` / `:b` / `:u` / `:code` / `:hl` / `:head` | | Toggle italic/bold/underline/code/highlight/heading on the current selection |

## Storage location

Everything is one JSON file:

- Linux: `~/.local/share/textpoppup/entries.json`
- macOS: `~/Library/Application Support/textpoppup/entries.json`
- Windows: `%APPDATA%\textpoppup\entries.json`

(Whatever [`dirs::data_dir()`](https://docs.rs/dirs) resolves to on
your platform, joined with `textpoppup/`.) Writes go through a
temp-file-then-rename so a crash mid-save can't corrupt or truncate
your journal. The file is plain JSON — safe to back up, `git`-track, or
inspect by hand.

## Extending TextPoppup

See `FEATURES.md` for how to add a new command, formatting type,
keyboard shortcut, or persisted field with minimal, localized changes,
and `ARCHITECTURE.md` for how the pieces fit together.
