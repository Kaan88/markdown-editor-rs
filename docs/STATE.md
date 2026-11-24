# State of the work

Rewritten, not appended to. Last rewritten 2026-11-24, when audit 4's fixes and the scroll fix
were merged to `main`. If `git log -1` is newer, doubt "In hand" first.

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
- **A wrapped file reopened past its end** (owner's report, D-199): with word wrap on, a place put
  back lowers its top once so the last row is no higher than the view's bottom. Not tried by the
  owner.
- **Phase 4, "Sort a folder by AI."** (`ai-library-plan.md` 5.7's second half, on 4a's card):
  merged from `f/p4` (`188c876`; D-150 to D-154, `DESIGN.md` 84). **Ask the AI** returns a scheme
  of at most 8 sections, **Sort N files** places each file with a confidence and a reason, up to
  "Requests at once" (3, or 1 for a local model); nothing is written until Add. Shots
  (`import-folder-scheme*`, `-ai*`, `-unsorted`) looked at on Windows; budgets in
  `docs/measurements/2026-11-23-phase-4-ai-sort.md` (exe +38,912 bytes, nothing beyond the runs'
  ranges). Two tests that failed only on CI are fixed (`f13892c`).
- **Phases W, 5.8, 4a, R2 and DD**, merged after `v1.3.0`, each built, tested, its shots looked
  at and its budgets measured (`docs/measurements/2026-11-23-*.md`); the owner has looked at none
  of their shots. **W** (D-140 to D-145, DESIGN 83): the AI task window, File, Split and Rewrite
  them, a rewrite applied in place with its original in the record. **5.8** (D-120): a Markdown
  note from a text file. **4a** (D-131 to D-133, DESIGN 81): a folder becomes a group of links,
  nothing copied or moved; exe +97 KB. **R2** (D-106 to D-108, DESIGN 79): a tick per change; the
  tinted, level preview is not built (it needs a request in the viewer's protocol). **DD** (D-130,
  DESIGN 80): a note dragged onto a heading; groups, sections and files from the file manager
  are not dragged.
- **Audit 4 and its fixes** (`docs/review-prompts/audit4/`): Fable's review of everything since
  audit 3, every finding fixed or left with a reason (`FIXES.md`), then a Fable review of the
  fixes and its findings fixed (D-155 to D-198); merged at `2f8399b`. Shared `main` is `ef88814`,
  pinned by both apps. Windows: 1584 tests, 400 shots checked. Budgets against 1.3.0 unchanged,
  memory lower, exe +396 KB (`docs/measurements/2026-11-24-1.4.0-against-1.3.0.md`). The owner
  has looked at none of the shots.

## In hand

- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.
- **mdview: click to zoom a large diagram**, asked for by the owner on 2026-11-23 for later;
  written up at the top of mdview's `docs/HANDOFF.md`. Not designed, not priced.

## Waiting for the owner

- **`docs/ai-library-plan.md`** section 8, in order after R2: V, drag and drop; W, 4a, 4 and 5
  are merged. Open: the resizer for V, a new crate to be priced first (rule 7).
- **Audit 4's questions**: C4, a same-named group kept in another folder is refused and one with
  no folder of its own is joined (D-183, which replaces D-178's interim); A F8, a rewrite's record
  is not confined and its undo names the files (D-162, Waiting); W11 needs `jiff` (rule 7); the
  shared `%20` and `write_of` causes (D-180).
- **Dependabot**: open PRs (mdedit #9 regex-automata; mdview #2 to #6; shared #2 to #7) and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Left until after
  the release; it is after it now.
- The notes library is used in a real build and its two faults are fixed, but nothing records
  approval.

## Next

1. Audit 4's fixes, reviewed and fixed again: the merges to `main`, shared first, then CI.
   The owner's four faults from the hand check (D-101) are merged; the closed mdview is still to
   be tried (first: whether it is missed with mdview 1.0.5 too).
2. Then the owner's look at R2's shots (and whether to add the viewer's page rows for the tinted
   preview, D-108), then section 8's order. M1, M2 in mdview; D after R2.
3. Phase 4's, 4a's and 5's shots (`import-folder*`, `note-*`) for the owner's look.

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

## Pins, branches, worktrees (tidied 2026-11-24)
- Shared crates at `ef88814` (shared `main`) in both apps. Viewer crates at viewer `e2b3e31`;
  winit `62fa727`, softbuffer `dd18fec`.
- Branches: `main`; the drawings (`mock/ai-review`, `mock/settings`, `mock/terminal`,
  `mock/downloader`, `mock/workbench` local only, `mock/launcher`, `mock/file-manager`,
  `mock/task-manager`), never merged; `archive/doc-reorg-draft`; `spike/v-resizer`; on GitHub
  `spike/frosted-window` and `chore/dependabot`; Hub's `hub/notes-file` in both apps. Merged
  branches are deleted, here and on GitHub.
- Kept as found, unpushed: shared `archive/settings-draft-uncommitted`, Hub's
  `wip/install-uncommitted` (69 lines in `hub-core` and a second `hub.ico`).
- Worktrees: `main`; `hub-wt\app-prs2\mdedit`. `md-wt\review-a4fix` holds the review of the
eview-a4fix` holds the review of the
  fixes, not a worktree.
