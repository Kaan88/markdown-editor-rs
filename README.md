# markdown-editor-rs

<img src="assets/mdedit.png" width="96" alt="The mdedit mark: an M and a pencil on a green tile">

`mdedit`: a light, fast text editor with syntax highlighting, the companion of
[markdown-viewer-rs](https://github.com/Kaan88/markdown-viewer-rs) (`mdview`). A Notepad
alternative for personal use that opens any text file, with the same hand-drawn interface as the
viewer.

**Status: phase 1 (read-only) designed; the workspace holds compiling stubs only.**
[docs/PLAN.md](docs/PLAN.md) holds the goals, the budgets, the phases and the decisions still
open; [docs/DESIGN.md](docs/DESIGN.md) the design of phase 1 and its three work packages.

The interface, text engine and highlighting come from
[markdown-shared-rs](https://github.com/Kaan88/markdown-shared-rs), which both applications use.
Work here starts after that extraction is done.

The mark in `assets/` is drawn by `assets/make_icon.py` in the viewer's repository
(`python assets/make_icon.py mdedit`), which holds the shapes of both marks.
