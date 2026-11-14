# Compare: copy a change across

## README
Feature list: "Copy a change across in a comparison: with the caret in a difference, put this side's lines in place of the other side's, or take the other side's lines, as one undo step."

Shortcut table: no new shortcuts. View menu and the command palette: "Copy this change to the other side", "Take this change from the other side". Next difference (F7) and Previous difference (Shift+F7) are also in the View menu now.

## DESIGN
- The caret of the active pane is in a difference block (or on the line right after the filler rows of a block this side has no lines of). "Copy this change to the other side" replaces the other document's lines of that block with this side's; "Take this change from the other side" does the reverse in this document.
- Only an editable document changes. In "Compare with saved" the saved copy (read-only) is never the target: Copy greys out with the working side active in the right pane, Take works. The commands are greyed (View menu, palette) when not compared, when the diff is out of date, when the caret is not in a difference, or when the target is read-only or lossy. Run by other paths (macro, test) they say why in a toast.
- One undo step in the changed document (Kind::Other). The target keeps its own line break; the break after the block, or the missing one at end of file, stays as it was. A block the source lacks removes the target's lines; a block the target lacks is inserted before the line named by the range.
- The source is unchanged; the diff refreshes by the normal edit path (worker, off the keystroke path). Only the block's lines are read, so large files cost nothing extra.
- Not recordable in macros.

## Release note
Compare: "Copy this change to the other side" and "Take this change from the other side" move one difference across as one undo step; Next and Previous difference are in the View menu.

## Handoff
Built: mdedit-core `merge` module (take_lines, replace_lines, unit tests), `compare::pick`/`Align::change_at_line`/`enabled`, `App::compare_copy`, EditOp::CopyLines, commands, strings, View menu entries, palette, `--copy-change across|take` for shots, app-level tests, four shots (compare-copied, compare-taken-dark, menu-view-compared, menu-view-compared-dark).
Decisions: no gutter arrow buttons (the compare view has no gutter between panes and the panes are ordinary editors; adding one would change the look); no new shortcut; next/previous difference already existed (F7/Shift+F7), only added to the menu.
Not verified: Windows (CI only); release-build budgets not measured (nothing runs at startup or on keystroke/scroll; the code runs only on the commands, and `enabled` runs per menu/palette build, a hunk lookup).
Left out: gutter arrows; copying part of a block (selection) or a single hunk inside a changed block.
