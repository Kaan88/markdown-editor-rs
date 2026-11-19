#!/usr/bin/env bash
# Makes mdedit.app from a release build, on a Mac: the executable in Contents/MacOS (with
# mdview-engine, the preview pane's engine, beside it when <target-dir> holds one; the release
# builds it, see packaging/engine-source.sh), Info.plist
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
[ ! -f "$bin/mdview-engine" ] || cp "$bin/mdview-engine" "$app/Contents/MacOS/"
[ ! -f "$bin/mdedit-ai" ] || cp "$bin/mdedit-ai" "$app/Contents/MacOS/"
"$root/packaging/macos/make-icns.sh" "$root/assets/mdedit-macos.png" "$app/Contents/Resources/mdedit.icns"
sed "s/@VERSION@/$version/g" "$root/packaging/macos/Info.plist" > "$app/Contents/Info.plist"
plutil -lint "$app/Contents/Info.plist"
# A second executable in Contents/MacOS is a nested code object: it is signed first, or the
# bundle's signature does not verify. (The arm64 linker signs it ad hoc already; x86_64's does not.)
[ ! -f "$app/Contents/MacOS/mdview-engine" ] || codesign --force --sign - "$app/Contents/MacOS/mdview-engine"
[ ! -f "$app/Contents/MacOS/mdedit-ai" ] || codesign --force --sign - "$app/Contents/MacOS/mdedit-ai"
codesign --force --sign - "$app"
echo "$app ($version)"
