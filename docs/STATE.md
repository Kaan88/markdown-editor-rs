# State of the work

Rewritten, not appended to. Last rewritten 2026-11-27, after the releases of mdedit 1.6.0 and
mdview 1.2.0.

## Released

- **mdedit 1.6.0**, 2026-11-27, tag `v1.6.0` at `130f5de`, release notes
  `docs/release-notes/1.6.0.md`. `ci` was green on all four jobs on main, the release workflow
  passed, and it is the latest release on GitHub. In it: window motion (D-232 to D-235, D-255 to
  D-283; DESIGN 96 to 98); the AI task window fixes (D-240 to D-254); the preview on the toolkit's
  renderer crates, with its security fixes and vertical-only font hinting (D-287, accepted by the
  owner). The hosted Mac self-update test is deferred and non-blocking (D-288).
- **mdedit 1.5.0**, 2026-11-25, tag `v1.5.0` at `51d8777`, sixteen assets. Record
  `docs/release-log.md`, notes `docs/release-notes/1.5.0.md`. The AI picture work of
  `docs/ai-pictures-plan.md` (F, E, L, O; C1, C2; V1 to V5 and the sent card; D-203 to D-230).
  **Not tried in a real window; no picture has been sent to a provider.** Copies are written on
  the UI thread (up to 200 MB).
- **mdview 1.2.0**, 2026-11-27, tag `v1.2.0` at `b2a2f7a`: the renderer switch to the toolkit's
  crates, with its security fixes. **mdview 1.1.0**, 2026-11-24, `0310e01`.
- The shared crates are not released; the apps pin them by commit. **Hub** is its own app (see
  its `docs/STATE.md`).

## 1.6.0: what is not verified

- Not run by hand in a real window: mdedit's motion (menus over a frosted window, Linux, macOS)
  and the AI task window fixes, which were seen in shots only. The owner tried mdview's menus only.
- No measurements on the merged 1.6.0 build. Before it, on Windows: launch median 78.0 ms,
  slowest 103.2 ms, 1 of 31 runs over 100 ms
  (`docs/measurements/2026-11-26-settings-theme-transitions.md`); idle private memory 27.0 MiB.
  Settings sampled 29.3 MiB at 100% scale, 34.6 at 125% and 41.0 at 150% (card only); the scaled
  runs were over 30 MiB before Settings opened. The owner accepted the overrun only during the
  170 ms theme fade (D-282).
- Linux launch and idle costs are unmeasured, and so is the Linux system-theme thread (D-273).

## 1.6.1, being released

- **mdedit 1.6.1** on toolkit `48eedb8` (D-289), with mdview 1.2.1, on the owner's word
  (2026-11-27); notes `docs/release-notes/1.6.1.md`. `Cargo.lock` gains no package. Besides the
  pin: a viewer from before the authenticated hand-over (mdview 1.2.0) is sent nothing, so the
  older-viewer toast now says to update mdview, not to save; a saved file's link to such a viewer
  stays, as for any older viewer. The AppImage's `AppRun` links to `usr/bin/mdedit`, as the
  toolkit's Linux self-update requires. On a PC whose app-data folder other accounts can write
  to (this one), the toolkit does not trust an old viewer's port file, so mdview 1.2.0 reads as
  not running. Not verified: the real AppImage updater check (toolkit AI1), the hand-over between
  real 1.6.0 and 1.6.1 windows, and measurements of the 1.6.1 build.

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

1. The owner's decision on the toolkit pin move and the patch releases (above). Nothing changes
   until then.
2. The owner's look at mdedit 1.6.0 in a real window: menu and Settings fades, and the AI task
   window's flow, in both themes.
3. The owner's look at the 1.4.0 and 1.5.0 shots (R2, W, 4a, 4, 5.8, DD, audit 4, the notes cards),
   and whether to add the viewer's page rows for R2's tinted preview (D-108); then section 8's
   order.
4. The closed mdview, still to be tried (D-101).

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

- Shared crates and embedded preview: `48eedb8` (D-289), the toolkit's main after its review.
  On Linux, mdedit finds a co-installed viewer off `PATH` (D-286). winit `62fa727`,
  softbuffer `dd18fec`.
- Branches: `main`; `f/shared-48eedb8` (1.6.1); `f/renderer` (merged into main at 130f5de; still on origin); the unmerged
  `mock/*` drawings; `archive/doc-reorg-draft`, `spike/v-resizer`; GitHub's `spike/frosted-window`
  and `chore/dependabot`. Kept as found, unpushed: shared `archive/settings-draft-uncommitted`,
  Hub's `wip/install-uncommitted`.
- Worktrees: `main`; `md-wt\renderer` (`f/shared-48eedb8`, 1.6.1);
  `hub-wt\app-prs2\mdedit` (hub/notes-file); fifteen detached worker worktrees under
  `%TEMP%\opencode\`, left from earlier sessions and not removed.
