# State of the work

Rewritten, not appended to. Last rewritten 2026-11-23, after mdedit 1.3.0 and mdview 1.0.5 were
released. If `git log -1` is newer, doubt "In hand" first.

## Released

- **mdedit 1.3.0**, 2026-11-23, tag `v1.3.0` at `ebe939d`, sixteen assets, `hub.txt` new. Record
  `docs/release-log.md`, notes `docs/release-notes/1.3.0.md` (the notes library; Split by headings
  or AI with records and undo; Move the notes; Settings as pages; D-097, D-098). Checked by hand
  before the tag (D-101; "Move the notes" not tried); budgets unchanged against 1.2.1
  (`docs/measurements/2026-11-23-1.3.0-against-1.2.1.md`).
- **mdview 1.0.5**, 2026-11-23, tag `v1.0.5` at `0a9bc7c`, fourteen assets: Settings as pages,
  `?closed`, a version resource and `hub.txt` (mdview `docs/HANDOFF.md`).
- The shared crates are not released; the apps pin them by commit. **Hub** is its own app (its
  `docs/STATE.md` is its record; 0.1.0 notes written, no tag).

## On `main`, not released

- **A file added to the library shows in the pane at once** (`fix/library-add-refresh`): its rows
  were made again only when a group or section was folded; now as it is written.
- **Browsing the library in the full preview** (`fix/preview-browse`, D-104): a tab still being
  read no longer ends the mode; `visual_sync` waits for it.
- **The delete-a-library-file question says where the notes go** (`fix/delete-library-notice`,
  D-103), and its card grows from its wrapped lines (shots `library-delete`,
  `library-delete-long`; not looked at by the owner).
- **A closed mdview is asked about again** (`fix/closed-mdview`, D-102, D-105): at 1, 2 and 4 s,
  with a refused check kept to its grace's end and a key or click asking at most once in two
  seconds. Not tried by the owner; keystroke and idle times not measured (not run).
- Otherwise nothing but documents since `v1.3.0`. Nothing since 1.2.0 has had an outside review
  (audit 3's fixes included).

## In hand

- **Phase 4, "Sort a folder by AI."** (`ai-library-plan.md` 5.7's second half, on 4a's card):
  built and tested on `f/p4`, worktree `md-wt\p4`, not merged (D-150 to D-154, `DESIGN.md` 84).
  **Ask the AI** returns a scheme of at most 8 sections with subsections, which the owner can add
  to, rename and delete; **Sort N files** asks for a section, a confidence and a reason per file,
  40 files a request, up to the connection's "Requests at once" (3, or 1 for a local model), 500
  files a run. Unsure and unplaced files land in **Unsorted**, unticked; **Tick only the sure
  ones** keeps the 0.6 and over; nothing is written until Add. Shots (`import-folder-scheme`,
  `-scheme-rename`, `-ai`, `-ai-sorting`, `-ai-asking`, `-ai-off`, `-ai-failed`, `-unsorted`, both
  themes) looked at on Windows; budgets measured against `main` at `4c95da0`
  (`docs/measurements/2026-11-23-phase-4-ai-sort.md`: exe +38,912 bytes, no figure beyond the
  runs' ranges). It waits for the owner's look and word to merge.
- **Phase W**, the AI task window over files (`5.6`, D-074, D-076, D-140 to D-145): merged from
  `f/pw`, not tagged; the owner has not looked at its shots. File them, Split them and Rewrite
  them; a rewrite is R2's review over the file, applied in place with its original in the run's
  record and undo in AI runs. Budgets measured (`docs/measurements/2026-11-23-task-window.md`):
  no change beyond the runs' ranges; shots `task-*` (both themes, `task-sent` in light) looked at;
  docs 83.
- **Phase 5.8**, a Markdown note from a text file (`5.8`, D-120): merged from `f/p5` - card,
  write, record, undo, 14 tests, 11 shots, budgets measured
  (`docs/measurements/2026-11-23-note-card.md`); pins on shared `main` (`0984eed`); a Rewrite
  record is listed as a Rewrite (D-145 lifts D-134's guard); the owner has not looked at its shots.
- **Phase 4a, "Add a folder to the library…"** (`5.7`, without the AI): merged from `f/p4a`. A
  folder becomes a group of links to its `.md`, `.markdown` and `.txt` files under their
  subfolders' headings three deep, in one library file; nothing is copied, moved or renamed
  (D-131 to D-133, `DESIGN.md` 81). The owner has not looked at its shots (`import-folder*`).
  Budgets: nothing on the launch path, no new dependency, figures inside both builds' ranges
  (`docs/measurements/2026-11-23-phase-4a.md`); exe +97 KB.
- **Phase R2**, the review with ticks (`5.5`, D-107): merged from `f/review-hunks`, not tagged;
  the owner has not looked at its shots. A tick per change, Apply and Copy result of the ticked
  ones, F7/Space, the wrapped list with a scrollbar, the strip, the cheap Preview (AFTER page).
  Built, tested, fmt and clippy clean, shots (`review-*`, both themes, `review-closed` new) looked
  at, budgets measured (5 runs against `main` at `32d6fe6`: no change beyond the runs' ranges).
  Docs: `DESIGN.md` 79, D-106 to D-108, `docs/measurements/2026-11-23-review-hunks.md`. **Not
  built: the tinted, level preview (D-108)**: it needs a request in the viewer's preview protocol
  (the page rows of every top-level block) and a move of its pin, which waits for the owner.
- **Phase DD, dragging a note in the notes pane** (D-130, `DESIGN.md` 80): merged from `f/pdd`,
  not tagged; the owner has not looked at its shots (`library-drag`). A note dropped on a group or
  section is moved there by the "Move to…" write; notes only. Not built, waiting for the owner:
  dragging a group or section row, and files dropped from the file manager (winit discards the
  drop point on Windows).
- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.
- **mdview: click to zoom a large diagram**, asked for by the owner on 2026-11-23 for later;
  written up at the top of mdview's `docs/HANDOFF.md`. Not designed, not priced.

## Waiting for the owner

- **`docs/ai-library-plan.md`** section 8, in order after R2: V, phase 4 (built on `f/p4`, waiting
  for the owner's look), drag and drop; W, 4a and 5 are merged. Open: the resizer for V, a new
  crate to be priced first (rule 7).
- **Dependabot**: open PRs (mdedit #9 regex-automata; mdview #2 to #6; shared #2 to #7) and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Left until after
  the release; it is after it now.
- The notes library is used in a real build and its two faults are fixed, but nothing records
  approval.

## Next

1. The owner's four faults from the hand check (D-101), for the next release: the add, the full
   preview, the delete notice and the closed mdview are merged; the closed mdview is still to be
   tried (first: whether it is missed with mdview 1.0.5 too).
2. Then the owner's look at R2's shots (and whether to add the viewer's page rows for the tinted
   preview, D-108), then section 8's order. M1, M2 in mdview; D after R2.
3. Phase 4's shots (`import-folder*`, including the scheme, the sorting, the AI-off and the
   unsorted cards) for the owner's look, then merging `f/p4`; then 4a's and 5's shots
   (`import-folder*`, `note-*`).

## Known to be wrong on `main`

- **A closed mdview is still missed** (D-101): closing it by hand leaves "Stop the preview" in
  mdedit after its window has the focus again (tried with 1.0.4, which sends no `?closed`), so
  D-097's focus check does not hold. D-102 and D-105 change this (merged, not tried): three
  rechecks, a check kept to the end of a link's grace, and an ask at most once in two seconds.
  With 1.0.4 and a graceful close the old code found it in 1 s against the real exe, so the
  failing case was not reproduced; a tab closed in a viewer that keeps running cannot be told
  from 1.0.4 at all.
- **The 512-file limit.** Past 512 `.md` files in the library folder those that sort last drop
  out; the foot says how many are shown but the rest cannot be listed. Flat notes count.
- Opening Settings during an AI review discards the result. The editor never reads the AI
  engine's version.
- **Deleting a library file orphans its notes.** It removes the list, never a note, so the folder
  keeps every note unlisted and splitting that document again writes nothing (every note reads
  "already there" and comes unticked). Found by the owner. The question now says the notes are
  kept on disk; deleting itself is unchanged.
- **The move**: a crash between the library rewrites and the digest update gives a half-undo;
  folders emptied in a home outside the library stay.
- **A debug build aborts on a closed stderr** (not a release one): something only debug prints
  with `std`'s `eprintln!`; CI runs release and is unaffected.
- **The `runs*` shots draw the clock**, so two runs of `tests/shots.sh` are not byte-identical.

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
- Shared crates at `0984eed` in both apps. Viewer crates at viewer `e2b3e31`; winit `62fa727`,
  softbuffer `dd18fec`.
- Branches: `main` and `f/p4` (phase 4, open); the drawings (`mock/ai-review`, `mock/settings`,
  `mock/terminal`, `mock/downloader`, `mock/workbench` local only, `mock/launcher`,
  `mock/file-manager`, `mock/task-manager`), never merged; `archive/doc-reorg-draft`; on GitHub
  `spike/frosted-window` and `chore/dependabot`. Merged branches are deleted, here and on GitHub.
- Kept as found, unpushed: shared `archive/settings-draft-uncommitted`, Hub's
  `wip/install-uncommitted` (69 lines in `hub-core` and a second `hub.ico`).
- Worktrees: `main`; `md-wt\pw`, `p4`, `ci-fix` and their bases, `spike-v`, `baseline-main`,
  `p4-mainbase`; `hub-wt\app-prs2\mdedit`. The empty `md-wt\closed-mdview` and `mv-notes` are
  held by a process; delete after a restart.
