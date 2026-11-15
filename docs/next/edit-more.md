# Compare, column selection, tables, Linux file paste: the small gaps (branch f/edit-more)

Run for all of it (Windows, release, `CARGO_TARGET_DIR` outside the worktree): `cargo fmt --all`,
`cargo clippy --locked --workspace --all-targets -- -D warnings` (clean), `cargo test --locked
--release --workspace` (all passed: 295 in the mdedit bin, 492 in mdedit-core, the rest
unchanged). Shots rendered by hand and looked at (light; the dark Markdown menu too): compare with
arrows, the View menu while comparing, the Markdown submenu (both fit 800 px), a column typed past
short lines. Not run in any case: a real mouse or keyboard session, Linux.
Typing path: the only new work is a `view.column` check per edit and a pad computation when a
column selection is typed on (nothing for ordinary editing); the compare arrows are found per
visible row only while two files are compared.

Commits: gaps 1 and 2 are one commit (they share `compare.rs`, the menu and the command table), 3
and 4 are one commit (4 needed no code), 5, 6 and 7 are one commit (they share `table.rs`) plus a
clippy follow-up, 8 is its own.

## 1. Compare: copy arrows in the gutter
- Built: `Hit::CopyChange { other, hunk }`, drawn in the leftmost 14 px of the gutter (the
  bookmark cell) at the first row of every difference of each pane, an arrow towards the other
  pane. The arrow of the pane with the keyboard copies from it into the other pane's document, the
  second pane's arrow copies from it into the active pane's document (`compare::pick_hunk`,
  `App::compare_copy_hunk`, the same `EditOp::CopyLines`, one undo step in the changed document).
  `Align::hunk_starting_at`, `hunk_ranges`. A pane with filler rows at a difference shows the arrow
  on the first filler row: it removes the other side's lines, as "Copy this change" does there.
  Tooltips, pointer cursor, hover tint.
- Decisions: the gutter is the pane's own (no new strip between panes), so the arrows sit at the
  left edge of each pane's gutter. Only while the diff is current; not drawn with word wrap on
  (rows are lines there); the bookmark dot of a line with an arrow is hidden by the arrow, and a
  press on the cell copies instead of toggling that bookmark. The View menu now shows the commands
  that start a comparison when nothing is compared and the ones that work on a comparison (next,
  previous, copy, take, copy line, stop) while one runs: with the new item the menu was 8 px too
  high for 800 px.
- Not verified: the click with a real mouse (the hit registration and the press are not driven;
  the function behind it is tested), wrapped text, a second pane on the left (the arrow direction
  there is by `other_left`, seen in no shot).
- Release note: Differences in a comparison have a small arrow in the gutter that copies that
  change to the other side.

## 2. Copy this line to the other side
- Built: command `CopyLineAcross` (palette and the View menu while comparing, no key):
  `Align::partner_of`, `compare::pick_line`. The caret's line replaces the line that shares its row
  in the other pane, when the block is a changed one (both sides have lines) and the row has a
  line on both sides: the diff model pairs the i-th line of a changed block's two sides, which is
  what the side-by-side view shows. Lines of the longer side's surplus have no partner and
  inserted or deleted blocks have no pair: greyed, and a toast says to copy the whole change.
  One undo step.
- Not verified: in a real window.
- Release note: New command "Copy this line to the other side" copies a single line of a changed
  block.

## 3. Column selection: virtual space
- Built: `multi::column_pads`; typing (`EditOp::Insert`) on a column selection that is still the one
  made (`view.column` matches) puts spaces before the text on every row that is only a caret at the
  end of a line narrower than the rectangle's left column, so the text lands in the column; one
  undo step. A drag or Alt+Shift+Right past the end already kept the width (`columns_past_end`);
  what typing did before was insert at the line's end. The existing test that expected `xX` on the
  short line `x` now expects `x X`.
- Decisions: only typing pads (not paste, not Enter); the caret of a short row is still drawn at
  the line's end, there is no caret drawn in the empty space.
- Not verified: with the mouse (the shot used `--select-column` and `--type`).
- Release note: Typing on a column selection that reaches past short lines pads them with spaces
  up to the column.

## 4. Column selection with word wrap: already consistent, no code
- With word wrap on, a column selection works on logical lines by logical (display) column: the
  rows and columns are the document's, whichever rows a line wraps to (`Ready::select_column`
  does not look at wrapping), and the pad of gap 3 is the same (a test types with wrap on). One
  difference remains and is as documented in DESIGN section 57: a mouse drag past the end of a
  wrapped line does not extend the width (`columns_past_end` is 0 when wrapped); the keys do.
- Release note: none.

## 5. Tables in list items
- Built: a table whose header line is `- | a | b |` (also `*`, `+`, `1.`, `1)`, inside a quote
  too) is found by `markdown::locate`: the header may start with a list marker when it does not fit
  as plain cells (`Row::marked`); the other rows are indented to line up under the text after the
  marker (the delimiter row's own indentation when it reaches that far, else spaces under the
  marker). Tab, Shift+Tab, Enter, Align table and all row and column commands keep the marker and
  the indentation. A later `- | x |` line is a new list item and ends the table. Tables that were
  recognised before read exactly as before (plain reading is tried first).
- Not verified: in a real window; a table in a list item that is itself nested in a deeper
  one (only the indentation of the delimiter row is followed).
- Release note: Tab, Enter and Align table now work in a Markdown table that starts a list item.

## 6. Sort table by this column
- Built: `TableOp::SortColumn`, command "Sort table by this column" (Markdown submenu and
  palette, no key). Body rows sort by the caret's column: ascending, or descending when the rows
  are already in ascending order (so a second use reverses). A column whose cells all parse as a
  number (trimmed, finite, starting with a digit, sign or dot) is compared as numbers, otherwise
  as text without regard to case. Equal rows keep their order; header and delimiter rows stay;
  the caret follows its row. One undo step; needs two body rows.
- Not verified: in a real window; numbers with thousands separators or units compare as text.
- Release note: New command "Sort table by this column" in the Markdown menu.

## 7. Move row up / down
- Move column left/right existed; move row did not. Built `TableOp::MoveRowUp`, `MoveRowDown`
  and commands "Move row up", "Move row down" (Markdown submenu after Delete row, palette). Body
  rows only; the first body row cannot go up, the last cannot go down. One undo step.
- Not verified: in a real window.
- Release note: New commands "Move row up" and "Move row down" for Markdown tables.

## 8. Pasting image files copied in a Linux file manager
- `arboard` 3.7 does have `file_list()` on Linux, but mdedit does not depend on it (the shared
  crate `mdshared-ui` does, and exposes only text), so wiring it would need a new dependency:
  not done. Instead the existing way images are read on Linux (`wl-paste` or `xclip`, in
  `platform::clipboard_png`) is used for the file list: `platform::clipboard_files` asks for
  `text/uri-list`, then `x-special/gnome-copied-files`, and `image_paste::parse_uri_list` reads
  the `file://` URIs (comments, `localhost`, percent escapes; other hosts skipped). `capture_png`
  now calls a shared `capture`. Windows code unchanged.
- Not verified: NOT COMPILED and NOT RUN on Linux (the Linux target cannot be checked here: a
  build script needs pkg-config for the target). `parse_uri_list` is tested on Windows. With neither
  tool installed nothing is found, as for images.
- Release note: On Linux, pasting image files copied in the file manager now links them (needs
  wl-paste or xclip, as pasting a picture does).
