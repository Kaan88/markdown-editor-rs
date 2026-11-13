# Handoff: mdedit and the shared crates

Written 2026-11-13, updated the same day after phases 2 and 3. Read this first, then
`docs/PLAN.md` (goals, budgets, phases, feature roadmap) and `docs/DESIGN.md` (the design of
phase 1, and section 16 on editing and saving as built).

## 1. Where things are

Three public repositories under `Kaan88`, all MIT, all pushed and clean on `main`:

| Repository | What | Head on 2026-11-13 |
|---|---|---|
| `markdown-viewer-rs` | `mdview`, the Markdown viewer. Its own `docs/HANDOFF.md` describes it | `b4e1617` |
| `markdown-shared-rs` | The crates both applications use | `843b50f` |
| `markdown-editor-rs` | `mdedit`, this repository | this commit |

The editor pins the shared repository by `rev` in its root `Cargo.toml` (see "Linux in a real window" for the current one), earlier on 2026-11-13 at
`843b50f3cc025480ee06226be1247d38df7442a2`. Three shared commits were made from this repository's
session on 2026-11-13: `a74aca4` (the updater's threads: `spawn_at_launch`, `spawn_check`,
`spawn_install`, `unix_now`), `d0ec8b8` (`Language::line_comment`) and `843b50f` (`Menu::above`,
a menu that stands on its anchor, for menus opened from a status strip). The viewer still starts
its updater threads with its own copies in `bins/mdv-app/src/app/commands.rs`; moving it to the
shared ones is a small change that was left to the viewer's own session. The editor does not yet
use the watcher's `set_folders` or the tree's `watched_dirs` (`e93719b`). The viewer pins it in `crates/core`,
`crates/render` and `bins/mdv-app`; check there which commit it is on.

On the Windows PC the three checkouts sit side by side in `C:\Users\kaank\Desktop`: `mv`,
`markdown-shared-rs`, `markdown-editor-rs`. Anywhere else, clone all three fresh. The viewer's
history was rewritten on 2026-11-12, so an older clone of it must be re-cloned, not pulled.

## 2. The user's rules

- **The goal for the editor (2026-11-12):** "as long as the app remains just as fast to open and
  work, implement useful features from notepad++." The agreed list and its order are in
  `docs/PLAN.md` under "Feature roadmap". Measure the budgets before and after each feature.
- The user approves the interface by looking at it. Screenshots and tests do not stand in for that.
- Report results plainly, including what was not run. Nothing counts as working on Windows until
  it has run on Windows.
- Commit and push as phases complete or when asked. There is no CI: run the tests yourself.
- On the Windows PC, launch a built `.exe` from Git Bash, not PowerShell (PowerShell refuses it as
  unsigned).
- **Shared first (2026-11-13):** "whatever can be shared will start shared. no need for
  duplicate work." Anything both applications could use is written in `markdown-shared-rs`,
  pushed, and pinned; the application keeps only its own names and settings.
- The user asked for cheaper models where they do the job, and on 2026-11-13 said: no Opus
  subagents; the main session does that work itself. So the headless crates with a clear
  specification go to Sonnet subagents (each in its own git worktree, so their builds do not
  break each other), and the design, the shell, the reviews and the fixes stay in the main
  session.

## 3. What was done on 2026-11-12

**Shared crates** (plan: `markdown-shared-rs/docs/PLAN.md`, steps 0 to 3 done):

| Crate | Holds |
|---|---|
| `mdshared-text` | `Bitmap`, `TextEngine`, `shared_fonts` |
| `mdshared-highlight` | syntect set-up, whole-block highlighting (viewer), `incremental` line highlighter (editor) |
| `mdshared-ui` | `gfx` (`Painter`), `theme`, `menu`, `textfield`, `tree`, `keys`, `platform`, `glyph`, `scroll`, `chrome` (tabs, caption buttons), `pane`, `overlay` (bubbles, dialog frame), `window` (resize bands) |
| `mdshared-app` | `paths`, `files`, `recents`, `settings` (key=value layer), `single` (hand-over), `watch` |

The viewer re-exports them at their old paths. After every step the viewer's 43 `--shot` renders
were compared byte for byte with renders taken before the move, and all matched; its tests pass
(319 there, 74 in the shared repository, more than the 368 before the move because tests were
added). `mdview.exe` went from 7,439,872 to 7,457,280 bytes.

Not done for the shared code: step 4 (the window loop as a framework) was put off on purpose.
Each application has its own loop; design the shared one from the two. Step 5 (a tag) waits for
that.

**Editor.** Phase 0 spikes (`spikes/*/RESULTS.md`) and phase 1, the read-only editor: tabs,
scrolling, caret and selection by keyboard and mouse, copy, find with counter, go to line, zoom,
Files pane, recent files, session restore, reload on change, single instance, syntax colours,
encodings (UTF-8, UTF-16 by BOM, legacy code pages guessed with `chardetng`) and line endings in
the status bar. Typing does nothing yet.

| Crate | Holds |
|---|---|
| `crates/mdedit-core` | Loading, encodings, line endings, the rope buffer, display columns, selection, find. Headless |
| `crates/mdedit-highlight` | The highlight worker thread over `mdshared_highlight::incremental` |
| `bins/mdedit` | The shell: window loop, text view (`view.rs`, `view/`), chrome, panes, overlays, session, `--shot` |

Measured on the PC, release build (phase 1, after the fixes):

| | Budget | Measured |
|---|---|---|
| First frame, small file | under 100 ms | 84 to 104 ms across runs, median about 90 |
| First frame, 100 MB file | under 500 ms | 138 to 199 ms warm, 271 ms cold |
| Private memory, small file | under 30 MiB | 27.5 MiB |
| Private memory, 100 MB file | 125 MiB | 133 MiB, over by 8 MiB, left as it is |
| Scroll frame, p95 | under 4 ms | 0.8 to 1.5 ms |
| Idle CPU, focused window | zero | none measured |
| Exe | under 6 MB | 5.7 MB |

**2026-11-13: phases 2 and 3.** Editing and safe saving, as `docs/DESIGN.md` section 16
describes them. Two Sonnet subagents wrote the headless parts (`buffer.rs` `replace` and `Change`,
`edit.rs`, `history.rs`; `save.rs`), the main session wrote the shell around them and
`HighlightService::edit`. Figures are in `docs/PLAN.md` under "State": a keystroke reaches the
pixels in under 1 ms on the small and the 100 MB file, and the phase 1 budgets hold.

## 4. What has not been done or checked

- **The user has not looked at either application since these changes.** The editor was opened on
  their screen once; no feedback yet. Nothing about the interface is approved.
- Nothing interactive in the editor was tried by hand: selection, copy into another program, find,
  tabs (drag, middle click, menu), the Files pane, theme switch, reload when the file changes, a
  second launch handing over to the first.
- **2026-11-13, later:** the user tried the build and reported that everything worked. Three
  things they raised turned out not to be faults (Redo after Backspace, a file drop with a tab
  open, heavy shadows; the last is the About card in dark mode, which draws its shadow twice
  over the dimmed window in shared code, and the user said to leave it). While checking them the
  main session drove a real window with synthetic keys, mouse and an OLE drag: typing, Backspace,
  Delete, undo, redo, the right-click menu's Undo, Save, a dropped file, New file and Save
  through the file dialog all behaved. That is not the user's approval of the interface.
- Updates (2026-11-13): v0.1.0 is published and `--check-update` answers "nothing newer"
  against it. **No update has been offered or installed by mdedit**, since nothing newer
  exists. The bar and the About buttons were only rendered with `--update-bar`; nobody has
  clicked them. "One installer or two" is still open; the release is the bare exe.
- Editing basics (2026-11-13): each shortcut was pressed once by script in a real window on a
  small Rust file and gave the right text. The user has not seen the longer right-click menu.
  Move line and the two every-line commands on large or odd files ran only in the unit tests.
- New file (2026-11-13): `Doc::path` is optional. Tested by one unit test and one scripted run
  in a real window; the user has not seen the untitled tab or the changed "+" button.
- Of phases 2 and 3, by hand, before that: nothing. Typing ran only through `--type-steps` (a real window,
  scripted keystrokes) and `--type`. Never pressed by a person: every editing key, dead keys and
  AltGr on the Turkish layout, cut and paste with another program, the three questions (Save
  changes, file changed on disk, reload and lose changes), Save as and its file dialog, closing
  the window with several modified tabs.
- `ReplaceFileW`'s fallback for file systems that do not support it (network shares, FAT) has
  not run. The code for other systems than Windows has not been built.
- Fixed 2026-11-13 ("Code folding" below): a view request the highlight worker could not
  finish inside one 8 ms slice was never answered.
- Undo steps are capped by the text they hold (64 MiB), not by their number.
- The first frame of a Markdown file was 125 ms in one run (`fixtures/sample.md`), over the
  100 ms budget set on `sample.rs`; whether phase 1 had that was not checked.
- The viewer's first-frame time and memory were not re-measured after the move to shared crates,
  and its installer was not rebuilt. The installed `mdview` on the PC is the build from before
  the move.
- Idle CPU with the editor's window unfocused was not measured.
- The input-method hand test (`spikes/ime/RESULTS.md`: Turkish Q dead keys and AltGr, a CJK IME)
  was not done. Phase 2 depends on what it shows.
- Linux (Debian 13, rustc 1.99.1, run 2026-11-13 over SSH on a VM): builds, and
  `cargo test --release --workspace` passes (372). `tests/shots.sh` renders 57 PNGs under Xvfb,
  byte-identical across two runs. Not done: a real X11/Wayland window, input, clipboard, file
  dialogs, IME, clippy. Known issues:
  - Fixed later the same day (see "Linux in a real window"): every icon drew as a box.
  - Debug-build `cargo test` fails four `mdedit-highlight` tests (no colours within the wait, even
    at 30 s); the same four fail on Windows. Tests are run with `--release`.
  - `platform.rs` `first_line` is dead code on Linux (one warning).
  - `tests/shots.sh` needs LF line endings; a Windows checkout with autocrlf breaks it on Linux
    (add a `.gitattributes` entry).
  - Fixed: `tabs_open_close_move` assumed Windows case-insensitive names; `examples/bench.rs`
    linked kernel32 on every platform (now gated, with a `/proc` version).
- The editor's small-file first frame sits at the edge of its budget: about 60 ms of it is
  syntect start-up before the first colours.

## 5. Decisions the user has not confirmed

These were made to keep moving and are easy to undo:

1. **`panic = "abort"`** in the editor's release profile. It saved 0.9 MB and brought the exe
   under 6 MB. A panic on any thread ends the process, and since 2026-11-13 there is unsaved
   text to lose. The viewer uses `unwind`. Unwinding alone would not save the text either; what
   does is unsaved-session restore (roadmap item 3), so that is the place to settle this.
2. **Crate names** `mdshared-*` (the shared plan called them placeholders).
3. **Step 4 put off**, as above.
4. **`mdshared-app` created** (open decision 1 of the shared plan, taken as it recommended).
5. Open in `docs/PLAN.md` and untouched: the editor's name, one installer or two, word-wrap
   default, Markdown conveniences.
6. Made on 2026-11-13 for phases 2 and 3: Tab inserts a tab character (no spaces option); the
   unsaved mark is a bullet before the file name; a file with replaced bytes is read-only;
   saving runs on the UI thread; the layout cache is keyed by line content; the IME stays off
   until phase 5.
7. Made on 2026-11-13 for New file: Ctrl+N and the "+" beside the tabs make an "Untitled N" tab
   (the "+" opened the file dialog before); it is UTF-8 with the platform's line ending; Save
   on it is Save as, offering "Untitled N.txt"; it is not in the session, so an untitled tab
   does not come back after a restart (a modified one asks before closing).
9. Made on 2026-11-13 for the editing basics (roadmap item 1, `mdedit-core`'s `lines.rs`, by a
   Sonnet subagent, wired by the main session): Duplicate line Ctrl+D, Move line Alt+Up and
   Alt+Down, Delete line Ctrl+Shift+K, Toggle comment Ctrl+K and Ctrl+/ (the language's line
   comment, from the shared `Language::line_comment`; a language without one gets a toast),
   UPPERCASE Ctrl+Shift+U, lowercase Ctrl+U, Trim trailing spaces and Tabs to spaces (no
   shortcut). All but the two moves are in the text's right-click menu. Trim and Tabs to spaces
   refuse a file over 8 MiB: they run on the UI thread at 4 to 7 s per 100 MB.
10. v0.1.0 was published on 2026-11-13 from `2ba9912` with `tools/release.ps1`
   (`docs/RELEASE.md`): the exe and `mdedit-update.txt`. It does not hold the editing basics.
8. Made on 2026-11-13 for updates, after the user said to wire the shared updater in: mdedit
   updates itself the way mdview does (`docs/DESIGN.md`, section 17). The manifest address is
   `https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt`,
   chosen by analogy with the viewer's; `MDEDIT_UPDATE_MANIFEST` names another one. One
   executable, no engine, no installer entry. The bar sits under the command bar; the two
   controls are in About, as in mdview. This ends the plan's "no network use at all".

## 6. What to do next

1. **The user looks at the editor and types in it.** Phases 2 and 3 meet their "done when"
   by measurement and tests only. The hand checks in section 4 come first, on the Turkish
   layout.
2. The feature roadmap in `docs/PLAN.md` is built to its end (2026-11-13). What each item
   left unbuilt is listed in its section below; nothing of it has been seen by the user.
3. The Files pane could follow changes in its folders with the shared watcher's new
   `set_folders` and the tree's `watched_dirs` (shared commit `e93719b`).
5. Any change to shared code: edit it in `markdown-shared-rs`, run the viewer's check (section 7),
   push, then move the `rev` in both applications.
6. Code the editor's shell copied from the viewer because it was generic but had not been moved
   (parts of the window loop, the bar, the find box wiring) is the input for step 4 of the shared
   plan.

## 7. Build and check

```bash
# in this repository; on the PC, keep temp files off the system drive's temp folder
export TEMP="$PWD/target/tmp" TMP="$PWD/target/tmp"; mkdir -p target/tmp
cargo build --release --workspace
cargo test --release --workspace        # 404 passed on 2026-11-13
cargo clippy --release --workspace --all-targets -- -D warnings
cargo fmt --all --check
bash tests/shots.sh target/shots/a      # 72 PNGs, deterministic; first run generates target/data
```

Useful flags of `mdedit.exe` (all in `bins/mdedit/src/args.rs`): `--no-session`, `--shot out.png`
with `--size WxH --scale N --light|--dark --menu NAME --dialog NAME --find TEXT --goto L[:C]
--select A-B --type TEXT --scroll N --hscroll N --hover X,Y --bookmark L --fold L --minimap`; `--trace` prints stage times, first
frame, memory and CPU time; `--scroll-steps N` times scroll frames; `--type-steps N` types N
keystrokes at the caret and times each from keystroke to pixels (it exits without saving);
`--exit-after-ms N` opens a real window and closes it.

```bash
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 fixtures/sample.rs
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 target/data/100mb.txt
target/release/mdedit.exe --no-session --trace --type-steps 300 --goto 20:5 fixtures/sample.rs
```

**The viewer's no-pixel-change check.** The script and its baseline lived in a session scratch
folder on the PC and are not in any repository. To recreate it: in the viewer, at a commit known
to be good, render a fixed set of `--no-session --shot` states (both themes, each `--menu`, each
`--dialog`, the empty state, panes, find, focus mode, page widths, scales 1.0, 1.5 and 2.0, a few
`--hover` positions; 43 were used) into a baseline folder; after a change, render again and `cmp`
every file, then run the workspace tests (which include the four goldens) and `cargo fmt --check`.
Putting that script into the viewer's repository would be a useful first task.

Fixtures are written with exact bytes by `fixtures/make_fixtures.py`; `.gitattributes` keeps git
from converting them.

## Minimap tested in a real window, and 0.4.0 (2026-11-13)

The owner asked for the left-out runs and then a release. Windows, launched from Git Bash,
scratch `MDEDIT_SETTINGS` files, mouse and keys by `SetCursorPos`, `mouse_event`, `keybd_event`,
a screenshot or a file check for each. No defect was found, so no code was changed (the version
went to 0.4.0 and nothing else).

- **Run, with what was seen:** (2) split view of two 50,000-line files: a press and a 60 px drag
  on the second pane's column scrolled only that pane (10,492 against 5,246 in the other), and
  the keyboard went to it (tab and status line); (3) folds (Shift+F9, thousands) and word wrap
  together: a drag moved the band and the text with wrapped lines in view; (4) 50k.rs beside
  100mb.txt was not run (see below); (5) the Files pane open, then with the results panel too,
  the window narrowed to 660 px (column hidden) and widened (back), the results panel closed
  (column full height again); (6) the right-click menu: Escape closed it, Down and Enter hid the
  minimap; (7) 453 carets (a column selection extended by the wheel over a long range), scrolled,
  and typed into (all 453 lines took the letters), no stall; (8) idle CPU: 0 to 15.6 ms of CPU
  time per 20 s with the window idle and the column on, the same as the exe built before the
  minimap (0 to 31 ms per 20 s; the clock ticks in 15.6 ms steps); (9) the main menu by keyboard:
  Down to View, Right opened the submenu with its first item lit, Down four times and Enter
  toggled Minimap, Escape and Left each closed only the submenu, Escape again closed the menu,
  Right on "Open recent" showed the scratch settings' recent file, a click on an item in the View
  submenu toggled it; (10) the installer: see below.
- **Not possible or not run:** (1) a scale other than 100 %: `--scale` exists only for `--shot`;
  in a window the scale is the display's, and the owner's display settings were not changed (the
  1.5 and 2.0 shots stay the only evidence). (4) Two large files together, 50k.rs with 100mb.txt in
  a split, was not run: the split in a real window was run with two copies of 50k.rs, and 100mb.txt
  alone was timed with `--trace` (shown 142 ms) but not opened beside another file in a window.
  (10) The installer was **not** run interactively: an install of mdedit (0.3.2, `AppId`
  C58A9AB1-...) already exists on this PC, so the wizard would have upgraded it in place; the
  silent variant was run instead, with no tasks, into `target\insttest` (`/VERYSILENT /TASKS=""
  /DIR=...`): the 0.4.0 setup installed the exe, icon and uninstaller there, the installed exe ran
  and Ctrl+Shift+M worked in it, the uninstaller removed the folder and the uninstall key. So the
  wizard's pages, and that the last page now offers only "Launch mdedit", were not seen on screen;
  the script (`mdedit.iss`) has one `[Run]` entry and no Default apps task. **A slip:** the
  uninstall key of the existing install is the same key, so the test install replaced it and the
  test uninstall deleted it, and the Start Menu shortcut with it; I had meant to export the key
  first and the export failed (Git Bash rewrote the `/y`). The existing install was put back by
  running the 0.3.2 setup silently into its own folder with all three tasks: its exe is the same
  file (SHA-256 5872957b...a026, 5,651,968 bytes), the uninstall key says 0.3.2 with its location,
  and the Start Menu and desktop shortcuts, the context-menu entry and "Open with" are there. It
  was not the exact original key (the unins000 files are rewritten), and its task choices were
  inferred from what was registered. Linux was not run.
- **Not run besides:** a drag on the column with folds, wrap and a split all at once; the first
  press of the keyboard menu on a high-DPI display; thousands of carets.
- **Measured** (the 0.4.0 exe, 5,678,592 bytes; budgets: 6,000,000, 100 ms, 30 MiB, 8 ms, 4 ms):
  sample.rs shown 75.4 to 76.5 ms off and 77.0 to 79.1 ms on; private memory 27.7 to 27.9 MiB off
  and 27.5 to 28.0 MiB on; keystroke p50 0.47 ms off, 0.59 to 0.60 ms on; scroll step p50 0.41 off,
  0.44 to 0.45 on; 50k.rs 0.48 and 0.44 off, 0.68 and 0.62 to 0.65 on; 100mb.txt 0.54 to 0.55 and
  0.55 to 0.56 off, 0.77 to 0.78 and 0.76 to 0.77 on. Tests 404 passed, clippy and fmt clean,
  72 shots, all byte-identical to the last run but `dialog-about` (it shows the version).

## Minimap, finished (2026-11-13, after the owner saw it: "looks great. finish it. and make it togglable")

First part of the day: the section below. This one builds what it left out. Design in
`docs/DESIGN.md` section 28 (updated). Not released, version unchanged, nothing pushed elsewhere.

- **Toggle.** Ctrl+Shift+M (free: no command had it; Ctrl+Alt is AltGr and was avoided), shown in
  the View menu and the shortcuts dialog. A right click on a column opens a one-item menu,
  "Hide minimap" (`PopupKind::Text`'s machinery, `overlays::minimap_menu`). Recordable in a macro,
  as Word wrap is (nothing to change: `recordable` leaves view toggles in; a test says the two
  agree).
- **Marks:** selection (and several carets), the caret's row (keyboard pane only), find matches at
  the right edge, bookmarks at the left edge. Drawn from the data the editor keeps, only for the
  window's lines, never a search of its own. See the design section.
- **Colour:** 78 % kept (60 % was grey). Compared side by side in a light and a dark shot.
- **Word wrap:** one row per line still. The band's top is exact to a wrapped row and its bottom
  edge counts the last visible wrapped row whole, so **the band can be up to one wrapped row too
  tall**; the scroll fraction is the editor's own (top line over lines less one).
- **Second pane:** each pane has its column (`Region::MinimapOther`, `Hit::MinimapOther`); the
  map request is per document (the worker already had a slot per document). The restructuring was
  the pattern already there (one more region, one more hit, arrays of nine); nothing beyond it.
  A drag holds the document, not the pane (`Drag::Minimap { doc }`), so it survives the panes
  changing roles when a press on the second column gives that pane the keyboard. The panes' two
  columns have the same top and height, which is what a drag after the swap relies on for the one
  event before the next frame has laid the areas out again.
- **Measured** (release, quiet PC, runs alternated; budgets: exe 6,000,000 bytes, shown 100 ms,
  memory 30 MiB, keystroke p50 8 ms, scroll step p50 4 ms):

  | | before the minimap | after, off | after, on |
  |---|---|---|---|
  | exe bytes | 5,651,968 | 5,678,592 | same exe |
  | shown, sample.rs (ms) | 73.7 to 77.5 (a first run at 93.7) | 76.4 to 86.8 (first two runs 119.5 and 104.1, machine busy) | 75.5 to 76.4 |
  | private memory, sample.rs (MiB) | 27.5 to 28.0 | 27.3 to 27.8 | 27.6 to 28.1 |
  | keystroke p50, sample.rs (ms) | 0.52 to 0.53 | 0.51 to 0.52 | 0.63 |
  | scroll step p50, sample.rs (ms) | 0.45 to 0.46 | 0.44 | 0.48 |
  | keystroke p50, 50k.rs (ms) | 0.53 | 0.52 | 0.71 to 0.72 |
  | scroll step p50, 50k.rs (ms) | 0.48 to 0.49 | 0.46 | 0.66 to 0.67 |
  | keystroke p50, 100mb.txt (ms) | 0.60 to 0.61 | 0.59 to 0.61 | 0.79 to 0.81 |
  | scroll step p50, 100mb.txt (ms) | 0.59 to 0.75 | 0.59 to 0.61 | 0.81 to 1.02 |

  Shown with the split and both columns (sample.rs and sample.md): 77.7, 84.0, 96.5 ms (the 96.5
  was the first run); memory 27.8 to 27.9 MiB. Idle CPU with the column on: CPU time at idle and
  at exit equal (no timer). The 1 MB single-line file with the column on: shown 53.7 ms, 16.5 MiB,
  CPU time equal at idle and exit; the 100 MB file: shown 142.8 ms. Typing with the split open
  cannot be timed (`--type-steps` has no split); it was typed in a real window (below).
- **Tests:** 404 passed, 0 failed (400 before this part): the new pure parts (selection columns,
  byte to column, wrapped scroll range and inverse, `Areas::with_minimap` for two panes), the
  recordable toggle. Clippy `-D warnings` and `fmt --check` clean.
- **Shots:** 72 now. New: `minimap-marks` (selection, find, bookmarks, caret row), `minimap-wrap`
  (`--wrap`), `minimap-scale-150`. The 64 older ones are byte-identical to the run before the
  minimap but `menu-view`, `menu-main-view` and `dialog-shortcuts` (they list the command and its
  key). The five minimap shots of the first part changed (caret row, 78 % colour, a column for
  each pane in `minimap-split`). Looked at: all new ones, `dialog-shortcuts`, and a scale 2.0
  shot (rows and columns are whole pixels at 1.5 and 2.0; the ink is 3 px tall and 2 px wide at
  1.5, 4 and 2 at 2.0).
- **Run in a real window** (Windows, scale 1.0, Git Bash launch, `MDEDIT_SETTINGS` scratch files,
  mouse and keys by `SetCursorPos`, `mouse_event`, `keybd_event`, window screenshots): split view
  with two columns (Shift+F8); letters typed in the left pane, a click on the right pane's
  column (the pane took the keyboard; letters typed went into its file), the wheel over that
  column (the pane scrolled, its band followed); the right-click menu, "Hide minimap" (columns
  and the setting gone); Ctrl+Shift+M (back); the window made 700 wide (both columns hidden),
  980 (both back) and 1280; Shift+F9 on the 50,000-line file (thousands of folds), a press and a
  drag on the column; word wrap on in a 760 px window, a drag on the column and the wheel (the
  band moved with wrapped lines in view); the results panel open (`--find-in-tabs`) with the
  column above it, and the find marks at the column's right edge; a plain `.txt` file (a press
  centred the view); the 1 MB single-line file (no stall, typed in it).
- **Not run:** scale other than 1.0 in a real window (shots at 1.5 and 2.0 only); Linux; a drag
  on the second pane's column (a press and the wheel were run); a drag with folds and wrap
  together; two large files in a split; typing with the split timed; the keyboard-only paths of
  the right-click menu (Escape, Enter); the column with the Files pane open; a selection of
  many carets over a long window (the code sorts them and looks at the window's lines only; no
  test with thousands); idle CPU in a real window by a tool (only `--trace`). The owner has seen
  the first part, not these additions.

## Minimap (2026-11-13, asked by the owner: "a minimap like Sublime Text")

`docs/DESIGN.md` section 28 has the design as built. Built to a written design from the main
session; not released, version not changed (still 0.3.2), nothing pushed to other repositories.

- **What it is.** View > Minimap (no key) shows a 96 px column at the right edge of the text with
  the document in miniature: one row per line that shows (folds skipped, word wrap ignored), a run
  of non-whitespace one coloured rectangle, the editor's colours at 60 %, a translucent band over
  the rows in view (8 %, 14 % over the column), the column scrolling in proportion to the editor
  in long files. Press outside the band: the editor centres on that line, a drag goes on from
  there; press inside it: a drag keeps the grab offset; the wheel scrolls the text. Off by
  default; remembered in the session (`minimap=1`); `--minimap` for shots and timing runs.
- **Decisions made without the owner.**
  - **The worker has a second request slot.** `peek` is one slot per document and the minimap
    wants another range of the document the editor is viewing, so `mdedit-highlight` gained
    `HighlightService::map` and a `map` flag on `Colored` (two tests). The worker serves view and
    peek first, then the map in 2 ms slices, then the background parse. This is in this
    repository's crate; nothing in the shared repository changed.
  - **Split view: only the pane with the keyboard has the column.** It was one call to
    `with_minimap` on the `text` area after the split, so it was not harder than giving none. The
    other pane keeps its full width. With the keyboard pane on the left, the column sits between
    the two panes. Under 320 px of text beside it (a pane in a narrow window) it hides.
  - **A run that crosses a colour change is split there**, not drawn in the colour of its first
    character (`fn main() {` is several colours in the editor).
  - **With word wrap** the band is the lines from the view's top line to the one at its bottom and
    the window's fraction is taken over those lines; without wrap it is exactly the editor's
    scroll fraction.
  - **Where the band is after a press outside it.** In a file longer than the column's rows the
    band does not land under the pointer (the column scrolls with the editor). The drag then moves
    the band by the pointer's movement from where it stands. Putting the band under the pointer
    instead would send the view elsewhere than the line that was clicked.
  - The first frame does not wait for the minimap's colours; a `--shot` does.
  - A line is read up to 512 bytes; wide characters count one column; the ink is cached as a
    bitmap per window, and typing draws the window again (no per-line patching).
- **Measured** (release, same quiet PC, runs alternated; budgets: exe 6,000,000 bytes, window
  shown 100 ms, memory 30 MiB, keystroke p50 8 ms, scroll step p50 4 ms):

  | | before | after, off | after, on |
  |---|---|---|---|
  | exe bytes | 5,651,968 | 5,666,304 | same exe |
  | shown, sample.rs (ms) | 74.8 to 75.3 (3 runs) | 74.6 to 76.3 (5 runs) | 77.1 to 82.4 (3 runs) |
  | private memory, sample.rs (MiB) | 27.8 | 27.5 to 27.7 | 27.8 to 28.0 |
  | keystroke p50, sample.rs (ms) | 0.51 to 0.52 | 0.47 | 0.57 to 0.59 |
  | scroll step p50, sample.rs (ms) | 0.44 to 0.45 | 0.40 | 0.43 |
  | keystroke p50, 50k.rs (ms) | 0.52 | 0.47 to 0.49 | 0.66 to 0.67 |
  | scroll step p50, 50k.rs (ms) | 0.46 | 0.43 | 0.61 to 0.62 |
  | private memory, 50k.rs (MiB) | 29.4 | 29.5 | 29.6 to 29.9 |
  | keystroke p50, 100mb.txt (ms) | 0.59 | 0.53 to 0.54 | 0.74 |
  | scroll step p50, 100mb.txt (ms) | 0.58 to 0.59 | 0.54 to 0.55 | 0.72 to 0.73 |
  | shown, 100mb.txt (ms) | 131 to 136 | 135 to 161 | 131 to 142 |

  The first build with the minimap on showed the window at 92 to 101 ms (sample.rs) and 110 to 118
  ms (50k.rs) because the first frame waited for the minimap's colours; that wait was removed (see
  above) and the table is of the final exe. Idle CPU with the minimap on: the CPU time at idle
  and at exit were equal on sample.rs and 100mb.txt (no timer was added); on 50k.rs the CPU time
  grows over the run as it does without it (the background parse). The 50k.rs file is already at
  29.4 MiB without the minimap (the 30 MiB budget was set on the small file); the minimap adds 0.1
  to 0.5 MiB (its bitmap is 96 x 700 px).
- **Tests.** `cargo test --release --workspace`: 400 passed, 0 failed (382 before): the arithmetic
  (marks, segments with tabs and cut-off, window and proportional scroll, pointer y to row, row to
  line with folds, drag and press) in `minimap.rs`, `Areas::with_minimap` in `ui.rs` (hide under
  320, split, results panel), the session and the argument, two for the worker. Clippy
  `-D warnings` and `fmt --check` clean. `tests/shots.sh` makes 69 shots (five new: `minimap`,
  `minimap-dark`, `minimap-long`, `minimap-fold`, `minimap-split`); the 64 earlier ones are
  byte-identical to a run made before the change except `menu-view` and `menu-main-view`, which
  list "Minimap"; two runs of the whole set were identical. I looked at the five new shots,
  `menu-view` and the real-window screenshots.
- **Run in a real window** (Windows, scale 1.0, launched from Git Bash with `MDEDIT_SETTINGS` on a
  scratch file and a copy of the 50,000-line file; mouse by `SetCursorPos` and `mouse_event`,
  screenshots of the window): the menu button, View, Minimap (turned on, `minimap=1` written);
  a press outside the band (the view went to the line at the pointer, 121 to 157 for a press on
  line 139); a drag after it (to line 13,000); a press inside the band and a drag of 40 px (the
  band moved 40 px); the wheel over the column (3 notches, 9 lines); a click in the text and
  typing 60 letters, a line break and 30 letters (the two long lines show in the column at once);
  Minimap off (full-width text, `minimap` gone from the file); on again; the window closed and
  started again on the same settings file (the column is there).
- **Not run:** scale other than 1.0 (the shots at 1.5 were not made for the minimap, only the
  unit arithmetic covers it); Linux; the split view in a real window (shot only), a drag with
  word wrap or folds in a real window (shots only for folds; wrap not even that); a window narrow
  enough to hide the column (unit test only); the results panel open with the column; a file that
  is not coloured at all in a real window (100mb.txt was only timed); the first frame with the
  column on a slow disk; the column on the 1 MB single-line file; a minimap request racing an
  edit in a real window beyond the typing above; idle CPU in a real window with the column on
  (only the `--trace` figures); typing with the split view open. **What is not built:** selection
  and find marks, bookmarks and the caret in the column, one row per wrapped row, the second pane's
  column, a width setting, a key for the command. (Of these, the marks, the second pane's column
  and the key were built later the same day: see "Minimap, finished" above.)

## Update look at every launch, and submenus on mouse-over (2026-11-13, 0.3.2, asked by the owner)

- **Update look.** The owner saw the launch look find nothing while the manual check found 0.3.1.
  Cause: `check_due` allowed the launch look once in 24 h, so an earlier look the same day
  suppressed it. Shared commit `1c3710a1ae2fed59585e37b2d4108c8d2802ac43` (branch
  `fix/update-look-every-launch` of `markdown-shared-rs`, on top of the old pin `fce8c8e`) makes it
  true at every launch with the setting on and removes `CHECK_EVERY`. All five `rev`s are moved to
  it. **That branch is pinned and the shared `main` was not moved.** Nothing in the editor used
  `CHECK_EVERY` and no test asserted "once a day"; only a doc comment in `session.rs` changed.
  Copies of 0.3.1 and older still look once a day, so they find 0.3.2 at launch only when their
  day is up, or by the manual check.
- **Submenus: a cascade.** `Popup` (`ui.rs`) now holds an optional `sub` popup and a `dwell`.
  Resting the pointer on "Open recent ›", "View ›" or "Macro ›" for 300 ms (`SUBMENU_DWELL`) opens
  the submenu beside the main menu, which stays; a click or Right opens it at once (Enter too, with
  the first item lit). The submenu sits level with the item when the pointer opened it, else at the
  menu's top, on the parent's left when there is room. Another item of the parent closes it (and
  starts that item's dwell); Escape or Left closes only the submenu; a click outside, the wheel or
  a right click close both; a click on a submenu item runs it and closes both. Up, Down and Enter
  act on the submenu while one is open. No other menu has a "›" item (checked: tab, text, find,
  line-ending and encoding menus), so none changed; the code is generic over `Act::submenu`. The
  dwell is a deadline in `Popup`, read by `timers()` like the tooltip's; with no menu open, or no
  item being rested on, there is no deadline and the loop waits. Built as a cascade because the
  popup code needed one extra field, not a rewrite (about 300 lines with tests).
- **Tests and shots.** 4 unit tests in `ui.rs` (`popup_tests`: dwell starts and keeps its
  deadline, a plain item or leaving cancels it, another item closes the submenu, the submenu keeps
  the parent lit, outside both changes nothing) place real menus headless. `--menu main-view` and
  `--menu main-macro` are new shot states; `tests/shots.sh` gains `menu-main-view` and
  `menu-main-macro-dark` (64 PNGs). I looked at those two and at `menu-main`.
- **Measured** (release): exe 5,651,968 bytes (budget 6,000,000); window shown 77 to 83 ms on
  quiet runs (95 to 109 ms in runs when the machine was busy; the published 0.3.1 exe showed the
  same spread in turns with it); private memory 27.3 to 27.8 MiB; keystroke p50 0.54 ms against
  0.50 ms for the 0.3.1 exe run in turns with it (0.47 was measured on a quieter day; budget 8).
  `cargo test --release --workspace` 382 passed, 0 failed; clippy `-D warnings` and `fmt --check`
  clean.
- **Run.** Update look: the release exe copied to `target\launchtest\` with `MDEDIT_SETTINGS` on a
  scratch file and `update_checked` one minute old: after the run the file said `update_checked` =
  the launch time (moved to now). The published 0.3.1 exe in the same setup left it at one minute
  old (the control). Hover, in a real window on Windows (Git Bash launch, cursor moved with
  `SetCursorPos`, screenshots of the window): the menu button clicked; after 100 ms on "View" no
  submenu; after 500 ms the View menu beside the main menu; the pointer into it kept View lit;
  to "Macro" the View menu closed and Macro opened; to "Go to line" it closed; a click on the text
  closed both menus.
- **Not run:** the keyboard paths (Right, Left, Escape, Enter into a submenu) in a real window
  (code only, no test); a click on an item inside a submenu in a real window; "Open recent ›" in
  a real window (the scratch settings had one recent entry, not hovered); scale 1.5 or 2; Linux;
  a submenu on a narrow window (it falls to the parent's right); idle CPU with a menu open was not
  measured (by reading the code the deadline is cleared when it fires); an update installed from
  the launch look. The owner has not seen any of it.

## More languages and an installer (2026-11-13, 0.3.1, asked by the owner)

- **Languages.** Looked up by file extension through `Language::for_path` before the change:
  coloured were Rust, C, C++, C#, Java, Go, Python, Ruby, PHP, JavaScript (also `.ts`, `.tsx`,
  `.jsx`, read with the JavaScript grammar), SQL, Bash, Batch, HTML, CSS, JSON, YAML, XML,
  Markdown, Makefile, Lua, Perl, R, Diff, Scala, Haskell, Erlang, Clojure, LaTeX, OCaml, Groovy
  and, from the hand-written scanners, TOML, Kotlin, Swift, PowerShell and Dockerfile. Not
  coloured: SCSS, Less, INI, `.env`, `.conf`, `.cfg`, CMake, Dart, Zig, Vue, Svelte, Elixir,
  Protobuf, Julia, Nim, Terraform, VB, F#, `.csv`, `.log`, `.txt` (the last three are plain text
  on purpose).
- **Added** in `markdown-shared-rs` `mdshared-highlight` `scan.rs`: TypeScript (`.ts`, `.tsx`,
  `.mts`, `.cts`; `.js` and `.jsx` stay on the JavaScript grammar), Dart, Zig, SCSS, Less, INI
  (`.ini`, `.cfg`, `.conf`, `.env` files, `.editorconfig`, `.gitconfig`) and CMake
  (`CMakeLists.txt`, `.cmake`). **They are hand-written line scanners, not `.sublime-syntax`
  files**: the existing extra languages (Kotlin, Swift, PowerShell, Dockerfile, TOML) are scanners
  too, a sublime syntax needs a second syntax dump or a YAML parse at start-up, and no licence
  text had to be taken from anyone. Nothing was downloaded. They are approximations: no
  regular-expression literals or JSX tags in TypeScript, no string interpolation colours except
  escapes, selectors in SCSS and Less are plain. Vue and Svelte were not added. `line_comment`
  covers them (`//` for TypeScript, Dart, Zig, SCSS, Less; `#` for INI and CMake). The viewer's
  fenced code blocks pick the new languages up when it moves its pin (`ts` fences change from the
  JavaScript grammar to the scanner).
- **Shared commit:** branch `feat/more-languages`, commit `fce8c8e71a9cbf0bab366bf7795f0e3860f9be3f`,
  pinned by the editor. **The shared `main` was not moved** (it is still at `870d620`); merging the
  branch is the owner's call. `cargo test --workspace --release` in the shared repository: the
  `mdshared-highlight` tests (31) and the rest pass; clippy with `-D warnings` is clean for
  `mdshared-highlight` but fails in `mdshared-app` (five warnings in test code, not touched here).
- **Installer.** `packaging\windows\mdedit.iss` is the viewer's `mdview.iss` adapted (same
  `[Setup]` choices: per-user, no elevation, `{localappdata}\Programs\mdedit`, no Start menu folder
  page, lzma2, modern wizard; same tasks wording and registry layout). The extension list is one
  line of the script. The wizard asks: where to install; Tasks page with "Register mdedit for text
  and source files" (checked), "Add Edit with mdedit to the context menu of all files" (off),
  "Create a desktop shortcut" (off); the last page offers Launch mdedit and, if associations were
  chosen, opening Settings at Default apps (off). **The viewer's installer lacks** the context-menu
  task and does not ask for whom to install (it is per-user only, so mdedit's does not either).
  `tools\release.ps1` builds it through `packaging\windows\build-installer.ps1` and uploads
  `mdedit-setup-<version>.exe` as the third asset; the manifest still names the bare exe.
- **Updater and an installed copy.** The shared updater (`mdshared-app` `selfupdate.rs`,
  `updater.rs`) does not look at how the app was installed: it renames the running exe aside to
  `.old` and moves the new one into the exe's own folder, and removes `.old` later. The per-user
  install folder is writable, so an installed mdedit updates in place; the uninstaller removes the
  files it installed by name, and the updated exe is the same name. An update leaves the installer's
  version number in "Apps" stale. Not tested: an update applied to an installed copy.
- **Measured** (release build, this PC): exe 5,650,432 bytes (budget 6,000,000; 5,637,632 before);
  installer 4,277,023 bytes; window shown 78.1 to 81.5 ms over five runs; private memory 27.5 to
  27.9 MiB; keystroke p50 0.47 ms. `cargo test --release --workspace` 378 passed, 0 failed;
  clippy `-D warnings` and `fmt --check` clean.
- **Run:** silent installs into a folder under `target` with `/VERYSILENT /CURRENTUSER` and
  `/TASKS=` of nothing, `assoc`, `assoc,contextmenu` and `contextmenu`: the keys appeared only with
  their task (the ProgId, `Software\mdedit\Capabilities` with 64 extensions, `Applications`,
  `RegisteredApplications`, `OpenWithProgids`; the `*\shell\mdedit` key with its command) and the
  uninstaller removed all of them and the folder. The installed exe answered `--check-update`
  with "nothing newer". A shot of a `.ts` file shows keywords, strings, comments and types in colour
  and "TypeScript" in the status strip.
- **Not run:** the installer's wizard clicked through by a person (only silent installs), "Open
  with" and Default apps as Windows draws them, a double click or the context-menu entry on a file,
  upgrading over an older install, the 62 shots (not re-rendered, so any shot of a `.ts`, `.ini`
  or `.conf` file may have changed), the new languages on Linux, an update applied to an installed
  copy, anything of the interface by the owner. The installer is not code-signed, so SmartScreen
  may warn. The unit tests colour a small sample per language; real files of each were not tried.

## Our own icons (2026-11-13, asked by the user: the same look on every system)

The user approved mockups (stroke 1.25 on a 16 px grid, round ends) and named no icon to
change. The shared `main` is at `870d620` with them; the user asked for them on main.

- How: `mdshared-ui` `glyph.rs` holds each of the 38 icons as SVG path data; `Gfx::icon`
  strokes it with `zeno` (the rasterizer `swash` already brings in) and keeps the mask per
  size; `Painter::icon` has the same signature as before, so neither app's code changes.
  Segoe Fluent Icons, the Fluent System Icons subset for Linux and `Face::Icon` are gone.
  Icons are grey antialiased now, where the font glyphs had ClearType colour fringes.
- A dot is a small circle whose stroke closes over its middle, and the dashed frame of
  Select all is drawn dash by dash, so there is one way of drawing and no fill.
- `cargo test -p mdshared-ui icon_sheet -- --ignored` in the shared repository writes
  `target/icons.png` and `icons-150.png`: every icon at 8.5 to 32 px in both themes.
- Measured after, on Windows: exe 5,637,632 bytes (142 KB more: the stroker and the path
  parser); window shown 76 to 82 ms; private memory 27.4 to 27.6 MiB; keystroke p50
  0.47 ms. 378 tests with `--release` (in a debug build
  `a_request_longer_than_one_slice_is_answered` runs out of its 60 s; whether it did before this change was not checked), clippy and fmt clean. All 62 shots differ from before, as they must.
- Looked at by me: the sheet, `menu-main`, `files-pane`. Not looked at: the other shots.
  The user has not seen the icons in a real window.
- Not done: the viewer still pins the commit before; it needs only its pin moved (the names
  and `Painter::icon` are unchanged) and new baselines for its shots.

## Split view (2026-11-13, the rest of roadmap item 10: the roadmap is done)

- Shift+F8 ("Two files side by side" in the View menu) shows two tabs beside each other:
  the active one, and the tab after it in a second pane at the right. F8, a click on the
  second pane, or a click on its tab gives it the keyboard: the panes change roles and each
  keeps its tab and its side. Any other tab chosen shows in the pane with the keyboard.
  Shift+F8 again, or closing the second pane's tab, ends the split.
- **It is two files, not two views of one file.** The user was told the difference on
  2026-11-13 and said: "leave as is, note it." So one file in both panes is not planned;
  do not build it unless asked. A
  document's caret, scroll, folds and colours belong to the document (`docs.rs` `Ready`);
  showing one document twice needs them taken out of it into a view of their own, which
  was not done. The same file cannot be shown in both panes.
- The pane without the keyboard has no caret and no line band. The wheel scrolls it (by
  lines; not sideways). Its scrollbars show but cannot be dragged, and a first click on it
  only gives it the keyboard: it does not place the caret.
- The halves are equal and cannot be resized; under 520 px of text width one pane shows.
  Nothing in the tab row marks the second pane's tab. The split is not in the session.
- How: `Docs::other` is the second pane's tab and `Docs::activate` swaps the roles when
  that tab is activated. The pane is a seventh region (`Region::Other`,
  `Areas::with_split`), drawn by the same `View::draw` with its hits replaced by one
  `Hit::Other`. Its colours are asked with the new `HighlightService::peek`, which the
  worker answers without making that document the one it parses in the background.
- Run in a real window on Windows by keys: Shift+F8, a letter and Ctrl+S, F8, a letter and
  Ctrl+S, F8, a letter, Shift+F8, F8 (the toast), a letter; and a click on the second pane
  followed by a letter. Each letter was in the right file. Seen in shots only (`split`,
  `split-dark`): the look, word wrap in a pane, colours in both. Not run: the wheel over
  the second pane, closing its tab, a split with the Files pane or the results panel open,
  a narrow window, two large files.
- Measured after: exe 5,495,808 bytes; window shown 77 to 79 ms (six runs; two of three
  runs right after the build were 85 and 90); private memory 27.7 MiB; keystroke p50
  0.48 ms; scroll step p50 0.55 ms. Typing with the split open was not timed (`--split`
  works for `--shot` only). 378 tests, clippy and fmt clean; the shots are byte-identical
  to the build before but `menu-view` and `dialog-shortcuts`; 62 shots.

## Linux at `4280e9d` (2026-11-13, last run of the day)

The same agent, same VM, the editor at `4280e9d` with its own pin of the shared branch.

- Builds (one warning, `first_line`); 377 tests pass (378 on Windows); `tests/shots.sh`
  gives 62 PNGs. Looked at: `split`, `split-dark`, `menu-macro`, `fold`: icons, both panes
  and colours are right. The emoji of `sample.md` draws as a box there (no colour emoji
  font on the VM, most likely; not looked into).
- In the real window, by file bytes: a macro recorded and played twice; a macro played to
  the end of the file from the menu (six lines, each changed once, none added); split view
  by Shift+F8 and F8 (each letter in the right file); a click on the second pane then a
  letter (in that pane's file, at the caret that pane had); copy read back with xclip. By
  screenshot: "Recording" in the status strip, the wheel scrolling the second pane.
- Window shown at 52 to 56 ms, keystroke p50 0.73 ms. The binary is 10,920,416 bytes
  (10,025,128 at `f0c75a6`, 9,572,392 at `4023958`); why it grew by 0.9 MB in a day when
  the Windows exe shrank was not looked into.
- Not done: Wayland; the other 58 shots were not looked at.

## Linux in a real window, and room in the exe (2026-11-13, later)

A Sonnet agent drove the editor on the VM (`dietpi@192.168.1.113`, Debian 13, LXDE and
Openbox on X display `:0`) with xdotool, checking saved files' bytes. Its scripts and work
files are in `~/mdedit-test/` there; it installed xdotool, xclip, wmctrl, scrot and zenity.

- Worked, by file bytes or screenshot: typing, Enter, Backspace, Delete, undo, redo, save;
  find, replace, replace all, go to line; a second launch handing its file to the first
  window (not with `--no-session`, which also turns the hand-over off); Ctrl+Tab, Ctrl+N, the
  Save changes question; reload when the file changes on disk; click, double click, wheel,
  the right-click menu, the main menu, the theme button; word wrap, the function list, find
  in files; move, resize and maximise through the hand-drawn title row (Openbox adds no
  title bar of its own), minimise and close; the folding keys. Idle CPU over 20 s: one tick
  focused, none unfocused. Window shown at 68 ms, keystroke p50 0.71 ms, the 100 MB file
  read in 141 ms. `--trace` prints no memory or CPU figures on Linux.
- Found and fixed in the shared repository, branch `feat/linux-desktop`, commit `1e244ec`
  (on top of `b74e8cb`; everything in it is under `cfg(not(windows))`):
  - Copy, cut and paste did nothing: the clipboard off Windows was a stub. It now uses
    `arboard`, with one clipboard kept for the life of the process. Checked on X11 with
    xclip both ways and between two editor windows. Not checked on Wayland.
  - Every icon was a box. Off Windows the icons now come from a 7 KB subset of Microsoft's
    Fluent UI System Icons (MIT, the licence is beside the font) built into the binary, with
    a second code point for each name in `glyph.rs`. They are a little lighter and smaller
    than Segoe's. Seen in pictures and sensible: those of the bars, menus, panes and
    dialogs. Not seen in any picture: ZOOM, DELETE, LINK, POWER, SELECT_ALL, FULLSCREEN,
    BACK_TO_WINDOW, IMAGE, COPY, CUT, PASTE, OPEN_WITH, MOON, WARN.
- The editor pins that commit (`1e244ec3ed5b62d9e38e2a894d4f3d242d11b6c8`). It is the
  shared `main` since the user said to push it (2026-11-13; a fast-forward from `b74e8cb`).
  The viewer's pin was not touched.
- File dialogs: nothing appeared on the VM, because `rfd` there needs either the desktop
  portal or zenity and the VM had neither. With zenity installed Ctrl+O opens its file
  chooser; choosing a file and Save as were not tried. The editor says nothing when no
  dialog can be shown.
- Not fixed: a double click on the title row does not maximise on Linux; `first_line` in
  `platform.rs` is unused there (one warning).
- With the newer shared code the Windows exe was 6,008,832 bytes. `Cargo.toml` now builds
  `mdshared-highlight` (the glue and the scanners, not syntect) and **the shell crate
  `mdedit` itself** for size: with fat LTO the shell is where every crate's generic code
  is made, and the exe is 5,490,688 bytes. Measured after, on Windows: window shown 75 to
  77 ms; first colours 51 to 52 ms (50 before); keystroke p50 0.46 to 0.50 ms, 0.58 ms on
  the 100 MB file, 0.66 ms in Markdown with wrap; scroll step p50 0.53 ms; the 100 MB file
  read in 101 to 103 ms (96 to 98 was measured earlier in the day, not side by side). All
  60 shots are byte-identical to the build before; 377 tests pass.

## Macros (2026-11-13, the first half of roadmap item 10)

`bins/mdedit/src/macros.rs`; `App::play_macro` in `app/commands.rs`. Written in the main
session.

- Ctrl+Shift+R starts recording and stops it; Ctrl+Shift+P plays the macro. "Play it to
  the end of the file" has no key. The three are in a new "Macro" entry of the main menu,
  which replaces it in place as "View" does. While recording, the status strip says
  "Recording".
- A macro is the keys pressed while recording (key, modifiers, the text typed), given again
  to `key_pressed`. So it holds typing, caret moves, the editing commands and what was typed
  in the find box. Not in it: the mouse, what was chosen from a menu, and the commands that
  open a dialog or change tabs, files or the look (`macros::recordable`). At most 10,000
  keys.
- "To the end of the file" plays again for as long as a run leaves the caret on a later
  line than the run before, and stops on the empty line after the last line break; at most
  100,000 runs. A macro that does not end one line further down runs once. It runs on the UI
  thread and cannot be stopped; the window waits.
- A question or menu that a key of the macro opens ends the playback.
- There is one macro, kept in memory: not saved, not named, gone when the editor closes.
  Each key is its own undo step, as when typed; one Undo does not take a whole run back.
- A search in a file over the size searched at once (`SYNC_FIND_BYTES`) runs on a worker, so
  a macro that searches there would go on before the search is done. Not tried.
- Decision made without the user: "Manage recent files…" left the main menu to make room
  (it is still in "Open recent"), since the menu was as tall as an 800 px window.
- Run in a real window on Windows: Ctrl+Shift+R, Home, "- ", End, ";", Down, Ctrl+Shift+R,
  Ctrl+Shift+P twice, Ctrl+S (three lines changed); and a macro played to the end of the
  file from the menu (a click on the menu button, then arrow keys and Enter), on a file
  with and one without a last line break: every line changed once. Not run: recording over
  the cap, a macro with a search, playback on a large file, the "Recording" field (no shot
  has it).
- Measured after: exe 5,984,256 bytes; window shown 77 to 78 ms; private memory 27.5 to
  28.0 MiB; keystroke p50 0.49 ms. 377 tests, clippy and fmt clean; the shots are
  byte-identical to the build before but `menu-main` and `dialog-shortcuts`; `menu-macro`
  is new (60 shots).

## Code folding (2026-11-13, the rest of roadmap item 9)

`docs/DESIGN.md` section 27. Written in the main session on the core that was already there.

- F9 folds the block the caret's line heads, or else the nearest block around it (the caret
  then goes to the end of that block's first line); on a folded block's first line it opens
  it. Shift+F9 folds every block that starts at the left margin; Ctrl+F9 opens all. The three
  are in the View menu. A block is the lines below a line that are indented more than it, so
  a closing brace stays in view under the folded line.
- A folded line has a grey box with three dots after its text and an arrowhead between its
  number and the text. Neither can be clicked: folding is by key and menu only.
- Up, Down, the page keys, the wheel, the scrollbar and a click go by the lines that show.
  A caret that comes into a fold opens it (Right at the end of a folded line, Ctrl+End, a
  find, go to line, a bookmark, a result of find in files). Several carets move by lines of
  the document, so they open the folds they enter.
- Folds follow edits. An edit that adds or removes lines inside a fold or in its first line
  opens it, and so does a line break typed at the very start of its first line. Undo does not
  fold again. A reload opens them all; they are not in the session.
- Colours are asked for the lines from the view's top to its bottom with the hidden ones
  between, at most 10,000 (`view::shown_lines`): under a larger fold the text is plain until
  the fold is scrolled out at the top.
- The highlight worker fault noted before is fixed: a request that could not be finished in
  one 8 ms slice was started again for ever. Each try now gets twice the time of the one
  before, up to 2 s (`mdedit-highlight`, test `a_request_longer_than_one_slice_is_answered`).
- With folds the caret's blink and moves repaint the whole text region, as with word wrap.
- Run in a real window by keys on Windows: go to line 18, F9, a letter, Down, a letter,
  Shift+F9, Ctrl+End, a letter, Ctrl+F9, Ctrl+S; the saved file had the letters at the end of
  line 17, of line 24 and of the file. Seen in shots only (`fold`, `fold-dark`): the marks,
  folds with word wrap. Not run in a window: the wheel, the scrollbar and clicks with folds,
  the menu's entries, Fold all on a large file (the core looks at the first 200,000 lines).
- The exe went to 6,005,248 bytes with it, so ten more crates are built for size
  (`icu_locale_core`, `icu_provider`, `icu_locale_fallback`, `litemap`, `yoke`, `writeable`,
  `tinystr`, `potential_utf`, `utf8_iter`, `zeno`): 5,978,624 bytes.
- Measured after: window shown 73 to 77 ms (four runs); private memory 27.3 to 28.3 MiB;
  keystroke p50 0.47 ms, the same with two folds on screen. Scroll steps were not measured
  again. 376 tests, clippy and fmt clean; of the 57 earlier shots all but `menu-view`,
  `dialog-shortcuts` and `dialog-about` are byte-identical (the first two list the new
  commands; why the About one differs from the morning's render was not looked into).

## Function list, and v0.2.0 (2026-11-13, half of roadmap item 9)

- Ctrl+Shift+2 (and "Function list" in the View menu) shows the active document's headings,
  functions and types in the left pane, in the Files tree's place; Ctrl+Shift+1 or the folder
  button in its title row brings the tree back, and each command hides the pane when its own
  list is the one showing. A click on a row goes to its line; the row of the caret's place is
  marked and scrolled into view. `bins/mdedit/src/outline.rs`.
- The list comes from the new crate `mdedit-outline` (written by a Sonnet agent to a given
  interface): line-wise rules for Markdown, Rust, Python, the C family, shell, TOML/INI, Ruby,
  Lua and CSS, no parser, so it misses things (parameter lists over several lines, methods of
  a JavaScript class) and other languages show "No list for this kind of file". It is a crate
  of its own so that it is built for size. A worker thread is given the text after every
  change while the list shows and gives up a list it is making when a newer text comes.
- The exe went to 6,050,816 bytes with it. Most of that was not the rules but
  `std::sync::mpsc`: each type sent on a channel costs about 15 KB. The outline and results
  workers and the load and find results of `Docs` now use a list or a slot under a `Mutex`
  (with a `Condvar` where something waits) and the exe is 5,984,768 bytes.
- Measured after: window shown 77 to 79 ms; keystroke p50 0.47 ms, and 0.56 ms while the
  function list shows; scroll step p50 0.40 ms; the 100 MB file loads in 98 ms.
- Not run in a real window: the function list at all (two shots only: `outline`,
  `outline-dark`), so not its clicks, its wheel, its updates while typing or the pane switch.
  The load and find results were run by the tests and the shots only since the change from
  channels.
- Folding came after this release: see "Code folding" above.
- **v0.2.0** was published from this state so that the updater can be tried from 0.1.0
  (`docs/RELEASE.md`).

## Find in files (2026-11-13, roadmap item 8)

- Ctrl+Shift+F looks for the find box's text, with its options, in every file of the Files
  pane's folder, or of the active file's folder when no folder is open. "Find in open tabs"
  (no key) looks in the tabs' text as it is now. Both are in the find box's right-click menu.
  What is found shows in a panel under the text (`bins/mdedit/src/results.rs`): a row for each
  file, a row for each match with its line number and the line, the match marked. A click on a
  match, or F4 and Shift+F4, opens the file and selects the match. The panel's close button
  (and the command "Close the results", no key) closes it and stops a search still running.
- The core is `mdedit_core::search` (`in_buffer`, `in_tree`), written by a Sonnet agent to a
  given interface and reviewed here. A folder is walked on a worker thread: files first, then
  subfolders, by name; `.git`, `node_modules`, `target` and names starting with a dot are
  skipped, as are files over 16 MiB, files with NUL bytes and files that cannot be read (the
  title row says how many). It stops at 20,000 matches, 2,000 of one file. Files are read with
  the editor's own loader, so encodings are detected the same way.
- A folder search reads files from disk: a tab with unsaved changes is searched as it was
  saved, and the match is then selected at the same offsets in the changed text, clamped to
  it, so it can be off. "Find in open tabs" skips tabs not read yet and tabs over 64 MiB and
  says "more were left out"; it runs while the window waits.
- Not built: replace in files, a way to choose the folder or file kinds, keyboard focus in
  the panel (arrows, Enter), a resizable panel (it is 230 px, less in a low window, none
  below about 450 px of window height), Escape to close it, results that follow edits.
- Run in a real window by keys: "ring" typed in the find box, Ctrl+Shift+F with
  `fixtures/sample.rs` open (34 matches in 2 files, 1 file skipped), F4 three times (the
  other file opened in a tab with the match selected), a click on a row, Shift+F4. Not run in
  a window: "Find in open tabs" (two shots only: `results`, `results-dark`), the menu entries,
  the wheel and the scrollbar of the panel, closing it, a search of a large folder, cancelling.
- The exe went over the budget with this (6,041,600 bytes), so fifteen more crates are built
  with `opt-level = "s"`: `icu_segmenter`, `icu_normalizer`, `icu_properties`, `softbuffer`,
  `memmap2`, `icu_collections`, `zerovec`, `zerotrie`, `tracing`, `tracing-core`, `indexmap`,
  `linked-hash-map`, `smol_str`, `miniz_oxide` and `aho-corasick`. Tried and
  taken back: `read-fonts` and `skrifa` save 55 KB but the window shows about 5 ms later;
  `bit-set`, `bit-vec`, `crc32fast` and `base64` save 2.5 KB and seemed to cost 2 ms.
- Measured after: exe 5,979,648 bytes; keystroke p50 0.48 ms; scroll step p50 0.40 ms;
  private memory 27.8 MiB; the 100 MB file loads in 96 ms. Window shown: 79 to 81 ms, and
  the same for the build before this feature run in turns with it (the machine was about
  2 ms slower than when 77 to 78 ms was measured). Find speed with `aho-corasick` built for
  size was not measured apart from the core's tests passing.

## Bookmarks (2026-11-13, roadmap item 7)

- Ctrl+F2 sets or removes a bookmark on the caret's line (also "Toggle bookmark" in the
  text's right-click menu); F2 and Shift+F2 go to the next and the previous one, going round
  at the ends; "Clear bookmarks" has no key and is in no menu (the shortcuts dialog lists it,
  nothing runs it yet). A bookmarked line has a dot left of its number.
- Bookmarks follow edits (`lines::shift_marks` in the core, called from `Ready::changed`,
  so Undo and Redo move them too) and a reload. A bookmark on a line that is joined into
  another goes to the joined line; pressing Enter at the start of a bookmarked line leaves the
  bookmark on the empty line above the text, not on the text.
- They are per tab and are not kept: closing the tab or the editor loses them.
- Not built: a click in the gutter to toggle one, cut/copy/delete of bookmarked lines.
- Run in a real window by keys: two bookmarks set, a line typed above them, F2 twice and
  Shift+F2 with a letter typed at each stop; the saved file had the letters on the right
  lines. The dot was seen in a shot only (`bookmarks`); the menu entry was not clicked.
- Measured after: exe 5,975,040 bytes; window shown 77 to 78 ms; keystroke p50 0.48 ms.
  The text menu is now 590 px tall: it fits an 800 px window with nothing to spare.

## Multiple carets and column selection (2026-11-13, roadmap item 6)

`docs/DESIGN.md` section 25. A Sonnet subagent wrote the core module (`multi.rs`, 17 tests and
a timing test); the shell part was written in the main session.

- Ctrl and a click adds a caret. Alt and a drag selects a column; Alt+Shift and an arrow
  makes or extends one from the keyboard. Escape, a plain click, the page keys or any command
  that is not listed below goes back to one caret.
- At every caret: typed text, Enter (a plain line break, no indent), Tab (a tab character),
  Backspace, Delete, paste, cut, and the caret keys with or without Shift. Each edit is one
  undo step, typed characters included (they do not merge into one step as with one caret),
  and Undo leaves one caret.
- Copy puts the selections one to a line, in text order. Paste gives one line to each caret
  when the clipboard has as many lines as there are carets, else the whole text to each.
- A column selection has no virtual space: on a line shorter than the column the selection
  ends at the line's end, and a drag cannot go past the text of the line under the pointer.
  Columns are display columns (a tab counts to its stop).
- With word wrap on, Up and Down move several carets by lines, not rows.
- With several carets the status strip says how many, the current-line band and the bracket
  boxes are off, and every caret move repaints the whole text region.
- Not built: Ctrl+D style "select next occurrence", Ctrl+click on a caret to remove it, Ctrl+
  Backspace and Ctrl+Delete at every caret (they go back to one caret), line commands at
  every caret.
- Run in a real window: Alt+Shift+Down three times, typing, Right, Backspace, Shift+End,
  Ctrl+C, Escape, Ctrl+End, Ctrl+V (four lines came out), a click and two Ctrl+clicks, typing,
  Ctrl+Z (one step), Ctrl+S; the saved file was checked. That run found that Copy kept only
  the last selection; fixed. Not run in a window: Alt and a drag (the script cannot drag),
  a column over tabs or wide characters, many thousands of carets (core timing only: 100,000
  carets, an insert 75 ms, its undo 68 ms), carets with word wrap on. No shot shows several
  carets: `--shot` has no switch for them.
- Measured after: exe 5,967,872 bytes (32,128 under the budget); window shown 78 to 80 ms;
  private memory 27.3 to 28.1 MiB; keystroke p50 0.48 ms; scroll step p50 0.54 ms; the 52
  shots are byte-identical to the build before.

## Room in the exe (2026-11-13)

`Cargo.toml` also builds `fontique`, `bincode`, `flate2`, `yazi` and `mdshared-ui` with
`opt-level = "s"`. The exe went from 5,987,328 to 5,948,416 bytes (51,584 under the budget).
All 52 shots are byte-identical to the build before. Measured after, three runs each: window
shown 77 to 80 ms (78 to 79 before); keystroke p50 0.48 ms (0.79 before); scroll step p50
0.54 ms on the 100 MB file (0.84 before); with wrap on in long paragraphs 1.20 ms (1.58
before). Why painting got faster was not looked into, and which of the five crates did it
was not separated. Tried and not kept: `encoding_rs` for size saves another 50 KB but reads a
100 MB Windows-1254 file in 551 ms instead of 508; `url`, `idna` and `fdeflate` save nothing.

## Word wrap (2026-11-13, the last of roadmap item 4)

View menu, "Word wrap", or Alt+Z; off by default, kept in the session (`wrap=1`), `--wrap`
for shots. Roadmap item 4 is done with it.

- A line longer than the view goes on in rows below: a row ends after the last space or tab
  that fits, a word longer than a row is cut between characters. The line number stands
  beside the first row only. There is no sideways scrolling.
- Rows are found from column counts (`display::wrap_rows` in the core), with the same
  estimate of widths the view already used for long lines, not from shaped text. Glyphs from
  fallback fonts that are wider than the estimate can run past the right edge and be cut
  there. 16 px are kept free at the right for the caret and the scrollbar.
- Up, Down, Page Up and Page Down go by rows and keep the column in the row. Home and End
  still go to the line's start and end, not the row's. A click and a drag find the row under
  the pointer.
- The scroll position is a line and a row in it. The vertical scrollbar's thumb shows the
  top line's number, not rows: it moves unevenly through a file whose lines differ much in
  length, a document that is one long line has no scrollbar (wheel and keys scroll it), and
  the last line can be scrolled up to the view's top.
- The caret's blink and moves repaint the whole text region with wrap on (without it, only
  the lines involved).
- A document read with a line longer than 262,144 characters is shown unwrapped though wrap
  is on (`WRAP_LINE_MAX`); nothing says so on screen. The figure is the one from reading the
  file: a longer line pasted later is wrapped, at about 20 ms a keystroke for each 1 MB of
  that line.
- Run in a real window by keys and one click on a file of long paragraphs: Alt+Z, Page Down,
  Ctrl+End, Up three times and typing, a click in the middle of a paragraph and typing,
  Ctrl+Home, Down, End. Not run in a window: the scrollbar's thumb with wrap, a selection
  drag that scrolls, resizing the window with wrap on, scale 1.5 and 2, tabs and wide
  characters at a row's end (unit tests only), the View menu's entry (the key was used).
- Measured after: exe 5,987,328 bytes (12,672 under the budget: the next feature needs
  room found for it); window shown 78 to 79 ms; private memory 27.6 to 27.8 MiB; keystroke
  p50 0.79 ms with wrap off (unchanged), 0.79 ms with wrap on in `sample.rs`, 1.58 ms with
  wrap on in paragraphs of up to 3,700 characters (1.42 ms there with wrap off); scroll step
  p50 0.83 ms on the 100 MB file with wrap on (0.84 off). Shots: `wrap`, `wrap-dark`.

## Encoding and line endings from the status strip (2026-11-13, roadmap item 5)

The last two fields of the status strip are buttons now. Roadmap item 5 is done.

- Line endings: the field opens a menu standing on it (Windows (CRLF), Unix (LF), Macintosh
  (CR), the document's own checked). Choosing one rewrites every line break as one undo step
  (`lines::convert_line_endings`), so the tab shows as modified and Undo brings the old breaks
  back. Refused with the same toast as Trim and Tabs to spaces above 8 MiB
  (`ALL_LINES_MAX_BYTES`); the core function took about 5.4 s on 100 MB, 3.3 million lines.
- Encoding: the menu has "Save in" UTF-8, UTF-8 with BOM, UTF-16 LE, UTF-16 BE and the system's
  code page (the current one checked), then "Reopen as" the same five, then "Reopen, detecting
  the encoding".
  - Save in changes nothing on disk: it sets the encoding the next Save writes and marks the
    tab modified (`Doc::encoding_changed`; it is not an undo step). When the text has a
    character the encoding cannot hold, it is refused with a toast naming the character and
    its line.
  - Reopen as reads the file again as that encoding whatever its bytes suggest
    (`LoadOptions::forced`, kept in `Doc::forced` so every later reload of the tab uses it,
    until "Reopen, detecting the encoding"). With unsaved changes the Discard question comes
    first (`After::ReadAs`); the choice is applied only once it is answered. The choice is
    not kept in the session: after a restart the file is detected again.
  - A file read with replacement characters (lossy, read-only) can still be reopened as
    another encoding; its Save in entries and its line-ending field are off.
  - Only those five encodings are offered. The other code pages the detector can choose
    (`Encoding::Legacy` of any `encoding_rs` page) cannot be picked by hand yet.
- Run in a real window by clicks: CRLF to LF, Save, then Save in UTF-16 LE, Save; the file's
  bytes were checked (FF FE, LF breaks). Not run in a window: Reopen as (covered by a `docs.rs`
  test and the core tests), the Discard question before a reopen, the refusal toast, a file near
  8 MiB, the menus at scale 1.5 or 2.
- The shared `Menu::above` is additive (a menu made without it is placed as before). The
  viewer's check was not run for it and the viewer's `rev` was not moved: its checkout belongs
  to another session.
- Measured after: exe 5,965,824 bytes; window shown 78 to 86 ms; private memory 27.6 to
  27.8 MiB; keystroke p50 0.78 ms. Shots: `menu-eol`, `menu-encoding`.

## View aids, first part (2026-11-13, roadmap item 4)

Zoom, go to line, the matching bracket and the other occurrences of the selected word are in;
the current line was already banded, and Show whitespace is in. Word wrap has its own section
above.

- Show whitespace: a dot in each space, a dash with a tick in each tab, and at each line's end
  a down arrow for LF, a return arrow for CRLF, a left arrow for CR (drawn with the text font
  and its fallbacks). Off by default, kept in the session (`whitespace=1`), `--whitespace` for
  shots. Other blank characters (no-break space and the like) get no mark.
- The main menu has a "View" entry that replaces it in place, as "Open recent" does: Files
  pane, Show whitespace, Zoom in, Zoom out, Reset zoom. "Files pane" left the main menu for it.
  Clicked through in a real window (the menu button, View, Show whitespace); the marks
  themselves were seen in shots.

- Matching bracket: with nothing selected, the bracket after the caret (else the one before
  it) and its partner get a grey box. `()`, `[]`, `{}` by nesting of the same kind, looked for
  within 64 KiB each way, strings and comments not told apart. There is no "go to matching
  bracket" command.
- Selected word: when the selection is one whole word (letters, digits, `_`, up to 128
  bytes), its other whole-word occurrences on the visible lines are tinted, case sensitive,
  on lines up to 8192 bytes. Neither aid has a setting to turn it off.
- Both were seen in shots only (`bracket`, `twins-dark`), not in a real window.

- Zoom: 50 to 400 % in steps of 10, kept in the session (`zoom=`), shown in the status strip
  when it is not 100. Ctrl and the wheel, Ctrl+- and Ctrl+0 were run in a real window. Zoom in
  by key is the shared "Ctrl++" (Ctrl+= on a US layout), which no key gives on a Turkish Q
  layout; that binding was not run. `--zoom N` for shots.
- Go to line: Ctrl+G types in the find box's place ("12", "12:5" or "12,5"); Enter goes there
  and gives the keyboard back to the text, Escape leaves. Run in a real window by keys.
- The main menu is as tall as an 800 px window now, so the zoom commands are not in it; the
  next menu entry needs a submenu or a shorter menu.
- `Cargo.toml` also builds `mdshared-app`, `winit` and `chardetng` for size: the exe is
  5,933,056 bytes with these features, window shown 78 to 82 ms, keystroke p50 0.77 ms.

## Unsaved text survives a close (2026-11-13, roadmap item 3)

`docs/DESIGN.md` section 19. Written in the main session.

- Decisions made without the user: closing the window (or Exit) no longer asks about unsaved
  changes when there is a session; the text is kept in `backup\` beside `settings.ini` and put
  back at the next start, the tabs marked modified. Closing a tab still asks. With
  `--no-session` closing asks as before. The backups are written only at close: a crash or a
  power cut loses unsaved text as it did before.
- A file tab gets its unsaved text back as one edit over the file as it is on disk now, so
  Undo gives the file's text. If the file changed on disk between the two runs nothing says
  so, and saving overwrites it.
- Run in a real window by script with `MDEDIT_SETTINGS` on a scratch file: typed into a file
  and a new document, Alt+F4 (no question, two backups, the file untouched); started again
  (both tabs back and modified), Ctrl+S on the file's tab, Alt+F4 (the file had the text, one
  backup left); started again, Ctrl+Z in the new document, Alt+F4 (no backup left).
- Measured after: exe 5,987,328 bytes (12 KB under the budget: the next feature will need
  room found for it), window shown 79 ms, private memory 27.6 to 27.8 MiB, 42 shots unchanged,
  268 tests, clippy and fmt clean.
- Not run: a large unsaved file (the text is written and read on the UI thread); a backup that
  cannot be written (the code then asks as before); a file deleted between the runs (the code
  restores it as a new document); closing before a restored tab was read (unit test only).

## Find and replace (2026-11-13, roadmap item 2)

`docs/DESIGN.md` section 18 describes it. A Sonnet subagent wrote the core part (whole word,
regular expressions, `replace.rs`); the shell part was written in the main session.

- Decisions made without the user: the strip sits under the command bar and opens with
  Ctrl+H; Replace all is Ctrl+Shift+Enter (Ctrl+Alt is AltGr); Replace all refuses files over
  24 MiB because it runs on the UI thread (1.4 to 2.2 s per 100 MB with dense matches); the
  option toggles are text ("Aa", "W", ".*"), not icons; the options are not kept in the
  session.
- The exe went to 6,061,056 bytes with the regex search. `Cargo.toml` now builds
  `regex-syntax`, `png`, `mdshared-fetch` and some cold crates with `opt-level = "s"`, which
  gives 5,977,088. `regex-syntax` at that level also parses syntect's patterns: the first
  colours of `fixtures/sample.rs` came at 55 to 57 ms, which was not compared with a build
  without it.
- Measured after: window shown 79 to 84 ms, private memory 27.8 to 28.0 MiB, keystroke p50
  0.78 ms, 42 shots identical across two runs, 266 tests, clippy and fmt clean.
- Run in a real window by script (keys only): Ctrl+H, a query, Tab, a replacement, Enter twice,
  Ctrl+Shift+Enter, Escape twice, Ctrl+Z, saving after each step; the saved files had 2, then
  29, then 2 replacements, so Replace all was one undo step.
- Not run: no click on the strip's buttons or toggles, nor on the menu's options; a regular
  expression was only used in tests and shots, not typed in a window; Replace all on a file
  near 24 MiB; the user has not seen the strip.
- Not built: "find in all open tabs" waits for the results panel of find in files (item 8);
  replace within the selection (the core takes `within`, the shell always passes the whole
  document); Undo with a box of the bar focused still does nothing.
