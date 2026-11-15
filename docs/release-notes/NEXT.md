Menus have underlined access keys, and there are more multi-caret, table, compare, results and bookmark commands.

## What is new

Editing

- Ctrl+Shift+D adds a caret at the next occurrence of the selected text, or selects the word under
  the caret first. Ctrl+click on a caret takes that caret away.
- Ctrl+Backspace and Ctrl+Delete delete a word at every caret, and Shift+Tab outdents at every
  caret and selection.
- Typing on a column selection that reaches past short lines pads them with spaces up to the
  column.
- Vue and Svelte files are syntax highlighted and listed in the Open dialog's filter.

Bookmarks

- Click the left edge of the line-number gutter to set or remove a bookmark.
- New commands cut, copy or delete all bookmarked lines.
- Bookmarks are remembered when you close a file and come back when you open it again, also after
  a restart.

Markdown tables

- Tab, Enter and Align table now work in a table that starts a list item.
- New commands "Sort table by this column", "Move row up" and "Move row down" in the Markdown
  menu.

Compare

- Differences in a comparison have a small arrow in the gutter that copies that change to the
  other side.
- New command "Copy this line to the other side" copies a single line of a changed block.

Find in files and the results panel

- The results panel takes the keyboard (F6, or right after a search): arrows move between matches,
  Enter opens one, Escape goes back to the text.
- Drag the top edge of the results panel to make it taller or shorter; the size is remembered.
- The results list follows your edits: rows of a document you change are refreshed a moment later.
- Find in files can be limited to a folder and to kinds of file such as `*.md;*.txt`.

Interface

- Menus show underlined access keys; press the letter to pick an item.
- The minimap's width can be set in Settings > View.
- On Windows 11, hovering the maximize button shows the snap-layouts flyout.
- On KDE Plasma (Wayland) the "Window backdrop" setting blurs the desktop behind the chrome.
- On Linux, pasting image files copied in the file manager now links them (needs wl-paste or
  xclip, as pasting a picture does).

Windows

- "Preview side by side" remembers where both windows were. They return there when the preview
  ends, and choosing "Preview side by side" again gives the arrangement up while the preview
  goes on. A window you have moved or resized in the meantime stays where you put it. Needs the
  mdview of the same release.

## Fixed

- "Close window" from the taskbar could leave mdedit open until the pointer moved over it.
