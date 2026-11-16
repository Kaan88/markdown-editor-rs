#!/usr/bin/env bash
# An .icns from the Mac icon's 1024 px source (assets/<name>-macos.png, written by the viewer's
# assets/make_icon.py with --macos: transparent, the tile 824 px wide as the macOS icon grid has
# it), on a Mac: sips makes the sizes, iconutil packs them.
#
#   packaging/macos/make-icns.sh <icon-1024.png> <out.icns>
set -euo pipefail
big="${1:?usage: make-icns.sh <icon-1024.png> <out.icns>}"
icns="${2:?out.icns}"
work="$(mktemp -d)"
set_dir="$work/icon.iconset"
mkdir "$set_dir"
for s in 16 32 128 256 512; do
    sips -z $s $s "$big" --out "$set_dir/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z $d $d "$big" --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$icns"
rm -rf "$work"
