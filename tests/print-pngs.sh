#!/usr/bin/env bash
# Prints PNG files as base64 between markers, for reading pictures out of a CI log where the
# artifacts cannot be downloaded. tools/mac-look-decode.sh writes them back to files.
#
#   tests/print-pngs.sh file.png...
for f in "$@"; do
    [ -f "$f" ] || { echo "missing: $f"; continue; }
    echo "=====PNG-BEGIN $(basename "$f")====="
    base64 < "$f" | tr -d '\n' | fold -w 76
    echo
    echo "=====PNG-END====="
done
