# mdedit: what it is, the budgets, and what is built

Stable. It changes when the owner changes what the app is for, or when a budget is measured
again. What is being worked on is in `docs/STATE.md`; why things were decided is in
`docs/DECISIONS.md`. The plan this file used to be, with its phases, risks and open questions
from 2026-11-12, is `docs/archive/PLAN-2026-11-12-to-2026-11-20.md`.

## What it is

The owner wanted, for personal use, "an editor as fast and pleasant as the viewer: a Notepad
alternative with syntax highlighting".

- A separate binary, `mdedit`, not a mode of mdview. Windows first; Linux since 0.5.0; macOS
  builds are released as well.
- Pure Rust, no webview, no toolkit. winit for the window and input, softbuffer for the pixels,
  every control drawn by hand, as in `mdview`.
- Plain text editing of any file, with syntax highlighting where a syntax exists. It saves in
  the file's own encoding and line endings and touches no other byte.
- It looks and behaves like the viewer: the same title row with tabs, command bar, menus,
  dialogs, themes and shortcuts, from the same shared crates.

## What it is not

- **Not an IDE.** No language servers, no build or debug, no plugins, no terminal.
- **Not a WYSIWYG Markdown editor.** Markdown is edited as text. Visual editing (Ctrl+E) shows
  the rendered page and opens one block at a time as its Markdown source; typing into rendered
  text stays out.
- **No network use**, except looking for and downloading its own updates (one setting turns that
  off), and the AI when the owner turns it on. The AI connection is off by default; when it is
  on, only the text a command is run on goes, and only to the provider chosen. The editor makes
  no request itself: the `mdedit-ai` engine does.
- **Set aside by the owner**, each can be reopened: screen-reader accessibility, plugins,
  printing, the FTP and project panels, syntax-aware folding (folding is by indentation).

## The rule for every feature

The owner, 2026-11-12: "as long as the app remains just as fast to open and work, implement
useful features from Notepad++." And, as passed on to the agents who built the 0.6.0 features
(2026-11-14): "app is really good, let's not ruin it."

A feature goes in only if the budgets below hold, measured before and after. It adds no work at
start and nothing on the keystroke or scroll path while it is not in use: it loads lazily, starts
no thread at launch, and brings no dependency that was not asked for.

mdedit has been feature complete since 2026-11-17. A new feature goes in only when the owner
asks for it.

## Budgets

| | Budget | Why |
|---|---|---|
| First frame, small file | under 100 ms | The viewer's figure; the editor has no parser to run |
| Private memory, small file | under 30 MiB | The viewer's, plus the buffer and undo |
| Keystroke to pixels | under 8 ms, typically 2 to 4 | One line re-shaped and one band repainted |
| Scroll frame | under 4 ms at 1280 x 800 | The viewer's budget |
| Opening a 100 MB file | first frame under 500 ms, memory about the file's size plus 25% | Loading must not shape or highlight what is not on screen |
| Idle CPU | zero | Timers only for the caret's blink and pending work |

The order they matter in: launch time first, then keystroke, scroll, memory and idle CPU. **The
exe size is not a gate** (the owner, 2026-11-14): it is reported with each release, and only a
sudden large jump needs explaining. The 1.2.0 Windows exe is 8,399,872 bytes.

What has been measured, on the owner's PC, release builds:

| When | First frame | Memory | Keystroke | Scroll |
|---|---|---|---|---|
| Phase 1, 2026-11-13 | 84 to 104 ms, median about 90 | 27.5 MiB | | p95 0.8 to 1.5 ms |
| After 0.6.0, 2026-11-15 | 85 ms | 27.8 MiB | | |
| 1.2.1 against `main`, 2026-11-21, 15 runs each | 110.7 and 111.9 ms | 27.7 and 27.7 MiB | p50 0.56 and 0.55 ms | p50 0.44 and 0.46 ms |
| Library phase 1, 2026-11-21, 7 runs each | 89.7 to 91.2 ms (medians) | 27.6 to 27.9 MiB | | |

Three things to know before reading a number:

- **The first-frame figure on this PC moves with the day**: about 90 ms on some, about 111 ms on
  others, for the same build. On the days it reads 111, the published release reads 111 too. So
  what is compared is **before against after**, two release builds run in turn on the same day
  (`docs/CODE.md`, "Measuring"). A change passes when the two are within the machine's noise.
- The 100 MB file: first frame 138 to 199 ms warm and 271 ms cold; memory 133 MiB, which is 8 MiB
  over its budget and was left as it is.
- Not measured since phase 1: idle CPU with the window unfocused. Never measured: any budget on
  macOS or Linux desktops.

## What is built

Every roadmap is built. One line each; the sections of `docs/DESIGN.md` have the detail.

- **Editing** (the roadmap agreed 2026-11-12, done 2026-11-13): line commands; find and replace
  with regular expressions; unsaved tabs survive a close; whitespace, word wrap, matching
  bracket, zoom, go to line; encoding and line-ending conversion; column selection and multiple
  carets; bookmarks; find in files on a worker; folding by indentation and a function list;
  macros; split view.
- **0.5.0 to 0.7.1**: the command palette, compare two files, replace in files; one file in both
  panes; the hand-off with mdview and the live preview; column selection, Markdown table
  editing, autosave and crash recovery; paste an image; word completion and snippets; rounded
  corners; a frosted window on Windows 11.
- **1.0.0 to 1.0.6**: the preview pane, with mdview's renderer built in (`embedded-preview`);
  selecting text and ticking task boxes on it; pinned tabs; tabs not opened for a week.
- **1.1.1 and 1.1.2**: AI rewriting, with the `mdedit-ai` engine in every package.
- **1.2.0 and 1.2.1**: visual editing; several custom AI providers; the caret kept per file.
- **On `main` for 1.3.0**: the notes library and its pane. See `docs/STATE.md`.

## What is planned

`docs/ai-library-plan.md`: a folder per group, splitting a document into notes, a review that
takes changes one by one, an AI task window over files, bringing a folder into the library.
Waiting for the owner's approval.

Further out, and only ideas: `docs/ecosystem-ideas.md`, a review of the apps that could join
mdview and mdedit on the shared crates.
