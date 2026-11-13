# markdown-editor-rs

<img src="assets/mdedit.png" width="96" alt="The mdedit mark: an M and a pencil on a green tile">

`mdedit`: a light, fast text editor with syntax highlighting, the companion of
[markdown-viewer-rs](https://github.com/Kaan88/markdown-viewer-rs) (`mdview`). A Notepad
alternative for personal use that opens any text file, with the same hand-drawn interface as the
viewer.

**Status: phases 1 to 3 are built (viewing, editing, safe saving); the user has tried it once
and approved nothing yet.** It opens files in tabs, scrolls, selects, finds, highlights syntax, detects encodings and
line endings, edits with undo and redo, and saves in the file's own encoding and line endings
without touching any other byte. New file (Ctrl+N) opens an untitled tab that Save asks a name
for. It has the line commands (duplicate, move, delete, toggle comment, change case, trim, tabs to
spaces), finds and replaces with match case, whole word and regular expressions (Ctrl+F, Ctrl+H),
changes a file's line endings and encoding from the status bar, wraps long lines when asked
(Alt+Z), edits at several carets (Ctrl+click) and column selections (Alt+drag, Alt+Shift+arrows),
keeps unsaved text when the window closes and puts it back at the next start, and updates itself
in place like mdview (v0.1.0 is the first release). There is no input-method
support yet.
[docs/PLAN.md](docs/PLAN.md) holds the goals, the budgets, the phases and the feature roadmap;
[docs/DESIGN.md](docs/DESIGN.md) the design (section 16 for editing and saving).

The painter, widgets, text engine and highlighting come from
[markdown-shared-rs](https://github.com/Kaan88/markdown-shared-rs), pinned to a commit in
`Cargo.toml`.

## Build

```powershell
cargo build --release --workspace
cargo test --release --workspace
target\release\mdedit.exe README.md
```

`bash tests/shots.sh <dir>` renders 33 interface states to PNG without a window.

The mark in `assets/` is drawn by `assets/make_icon.py` in the viewer's repository
(`python assets/make_icon.py mdedit`), which holds the shapes of both marks.
