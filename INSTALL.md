# Installing and configuring TextPoppup

TextPoppup is a single self-contained binary (~1 MB, no runtime, no daemon,
no network access). It stores your journal in one JSON file and reads
nothing else.

- [Install](#install)
- [Configure a hotkey](#configure-a-hotkey)
- [Make it a floating popup](#make-it-a-floating-popup-hyprland)
- [Where your notes live](#where-your-notes-live)
- [Uninstall](#uninstall)
- [Troubleshooting](#troubleshooting)

## Install

### Option 1 — prebuilt binary (Linux x86_64)

Grab the latest tarball from the
[Releases page](../../releases/latest), then:

```bash
tar -xzf textpoppup-*-x86_64-linux.tar.gz
cd textpoppup-*-x86_64-linux
install -Dm755 textpoppup ~/.local/bin/textpoppup
```

Verify the download if you like — each release ships a `.sha256`:

```bash
sha256sum -c textpoppup-*.tar.gz.sha256
```

Make sure `~/.local/bin` is on your `PATH`:

```bash
# bash / zsh — add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.local/bin:$PATH"
# fish
fish_add_path ~/.local/bin
```

### Option 2 — build from source

Needs the Rust toolchain ([rustup](https://rustup.rs) or your package
manager; on Arch: `sudo pacman -S rust`).

```bash
git clone <this-repo> textpoppup
cd textpoppup
./install.sh          # builds --release and installs to ~/.local/bin
```

Or do it by hand:

```bash
cargo build --release
install -Dm755 target/release/textpoppup ~/.local/bin/textpoppup
```

Run it with `textpoppup`. Press `?` inside for the full keybinding list.

### Optional: clipboard integration

Yank and paste share the system clipboard when one of these is installed —
TextPoppup uses whichever it finds, and falls back to an internal register
if none are present:

| Session | Package |
|---|---|
| Wayland | `wl-clipboard` (`wl-copy` / `wl-paste`) |
| X11 | `xclip` or `xsel` |
| macOS | built in (`pbcopy` / `pbpaste`) |

```bash
sudo pacman -S wl-clipboard      # Arch / CachyOS, Wayland
sudo apt install wl-clipboard    # Debian / Ubuntu, Wayland
sudo apt install xclip           # X11
```

## Configure a hotkey

TextPoppup runs in a terminal, so a global hotkey is set up in your window
manager or desktop — it launches a terminal that runs `textpoppup`. Pick
your setup below. The examples use `Super+N` and `kitty`; swap in whatever
key and terminal you prefer (`alacritty -e`, `foot`, `wezterm start`,
`gnome-terminal --`).

**Toggle behaviour** — the snippets below open the popup on first press and
dismiss it on second press. `pkill -x textpoppup` matches only this program,
so it can never close another window.

> Note: dismissing discards an *unsaved* draft. Save with `Ctrl+S` (or
> `:w` / `:wq`) first; saved entries are always on disk.

### Hyprland

In `~/.config/hypr/hyprland.conf`:

```ini
bind = SUPER, N, exec, pkill -x textpoppup || kitty --class textpoppup -e textpoppup
```

If your config is the newer Lua format (`~/.config/hypr/keybinds.lua`):

```lua
hl.bind(mainMod .. " + N", hl.dsp.exec_cmd(
    "pkill -x textpoppup || kitty --class textpoppup -e textpoppup"))
```

### sway / i3

In `~/.config/sway/config` (or `~/.config/i3/config`):

```
bindsym $mod+n exec pkill -x textpoppup || kitty --class textpoppup -e textpoppup
```

### GNOME

```bash
KEY=/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/textpoppup/
gsettings set org.gnome.settings-daemon.plugins.media-keys custom-keybindings "['$KEY']"
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$KEY name 'TextPoppup'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$KEY binding '<Super>n'
gsettings set org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:$KEY command 'kitty -e textpoppup'
```

Or: Settings → Keyboard → View and Customize Shortcuts → Custom Shortcuts.

### KDE Plasma

System Settings → Shortcuts → Add New → Command/URL, with the command
`kitty --class textpoppup -e textpoppup`, then assign `Super+N`.

### macOS

Create an Automator "Quick Action" running
`/opt/homebrew/bin/kitty -e textpoppup`, then bind it under System Settings
→ Keyboard → Keyboard Shortcuts → Services. Launchers like Raycast or
Alfred can do the same in one step.

## Make it a floating popup (Hyprland)

By default your terminal opens TextPoppup like any other window. To make it
behave like a proper quick-capture popup — floating, centred, above
everything — add window rules matching the `--class textpoppup` from the
hotkey above.

`~/.config/hypr/hyprland.conf`:

```ini
windowrulev2 = float, class:^(textpoppup)$
windowrulev2 = center, class:^(textpoppup)$
windowrulev2 = size 900 600, class:^(textpoppup)$
windowrulev2 = pin, class:^(textpoppup)$
windowrulev2 = animation slide right, class:^(textpoppup)$
```

Lua format (`~/.config/hypr/rules.lua`):

```lua
hl.window_rule({
    match = { class = "^(textpoppup)$" },
    float = true, center = true, size = "900 600", pin = true,
})
hl.window_rule({
    match = { class = "^(textpoppup)$" },
    animation = "slide right",
})
```

`pin` keeps it above other windows on every workspace — handy, but it also
means the popup follows you when you switch workspace. Drop that line if
you'd rather it stay put.

For sway, the equivalent is:

```
for_window [app_id="textpoppup"] floating enable, resize set 900 600, move position center
```

## Where your notes live

One JSON file, written atomically (temp file + rename) so a crash can't
truncate it:

| OS | Path |
|---|---|
| Linux | `~/.local/share/textpoppup/entries.json` |
| macOS | `~/Library/Application Support/textpoppup/entries.json` |
| Windows | `%APPDATA%\textpoppup\entries.json` |

It's plain JSON and safe to back up, sync, or keep in a git repo. Every
entry keeps its full version history, so the file grows with edits rather
than overwriting them.

```bash
# quick backup
cp ~/.local/share/textpoppup/entries.json ~/notes-backup.json
```

There is no config file: keybindings and colours are compiled in. Changing
them means editing `src/keybind.rs` or `src/format.rs` and rebuilding —
`FEATURES.md` documents exactly where.

## Uninstall

```bash
rm ~/.local/bin/textpoppup
# your notes are separate; delete them only if you mean to
rm -rf ~/.local/share/textpoppup
```

Then remove the hotkey and window rules you added above.

## Troubleshooting

**The hotkey does nothing.** Check the binary is reachable from a
non-interactive shell — window managers don't load your shell config:

```bash
command -v textpoppup     # must print a path
```

If it doesn't, use the absolute path (`/home/you/.local/bin/textpoppup`) in
the keybinding.

**It opens then closes instantly.** Your terminal's "run a command" flag
differs. kitty/alacritty/foot use `-e`, gnome-terminal uses `--`, wezterm
uses `start`. Test the command in a shell first.

**`Ctrl+1`–`Ctrl+9` don't switch history versions.** Plain terminals can't
encode `Ctrl`+digit; it works in terminals supporting the
[Kitty keyboard protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)
(kitty, foot, wezterm, recent alacritty). `h`/`l`, arrows, `Tab` and
`gg`/`G` work everywhere.

**Yank/paste doesn't reach other apps.** Install a clipboard tool (see
above). Without one, yank and paste still work inside TextPoppup.

**Mouse selection doesn't work.** TextPoppup captures the mouse so you can
click history tabs. Hold `Shift` and drag for your terminal's own
selection.
