A file added to the notes library shows in the pane's tree at once.

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

## Fixed

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
