#!/usr/bin/env bash
# Remove a per-user some-ting install (see install.sh).
set -euo pipefail

bindir="${XDG_BIN_HOME:-$HOME/.local/bin}"
datadir="${XDG_DATA_HOME:-$HOME/.local/share}"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart"

rm -fv "$bindir/some-ting" "$bindir/some-ting-listen" \
       "$datadir/applications/com.thejof.SomeTing.desktop" \
       "$datadir/icons/hicolor/scalable/apps/com.thejof.SomeTing.svg" \
       "$autostart/com.thejof.SomeTing.desktop"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$datadir/applications" 2>/dev/null || true
echo "✓ some-ting removed (your ~/.claude/keybindings.json was left untouched)"
