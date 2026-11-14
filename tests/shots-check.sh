#!/usr/bin/env bash
# The smoke check of CI: after tests/shots.sh <dir>, every shot the script always takes (a `shot NAME`
# line at the start of a line; the indented ones are taken only where a font or tool is there) must
# exist, and every PNG in the folder must be a non-empty PNG. No pixel comparison: fonts differ
# between machines.
#   tests/shots-check.sh target/shots
set -euo pipefail
dir="${1:?usage: tests/shots-check.sh <output-dir>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
# The names are the first argument of every unconditional `shot NAME ...` line of shots.sh.
want="$(sed -n 's/^shot \([A-Za-z0-9@._-]*\)[ ].*/\1/p' "$root/tests/shots.sh" | sort -u)"
[ -n "$want" ] || { echo "shots-check: no shots found in shots.sh" >&2; exit 1; }
bad=0 n=0
for name in $want; do
    n=$((n + 1))
    f="$dir/$name.png"
    if [ ! -s "$f" ]; then echo "missing or empty: $f" >&2; bad=$((bad + 1)); continue; fi
    [ "$(head -c 8 "$f" | od -An -tx1 | tr -d ' \n')" = "89504e470d0a1a0a" ] || { echo "not a PNG: $f" >&2; bad=$((bad + 1)); }
done
for f in "$dir"/*.png; do
    [ -s "$f" ] || { echo "empty: $f" >&2; bad=$((bad + 1)); continue; }
    [ "$(head -c 8 "$f" | od -An -tx1 | tr -d ' \n')" = "89504e470d0a1a0a" ] || { echo "not a PNG: $f" >&2; bad=$((bad + 1)); }
done
echo "shots-check: $n shots wanted, $bad bad, $(find "$dir" -name '*.png' | wc -l) PNGs in $dir"
[ "$bad" = 0 ]
