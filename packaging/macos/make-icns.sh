#!/usr/bin/env bash
# An .icns from an SVG, on a Mac: Quick Look draws the SVG at 1024 px, sips makes the other
# sizes, iconutil packs them.
#
#   packaging/macos/make-icns.sh <icon.svg> <out.icns>
set -euo pipefail
svg="${1:?usage: make-icns.sh <icon.svg> <out.icns>}"
icns="${2:?out.icns}"
work="$(mktemp -d)"
qlmanage -t -s 1024 -o "$work" "$svg" >/dev/null 2>&1
big="$work/$(basename "$svg").png"
[ -f "$big" ] || { echo "qlmanage drew nothing for $svg" >&2; exit 1; }
set_dir="$work/icon.iconset"
mkdir "$set_dir"
for s in 16 32 128 256 512; do
    sips -z $s $s "$big" --out "$set_dir/icon_${s}x${s}.png" >/dev/null
    d=$((s * 2))
    sips -z $d $d "$big" --out "$set_dir/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$set_dir" -o "$icns"
rm -rf "$work"
