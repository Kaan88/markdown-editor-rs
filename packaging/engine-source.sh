#!/usr/bin/env bash
# Checks out Kaan88/markdown-viewer-rs at the revision mdedit pins (the `rev` of mdview-preview in
# bins/mdedit/Cargo.toml) into DIR, for building mdview-engine, the helper process that draws the
# preview pane's formulas and diagrams. The engine is built from the viewer's own workspace, with
# its lockfile and its size-first release profile, so it always matches the renderer mdedit
# carries:
#
#   packaging/engine-source.sh DIR
#   cargo build --release --locked -p mdview-engine --manifest-path DIR/Cargo.toml
#
# A checkout already at the revision is kept. Needs git and the network (github.com).
set -euo pipefail
dir="${1:?usage: engine-source.sh DIR}"
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
rev="$(grep -o 'markdown-viewer-rs", rev = "[0-9a-f]\{40\}' "$root/bins/mdedit/Cargo.toml" | head -1 | grep -o '[0-9a-f]\{40\}$' || true)"
[ -n "$rev" ] || { echo "engine-source: no viewer revision in bins/mdedit/Cargo.toml" >&2; exit 1; }
if [ "$(git -C "$dir" rev-parse HEAD 2>/dev/null || true)" != "$rev" ]; then
    rm -rf "$dir"
    git init -q "$dir"
    git -C "$dir" fetch -q --depth 1 https://github.com/Kaan88/markdown-viewer-rs "$rev"
    git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
fi
echo "engine source: markdown-viewer-rs $rev in $dir"
