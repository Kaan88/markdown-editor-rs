# State of the work

Rewritten, not appended to. Last rewritten 2026-11-25, when mdedit 1.5.0 was released. If `git log -1`
is newer, doubt "In hand" first.

## Released

- **mdedit 1.5.0**, 2026-11-25, tag `v1.5.0` at `51d8777`, sixteen assets. Record
  `docs/release-log.md`, notes `docs/release-notes/1.5.0.md`. In it, since 1.4.0, the whole of
  `docs/ai-pictures-plan.md`: **F, E, L, O** (D-203, D-211, D-214), one "AI task…" command, the
  AI task from the Notes pane, what opens after a run, Settings › AI tasks; **C1, C2** (D-210,
  D-215 to D-220, DESIGN 88, 89), File them copies or moves files into the library; **V1 to V5
  and the sent card** (D-221 to D-230, DESIGN 90 to 95), Write notes from pictures with its plan
  on a card, written with a record of shared's kind `notes` that AI runs can undo. Fable reviewed
  every milestone until VERDICT: OK. Budgets unchanged, exe +439 KB
  (`docs/measurements/2026-11-25-write-notes.md`). **Not tried in a real window**, by the owner or
  by me; **no picture has been sent to a provider**; the unix fake-CLI tests of the engine run
  only in CI; copies are written on the UI thread (up to 200 MB); the plan lives in its task
  window (D-228), not yet "Plan ready" in the Notes pane.
- **mdview 1.1.0**, 2026-11-24, tag `v1.1.0` at `0310e01`, fourteen assets: click to zoom a large
  diagram, the notes library, audit 4's report E (mdview `docs/HANDOFF.md`).
- The shared crates are not released; the apps pin them by commit. Shared `main` is `294d330`:
  `feat/anim` (motion, tokens, the new widgets) merged with `ci` green on Linux, Windows and both
  Macs. **Hub** is its own app (its `docs/STATE.md` is its record).

## On `main`, not released

- Nothing.

## In hand

- **Motion in mdedit and mdview**, starting now that `feat/anim` is in shared `main`: shared
  `docs/ROADMAP.md`, "Fast-tracked for the md apps" (D1 to D7, and X3, X4, L1, L3, L10, W2, W3).
  Order (Fable's review): shared's D2, D3 first; then the menu open fade in mdview, then in mdedit
  with the budgets measured before and after (the completion popup left out); toasts and tooltips;
  dialogs and cards. No tab cross-fade in mdedit (memory), no sliding panes. Both apps pin
  shared `main` from `294d330` on.
- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.

## Waiting for the owner

- **`docs/ai-library-plan.md`** section 8, in order after R2: V, drag and drop; W, 4a, 4 and 5
  are merged. Open: the resizer for V, a new crate to be priced first (rule 7).
- **`docs/ai-web-plan.md`**, the AI searching the web, asked each run: its five decisions. A
  throwaway spike (Claude Code with only WebSearch, no ask) worked better than the owner expected
  (2026-11-24); its branch is deleted, its build kept in `md-wt\ai-web` (detached).
- **Audit 4's questions**: C4, a same-named group kept in another folder is refused and one with
  no folder of its own is joined (D-183, which replaces D-178's interim); A F8, a rewrite's record
  is not confined and its undo names the files (D-162, Waiting); W11 needs `jiff` (rule 7); the
  shared `%20` and `write_of` causes (D-180).
- **Dependabot**: open PRs (mdedit #9 regex-automata; mdview #2 to #6; shared #2 to #7) and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Left until after
  the release; it is after it now (1.4.0).
- The notes library is used in a real build and its two faults are fixed, but nothing records
  approval.

## Next

1. The motion above.
2. The owner's look at 1.4.0's and 1.5.0's shots (R2, W, 4a, 4, 5.8, DD, audit 4, the notes
   cards) and whether to add the viewer's page rows for R2's tinted preview (D-108); then section 8's order. M1, M2 in
   mdview; D after R2.
3. The closed mdview, still to be tried (first: whether it is missed with mdview 1.0.5 too).

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
- Opening Settings during an AI review discards the result.
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

- Shared crates: mdedit at `ac9fde1` (shared `main`, the notes kind); mdview at `ef88814`.
  Viewer crates at viewer `e2b3e31`; winit `62fa727`, softbuffer `dd18fec`.
- Branches: `main`; the drawings (`mock/ai-review`, `mock/settings`, `mock/terminal`,
  `mock/downloader`, `mock/workbench` local only, `mock/launcher`, `mock/file-manager`,
  `mock/task-manager`, `mock/ai-pictures`), never merged; `archive/doc-reorg-draft`; `spike/v-resizer`; on GitHub
  `spike/frosted-window` and `chore/dependabot`; Hub's `hub/notes-file` in both apps. Merged
  branches are deleted, here and on GitHub.
- Kept as found, unpushed: shared `archive/settings-draft-uncommitted`, Hub's
  `wip/install-uncommitted` (69 lines in `hub-core` and a second `hub.ico`).
- Worktrees: `main`; `hub-wt\app-prs2\mdedit`; `md-wt\ai-pictures`; `md-wt\ai-web` (detached at
  the spike, kept for its exe). `md-wt\review-a4fix` holds the review of the
  fixes, not a worktree.
