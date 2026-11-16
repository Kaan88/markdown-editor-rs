#!/usr/bin/env bash
# On a macOS runner: starts the app in a real window, waits for it, and takes pictures of the
# screen with screencapture, so the native window (title bar, buttons, Retina-less scale) can be
# looked at without a Mac. Prints each picture as base64 between markers when PRINT=1, which is
# how a session that cannot download artifacts gets them (tools/mac-look-decode.sh).
#
#   tests/mac-window.sh <out-dir> <exe> [app args...]
set -uo pipefail
out="${1:?usage: tests/mac-window.sh <out-dir> <exe> [args...]}"
exe="${2:?exe}"
shift 2
mkdir -p "$out"
"$exe" "$@" > "$out/app.log" 2>&1 &
pid=$!
# The window: wait until the process owns one (System Events), up to 20 s.
for _ in $(seq 1 40); do
    n=$(osascript -e "tell application \"System Events\" to count windows of (first process whose unix id is $pid)" 2>/dev/null || echo 0)
    [ "${n:-0}" -gt 0 ] && break
    sleep 0.5
done
echo "windows of $pid: ${n:-0}"
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true" 2>&1 || true
sleep 2
screencapture -x "$out/screen.png" && echo "screen captured"
# The window alone, by its id from the window server.
wid=$(swift -e 'import CoreGraphics; let pid = Int32(CommandLine.arguments[1])!; let l = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]; for w in l where (w[kCGWindowOwnerPID as String] as? Int32) == pid && (w[kCGWindowLayer as String] as? Int) == 0 { print(w[kCGWindowNumber as String]!); break }' "$pid" 2>/dev/null)
echo "window id: ${wid:-none}"
if [ -n "${wid:-}" ]; then
    screencapture -x -o -l "$wid" "$out/window.png" && echo "window captured"
fi
kill "$pid" 2>/dev/null
wait "$pid" 2>/dev/null
echo "--- app log"
cat "$out/app.log"
