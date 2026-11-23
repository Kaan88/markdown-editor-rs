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
shot column --light --select-column 3:5-12:14 "$fx/sample.rs"
shot column-dark --dark --select-column 3:5-12:14 "$fx/sample.rs"
shot column-cjk --light --select-column 1:3-5:9 "$fx/cjk.txt"
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
shot table-md --light --goto 6:4 "$fx/table.md"
shot table-md-dark --dark --goto 7:12 "$fx/table.md"
shot menu-text-markdown-table --light --goto 6:4 --menu text-markdown "$fx/table.md"
shot menu-text-markdown-table-dark --dark --goto 6:4 --menu text-markdown "$fx/table.md"
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
shot results-focus --light --find self --find-in-tabs --results-focus "$fx/sample.rs" "$fx/sample.md"
shot results-scope --light --folder "$fx" --find the --find-in-files --results-kinds "*.md;*.txt" "$fx/sample.md"
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
shot recovered-light --light --goto 5:1 --type "edited " --demo-recovered 2 "$fx/sample.rs"
shot recovered-dark --dark --goto 5:1 --type "edited " --demo-recovered 1 "$fx/sample.rs"
shot update-available --light --update-bar available "$fx/sample.rs"
shot update-installing --dark --update-bar installing "$fx/sample.rs"
shot update-installed --dark --update-bar installed
shot update-installed-plain --light --update-bar installed-plain

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
# Settings, a page at a time (docs/settings-redesign.md on mock/settings): every page in both
# themes at 1280x800, the AI pages with a custom provider and the answers the engine would give
# (--ai-demo), and the tallest pages at the smallest window, 520x360.
custom=(--pref ai_connection=openai --pref ai_preset=custom --pref "ai_custom_0_name=LM Studio (local)" --pref ai_custom_0_url=http://localhost:1234/v1)
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    for page in appearance editor indentation typing notes-library tabs updates; do
        shot "settings-$page$t" $th --dialog settings --settings-section "$page" "$fx/sample.rs"
    done
    shot "settings-ai-service$t" $th "${custom[@]}" --ai-demo settings "$fx/sample.rs"
    shot "settings-ai-rewrite$t" $th --pref ai_connection=claude-api --ai-demo settings --settings-section ai-rewrite "$fx/sample.rs"
done
# The other states: the tab rules on (the rows under them enabled), the connection off (AI
# rewrite's rows disabled with the reason), Claude Code found, a model list that failed with a
# model typed, and the window backdrop set to Mica (a shot has no window, so the chrome is flat).
shot settings-tabs-on --light --pref skip_brief=1 --dialog settings --settings-section tabs "$fx/sample.rs"
shot settings-ai-rewrite-off --light --dialog settings --settings-section ai-rewrite "$fx/sample.rs"
shot settings-ai-service-cli --light --pref ai_connection=claude-code --ai-demo settings "$fx/sample.rs"
shot settings-ai-rewrite-error --light "${custom[@]}" --pref ai_custom_0_model=gpt-4.1-mini --ai-demo settings-error --settings-section ai-rewrite "$fx/sample.rs"
shot settings-appearance-mica --dark --pref backdrop=mica --dialog settings "$fx/sample.rs"
shot settings-scale --light --scale 1.5 --dialog settings --settings-section editor "$fx/sample.rs"
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "settings-min-tabs$t" $th --size 520x360 --dialog settings --settings-section tabs "$fx/sample.rs"
    shot "settings-min-ai-service$t" $th --size 520x360 "${custom[@]}" --ai-demo settings "$fx/sample.rs"
done
shot settings-min-editor --light --size 520x360 --dialog settings --settings-section editor "$fx/sample.rs"
shot settings-min-appearance --light --size 520x360 --dialog settings "$fx/sample.rs"
shot settings-min-ai-rewrite-error --light --size 520x360 "${custom[@]}" --pref ai_custom_0_model=gpt-4.1-mini --ai-demo settings-error --settings-section ai-rewrite "$fx/sample.rs"
shot settings-min-scale2 --light --size 520x360 --scale 2 --dialog settings --settings-section tabs "$fx/sample.rs"
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
# Copy a change across: the caret in the first difference, then its lines go to the other side (or
# come from it) and the comparison is made again; the View menu with the caret in a difference.
shot compare-copied --light --next-difference 1 --copy-change across --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot compare-taken-dark --dark --next-difference 1 --copy-change take --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot menu-view-compared --light --menu view --next-difference 1 --compare "$fx/compare-a.rs" "$fx/compare-b.rs"
shot menu-view-compared-dark --dark --menu view --next-difference 1 --compare "$fx/compare-a.rs" "$fx/compare-b.rs"

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

# Preview in mdview: the View menu no longer lists the preview (issue #7), the tab menu of a
# Markdown file says what a click does (a file of another language lists the preview only while it
# is previewed), and the mark of a previewed document.
shot menu-view-md --light --menu view "$fx/sample.md"
shot menu-view-md-linked-dark --dark --demo-link --menu view "$fx/sample.md"
shot menu-view-rs-linked --light --demo-link --menu view "$fx/sample.rs"
shot menu-tab-md --light --menu tab "$fx/sample.md" "$fx/sample.rs"
shot menu-tab-md-linked-dark --dark --demo-link --menu tab "$fx/sample.md" "$fx/sample.rs"
shot previewing --light --demo-link --goto 12:3 "$fx/sample.md"
shot previewing-dark --dark --demo-link --goto 12:3 "$fx/sample.md"
# A document that has no file is previewed too: the commands are listed for it, and it carries
# the mark while mdview shows its text.
shot menu-view-untitled --light --new --menu view
shot menu-tab-untitled-linked-dark --dark --new --demo-link --menu tab "$fx/sample.md"
shot previewing-untitled --light --new --demo-link --type $'# Draft\n\nNot saved yet.\n'
shot previewing-untitled-dark --dark --new --demo-link --type $'# Draft\n\nNot saved yet.\n'

# Issues #7 and #8. The Preview button's arrow: the three modes to open once, then the button's
# own as a radio (drawn as with mdview found); mdview chosen for the button. The bar with New,
# Save (lit by an edit) and AI edit while the AI is on, the AI menu under AI edit, and the bar at
# 900 and 560 px. The settings rows: the preview mode on Editor, the rich paste on Typing, both at
# 520x360 too.
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "preview-menu$t" $th --menu preview "$fx/sample.md"
    shot "preview-menu-mdview$t" $th --pref preview_mode=mdview --menu preview "$fx/sample.md"
    shot "bar-new-save-ai$t" $th --pref ai_connection=claude-api --type x "$fx/sample.md"
    shot "bar-ai-menu$t" $th --pref ai_connection=claude-api --menu bar-ai "$fx/sample.md"
    shot "bar-900$t" $th --size 900x500 --pref ai_connection=claude-api --type x "$fx/sample.md"
    shot "bar-560$t" $th --size 560x500 --pref ai_connection=claude-api --type x "$fx/sample.md"
    shot "settings-editor-preview$t" $th --dialog settings --settings-section editor "$fx/sample.md"
    shot "settings-typing-rich$t" $th --pref paste_rich=true --dialog settings --settings-section typing "$fx/sample.md"
    shot "settings-min-typing$t" $th --size 520x360 --dialog settings --settings-section typing "$fx/sample.md"
    # The Preview button lit while its own mode's preview is on: in mdview, and side by side;
    # the arrow then offers to stop it. The empty screen's hint in the smallest text area.
    shot "preview-lit-mdview$t" $th --pref preview_mode=mdview --demo-link "$fx/sample.md"
    shot "preview-lit-side$t" $th --pref preview_mode=side-by-side --demo-link "$fx/sample.md"
    shot "preview-menu-open-mdview$t" $th --pref preview_mode=mdview --demo-link --menu preview "$fx/sample.md"
    shot "preview-menu-open-side$t" $th --pref preview_mode=side-by-side --demo-link --menu preview "$fx/sample.md"
    shot "empty-min-files$t" $th --size 520x400 --files
done
shot settings-min-editor-dark --dark --size 520x360 --dialog settings --settings-section editor "$fx/sample.rs"

# Emoji presentation: the first line is colour clusters, the second the bare characters (text
# glyphs); the caret after the keycap on line 3 must sit right after it.
shot emoji --light "$fx/emoji.txt"
shot emoji-dark --dark "$fx/emoji.txt"
shot emoji-caret --light --goto 3:11 "$fx/emoji.txt"

# Indentation follows the file: the field in the status strip, its tooltip, and where the strip is
# too crowded for it (the tooltip then sits on the language field).
shot status-indent-tip --light --size 720x400 --hover 555,387 "$fx/sample.rs"
shot status-indent-crowded --light --size 520x400 --zoom 150 --demo-link --select 1:1-30:1 --hover 340,387 "$fx/sample.md"

# Word completion and snippets: the list under the caret (words of the file, then a Markdown
# snippet beside its words), the first snippet taken with its stop selected, the list above the caret
# when there is no room below, and the setting.
shot complete-words --light --goto 8:4 --menu complete "$fx/complete.md"
shot complete-words-dark --dark --goto 8:4 --menu complete "$fx/complete.md"
shot complete-snippet --light --goto 10:4 --menu complete "$fx/complete.md"
shot complete-snippet-dark --dark --goto 10:4 --menu complete "$fx/complete.md"
shot complete-snippet-taken --light --goto 10:4 --menu complete-snippet "$fx/complete.md"
shot complete-snippet-taken-dark --dark --goto 10:4 --menu complete-snippet "$fx/complete.md"
shot complete-above --light --size 900x340 --goto 8:4 --menu complete "$fx/complete.md"

# The window as the client-drawn margin shows it (--frame): the shadow and the rounded corners over
# a flat backdrop, at scale 1 and 2 in both themes, and the other two choices of Window corners.
# The Settings dialog's Appearance page has the row.
shot frame-round-light --light --size 520x300 --frame round "$fx/sample.rs"
shot frame-round-dark --dark --size 520x300 --frame round "$fx/sample.rs"
shot frame-round-light-2x --light --size 520x300 --scale 2 --frame round "$fx/sample.rs"
shot frame-round-dark-2x --dark --size 520x300 --scale 2 --frame round "$fx/sample.rs"
shot frame-small-light --light --size 520x300 --frame small "$fx/sample.rs"
shot frame-square-light --light --size 520x300 --frame square "$fx/sample.rs"

# The plan screen (docs/ai-library-plan.md 5.3), from documents built in the program, in both
# themes: headings with every line in a note, calm gaps, the rest ticked and selected, a warning and
# a moved boundary, the same file split before, the name field open, 160 notes (the density strip),
# and 800x600 (no preview).
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "plan-split$t" $th --plan-demo split
    shot "plan-split-gaps$t" $th --plan-demo split-gaps
    shot "plan-split-rest$t" $th --plan-demo split-rest
    shot "plan-split-warn$t" $th --plan-demo split-warn
    shot "plan-split-again$t" $th --plan-demo split-again
    shot "plan-split-rename$t" $th --plan-demo split-rename
    shot "plan-split-many$t" $th --plan-demo split-many
    shot "plan-split-small$t" $th --size 800x600 --plan-demo split-small
done

# The original in the Recycle Bin (5.4): the box offered, ticked with its sentence and the footer's,
# wanted while lines are in no note (the strip's gaps in the warning colour, and the one-click
# answer), and each reason the window can draw: a drive with no bin, a run of more than one
# document, a bin the system does not offer. Then the report of a split that put its original in
# the bin, still listed in a group, and of one whose original stayed, with the reason.
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "plan-split-bin-offered$t" $th --plan-demo split-bin-offered
    shot "plan-split-bin$t" $th --plan-demo split-bin
    shot "plan-split-bin-gaps$t" $th --plan-demo split-bin-gaps
    shot "plan-split-bin-wanted$t" $th --plan-demo split-bin-wanted
    shot "plan-split-bin-drive$t" $th --plan-demo split-bin-drive
    shot "plan-split-bin-several$t" $th --plan-demo split-bin-several
    shot "plan-split-bin-system$t" $th --plan-demo split-bin-system
    shot "plan-split-bin-report$t" $th --plan-demo split-bin-report
    shot "plan-split-bin-kept$t" $th --plan-demo split-bin-kept
done

# Split by AI (5.2), with a connection chosen and a made-up answer (no request is made): the panel
# with split prompts, the line while it asks, a plan from the AI with a boundary moved to a
# paragraph's edge and an overlap, 240 notes of which 54 are dropped, an answer that could not be
# read, and a document too long to send.
ai="--pref ai_connection=claude-api --pref ai_model_claude-api=claude-opus-5-5 --pref ai_effort_claude-api=high"
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "plan-split-ai-ask$t" $th $ai --plan-demo split-ai-ask
    shot "plan-split-ai-asking$t" $th $ai --plan-demo split-ai-asking
    shot "plan-split-ai$t" $th $ai --plan-demo split-ai
    shot "plan-split-ai-dropped$t" $th $ai --plan-demo split-ai-dropped
    shot "plan-split-ai-unreadable$t" $th $ai --plan-demo split-ai-unreadable
    shot "plan-split-ai-long$t" $th $ai --plan-demo split-ai-long
done

# "AI runs…" (5.9): the list, the question before undoing a run, and the question when the run's
# source is in the Recycle Bin (that one is never a question: it says what to put back).
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "runs$t" $th --dialog runs
    shot "runs-undo$t" $th --dialog run-undo
    shot "runs-undo-recycled$t" $th --dialog run-undo-recycled
    # Undo refuses when the original the notes were cut from is gone or has changed, and a record
    # of a later version is listed with nothing to press.
    shot "runs-undo-original$t" $th --dialog run-undo-original
    shot "runs-newer$t" $th --dialog runs-newer
done

# The "Plan ready" row at the foot of the Notes pane (5.3): a plan Escape kept, with Show and Drop.
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "plan-ready$t" $th --plan-demo split --plan-parked
done

# "Move the notes…" (5.12), from a plan made in the program: the card with ready rows (notes that
# carry pictures and rewrite links, D-091), a taken name, a picture whose name is taken, a note
# listed twice, a link and a note outside the library; the same card at 800x600; and the results
# panel after a move, with the files that went with their notes and the links rewritten.
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "move-notes$t" $th --move-demo move
    shot "move-notes-small$t" $th --size 800x600 --move-demo move
    shot "move-results$t" $th --move-demo results
done

# Dragging a note in the notes pane (D-130): the note held, its chip at the pointer, and the
# heading it would go into marked, in both themes.
for t in "" "-dark"; do
    th=--light
    [ -n "$t" ] && th=--dark
    shot "library-drag$t" $th --library-demo drag
done

ls "$out" | wc -l
