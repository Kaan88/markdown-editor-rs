#!/usr/bin/env bash
# Check the source guard without mutating the checkout that will be built.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
src="${1:?usage: engine-source-check.sh CLEAN_ENGINE_CHECKOUT}"
scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
cp -a "$src" "$scratch/source"
script="$root/packaging/engine-source.sh"
lock="$root/Cargo.lock"
bash "$script" "$scratch/source"
bash "$script" --check-lock "$lock"
sed 's/#[0-9a-f]\{40\}"/#0000000000000000000000000000000000000000"/g' "$lock" > "$scratch/mismatch.lock"
if bash "$script" --check-lock "$scratch/mismatch.lock" > "$scratch/log" 2>&1; then
    echo "accepted a viewer lock at a different revision" >&2
    exit 1
fi
grep -Fq 'viewer must use the same toolkit revision' "$scratch/log"

reject_dirty() {
    if bash "$script" "$scratch/source" > "$scratch/log" 2>&1; then
        echo "accepted a dirty engine checkout" >&2
        exit 1
    fi
    grep -Fq 'dirty or extra files' "$scratch/log"
}
touch "$scratch/source/untracked-probe"
reject_dirty
rm "$scratch/source/untracked-probe"
printf '\n# Test edit\n' >> "$scratch/source/Cargo.toml"
reject_dirty
echo 'Engine source guard rejected a mismatched lock, untracked file, and tracked edit'

# Reproduce a cache restore under an enclosing repository. Redirect fetches
# to the already checked-out toolkit, so the regression needs no network.
origin="$(cd "$scratch/source" && { pwd -W 2>/dev/null || pwd -P; })"
mkdir "$scratch/source/target"
for restored in git-shell empty; do
    dest="$scratch/source/target/$restored"
    mkdir "$dest"
    if [ "$restored" = git-shell ]; then mkdir "$dest/.git"; fi
    GIT_CONFIG_COUNT=1 \
        GIT_CONFIG_KEY_0="url.$origin.insteadOf" \
        GIT_CONFIG_VALUE_0=https://github.com/Kaan88/markdown-shared-rs \
        bash "$script" "$dest"
    test "$(git -C "$dest" rev-parse HEAD)" = "$(git -C "$scratch/source" rev-parse HEAD)"
    test -f "$dest/crates/mdshared-md-engine/Cargo.toml"
done
echo 'Engine source guard recovered empty cache directories inside a repository'
