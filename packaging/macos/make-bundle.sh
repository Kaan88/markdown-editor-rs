#!/usr/bin/env bash
# Makes mdedit.app from a release build, on a Mac: the executable in Contents/MacOS, Info.plist
# (Markdown, plain text and source files as documents it edits; macOS 12 or later), the icon
# from assets/mdedit-macos.png, and the ad-hoc signature the system requires of Apple Silicon code (no
# Developer ID: see the viewer's docs/MACOS.md, decision 2).
#
#   packaging/macos/make-bundle.sh <target-dir> <out-dir>    # e.g. target/release dist
set -euo pipefail
bin="${1:?usage: make-bundle.sh <target-dir> <out-dir>}"
out="${2:?out-dir}"
root="$(cd "$(dirname "$0")/../.." && pwd)"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)"
app="$out/mdedit.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin/mdedit" "$app/Contents/MacOS/"
"$root/packaging/macos/make-icns.sh" "$root/assets/mdedit-macos.png" "$app/Contents/Resources/mdedit.icns"
sed "s/@VERSION@/$version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
codesign --force --sign - "$app"
echo "$app ($version)"
