mdedit, not yet released: the AI task window made one command with an Add menu, the AI task on a note or a section of the Notes pane, pictures opened in the system's viewer, and fixes to the bar and the task window.

## Fixed

- **AI edit stays with its group on the bar.** With a document that has no preview (a `.rs` or a
  `.txt` file), the AI edit button stood apart at the end of an empty gap where the Preview button
  would be. It now sits right after the Notes button, where Preview sits when it is shown.
- **The task window's destination row no longer overlaps.** The folder the files go to was drawn
  over the "under a section" field when the window was narrow or the path long. It is now cut to
  the room beside the field, keeping its end, and left out when there is none.

## Changed

- **One "AI task…" command.** "AI task on your files…" and "AI task on this file…" did the same
  thing. Now there is "AI task…", whose Add… button lists your open files, the one in front
  first, and Files… to pick others. "AI task on this file…" is in a tab's right-click menu, for
  that tab's file.
- **A picture in the library opens in your picture viewer**, and a PDF or an office document in
  its own program, instead of as unreadable text in a tab. A program in the library is never run:
  its folder is shown.
- **"File them" leaves pictures out** and says why: a picture is not a note of its own.
- **"File them" can be undone** from AI runs, like a rewrite: the links it added go, and the group
  it made, if it is unchanged.
