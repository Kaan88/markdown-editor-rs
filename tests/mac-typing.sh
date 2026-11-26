#!/usr/bin/env bash
# Exercise macOS shortcuts and the dead key only in the verified scratch editor.
set -euo pipefail
out="${1:?usage: tests/mac-typing.sh <out-dir> <app>}"
target="${2:?app}"
[[ "${RUNNER_ENVIRONMENT:-}" == github-hosted ]] || { echo "Typing (including Cut/Paste) requires a GitHub-hosted runner" >&2; exit 1; }
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
source "$(dirname "$0")/mac-launch.sh"
file="$out/typed-$(uuidgen).txt"
: > "$file"
launch "$target" --light "$file"

keys() { ui keys "$1" main "$wid" "${@:2}"; }
check() {
    for _ in $(seq 1 40); do
        [[ "$(cat "$file")" == "$2" ]] && break
        sleep 0.25
    done
    wait_ui window clean main >/dev/null
    [[ "$(cat "$file")" == "$2" ]] || mac_fail "$1: unexpected saved text"
    echo "PASS $1"
}
# abc + Space, Option+E then E, Command+S.
keys clean 0 11 8 49 14:opt 14 1:cmd
check "typing, a dead key and Command+S" "abc é"
# Command+Left, Shift+X, Option+Right, Shift+Y, Save.
keys clean 123:cmd 7:shift 124:opt 16:shift 1:cmd
check "Command+Left and Option+Right" "XabcY é"
keys clean 6:cmd 1:cmd
check "Command+Z" "Xabc é"
keys clean 0:cmd 7:cmd 9:cmd 9:cmd 1:cmd
check "Command+A, X and V" "Xabc éXabc é"
ui capture clean main "$wid" "$out/window.png"

keys clean 31:cmd
panel="$(wait_ui window clean open)" || mac_fail "the owned Open panel did not appear"
ui capture clean open "$panel" "$out/open-panel.png"
ui keys clean open "$panel" 53
[[ "$(wait_ui window clean main)" == "$wid" ]] || mac_fail "Open panel did not dismiss"
echo "PASS the owned Open panel appeared and dismissed"

# The native menu action uses this PID's accessibility object. Never
# capture the desktop or the menu bar (which is not a scratch window).
ui edit-menu clean main "$wid"
keys clean 53
keys clean 3:ctrl:cmd
sleep 2
wid="$(wait_ui window clean main)"
ui capture clean main "$wid" "$out/full-screen.png"
keys clean 3:ctrl:cmd
sleep 2
wid="$(wait_ui window clean main)"
wait_ui recent clean main "$wid"
echo "PASS the scratch identity's recent list contains the exact document"

keys clean 18:shift
[[ "$(wait_ui window dirty main)" == "$wid" ]] || mac_fail "scratch document did not become dirty"
ui capture dirty main "$wid" "$out/edited.png"
"$mac_tool" quit-title "$out/edited.png" closed
keys dirty 12:cmd
sleep 1
ui capture dirty main "$wid" "$out/quit.png"
"$mac_tool" quit-title "$out/quit.png" open
keys dirty 53
sleep 1
ui capture dirty main "$wid" "$scratch/cancelled.png"
"$mac_tool" quit-title "$scratch/cancelled.png" closed
check_saved="Xabc éXabc é"
[[ "$(cat "$file")" == "$check_saved" ]] || mac_fail "Quit confirmation changed the saved document"
echo "PASS the owned Quit confirmation appeared and was cancelled"
