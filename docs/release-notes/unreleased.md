A file added to the notes library shows in the pane's tree at once.

(Not a release: this file collects the notes of what is on `main` and not released. When the
version is chosen it is renamed `<version>.md`, and its first line above is the one the update bar
shows.)

## Added

- **Add a folder to the notes library, by its subfolders.** "Library → Add a folder…" takes a
  folder the owner picks and lists its `.md`, `.markdown` and `.txt` files in the library, one
  link per file, under their subfolders' headings three levels deep. Nothing is copied, moved or
  renamed: the files stay where they are and the library lists them. A group for that folder is
  made, or a group the library already has by that name is added to; a file it already lists is
  shown as such and is not written again, and one line per ticked file goes into the group's
  library file in a single write. The card previews a file's first lines, a subfolder's heading
  ticks and unticks everything under it, and a folder that is too large, the library itself, or
  one with nothing to add is refused with the reason on the card.

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
