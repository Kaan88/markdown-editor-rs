# B+ spike: one block at a time

The question (docs/PLAN.md, "Visual editing, one block at a time"): can the rendered page turn one
block into its source, in place, fast enough and without the page jumping about?

What it is: one window, mdview's renderer at the revision mdedit pins, mdshared-ui's painter.
A click on a top-level block shows its source in a plain box where it was; Esc or a click outside
puts it back and renders again. No renderer change: while a block is open, the text before it and
the text after it are rendered as two documents, above and below the box.

    cargo run --release                     (sample.md)
    cargo run --release -- file.md
    cargo run --release -- --bench file.md  (opens, types 12 characters plus Enter into, and
                                             closes every block, three rounds; timings.txt)

Not in it: Ctrl+E (the whole source), selection, undo, mouse wheel inside the box, a monospace
face (the box uses the UI face), dark theme, the caret's block opened from the source.

## Measured, 2026-11-20, owner's Windows 11 PC, release build, 900x800 at scale 1

From the input to the frame presented.

| | sample.md (27 lines, 10 blocks) | docs/PLAN.md (423 lines, 77 blocks) |
|---|---|---|
| Open a block, p50 / p95 | 3.8 / 4.5 ms | 31.6 / 47.3 ms |
| Close it, p50 / p95 | 3.8 / 9.1 ms | 32.5 / 47.2 ms |
| Keystroke in the box, p50 / p95 | 0.9 / 2.5 ms | 1.7 / 4.5 ms |
| Enter (box grows a line), p50 | 0.9 ms | 1.4 ms |
| The box's top against the block's | within 6 px | within 6 px |
| The page below, on open | -19 to +53 px | -390 to +28 px |

Reading:
- Typing is cheap at any length: only the box is drawn; the page around it is not rendered
  again until the block closes.
- Opening and closing grow with the document, because the spike renders whole pages
  (`render`). The real feature would use the renderer's incremental path, as the preview pane
  does (only the rows in view, 8 to 9 ms a keystroke on Windows). Not measured here.
- The page below moves by the difference between the rendered block and its source: tables and
  lists come out taller as source, headings shorter, a long code block or table shorter by a lot
  (the -390 px). The top of the box stays put, so the block under the pointer does not move; what
  is below it does. Whether that feels bad is for the owner to judge by using it.
- Rendering the two halves alone can differ from the whole: a reference link or footnote
  defined on the other side of the open block shows unresolved until it closes.

### complex.md (front matter, three pictures, a wide table, nested lists and quotes, code, math, Mermaid; 25 blocks)

Images load from the file's folder; formulas and diagrams through the installed
`%LOCALAPPDATA%\Programs\mdedit\mdview-engine.exe`.

| | complex.md |
|---|---|
| Open a block, p50 / p95 | 39.8 / 41.7 ms |
| Close it, p50 / p95 | 91.3 / 93.4 ms |
| Keystroke in the box, p50 / p95 | 1.2 / 3.2 ms |
| The page below, on open | -423 to +119 px |

Closing is slow because the spike makes a new renderer each time, so every formula and diagram
goes to the engine again; mdedit's pane keeps one renderer and its caches. The -423 px is a
picture: one line of source where a tall image was.

## Not done

- Not run on Linux, not at scale 1.5 or 2, not with a 100 MB file.

## Verdict

The owner tried sample.md and complex.md on 2026-11-20: "fast, jumps are not jarring. it's
perfect". B+ goes ahead (docs/PLAN.md, steps 1 to 4).
