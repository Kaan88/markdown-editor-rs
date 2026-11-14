# Handoff: mdedit and the shared crates

Written 2026-11-13, updated the same day after phases 2 and 3; sections 4 to 7 had a truth pass for
the release of 0.5.0 on 2026-11-14 (see "Release 0.5.0" below section 7). Read this first, then
`docs/PLAN.md` (goals, budgets, phases, feature roadmap) and `docs/DESIGN.md` (the design of
phase 1, and section 16 on editing and saving as built).

## 1. Where things are

Three public repositories under `Kaan88`, all MIT, all pushed and clean on `main`:

| Repository | What | Head on 2026-11-13 |
|---|---|---|
| `markdown-viewer-rs` | `mdview`, the Markdown viewer. Its own `docs/HANDOFF.md` describes it | `b4e1617` |
| `markdown-shared-rs` | The crates both applications use | `843b50f` |
| `markdown-editor-rs` | `mdedit`, this repository | this commit |

The editor pins the shared repository by `rev` in its root `Cargo.toml` (for 0.5.0: `7cd6461`, see "Release 0.5.0"), earlier on 2026-11-13 at
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

- **Approval by eye:** the owner has approved the icons and the minimap by looking at them.
  The rest of the interface has not been explicitly approved. (The older notes below say the
  user had not looked at the editor; the owner has since used it and raised the update look and
  the submenus, see 0.3.2.)
- Nothing interactive in the editor was tried by hand: selection, copy into another program, find,
  tabs (drag, middle click, menu), the Files pane, theme switch, reload when the file changes, a
  second launch handing over to the first (not re-checked; the owner has used the editor since,
  see the next item and 0.3.2).
- **2026-11-13, later:** the user tried the build and reported that everything worked. Three
  things they raised turned out not to be faults (Redo after Backspace, a file drop with a tab
  open, heavy shadows; the last is the About card in dark mode, which draws its shadow twice
  over the dimmed window in shared code, and the user said to leave it). While checking them the
  main session drove a real window with synthetic keys, mouse and an OLE drag: typing, Backspace,
  Delete, undo, redo, the right-click menu's Undo, Save, a dropped file, New file and Save
  through the file dialog all behaved. That is not the user's approval of the interface.
- Updates: 0.4.0 was the last release published before 0.5.0 (and 0.3.1, 0.3.2 before it, with an
  installer since 0.3.1). The bar and the About buttons were rendered with `--update-bar`; that an
  update was offered and installed by a running mdedit through the bar (not re-checked). An update
  of an AppImage by itself has not run.
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
  not run. The code for other systems than Windows was built and run on Linux from 2026-11-14
  (see "Linux: the shared Linux code adopted").
- Undo steps are capped by the text they hold (64 MiB), not by their number.
- The first frame of a Markdown file was 125 ms in one run (`fixtures/sample.md`), over the
  100 ms budget set on `sample.rs`; whether phase 1 had that was not checked (not re-checked).
- Idle CPU with the editor's window unfocused was not measured.
- The input-method hand test (`spikes/ime/RESULTS.md`: Turkish Q dead keys and AltGr, a CJK IME)
  was not done. Phase 2 depends on what it shows.
- Linux (2026-11-14, in 0.5.0): the editor builds and runs on CachyOS with KDE Plasma 6 on
  Wayland, and sway ran headless in a container; the AppImage is built in an Ubuntu 20.04
  container. File dialogs, the window identity (taskbar name and icon), downloads and the update
  check use the shared Linux code; emoji draw in colour. What is still open on Linux: GNOME and
  other desktops than those two, X11, fractional scales in a real window, two monitors with
  different scales, input methods, and the file dialogs beside the window (they have no parent,
  so they are not modal; not re-checked). "Show in file manager" opens the folder only
  (not re-checked). `tests/shots.sh` needs LF line endings (a Windows checkout with autocrlf
  breaks it on Linux; `.gitattributes` covers only `fixtures/**`). Nothing in 0.5.0 has run on
  Windows.
- The editor's small-file first frame sits at the edge of its budget: about 60 ms of it is
  syntect start-up before the first colours.

## 5. Decisions the user has not confirmed

These were made to keep moving and are easy to undo:

1. **`panic = "abort"`** in the editor's release profile. It saved 0.9 MB and brought the exe
   under 6 MB. A panic on any thread ends the process, and there is unsaved text to lose. The
   viewer uses `unwind`. Unsaved-session restore (roadmap item 3, built 2026-11-13) is what
   keeps the text, so the abort is less costly than when this was written; still not confirmed
   by the owner.
2. **Crate names** `mdshared-*` (the shared plan called them placeholders).
3. **Step 4 put off**, as above.
4. **`mdshared-app` created** (open decision 1 of the shared plan, taken as it recommended).
5. Open in `docs/PLAN.md` and untouched: the editor's name and the word-wrap default (not
   re-checked). Markdown conveniences were built for 0.5.0 (Markdown commands, list continuation
   with Tab). An installer exists since 0.3.1 beside the bare exe.
6. Made on 2026-11-13 for phases 2 and 3: Tab inserts a tab character (since 0.5.0 there is the
   setting "Indent with spaces", and the indentation of the file is detected and followed, see
   "Findings of the driven test of 0.5.0"); the unsaved mark is a bullet before the file name; a file with replaced bytes is read-only;
   saving runs on the UI thread; the layout cache is keyed by line content; the IME stays off
   until phase 5 (not re-checked).
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

1. **The owner looks at the interface.** Icons and minimap are approved by eye; the rest is
   not explicitly. The hand checks listed in section 4 (typing on the Turkish layout, dead keys
   and AltGr, cut and paste with another program, the three questions, Save as, closing with
   several modified tabs) were not re-checked.
2. **Linux is to be tested by the owner** (section 4; the owner will test it). It has been built
   and run on KDE Plasma 6 and sway only, by agents.
3. **Move the shared `main`.** The shared work lives on the branch `feat/linux` (`7cd6461` is
   pinned by 0.5.0), to become the shared `main`. A push to the shared `main` is refused from
   this PC; the owner moves it, then the `rev`s here and in the viewer can name commits on `main`.
4. The feature roadmap in `docs/PLAN.md` is built to its end. What each item left unbuilt is
   listed in its own dated section below. The list for the next release is in `docs/PLAN.md`
   ("Next release") and in "Release 0.5.0" below.
5. The Files pane could follow changes in its folders with the shared watcher's `set_folders`
   and the tree's `watched_dirs` (shared commit `e93719b`) (not re-checked).
6. Any change to shared code: edit it in `markdown-shared-rs`, run both applications'
   `tests/shots.sh` before and after, push, then move the `rev` in both.
7. Code the editor's shell copied from the viewer because it was generic but had not been moved
   (parts of the window loop, the bar, the find box wiring) is the input for step 4 of the
   shared plan.

## 7. Build and check

```bash
# in this repository; on the PC, keep temp files off the system drive's temp folder
export TEMP="$PWD/target/tmp" TMP="$PWD/target/tmp"; mkdir -p target/tmp
cargo build --release --workspace
cargo test --release --workspace        # 525 passed early on 2026-11-14; later sections give 596 (not re-run for the release)
cargo clippy --release --workspace --all-targets -- -D warnings
cargo fmt --all --check
bash tests/shots.sh target/shots/a      # deterministic; first run generates target/data. 72 PNGs when this was
                                        # written; the dated sections give 158 on 2026-11-14, then 4 more
# The script pins --scale 1.0 for shots that give none: without it the shots take the Windows
# display scale, and a changed display scale changes every picture.
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

**The viewer's no-pixel-change check** is the viewer's own `tests/shots.sh` (added 2026-11-13,
55 states): run it into a baseline folder at a good commit and again after a change, `cmp` every
file, then run the workspace tests (which include the four goldens) and `cargo fmt --check`.

Fixtures are written with exact bytes by `fixtures/make_fixtures.py`; `.gitattributes` keeps git
from converting them.

## Release 0.5.0 (2026-11-14)

mdedit 0.5.0, for Windows and, for the first time, Linux. Version 0.4.1 was never published, so 0.5.0
holds everything dated 2026-11-13 and 2026-11-14 below: command palette, settings, compare two
files, replace in files, the same file in both panes, saved macros, fold by mouse, auto-pairing
and indentation that follows the file, line commands, Markdown commands, "Preview in mdview" with
live preview, the window shadow, and the Linux port.

**The files.** Four assets: the AppImage, the Windows exe, the Windows setup and `mdedit-update.txt`
(one manifest with a `windows-x86_64` and a `linux-x86_64` entry). The AppImage is x86-64, built in
an Ubuntu 20.04 container, so it needs glibc 2.31 or later. It needs the system's libc,
fontconfig, freetype and the X11/xcb libraries, and loads libcurl and the Wayland libraries at run
time. It updates itself. On first run it writes its desktop entry and icon into the user's data
folder and registers its file types without taking defaults; "Make mdedit the default" is a command
in Settings. The Windows files were cross-compiled on Linux (cargo-xwin; Inno Setup under Wine)
and checked under Wine only. How they are made: `docs/RELEASE.md`, "Releasing from Linux".

**Pins.** The shared crates are the branch `feat/linux` of `Kaan88/markdown-shared-rs` at `7cd6461`,
to become its `main` (a push to the shared `main` is refused from here; the owner moves it).
Two forks are carried on Linux and resolved for all platforms through `[patch.crates-io]`:
`Kaan88/winit`, branch `md-0.30.13` (a content margin for the client-drawn shadow) and
`Kaan88/softbuffer`, branch `md-0.4.8` (ARGB buffers on Wayland).

**The last builds made and run on Windows itself** were mdedit 0.4.0 (and mdview 0.2.2). 0.4.1 was
built on Windows but never published. Nothing in 0.5.0 has run on Windows.

### Decisions taken without the owner

The owner said on 2026-11-14: use your best judgement for open questions and make a note. These
were taken that way, or agreed on that day where it says so.

- **The exe size is reported, not a gate** (the owner agreed on 2026-11-14). Launch time is the
  first budget. The build profile was left as it is until a build compiled for speed with
  unwinding can be compared on real Windows.
- **Window shadow.** On KDE the window uses KWin's own protocol. Other Wayland desktops get the
  client-drawn margin (patched winit and softbuffer). X11 gets none by default, because a tiled
  X11 window would draw its shadow over its neighbour.
- **A second fork (softbuffer)** was made beside the approved winit fork, because stock softbuffer
  cannot present a transparent margin on Wayland.
- **Checked menu items have a check-mark column of their own** before the icons: a tinted icon
  alone did not read as "on".
- **Indentation is detected per file and followed.** A driven test found tabs being inserted into
  space-indented files.
- **Submenus stand beside their parent**, and a wheel turn over an open menu is swallowed (it
  closed the whole menu before).
- **Live preview.** The viewer returns to the saved file when the editor is gone (it checks every
  5 s while a tab is live). Text is sent only while the document is modified. The cap is 2 MiB.
- **Versions.** mdedit 0.5.0 and mdview 0.3.0, as feature releases; 0.4.1 and 0.2.3 were never
  published.
- **The AppImage keeps the default squashfs compression.** It adds about 18 ms to start-up; only
  an uncompressed image (2.5 times the size) was measurably faster (numbers in the viewer's
  HANDOFF, section 12).
- **The Windows files are cross-built and Wine-checked.** This was the owner's choice ("find a way
  to cross compile windows").

### Not verified

- A run on real Windows of anything in this release: the command palette, settings, compare,
  split, the hand-off, live preview, the installer's wizard pages, Explorer icons, emoji with the
  variation selector on Windows, and SmartScreen.
- GNOME and other desktops than KDE Plasma 6 and sway (sway ran headless).
- Fractional display scales in a real window on Linux.
- Two monitors with different scales.
- The toast when the editor closes during a live preview: the viewer's return to the file was
  seen, the toast was not.

### Next release (agreed with the owner 2026-11-14)

- Copy a change across in compare.
- Finer sync with the viewer inside long paragraphs and code blocks.
- Live preview for a document that has never been saved.
- Multi-caret support in the line commands (sort, join, headings, table alignment).
- Carried over: the speed-versus-size build comparison on Windows, and a native Windows run of this
  release.

The same list is in `docs/PLAN.md`.

## Findings of the driven test of 0.5.0, fixed (2026-11-14, not published; in 0.5.0)

One commit per item. Design: DESIGN.md section 56 (indentation) and the notes below.

1. **Indentation follows the file.** `mdedit-core::indent::detect` (pure, tested), run when a
   document is read, restored or saved under a new name, never while typing; `Prefs::assist_for`;
   setting "Detect indentation from the file"; status field "Spaces: 4" / "Tabs" (dropped when the
   strip is crowded, the tooltip then sits on the language field). Enter and the Markdown list Tab
   continue a line's own kind. New and compared tabs now take the preferences (they were made with
   the defaults until a setting changed).
2. **Icons.** Shared pin 090d4da: the Lines submenu has SORT, SORT_DESC, SORT_CASE, REVERSE,
   DEDUPE, JOIN; "Make mdedit the default" has CHECK (not visible in a shot: it is listed only
   from an AppImage). Every `--menu` shot was looked at: the only items left without an icon are
   the Markdown submenu's Bold, Italic, Code, Strikethrough, Heading level and Align table (no
   shared glyph fits; they stay empty rather than borrow one).
3. **Wheel.** While a menu, submenu or the palette is open a wheel turn over a menu that does not
   scroll, or outside all of them, does nothing; over one that scrolls it scrolls it. The palette
   takes the wheel only over its card. Same rule in the viewer.
4. **In selection after Escape.** Escape in the find box only takes the keyboard out of it, the
   replace strip stays open, so Ctrl+H found `replace_open` set and skipped capturing the
   selection. `capture_replace_scope(already_open)` now captures a multi-line selection every time
   Replace is asked; a caret or one-line selection with the strip open leaves the range alone.
5. **Palette footer.** Shared `d44bc14`: no count while a message row ("Go to line 25") stands in
   for results and there are no entries.
6. **Folder to a second launch.** The shared request listener dropped anything that was not a file
   (shared `9222f96` lets requests name folders; the paths-only receivers still get files only).
   `mdedit <dir>` and `--folder` go over; `App::open` already made a folder the Files pane's. A first
   launch with a directory argument made a (failed) tab of it; it now takes it as its folder,
   `--folder` winning, and shows the Files pane.
7. **Live dot without an edit.** Text is sent only while the document is modified; the link is no
   longer seeded with a version; going back to the saved text (undo) sends `live_off` once.
8. **Edit in mdedit opening a second window.** Not a bug: `--no-session` neither listens on nor
   forwards to the instance channel (`app.rs`, `session_path` is `None`), so a test window started
   that way takes no request. A normal window listens (`listen_requests_waking`, keyed by the settings
   path: a window started with another `MDEDIT_SETTINGS` than the viewer's launch is another
   instance). Test: `a_request_arrives_over_the_instance_port` (goto, from, place).
9. **Window shadow on KDE.** Measured on this machine (KDE Wayland, release binary,
   `MDEDIT_SETTINGS` scratch, `--no-session`, over the shared `shadow` example's white backdrop,
   one `spectacle` shot): grey 248 of the backdrop darkens to 212 5 px outside the editor's left edge
   (225 at 10 px, 241 at 20 px, 247 at 30 px, 248 at 35 px) and to 229 at 5 px above the top edge; the viewer binary over
   the same backdrop gives the same curve. The KWin route attaches. Only left and top edges were
   over the backdrop, right and bottom were not measured.
10. **Report-only.** (a) `Resized` sets `self.maximized = window.is_maximized()` every time, so a
    window-manager unmaximise updates the glyph. (b) The 6 px band: winit's `RESIZE_BAND` (6) widens
    the input region, shared `edge_at` has no lower bound (`x < EDGE`), so 4 px outside is inside
    both; nothing in code limits it to 1 px. On the default KDE route there is no client margin and
    no band at all (KWin owns the area outside). Not reproduced without input.

NOT run: Windows, X11, GNOME, anything interactive beyond the one screenshot above.

**Correction (2026-11-14).** The "unexplained modified tabs and stray newlines" seen in the driven
test were the owner typing into the test window.

## Emoji presentation, a stray column, the exe size (2026-11-14, after the first 0.5.0 work, not published; in 0.5.0)

**Emoji.** Shared pin moved to `bbb1c7a`; `View::build` (`bins/mdedit/src/view.rs`, the one place
that makes a layout: lines, long-line chunks, line numbers and the metrics samples all go through
it) calls `mdshared_text::push_emoji_runs(&mut b, text, &stack)` after the default and run styles,
with the same family string. Nothing changes on Windows (the helper is empty there). `fixtures/emoji.txt`
(made by `make_fixtures.py`), shots `emoji`, `emoji-dark`, `emoji-caret` (caret after a keycap,
`--goto 3:11`). Looked at: the first line in colour, the bare characters on the second as text
glyphs, the caret right after the keycap. Cost (a scratch benchmark of `build`, 3000 runs, not
committed): ASCII line 14.7-17 us with and 14.8-15.1 us without; a line with emoji about 29 us
against 20 us. No window needed, so no keystroke trace in the sway container.

**The black column.** Cause: with the second pane at the left and its minimap beside it, the minimap
draws the dividing line at the right of the pane, but `split_inner` still cut the text by one pixel
on that side (as if the region drew the line), and nothing painted that pixel (x=543, y 84..773 in
`compare-saved`). Fix: `split_inner(rect, other_left, px, minimap)` keeps the whole region in that
one case; the three callers pass `areas.minimap_other.is_some()`. Test
`a_split_view_leaves_no_pixel_unpainted` paints the screen with a sentinel, composes in full and
finds none left, over same file or two files, the second pane at either side, minimap on or off, the
share 0.3/0.5/0.7 (a pane too narrow for a minimap goes without, so one side only), scale 1 and 1.5
(it failed on exactly that column before the fix). Shots: only `compare-saved` changed (that column
now the page colour, and the second pane's scrollbars one pixel further right). The alpha prefill in
`compose` stays as a guard.

**The exe size.** Built with the documented cross build at commits of the first-parent history
(`dcf7c7d` is on the `main` side; `features-files` came in with the merge `afb3766`): dcf7c7d 5,892,096;
afb3766 (merge: command palette, compare, replace in files) 6,058,496 (+166,400); eedd8be (one
file in both panes) 6,072,832 (+14,336); 58d27ee (pin a3e2ef0: sister, place, shadow) 6,079,488
(+6,656); 340c153 (hand-off with mdview) 6,107,648 (+28,160); e2aae5b (live preview) and d3d3643
6,111,744 (+4,096, then 0). So three quarters of the 220 KB is the palette, compare and replace in
files branch, not the hand-off. Cause checked: `Cargo.lock` and the dependency features are the same
at dcf7c7d and d3d3643 apart from the shared pin; a link map of the unstripped build has no wayland,
zbus, x11rb or xkbcommon symbol, so nothing Linux-only reaches the Windows exe. `.text` +155 KB,
`.rdata` +10 KB. Symbol sizes (linker map, dcf7c7d against afb3766, `.text`): replace-in-files UI
23 KB and its worker 10.5 KB; the palette 13.8 KB plus the shared palette 10.4 KB and fuzzy match
6.3 KB (they were in the shared crate already but unused, so not linked before); the diff for compare
13.8 KB and compare 5 KB; the app's event code for the three +48 KB. That is about 3,500 new lines
of non-test code at `opt-level = "s"`: honest code, no fix, the release profile untouched.

## Window shadow routes, settings shots, DESIGN, timings (2026-11-14, after the hand-off work; not published; in 0.5.0)

Part A is commit `1d96351` (shadow), part B the commit after it. Design: `docs/DESIGN.md` sections
54 (hand-off, live preview) and 55 (shadow). The shadow is the viewer's adoption copied
(`~/src/md/view` `7734d6a`, merge `5dad417`; `markdown-shared-rs/docs/LINUX.md` "Window shadow").
Shared pin is now `99f810c` everywhere; winit and softbuffer come from the two patched forks
(`[patch.crates-io]`), and `mdshared-ui` has `client-margin`.

**What was built.** `Shadow::prepare`, `attach_mode(.., Mode::from_env())`,
`set_premultiplied_alpha(true)` when `wants_alpha()`, `set_min_content_size`. `App.size` is the
content size; `Resized`, `ScaleFactorChanged`, `CursorMoved` and the saved window size convert; the
present path puts the content at the margin offset (rationale per line in the commit message and
DESIGN 55). The edge test has no lower bound (shared `edge_at`; tests for x < 0). Placement
(`--place`, side by side) hands `place::set` a window-geometry rectangle; on KDE that is KWin's
`frameGeometry` on both routes, elsewhere on Wayland `set` returns false and the toast says so, so
the margin cannot make it wrong there. Not handled: forced `MD_SHADOW=client` on X11, where
`place::set` (shared) subtracts outer minus inner and the inner size includes the margin; Auto
never picks that route.

**Alpha finding.** Shared `Bitmap` fills, blends and blits keep or write 255, and the minimap's
bitmap is cleared (255) before it is blitted. `Bitmap::new` is all zero, though, so a pixel no region
paints stays at alpha 0. A temporary scan of all 158 shots found exactly one: `compare-saved`
(minimaps on in a compare view), column x=543, y 84..773, black in the shot before and after. Cause
(not fixed, left as drawn): the other pane's rectangle is cut by one device pixel on the divider
side (`split_inner`) although its minimap, not the divider, is at that side. Fix made: `compose`
sets alpha 255 when it allocates `screen` (colours unchanged); test `a_composed_frame_is_opaque`.
Proposed for shared: `Bitmap::new` should initialise alpha to 255 as its doc comment says.

**Proofs run (this machine, headless: no window on the host display, no pointer, no keys).**
- `cargo test --release --workspace`: 596 passed, 13 ignored (593 before; new: saved size over five
  restarts at six scales, resize band outside the content, opaque frame; plus the ignored cost
  test below). Clippy `-D warnings` and `fmt --check` clean.
- `tests/shots.sh`: the 158 shots of the build before part A and after it are byte-identical
  (`cmp`); part B adds 4 (`settings-preview*`), the 158 are still identical.
- Windows cross build (`tools/release-cross.sh --no-installer`, Docker, cargo-xwin): compiles with
  `--locked`; exe 6,111,744 bytes after part A, and the same 6,111,744 at `d3d3643` (different
  hashes, section padding). **That is over the 6,000,000-byte budget and was before this work**
  (the 0.5.0 release exe was 5,892,096; the hand-off work in between grew it). Not run on Windows.
- Live window, sway headless in Docker (image `mdedit-sway`, built in
  `~/.cache/md-linux-test/edit-sway/` from archlinux with sway, swaybg, grim, imagemagick;
  `WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman`, 1600x1000 output with a
  light solid background, `--cap-add SYS_NICE` because the sway binary has file capabilities;
  scripts `run.sh`, `inner.sh`, `steps.sh`; the release binary and `fixtures/` mounted read-only;
  `mdedit --no-session sample.rs`; screenshots with `grim`, outputs in
  `~/.cache/md-linux-test/edit-sway/out/`). Client route (Auto on sway) against `MD_SHADOW=none`:
  | state | `swaymsg get_tree` rect, client | rect, none | content crop vs none (`compare -metric AE`) |
  |---|---|---|---|
  | start | 160,100 1280x800 | same | 0 differing pixels |
  | `resize set 900 620` | 350,190 900x620 | same | 0 |
  | fullscreen | 0,0 1600x1000 | same | 0 (the whole output) |
  | fullscreen off | 350,190 900x620 | same | 32 px: the caret (2x19 at 61,84), blink phase |
  The rectangle equals the content size, the shadow shows around it (e.g. 10 px left of the window
  the background goes 224 to 214; none: 224), no stale margin after the resize, fullscreen has none.
  Screenshots: `client-initial.png`, `client-resized.png`, `client-full.png`, `client-back.png`, and
  the `none-*` set, in the same folder. Looked at: `client-initial.png`, `client-resized.png`,
  `client-full.png`.

**Part B.**
- Shots looked at (`~/.cache/md-linux-test/edit-base/shots-b/`): `settings`, `settings-narrow`,
  the new `settings-preview`, `-dark` (not opened), `-scale`, `-narrow` (the Preview section scrolled
  into view with the new `--settings-section Preview` shot flag), `menu-view-md-linked-dark`,
  `menu-view-rs-linked`, `menu-tab-md-linked-dark`, `previewing`. Nothing was off: the help line
  of "Live preview while typing" fits whole in all four layouts, the nav marks Preview, the
  entries and their checks are aligned, the status strip says "Previewing". The first screen of
  the existing settings shots never shows the new row (it is below the fold), hence the new shots.
  The help-line test already iterated every row; it now also asserts the live preview row exists with
  a help line.
- DESIGN 54 (hand-off, live preview) written against SISTER.md.
- Timings, in the sway container (the only way to get a window headless); before-build (`d3d3643`)
  and after-build interleaved, 8 rounds each, `sample.rs`, 300 keystrokes, 200 scroll steps; min /
  median:
  | | before | after (client shadow) | after, `MD_SHADOW=none` |
  |---|---|---|---|
  | window shown (ms) | 72.7 / 77.0 | 71.8 / 74.6 | 72.4 / 74.7 |
  | private memory (MiB) | 19.1 / 19.3 | 19.2 / 19.25 | 19.1 / 19.1 |
  | keystroke p50 (ms) | 0.55 / 0.56 | 0.49 / 0.50 | 0.48 / 0.49 |
  | keystroke p95 (ms) | 0.63 / 0.75 | 0.54 / 0.86 | 0.51 / 0.53 |
  | scroll p50 (ms) | 0.50 / 0.50 | 0.44 / 0.45 | 0.43 / 0.44 |
  No regression; the after builds are a little faster on keystroke and scroll (the pinned shared
  code and a changed link differ too, so it is not credited to the shadow). The Linux binary
  grew 12,033,952 to 12,075,104 bytes. Numbers are from sway headless (pixman, no GPU), not
  from the KDE session.
- Cost of the live preview's send on the UI thread: test `live_send_cost_on_the_ui_thread`
  (`app/handoff.rs`, ignored; run with `cargo test --release -p mdedit live_send_cost --
  --ignored --nocapture --test-threads=1`). A 193,500-byte document in 8,400 lines, 300 edits, a
  listener standing in for the viewer: `link_poll` with a send is 0.3 microseconds p50 and p95
  (max 1.4 to 3), with nothing due 0.07 microseconds; the edit itself 1.6 microseconds. The
  rope snapshot and the slot hand-over are what it does; the sends were real (the stand-in
  received updates with the whole 193,500 bytes; the courier coalesces, 3 of 300 arrived).

**NOT run.** Anything on the host display (nothing was opened there), the pointer or keyboard
(so: the edge resize from the outer band, a click through the margin, `drag_resize_window`,
focus changes with the real compositor), the Windows exe, GNOME (Mutter), X11 and Xwayland, KDE's
own route in a window (the KWin route is the old code and the shots show it unchanged), fractional
scales, `--place` and side by side with the client route (no desktop to place on in sway; KDE's
script not run). The owner has not looked at the shadow or the settings shots.

## Hand-off with mdview (2026-11-14, parts 1-4; not published; in 0.5.0)

Built: shared pin 1e39bc1; receiving (`--from`, `--follow`, `--place`, `--goto L[:C]`, `--run`, `--demo-link`; requests
from a second launch go to the running window); "Preview in mdview" (Ctrl+Shift+V, View and tab menus, palette) and
"Preview side by side"; linked mode (`bins/mdedit/src/link.rs`: at most 10 updates a second, newest wins, a courier
thread so a hung viewer never blocks the window, nothing runs unless a link exists); live preview of unsaved text
(setting "Live preview while typing", on by default; the whole text and caret line, never over 2 MiB, one toast).
Ctrl+Shift+V still pastes in the find box. Tests: 593 pass, clippy and fmt clean.

Ran for real (KDE Wayland, DP-2, both apps running): side by side placement (editor 0,0 1720x1396, viewer beside it);
a scripted edit shows in the viewer unsaved with the live dot in its tab. When running two apps on one machine for
this, each needs its own settings folder and BOTH variables for the other app: `MDVIEW_SETTINGS` and
`MD_SISTER_MDVIEW_SETTINGS` for the viewer, `MDEDIT_SETTINGS` and `MD_SISTER_MDEDIT_SETTINGS` for the editor (the viewer's
liveness check looks the editor up by the second one and otherwise ends the link after 5 s).

NOT done: part 5 (window shadow adoption), timings (keystroke with a link, send cost), the `--goto 20` second-launch
check, regenerating the shots (settings shots will differ after the new row), DESIGN.md section. NOT run: Windows,
X11, GNOME, anything interactive.

## One file in both panes of the split (2026-11-14, after the first 0.5.0 work, not published; in 0.5.0)

Design: `docs/DESIGN.md` section 53 (written before the code). Commits on `main`: `a778d7f` (the
flaky test), `b115477` (the design), `2c306c3` (the refactor, no behaviour change), `eedd8be` (the
feature), `1938963` (tests); the docs commit after them.

- **The flaky test** `a_change_on_disk_does_not_replace_unsaved_text`: `open()` starts a load on a
  thread and `load_now()` replaces it. When the first thread had finished, its result sat in the
  queue, and `wait_load` woke on it although `apply_load` then dropped it (old revision), so the
  test went on before the real load had finished. `wait_load` now drops results of replaced loads
  and closed tabs before it decides to wait. A new test reproduces it deterministically; the old
  one passed 200 runs twice under a full `cargo build` load.
- **Refactor (`2c306c3`)**: `ViewState` holds what one way of looking at a document is (carets,
  scroll, folds, colours, find mark, wrap cache); `Ready.view` is the pane with the keyboard,
  `Ready.twin` the other. Proof: 147 of 147 shots byte-identical to the build of `f6c92d5`, 555
  tests, clippy and fmt clean.
- **The feature**: "Show this file in the other pane" (View menu, tab menu, palette; no key: none
  was obviously free). Both panes keep their own caret, selection(s), scroll, folds and minimap;
  an edit, Undo, Redo, Replace all or a macro in either is mapped through in the other
  (`Change::pieces`, `Change::map_offset` in core); the tab appears once, with the second-pane
  line under it running the width of the tab; closing the tab closes both views; unsplitting (or
  opening another tab for the keyboard) keeps the view that has the keyboard (or leaves the
  document in the other pane with its view); "Compare with the other pane" is greyed for one
  file; the session keeps a same-file split as the existing `split=` line (the path is the active
  file's) and restores it with a second view. Highlight requests carry a `slot`, so the two views
  of one document get their own colours from the one parse.
- **Decisions made without the owner**: folds per view (not shared); find marks and the match list
  shared, the current match per view; bookmarks shared; the unfocused view's caret is not drawn;
  the focus keys swap the two views and the panes' sides, as for two tabs; "Compare with file" or
  "with saved" end the same-file split first; flags for shots: `--split-same`, `--other-goto`,
  `--other-select`, `--other-fold`, `--other-scroll`; the three-way difference of the view-menu
  and tab-menu shots is the new row only.
- **Tests**: 568 pass, 12 ignored (554 before this work; 14 new: 2 core `Change` mapping, 1
  highlight slots, 1 `wait_load`, 5 in `docs.rs` (mapping through undo/redo, replace all and
  several carets, folds, find per view, closing/unsplitting) plus 1 for another tab taking the
  keyboard, 4 in `files_tests.rs` (commands and menus, a macro, the session, closing), and an
  extended session test).
- **Shots**: 151 now. The 147 earlier ones are byte-identical to the baseline except the six that
  list the new command (`menu-view`, `menu-view-checked`, `menu-view-checked-dark`,
  `menu-view-compare`, `menu-main-view`, `menu-tab`). New: `same-file-split` (selection in the
  left pane, a fold and another place in the right, minimaps, light), `same-file-split-dark`
  (scrolled differently, minimaps), `same-file-split-left` (second pane at the left, word wrap,
  Markdown, 900x600), `same-file-split-find` (find marks in both, dark, minimaps). I looked at
  all four and the two menu shots.
- **Measured** (this machine was loaded by other work: load average 16 to 29, so medians are
  noisy and the minima are the fairer comparison; 10 interleaved runs, base = `f6c92d5` build):
  | | base min / median | now min / median |
  |---|---|---|
  | `sample.rs` window shown (ms) | 88 / 95 | 82 / 97 |
  | `sample.rs` private memory (MiB) | 20.6 / 20.7 | 20.6 / 20.65 |
  | `sample.rs` keystroke p50 (ms) | 0.54 / 0.57 | 0.51 / 1.04 (one pair of noisy runs) |
  | `sample.rs` scroll p50 (ms) | 0.47 / 0.55 | 0.46 / 0.49 |
  | `50k.rs` (quieter second batch) shown | 70 / 90 | 76 / 86 |
  | `50k.rs` keystroke p50 / p95 | 0.47 / 0.51 and 0.49 / 0.94 | 0.47 / 0.50 and 0.49 / 0.65 |
  | `50k.rs` scroll p50 | 0.45 / 0.46 | 0.45 / 0.46 |
  | `100mb.txt` shown / private / keystroke p50 / scroll p50 | 182 / 124.4 MiB / 0.63 / 0.65 | 181 / 124.4 MiB / 0.69 / 0.74 |
  50,000-line file in both panes with the second at line 25,000, typing in the first (300
  keystrokes): p50 0.66 min / 0.69 median, p95 0.71 / 1.14, private 23.1 MiB (one pane: 22.1);
  with both minimaps: p50 1.10 / 1.12, p95 1.14 / 1.53, scroll p50 0.65 / 0.68. All far under
  the 8 ms budget. Colouring the second pane's far position costs the parse up to there (about 1.8
  s of CPU to line 40,000), the same as `--goto 40000` already did in one pane.
- **Not run**: everything interactive (no mouse, no keyboard in a real window: the menu and tab
  entries were clicked by no one, the tab underline was seen only in shots); Windows (nothing of
  this ran there); the owner has not looked at it. Not built: a key for the command; the second
  view's caret drawn while unfocused; folds shared as an option.

## Typing, Markdown, settings, shadow (2026-11-14, 0.5.0, not published; in 0.5.0)

One machine (CachyOS, Wayland); nothing run on Windows, nothing driven by hand: tests, `--shot`
and `--trace` only. Design: DESIGN.md sections 43 to 48. Commits: A `4149108`, line commands
`a2fb6eb`, typing help `d24e906`, Markdown `2f23d38`, settings `76ffe89`, then shadow and 0.5.0.

- Sent-back items: in-selection replace governs counter, current match, marks and minimap; fold
  chevron without tooltip, centred in the gap; the fold-box tooltip clip is in the shared crate
  (`Bubble::tip`/`Bubble::hint` in mdshared-ui overlay.rs size the box from a fractional width;
  `w.ceil()` there is the proposed fix; not rebuilt or verified).
- Keys: Ctrl+J, Ctrl+B, Ctrl+I, Ctrl+Shift+H, Ctrl+, were free; Link has none.
- Review round (same day): submenu placement (`sub_side`), indent with spaces for blocks (`indent_with`/`outdent_with`), settings help lines fitted, shared pin `9c1290b`, replace box aligned with the find box (DESIGN 49).
- Timing (sample.rs, 300 keystrokes, noisy machine): before p50 0.51-0.62 ms, p95 0.76-1.52 ms;
  after p50 0.54-0.90 ms, p95 1.09-1.30 ms; first frame 74-100 ms both; private 20.6 MiB.
- Tests: 525 pass with `--release`; clippy and fmt clean. Shots in `~/.cache/md-linux-test/b4`.
- Not run: anything interactive, Windows, Wayland shadow (KWin), the settings dialog by hand, the
  AppImage "make default" row, "Check now".
- Commit trailers from here on name Claude Sonnet 5.5 (the earlier ones in this series, Fable 5.2).

## Command palette, compare two files, replace in files: the interface (2026-11-14, branch `features-files`, not merged at the time, not published; in 0.5.0)

Wired three finished pieces of headless logic into the window, on the branch `features-files` (from
`79e70ec`). Done on one machine (CachyOS, Wayland); nothing run on Windows; nothing driven by hand
(only tests, `--shot`, and `--trace --exit-after-ms`/`--type-steps`/`--scroll-steps`). DESIGN 50
(palette), 51 (compare in the window), 52 (replace in files in the window).

**Built**
- **Command palette** (`Ctrl+P`): `palette.rs` (sources), `app/palette_events.rs` (the host).
  Open tabs, recent and the folder's files (worker, 5,000 cap, kept until the folder changes), `>`
  commands with hints and greyed ones, `:` go to line, `@` symbols, `?` the prefixes.
  `--palette <query>`.
- **Compare two files**: `compare.rs` (`Align`, `Compare`, the worker with a 150 ms debounce),
  `app/compare_events.rs` (commands, F7/Shift+F7, the per-frame checks), changes in `view.rs`
  (filler rows, tints, marks inside lines), `minimap.rs` (tints, no text on fillers), `docs.rs`
  (`Ready::align`, `read_only`, `Doc::title`, `Docs::open_copy`), core `fold.rs` (`Folds::suspend`).
  Compare with the other pane, with a file, with saved (a read-only "name (saved)" tab at the left);
  in the View menu, the tab menu and the palette. `--compare A B`, `--compare-saved`,
  `--next-difference N`. No merge arrows.
- **Replace in files**: `replace_ui.rs` (state, workers, dialog and report drawing),
  `app/replace_files.rs` (the steps); a replacement box and "Replace all…" in the results panel, the
  confirmation dialog (tick boxes, preview, backups, skipped files), a progress line with Cancel, the
  report and "Undo replace". `--results-replace`, `--replace-dialog`, `--replace-report` for shots.

**Decisions made without the owner**
- F7 and Shift+F7 for next and previous difference; no existing binding used them. `Ctrl+P` for the
  palette (it was unbound).
- The saved copy is at the left, the edited text at the right. The View menu has the three start
  commands and Stop comparing; the difference keys are in the palette and on F7 only. The tab menu
  has Compare with file… and Compare with saved.
- While comparing, the Word wrap item still flips the setting but does nothing until the comparison
  ends; fold commands do nothing (folds are kept and follow edits, and come back).
- The marks inside a line are not drawn while the diff is out of date; the strip says "Comparing…".
- Difference colours are fixed per theme in `compare.rs` (the shared palette has none).
- A greyed command in the palette cannot be chosen; the file list is not built for the empty query.
- Replace in files: Enter replaces and Escape cancels in the dialog; replacing with an empty text
  is allowed; the preview shows the first lines of the file, not a diff of all of it; a new search
  or a replace ends the previous report and its undo.
- A fixture pair, `fixtures/compare-a.rs` and `compare-b.rs` (shots), and `shots.sh` makes a fresh
  folder for the replace shots (the report shot really replaces).

**Checks run**: `cargo test --release --workspace` 517 passed before (the branch point), 542 after
(25 new: 17 in `app/files_tests.rs`, of them 9 for the palette, 1 for replace in files and 7 for
compare; 5 in `compare.rs`; 1 each in `fold.rs`, `args.rs` and `replace_ui.rs`); clippy `-D warnings` and `fmt --check`
clean. `tests/shots.sh`: 98 shots before, 117 after; the 19 new ones were looked at (palette x8,
compare x7, replace in files x4). Of the 98, 86 are byte-identical and 12 differ, all because of
what was added: `menu-main`, `-dark`, `menu-main-view`, `menu-main-macro-dark`, `menu-tall-main`,
`menu-tall-main-end` (the main menu has "Command palette…"), `menu-view`, `menu-view-checked`,
`-dark` (the Compare items), `menu-tab` (the two compare items), `dialog-shortcuts` (new commands),
`files-pane` (the fixtures folder holds two more files).

**Measured on Linux (window shown, `sample.rs`, 10 interleaved runs of each, branch point against
the branch)**: window shown median 73.6 ms before and 72.8 ms after (range 68.5 to 100.2 and 67.5
to 99.3); private memory 20.6 to 20.8 MiB both; idle CPU 80 ms both; keystroke p50 0.47 ms both;
scroll step p50 0.42 ms both (6 runs each); the exe grew 11,476,744 to 11,683,112 bytes. With
compare on (two 63 and 72 line files): keystroke p50 0.41 to 0.42 ms, scroll p50 0.34 to 0.35 ms;
two 50,000 line files with 27 differences: keystroke p50 0.42 to 0.43 ms (p95 up to 0.71),
scroll p50 0.40 ms. A first build of the compare work scrolled 0.07 ms slower (paint 0.27 to 0.34):
bisected to `Docs::sync_wrap` taking `ready_mut()` and writing the fold flag on every frame;
it now writes only when the value changes and the figures are the same as before. Not confirmed
from the trace that the compare was on while the `--type-steps` keystrokes were made (it starts
when both files are read, before the steps, but I did not print it).

**Not run**: anything interactive. Nothing was clicked or typed by hand: the palette (opening,
keys, clicking a row, the wheel), the compare commands from the menus and the file dialog
(Compare with file…), F7, scrolling and typing in a compared pane by hand, the minimap click in
compare mode, dragging the divider while comparing, the replace dialog's clicks, tick boxes and
preview under the pointer, Cancel while a large replacement runs, Undo replace by click. Windows:
nothing; the blocking dialog path for Compare with file… (`#[cfg(windows)]`) was written the way
Open files is and has not been built. A real replacement of many files, and a compare of two very
different large files (the approximate case), were not timed in a window. The shots and the tests
stand in for these only as far as they go; the owner has to look at the interface.


After the merge of `origin/main` (dcf7c7d) into `features-files`: 554 tests pass in the workspace
(153 mdedit, 335 core, 30, 14, 21 and 1 in the other crates), clippy `-D warnings` and `cargo fmt
--check` clean; the shot set is 147 (128 on main plus 19 new). 13 of main's 128 differ: the six
menu shots with the View and tab menus' Compare items (menu-main*, menu-view*, menu-tab, menu-tall-*),
dialog-shortcuts (new keys), files-pane (the two new fixtures) and dialog-recent (the path of the
checkout it was rendered in). One test, `a_change_on_disk_does_not_replace_unsaved_text`, failed
once in the first full run and passed in five later runs; it was not looked into. Timings were
measured before the merge, not again after it.

## Gaps closed: icons, chevrons, split, macros, find memory, title row (2026-11-14, still 0.4.1, not published; in 0.5.0)

Done on one machine (CachyOS, Wayland), nothing run on Windows, nothing driven by hand: only
tests, `--shot`, and `--trace --exit-after-ms`. Merges first: `3daf35d` (core-edit) and `b4f26e7`
(core-files, with the core's `fuzzy.rs` removed). Shared pin `d6e6cf1` (later `9c1290b`).

- **Icons**: every menu item has its glyph, the replace strip's toggles are icons, shots for the
  menus that had none (`--menu` names added; see `tests/shots.sh`). DESIGN 11.
- **Fold chevrons** (DESIGN 37): `10f8d25`. Unit tests for the hit cell and the click.
- **Split view** (DESIGN 38): divider, ratio/side/file in the session, unfocused-pane click,
  scrollbars, Shift+wheel, tab underline. `f579149`.
- **Macros** (DESIGN 39): `macros.txt`, named, bounded. `1d65863`.
- **Find memory and replace in the selection** (DESIGN 40, 41): `c44cdf8`.
- **Title row** (DESIGN 42): `a65b61e`.

**Same file in both panes: what would have to move.** Not built (the owner said leave it). The
caret and selection (`sel`, `extra`), scroll (`top`, `top_row`, `left`), folds (`folds`), find marks
current (`find.current`) and the colours (`colors`, `map_colors`, keyed in the highlight service
by `DocId`, with `peek` for the second pane) are fields of `docs.rs` `Ready`. A second view of the
same document would need these in a per-view struct (one per pane), with the buffer, history and
find matches staying in `Ready`; the highlight service would be keyed by (document, view), the
minimap plan already is per pane (`map_lines`, `map_plan`), and `Region::Other` would draw a view,
not a tab. `Docs::other` is a tab index today; it would become a view of any tab.

**Decisions made without the owner**
- Fold chevrons: hit area is the right 24 px of the gutter row, not the whole gutter.
- Up/Down in find and replace boxes take the history only when it is not empty (DESIGN 40).
- "In selection" is on by default only when the selection spans more than one line; the match
  counter still counts the whole file; the toggle shows only while a range is kept.
- Macros: playing a saved one makes it the "last" one that the play keys run.
- Title row: deferred drag off Windows only; the 4 px slop is a guess.
- Second pane's tab marker: a 2 px muted underline, chosen to stay quieter than the active tab.

**Measured on Linux (window shown, `sample.rs`, 10 interleaved runs each)**: before 70 to 92 ms
(median about 82), after 68 to 86 (median about 80), so no change; private memory 20.6 to 20.8 MiB
before and after; keystroke p50 0.52 ms before, 0.49 after; scroll step p50 0.44 ms both. The exe
grew 11,409,136 to 11,476,832 bytes (this Linux build, all of the work above). Macro and find
history loading: macros are read on first use; the session's histories are 40 short strings. A
launch with a session holding full histories was not timed.

**Checks run**: `cargo test --release --workspace` 516 passed (404 at the start of the work),
clippy `-D warnings` and `fmt --check` clean; `tests/shots.sh`: every shot without a menu, a
replace strip, fold, minimap-fold or split was byte-identical to before each part; new shots were
looked at one by one.

**Not run**: anything interactive. Not run by hand or seen: dragging the divider, the clicks in the
second pane, the fold chevron hover and click, Up/Down in the boxes, the in-selection toggle with
the mouse, the macro prompt and dialog, the title row double click on Wayland (it needs the
mouse; only the decision is unit tested). Nothing was built or run for Windows, including the
deferred title drag's `cfg`. The fold-box tooltip in `fold-box-hover` is cut ("Open the block
(F...") by the shared bubble width; not investigated.

## Linux: the shared Linux code adopted (2026-11-14, still 0.4.1, not published; in 0.5.0)

Run on one machine: CachyOS, KDE Plasma on Wayland, cargo 1.99, build dir
`~/.cache/md-linux-test/edit`. Shared pin moved to `84de734a1aaecc50cca80b23bacee127b6778ef3` (now `d6e6cf1`, see the 2026-11-14 icons section)
(branch `feat/linux`, not yet on shared `main`). Everything below is `cfg(not(windows))` unless
said; **nothing was built or run for Windows**. The Windows code paths are the old ones.

- **Repaint**: `present` copies the whole frame unless `buffer.age() == 1` (was `== 0`).
  Wayland swaps two buffers (age 2) and a resized buffer keeps its age. Not seen on screen: the
  glitched toolbar on maximise was not re-tried (no driven tests on this run).
- **File dialogs**: Open files, Open folder and Save as use the shared `pick_*_async`; the helper
  thread's `done` only posts `Wake::Dialog(Picked)`, handled on the UI thread (`dialog_done` in
  `app/commands.rs`). `dialog_busy` ignores a second request. Save as inside a flow: `save_as`
  returns `SaveAs::Waiting`, `Picked::Save` carries the document id and the `After`; a result calls
  `request(after)` (as the Save button did), a cancel, a failed write or a closed tab calls
  `cancel_pending`. Windows keeps the blocking calls (`platform::pick_files`, `pick_save`).
  `rfd` is no longer a direct dependency off Windows. **Not run**: no dialog was opened (the
  owner's PC); the flows are untested beyond compiling. Known gap: while a Save as dialog of a
  close flow is open the window is live, and another close request made meanwhile would have its
  question cleared by the cancel.
- **Fonts**: the editor's `EXTRA_FALLBACKS` is gone; Windows calls `mdshared_text::add_fallbacks`
  on its font copy, elsewhere `shared_fonts()` has it. Code font string off Windows ends
  `..., Noto Color Emoji, monospace`. Looked at in `md-light.png`: the emoji is a coloured face.
- **Window identity**: `window::identify(attrs, "mdedit", assets/mdedit.png)`;
  `desktop_entry::ensure(..)` once in `run`. Run with `$APPIMAGE` set and `XDG_DATA_HOME` on a
  scratch folder: `mdedit.desktop` and the icon were written, the entry looks right. New command
  `MakeDefault` ("Make mdedit the default for text files", no key), in the main menu after
  "Keyboard shortcuts", only off Windows, only when `$APPIMAGE` names a file, hidden once
  `is_default`. Seen in a `--shot` with `$APPIMAGE` set; the command itself was **not run**
  (it would edit the owner's `mimeapps.list`).
- **Updates**: `--check-update` reaches GitHub on Linux (the manifest says 0.4.0, so "nothing
  newer"; a 404 and a refused connection give `Status(404)` and `Os(7)`). With `$APPIMAGE` set the
  launch check ran (`update_checked` written to the settings). A local http manifest is refused
  ("not an https URL"), so the bar and Update were **not** run with a real offer. The wording
  "Only an installed copy can update itself." (shared `NOT_INSTALLED`) is acceptable on Linux but
  says nothing about AppImage; left to shared.
- **`--trace`** on Linux: memory from `/proc/self/status` (`RssAnon` as private, `VmHWM` as peak)
  and CPU time from `/proc/self/stat` (user + system, 10 ms ticks). Unit-tested on sample text.
- **Not done** (fixed later the same day, see the section above): double click on the title row did not maximise (a likely fix is to call `drag_window` only on the first pointer move with
  the button down, so that a double click never starts the compositor's move grab). "Show in file
  manager" still opens the folder only.
- **Checks**: `cargo test --release --workspace` 404 passed (403 before plus one);
  clippy `-D warnings` and `fmt --check` clean. `tests/shots.sh` before (HEAD `7eb4b9b` with the
  old pin) and after: 74 shots, 66 byte-identical; the 8 that differ are exactly those with an
  emoji (md-light, utf16le, utf16be, outline-dark, split, minimap-dark, minimap-split, tabs).
- **Budgets on Linux (information)**: window shown 72 to 78 ms on `sample.rs`; private memory
  20.6 to 20.8 MiB (peak 49 MiB); 100 MB file: loaded 115 ms, window shown 190 ms, private
  124.5 MiB; keystroke p50 0.48 ms, p95 0.57 ms (300 keystrokes).

## Menus taller than the window scroll (2026-11-13, 0.4.1, not published; in 0.5.0)

Found in a real window 665 to 760 logical px tall: the main menu rose to the window's top edge
over the caption buttons and lost "Exit". It depends on the window's height, not the scale.

- **What existed:** nothing scrolled. `mdshared_ui::menu::Menu::place` clamped a menu into the
  window with a 4 px margin and cut its height (`h.min(height - 8)`), so the rows below were
  unreachable and the menu covered the title row.
- **Shared** (branch `fix/tall-menus` of `markdown-shared-rs`, from `chore/finalize`, head
  `f49edd3`; the shared `main` was not moved): `Menu::place_in` / `draw_in` take the area a menu
  may use. A menu pushed up to fit stops at the area's top (below the title row); one taller
  than the area is as tall as fits and scrolls: `scroll_by` (wheel), `step` keeps the highlighted
  row in view (and wraps as before), a thin scrollbar (`scroll::scrollbar`) shows the position,
  hits use the placed, scrolled rows and only inside the menu, `row_rect` and `scroll` give the
  placed row. `scroll_to_end` is for shots. `place` and `draw` are the old calls (the whole
  window). A menu that fits and was opened inside the title row (the tab's own menu opens at the
  pointer) stays where it was opened. Four unit tests.
- **Editor:** `bins/mdedit/src/app.rs` gives both menus `menu_room(title, w, h)` (the window below
  `areas.title`; the pre-placement for the damage rectangle must use the same room, or the two
  calls clamp the scroll differently) and the overlay signature includes the scroll;
  `app/input.rs` `wheel` scrolls the menu under the pointer when it is taller than its room and
  closes the menus as before otherwise; `--menu main-end` and two shots (`menu-tall-main`,
  `menu-tall-main-end`, 1000x520) in `tests/shots.sh`, now 74 PNGs.
- **Ran:** `tests/shots.sh` before and after: the 72 existing PNGs byte-identical; the two new ones
  looked at. `cargo test --release --workspace` 404 passed (the editor has no new test of its
  own: the logic is the shared one), clippy `-D warnings` and fmt clean. Real window at the
  display's scale 1.0, 1000x560: the main menu below the title row with Exit reachable; the
  wheel over it scrolled it to Exit; "About mdedit" clicked opened its dialog; Down 20 times
  wrapped to the first row, Up from it to Exit with the menu scrolled to the end; Right on Macro
  opened its submenu inside the window; the encoding picker opened above the status strip inside
  the window. Measured (release, small file): exe 5,679,616 bytes (5,678,592 before), window shown
  about 77 to 85 ms (one cold run 236 ms), private memory 27.8 MiB, keystroke p50 0.48 ms.
- **Not run:** another display scale (the display was at 1.0), a context menu or submenu taller
  than the window, a scrollbar drag (the bar only shows the position), a touch pad's pixel wheel,
  Linux. The user has not seen it.

## Clean-up of the three repositories (2026-11-13, no release)

Headless work, asked by the owner to leave the code finished. No version change.

- **Shared** (`chore/finalize` at `24865c593279c977d610ba10c4df4e0ada9a371b`, on top of
  `1c3710a`; the shared `main` was not moved): `cargo clippy --workspace --all-targets -- -D
  warnings` passes. `chunks_exact(N)` became `as_chunks::<N>()`, `% 2 == 0` became
  `is_multiple_of`, `Gfx` got `Default`, a test's `clone()` into a slice became `from_ref`, and
  five drawing or button functions and one test type got an `#[allow]` with a reason. New
  `mdshared_ui::menu::row_span(items, i)` gives a row's offset below the menu's top and its
  height, with a test tying it to the rows `Menu::place` lays out. Shared tests and fmt pass.
- **Editor:** all five `rev`s moved to `24865c5`. The editor had no mirrored menu numbers in
  its code (its cascade follows the pointer, or the menu's top for the keyboard); only the test
  helper `row` in `ui.rs` used `4.0` and now uses `row_span`.
- **Debug tests.** `a_request_longer_than_one_slice_is_answered` asked 5,000 lines of an
  unoptimized highlighter, which is about a hundred times slower and worse than linear (1,000
  lines took 1 s, 2,000 took 3.4 s). A debug build now asks 1,000 lines (it passes in under
  2 s); release still asks 5,000. The whole workspace ran once in debug: 404 passed, none failed,
  the `mdedit` test binary 29 s in all (no single test timed; none could pass 30 s).
  The "four `mdedit-highlight` tests fail in debug" note in section 4 was from before and
  is gone.
- **`first_line`** in `platform.rs` is now `#[cfg(any(windows, test))]`: only the Windows
  clipboard reads UTF-16. Not compiled for Linux: the target is installed but its system
  libraries (fontconfig, X11 `pkg-config`) are not on this PC; the cfgs were read instead.
- Checks: `cargo test --release --workspace` 404 passed; clippy `-D warnings` and fmt clean;
  72 shots byte-identical to a baseline from the commit before; exe 5,678,592 bytes. The
  first comparison differed in every picture because the Windows display scale had changed
  between the two runs (1280x800 against 2240x1400 pixels); `tests/shots.sh` now pins
  `--scale 1.0`, and the baseline was rebuilt from the old code with the pinned script.

## Minimap tested in a real window, and 0.4.0 (2026-11-13)

The owner asked for the left-out runs and then a release. Windows, launched from Git Bash,
scratch `MDEDIT_SETTINGS` files, mouse and keys by `SetCursorPos`, `mouse_event`, `keybd_event`,
a screenshot or a file check for each. No defect was found, so no code was changed (the version
went to 0.4.0 and nothing else).

- **Run, with what was seen:** (2) split view of two 50,000-line files: a press and a 60 px drag
  on the second pane's column scrolled only that pane (10,492 against 5,246 in the other), and
  the keyboard went to it (tab and status line); (3) folds (Shift+F9, thousands) and word wrap
  together: a drag moved the band and the text with wrapped lines in view; (4) 50k.rs beside
  100mb.txt in a split with both columns: a drag on each column, ten wheel notches, letters
  typed in each file and a click between them took 3.1 s of scripted steps in all, no stall; (5) the Files pane open, then with the results panel too,
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
  1.5 and 2.0 shots stay the only evidence). (10) The installer was **not** run interactively: an install of mdedit (0.3.2, `AppId`
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
