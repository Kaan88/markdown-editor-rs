# The AI workflow's first milestone (`f/ai-workflow`) against `main`

Measured 2026-11-25 on the Windows PC with `tools/compare-builds.py`, 7 interleaved runs, every
window `--behind` (`MDEDIT_ARGS=--behind`), `--no-session`, `MDEDIT_SETTINGS` naming a scratch
`settings.ini` (one file for all the runs). Absolute paths to both executables. A is `main` at
`37cd0b9`, built in a second worktree; B is `f/ai-workflow` at `04f27d5` (phases F, E, L and O of
`docs/ai-pictures-plan.md`, and the toolbar and WHERE IT GOES fixes).

**Verdict.** No change on the budgets. Nothing of this work is on the launch path: no new
dependency, no new startup work, no timer. The pictures guard (F) runs when a file is opened, the
one AI task command (E) and the library row's line (L) are a menu's items, and what opens after a
run and the task limits (O) are read when a run ends or the window opens. The four settings are
four more lines of `settings.ini`, parsed with the rest.

```
Medians of 7 interleaved runs. A = mdedit.exe (release), B = mdedit.exe (release)

sample.rs
                             A         B      B-A   A range / B range
  first frame ms         88.30     92.10    +3.80   86.50-105.70 / 88.40-122.40
  idle private MiB       26.90     26.70    -0.20   26.70-27.20 / 26.50-27.20
  keystroke p50 ms        0.53      0.59    +0.06   0.53-0.57 / 0.57-0.59
  keystroke p95 ms        0.76      0.92    +0.16   0.72-1.10 / 0.75-1.01
  scroll p50 ms           0.43      0.48    +0.05   0.43-0.47 / 0.46-0.49
  scroll p95 ms           0.58      0.67    +0.09   0.54-0.87 / 0.56-0.77

sample.md
                             A         B      B-A   A range / B range
  first frame ms        122.50    117.40    -5.10   117.40-124.20 / 110.40-130.50
  idle private MiB       34.50     34.40    -0.10   34.10-35.40 / 34.20-34.80
  keystroke p50 ms        0.71      0.75    +0.04   0.69-0.79 / 0.75-0.78
  keystroke p95 ms        0.97      1.04    +0.07   0.93-1.42 / 0.96-1.20
  scroll p50 ms           0.56      0.61    +0.05   0.56-0.61 / 0.61-0.63
  scroll p95 ms           0.74      0.77    +0.03   0.64-1.11 / 0.70-0.95
```

The first-frame differences go both ways (+3.8 ms on `sample.rs`, -5.1 ms on `sample.md`) and are
inside the runs' own ranges. The keystroke and scroll medians are a few hundredths of a
millisecond apart, far under the 8 ms and 4 ms budgets; the keystroke and scroll paths are not
touched by this work. As in the earlier measurements, `sample.md` is over the first-frame and
memory budgets in both builds by the same amount; this change does not touch that.

**The executable**: 9,588,736 bytes on `main`, 9,626,624 on the branch (+37,888, about +0.4%):
the pictures guard, the library row's line, the AI tasks settings page and their tests' support.
No new crate, and `Cargo.lock` moved only to the shared crates' rev `50e3401`, which adds no
dependency. `mdedit-ai.exe` is 2,270,208 bytes on both.

**Not measured**: the task window and the Notes pane's menu themselves — they have no budget of
their own in `docs/PLAN.md` and are not on the four paths measured here.
