#!/usr/bin/env bash
# Checks out Kaan88/markdown-shared-rs at the renderer revision resolved in mdedit's Cargo.lock
# into DIR, for building mdview-engine, the helper process that draws the preview pane's
# formulas and diagrams. The engine is built from the toolkit workspace, with
# its lockfile and its size-first release profile, so it always matches the renderer mdedit
# carries:
#
#   packaging/engine-source.sh DIR
#   cargo build --release --locked -p mdshared-md-engine --manifest-path DIR/Cargo.toml
#
# A clean checkout already at the revision is kept. Needs git and the network (github.com).
set -euo pipefail
default_dir="${XDG_CACHE_HOME:-$HOME/.cache}/mdedit/engine-src"
if [ -n "${RUNNER_TEMP:-}" ]; then default_dir="$RUNNER_TEMP/engine-src"; fi
dir="${1:-$default_dir}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
renderer_revision() {
    awk '
    { sub(/\r$/, "") }
    /^\[/ { renderer = 0 }
    /^name = "mdshared-md-render"$/ { renderer = 1 }
    renderer && /^source = "git\+https:\/\/github\.com\/Kaan88\/markdown-shared-rs\?rev=[0-9a-f]+#[0-9a-f]+"$/ {
        sub(/^.*#/, ""); sub(/"$/, ""); print
    }
    ' "$1"
}
rev="$(renderer_revision "$root/Cargo.lock")"
[[ "$rev" =~ ^[0-9a-f]{40}$ ]] || { echo "engine-source: no unique shared renderer revision in Cargo.lock" >&2; exit 1; }

if [ "$dir" = --check-lock ]; then
    other="${2:?usage: engine-source.sh --check-lock VIEWER_LOCK}"
    # Both projects must resolve the same shared renderer, regardless of abbreviated rev spelling.
    if [ "$(renderer_revision "$other")" != "$rev" ]; then
        echo "engine-source: the viewer must use the same toolkit revision ($rev)" >&2
        exit 1
    fi
    exit 0
fi

if [ -L "$dir" ]; then
    echo "engine-source: refusing a symlink at $dir" >&2; exit 1
fi
top="$(git -C "$dir" rev-parse --show-toplevel 2>/dev/null || true)"
if [[ -n "$top" && "$top" -ef "$dir" ]]; then
    dirty="$(git --no-optional-locks -C "$dir" status --porcelain --untracked-files=all --ignored)"
    [ -z "$dirty" ] || { echo "engine-source: dirty or extra files in $dir" >&2; exit 1; }
else
    # A cache restore can leave an empty directory tree, including .git/.
    if [ -e "$dir" ]; then
        [ -d "$dir" ] && [ -z "$(find "$dir" ! -type d -print -quit)" ] ||
            { echo "engine-source: refusing nonempty non-checkout $dir" >&2; exit 1; }
        find "$dir" -depth -type d -exec rmdir {} \;
        [ ! -e "$dir" ] || { echo "engine-source: could not clear empty checkout $dir" >&2; exit 1; }
    fi
    mkdir -p "$(dirname "$dir")"
    git init -q "$dir"
fi
if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" != "$rev" ]; then
    git -C "$dir" fetch -q --depth 1 https://github.com/Kaan88/markdown-shared-rs "$rev"
    git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
fi
echo "engine source: markdown-shared-rs $rev in $dir"
