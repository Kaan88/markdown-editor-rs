The next release, not yet versioned or released: column selection, copy a change across in
compare, line commands at every caret, Markdown table editing, and crash recovery.

## What is new

Editing

- Column selection counts tabs and wide characters by their width, a drag past the end of short
  lines keeps its width, and copy, cut and paste work row for row.
- The line commands (sort, join, duplicate, move, delete, comment, case, trim, heading, align table
  and others) now act at every caret or selection, as one undo step.
- Markdown tables: Tab and Shift+Tab move between cells and line the columns up, Enter at the end
  of a row adds a row, and the Markdown menu has row and column commands (insert, delete, move).

Compare

- "Copy this change to the other side" and "Take this change from the other side" move one
  difference across as one undo step; Next and Previous difference are in the View menu.

Safety

- Unsaved text now survives a crash or a power loss: mdedit keeps a recovery copy of changed
  documents in the background and puts them back, as modified tabs, at the next start.
