# TextPoppup

A fast-starting, Vim-flavored terminal journal. Hit a key, jot a
thought, get out of the way — every entry keeps its full edit history,
and you can format text (italic, bold, …) inline without leaving the
keyboard.

Written in Rust for startup speed: the release binary launches in a
few milliseconds and does one disk read on start, one disk write per
save. There is no background daemon and no network access.

## Installation

**Prebuilt binary (Linux x86_64)** — download the latest tarball from the
[Releases page](../../releases/latest):

```bash
tar -xzf textpoppup-*-x86_64-linux.tar.gz
cd textpoppup-*-x86_64-linux
install -Dm755 textpoppup ~/.local/bin/textpoppup
```

**From source** — needs the Rust toolchain ([rustup](https://rustup.rs) or
your package manager):

```bash
./install.sh            # builds --release, installs to ~/.local/bin
```

or by hand:

```bash
cargo build --release
install -Dm755 target/release/textpoppup ~/.local/bin/textpoppup
```

Optionally install a clipboard tool so yank/paste is shared with other
applications: `wl-clipboard` on Wayland, `xclip` or `xsel` on X11.

📖 **[INSTALL.md](INSTALL.md)** has the full walkthrough: hotkey setup for
Hyprland / sway / i3 / GNOME / KDE / macOS, floating-popup window rules,
backups, uninstall, and troubleshooting.

## Running

```bash
textpoppup
```

That's the whole interface — no subcommands or flags. Press `?` inside
the app for a keybinding/command cheat sheet at any time.

### Wiring it to a hotkey

TextPoppup is a terminal app, so a system-wide hotkey is bound by your
window manager or desktop, launching a terminal that runs `textpoppup`.
This binding opens it on first press and dismisses it on second press:

```ini
# Hyprland — ~/.config/hypr/hyprland.conf
bind = SUPER, N, exec, pkill -x textpoppup || kitty --class textpoppup -e textpoppup
```

```
# sway / i3
bindsym $mod+n exec pkill -x textpoppup || kitty --class textpoppup -e textpoppup
```

See **[INSTALL.md](INSTALL.md#configure-a-hotkey)** for GNOME, KDE, macOS,
and for the window rules that make it a floating, centred, always-on-top
popup rather than an ordinary terminal window.

## Basic usage

TextPoppup opens into the **journal view**: every entry you've written,
oldest first, each showing its ID, creation timestamp, and current
text.

The bottom-left corner always shows which mode you're in, and therefore
which keys are live: `JOURNAL` (browsing the list), `NORMAL` / `INSERT` /
`VISUAL` (in the editor), plus `COMMAND`, `HISTORY`, `TITLE`, `HELP` and
`CONFIRM`. The rest of that line is a hint for the current mode, or the
last status message.

The **cursor shape** follows suit, like Vim: a thin bar while you're typing
in INSERT, a solid block otherwise. NORMAL and VISUAL share the block — the
shape tells you whether you're typing, the badge names the exact mode. Your
shell's own cursor is restored when TextPoppup exits.

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

**Blank space never gets marked.** Every effect runs from the first word to
the last, so indentation and trailing spaces stay clean — a strikethrough
or underline on an indented line doesn't drag a line through the empty
margin:

```
    b̶u̶y̶ ̶m̶i̶l̶k̶          ← :done on "    buy milk   "
```

Over a multi-line selection each line is handled separately, so the gap
between lines isn't marked either.

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
| `gg` / `G`, `Home` / `End` | Jump to first / last entry |
| `Page Up` / `Page Down` | Move 5 entries |
| `n`, `o` | New entry |
| `i`, `Enter` | Edit selected entry |
| `t` | Title / retitle selected entry (popup) |
| `d` | Delete selected entry (confirms) |
| `H` | Open history for selected entry |
| `:` | Command line |
| `?` | Help |
| `q` | Quit |

**Editor**

The editor opens in **NORMAL** mode, like Vim — press `i` to start typing.
Line numbers are shown in a gutter; soft-wrapped continuation rows are
left unnumbered, and the cursor's line number is highlighted.

*Entering insert mode*

| Key | Action |
|---|---|
| `i` / `a` | Insert before / after the cursor |
| `I` / `A` | Insert at line start / line end |
| `o` / `O` | Open a new line below / above |
| `cw` / `cc` / `C` | Change word / line / to end of line |
| `Esc` | Back to NORMAL mode |

*Moving (NORMAL / VISUAL)*

| Key | Action |
|---|---|
| `h`/`l`/`j`/`k` | Move cursor |
| `w` / `b` / `e` | Next word / previous word / end of word |
| `gg` / `G` | First / last line of the entry |
| `0` / `^` / `$` | Line start / first non-blank / line end |

*Ordinary keyboard keys* — these work in **every** editor mode, including
while you're typing, so you don't have to leave insert mode to jump around:

| Key | Action |
|---|---|
| arrows | Move cursor |
| `Home` / `End` (`Orig` / `Fin`) | Line start / line end |
| `Ctrl+Home` / `Ctrl+End` | Start / end of the whole entry |
| `Ctrl+←` / `Ctrl+→` | Previous / next word |
| `Page Up` / `Page Down` | Move a screenful |
| `Delete` | Delete the character under the cursor |
| `Ctrl+W` | Delete the word before the cursor |
| `Ctrl+V` | Paste |

`Delete`, `Backspace` and `Ctrl+W` leave the yank register alone, so ordinary
typing never overwrites what you copied — only Vim's `x`/`d`/`c`/`y` fill it.

*Changing text*

| Key | Action |
|---|---|
| `x` | Delete the character under the cursor |
| `dd` / `D` | Delete the line / to end of line |
| `dw` / `db` / `d0` / `d$` | Delete word forward / back / to line start / to line end |
| `r<char>` | Replace the character under the cursor |
| `J` | Join this line with the next |
| `Tab` / `Shift+Tab` | Indent / dedent (4 spaces) — while typing |
| `u` / `Ctrl+r` | Undo / redo |

*Yank and paste* — shared with the system clipboard, so you can copy here
and paste in a browser, or the other way round.

| Key | Action |
|---|---|
| `yy` / `Y` | Yank the line |
| `y` (in VISUAL) | Yank the selection |
| `p` / `P` | Paste after / before the cursor |
| `Ctrl+V` | Paste while typing |
| `d` / `x` / `c` | Also cut into the register, like Vim |

*Selecting*

| Key | Action |
|---|---|
| `v` | Start/clear a selection at the cursor (VISUAL) |
| `V` | Select the whole current line |
| `d` / `x` / `c` / `y` / `p` (in VISUAL) | Delete / change / yank / paste over the selection |

*Saving and leaving*

| Key | Action |
|---|---|
| `Ctrl+S`, `:wq` | Save as a new version and close |
| `:w` | Save and keep editing |
| `:q` / `:q!` | Close (refuses unsaved work) / close discarding changes |
| `Esc` | Clear selection, then leave the editor (unsaved changes discarded) |
| `H` | Open history for the entry being edited |
| `:` | Command line |

Notes on the Vim emulation, so nothing surprises you:

- Words are whitespace-delimited (Vim's `W`/`B`/`E` behaviour).
- A whole insert session is **one** undo step, as in Vim.
- Running any `:`-command returns you to NORMAL mode (a command issued
  from VISUAL ends the selection).
- `Tab` inserts 4 spaces rather than a tab character, because the layout
  engine treats one character as one column. A tab that arrives by paste is
  kept in the text but *displayed* as a single space, for the same reason.
- **Counts and text objects are not implemented**: `5j`, `d3w`, `ciw`,
  `yiw`, `%`, macros, marks, registers other than the default one, and `/`
  search all do nothing. The motions and operators listed above are the
  complete set.

**History view**

| Key | Action |
|---|---|
| `h`/`l`, `←`/`→`, `Tab`/`Shift+Tab` | Switch version |
| `gg` / `G`, `Home` / `End` | Jump to first / latest version |
| `Page Up` / `Page Down` | Previous / next version |
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
| `:w` | `:write` | Save the draft as a new version, keep editing |
| `:wq` | `:x` | Save and close the editor (quits from the journal) |
| `:quit` | `:q` | Close the editor (refuses unsaved work), or quit the app |
| `:q!` | `:quit!` | Close discarding changes, or quit the app |
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
