#!/usr/bin/env bash
# Writes the PNGs that tests/print-pngs.sh printed into a CI log back to files.
#
#   gh run view <run> --repo <owner/repo> --log | tools/mac-look-decode.sh <out-dir>
set -euo pipefail
out="${1:?usage: tools/mac-look-decode.sh <out-dir>}"
mkdir -p "$out"
# gh prefixes every line with the job and step names and a timestamp: keep what follows them.
sed -E 's/^[^\t]*\t[^\t]*\t[0-9T:.-]+Z //' | awk -v out="$out" '
    /=====PNG-BEGIN / { name = $2; sub(/=====$/, "", name); file = out "/" name ".b64"; printf "" > file; on = 1; next }
    /=====PNG-END=====/ { on = 0; close(file); print file; next }
    on { print >> file }
' | while read -r b64; do
    base64 -d < "$b64" > "${b64%.b64}" && rm "$b64" && echo "${b64%.b64}"
done
