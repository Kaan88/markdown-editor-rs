# Multiple carets and bookmarks: the small gaps (branch f/multicaret-more)

Run for all of it (Windows, release, `CARGO_TARGET_DIR` outside the worktree): `cargo fmt --all`,
`cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --release
--workspace` (all passed: 276 tests in the mdedit bin, 485 in mdedit-core, the rest unchanged).
Shots rendered by hand and looked at: the text menu and its Lines submenu (both fit, light; Lines
also rendered dark and not looked at). Not run in any case: a real window with a mouse or keyboard.
Typing path: nothing was added for a single caret (every new branch is behind `view.extra` being
non-empty, or runs only on its own command or click).

## 1. Select next occurrence
- Built: `multi::select_next` (core), `Ready::select_next`, command `SelectNextOccurrence`.
  With a caret it selects the word at every caret; with a selection it adds the next exact-case
  match after the primary (wrapping), skipping matches already selected; the new one is primary.
  Over 4096 bytes of selected text it does nothing. A toast says when there is nothing to add.
- Decisions: key Ctrl+Shift+D (free in the command table; Ctrl+D stays Duplicate line). In the
  text (right-click) menu after Select all, and in the command palette. There is no Edit menu.
  Matches are case sensitive and not whole-word.
- Not verified: the key in a real window; Turkish layout; the menu entry was seen in a shot only.
- Release note: Ctrl+Shift+D adds a caret at the next occurrence of the selected text (or selects
  the word under the caret first).

## 2. Ctrl+click removes a caret
- Built: `multi::remove_at`, `Ready::remove_caret`, used by the Ctrl+click path in `input.rs`.
  A click in or touching a caret or selection removes it when there are two or more; the primary
  becomes the one before. With one caret Ctrl+click behaves as before.
- Not verified: the mouse click itself (no test drives the pointer); only the functions are tested.
- Release note: Ctrl+click on a caret takes that caret away.

## 3. Delete word at every caret
- Built: `MultiOp::WordLeft/WordRight` in `multi::edit`, wired for Ctrl+Backspace and Ctrl+Delete.
  One undo step. `multi::edit` now merges any run of overlapping removals (a word removed to the
  left can reach back over earlier removals), where before it only merged with the last one; the
  existing random test against a model still passes. On a column selection the short-row carets are
  left alone, as for Backspace.
- Not verified: in a real window.
- Release note: Ctrl+Backspace and Ctrl+Delete delete a word at every caret.

## 4. Outdent with several carets
- Built: `LineOp::Outdent` in `multiline`; the touched lines of all selections are one set, each
  loses one unit once; selection ends in removed whitespace go to the line start. One undo step.
  Also removed an unreachable match arm left in `multi.rs` by item 3.
- Not verified: in a real window; Markdown tables with several carets (Shift+Tab in a table moves
  between cells only with one caret, as before).
- Release note: Shift+Tab outdents at every caret and selection.

## 5. Click in the gutter to toggle a bookmark
- Built: `Hit::Bookmark(line)`: the left 14 px of each shown line's gutter (the dot is drawn
  there). A press toggles that line's bookmark and does not move the caret or start a drag. The
  fold cell, registered after it, wins where they overlap (a gutter without numbers). The rest of
  the gutter acts as before (a press there is a text press). Not registered for the second pane
  and not in band repaints (same rule as the fold cell). Pointer cursor over it.
- Not verified: the click in a real window (hit registration and press were not driven; only
  `toggle_bookmark_at` is tested), the cell over wrapped continuation rows, a second pane.
- Release note: Click the left edge of the line-number gutter to set or remove a bookmark.

## 6. Cut, copy, delete bookmarked lines
- Built: `lines::marked_lines_text`, `lines::delete_marked_lines` (core), `EditOp::DeleteBookmarked`,
  three commands (no keys) in the palette and at the end of the Lines submenu. Copy puts the lines
  with the document's line break; Cut is copy then delete; each delete is one undo step. As with
  Delete line, a last line without a break takes the break before it. The bookmarks are cleared
  after Cut or Delete (their lines are gone); Undo brings back the text, not the bookmarks.
  Greyed when there are no bookmarks.
- Not verified: Cut and Copy against the real clipboard (the test avoids it), the menu clicked.
- Release note: New commands cut, copy or delete all bookmarked lines.

## 7. Bookmarks kept when a file is closed and reopened: SKIPPED
- The session keeps only the tab paths. mdedit remembers no per-file state at all (not even the
  caret position; it does not use `mdshared_app::places`). Doing it needs a new session key, a
  capture on tab close and exit, a restore when a file finishes loading and a clamp for a changed
  file, and the session signature is computed on every event-loop pass. That is a design decision
  of its own, not a small gap. `Places` (limit 300, `path|position`) in the pinned shared crate
  could hold a line list without changing the shared crates; nothing was built.
