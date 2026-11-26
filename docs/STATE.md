# State of the work

Rewritten, not appended to. Last rewritten 2026-11-26, after the X3 review. If
`git log -1` is newer, doubt "In hand" first.

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
- The shared crates are not released; the apps pin them by commit. mdedit pins shared `main` at
  `09b01c1` (through `8d35a07`, shared PR 24 with the toasts' wait, plus the D4 hover flash, the
  colour row, the backdrop veil, the pool's cancel and the hand-over; `ci` green on Linux, Windows
  and both Macs; the lock change accepted by the owner, D-234; the pin moved three times since, by
  the shared `rev` alone: `f45a71c` (D-250), `535ad4c` (D-252) and `09b01c1` (D-256); D-253 moved
  no pin). **Hub** is its own app (its `docs/STATE.md` is its record).

## On `main`, not released

- **Motion, step 1: menus fade in as they open** (merged 2026-11-25 on the owner's word, after
  they tried mdview's). Menus, context menus and submenus
  come in over 117 ms (the owner, on mdview: "this is better"); closing is instant; the completion
  list never fades. One "Animations" switch in Settings › Appearance, on by default; off, or the
  system asking for less motion, makes everything instant (D-232 to D-235, DESIGN 96, notes in
  `docs/release-notes/NEXT.md`). Budgets inside the noise, exe +58 KB over `main`
  (`docs/measurements/2026-11-25-menu-fade.md`). Tried by the owner in mdview only; **not seen** over a frosted
  window, on Linux or macOS. The 117 ms is set in each app's `motion.rs`; it belongs in shared's
  `anim::document::MENU` at the next pin move (rule 8).
- The release workflow: the next release carries its full notes as `release-notes-<version>.md`,
  named by `hub.txt`'s `notes.file` (D-231; seventeen assets; not yet run on a real release).

## In hand

- **Motion, on `f/motion2`**: menus fade (released work), the toasts (D-251 to D-255,
  DESIGN 97), tooltips (M-C, D-257 and D-264), and the plan below. Not merged or tried by hand.
  The branch pins `1588e0f` (D-268), ready for L1's opt-in cascade cancellation.
  Left open: the status snap in
  `Motion::sync_toasts` (`Animator::set`), still needed with motion on because shared's animator
  keeps a target's first delay. **On the owner's word: quick, quiet motion; menus open in
  117 ms; menus and dialogs close instantly; everything under the one Animations setting, and
  motion off draws exactly as now.**
  - **M-D, Settings and tree chevrons**: done, `8ab68f1` with the review fixes at `46a7740`
    (DESIGN 98, D-259 corrected by D-263):
    `settings::pages::Form::draw_animated` in place of `draw`; tree chevrons through
    `glyph::draw_disclosure_animated_with` (motion from `motion.rs`); the groups `SIDEBAR` and
    `SETTINGS` keep their paint deadline on skips (D-263/D-267). Shots: `settings-switch-*`,
    `tree-chevron-*`. Pages and dialog closing stay instant.
  - **M-E, title row, X3 and the rest of the popups**: D4 (the title row) is done, `9b7551f`
    (DESIGN 98, D-261 and D-265): `chrome::draw_win_button_animated`, `draw_tab_animated`,
    `draw_new_tab_button_animated`, group `TITLE`, shots `title-hover-*`, `tab-hover-*` (shared
    #31's `t == 0` flash fix is in `09b01c1`). X3 is wired at `d116f9e` (D-262): menu-row
    hover fades, the command palette pops in, and the crash bar slides in just after launch with motion on.
    Notices present on the first paint, returning rename offers, and completion stay instant (D-266).
    Popup damage uses `presence_bounds()`. Windows launch: 121.40 ms and 134.4 ms exceeded
    the 100 ms budget; all later 31 at `0bacb93` were under (97.0 ms max). Idle CPU and
    transition frames weren't measured in that [repeat](measurements/2026-11-26-motion-launch-repeat.md). [Stale-tab launch](measurements/2026-11-26-motion-stale-launch.md): five extra full-height frames eliminated; slowest 102.0 ms after.
  - **M-F, four fixes**: L1, `submenu::Cascade` for the hand-made submenus and their dwell
    timer; L3, the plain preview's scrollbar from shared; W2, focus for the Files and Functions
    panes; W3, `window_state` restored onto a monitor that still exists.
  Next: M-F in order; then the requested Settings opening fade and light/dark crossfade. No
  page-change transitions.
- **The AI task window's flow fixes** (the UI run of 2026-11-25, twenty findings), on
  `f/task-fixes`, in Fable's second round.
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

1. The task window's fixes, then more motion.
2. The owner's look at 1.4.0's and 1.5.0's shots (R2, W, 4a, 4, 5.8, DD, audit 4, the notes
   cards) and whether to add the viewer's page rows for R2's tinted preview (D-108); then section 8's order. M1, M2 in
   mdview; D after R2.
3. The closed mdview, still to be tried (first: whether it is missed with mdview 1.0.5 too).

## Known to be wrong on `main`

- A debug-profile `cargo test` fails `closed_stderr` on the motion branch: ICU4X prints a data
  warning with `eprintln!` in debug builds only, and the test closes stderr. Release builds (what
  CI and CODE.md test) are fine. Not yet checked whether debug `main` fails it too.
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
- Worktrees: `main`, `hub-wt\app-prs2\mdedit`, `md-wt\ai-pictures`, `md-wt\ai-web` (spike exe); `md-wt\review-a4fix` holds the review, not a worktree.
