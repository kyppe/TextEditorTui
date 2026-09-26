#!/usr/bin/env bash
# Installs TextPoppup into ~/.local/bin (override with PREFIX=...).
#
#   ./install.sh                 # build from source and install
#   ./install.sh --binary ./textpoppup   # install an already-built binary
#
# Does not touch your window-manager config; see INSTALL.md for the hotkey.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.local}"
BINDIR="$PREFIX/bin"
BINARY=""

while [ $# -gt 0 ]; do
    case "$1" in
        --binary) BINARY="${2:?--binary needs a path}"; shift 2 ;;
        # Print the header comment block (stops at the first non-comment line).
        -h|--help) awk 'NR>1 && /^#/ {sub(/^# ?/,""); print; next} NR>1 {exit}' "$0"; exit 0 ;;
        *) echo "unknown option: $1" >&2; exit 2 ;;
    esac
done

if [ -z "$BINARY" ]; then
    command -v cargo >/dev/null 2>&1 || {
        echo "cargo not found. Install Rust from https://rustup.rs, or pass" >&2
        echo "a prebuilt binary: ./install.sh --binary ./textpoppup" >&2
        exit 1
    }
    echo "==> building (release)"
    cargo build --release --locked
    BINARY="target/release/textpoppup"
fi

[ -f "$BINARY" ] || { echo "no such binary: $BINARY" >&2; exit 1; }

echo "==> installing to $BINDIR/textpoppup"
install -Dm755 "$BINARY" "$BINDIR/textpoppup"

case ":$PATH:" in
    *":$BINDIR:"*) ;;
    *)
        echo
        echo "NOTE: $BINDIR is not on your PATH. Add this to your shell config:"
        echo "  bash/zsh:  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo "  fish:      fish_add_path \$HOME/.local/bin"
        ;;
esac

echo
echo "Done. Run 'textpoppup' to start, '?' inside for help."
echo "To bind it to a hotkey, see INSTALL.md."
