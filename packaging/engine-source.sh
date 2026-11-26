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
# A checkout already at the revision is kept. Needs git and the network (github.com).
set -euo pipefail
dir="${1:?usage: engine-source.sh DIR}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rev="$(awk '
    { sub(/\r$/, "") }
    /^\[/ { renderer = 0 }
    /^name = "mdshared-md-render"$/ { renderer = 1 }
    renderer && /^source = "git\+https:\/\/github\.com\/Kaan88\/markdown-shared-rs\?rev=[0-9a-f]+#[0-9a-f]+"$/ {
        sub(/^.*#/, ""); sub(/"$/, ""); print
    }
' "$root/Cargo.lock")"
[[ "$rev" =~ ^[0-9a-f]{40}$ ]] || { echo "engine-source: no unique shared renderer revision in Cargo.lock" >&2; exit 1; }
if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" != "$rev" ]; then
    rm -rf "$dir"
    git init -q "$dir"
    git -C "$dir" fetch -q --depth 1 https://github.com/Kaan88/markdown-shared-rs "$rev"
    git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
fi
echo "engine source: markdown-shared-rs $rev in $dir"
