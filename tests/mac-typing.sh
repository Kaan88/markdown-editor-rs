#!/usr/bin/env bash
# On a macOS runner: types into mdedit in a real window through System Events and checks what
# was saved. Covers what only a Mac has: Command as the primary modifier (Command+S saves,
# Command+Left goes to the line's start) and a dead key through the input method (Option+E, then
# E, is "é"). Prints PASS or FAIL lines; exits non-zero on a failure.
#
#   tests/mac-typing.sh <out-dir> <exe>
set -uo pipefail
out="${1:?usage: tests/mac-typing.sh <out-dir> <exe>}"
exe="${2:?exe}"
mkdir -p "$out"
file="$(cd "$out" && pwd)/typed.txt"
: > "$file"
"$exe" --no-session "$file" > "$out/app.log" 2>&1 &
pid=$!
se() { osascript -e "tell application \"System Events\" to $1"; }
for _ in $(seq 1 40); do
    n=$(se "count windows of (first process whose unix id is $pid)" 2>/dev/null || echo 0)
    [ "${n:-0}" -gt 0 ] && break
    sleep 0.5
done
echo "windows of $pid: ${n:-0}"
se "set frontmost of (first process whose unix id is $pid) to true" || true
sleep 1.5
fail=0
check() { # name, expected content
    sleep 1
    got="$(cat "$file")"
    if [ "$got" = "$2" ]; then
        echo "PASS $1"
    else
        echo "FAIL $1: expected [$2], file has [$got]"
        xxd "$file" | head -5
        fail=1
    fi
}
se 'keystroke "abc "'
# A dead key: Option+E puts the accent on the next letter.
se 'keystroke "e" using option down'
se 'keystroke "e"'
# Command+S saves.
se 'keystroke "s" using command down'
check "typing, a dead key and Command+S" "abc é"
# Command+Left goes to the line's start (key code 123 is the left arrow).
se 'key code 123 using command down'
se 'keystroke "X"'
# Option+Right goes by word: over "Xabc".
se 'key code 124 using option down'
se 'keystroke "Y"'
se 'keystroke "s" using command down'
check "Command+Left and Option+Right" "XabcY é"
# Command+Z undoes the Y.
se 'keystroke "z" using command down'
se 'keystroke "s" using command down'
check "Command+Z" "Xabc é"
screencapture -x "$out/screen.png" && echo "screen captured"
kill "$pid" 2>/dev/null
wait "$pid" 2>/dev/null
echo "--- app log"
cat "$out/app.log"
exit $fail
