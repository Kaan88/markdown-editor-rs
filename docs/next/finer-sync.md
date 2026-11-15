# finer-sync: the preview follows the caret inside long blocks (mdedit side)

Shared commit: `368aef1` (branch `f/finer-sync` of markdown-shared-rs: `docs/SISTER.md` and one
doc comment; no code changed there). The viewer's side, where most of the work is, is on the
branch of the same name in markdown-viewer-rs. This branch is on top of `f/live-preview-unsaved`.

## README

Change the "With mdview" item of the feature list to end with:

  ... The preview follows the caret line by line inside long paragraphs, lists, code blocks and
  tables, and along a paragraph written as one long line.

No new shortcut, no setting.

## DESIGN

**The caret's place, not only its line.** While a document is previewed, each update to the
viewer carries the caret's line and now also its byte in that line, from 1 (`?col=`, the same
number `--goto line:column` takes). The viewer shows the row of the page that holds that place:
the line inside a long paragraph, list item or code block, the row of a table, and for a
paragraph written as one long line the row the caret is in (the viewer's
`docs/next/finer-sync.md`, later its DESIGN.md, has the mapping).

- An update is due when the caret is on another line, as before, or has moved to another
  stretch of 32 bytes of the same line (`link::COLUMN_STEP`). Moving inside a stretch sends
  nothing, so a short line never sends more than it did; a long line sends at most the usual
  ten updates a second while the caret travels along it.
- An update that goes anyway (a new line, typed text) carries the exact byte.
- The first hand-over (`Preview in mdview`) starts the viewer with the line alone, as before
  (`--line`); when the caret is further along a long line, the first update says so.
- The viewer's "Edit in mdedit" now names a column when the row at the top of its page is
  inside a line that wraps; the caret goes to that byte (`go_to`, which always took one).

**Older builds.** Nothing new on the wire: `?col=` was always part of a request and an mdview
that predates this skips it, showing the block's first line as before. An older mdedit sends no
column and this release's mdview shows the line's beginning.

## Release note

- The side-by-side preview follows the caret inside long paragraphs, lists, code blocks and
  tables, not only from block to block. Needs the mdview of the same release.

## Handoff

Built:

- `bins/mdedit/src/link.rs`: `Active::column`, `Update::column`, `COLUMN_STEP`; `Linked::sent`
  is the line and the stretch of it; `Docs::link_poll` reads the caret's byte in its line.
- Tests: `link::tests::a_caret_moving_along_a_long_line_tells_the_viewer_every_stretch`,
  `app::handoff::tests::the_caret_in_a_long_line_tells_the_viewer_how_far_along_it_is` (through
  a stand-in for the viewer's window: the column on the wire, in bytes for two-byte letters, the
  text with it, and the caret put at a column the viewer names).

Decisions taken, for the owner to confirm:

- The column is a byte offset, not a character or a visual column: it is what `--goto` already
  meant, and the viewer maps bytes of the file.
- 32 bytes is the stretch: about a third of a row of the preview at its usual width.
- No shot: nothing new is visible in the editor.

Budgets, measured on this PC (Windows 11, release builds, three runs each of
`--trace --type-steps 200` and `--trace --scroll-steps 120` on the 610-line test document, no
preview running): keystroke to pixels p50 0.58 to 0.59 ms before (`merge/next-release` at
`2f4fcd0`) and 0.58 ms after, p95 0.62 to 0.64 and 0.63 to 0.66; a scroll step's own work p50
0.39 to 0.40 ms before, 0.40 ms after. No difference beyond the runs' own spread.
Nothing runs for it while no document is previewed; while one is, a pass of the loop reads one
more number from the buffer (the start of the caret's line).

Tried in real windows on Windows 11: a script started this mdedit on a 610-line
test document with `--run PreviewSideBySide`, which started the mdview of the same branch beside
it (both with scratch settings), posted arrow, Home and End keys to the editor's window and saved
each window's own picture after each. Seen: the preview follows the caret through a 300-line code
block and a 300-line paragraph, and along a 27000-byte one-line paragraph (byte 9000, End, Home);
the first update after the hand-over carries the column. Not tried in a real window: the viewer's
"Edit in mdedit" with a column (tested headless), typing with live text on a long line, and
anything by hand.

Not verified: Linux in real windows; a person's judgement of how the following feels.
