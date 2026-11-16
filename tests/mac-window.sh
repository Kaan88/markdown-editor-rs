#!/usr/bin/env bash
# On a macOS runner: starts the app in a real window (a bundle through `open`, as the Mac starts
# it), waits for its window and takes pictures with screencapture: the whole screen and the window
# alone. For looking at the native window without a Mac (tests/print-pngs.sh prints them into the
# log).
#
#   tests/mac-window.sh <out-dir> <app-bundle-or-exe> [app args...]
set -uo pipefail
out="${1:?usage: tests/mac-window.sh <out-dir> <app> [args...]}"
target="${2:?app}"
shift 2
mkdir -p "$out"
source "$(dirname "$0")/mac-launch.sh"
launch "$target" "$@"
wid="$(window_id)"
echo "window id: ${wid:-none}"
sleep 2
screencapture -x "$out/screen.png" && echo "screen captured"
if [ -n "${wid:-}" ]; then
    screencapture -x -o -l "$wid" "$out/window.png" && echo "window captured"
fi
stop
