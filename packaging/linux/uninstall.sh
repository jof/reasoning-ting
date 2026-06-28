#!/usr/bin/env bash
# Remove a per-user reasoning-ting install (see install.sh).
set -euo pipefail

bindir="${XDG_BIN_HOME:-$HOME/.local/bin}"
datadir="${XDG_DATA_HOME:-$HOME/.local/share}"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

rm -fv "$bindir/reasoning-ting" "$bindir/reasoning-ting-listen" \
       "$datadir/applications/com.thejof.ReasoningTing.desktop" \
       "$datadir/icons/hicolor/scalable/apps/com.thejof.ReasoningTing.svg" \
       "$autostart/com.thejof.ReasoningTing.desktop"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$datadir/applications" 2>/dev/null || true
echo "✓ reasoning-ting removed (your ~/.claude/keybindings.json was left untouched)"
