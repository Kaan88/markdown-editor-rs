# The budgets since phase 1, measured 2026-11-21

What `CLAUDE.md` rule 1 asks for, paid: the phase-1 measurement (`d4e3ad6`, 2026-11-21 16:05)
against `main` at `d3b1391` (2026-11-21 20:31), which holds phase 2 and the library writers
(`3453a7a`), phase 3 (`8522c82`), phase 0 with the leak fix and the extra unfiltered read
(`f00ee60`), phase 6a (`487e184`), the group folder on each row (`e4c59aa`), the shared-widget
adoption (`5688e6a`), the split model (`6f9eb6b`) and the plan screen (`d3b1391`). Both release
builds, run in turn, 8 runs per configuration. Windows only.

**Read the first-frame column with the second section in view: this PC is roughly four times
slower tonight than it was this afternoon, for every build on it, including the published one.**

## The baseline, quoted from `docs/archive/HANDOFF-2026-11-13-to-2026-11-21.md`

Release builds of `1303755` and `d4e3ad6`, run in turn, `--trace --exit-after-ms 2500` on
`fixtures/sample.rs`, a fresh settings folder per run, 7 runs each; medians, min-max:

| | First frame, ms | Private, MiB | CPU at idle, ms |
|---|---|---|---|
| Before, no pane | 91.2 (88.4-95) | 27.7 (27.4-28.1) | 109.4 |
| After, no pane | 89.7 (87.9-95) | 27.9 (27.7-28) | 109.4 |
| After, Files face restored | 91.2 (81.6-107) | 27.6 (27.4-27.9) | 109.4 |
| After, Functions face restored | 90.7 (88.7-96.2) | 27.9 (27.6-28.1) | 109.4 |
| After, Notes face restored | 90.0 (86.5-98) | 27.9 (27.7-28) | 109.4 |

## Today, A = `d4e3ad6` (phase 1) against B = `d3b1391` (`main`)

8 runs per configuration per build; medians, min-max in brackets. "Notes" means a session with
`files=1`, `pane=notes`, so the pane's face is restored and the library is read on its first
tick. The library sizes are below.

| | A first frame, ms | B first frame, ms | A private, MiB | B private, MiB | A CPU at idle, ms | B CPU at idle, ms |
|---|---|---|---|---|---|---|
| No pane, `--no-session` | 417.6 (366.7-564.3) | 420.8 (383.3-894.4) | 27.5 (27.3-27.8) | 27.5 (27.3-27.7) | 140.6 | 117.2 |
| No pane, session restored | 408.6 (397.9-434.4) | 410.7 (389.3-474.7) | 27.7 (27.4-28.0) | 27.5 (27.4-27.8) | 125.0 | 132.8 |
| Notes, empty library | 419.7 (393.3-582.1) | 411.6 (400.0-445.8) | 27.6 (27.4-27.8) | 27.7 (27.5-27.8) | 140.6 | 140.6 |
| Notes, 8 group files, 200 notes | 418.7 (391.4-494.4) | 443.9 (389.1-470.8) | 27.8 (27.6-28.1) | 27.9 (27.7-28.2) | 164.1 | 125.0 |
| Notes, 70 group files, 2000 notes | 412.8 (401.3-467.0) | 435.6 (403.8-1080.1) | **29.4** (29.1-29.6) | **30.4** (30.1-30.8) | 203.1 | 234.4 |

"The library" here is 64 group files and 1,832 notes as the pane sees them: the folder holds 70
files but `MAX_FILES` reads 64, a known limit (`docs/STATE.md`, "The 64-file limit"). The 200-note
library is 8 files and all 200 notes. Notes live in a folder per group, one deeper per section,
as phase 6a writes them.

Keystroke and scroll, `--type-steps 300` and `--scroll-steps 300`, 8 runs each, medians
(min-max):

| | A keystroke p50 / p95, ms | B keystroke p50 / p95, ms | A scroll p50 / p95, ms | B scroll p50 / p95, ms |
|---|---|---|---|---|
| No pane | 0.97 (0.70-1.38) / 1.41 (0.93-1.80) | 1.10 (0.78-1.36) / 1.48 (1.16-2.14) | 0.71 (0.58-1.08) / 1.14 (0.76-1.72) | 0.95 (0.54-1.15) / 1.35 (0.85-1.81) |
| Notes, 200 notes | 0.87 (0.68-1.31) / 1.34 (0.99-1.69) | 0.83 (0.56-1.36) / 1.18 (0.76-1.83) | 0.62 (0.50-1.06) / 0.90 (0.69-1.59) | 0.59 (0.44-1.09) / 0.97 (0.66-1.51) |
| Notes, 2,000 notes | 0.89 (0.71-1.41) / 1.30 (1.01-1.89) | 0.87 (0.74-1.31) / 1.45 (1.02-1.60) | 0.73 (0.56-1.13) / 1.14 (0.85-1.46) | 0.62 (0.49-1.13) / 0.88 (0.71-1.65) |

Both figures are about twice the 0.56 and 0.44 ms the PLAN records for 1.2.1 on this PC, which is
the machine again (the same slowdown that shows in the first frame); A and B agree with each
other in every row, which is what the before-against-after test reads.

## `library_tick`, measured directly

A small standalone crate (not in any repository) depending on `mdshared-app` at the rev each
build pins — `c603b23` for `main`, `fada09b` for phase 1 — calling `Library::changed()` 300 times
and `library::read()` 20 times per library, release, file cache warm:

| | 8 files, 200 notes | 64 files, 1,832 notes |
|---|---|---|
| `changed()` — one tick, nothing changed: p50 (p95, max) | 0.281 (0.315, 0.561) ms | **2.250 (2.665, 3.299) ms** |
| `read()` — a full re-read: p50 (p95) | 4.126 (4.551) ms | **38.845 (42.954) ms** |
| `changed()` at phase 1's shared rev | 0.281 ms | 2.202 ms |
| `read()` at phase 1's shared rev | 4.053 ms | 38.815 ms |

The same cost in the running window, from the trace of the startup runs: the first frame to
"first colours", which is where the first tick's library read and the pane's first paint land
(median of 8, min-max) —

| | A | B |
|---|---|---|
| No pane | 28.8 (27.3-30.1) ms | 29.2 (27.9-116.4) ms |
| Notes, empty library | 29.4 (27.7-30.1) ms | 29.3 (28.2-32.8) ms |
| Notes, 200 notes | 41.0 (35.7-53.0) ms | 37.5 (35.3-48.7) ms |
| Notes, 2,000 notes | 112.7 (90.6-134.5) ms | 107.8 (98.1-173.8) ms |

and the CPU the process spends after it has settled, over a 6-second run (median of 8,
min-max), `--exit-after-ms 6000`:

| | A | B |
|---|---|---|
| No pane | 31.2 (15.6-62.5) ms | 31.3 (15.6-62.5) ms |
| Notes, empty library | 31.3 (15.6-62.4) ms | 39.1 (15.6-78.1) ms |
| Notes, 200 notes | 39.1 (15.6-62.5) ms | 31.2 (15.6-93.7) ms |
| Notes, 2,000 notes | 62.5 (46.8-125.0) ms | 54.7 (46.8-140.6) ms |

## One line per budget

- **First frame under 100 ms — not verified today, no change before against after.** Every
  build on this PC reads about 410 to 440 ms tonight (medians above). The installed 1.2.1
  release reads 382.9, 384.7 and 390.1 ms in three runs, and the `d4e3ad6` exe that measured
  89.7-91.2 ms at 16:12 today reads 390.9, 392.5 and 406.8 ms now. So the number is the machine,
  not the code; A and B are within the ranges of each other in every configuration (largest
  median gap 25 ms, ranges all overlapping).
- **Private memory under 30 MiB — met without the pane and with a realistic library (27.5 and
  27.9 MiB); not met with the 2,000-note library restored: 30.4 MiB (30.1-30.8) on `main`.**
  Phase 1 held the same library at 29.4 MiB (29.1-29.6), so this is +1.05 MiB from the work
  since, and it is the phase-0 extra unfiltered read (`f00ee60`) that pays it: `docs.library_read`
  keeps the whole read beside the filtered `docs.library`, and the second copy grows with the
  number of notes.
- **Keystroke under 8 ms — met.** p50 0.83 to 1.10 ms, p95 1.18 to 1.48 ms, single worst run
  2.14 ms, in all three configurations and both builds. The library, open and 2,000 notes, does
  not change it.
- **Scroll frame under 4 ms — met.** p50 0.59 to 0.95 ms, p95 0.88 to 1.35 ms, single worst run
  1.81 ms, all configurations.
- **Idle CPU zero — met with no pane or a small library; not met while the Notes face shows a
  large library.** Nothing ticks in the other faces. With 2,000 notes the pane checks the folder
  once every 800 ms or so (in practice on every caret blink), and that check costs 2.25 ms: a
  directory listing plus two stats per group file, ~0.4 % of one core while the window is
  focused. It is not a timer of its own: `library_tick` runs from `about_to_wait`, so with
  nothing else waking the loop the library is not checked at all.
- **Nothing new on the launch path — met for the first frame, at a price after it.** The first
  frame does not read the library: the read is on the first tick, after the frame is presented.
  What it costs there is 108 to 113 ms of delay before "first colours" on the 2,000-note library
  (29 ms without a pane), and 37 to 41 ms on the 200-note one. `mdedit_core::split` is reachable
  only from `plan_ui`/`plan_demo`/`plan_draw`; nothing calls it at start (the plan screen opens
  from `--plan-demo` or from `App::plan_open_active`, which the command table's "Split into
  notes…", with no shortcut, is the only caller of), so the split model adds code and no startup
  work.

The exe grew from 8,454,144 to 8,659,456 bytes (+205,312, +2.4 %) for everything above. Not a
gate; recorded because the jump is larger than a rounding.

`main` moved on again while this was being written (another agent's plan-screen work); the
commits after `d3b1391` are not in these numbers.

## What was not measured, and why

- **macOS and Linux.** No desktop of either here; no budget on either has ever been measured.
- **The 100 MB file** (first frame under 500 ms, memory about the file's size plus 25 %). Not in
  the list of what landed, and this machine's I/O is the thing that is slow tonight.
- **The row rebuild after a library file changes.** The re-read is measured (38.8 ms on the
  2,000-note library) and it happens on the UI thread inside the tick; the 1,832-row rebuild and
  repaint that follow it were not isolated from the rest of the first tick's ~70 ms of work.
- **Keystroke and scroll with an empty library.** The typing phase had no pane, 200 notes and
  2,000 notes; the empty-library pane is covered by its startup row only.
- **A long idle with the window unfocused.** The tick only runs when something else wakes the
  loop, and the script could not hold the window unfocused and still guarantee wakes, so the
  "no CPU at all when nothing happens" case rests on reading the code, not on a run.
- **The published 1.2.1 against `main` today.** It was run three times only, as a probe to show
  the machine's state (382.9-390.1 ms).
- **Windows' own noise.** The ranges contain multi-hundred-millisecond first-frame outliers
  (up to 1,455 ms) that are the machine stalling, not the app; medians are reported with the
  ranges so they are visible.

## Method, so it can be repeated

- Worktrees `C:\Users\kaank\Desktop\md-wt\a` at `d4e3ad6` and `b` at `d3b1391`, both
  `cargo build --release -p mdedit`, measured, then removed.
- `data\run-measure.ps1` (PowerShell): per run a fresh folder under
  `%TEMP%\opencode\meas\s`, `MDEDIT_SETTINGS` naming its `settings.ini`, seeded with
  `[mdedit]` / `files=1` / `pane=notes` / `check_updates=0` and a directory junction `library`
  into the scratch library. The junction is deliberate: phase 1 has no `library_dir`
  preference (it came in phase 2), so the folder must sit beside the settings file for both
  builds to see the same library. An earlier attempt that named the library with `library_dir`
  was thrown away: build A silently read an empty library, and its numbers would have compared a
  pane with nothing in it against B's full one.
- Starts: `mdedit.exe --trace --size 1280x800 --exit-after-ms 2500 fixtures/sample.rs` (startup);
  `--exit-after-ms 6000` (idle CPU); `--type-steps 300` and `--scroll-steps 300` (keystroke,
  scroll). `--no-session` for the control only; the rest restore the seeded session.
- Order: for run *n*, A then B when *n* is odd, B then A when even; every configuration uses the
  same run index, so the two builds are never measured one after the other in blocks.
- Every Notes run was checked before its numbers were used: the session it writes back still
  says `files=1` and `pane=notes` (8 of 8 in every configuration), memory rises with the library
  (27.5 to 29.4/30.4 MiB), and the first tick's work rises with it (29 to 108/113 ms). The
  script throws if the pane was not restored.
- Machine: Windows, i7-14700KF, 28 logical processors, 34 GB RAM, Hyper-V present, Balanced
  power plan, real-time antivirus off, about 5 % load during the runs. A
  `qemu-system-x86_64-headless` process held one thread at about 34 % of a core throughout.
- 240 measured starts in the reported set; the tick bench is the crate under `md-wt\bench-main`
  and `md-wt\bench-p1`, results in `md-wt\data\results.csv` (scratch; the numbers are above).
- The 200-note and 2,000-note libraries are generated by `md-wt\data\make-libs.ps1`.

## What is surprising

1. **The tick does not re-read.** With nothing changed it costs 0.28 ms (200 notes) or 2.25 ms
   (2,000 notes) of `read_dir` and stats; it reads a file only when a stamp has moved. The
   assumption that it re-reads every 800 ms is not what the code does — but when something does
   change, the re-read is 38.8 ms in the event loop, which is ten scroll frames.
2. **The memory budget is crossed by the 2,000-note library, on `main` only**, by 1.05 MiB that
   came in with the unfiltered read. Nothing else moved.
3. **The tick is not on a timer.** It is checked whenever the loop happens to wake, so an
   unfocused window with the notes pane open does not notice a library file changing at all.
4. **The absolute first-frame figure on this PC can move fourfold with no code change**
   (`~90 → ~390 ms` between 16:12 and 20:00 today), which is why this document compares A with
   B and not with the phase-1 table's absolute numbers.
