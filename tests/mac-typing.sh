#!/usr/bin/env bash
# On a macOS runner: types into mdedit in a real window through System Events and checks what
# was saved. Covers what only a Mac has: Command as the primary modifier (Command+S saves,
# Command+Left goes to the line's start) and a dead key through the input method (Option+E, then
# E, is "é"). Prints PASS or FAIL lines; exits non-zero on a failure.
#
#   tests/mac-typing.sh <out-dir> <app-bundle-or-exe>
set -uo pipefail
out="${1:?usage: tests/mac-typing.sh <out-dir> <exe>}"
exe="${2:?exe}"
mkdir -p "$out"
file="$(cd "$out" && pwd)/typed.txt"
: > "$file"
source "$(dirname "$0")/mac-launch.sh"
launch "$exe" --no-session "$file"
se() { osascript -e "tell application \"System Events\" to $1"; }
wid="$(window_id)"
echo "window id: ${wid:-none}"
echo "frontmost: $(osascript -e 'tell application "System Events" to get name of first process whose frontmost is true' 2>&1)"
sleep 1
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
# The menu bar's Edit items leave the keys to the window: Command+A, X, then V twice.
se 'keystroke "a" using command down'
se 'keystroke "x" using command down'
se 'keystroke "v" using command down'
se 'keystroke "v" using command down'
se 'keystroke "s" using command down'
check "Command+A, X and V" "Xabc éXabc é"
screencapture -x "$out/screen.png" && echo "screen captured"
# The system's open panel (rfd, from a helper thread) opens with Command+O and closes with Escape.
se 'keystroke "o" using command down'
sleep 2
screencapture -x "$out/open-panel.png" && echo "open panel captured"
se 'key code 53'
sleep 1
if kill -0 "$pid" 2>/dev/null; then echo "PASS the open panel opens and closes"; else echo "FAIL the open panel ended the process"; fail=1; fi
# The Edit menu, opened from the menu bar: its items are disabled over the window.
se 'tell process "mdedit" to click menu bar item "Edit" of menu bar 1' || echo "no Edit menu"
sleep 1
screencapture -x "$out/edit-menu.png" && echo "edit menu captured"
se 'key code 53'
sleep 0.5
# Control+Command+F: the window's full screen (the Window menu's item), and back.
se 'keystroke "f" using {control down, command down}'
sleep 3
screencapture -x "$out/full-screen.png" && echo "full screen captured"
se 'keystroke "f" using {control down, command down}'
sleep 3
# The system's list of this application's recent documents has the file.
recent="$HOME/Library/Application Support/com.apple.sharedfilelist/com.apple.LSSharedFileList.ApplicationRecentDocuments"
ls "$recent" 2>/dev/null | grep -i mdedit || true
if ls "$recent" 2>/dev/null | grep -qi 'kaan88.mdedit'; then
    echo "PASS the system lists mdedit's recent documents"
else
    echo "FAIL no recent documents of mdedit in the system's list"
    fail=1
fi
# Command+Q with unsaved changes asks first (winit's own Quit would end the process at once).
se 'keystroke "!"'
sleep 1
# Unsaved now: the close button has its dot.
screencapture -x "$out/edited.png" && echo "edited captured"
se 'keystroke "q" using command down'
sleep 2
if kill -0 "$pid" 2>/dev/null; then
    echo "PASS Command+Q with unsaved changes asks first"
else
    echo "FAIL Command+Q with unsaved changes ended the process"
    fail=1
fi
screencapture -x "$out/quit.png" && echo "quit captured"
stop
exit $fail
