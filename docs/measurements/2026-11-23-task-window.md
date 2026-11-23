# The AI task window over files (phase W, `f/pw`) against R2

Measured 2026-11-23 on the Windows PC with `tools/compare-builds.py`, 7 interleaved runs, every
window `--behind` (`MDEDIT_ARGS=--behind`), `--no-session`, `MDEDIT_SETTINGS` naming a scratch
`settings.ini` (one file for all the runs). Absolute paths to both executables. A is
`f/review-hunks` at `9f298d4` (R2, the branch W sits on), built in a second worktree; B is `f/pw`
at `c2bfd22`.

**Verdict.** No change on the budgets. The task window is drawn only while it is open, and nothing
of it is on the launch path: no new dependency, no new startup work (`task_ui`, `task_draw` and
`app/task.rs` are reached from the command and the drop alone), and no timer. The window's own
files are read only when the command runs; nothing is read when the app opens.

```
Medians of 7 interleaved runs. A = mdedit.exe (release), B = mdedit.exe (release)

sample.rs
                             A         B      B-A   A range / B range
  first frame ms         85.60     86.20    +0.60   83.40-88.90 / 83.30-87.90
  idle private MiB       27.40     27.20    -0.20   27.20-27.70 / 27.00-27.50
  keystroke p50 ms        0.58      0.59    +0.01   0.56-0.70 / 0.57-0.60
  keystroke p95 ms        0.95      0.97    +0.02   0.91-1.27 / 0.95-1.16
  scroll p50 ms           0.44      0.44    +0.00   0.43-0.46 / 0.44-0.45
  scroll p95 ms           0.75      0.73    -0.02   0.72-0.79 / 0.70-0.78

sample.md
                             A         B      B-A   A range / B range
  first frame ms        111.90    109.90    -2.00   105.40-113.00 / 104.50-117.10
  idle private MiB       36.50     36.50    +0.00   36.00-36.80 / 36.10-36.80
  keystroke p50 ms        0.74      0.76    +0.02   0.73-0.75 / 0.75-0.77
  keystroke p95 ms        1.10      1.13    +0.03   1.08-1.11 / 1.10-1.16
  scroll p50 ms           0.57      0.58    +0.01   0.57-0.58 / 0.57-0.58
  scroll p95 ms           0.81      0.80    -0.01   0.78-0.91 / 0.76-0.83
```

The differences are inside the runs' own ranges, both ways. As in 1.3.0's and R2's measurements,
`sample.md` is over the first-frame and memory budgets in both builds by the same amount; this
change does not touch that, because the task window is not on the launch path.

**The executable**: 9,226,240 bytes on R2, 9,328,640 on the branch (+102,400, about +1.1%): the
window, its drawing, the three tasks and their tests. No new crate, and `Cargo.lock` moved only to
the shared crates' rev 38fa7b8 (the runs record's `Kind::Rewrite` and `Original`, D-076), which
adds no dependency.

**Not measured**: the window itself (its draw, its list, a request) — it has no budget of its own
in `docs/PLAN.md`, and it is not on any of the four paths measured here. The AI engine's own cost
is `mdedit-ai`'s and is not touched.
