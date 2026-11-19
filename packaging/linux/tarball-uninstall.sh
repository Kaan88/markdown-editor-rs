#!/bin/sh
# Removes what install.sh put under ~/.local, or under the prefix given as the argument:
#   ./uninstall.sh [PREFIX]
set -eu
app=mdedit
prefix="${1:-${HOME:?HOME is not set}/.local}"
case "$prefix" in /*) ;; *) prefix="$(pwd)/$prefix" ;; esac
rm -f "$prefix/bin/$app" "$prefix/lib/$app/mdview-engine" "$prefix/lib/$app/mdedit-ai" "$prefix/share/applications/$app.desktop" \
      "$prefix/share/icons/hicolor/256x256/apps/$app.png"
rmdir "$prefix/lib/$app" 2>/dev/null || true
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
echo "removed $app from $prefix"
