# Task manager mockups (branch `mock/task-manager`, never merged)

Drawn by the app's own painter, fonts, palette and widgets, behind `--mock tm-<name>` with `--shot`.
Throwaway: `bins/mdedit/src/mock*.rs`, `mock/tm*.rs`. Every process, figure and graph is made up;
nothing was measured and no process was touched. The plan is `docs/task-manager-plan.md`.

Regenerate: `cargo build --release -p mdedit`, then `pwsh shots/mock-tm/make.ps1`
(`-Exe` for another build; a fast build: `CARGO_TARGET_DIR=target/fast`, LTO off). Every run is
`--behind --no-session` with `MDEDIT_SETTINGS` in a scratch folder (rule 9). Each shot has
`-light.png` and `-dark.png`; `-compact-` ones are 860x600; the tray is 720x620, compact mode 380x590.

| Shot | Shows |
|---|---|
| `tm-procs` | Processes, grouped (default): Apps, Background, Windows; Firefox collapsed with its count; statuses |
| `tm-tree` | The true parent and child tree, with Publisher ("Not signed" in amber) |
| `tm-flat` | Flat, sorted by memory, the wide column set (Commit, Handles, Threads, User, Publisher) |
| `tm-search` | A filter, "rust" |
| `tm-details`, `-modules`, `-handles`, `-net`, `-env` | The details pane, five tabs |
| `tm-details-denied` | A SYSTEM process: what a standard user cannot read, and "Show this once" |
| `tm-context` | Context menu with the Priority submenu; "Search online" off |
| `tm-endtree`, `tm-affinity`, `tm-elevate` | Questions: end a tree; processors; administrator rights (two choices) |
| `tm-perf-cpu`, `-cores`, `-mem`, `-disk`, `-net`, `-gpu` | Performance, with "Busiest now" |
| `tm-startup`, `-startup-admin` | Startup apps and exactly what turning one off writes; an all-users entry |
| `tm-services` | Services, shields on actions that need rights |
| `tm-files`, `-files-deep` | What is holding a file (quick) and a folder (deep) |
| `tm-settings`, `-privacy`, `-cost` | Options, the network and rights page, the app's own cost popover |
| `tm-tray`, `tm-compact` | Tray flyout and the icon enlarged; compact always-on-top window |

## What drawing taught

- The first draw showed Mbps per process for a standard user, which Windows does not give. It now shows
  connections, the plan says per-process bytes need administrator, and "Disk" became "Disk I/O".
- The status bar has to carry the app's own cost, and `tm-cost` explains it.
- The shared tab strip draws a document icon and a close button on every tab, wrong for pages; at 860 px
  the labels clip. A tab variant in `mdshared_ui::chrome` is in the plan (phase 1).
- Heat shading in one hue (the accent) reads in both themes; a red-to-green scale would not.
- A protected process needs its own card ("protected by Windows"), not "access denied".
- The "Startup impact" column is not available without our own recording; it is shown for illustration.
- The window is tall rows of numbers: 28 px rows fit 24 processes at 1280x800, so grouping and
  collapsing matter more than in the file manager.

Not drawn: a services context menu, the Run-new-task dialog, the History 10 min and 1 hour graphs,
column chooser, an elevated ("Administrator") window, the Explorer verb.
