The AI review of a rewrite now has a tick for each change, so you can take part of what the AI did.

(Not a release: this file collects the notes of what is on `main` and not released. When the
version is chosen it is renamed `<version>.md`, and its first line above is the one the update bar
shows.)

## New

- **A Markdown note from a text file.** On a group's row in the Notes pane, "Markdown note from a
  file…" takes any text file — a plain-text note, a letter, a file of ideas — and has the AI make a
  new Markdown note of it in that group's folder, listed with the group's other notes. The card
  says what is sent and what will be written before anything leaves the machine; the answer is
  shown as its changes and as the note; the original file is left exactly as it was, and the note
  is never written over. The run is listed in "AI runs…", where undo takes the note and its line
  out again — unless the original has changed since, in which case the note may be the only copy
  of its text and is kept.
- **Take only some of the AI's changes.** The review of a rewrite is now a list of changes, each with a tick box. Untick a change and the old text stays there; the footer says "12 changes · 8 to apply · +212 −83 lines" and **Apply 8 changes** puts in just those, in one step, so Ctrl+Z brings everything back. **F7** and **Shift+F7** (and the arrows in the bar) go to the next and the previous change, **Space** ticks or unticks the one you are on, **Tick all** and **Untick all** do what they say, and a strip shows where each change is in the document and whether it is taken; click a mark to go there. The words that changed inside a changed line are tinted, and **Copy result** copies the text with only the ticked changes. With nothing ticked there is nothing to apply, and the button says so. In **Preview**, the page on the right shows only the ticked changes ("8 of 12 changes taken") and is made again a moment after the last tick.

- **Drag a note to another heading.** In the notes pane, drag a note row onto a group or a
  section and it is moved there - its line in the library file, the same write "Move to…" makes,
  so an open library file with changes not saved refuses it. The heading it would go into is
  marked while you drag, the note is shown set aside with its name at the pointer, a plain click
  still opens it, `Esc` puts it back, and holding it at the pane's edge scrolls the list. Notes
  only; no file of yours is copied, moved or deleted.

- **Add a folder to the notes library, by its subfolders.** "Library → Add a folder." takes a
  folder you pick and lists its `.md`, `.markdown` and `.txt` files in the library, one link per
  file, under their subfolders' headings three levels deep. Nothing is copied, moved or renamed:
  the files stay where they are and the library lists them. A group for that folder is made, or a
  group the library already has by that name is added to; a file it already lists is shown as such
  and is not written again, and one line per ticked file goes into the group's library file in a
  single write. The card previews a file's first lines, a subfolder's heading ticks and unticks
  everything under it, and a folder that is too large, the library itself, or one with nothing to
  add is refused with the reason on the card.

- **An AI task window over files.** "AI task on your files…" in the AI and Library menus and the
  command palette opens one window over files you add or drop on it, and opens none of them in a
  tab. **File them** asks the AI where each belongs and shows a plan of titles and sections before
  anything is written; **Split them** opens each file's plan card in turn; **Rewrite them** sends
  one file at a time, shows the review with a tick per change, and **Apply** rewrites the file in
  place — its original is kept in the run's record, so "AI runs…" can bring it back. A picture's
  bytes are never read (its name is linked), a PDF or Word file is linked by name for File them and
  left out, with the reason on its row, for the other two. "Show what is sent" puts the exact text
  of every request in a read-only tab before anything is sent. With the AI off, File them and
  Split them still work; Rewrite them says the connection is off and offers Settings.

## Fixed

- **The review of a rewrite has a scrollbar and no longer cuts long lines.** A long review scrolls by its scrollbar, by dragging it or by clicking its track, and a line longer than the window now wraps inside its column instead of being cut off.
- **A file added to the notes library shows in the pane's tree at once.** "Add this file to the
  notes library…" wrote the note and its line, but the pane's rows were made again only when a
  group or a section was folded and opened, so a file added under a folded heading looked as
  though it had not been added. The rows are made again as the note is written, with what was
  folded above it opened, and a name being typed in the pane's box goes with them, its keyboard
  back in the text.
- **Browsing the notes library keeps the full preview.** In visual editing, opening note after
  note from the library held the preview for two or three and then dropped the window back to
  editing: a note just opened is a tab still being read, so its kind was not yet known and the
  mode took it for a file the page cannot show. It now waits for the read, closes the old note's
  open block while it waits, and follows the new note once read; a read that fails ends the mode,
  as any file the page cannot show does.

- **The question before deleting a library file says where its notes go.** It named the file and
  the group and said how many notes it listed, but not that those notes stay on disk with nothing
  listing them; a file that lists none no longer gets that second sentence. The question's card
  also grows with the lines its words wrap to: a long group or file name used to run a fourth
  line through the divider into the buttons.

- **A closed mdview is found out even when the preview was just starting.** Closing mdview by hand
  while a preview was still in its first five seconds dropped the check the window's focus made,
  so "Stop the preview" stayed up for good; the check is now kept and made once the grace ends. A
  key press or a click still asks at most once in two seconds, and typing no longer restarts the
  background checks, so steady typing does not probe about once a second; only key presses ask,
  not releases or auto-repeat.
