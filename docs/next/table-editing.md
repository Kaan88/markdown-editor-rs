# Table editing (key: table-editing)

## README

Feature list: **Markdown tables.** In a pipe table, Tab and Shift+Tab move between cells and line the
columns up (Tab in the last cell adds a row), Enter at the end of a row adds a row, and the Markdown
submenu (and the command palette) has Insert row below/above, Delete row, Insert column right/left,
Delete column and Move column left/right, next to Align table. Tables in block quotes and indented
tables keep their prefix.

Shortcut table: `Tab` / `Shift+Tab` in a table: next / previous cell. `Enter` at the end of a table row:
new row. (Keyless commands: the eight table commands, by menu or palette.)

## DESIGN

- A table is what `markdown::format_table` already finds (header, delimiter row, pipe lines; limits
  TABLE_ROWS 2000, TABLE_COLS 64). Extended: the prefix of a row is its spaces, tabs and `>` markers, and
  the table ends where the quote depth changes; a pipe in an escaped `\|` or in an inline code span does
  not separate cells (`markdown::bounds_of`).
- Tab / Shift+Tab (`table::next_cell`): only in a Markdown document, one caret, selection (if any) within
  one line, caret in a table; otherwise the ordinary indent runs. The table is lined up (display width,
  alignment row respected, min 3) and the caret goes to the start of the text of the next/previous cell.
  After a row's last cell: first cell of the next row, the delimiter row skipped. Tab in the last cell of
  the last row adds an empty row. A short row gets the missing cell. Shift+Tab in the header's first cell
  stays (only lines up). Nothing is lined up while typing.
- Enter (`table::row_after`): no selection, caret after the last non-blank character of a table row: new row
  of empty cells under it (from the header or delimiter row: the first body row), columns lined up, caret in
  its first cell. At the end of a body row whose cells are all empty the row is emptied (its `>` markers
  stay) so the table ends and the caret is on a line of its own. Enter elsewhere in a row is the normal
  Enter (list/quote continuation unchanged).
- Commands (`table::edit`, `TableOp`): act on the caret's row/column; header and delimiter rows are not
  deleted, no row above the header, the last column is not deleted, column moves and inserts keep the
  alignment row. New rows/columns are empty cells; a new column's delimiter is `---`. The table is lined up.
  Refused with a toast: "This is for Markdown files." / "The caret is not in a table."
- Every operation is one undo step, one replacement of the part of the table that changed; line endings of
  the rows that stay are kept, new rows use the document's.
- No setting: Settings has no Markdown group; Tab in a table is only taken in Markdown documents.

## Release note

Markdown tables: Tab and Shift+Tab move between cells and line the columns up, Enter at the end of a row
adds a row, and the Markdown menu has row and column commands (insert, delete, move).

## Handoff

Built: `crates/mdedit-core/src/table.rs` (new; 15 unit tests incl. a hostile-input run), the refactor of
the table finder in `markdown.rs` (`locate`, `lay_out`, `bounds_of`; quotes and code spans), `EditOp::MdTableOp`,
`table_keys` in `docs.rs` (Tab/Shift+Tab/Enter hook, runs before the typing help), eight commands
(`C::MdRowBelow` ... `MdMoveColumnRight`) in commands, palette, Markdown submenu (existing glyphs PLUS,
DELETE, BACK, FORWARD), `fixtures/table.md` and four shots in `tests/shots.sh`.

Decisions: caret goes to the cell start, no selection of cell text; Enter in an empty row ends the table
(otherwise a table at the end of a file could not be left with Enter); commands are enabled for any open
document like the other Markdown commands and refuse with a toast; no setting (no Markdown group in Settings);
a table whose header begins with a list marker (`- | a |`) is not recognised.

Not verified: Windows and release-build budgets (nothing runs at startup and nothing on the typing path:
the new code runs only for Tab, Shift+Tab, Enter and the commands, and for Tab/Enter only after
`assist.markdown` is true; Enter in Markdown additionally locates the table around the caret, which is
bounded to 2000 rows and returns early on a line with no pipe). Real-window behaviour was not tried.
Left out: tables inside list items whose first line is the table, moving rows, sorting.
