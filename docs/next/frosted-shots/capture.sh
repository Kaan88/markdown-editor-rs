#!/usr/bin/env bash
# Opens mdedit for a few seconds over a colourful pattern window and saves a screen capture.
#   capture.sh <name> <active|inactive> <mdedit arguments...>
# Run from Git Bash on Windows. Uses a scratch settings file and --no-session; changes nothing on
# the system. The pattern window is the frosted spike's (`spikes/frosted` on the branch
# spike/frosted-window), built into the same target folder.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
t="${CARGO_TARGET_DIR:-/c/Users/kaank/Desktop/md-wt/target-frosted}"
exe="${MDEDIT_EXE:-$t/release/mdedit.exe}"
pattern="$t/release/frosted-spike.exe"
name="$1"; focus="$2"; shift 2
export MDEDIT_SETTINGS="$(mktemp -d)/settings.ini"
hold=4000
"$pattern" --pattern --pos 0,0 --size "${PATTERN_SIZE:-2600,1300}" --auto-exit-ms $((hold + 2500)) 2>/dev/null &
sleep 0.8
"$exe" --no-session --exit-after-ms $hold --trace "$@" 2>"$here/$name.log" &
sleep 1.6
if [ "$focus" = inactive ]; then
    # Another window takes the keyboard: a small pattern window off to the side.
    "$pattern" --pattern --pos 1500,700 --size 200,120 --auto-exit-ms 2200 2>/dev/null &
    sleep 1.0
fi
# Where the window is, from its own trace: "window (x0,y0)-(x1,y1) client ...". 40 px around it.
rect="$(grep -oE 'window \([-0-9]+,[-0-9]+\)-\([-0-9]+,[-0-9]+\)' "$here/$name.log" | head -1 | sed -E 's/[)]-[(]/ /; s/[^-0-9]+/ /g')"
read -r x0 y0 x1 y1 <<<"$rect"
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$here/grab.ps1")" \
    -X $((x0 - 40)) -Y $((y0 - 40)) -W $((x1 - x0 + 80)) -H $((y1 - y0 + 80)) -Out "$(cygpath -w "$here/$name.png")"
wait
grep -h "backdrop\|first frame after" "$here/$name.log" || true
rm -f "$here/$name.log"
