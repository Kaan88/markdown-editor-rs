# Plan: mdedit, a light text editor

## Status: feature complete, in maintenance (2026-11-17)

The owner agreed on 2026-11-17 that mdedit is feature complete. Every roadmap below is built; the
sections that follow are history, not open work. From here on:

- Fix bugs found in daily use, and keep dependencies current: Dependabot opens pull requests
  every Monday (`.github/dependabot.yml`); `ci` runs on each; merge by hand once it passes and the budgets below are measured.
  The sister crates (`mdshared-*`, `mdview-*`) and the winit and softbuffer forks are moved by hand.
- A new feature goes in only when the owner asks for it, under the same rules as before.
- Release only the app a change affects (mdview 1.0.1 carried no change of its own and was
  withdrawn).
- Known small gaps, not planned unless they get in the way: no access keys in the line-ending and encoding menus; an empty function list for Vue and Svelte; a column selection pads short lines on typing but not on paste or Enter; no compare arrows with word wrap on; the Markdown menu has no room left in an 800 px window.
- Set aside by the owner (can be reopened): screen-reader accessibility; plugins; printing; the FTP and project panels; syntax-aware folding.
- Never checked by hand: the KDE blur on Linux; pasting image files copied in a Linux file manager; the Linux AppImage and packages; an installed copy updating itself and restarting.

Written 2026-11-12. The user said to start the same day. Phases 0 to 3 are built (see "State"
below); the user has not yet looked at the interface, so nothing about it is approved.

## State (2026-11-13)

**The rule for every feature (the user, 2026-11-12):** "as long as the app remains just as fast to
open and work, implement useful features from Notepad++." A feature goes in only if first frame,
keystroke time, scroll frame, memory and idle CPU stay within the budgets below, measured before
and after.

**The exe size is not a gate (the owner, 2026-11-14).** It is reported with each release, and only
a sudden large jump needs explaining. The gates are, in order: launch time first, then keystroke,
scroll, memory and idle CPU. The size figures and the "Exe" rows of the tables below are kept as
information and are not budgets. Still to do, on real Windows before the build profile is
changed: compare the current build with one that has the editor crate compiled for speed and
`panic = "unwind"`. The profile has not been changed.

**Phase 0, done.** Results in `spikes/*/RESULTS.md`, measured on a loaded machine:

- Buffer: `ropey`. A 100 MB file loads in about 75 ms; edits and lookups take microseconds.
- Long lines: lines up to 4 KB are shaped whole; longer ones in visible chunks of about 1 KB.
- Highlighting: syntect with a stored state every 16 lines and time-boxed slices on a worker.
  Markdown through syntect is slow (about 2,000 lines a second).
- Input methods: the test window builds and runs; its typing script in `spikes/ime/RESULTS.md`
  has NOT been done by hand.

**Phase 1, built, not yet seen by the user.** Measured on the PC, release build:

| | Budget | Measured |
|---|---|---|
| First frame, small file | under 100 ms | 84 to 104 ms across runs, median about 90 |
| First frame, 100 MB file | under 500 ms | 138 to 199 ms warm, 271 ms cold |
| Private memory, small file | under 30 MiB | 27.5 MiB |
| Private memory, 100 MB file | file size plus 25% (125 MiB) | 133 MiB: over by 8 MiB |
| Scroll frame, p95 | under 4 ms | 0.8 to 1.5 ms |
| Idle CPU | zero | none measured, focused window with caret blink |
| Exe (information, not a gate) | was under 6 MB | 5.7 MB |

Notes: the release profile uses `panic = "abort"` to keep the exe small (0.9 MB of unwind tables
gone; the size is no longer a gate, see above), so a panic on any thread ends the process; the viewer uses `unwind`. The 100 MB memory
figure was left as it is. Idle CPU with the window unfocused was not measured. Not done by hand:
everything interactive (selection, copy, find, tabs, Files pane, theme switch, reload on change,
second-instance hand-over).

**Phases 2 and 3, built 2026-11-13, not yet seen by the user.** Typing, deletion, selection
editing, clipboard, undo and redo, auto-indent, Tab and Shift+Tab; Save, Save as, the unsaved mark
and the questions before closing, and the handling of a file that changed on disk
(`docs/DESIGN.md`, section 16). Measured on the PC, release build:

| | Budget | Measured |
|---|---|---|
| Keystroke to pixels, small file | under 8 ms, typical 2 to 4 | p50 0.8 ms, p95 0.9 ms, max 1.7 ms (`--type-steps 300`, three runs) |
| Keystroke to pixels, 100 MB file | the same | p50 0.9 ms, p95 0.95 ms, max 1.4 ms |
| Keystroke to pixels, Markdown file | the same | p50 1.8 ms, p95 2.2 ms (one run) |
| First frame, small file | under 100 ms | 80 to 90 ms |
| First frame, 100 MB file | under 500 ms | 135 to 212 ms |
| Private memory, small file | under 30 MiB | 27.8 MiB |
| Private memory, 100 MB file | 125 MiB | 133.4 MiB, as before |
| Scroll frame, p95 | under 4 ms | 0.9 ms small, 2.0 ms on the 100 MB file |
| Exe (information, not a gate) | was under 6 MB | 5.8 MB |
| Save, 100 MB | none set | 88 ms as UTF-8, 261 ms as UTF-16 |

The keystroke figure is the edit plus the frame's own work up to handing the pixels to the
system; it leaves out the system's present and the later frame that brings the new colours. The
scripted edit run (20,000 operations against a plain-string reference, then undo and redo of all
of them) and the round trips (every fixture that loads without replaced bytes, plus 135 generated
files over UTF-8, BOM, UTF-16 LE and BE, Windows-1254, Windows-1252, Shift_JIS, GBK and every
line-ending mix) pass byte for byte. Not done by hand: anything interactive, including every
editing key, the questions before closing, Save as, and a file changing on disk under unsaved
text. Not measured: idle CPU after these changes.

## Feature roadmap (agreed 2026-11-12)

After editing (phase 2) and safe saving (phase 3), in this order:

1. Editing basics: duplicate, move and delete line, indent and outdent, toggle comment, change
   case, trim trailing spaces, tabs to spaces.
2. Find and replace: match case, whole word, regular expressions, replace all, find in all open
   tabs, mark all matches, count. (Done 2026-11-13, except find in all open tabs, which waits
   for item 8's results panel.)
3. Unsaved-session restore: tabs with unsaved text survive a close. (Done 2026-11-13.)
4. View aids: show whitespace and line endings, word wrap, current-line highlight, matching
   bracket, other occurrences of the selected word, zoom, go to line. (Done 2026-11-13.)
5. Encoding and line-ending conversion from the status bar. (Done 2026-11-13.)
6. Column selection and multiple carets. (Done 2026-11-13.)
7. Bookmarks on lines. (Done 2026-11-13.)
8. Find in files, on a worker. (Done 2026-11-13.)
9. Code folding and a function list, only if typing stays as fast. (Done 2026-11-13; folding
   is by indentation.)
10. Macro record and playback; split view. (Done 2026-11-13; the split showed two files, not
    one file twice, and the user said to leave it so. The same file in both panes was built on
    2026-11-14, in 0.5.0.)

Left out: plugins, the FTP and project panels, printing. Syntax-aware folding was considered and
is not planned: folding stays by indentation (decision 2026-11-15).

## Next release (agreed with the owner 2026-11-14)

Agreed after mdedit 0.5.0 and mdview 0.3.0 (Windows and Linux). Each item has to keep the budgets
above, measured before and after.

Status (2026-11-15): every item below was built and is in mdedit 0.6.0 (the status words in the
list were written while the work was in progress and are corrected here).

- Copy a change across in compare. Done, in 0.6.0.
- Finer sync with the viewer inside long paragraphs and code blocks. Done, in 0.6.0.
- Live preview for a document that has never been saved. Done, in 0.6.0.
- Multi-caret support in the line commands (sort, join, headings, table alignment). Done, in 0.6.0.
- Owner request 2026-11-14: a "Restart" button once an update is installed, both apps, only if it
  is simple to do. Done, in 0.6.0. A real update followed by Restart has not been run.
- Also done, not on this list before: column selection (DESIGN section 57), Markdown table
  editing (section 60), and autosave and crash recovery (section 61).
- Budgets for the five features: measured on 2026-11-15 on the merged head after 0.6.0 (see
  `docs/HANDOFF.md`, "After 0.6.0: gaps closed"): first frame with `sample.rs` 85 ms (budget under
  100 ms), private memory 27.8 MiB (budget under 30 MiB); both met.
- Carried over, now done: the speed-versus-size build comparison on Windows (exe 8.76 MB against 6.48 MB, no faster in start or keystroke: the profile stays).
  Still owed: a hand run of 0.5.0 and 0.6.0 on a Windows desktop.
- Rounded window corners (owner request 2026-11-14, agreed 2026-11-14): **built and in 0.6.0.**
  Default radius 8 logical px, scaled with the display, square when maximised,
  tiled or fullscreen, with a setting (Square, Small = 4, Round = 8). It sits between KDE Breeze
  (about 5 px, from bug reports) and GNOME libadwaita (about 12 px, from a 2021 commit and forum
  estimates) and matches the 8 px radius of the apps' own dialogs. There is no standard way to ask
  a Linux desktop for its radius. On Windows 11 the system rounds (the DWM corner preference),
  nothing is drawn. On KDE it needs the client-margin shadow route, because KWin's shadow route
  leaves the window opaque and square: with rounding wanted, `Auto` takes the margin there too
  (docs/DESIGN.md section 64).

## After 0.6.0 and 0.4.0 (written 2026-11-15; item 1 asked for by the owner, the rest proposed)

mdedit 0.6.0 and mdview 0.4.0 are released and every item of "Next release" above is in them.
The full list of what was left, for both apps, is in the viewer's `docs/PLAN.md`, section "After
0.4.0 and 0.6.0". The editor's part of it, with the state on 2026-11-15 (branch `merge/next`, not
released, the owner has not looked at any of it; details in `docs/HANDOFF.md`, "After 0.6.0: gaps
closed"):

1. **Windows return to where they were when a side-by-side preview ends** (owner request
   2026-11-15, both apps). mdedit remembers its window's place when `tile_left` moves it and
   goes back there when the preview ends: the preview is turned off, the previewed document is
   closed, or the viewer or its live tab is gone. A window the user has moved or resized since
   stays where it is. Exiting while tiled saves the remembered place, not the left half. The
   code goes beside `mdshared_ui::place`, for both apps. Details and open questions: the
   viewer's plan. **Done**, merged into `main`, not released.
2. **Frosted window on Linux** (KDE blur): the section "Later" below. **Built on `merge/next`
   (non-Windows code only), compiled by CI on Linux, never run on Linux.**
3. **Owed checks:** the owner's look at 0.6.0 and at `merge/next`, a real update followed by
   Restart, the AppImage by hand on Linux. Done on 2026-11-15: startup time and memory against the
   budgets (met; an empty start took 435 ms instead of about 50 ms, fixed) and the
   speed-versus-size build comparison (numbers in the handoff).
4. **The small gaps of the roadmap items, closed on `merge/next` (2026-11-15, not released):**
   - Multiple carets: select next occurrence (Ctrl+Shift+D), Ctrl+click removes a caret, delete
     word and outdent at every caret. Done.
   - Bookmarks: click in the gutter, cut, copy and delete bookmarked lines, kept across closing a
     file and across runs. Done.
   - Results panel: keyboard (F6), resizable, follows edits, folder and kinds of file. Done.
   - Minimap: width setting. Done (its marks were built before).
   - Compare: copy arrows in the gutter, copy a single line across. Done.
   - Column selection: virtual space when typing (word wrap needed no change). Done.
   - Markdown tables: tables in list items, sort by column, move row up and down. Done.
   - Paste of image files copied in a Linux file manager. Built, only compiled by CI, never run.
   - Vue and Svelte highlighting, menu access keys, the snap-layouts flyout (Windows 11): taken
     from the shared crates. Done; the flyout was checked by window messages, not a real pointer.
   - **Not planned, by decision:** syntax-aware folding (folding stays by indentation).
     Crash reporting, accessibility, input methods (IME), plugins, the FTP and project panels and
     printing were set aside by the owner earlier.

"Find in open tabs" (roadmap item 2) is built: it came with the results panel.


## Later (owner request 2026-11-15): a frosted window (Mica, Acrylic, blur on Wayland)

For both apps (the code goes in the shared crates). Status 2026-11-15: the Windows 11 part is in
0.6.0; the KDE Wayland part is built on `merge/next` and was only compiled by CI, never run. The
text below was written before either was built: it is what the platforms offer.

- **Windows 11:** the system draws it. One DWM call (`DWMWA_SYSTEMBACKDROP_TYPE`, Windows 11 22H2
  and later) chooses Mica, Mica Alt or Acrylic. It shows only where the window's own pixels are
  transparent, and softbuffer presents through GDI, which does not reliably carry an alpha channel.
  That is the open question: either GDI carries it, or the Windows present path needs what the
  softbuffer fork already does for Wayland (ARGB buffers). Windows 10 has no supported way: flat
  colour there.
- **Linux, Wayland:** KDE has a blur-behind protocol, and the window already has an ARGB surface
  where the client-drawn margin is used (the same route question as the rounded corners). GNOME
  offers applications no blur: flat colour there. Other compositors: only where a blur protocol
  exists. X11: under KWin only.
- **Where:** behind the chrome only (title row, tab strip, Files pane, status strip). The text area
  stays opaque: text is blended against a known colour and reads worse over a shifting backdrop.
- **Rules:** a setting that includes "off"; every surface keeps its flat colour as the fallback
  (Mica is flat when the window is inactive, on battery saver, or with transparency effects off);
  the budgets hold (the system does the blur, the app pays nothing per frame).
- **First step:** a spike on Windows: one window, the DWM call and a transparent title row. It
  answers the GDI alpha question and says whether this is a small feature or a change to how the
  window is presented. Then the same on KDE Wayland.

## What it is

From the viewer's plan (section E, 2026-11-11): the user wants, for personal use, "an editor as
fast and pleasant as the viewer: a Notepad alternative with syntax highlighting". It is a second
project with the same goals (light, fast, pleasant), not a mode of mdview: a separate binary that
opens any text file and shares the shell, text and highlighting code with the viewer.

- Binary `mdedit`, Windows first, pure Rust, no webview, no toolkit. winit for the window and
  input, softbuffer for the pixels, every control drawn by hand, as in `mdview`.
- Plain text editing of any file, with syntax highlighting where a syntax exists.
- It looks and behaves like the viewer: the same title row with tabs, command bar, menus,
  dialogs, themes and shortcuts.

## What it is not

- Not an IDE: no language servers, no build or debug, no plugins, no terminal. (Word completion
  from the open files was added in 0.6.0.)
- Not a WYSIWYG Markdown editor. Markdown is edited as text; the rendered page is the viewer's job.
- No multiple cursors, split views, minimap or code folding in the first version (all built
  since; see the roadmap above).
- No network use, except looking for and downloading its own updates (added 2026-11-13 with
  the shared updater; a small manifest from GitHub at most once a day, off with one setting).

## Depends on

[markdown-shared-rs](https://github.com/Kaan88/markdown-shared-rs), pinned to a commit (`rev` in the root `Cargo.toml`): `Painter`,
palettes, menus, the single-line text field (for the find box), the folder tree, the window frame,
`platform`, `TextEngine`, `Bitmap` and the syntect set-up. That repository's plan has the
extraction steps; this project starts after its step 4 (or step 3, with its own window loop, if
step 4 is dropped).

## Budgets (proposals, to be measured)

The viewer's figures on the same PC, 2026-11-12, are the reference: 24 MiB private memory with a
small document, 63-98 ms to first frame on a real launch, a 7.3 MB exe.

| | Target | Why |
|---|---|---|
| First frame, small file | under 100 ms | The viewer's figure; the editor has no parser to run |
| Private memory, small file | under 30 MiB | The viewer's, plus the buffer and undo |
| Keystroke to pixels | under 8 ms, typical 2-4 ms | One line re-shaped and one band repainted |
| Scroll frame | under 4 ms at 1280x800 | The viewer's budget |
| Opening a 100 MB file | first frame under 500 ms, memory about file size plus 25% | Loading must not shape or highlight what is not on screen |
| Idle CPU | zero | Timers only for caret blink and pending work |
| Exe (information, not a gate since 2026-11-14) | was under 6 MB | syntect's syntax set is most of it |

## Design

**Buffer.** A rope (the `ropey` crate is the candidate) or a piece table of our own. Phase 0
decides by measurement: load time and memory on a 100 MB file, insert at random positions, line
index lookups. The buffer holds UTF-8 whatever the file's encoding was.

**Lines and layout.** Only the lines on screen are shaped, with parley through the shared
`TextEngine`. Heights without wrapping are one line each, so scrolling a huge file needs no
estimates; with word wrap on, off-screen heights are estimated and corrected as in the viewer's
patchable layout. An edit re-shapes the lines it touched.

**Highlighting.** syntect keeps a parse state per line. An edit re-highlights from the edited line
until the state matches what was stored, in slices between events, the visible lines first. Files
over a size limit, and lines over a length limit, stay plain.

**Undo and redo.** A list of edits (position, removed text, inserted text) grouped into steps by
typing bursts and by command. Unlimited within a memory cap. Not kept across restarts in the
first version.

**Caret, selection, keyboard.** The usual Windows text editing set: word and line movement,
Shift selection, double and triple click, drag selection with auto-scroll, Tab and Shift+Tab
indenting a selection, auto-indent on Enter. Grapheme clusters, not bytes or code points, are the
unit of movement and deletion.

**Input methods.** winit's IME events with the candidate window placed at the caret. The viewer
set non-Latin input aside; an editor cannot. Turkish dead keys and AltGr are checked by hand
early, CJK composition before release.

**Encodings and line endings.** Detect UTF-8 (with and without BOM), UTF-16 LE and BE by BOM, and
fall back to the system code page (Windows-1254 on this PC) when the bytes are not valid UTF-8;
`encoding_rs` is the candidate. The encoding and the line ending style (CRLF, LF, mixed) are shown
in the window and kept on save unless the user changes them. A file with invalid bytes is opened
read-only with a notice, never silently repaired.

**Saving.** Write to a temporary file beside the target, flush, then replace the original so its
attributes and permissions are kept (`ReplaceFileW` on Windows). A file changed on disk since it
was opened is never overwritten without asking. Unsaved changes are marked on the tab, and
closing asks.

**Crash safety.** **Open:** whether to keep a recovery copy of unsaved buffers. Notepad on
Windows 11 restores unsaved tabs; doing the same means writing buffers to the session folder on a
timer.

**Find and replace.** The viewer's find box and counter, plus replace, replace all, match case and
whole word. Regular expressions are a later step.

**Files, tabs, session.** As in the viewer: tabs, the Files pane, recent files, single instance
with hand-over, session restore, OS file watching. A file changed on disk reloads by itself only
while the buffer has no unsaved changes; otherwise it asks.

**Working with the viewer.** "Open in mdedit" in the viewer's tab and page menus, and "Preview in
mdview" here, each a plain process launch with the path. The viewer already reloads a file when it
changes and keeps the reader's place, so saving here updates the preview there. **Open:** whether
the installer is one package for both or one each.

## Phases

Each phase ends with a build the user runs. The user approves the interface by looking at it.

| Phase | Work | Done when |
|---|---|---|
| 0 | Spikes, throwaway: rope against piece table on a 100 MB file; viewport-only shaping of long lines; incremental syntect on a 50,000-line file; an IME round trip in winit | Numbers recorded; buffer chosen |
| 1 | Read-only: open a file in the shared shell, tabs, scrolling, selection, copy, find, highlighting, encodings detected and shown | First-frame, memory and scroll budgets met on the small and the 100 MB file |
| 2 | Editing core: caret, typing, deletion, selection editing, clipboard, undo and redo, auto-indent | The keystroke budget met; a scripted edit run compared with a reference buffer |
| 3 | Saving: safe save, encoding and line endings kept, changed-on-disk handling, unsaved marks and prompts | Round-trip tests over UTF-8, BOM, UTF-16, Windows-1254, CRLF, LF and mixed files change no byte they should not |
| 4 | Find and replace; word wrap; go to line; zoom; the settings that exist in the viewer | Used by the user for real work for a week |
| 5 | IME and non-Latin text checked by hand; large-file limits set from measurements | The user's sign-off |
| 6 | Installer; the hand-offs with the viewer; file associations offered, never taken | Installed and uninstalled cleanly on the PC |

## Risks

- **The text view is the whole project.** The viewer's `textfield.rs` is a single-line field; a
  multi-line editor with wrapping, long lines and IME is new work with no toolkit widget to lean
  on. The viewer's plan says "budget weeks, not days", and that still holds.
- **Very long lines** (minified files) defeat line-based shaping. They need a cap or shaping by
  chunks; phase 0 measures it.
- **syntect's speed** on large files, and its lack of some languages (Kotlin, Swift, PowerShell
  and Dockerfile are missing in the viewer too). Adding syntax definitions grows the exe.
- **Fidelity on save** is where an editor loses trust. Phase 3's byte-exact round-trip tests come
  before any convenience.
- **Accessibility.** A hand-drawn interface has no screen-reader support. The viewer set this
  aside; it matters more for an editor and is not planned here (confirmed as set aside by the
  owner, 2026-11-15, with crash reporting, input methods, plugins, FTP and project panels and
  printing).

## Rules carried over from the viewer

- Report results plainly, including what was not run. Nothing counts as working on Windows until
  it has run on Windows.
- Commit and push as phases complete or when asked. No CI: run the tests yourself.
- Spikes are throwaway code, kept for reference.
- Linux after Windows. (Linux was done for 0.5.0: an AppImage, run on KDE Plasma and sway.)

## Open decisions

1. **The name.** `mdedit` and `markdown-editor-rs` follow the viewer's naming, as asked, though
   the editor opens any text file. Renaming is cheap before phase 1.
2. **Recovery of unsaved buffers** (above).
3. **One installer or two** (above).
4. **Word wrap default** (built off by default on 2026-11-13, the same for every file type;
   not confirmed), and whether Markdown files get conveniences (list continuation, bold
   and italic shortcuts) or stay plain text like everything else.
5. **When to start.** The viewer's plan says not before the reader has shipped (its roadmap steps
   A3 to A6) and its own next list is done or paused.
