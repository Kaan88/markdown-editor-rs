# State of the work

Rewritten, not appended to. Last rewritten 2026-11-28, after the releases of mdedit 1.6.2 and
mdview 1.2.2.

## Released

- **mdedit 1.6.2**, 2026-11-28, tag `v1.6.2` at `b02ff89`, release notes
  `docs/release-notes/1.6.2.md`, released with mdview 1.2.2 as a pair on the owner's word ("make a
  release"). `ci` was green on main, the release workflow passed, and it is the latest release on
  GitHub; the Windows exe's SHA-256 matches `mdedit-update-2.txt`. Toolkit `84c0fc8` (D-291). PRs
  #14 (a lost focus ends every drag; the bar's boxes scroll past their ends) and #15 (CLAUDE.md,
  using the toolkit), and the palette and plan screen moved to the toolkit's pointer routes:
  their text fields take the run of clicks, the drag past an end and an edit menu. Tests
  `app/toolkit_routes_tests.rs`; shots `palette-field-menu*` and `plan-split-rename-menu*`
  (`--field-menu`), whose Paste row follows the clipboard of the PC that takes them. `Cargo.lock`
  gained no package. The toolkit gives no bounds for a field's edit menu, so while the palette's
  is open its overlay bound is the whole window (toolkit ROADMAP L21).
- **mdedit 1.6.1**, 2026-11-27, tag `v1.6.1` at `472c53b`, release notes
  `docs/release-notes/1.6.1.md`, released with mdview 1.2.1 as a pair. Toolkit `48eedb8` (D-289): its
  crash, hang and hand-over fixes. `Cargo.lock` gained no package. A viewer from before the
  authenticated hand-over (mdview 1.2.0) is sent nothing, so the older-viewer toast says to update
  mdview, not to save; a saved file's link to such a viewer stays, as for any older viewer. On a PC
  whose app-data folder other accounts can write to, the toolkit does not trust an old viewer's
  port file, so mdview 1.2.0 reads as not running.
- **mdedit 1.6.0**, 2026-11-27, tag `v1.6.0` at `130f5de`, release notes
  `docs/release-notes/1.6.0.md`. `ci` was green on all four jobs on main and the release workflow
  passed. In it: window motion (D-232 to D-235, D-255 to
  D-283; DESIGN 96 to 98); the AI task window fixes (D-240 to D-254); the preview on the toolkit's
  renderer crates, with its security fixes and vertical-only font hinting (D-287, accepted by the
  owner). The hosted Mac self-update test is deferred and non-blocking (D-288).
- **mdedit 1.5.0**, 2026-11-25, tag `v1.5.0` at `51d8777`, sixteen assets. Record
  `docs/release-log.md`, notes `docs/release-notes/1.5.0.md`. The AI picture work of
  `docs/ai-pictures-plan.md` (F, E, L, O; C1, C2; V1 to V5 and the sent card; D-203 to D-230).
  **Not tried in a real window; no picture has been sent to a provider.** Copies are written on
  the UI thread (up to 200 MB).
- **mdview 1.2.2**, 2026-11-28, tag `v1.2.2` at `cf22524`, on toolkit `84c0fc8`.
  **mdview 1.2.1**, 2026-11-27, tag `v1.2.1` at `e4ae1e2`, on toolkit `48eedb8`.
  **mdview 1.2.0**, 2026-11-27, tag `v1.2.0` at `b2a2f7a`: the renderer switch to the toolkit's
  crates, with its security fixes. **mdview 1.1.0**, 2026-11-24, `0310e01`.
- The shared crates are not released; the apps pin them by commit. **Hub** is its own app (see
  its `docs/STATE.md`).

## 1.6.0 to 1.6.2: what is not verified

- Not run by hand in a real window: mdedit's motion (menus over a frosted window, Linux, macOS)
  and the AI task window fixes, which were seen in shots only. The owner tried mdview's menus only.
- No measurements on the merged 1.6.0 build. Before it, on Windows: launch median 78.0 ms,
  slowest 103.2 ms, 1 of 31 runs over 100 ms
  (`docs/measurements/2026-11-26-settings-theme-transitions.md`); idle private memory 27.0 MiB.
  Settings sampled 29.3 MiB at 100% scale, 34.6 at 125% and 41.0 at 150% (card only); the scaled
  runs were over 30 MiB before Settings opened. The owner accepted the overrun only during the
  170 ms theme fade (D-282).
- Linux launch and idle costs are unmeasured, and so is the Linux system-theme thread (D-273).
- 1.6.1: the real AppImage updater check (toolkit AI1), the hand-over between real 1.6.0 and
  1.6.1 windows, and measurements of the 1.6.1 build. The release packages were not run by hand.
- 1.6.2: by hand in a real window (the palette's and plan screen's new mouse handling and edit
  menus); the installer and packages by hand. Measured against 1.6.1 on the
  published exes (`docs/measurements/2026-11-28-1.6.2-against-1.6.1.md`): first frame and memory
  the same; the keystroke and scroll frame 0.05 to 0.12 ms slower (about 12 %, ranges apart),
  all of it in paint, far inside the budgets and not explained (no painting code changed). `sample.md` idles at 36 MiB in both builds.

## Waiting for the owner

- **AI library work**, `docs/ai-library-plan.md` section 8, in order after R2: V, drag and drop;
  W, 4a, 4 and 5 are merged. Open: the resizer for V; a new crate to be priced first (rule 7).
- **`docs/ai-web-plan.md`**: the AI searching the web, asked each run. Its five decisions.
- **Audit 4's questions**: C4 (D-183 replaces D-178's interim); A F8 (D-162, Waiting); W11 needs
  `jiff` (rule 7); the shared `%20` and `write_of` causes (D-180).
- **Dependabot**: open PRs (mdedit #9; mdview #2 to #6; shared #2 to #7), and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Not handled yet.
- The notes library is used in a real build and its two faults are fixed, but nothing records
  approval.

## In hand

- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.

## Next

1. The owner's look at mdedit 1.6.2 in a real window: the palette's and plan screen's edit menus
   (`palette-field-menu*`, `plan-split-rename-menu*` are the shots): menu and Settings fades, and the AI task
   window's flow, in both themes.
2. The owner's look at the 1.4.0 and 1.5.0 shots (R2, W, 4a, 4, 5.8, DD, audit 4, the notes cards),
   and whether to add the viewer's page rows for R2's tinted preview (D-108); then section 8's
   order.
3. The closed mdview, still to be tried (D-101).

## Known to be wrong on `main`

- **A closed mdview is still missed** (D-101): closing it by hand leaves "Stop the preview" in
  mdedit after its window has the focus again (tried with 1.0.4, which sends no `?closed`), so
  D-097's focus check does not hold. D-102 and D-105 change this (merged, not tried): three
  rechecks, a check kept to the end of a link's grace, and an ask at most once in two seconds.
  With 1.0.4 and a graceful close the old code found it in 1 s against the real exe, so the
  failing case was not reproduced; a tab closed in a viewer that keeps running cannot be told
  from 1.0.4 at all.
- A debug-profile `cargo test` failed `closed_stderr`: ICU4X prints a data warning with
  `eprintln!`, and the test closes stderr. Release builds (CI) were unaffected; debug has not been
  rechecked on the merged tree.
- **The 512-file limit.** Past 512 `.md` files in the library folder, those that sort last drop
  out; the foot says how many are shown but the rest cannot be listed. Flat notes count.
- Opening Settings during an AI review discards the result.
- **Deleting a library file orphans its notes.** It removes the list, never a note, so the folder
  keeps every note unlisted, and splitting that document again writes nothing (every note reads
  "already there" and comes unticked). The question now says the notes are kept on disk; deleting
  itself is unchanged.
- **The move**: a crash between the library rewrites and the digest update gives a half-undo;
  folders emptied in a home outside the library stay.
- The `runs*` shots draw the clock, so two runs of `tests/shots.sh` are not byte-identical.

## Not checked by hand

The Windows installer; the Linux and macOS packages by hand; the hosted Mac self-update test
(deferred, D-288); the real AppImage updater check; Linux blur and image paste; Hub reading a live
`hub.txt`.

## Known gaps, not planned unless they get in the way

A Linux trash entry gets no deletion date (D-092); a note missing on disk is marked at the next
re-read; the notes pane has no sort or filter; side-by-side has no scrollbar; no access keys in
the line-ending and encoding menus; no function list for Vue or Svelte; column selection pads
short lines on typing only; no compare arrows with wrap on; the Markdown menu needs more than
800 px. Split by AI has no progress row with Cancel and no headings per source; the Recycle Bin
cannot restore an original (D-071).

## Pins, branches, worktrees

- Shared crates and embedded preview: `84c0fc8` (D-291), the toolkit's main with `FieldInput`.
  On Linux, mdedit finds a co-installed viewer off `PATH` (D-286). winit `62fa727`,
  softbuffer `dd18fec`.
- Branches: `main`; `f/1.6.2` (merged into main at b02ff89, local only); `f/shared-48eedb8` (merged into main at 472c53b, local
  only); `f/renderer` (merged into main at 130f5de; still on origin); the unmerged
  `mock/*` drawings; `archive/doc-reorg-draft`, `spike/v-resizer`; GitHub's `spike/frosted-window`
  and `chore/dependabot`. Kept as found, unpushed: shared `archive/settings-draft-uncommitted`,
  Hub's `wip/install-uncommitted`.
- Worktrees: `main`; `md-wt\renderer` (`f/1.6.2`, merged);
  `hub-wt\app-prs2\mdedit` (hub/notes-file); fifteen detached worker worktrees under
  `%TEMP%\opencode\`, left from earlier sessions and not removed.
