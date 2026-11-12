# markdown-editor-rs

<img src="assets/mdedit.png" width="96" alt="The mdedit mark: an M and a pencil on a green tile">

`mdedit`: a light, fast text editor with syntax highlighting, the companion of
[markdown-viewer-rs](https://github.com/Kaan88/markdown-viewer-rs) (`mdview`). A Notepad
alternative for personal use that opens any text file, with the same hand-drawn interface as the
viewer.

**Status: phases 1 to 3 are built (viewing, editing, safe saving); the user has not yet looked
at it.** It opens files in tabs, scrolls, selects, finds, highlights syntax, detects encodings and
line endings, edits with undo and redo, and saves in the file's own encoding and line endings
without touching any other byte. There is no New file command yet, and no input-method support.
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
