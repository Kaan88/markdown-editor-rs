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
t="${CARGO_TARGET_DIR:-target}"
exe="$t/release/mdedit.exe"
[ -x "$exe" ] || exe="$t/release/mdedit"
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
    # The scale is pinned, so the system's display scale cannot change the pictures; a shot
    # that asks for its own --scale comes later on the line and wins.
    "$exe" --no-session --scale 1.0 --shot "$out/$name.png" "$@"
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
shot menu-text-dark --dark --select 9:1-10:20 --menu text "$fx/sample.rs"
shot menu-text-lines --light --select 9:1-10:20 --menu text-lines "$fx/sample.rs"
shot menu-text-lines-dark --dark --select 9:1-10:20 --menu text-lines "$fx/sample.rs"
shot menu-text-convert --light --select 9:1-10:20 --menu text-convert "$fx/sample.rs"
shot menu-text-convert-dark --dark --select 9:1-10:20 --menu text-convert "$fx/sample.rs"
shot menu-text-markdown --light --select 3:1-3:30 --menu text-markdown "$fx/sample.md"
shot menu-text-markdown-dark --dark --select 3:1-3:30 --menu text-markdown "$fx/sample.md"
shot menu-text-lines-left --light --select 9:1-10:20 --menu text-lines --menu-at 0.0 "$fx/sample.rs"
shot menu-text-lines-mid --light --select 9:1-10:20 --menu text-lines --menu-at 0.45 "$fx/sample.rs"
shot menu-text-lines-right --light --select 9:1-10:20 --menu text-lines --menu-at 0.97 "$fx/sample.rs"
shot menu-text-convert-left --light --select 9:1-10:20 --menu text-convert --menu-at 0.0 "$fx/sample.rs"
shot menu-text-convert-mid --light --select 9:1-10:20 --menu text-convert --menu-at 0.45 "$fx/sample.rs"
shot menu-text-convert-right --light --select 9:1-10:20 --menu text-convert --menu-at 0.97 "$fx/sample.rs"
shot menu-text-markdown-left --light --select 3:1-3:30 --menu text-markdown --menu-at 0.0 "$fx/sample.md"
shot menu-text-markdown-mid --light --select 3:1-3:30 --menu text-markdown --menu-at 0.45 "$fx/sample.md"
shot menu-text-markdown-right --light --select 3:1-3:30 --menu text-markdown --menu-at 0.97 "$fx/sample.md"
shot menu-text-md --light --select 3:1-3:30 --menu text "$fx/sample.md"
shot menu-find --light --find ring --menu find "$fx/sample.rs"
shot replace --light --find self --whole-word --replace this "$fx/sample.rs"
shot replace-dark --dark --find '(\w+)_(\w+)' --regex --match-case --replace '$2_$1' "$fx/sample.rs"
shot replace-bad-pattern --light --find '(self' --regex --replace x "$fx/sample.rs"
shot replace-narrow --light --size 520x600 --find self --replace this "$fx/sample.rs"
shot replace-selection --light --select 27:1-33:1 --find self --replace this "$fx/sample.rs"
shot replace-selection-dark --dark --select 27:1-33:1 --find self --replace this "$fx/sample.rs"
shot replace-selection-none --light --select 8:1-16:1 --find self --replace this "$fx/sample.rs"
shot typing-pairs --light --goto 8:1 --type-keys 'let v = vec![(1, "a' "$fx/sample.rs"
shot typing-pairs-dark --dark --goto 8:1 --type-keys 'let v = vec![(1, "a' "$fx/sample.rs"
shot typing-enter-block --light --goto 8:1 --type-keys $'fn f() {\n' "$fx/sample.rs"
shot zoom --light --zoom 150 --goto 30:9 "$fx/sample.rs"
shot bracket --light --goto 17:41 "$fx/sample.rs"
shot twins-dark --dark --select 28:19-28:23 "$fx/sample.rs"
shot whitespace --light --whitespace "$fx/tabs-controls.txt"
shot wrap --light --wrap --size 700x600 "$fx/sample.md"
shot bookmarks --light --bookmark 3 --bookmark 12 --goto 12:1 "$fx/sample.rs"
shot fold --light --fold 10 --fold 27 --goto 5:1 "$fx/sample.rs"
shot fold-dark --dark --wrap --whitespace --size 520x600 --fold 10 --fold 17 "$fx/sample.rs"
shot results --light --find self --find-in-tabs "$fx/sample.rs" "$fx/sample.md"
shot results-dark --dark --find the --find-in-tabs "$fx/sample.md" "$fx/sample.rs"
shot outline --light --outline --goto 30:1 "$fx/sample.rs"
shot outline-dark --dark --outline "$fx/sample.md"
shot outline-md --light --outline --goto 20:1 "$fx/sample.md"
shot wrap-dark --dark --wrap --whitespace --size 520x600 --goto 12:40 "$fx/sample.rs"
shot whitespace-dark --dark --whitespace "$fx/mixed.txt"
shot menu-view --light --menu view "$fx/sample.rs"
shot menu-macro --light --menu macro "$fx/sample.rs"
shot menu-main-view --light --menu main-view "$fx/sample.rs"
shot menu-main-macro-dark --dark --menu main-macro "$fx/sample.rs"
shot split --light --split --goto 12:5 "$fx/sample.rs" "$fx/sample.md"
shot split-dark --dark --split --wrap --size 900x600 "$fx/sample.md" "$fx/sample.rs"
shot minimap --light --minimap "$fx/sample.rs"
shot minimap-dark --dark --minimap "$fx/sample.md"
shot minimap-long --light --minimap --goto 25000 "$data/50k.rs"
shot minimap-fold --light --minimap --fold 10 --fold 27 --goto 5:1 "$fx/sample.rs"
shot minimap-split --dark --minimap --split --goto 12:5 "$fx/sample.rs" "$fx/sample.md"
shot minimap-marks --dark --minimap --select 9:1-14:3 --find self --bookmark 7 --bookmark 20 --goto 12:5 "$fx/sample.rs"
shot minimap-wrap --light --minimap --wrap --size 900x600 --goto 12:40 "$fx/sample.md"
shot minimap-scale-150 --light --minimap --scale 1.5 --find self "$fx/sample.rs"
shot menu-eol --light --menu eol "$fx/sample.rs"
shot menu-encoding --dark --menu encoding "$fx/sample.rs"
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

# Menus with their icons, both themes, checked items.
shot menu-main-dark --dark --menu main "$fx/sample.rs"
shot menu-view-checked-dark --dark --wrap --whitespace --minimap --menu view "$fx/sample.rs"
shot menu-view-checked --light --wrap --whitespace --minimap --menu view "$fx/sample.rs"
shot menu-macro-rec --dark --menu macro-rec "$fx/sample.rs"
shot menu-minimap --light --minimap --menu minimap "$fx/sample.rs"
shot menu-find-opts --dark --find ring --match-case --regex --menu find "$fx/sample.rs"
shot menu-find-opts-light --light --find ring --whole-word --menu find "$fx/sample.rs"
shot menu-recent-dark --dark --menu recent "$fx/sample.rs" "$fx/sample.md" "$fx/sample.toml"
shot menu-eol-dark --dark --menu eol "$fx/sample.rs"
shot menu-encoding-light --light --menu encoding "$fx/sample.rs"

# Split view: the second pane's tab marked, a dragged divider (not half), the second pane at the left.
shot split-ratio --dark --split --split-ratio 0.35 --hover 441,300 --goto 12:5 "$fx/sample.rs" "$fx/sample.md"
shot split-left --light --split --split-left --split-ratio 0.62 "$fx/sample.rs" "$fx/sample.md"

# Macros: saved ones in the menu, the dialog that deletes them, the box that asks a name.
shot menu-macro-saved --light --demo-macros --menu macro "$fx/sample.rs"
shot menu-macro-saved-dark --dark --demo-macros --menu macro "$fx/sample.rs"
shot dialog-macros --dark --demo-macros --dialog macros "$fx/sample.rs"
shot dialog-macros-light --light --demo-macros --dialog macros --size 700x400 "$fx/sample.rs"
shot settings --light --dialog settings "$fx/sample.rs"
shot settings-dark --dark --dialog settings "$fx/sample.rs"
shot settings-scale --light --scale 1.5 --dialog settings "$fx/sample.rs"
shot settings-narrow --light --size 560x640 --dialog settings "$fx/sample.rs"
# The Preview section (the live preview row and its help line), scrolled into view.
shot settings-preview --light --dialog settings --settings-section Preview "$fx/sample.rs"
shot settings-preview-dark --dark --dialog settings --settings-section Preview "$fx/sample.rs"
shot settings-preview-scale --light --scale 1.5 --dialog settings --settings-section Preview "$fx/sample.rs"
shot settings-preview-narrow --light --size 560x640 --dialog settings --settings-section Preview "$fx/sample.rs"
shot pref-tab8 --light --pref tab_width=8 "$fx/tabs-controls.txt"
shot pref-no-numbers --light --pref line_numbers=0 --pref highlight_line=0 --goto 12:5 "$fx/sample.rs"
shot pref-font18 --light --pref font_size=18 "$fx/sample.rs"
shot pref-family --light --pref "font_family=DejaVu Sans Mono" "$fx/sample.rs"
shot pref-family-unknown --light --pref "font_family=No Such Font 123" "$fx/sample.rs"
shot macro-name-prompt --light --demo-macros --macro-prompt "Bullet the line" "$fx/sample.rs"

# Fold chevrons: the pointer in the gutter (faint chevrons, one hovered), a folded block, both themes.
shot fold-gutter --light --hover 46,263 "$fx/sample.rs"
shot fold-gutter-dark --dark --hover 46,263 --fold 27 "$fx/sample.rs"
shot fold-gutter-wrap --light --wrap --size 700x500 --hover 46,264 "$fx/sample.md"
shot fold-box-hover --light --fold 10 --hover 230,263 "$fx/sample.rs"

# A menu taller than its window: kept below the title row, scrolling, and scrolled to its end.
shot menu-tall-main --light --size 1000x520 --menu main "$fx/sample.rs"
shot menu-tall-main-end --light --size 1000x520 --menu main-end "$fx/sample.rs"

# The command palette: empty (open tabs), commands, go to line, symbols, both themes.
shot palette-empty --light --palette "" "$fx/sample.rs" "$fx/sample.md"
shot palette-empty-dark --dark --palette "" "$fx/sample.rs" "$fx/sample.md"
shot palette-commands --light --palette ">fold" "$fx/sample.rs"
shot palette-commands-dark --dark --palette ">fold" "$fx/sample.rs"
shot palette-line --light --palette ":120" "$fx/sample.rs"
shot palette-line-dark --dark --palette ":120" "$fx/sample.rs"
shot palette-symbols --light --palette "@" "$fx/sample.rs"
shot palette-symbols-dark --dark --palette "@" "$fx/sample.rs"

# Compare two files: the two versions of compare-*.rs, scrolled to a change, with the minimap, and
# a file against its saved text.
shot compare --light --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot compare-dark --dark --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot compare-scrolled --light --minimap --next-difference 5 --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot compare-scrolled-dark --dark --minimap --next-difference 4 --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot compare-saved --dark --minimap --type "// edited " --compare-saved "$fx/compare-a.rs"
shot compare-saved-light --light --type "// edited " --compare-saved "$fx/compare-a.rs"
shot menu-view-compare --light --menu view "$fx/sample.rs" "$fx/sample.md"

# Replace in files, in a folder made fresh for each shot (the report shot really replaces).
rfolder() {
    rm -rf "$data/rfolder"
    mkdir -p "$data/rfolder/sub"
    cp "$fx/sample.rs" "$fx/sample.py" "$fx/sample.md" "$data/rfolder/"
    cp "$fx/sample.rs" "$data/rfolder/sub/ring.rs"
}
rfolder
shot replace-files-panel --light --folder "$data/rfolder" --find self --find-in-files --results-replace this "$data/rfolder/sample.rs"
rfolder
shot replace-files-dialog --light --folder "$data/rfolder" --find self --find-in-files --results-replace this --replace-dialog "$data/rfolder/sample.rs"
rfolder
shot replace-files-dialog-dark --dark --folder "$data/rfolder" --find self --find-in-files --results-replace this --replace-dialog "$data/rfolder/sample.rs"
rfolder
shot replace-files-report --light --folder "$data/rfolder" --find self --find-in-files --results-replace this --replace-report "$data/rfolder/sample.rs"
rm -rf "$data/rfolder"

# One file in both panes: each pane its own caret, selection, scroll, folds and minimap.
shot same-file-split --light --minimap --split-same --select 9:1-14:3 --other-fold 16 --other-goto 70:1 "$fx/sample.rs"
shot same-file-split-dark --dark --minimap --split-same --goto 3:1 --other-goto 40:1 --other-scroll 120 "$fx/sample.rs"
shot same-file-split-left --light --split-same --split-left --split-ratio 0.55 --wrap --size 900x600 --other-goto 60:1 "$fx/sample.md"
shot same-file-split-find --dark --minimap --split-same --find self --other-goto 60:1 "$fx/sample.rs"

# Preview in mdview: the commands in the View menu and the tab menu of a Markdown file (a file of
# another language lists them only while it is previewed), and the mark of a previewed document.
shot menu-view-md --light --menu view "$fx/sample.md"
shot menu-view-md-linked-dark --dark --demo-link --menu view "$fx/sample.md"
shot menu-view-rs-linked --light --demo-link --menu view "$fx/sample.rs"
shot menu-tab-md --light --menu tab "$fx/sample.md" "$fx/sample.rs"
shot menu-tab-md-linked-dark --dark --demo-link --menu tab "$fx/sample.md" "$fx/sample.rs"
shot previewing --light --demo-link --goto 12:3 "$fx/sample.md"
shot previewing-dark --dark --demo-link --goto 12:3 "$fx/sample.md"

ls "$out" | wc -l
