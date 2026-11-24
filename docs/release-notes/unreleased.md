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

- **Sort a folder by AI.** On "Add a folder." the card has **Ask the AI**: with a connection
  chosen, it reads the files' paths, titles and first lines and answers with a scheme of up to 8
  sections with subsections under them, which you can add to, rename and delete before anything
  else is asked. **Sort N files** then puts each file under a section, 40 files to a request, three
  requests at a time (one for a local model), and the plan comes back as rows with the AI's
  confidence and a one-line reason dim under each name. **Tick only the sure ones** takes the 0.6
  and over; a file the AI could not place sits under **Unsorted**, unticked, and one it named a
  section the scheme does not have for says so. Nothing is written until **Add**, the folder's
  files are never moved or copied, and **Stop** (or Escape) cancels the requests and gives back the
  scheme you edited. **Requests at once** on the AI page sets the number, per connection.
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

- **The notes library leaves your own files beside it alone.** Every write of a library file used
  a temporary file named after it with `.tmp`, so a `personal.md.tmp` of your own was overwritten
  and then taken away. Writes now use a name of their own and leave nothing behind when they fail.
  A library file keeps its permissions, and a read-only one is not replaced: "Split into
  notes…" into a group whose library file is read-only is refused before anything is written,
  and its card says why. A file whose name is too long to take that temporary name on Windows
  is written through a short one of its own, where it could not be written at all.
- **A library file that starts with a byte order mark is written like any other.** "Rename",
  "Set the group's folder…" and "New note here…" did nothing, or dropped the folder line they
  meant to write, when the file began with one; the mark is now kept. A library file that is not
  UTF-8 text is left exactly as it is, and the message says why, rather than being rewritten with
  every accented letter changed; so is a UTF-16 file that begins with a UTF-8 mark.
- **"Split into notes…" refuses a tab that is behind its file.** When another program had saved
  the file and the tab had not read it again, the notes were cut from the old text, and putting
  the original in the Recycle Bin could take the newer file. Now it asks you to reload first.
- **An undo that could not finish can be pressed again.** When the Recycle Bin would not take a
  note, or a line could not come out of its library file, the run was marked undone and the note
  was left with nothing listing it. Such a run now reads "partly undone" in "AI runs…", and Undo
  takes the rest out. A note open in a tab with changes not saved is kept, and the panel says
  so.
- **Undo of a run puts its library file back exactly as it was.** Taking the run's links out
  could leave behind a blank line the links had brought, or change the blank lines and the last
  line of your own. While nothing else has changed the file since the run, undo now gives back
  the very text it held before, its blank lines, line ends, last newline and byte order mark as
  they were; a file changed since is undone line by line, as before.
- **A run whose links could not be written can be finished.** When the notes were written but
  their library file could not be updated, the message offered a "Try again" that nothing
  offered. "AI runs…" now has Finish for it, and the message says so. Finish under a section
  writes the heading too, and undo takes it out again.
- **"Edit a library file" never writes over `notes.md`.** When the folder read as empty, it could
  write its template over a `notes.md` that was there.
- **The question before a section is taken out of the library grows with its words**, as the
  delete question does, so a long section name no longer runs into the buttons.
- **"Move the notes…" takes a moved file's place in the recent files, its caret and its
  bookmarks with it**, as a rename does.
- **On Linux and macOS, "Find the file…" writes the link of the note it was opened for**, even
  when another note's picker was answered in between, and `home: Trip/..` is refused as the
  library folder itself whether `Trip` exists or not.
- **Visual editing no longer asks a new tab a question meant for the old one** on the frame of a
  tab switch.
- **The AI task window** files a file of Russian, Greek or Chinese text without closing the
  program (a digest cut inside a character); a rewrite run of many large files can be undone, in
  more than one row of "AI runs…" when its originals pass one record, and still after an undo
  that could not finish; File them no longer writes a group's library file that a tab holds
  with changes not saved, and keeps its byte order mark; Discard and Copy result go
  on to the next file; a review kept from a closed window writes its own file and sends nothing;
  the task cannot be switched while a request is out; a window parked by "Show what is sent"
  comes back with its task; Run shows a kept review; a file edited after it was added is sent as
  it is now; the 20-file limit of a rewrite counts the ticked files only; Rewrite them with no
  instruction says it needs one; and in Split them, the AI's answer for one file's plan no longer
  lands on the next file's.
- **Sort a folder by AI**: "Keep the folders" after a second Ask gives back the folders' own
  headings, not the first sort's.
- **Dragging a note** that is let go over the editor, at a row's height, no longer moves it, and
  a library read that finishes while a note is held no longer moves or opens another note.
- **A Markdown note from a file** whose write makes no note keeps its card and the AI's answer,
  with the reason, and leaves no "done" run behind; a name taken while the AI answered gets the
  next free name. When the cause can pass, such as a tab holding the group's library file with
  changes not saved, the button stays on for a second try.
- **Add a folder** no longer joins a group of the same name whose notes are kept in another
  folder: it names the group and that folder and adds nothing. A group with no folder of its
  own is joined, and the card says so. The library's own folder inside the one picked is left
  out; a file whose name the library cannot hold is left out and named while the rest go in; a
  title is one line; a folder past 50,000 entries or 64 deep is refused with the reason. Running
  the command again stops a folder being read, a read that ends while another card is open
  waits for it, and a refusal at Add is said in the card's footer with Add still on.
- **A row's menu in the notes pane no longer acts on another row.** Left open while the library
  was read again, its "Remove from the library" took out the line of whatever note then had that
  place in the list, with no question. The menu now closes when the rows under it have moved,
  whatever moved them.
- **Moving a note between two library files after one of them changed on disk** no longer lists
  it in both: the move looks at the note's line first, writes neither file, and says why.
