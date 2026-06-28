#!/usr/bin/env bash
# Install reasoning-ting (GUI tray + CLI) for the current user, no root needed.
#
# Installs into ~/.local (XDG user dirs):
#   ~/.local/bin/reasoning-ting            the menu-bar/tray app
#   ~/.local/bin/reasoning-ting-listen     the headless CLI daemon
#   ~/.local/share/applications/      .desktop launcher
#   ~/.local/share/icons/hicolor/…    app icon
#
# Build first (see listener/README.md):
#   cd listener && cargo build --release --features gui
#
# This is deliberately an UNSANDBOXED install. reasoning-ting needs host access that
# Flatpak/Snap sandboxes deny: it reads /proc to tell which window is focused
# (the focus guard), injects keystrokes via XTEST, writes ~/.claude, and touches
# the TINGDISK USB volume. See docs/PACKAGING.md.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
bindir="${XDG_BIN_HOME:-$HOME/.local/bin}"
datadir="${XDG_DATA_HOME:-$HOME/.local/share}"
appdir="$datadir/applications"
icondir="$datadir/icons/hicolor/scalable/apps"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

gui="$repo_root/listener/target/release/reasoning-ting"
cli="$repo_root/listener/target/release/reasoning-ting-listen"

if [[ ! -x "$gui" ]]; then
  echo "error: $gui not found." >&2
  echo "Build it first:  (cd '$repo_root/listener' && cargo build --release --features gui)" >&2
  exit 1
fi

mkdir -p "$bindir" "$appdir" "$icondir"
install -m755 "$gui" "$bindir/reasoning-ting"
[[ -x "$cli" ]] && install -m755 "$cli" "$bindir/reasoning-ting-listen"
install -m644 "$repo_root/packaging/linux/com.thejof.ReasoningTing.desktop" "$appdir/com.thejof.ReasoningTing.desktop"
install -m644 "$repo_root/packaging/linux/com.thejof.ReasoningTing.svg" "$icondir/com.thejof.ReasoningTing.svg"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$appdir" 2>/dev/null || true
command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache "$datadir/icons/hicolor" 2>/dev/null || true

# Optional: start automatically at login (tray app).
if [[ "${1:-}" == "--autostart" ]]; then
  mkdir -p "$autostart"
  install -m644 "$appdir/com.thejof.ReasoningTing.desktop" "$autostart/com.thejof.ReasoningTing.desktop"
  echo "✓ autostart enabled ($autostart/com.thejof.ReasoningTing.desktop)"
fi

echo "✓ installed reasoning-ting → $bindir/reasoning-ting"
echo
echo "Next steps:"
echo "  1. Make sure $bindir is on your PATH."
echo "  2. Bind the voice key in Claude Code:  reasoning-ting → menu → 'Write Claude keybinding (f12)'"
echo "     (or run: reasoning-ting-listen --help)."
echo "  3. i3/sway/dwm users: you need an SNI tray host so the icon shows —"
echo "     e.g. run 'snixembed' (bridges to i3bar). GNOME/KDE show it natively."
echo "  4. Launch 'reasoning-ting' (or log out/in if you used --autostart)."
