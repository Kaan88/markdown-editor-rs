# mdedit

A light text editor in Rust, the companion of the Markdown viewer `mdview`
(Kaan88/markdown-viewer-rs). winit + softbuffer, every control hand-drawn, built on the crates in
Kaan88/markdown-shared-rs.

Before doing anything, read `docs/HANDOFF.md`: current state, decisions already made, and what to
do next. The plan and feature roadmap are in `docs/PLAN.md`, the design in `docs/DESIGN.md`.

- A feature goes in only if the app stays as fast to open and work in: measure the budgets in
  `docs/PLAN.md` before and after.
- Commit and push only as phases complete or when asked. There is no CI, so run the tests yourself.
- `spikes/` is throwaway code, kept for reference.
- The user approves the interface by looking at it; screenshots and tests do not stand in for that.
- Report results plainly, including what was not run. Nothing counts as working on Windows until
  it has run on Windows.
