#!/usr/bin/env bash
# Renders mdedit's screenshot set with --shot into the directory given as the only argument.
# Run it twice into two directories and compare: every PNG must be byte-identical.
#
#   tests/shots.sh target/shots/a
#
# Needs a release build (cargo build --release -p mdedit). The large files are made under
# target/data on the first run and never committed. Every shot uses --no-session, so the user's
# session is neither read nor written.

set -euo pipefail

out="${1:?usage: tests/shots.sh <output-dir>}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
exe="target/release/mdedit.exe"
[ -x "$exe" ] || exe="target/release/mdedit"
fx="fixtures"
data="target/data"
mkdir -p "$out" "$data"

py=python
command -v "$py" >/dev/null 2>&1 || py=python3

# Large inputs, made once.
if [ ! -f "$data/100mb.txt" ]; then
    cargo run --release -q -p mdedit-core --example gen -- lines 100 "$data/100mb.txt"
fi
if [ ! -f "$data/1mb-line.txt" ]; then
    cargo run --release -q -p mdedit-core --example gen -- long 1 "$data/1mb-line.txt"
fi
if [ ! -f "$data/50k.rs" ]; then
    # sample.rs repeated until exactly 50,000 lines.
    "$py" - "$fx/sample.rs" "$data/50k.rs" <<'EOF'
import sys
src = open(sys.argv[1], encoding="utf-8").read().splitlines()
lines = []
while len(lines) < 50000:
    lines.extend(src)
open(sys.argv[2], "w", encoding="utf-8", newline="\n").write("\n".join(lines[:50000]) + "\n")
EOF
fi

shot() {
    local name="$1"
    shift
    "$exe" --no-session --shot "$out/$name.png" "$@"
}

shot rs-light --light "$fx/sample.rs"
shot rs-dark --dark "$fx/sample.rs"
shot md-light --light "$fx/sample.md"
shot toml-dark --dark "$fx/sample.toml"
shot py-light --light "$fx/sample.py"
shot empty-light --light
shot empty-dark --dark
shot utf16le --light "$fx/utf16le.txt"
shot utf16be --dark "$fx/utf16be.txt"
shot cp1254 --light "$fx/cp1254.txt"
shot mixed --light "$fx/mixed.txt"
shot tabs-controls --light "$fx/tabs-controls.txt"
shot invalid-utf16 --light "$fx/invalid-utf16.txt"
shot cjk --light "$fx/cjk.txt"
shot rs50k-goto --dark --goto 40000 "$data/50k.rs"
shot big-goto --light --goto 1000000 "$data/100mb.txt"
shot long-hscroll --light --hscroll 400000 "$data/1mb-line.txt"
shot select --light --select 3:5-12:10 "$fx/sample.rs"
shot find --light --find self "$fx/sample.rs"
shot find-dark --dark --find ring "$fx/sample.rs"
shot menu-main --light --menu main "$fx/sample.rs"
shot menu-recent --light --menu recent "$fx/sample.rs" "$fx/sample.md" "$fx/sample.toml"
shot menu-tab --dark --menu tab "$fx/sample.rs" "$fx/sample.md"
shot menu-text --light --select 9:1-10:20 --menu text "$fx/sample.rs"
shot menu-find --light --find ring --menu find "$fx/sample.rs"
shot replace --light --find self --whole-word --replace this "$fx/sample.rs"
shot replace-dark --dark --find '(\w+)_(\w+)' --regex --match-case --replace '$2_$1' "$fx/sample.rs"
shot replace-bad-pattern --light --find '(self' --regex --replace x "$fx/sample.rs"
shot replace-narrow --light --size 520x600 --find self --replace this "$fx/sample.rs"
shot zoom --light --zoom 150 --goto 30:9 "$fx/sample.rs"
shot dialog-about --dark --dialog about "$fx/sample.rs"
shot dialog-shortcuts --light --dialog shortcuts "$fx/sample.rs"
shot dialog-recent --light --dialog recent "$fx/sample.rs" "$fx/sample.md" "$fx/cjk.txt"
shot hover-open --light --hover 46,63 "$fx/sample.rs"
shot scale-150 --light --scale 1.5 "$fx/sample.rs"
shot small --light --size 520x360 "$fx/sample.rs"
shot files-pane --light --files --folder "$fx" "$fx/sample.rs"
shot tabs --dark "$fx/sample.md" "$fx/sample.rs" "$fx/sample.toml" "$fx/sample.py" \
    "$fx/turkish.txt" "$fx/cjk.txt" "$fx/crlf.txt"
shot typed --light --goto 5:1 --type "edited " "$fx/sample.rs"
shot dialog-unsaved --dark --goto 5:1 --type "edited " --dialog unsaved "$fx/sample.rs"
shot update-available --light --update-bar available "$fx/sample.rs"
shot update-installing --dark --update-bar installing "$fx/sample.rs"
shot update-installed --dark --update-bar installed

ls "$out" | wc -l
