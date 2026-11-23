The AI review of a rewrite now has a tick for each change, so you can take part of what the AI did.

(Not a release: this file collects the notes of what is on `main` and not released. When the
version is chosen it is renamed `<version>.md`, and its first line above is the one the update bar
shows.)

## New

- **Take only some of the AI's changes.** The review of a rewrite is now a list of changes, each with a tick box. Untick a change and the old text stays there; the footer says "12 changes · 8 to apply · +212 −83 lines" and **Apply 8 changes** puts in just those, in one step, so Ctrl+Z brings everything back. **F7** and **Shift+F7** (and the arrows in the bar) go to the next and the previous change, **Space** ticks or unticks the one you are on, **Tick all** and **Untick all** do what they say, and a strip shows where each change is in the document and whether it is taken; click a mark to go there. The words that changed inside a changed line are tinted, and **Copy result** copies the text with only the ticked changes. With nothing ticked there is nothing to apply, and the button says so. In **Preview**, the page on the right shows only the ticked changes ("8 of 12 changes taken") and is made again a moment after the last tick.

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
