#!/bin/sh
# Installs mdedit from this folder into ~/.local, or into the prefix given as the argument:
#   ./install.sh [PREFIX]
# Copies bin/ and share/ under PREFIX and points the desktop entry's Exec at the installed binary,
# so the launcher works even when PREFIX/bin is not on PATH. uninstall.sh removes it again.
set -eu
app=mdedit
here="$(cd "$(dirname "$0")" && pwd)"
prefix="${1:-${HOME:?HOME is not set}/.local}"
case "$prefix" in /*) ;; *) prefix="$(pwd)/$prefix" ;; esac

mkdir -p "$prefix/bin" "$prefix/share/applications" "$prefix/share/icons/hicolor/256x256/apps"
cp "$here/bin/$app" "$prefix/bin/$app.new"
chmod 755 "$prefix/bin/$app.new"
mv -f "$prefix/bin/$app.new" "$prefix/bin/$app"
cp "$here/share/icons/hicolor/256x256/apps/$app.png" "$prefix/share/icons/hicolor/256x256/apps/$app.png"
# Exec is rewritten to the absolute path; a path with a space is quoted as the desktop entry
# specification asks.
exec_path="$prefix/bin/$app"
case "$exec_path" in *" "*) exec_path="\"$exec_path\"" ;; esac
sed "s|^Exec=$app |Exec=$exec_path |" "$here/share/applications/$app.desktop" > "$prefix/share/applications/$app.desktop"
chmod 644 "$prefix/share/applications/$app.desktop"

command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$prefix/share/applications" 2>/dev/null || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && [ -f "$prefix/share/icons/hicolor/index.theme" ] \
    && gtk-update-icon-cache -q -t -f "$prefix/share/icons/hicolor" 2>/dev/null || true

echo "installed $app into $prefix"
case ":$PATH:" in *":$prefix/bin:"*) ;; *) echo "note: $prefix/bin is not on your PATH" ;; esac
