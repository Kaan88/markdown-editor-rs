# Column selection

## README
- Column (block) selection: Alt+drag, or Alt+Shift+Arrows from the caret. One selection per line over
  a rectangle of visual columns; type, delete, paste, copy, cut and undo work on all lines at once.

| Keys | Action |
|---|---|
| Alt+Shift+Arrows, Alt+Drag | Select a column |

## DESIGN
- Built on the multi-selection model: one selection per line from the first to the last line of the
  rectangle, the caret's line is the primary.
- Columns are visual (`mdedit_core::vcol`): a tab counts to its next stop, a wide (East Asian wide
  or fullwidth) character 2, a combining character 0, everything else 1. A column inside a tab or a
  wide character is its start. Caret motions keep their own count; only this selection measures so.
- A line shorter than the column gets a caret (or a shorter selection) at its end; there is no
  virtual space. Dragging past a line's end keeps the rectangle's width over short lines.
- Typing replaces/inserts on every row. Backspace and Delete on a column with text remove the
  selected text and leave rows that are only a caret alone; with only carets they act as usual.
- Copy and cut join all rows (empty rows kept) with the document's line ending. Pasting text with as
  many lines as carets puts one line on each; a copied column pastes back row for row, including a
  last empty row. Otherwise the text goes in whole at each caret, as before.
- Undo is one step. Escape leaves one caret. Dragging outside the text scrolls it.
- Word wrap: rows are logical lines (not wrapped rows), columns are logical columns; dragging past
  the end of a wrapped line does not extend the width.
- Linux: many window managers take Alt+drag to move windows; Alt+Shift+Arrows is the keyboard way.
  Kept the same on every platform.
- `--select-column <line:col>-<line:col>` (1-based visual columns) for screenshots.

## Release note
Column selection counts tabs and wide characters by their width, a drag past the end of short lines
keeps its width, and copy, cut and paste work row for row.

## Handoff
- Built: `vcol.rs` (visual columns), `multi::column` using it, `Ready::select_column/line_col`,
  drag past line end (`View::columns_past_end`), autoscroll while dragging, column-aware copy and
  Backspace/Delete/Paste in `Docs::edit`, `--select-column`, a Keyboard shortcuts row, shots
  column, column-dark, column-cjk. Alt+drag and Alt+Shift+Arrows already existed; no new shortcuts.
- Not verified: Windows and a real window with a mouse (drag, autoscroll, Alt+drag under a Linux
  window manager); release-build budgets (nothing runs unless a column selection is made; the
  keystroke path adds one comparison).
- Left out: virtual space; wrapped-row columns.
