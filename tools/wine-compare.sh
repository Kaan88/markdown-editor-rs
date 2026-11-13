#!/usr/bin/env bash
# Runs tests/shots.sh once with a native-Windows-built exe and once with a cross-built exe, both
# under the same Wine prefix (with the Windows fonts), and compares the PNGs byte for byte.
#
#   tools/wine-compare.sh NATIVE_EXE CROSS_EXE        e.g. the Windows build of the same commit
#
# Output: target/wine-compare/{native,cross}/*.png and the verdict, one line per differing PNG.
# This shows the two exes behave the same under Wine; it is not a Windows run.
set -euo pipefail

native="$(realpath "${1:?native exe}")"
cross="$(realpath "${2:?cross exe}")"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cache="${MD_BUILD_CACHE:-$HOME/.cache/md-build}"
export WINEPREFIX="$cache/wine-prefix" WINEARCH=win64 WINEDEBUG=-all WINEDLLOVERRIDES="mscoree,mshtml="
fonts="$WINEPREFIX/drive_c/windows/Fonts"
mkdir -p "$fonts"
for dir in /usr/local/share/fonts/WindowsFonts /mnt/C/Windows/Fonts; do
    if [ -d "$dir" ]; then cp -n "$dir"/* "$fonts"/ 2>/dev/null || true; break; fi
done

# The large inputs shots.sh makes under target/data, made once with the host's cargo.
cd "$root"
if [ ! -f target/data/100mb.txt ] || [ ! -f target/data/1mb-line.txt ]; then
    mkdir -p target/data
    cargo run --release -q -p mdedit-core --example gen -- lines 100 target/data/100mb.txt
    cargo run --release -q -p mdedit-core --example gen -- long 1 target/data/1mb-line.txt
fi

run_set() { # name exe
    local name="$1" exe="$2" scratch="$root/target/wine-compare/tree"
    rm -rf "$scratch" "$root/target/wine-compare/$name"
    mkdir -p "$scratch/target/release" "$scratch/tests"
    cp -r fixtures "$scratch/"
    cp tests/shots.sh "$scratch/tests/"
    ln -s "$root/target/data" "$scratch/target/data"
    # shots.sh runs target/release/mdedit.exe directly: a one-line shim runs the real exe in Wine.
    printf '#!/bin/sh\nexec wine "%s" "$@"\n' "$exe" > "$scratch/target/release/mdedit.exe"
    chmod +x "$scratch/target/release/mdedit.exe"
    bash "$scratch/tests/shots.sh" "$root/target/wine-compare/$name" >/dev/null 2>"$root/target/wine-compare/$name.log"
}
mkdir -p target/wine-compare
# One after the other in the same scratch tree: the recent-files dialog shows paths.
run_set native "$native"
run_set cross "$cross"

same=0 diff=0
for f in target/wine-compare/native/*.png; do
    b="$(basename "$f")"
    if cmp -s "$f" "target/wine-compare/cross/$b"; then same=$((same + 1)); else diff=$((diff + 1)); echo "DIFFERENT: $b"; fi
done
echo "identical: $same, different: $diff, native shots: $(ls target/wine-compare/native | wc -l), cross shots: $(ls target/wine-compare/cross | wc -l)"
