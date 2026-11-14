# Multi-caret support in the line commands

## README
- Line commands work with several carets or selections: duplicate, move, delete, comment, case,
  trim, tabs to spaces, line endings, sort, reverse, remove duplicates, join, heading level and
  align table each act at every caret, and undo in one step.

## DESIGN
Rules per command (full text in the header of `crates/mdedit-core/src/multiline.rs`). The lines a
selection touches are as for one selection. Selections that share a line (for the commands marked
"adjacent", also lines that follow one another) make one block; a command acts once on each block.

- Duplicate: every block is copied below itself; selections move onto the copy.
- Move up/down (adjacent): every block moves one line; a block at the document's edge stays, the
  others still move.
- Delete line (adjacent): all touched lines go; one caret per removed block, in the same column.
- Toggle comment: the touched lines are one set: all commented means uncomment, otherwise comment
  every non-blank line at the smallest indentation. Only blank lines: each caret's line.
- Case: each non-empty selection; carets change nothing.
- Trim, tabs to spaces, line endings: the whole document as with one caret; carets follow text.
- Sort, reverse, remove duplicates (adjacent): each block alone (duplicates are looked for inside a
  block, lines never move between blocks); only carets: the whole document, as with one caret.
- Join: a block joins into its first line; a selection in one line joins it with the next.
- Heading level: every block steps by its own first non-blank line. Align table: each table once.
- The size limits (`BLOCK_TOO_LARGE`) apply to the blocks together.
- Indent and Outdent are unchanged (Outdent with several carets still leaves one caret).

## Release note
The line commands (sort, join, duplicate, move, delete, comment, case, trim, heading, align table
and others) now act at every caret or selection, as one undo step.

## Handoff
- Built: `mdedit_core::multiline` (+ `multiline/tests.rs`, about 60 tests with two and three carets,
  overlapping and adjacent selections), `*_range`/`*_pts` variants in `lines.rs`/`markdown.rs`,
  `History::fuse_since` (one undo step), shell wiring in `Docs::edit` (`line_edit`).
- An old test that asserted "a command other than typing leaves one caret" was changed to the new rule.
- Not verified: Windows, a real window, release-build budgets (nothing runs with one caret: one
  `is_empty` check).
- Left out: Outdent with several carets; MdWrap/MdLink already worked on every caret.
