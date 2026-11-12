# Plan: mdedit, a light text editor

Written 2026-11-12. Nothing here is built, and nothing should be until the user says so. The
budgets are proposals until measured, and the decisions marked **Open** are the user's.

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

- Not an IDE: no language servers, no completion, no build or debug, no plugins, no terminal.
- Not a WYSIWYG Markdown editor. Markdown is edited as text; the rendered page is the viewer's job.
- No multiple cursors, split views, minimap or code folding in the first version.
- No network use at all.

## Depends on

[markdown-shared-rs](https://github.com/Kaan88/markdown-shared-rs), pinned to a commit: `Painter`,
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
| Exe | under 6 MB | syntect's syntax set is most of it |

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
  aside; it matters more for an editor and is not planned here.

## Rules carried over from the viewer

- Report results plainly, including what was not run. Nothing counts as working on Windows until
  it has run on Windows.
- Commit and push as phases complete or when asked. No CI: run the tests yourself.
- Spikes are throwaway code, kept for reference.
- Linux after Windows.

## Open decisions

1. **The name.** `mdedit` and `markdown-editor-rs` follow the viewer's naming, as asked, though
   the editor opens any text file. Renaming is cheap before phase 1.
2. **Recovery of unsaved buffers** (above).
3. **One installer or two** (above).
4. **Word wrap default**, and whether Markdown files get conveniences (list continuation, bold
   and italic shortcuts) or stay plain text like everything else.
5. **When to start.** The viewer's plan says not before the reader has shipped (its roadmap steps
   A3 to A6) and its own next list is done or paused.
