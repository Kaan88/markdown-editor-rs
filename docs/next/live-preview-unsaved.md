# live-preview-unsaved: preview of a document that has no file (mdedit side)

Shared commit: `0ba3b6129b7ccd60c62275652e46c297ec5b5fc4` (branch `f/live-preview-unsaved` of
markdown-shared-rs). The viewer's side is on the branch of the same name in markdown-viewer-rs.

## README

Change the "With mdview" item of the feature list to say:

- **With mdview:** Preview in mdview (Ctrl+Shift+V) opens the file in the viewer at the caret's
  line and keeps it there as you move and type; Preview side by side also puts the two windows
  next to each other. A document that was never saved is previewed too: mdview shows its text
  under the tab's name, and when you save it the preview becomes the file's.

No new shortcut.

## DESIGN

**A document that has no file.** "Preview in mdview" and "Preview side by side" work on an
untitled document. Nothing is written to disk for it: the text travels in the message.

- The document goes by a name, `mdedit-<process id>-<document number>` (`link::doc_name`), which
  is the same for as long as the tab lives and belongs to no other tab or editor. The message
  carries `?doc=<name>`, `?title=<the tab's name>` and, the first time, `?open` (see "A document
  that has no file" in the shared `docs/SISTER.md`). No path is sent.
- The viewer is started bare when none runs (it could not be given a path), and the first message
  asks it to show the document; with "Preview side by side" that message also carries the
  viewer's place. The first message is tried for the same 5 s as an opening of a file.
- The text is always sent, whatever "Live preview while typing" says: there is no file the viewer
  could read instead. Typing sends it again at the usual pace (at most every 100 ms, the newest
  wins); a caret move alone sends the line.
- A document over the protocol's cap (2 MiB) is not previewed: a toast says "This document is too
  large to preview before it is saved."
- A viewer that does not know such documents answers the plain `ok`. The editor then ends the
  link and says "This mdview cannot preview a document that has no file: save it, or update
  mdview." Nothing is opened in that viewer.
- Saving the document (Save or Save As) tells the viewer once that the document is that file now
  (`?doc=<name>` with the path). The link carries on as a link to the file, with the rules of a
  saved file: the text is sent only while it differs from the file and "Live preview while
  typing" is on.
- Closing the tab, "Preview in mdview" again, or leaving the editor ends the preview as for a
  file. The viewer keeps the last text it was given.
- The viewer's "Edit in mdedit" on such a tab asks the running editor for the document by its
  name; the editor switches to that tab and goes to the line. A name that is not a tab of this
  editor is ignored.
- Menus: the two commands are listed in the View menu and the tab menu for a document that has no
  file, whatever its language (it has no name to tell its kind by), and are no longer disabled
  there. The palette lists them whenever a tab is open.

## Release note

- Preview in mdview and Preview side by side now work on a document that was never saved: mdview
  shows its text as you type, and saving turns the preview into the file's. Needs the mdview of
  the same release.

## Handoff

Built:

- `bins/mdedit/src/link.rs`: `Target::{File, Untitled}`, `doc_name`, `Opening`, `Links::
  link_untitled`, `Links::saved`, `Links::take_older`; the courier's send returns the shared
  `Delivery`; `Docs::link_poll` turns the link of a saved document into a link to its file and
  always sends the text of a document without a file.
- `bins/mdedit/src/app/handoff.rs`: `preview_untitled`, `handed_untitled`.
- Strings: `PREVIEW_UNSAVED_TOO_LARGE`, `PREVIEW_VIEWER_OLDER`; `PREVIEW_NEEDS_FILE` is gone
  (nothing says it any more).
- Arguments for shots and scripts: `--new` (start with an untitled document in front);
  `--demo-link` now also marks an untitled document as previewed.
- Shots: `menu-view-untitled`, `menu-tab-untitled-linked-dark`, `previewing-untitled`,
  `previewing-untitled-dark`.

Decisions taken, for the owner to confirm:

- The text of an unsaved document is sent live even with "Live preview while typing" off.
- The preview commands are listed for every document without a file, not only once its language
  is set to Markdown.
- Over 2 MiB such a document is not previewed at all (a toast).
- With an older mdview the editor says so and ends the link; the viewer was started bare by then
  and stays open, empty.
- `--new` was added as a launch argument.

Tried in real windows on Windows 11 (release builds, scratch settings, driven by script): Preview
side by side on a new document started mdview and tiled the two windows; typing updated the
viewer; a relative image and link showed as missing; Save As through the real dialog turned the
viewer's tab into the file's with no second tab; typing after the save was live again; a second
untitled document was previewed beside it and its preview stopped.

Budgets, measured on this PC with `--trace` on release builds of the base (`5ccd143`) and of
this branch, run in turn: first frame of `fixtures/sample.md` 111 to 124 ms before and 112 to
124 ms after (the machine was busier than when the PLAN's figures were taken); keystroke to
pixels p50 0.73 ms, p95 0.84 to 0.88 ms before and p50 0.72 ms, p95 0.86 to 0.98 ms after;
scroll frame p50 0.61 ms before and 0.62 ms after. Nothing runs for the feature until a
document is previewed. Not measured: memory, the 100 MB file (no code on those paths changed).

Not verified:

- Linux in real windows (CI runs the tests and shots there; the unix-only test
  `preview_of_a_document_without_a_file_starts_the_viewer_bare` runs only on CI).
- A really older mdview binary: the case is covered by a test with a receiver that answers as
  the old code does, and by a shared test, not by running mdview 0.3.1.
- "Edit in mdedit" from the viewer's untitled tab in real windows (app-level tests on both sides
  only).
