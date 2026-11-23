# State of the work

Rewritten, not appended to. Last rewritten 2026-11-23, after mdedit 1.3.0 and mdview 1.0.5 were
released. If `git log -1` is newer, doubt "In hand" first.

## Released

- **mdedit 1.3.0**, 2026-11-23, tag `v1.3.0` at `ebe939d`. Sixteen assets, `hub.txt` the new one.
  Record: `docs/release-log.md`; notes: `docs/release-notes/1.3.0.md`. In short: the notes
  library and its pane; Split into notes by headings or by AI, with a record per run and undo; the
  original into the Recycle Bin; Move the notes with their pictures; Settings as pages; issues #7
  and #8 (closed); a closed mdview noticed (D-097, which the owner's check found does not hold:
  below); a version resource and `hub.txt` for Hub (D-098). Checked by the owner by hand before
  the tag (D-101; "Move the notes" not tried); budgets unchanged against 1.2.1
  (`docs/measurements/2026-11-23-1.3.0-against-1.2.1.md`).
- **mdview 1.0.5**, 2026-11-23, tag `v1.0.5` at `0a9bc7c`, fourteen assets: Settings as pages,
  `?closed` to mdedit, a version resource and `hub.txt`. Its note line was published as "(not
  released yet)" and corrected on the release the same hour, with the owner's yes; copies that
  saw it first keep it until the next release. Recorded in mdview's `docs/HANDOFF.md`.
- The shared crates are not released; the apps pin them by commit.
- **Hub**, the launcher and updater, is built as its own app (`Kaan88/mdhub`, `Desktop\hub`; its
  `docs/STATE.md` is its record; 0.1.0 notes written, no tag).

## On `main`, not released

- **A file added to the notes library shows in the pane's tree at once** (branch
  `fix/library-add-refresh`, merged): the note and its line were written, but the pane's rows were
  made again only when a group or a section was folded and opened, so the add looked lost. The rows
  are made again as it is written, with what was folded above the note opened, and a name being
  typed in the pane's box goes with them, its keyboard back in the text.
- **Browsing the library in the full preview** (visual editing, branch `fix/preview-browse`,
  merged, D-104): a tab still being read no longer ends the mode; `visual_sync` waits for it.
- **The question before deleting a library file says where the notes go** (branch
  `fix/delete-library-notice`, merged, D-103): it said only that the file goes, though the notes it
  lists stay on disk with nothing listing them; a file that lists none no longer gets that second
  sentence. Its card was a fixed 460x196, and a long group or file name wrapped a fourth line
  through the divider into the buttons; the card now grows from the lines the words wrap to (shots
  `library-delete`, `library-delete-long`; the owner has not looked at them).
- **A closed mdview is asked about again** (branch `fix/closed-mdview`, merged, D-102 and D-105): after
  1, 2 and 4 s; a check refused while a link is still in its grace is kept and made once when the
  grace ends; a key press or a click asks at most once in two seconds, answered once and starting
  no rechecks. Not tried by the owner; keystroke and idle times not measured (the app was not run).
- Otherwise nothing but documents since `v1.3.0`. Nothing since 1.2.0 has had an outside review
  (audit 3's fixes included).

## In hand

- **Phase 5.8**, a Markdown note from a text file (`ai-library-plan.md` 5.8, D-120): built on
  branch `f/p5` - card, write, record, undo, 13 tests, 11 shots, budgets measured
  (`docs/measurements/2026-11-23-note-card.md`); not merged.
- **Phase R2**, the review with ticks (`ai-library-plan.md` 5.5, D-107): **merged from
  `f/review-hunks`, not tagged; the owner has not looked at its shots.** A tick per change, Apply and Copy result of the
  ticked ones, F7/Space, the wrapped list with a scrollbar, the strip, the cheap Preview (the
  AFTER page shows the ticked changes). Built, tested (`cargo test --release --workspace` green),
  fmt and clippy clean, the shots (`review-*`, both themes, `review-closed` new) looked at on
  Windows, the budgets measured (5 runs against `main` at `32d6fe6`: no change beyond the runs'
  ranges). Docs: `docs/DESIGN.md` 79, `docs/DECISIONS.md` D-106, D-107, D-108 and
  `docs/measurements/2026-11-23-review-hunks.md`. **Not built: the tinted, level preview
  (D-108)**: it needs a request in the viewer's preview protocol (the page rows of every
  top-level block) and a move of its pin, which waits for the owner. Not looked at by the owner
  yet (rule 6).
- **Phase DD, dragging a note in the notes pane** (D-130, `docs/DESIGN.md` 80): merged from `f/pdd`,
  not tagged; the owner has not looked at its shots (`library-drag`). A note dropped on a group or a
  section is moved there by the "Move to…" write; notes only. Not built, waiting for the owner:
  dragging a group or section row, and files dropped from the file manager (winit discards the drop
  point on Windows).
- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.
- **mdview: click to zoom a large diagram**, asked for by the owner on 2026-11-23 for later;
  written up at the top of mdview's `docs/HANDOFF.md`. Not designed, not priced.

## Waiting for the owner

- **`docs/ai-library-plan.md`** section 8, in order after R2: W, V, 4a, 4, 5, drag and drop.
  Open: the resizer for phase V, a new crate to be priced first (rule 7).
- **The apps that may join**: `docs/ideas/README.md` lists each plan and drawing (terminal, file
  manager, task manager, downloader, workbench, and the quickest first release).
- **Dependabot**: open PRs (mdedit #9 regex-automata; mdview #2 to #6; shared #2 to #7) and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Left until
  after the release; it is after it now.
- The notes library has been used in a real build and its two faults fixed, but it is not
  recorded anywhere as approved.

## Next

1. The owner's four faults from the hand check (D-101), for the next release: the add, browsing
   the library in the full preview, the delete notice and the closed mdview are all merged; the
   closed mdview is still to be tried by the owner (first: whether it is missed with mdview 1.0.5
   too).
2. Then the owner's look at R2's shots (and whether to add the viewer's page rows for the tinted
   preview, D-108), then section 8's order. M1, M2 in mdview; D after R2.

## Known to be wrong on `main`

- **The AI review has no scrollbar and cuts long lines.** Fixed on `f/review-hunks` by R2 (the
  list wraps and has the scrollbar), not merged.
- **A closed mdview is still missed** (the owner's hand check, D-101): closing mdview by hand
  leaves "Stop the preview" in mdedit, even after mdedit's window has the focus again (tried with
  mdview 1.0.4, which sends no `?closed`). D-097's focus check does not hold. D-102 and D-105
  change this (merged, not yet tried by the owner): three rechecks at
  1, 2 and 4 s, an ask kept to the end of a link's grace instead of dropped, and an ask on a key
  press or a click at most once in two seconds. With mdview 1.0.4 and a graceful close the old
  code already found it in 1 s in a test against the real exe, so the owner's failing case was
  not reproduced here; a tab closed in a viewer that keeps running cannot be told from 1.0.4 at
  all.
- **The 512-file limit.** Past 512 `.md` files in the library folder those that sort last drop
  out; the foot says how many are shown but the rest cannot be listed. Flat notes count.
- Opening Settings during an AI review discards the result.
- The editor never reads the AI engine's version.
- **Deleting a library file orphans its notes.** It removes the list, never a note, so the folder
  keeps every note with nothing listing them and splitting that document again writes nothing
  (every note reads "already there" and comes unticked). Found by the owner. The question now says
  the notes it lists are kept on disk (shots `library-delete`, `library-delete-long`); deleting
  itself is unchanged.
- A pane the owner opened keeps painting the last page after every tab is closed;
  `ui.page_selected` is not cleared when the pane closes.
- **The move**: a crash between the library rewrites and the digest update gives a half-undo;
  folders emptied in a home outside the library stay.
- **A debug build still aborts on a closed stderr** (a release build does not): something that
  runs only in debug prints with `std`'s `eprintln!`. A plain debug `cargo test` fails
  `closed_stderr`; CI runs release and is unaffected.
- **Shared CI on Linux:** `pane::tests::names_show_while_they_fit…` measured 230 px on one Linux
  run and passed on the one before: it depends on the runner's fonts.
- **The `runs*` shots draw the clock**, so two runs of `tests/shots.sh` are not byte-identical.
- The uninstall entry of an installed copy says the version it was installed at (1.0.6 on the
  owner's PC); from 1.3.0 an update the app installs itself records the new one (D-098).

## Known gaps, not planned unless they get in the way

A Linux trash entry gets no deletion date (D-092); a note missing on disk is marked at the next
re-read; the Files and Functions faces have no keyboard; the notes pane has no sort or filter; the
plain preview and side-by-side have no scrollbar; no access keys in the line-ending and encoding
menus; no function list for Vue or Svelte; column selection pads short lines on typing only; no
compare arrows with wrap on; the Markdown menu needs more than 800 px. Split by AI has no progress
row with Cancel and no headings per source; the Recycle Bin cannot restore an original (D-071).

## Not checked by hand

Linux blur, image paste on Linux, the AppImage and packages, the macOS builds, the Windows
installer, an installed copy updating itself, and `hub.txt` read by Hub from a real release.

## Pins, branches, worktrees (tidied 2026-11-23)

- Shared crates at `09c67cd` in both apps. Viewer crates at viewer `e2b3e31`; winit `62fa727`,
  softbuffer `dd18fec`.
- Branches here: `main`; the drawings `mock/ai-review`, `mock/settings`, `mock/terminal`,
  `mock/downloader`, `mock/workbench` (local only), `mock/launcher`, `mock/file-manager`,
  `mock/task-manager` (also on GitHub), never to be merged; `archive/doc-reorg-draft` (local:
  the D-058 rearrangement as drafted); on GitHub, `spike/frosted-window` and `chore/dependabot`.
  Every merged branch was deleted, here and on GitHub, in all three repos.
- Kept as found, local, unpushed: the shared repo's `archive/settings-draft-uncommitted`, Hub's
  `wip/install-uncommitted` (69 lines in `hub-core` and a second `hub.ico`).
- Worktrees: `main` (`Desktop\markdown-editor-ai`); `md-wt\baseline-main`, `closed-mdview`,
  `delete-library-notice`, `library-add-refresh`, `preview-browse` and `review-hunks`;
  `hub-wt\app-prs2\mdedit`; and a session's detached scratchpad under `Temp`. The folders three
  finished sessions still hold under `.claude\worktrees\` are empty.
