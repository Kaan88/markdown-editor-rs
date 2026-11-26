#!/usr/bin/env bash
# Capture only the new scratch bundle's verified document window.
set -euo pipefail
out="${1:?usage: tests/mac-window.sh <out-dir> <app> [args...] <file>}"
target="${2:?app}"
shift 2
[[ $# -gt 0 ]] || { echo "Expected a fixture document" >&2; exit 1; }
mkdir -p "$out"
out="$(cd "$out" && pwd -P)"
source "$(dirname "$0")/mac-launch.sh"
fixture="${!#}"
document="$out/window-$(uuidgen).${fixture##*.}"
cp "$fixture" "$document"
launch "$target" "${@:1:$#-1}" "$document"
ui capture clean main "$wid" "$out/window.png"
