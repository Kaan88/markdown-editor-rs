# Results panel, minimap and bookmarks: the small gaps (branch f/panels-more)

Run for all of it (Windows, release, `CARGO_TARGET_DIR` outside the worktree): `cargo fmt --all`,
`cargo clippy --locked --workspace --all-targets -- -D warnings` (clean), `cargo test --locked
--release --workspace` (all passed: 290 tests in the mdedit bin, 487 in mdedit-core, the rest
unchanged). Shots rendered by hand with the built exe and looked at: the results panel with the
keyboard in it (light), the panel at a dragged height (light), the folder and kinds boxes (light and
dark), the minimap at 160 px (light), the Settings View page (light). Not run in any case: a real
window with a mouse or keyboard, Linux, the whole `tests/shots.sh` set (its large inputs need
Python, which this PC only has as the Store alias), CPU/memory/keystroke timings.

Cost when nothing of this is on screen: with the panel closed `results_sync` returns at its first
line and nothing else was added to a keystroke or a frame. With the panel open one pass over the
open tabs (revision numbers and a hash lookup each) runs after each event, and a search on a worker
runs only after a tab with listed matches has been left alone for 300 ms. The session signature
gained three small values (the panel height, the two boxes' text and a counter of closed files'
bookmarks).

## A1. Keyboard in the results panel
- Built: the panel can have the keyboard (`Results::focused`; it is not part of the find bar). It
  gets it from the new command "Go to the results" (F6, palette), from the key that searched
  (Ctrl+Shift+F and "Find in open tabs" from the menu or palette: the answer is what the key was
  for, unless the find box was asked for its text instead), and from a click in the panel. Up, Down,
  Home, End, PageUp and PageDown move the highlighted row between match rows (no wrap, pages are
  as many rows as the list shows); Enter opens the row at its match and gives the keyboard back to
  the text; Escape gives it back to the text and leaves the panel open. The panel's top line is
  drawn in the accent colour while it has the keyboard. A click on a row opens it and keeps the keyboard
  in the panel, so that Up and Down go on from that row. Shortcuts (F4, Ctrl+F and the rest) still
  work; Left, Right, Backspace, Delete and Tab are kept from the text while the panel has the keyboard.
  `--results-focus` renders it for a shot.
- Decisions: Escape follows the find box (it returns to the text; it does not close the panel;
  "Close the results" or the close button does). The highlighted row is the existing `selected`
  row (the last one gone to), so F4 and Shift+F4 go on from where the keyboard is. Enter takes the
  keyboard to the text so the match can be edited; the arrows only highlight.
- Not verified: keys and click in a real window; Turkish layout (F6 is a function key).
- Release note: The results panel takes the keyboard (F6, or right after a search): arrows move
  between matches, Enter opens one, Escape goes back to the text.

## A2. Resizable results panel
- Built: the panel's top edge (a 5 px strip across the seam, pointer shown as a row resize) is
  dragged to change the height; 110 px at least, the text keeps its 140 px. Kept in the session as
  `results_h` and applied at start. `Areas::with_results` takes the height; `dragged_height` is the
  limit arithmetic.
- Not verified: the drag in a real window (a shot with `results_h` in a scratch settings file was
  looked at; the drag path is only compiled and its limit function tested); a window resized
  while the panel is dragged tall (the existing "text keeps its minimum" rule applies).
- Release note: Drag the top edge of the results panel to make it taller or shorter; the size is remembered.

## A3. Results follow edits
- Built: when a document that has matches listed is edited, its rows are marked at once (faint,
  "updating..." in the title, and a click or Enter on one says the document changed and goes
  nowhere), and 300 ms after its last change it is searched again on a worker thread from a cheap
  snapshot of its text; the answer replaces its rows (a file left with no match loses its rows).
  It works for "Find in open tabs" and for the rows of a folder search whose file is open in a tab
  (those are first read from disk, so a tab with unsaved changes is refreshed from its text as soon
  as the first pass sees it). The selected row keeps its place when it is after the changed rows.
  A new search or closing the panel drops any answer still coming.
- Decisions: stale rows are not offered as places to go to rather than adjusted; no file that lost
  all its matches gets a row back when a later edit adds one (search again). Replace in files still
  plans from disk, as before.
- Not verified: in a real window; many tabs with thousands of matches.
- Release note: The results list now follows your edits: rows of a document you change are refreshed a moment later.

## A4. Folder and kinds of file for Find in files
- Built: after a folder search two boxes show under the panel's title row: the folder (empty: the
  open folder, as before; a relative path counts from it) and the kinds of file (patterns such as
  `*.md;*.txt`, `*` and `?`, case blind, a leading `.` as in `.md` means `*.md`, `!*.min.js` leaves
  names out). Enter in either box searches again with the find box's text and Tab goes between them.
  Both keep what was typed, in the session (`find_folder`, `find_kinds`). The title says the kinds.
  A folder that is not there gives a toast and keeps the old rows. Replace in files plans from the
  same files (with kinds chosen it lists them with the same walk and the same filter). Core:
  `search::NameFilter` and `search::in_tree_named`.
- Decisions: the boxes live in a 34 px row of the panel that shows only for a folder search (so
  the first search is made with the defaults, then refined), which takes that height from the list;
  no folder picker dialog, the path is typed.
- Not verified: typing in the boxes in a real window; a folder on a network drive; the shots of
  the replace panel (`replace-files-*`) changed by the extra row and were not compared with the
  earlier set.
- Release note: Find in files can be limited to a folder and to kinds of file such as `*.md;*.txt`.

## B. Minimap
- Already built (checked in the code and the handoff, not rebuilt): marks for the selections, the
  find matches (right edge), the bookmarks (left edge) and the caret's row (`minimap.rs`, drawn from
  the data the editor keeps); a column for each pane of a split (`Region::MinimapOther`).
- Built: a width setting, Settings > View > "Minimap width" (48 to 240 px, default 96,
  `Prefs::minimap_width`, key `minimap_width` in the preferences section, read through
  `live::minimap_width`). `Areas::with_minimap_width` is the arithmetic; both panes' columns use it
  and the "text keeps 320 px or the column hides" rule is unchanged.
- Not verified: the setting changed in a real window (a shot with it set in a scratch file and
  the Settings page were looked at); the column's drawing at an odd width at scale 1.5.
- Release note: The minimap's width can be set in Settings > View.

## C. Bookmarks kept across closing a file and across runs
- Checked first: mdedit has no per-file place store (the viewer's `places` are not used here).
- Built: `marks.rs`: the lines (0-based) of the 50 latest files, at most 100 lines each, in the
  session as `mark=1,4;path` lines. A file's bookmarks are put in the list when its tab closes
  (a counter in the session signature then makes the session save), the open files' bookmarks are
  added when the session is written (at exit and with any other save), and they are given to a file
  the first time it is read in a window (not to a reload, which keeps its own), keeping only lines
  that are still lines of it. A file closed with none forgets what it had.
- Decisions: bookmarks of an open file are not written until it closes or the window exits, so a
  crash loses the ones set since; lines are not moved if the file changed on disk while it was
  closed. They apply by path (as tabs do), not by content.
- Not verified: a real close and restart; Windows path case differences beyond `same_path`.
- Release note: Bookmarks are remembered when you close a file and come back when you open it again, also after a restart.
