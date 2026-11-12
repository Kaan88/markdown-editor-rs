# markdown-editor-rs

<img src="assets/mdedit.png" width="96" alt="The mdedit mark: an M and a pencil on a green tile">

`mdedit`: a light, fast text editor with syntax highlighting, the companion of
[markdown-viewer-rs](https://github.com/Kaan88/markdown-viewer-rs) (`mdview`). A Notepad
alternative for personal use that opens any text file, with the same hand-drawn interface as the
viewer.

**Status: phase 1 (read-only) is built; the user has not yet looked at it.** It opens files in
tabs, scrolls, selects, copies, finds, highlights syntax and detects encodings and line endings.
Typing does nothing yet: editing and saving are phases 2 and 3.
[docs/PLAN.md](docs/PLAN.md) holds the goals, the budgets, the phases and the feature roadmap;
[docs/DESIGN.md](docs/DESIGN.md) the design of phase 1.

The painter, widgets, text engine and highlighting come from
[markdown-shared-rs](https://github.com/Kaan88/markdown-shared-rs), pinned to a commit in
`Cargo.toml`.

## Build

```powershell
cargo build --release --workspace
cargo test --release --workspace
targetelease\mdedit.exe README.md
```

`bash tests/shots.sh <dir>` renders 33 interface states to PNG without a window.

The mark in `assets/` is drawn by `assets/make_icon.py` in the viewer's repository
(`python assets/make_icon.py mdedit`), which holds the shapes of both marks.
