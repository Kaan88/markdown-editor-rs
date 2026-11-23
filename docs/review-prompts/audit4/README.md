# Audit 4: Fable's review of everything since audit 3 (2026-11-24)

Five Fable reviewers ran read-only, in parallel, each on its own worktree:

| Report | Scope | Tree |
|---|---|---|
| `a-mdedit-core.md` | mdedit: audit 3's fixes, split/runs/plan/move/undo/recycle, the notes pane, the fixes since 1.3.0, Settings | mdedit `1d503d8`, from `6cdeab2` |
| `b-mdedit-ai.md` | mdedit: R2 (review with ticks), W (task window), phase 4 (AI sort), the AI plumbing | mdedit `1d503d8`, from `6cdeab2` |
| `c-mdedit-library-writes.md` | mdedit: 4a (add a folder), phase 5 (a note from a text file), DD (drag a note) | mdedit `1d503d8`, from `6cdeab2` |
| `d-shared.md` | shared crates | shared `0984eed`, from `c603b23` |
| `e-mdview.md` | mdview since `v1.0.0` | mdview `8612e74` |

The probe tests they wrote are in `probes/`. They were taken out of the worktrees, so nothing in
any repo carries them. Nothing has been fixed yet. The owner decides what is fixed, and in which
order.

A finding that two reviewers made is listed once below, and each report's own numbers are
given. "Proven" means a probe test shows it. "Read" means it was found by reading only.

## Release blockers

1. **Crash** (B1, proven). In `bins/mdedit/src/task_ui.rs:682`, `digest_of` truncates at 2,048
   bytes, which can land inside a multi-byte character. A note that starts with about a page of
   Cyrillic, Greek or CJK text, added to "File them" with the AI on, aborts a release build.
2. **Undo lost on a large rewrite** (B2, D2, proven). A rewrite record over 4 MiB can never be
   read back (shared `runs.rs:34`, `:1211-1214`). The allowed limits, 20 files of 200 KB,
   produce such a record. The files are rewritten, and "AI runs" cannot undo them.
3. **`<file>.tmp` destroyed** (D1, B6, A7/F7; proven in shared). `write_atomic_bytes` uses
   `<path>.tmp` as its scratch name, and it now writes the owner's own files: rewrite, its undo,
   moved notes' links, and `put_note`. An existing `<name>.tmp` is truncated or removed, and a
   failed write leaves one behind. On Linux and macOS the rename also resets the file's mode
   (read). This is audit 3's C9, still open.
4. **A plan from a stale tab bins the newer document** (A F1, proven). In `app/plan.rs:46-75`,
   the notes come from the tab's buffer and the digest from the disk, and nothing compares the
   two. The newer file can be got back from the Recycle Bin.
5. **A non-UTF-8 library file rewritten lossily** (D3, proven). In `library/put.rs:122-141`,
   `put_note` drops `decode`'s `invalid` flag, so a Windows-1254 `Çağrı` is written back as
   `�a�r�`. mdview's "Add to the notes library" reaches it.

## Wrong behaviour that can lose or misplace things

- **Undo of a refused note** (A F2, proven). A note the Recycle Bin refuses at undo stays on disk
  with its line gone. The run is marked undone and cannot be undone again.
- **Finish** (A F4, proven). `finish` does not record the headings it adds, so undo leaves them.
- **Undo removes a heading with the owner's sub-heading under it** (A F5, proven). Audit 3's C6
  is not fixed.
- **Finish offered only for Stopped or Writing** (A F3). Audit 3's C4 is not fixed, and a test
  asserts the old words.
- **A file rewritten twice in one record is half undone** (D, proven). Whether mdedit can make
  such a record is not proven.
- **A run whose group file was never made, with its home outside the library, is stuck** (D,
  proven).
- **"Move the notes…" offers, ticked, to move another group's library file out of the library**
  (A F6; the plan is proven, the write was not run).
- **Escaping a rewrite record** (A F8, read). A rewrite record is not kept inside the library,
  and no decision replaces D-072 for it. A7's check is lexical, so `home: Trip/..` passes
  (A F14, read).
- **DD drops outside the pane** (C1, proven). Letting go of a note over the editor, at a row's
  height, still moves it.
- **DD holds the note by row index** (C2, proven; A F15 and F16 read). Rows made again during a
  drag, by a key or a finished library read, move another note. A row menu left open acts on
  whatever row now has its index.
- **Phase 5 failed write** (C3, proven). A write that fails after the record is made loses the
  card and the AI's answer, and leaves a "Done" record that made nothing.
- **4a joins a same-named group** (C4, proven). A group with the same name and another `home:` is
  joined silently, and the card names the wrong folder. DESIGN 81 says it is refused. **The owner
  decides: refuse it, or join it and say so.**
- **4a offers the library's own files** (C5, proven). Picking the folder that holds the library
  offers its library files as notes, ticked.
- **mdview's "Add to the notes library" while mdedit has the file open** (E3, plausible). It
  writes a library file mdedit may have open with unsaved changes.
- **Pasted HTML** (D, proven by its delegate). `&lt;<span>script&gt;` becomes a live `<script>`.
- **Recycle Bin on Windows** (D, read). A file larger than the Recycle Bin's capacity may be
  deleted for good and reported as `Ok`.

## Wrong behaviour, the task window (W)

- Discard and Copy result ask for the same file again, so the window never moves on (B3,
  proven).
- A kept file review applied after another window is open marks the wrong file done. It also
  sends a file's text with an empty instruction (B4, proven).
- The task switch takes a click while a request is out: a filing answer becomes a rewrite
  review, and Split can wait forever (B5, proven in part).
- A window parked by "Show what is sent" comes back as File them (B7, proven).
- From reading:
  - Split them can apply one file's answer to another file's plan.
  - A file edited after it was added is sent as its old text.
  - Rewrite them runs with an empty instruction.
  - A kept review cannot be reached while the window is open.
  - The 20-file limit counts unticked files.
  - (B10 to B14.)

## Wrong behaviour, other

- mdedit:
  - "Keep the folders" after an AI sort keeps the AI's sections (B8, proven).
  - 4a:
    - A `%20` in one file name refuses the whole folder, at Add, naming no file (C6,
      proven).
    - Bare-CR files give a "title" up to 8 KB long (C7, proven).
    - The walk cannot be stopped (C8, read).
    - A new group is made in two writes (C9, read).
  - Off Windows, "Find the file…" can rewrite the wrong note's link (A F12, read).
  - Audit 3's A8 is fixed for a kept plan, but not for a tab that has not re-read its file (A).
- mdview:
  - **Idle CPU** (E1, proven): every pass of the event loop resolves on disk the path of each
    tab mdedit asked for, 131 µs with three tabs.
  - Zoom opens at 2 × scale × the page's size, so 4x at 200% display scaling (E2, proven).
  - Zoom stays up over a document mdedit replaced (E, proven).
  - From 240 library files up, the palette lists notes under the wrong file's header (E, read).
  - The palette resolves every note's path on each typed character (E, plausible).
  - A task-box tick can pair with a newer text (E, plausible, narrow).
- Shared: `overlay::draw_dialog_button_disabled` was changed in place, against rule 8. Its one
  caller is mdedit's `ai_ui.rs:586`.

## Test gaps

- mdedit:
  - The requests-at-once test passes with its limit deleted (B9, proven by mutation).
  - DD's let-go-outside test passes only because of its fixture (C10).
  - `remove_own_file` is reached by no test.
  - The dry run before the file is made has no test.
  - No test walks through a symlink or junction.
  - Phase 5's failed write has no test.
  - A test asserts C4's old toast words (A F10).
- mdview: the zoom-pan assertion passes with the drag removed (E, proven).
- Shared: three test gaps (D).

## Nits

About 25 across the five reports, each in its own report: blank lines left by undo, case rules,
a cross-file drag's order, nits in zoom, `?closed` and the protocol, and others.

## Not covered

Each report ends with what it did not reach. Most of it:
- the drawing code (`*_draw.rs`, the shared plan card's drawing);
- `ai_ui.rs`;
- parts of `moves.rs` and `moves_links.rs`;
- the shared `library.rs` writers and the paged settings form;
- audit 3's C8 (costs on the UI thread);
- going through the existing tests one by one for "fails with its fix reverted".

Nothing was run on Linux or macOS, and nothing in a real window.
