#!/usr/bin/env bash
# An .icns from the app mark's SVG (256 x 256, assets/make_icon.py), on a Mac: Quick Look draws it
# at 1024 px with the margin Mac icons keep, sips makes the other sizes, iconutil packs them.
#
#   packaging/macos/make-icns.sh <icon.svg> <out.icns>
set -euo pipefail
svg="${1:?usage: make-icns.sh <icon.svg> <out.icns>}"
icns="${2:?out.icns}"
work="$(mktemp -d)"
# A Mac icon leaves a margin: its tile is about 824 of 1024 px (80 %). The mark's tile is 240 of
# its 256 units (8 to 248), so the view box grows to 298 units around the same centre.
padded="$work/$(basename "$svg")"
sed 's/viewBox="0 0 256 256"/viewBox="-21 -21 298 298"/' "$svg" > "$padded"
grep -q 'viewBox="-21 -21 298 298"' "$padded" || { echo "$svg: no 256 x 256 view box" >&2; exit 1; }
qlmanage -t -s 1024 -o "$work" "$padded" >/dev/null 2>&1
big="$padded.png"
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
