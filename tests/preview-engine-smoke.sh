#!/usr/bin/env bash
# Exercise the packaged editor through its launcher, with and without its sibling helper.
# Usage: bash tests/preview-engine-smoke.sh LAUNCHER REAL_BIN_DIR
set -euo pipefail
launcher="${1:?missing editor launcher}"
bin="${2:?missing directory containing the real editor}"
here="$(cd "$(dirname "$0")" && pwd)"
engine="$bin/mdview-engine"
parent_engine="$(dirname "$bin")/mdview-engine"
saved="$bin/mdview-engine.smoke-original"
cleanup() {
    if [ -f "$saved" ]; then mv "$saved" "$engine"; fi
    rm -f "$parent_engine"
    rm -rf "$scratch"
}
test -x "$engine"
for path in "$saved" "$parent_engine"; do
    if [ -e "$path" ] || [ -L "$path" ]; then
        echo "smoke test needs a fresh install tree: $path already exists" >&2
        exit 1
    fi
done
scratch="$(mktemp -d)"
trap cleanup EXIT

cat > "$scratch/input.md" <<'EOF'
# Packaged preview

![Engine probe](probe.svg)
EOF
cat > "$scratch/probe.svg" <<'EOF'
<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80">
  <rect width="120" height="80" fill="#df15ac"/>
</svg>
EOF

# Also catch a helper spawned via stale search rules, even if it draws nothing.
cat > "$parent_engine" <<'EOF'
#!/bin/sh
echo started >> "$MDEDIT_UNEXPECTED_HELPER"
exit 1
EOF
chmod +x "$parent_engine"
export MDEDIT_UNEXPECTED_HELPER="$scratch/unexpected-helper"
for state in with without; do
    if [ "$state" = without ]; then mv "$engine" "$saved"; fi
    bash "$here/isolated.sh" env PATH="$(dirname "$bin"):$PATH" \
        "$launcher" --no-session --behind --light --scale 1.0 --preview-pane \
        --shot "$scratch/$state.png" "$scratch/input.md"
    test ! -e "$MDEDIT_UNEXPECTED_HELPER"
done
py=python
command -v "$py" >/dev/null 2>&1 || py=python3
"$py" "$here/preview-engine-pixels.py" "$scratch/with.png" "$scratch/without.png"
echo "No fallback helper started"
