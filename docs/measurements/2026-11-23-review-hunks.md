# The AI review with ticks (phase R2, `f/review-hunks`) against `main`

Measured 2026-11-23 on the Windows PC with `tools/compare-builds.py`, 7 interleaved runs, every
window `--behind` (`MDEDIT_ARGS=--behind`), `--no-session`, `MDEDIT_SETTINGS` naming a scratch
`settings.ini` (one file for all the runs, not a fresh one per run: the tool does not make one).
Absolute paths to both executables. A is `main` at `32d6fe6`, built in a second worktree; B is
`f/review-hunks` at `7467711` (the code; the commits after it are documents and one test).

**Verdict.** No change on the budgets. The review is drawn only while it is open, and nothing of
it is on the launch path: no new dependency, no new startup work (`hunks_ui` and `hunks` are
called from the review's code alone), no timer unless a review with an open Preview has a tick not
yet shown (`ai_review_due` returns at once when there is no review).

```
Medians of 7 interleaved runs. A = mdedit.exe (release), B = mdedit.exe (release)

sample.rs
                             A         B      B-A   A range / B range
  first frame ms         84.80     86.80    +2.00   82.00-103.50 / 80.10-94.00
  idle private MiB       27.30     27.40    +0.10   27.10-27.40 / 27.00-27.60
  keystroke p50 ms        0.54      0.51    -0.03   0.51-0.56 / 0.49-0.57
  keystroke p95 ms        0.80      0.66    -0.14   0.64-1.10 / 0.56-0.81
  scroll p50 ms           0.42      0.42    +0.00   0.41-0.45 / 0.41-0.43
  scroll p95 ms           0.54      0.56    +0.02   0.49-0.76 / 0.49-0.66

sample.md
                             A         B      B-A   A range / B range
  first frame ms        124.00    120.00    -4.00   113.40-163.20 / 112.80-129.90
  idle private MiB       36.60     36.50    -0.10   36.00-37.00 / 36.00-37.00
  keystroke p50 ms        0.69      0.70    +0.01   0.67-0.85 / 0.68-0.73
  keystroke p95 ms        0.92      0.94    +0.02   0.79-1.55 / 0.85-1.01
  scroll p50 ms           0.56      0.56    +0.00   0.55-0.61 / 0.55-0.58
  scroll p95 ms           0.68      0.67    -0.01   0.60-1.16 / 0.60-0.84
```

The differences are inside the runs' own ranges, both ways. As in 1.3.0's measurement,
`sample.md` is over the first-frame and memory budgets in both builds by the same amount; this
change does not touch that.

**The executable**: 9,201,152 bytes on `main`, 9,225,728 to 9,226,240 on the branch (+25 KB,
+0.27%): the list, the composition and the layout code, less the old flattening.

**Not measured.** The time to draw the review itself (a long list, a wrapped list, a drag of
its scrollbar) and to make its layout; Linux and macOS; a 100 MB file; the memory of an open
review with its Preview panes. The layout is made once per width and scale (not per tick or per
frame) and measures text in windows of at most 512 characters, which is why a long result should
not cost a frame, but that is argued from the code here, not timed.

## Second measurement, after the last commits

Measured again 2026-11-23, 5 interleaved runs, after the merge of `origin/main` (`6b50a29`) and the
last commits of the branch (the `review-closed` demo, one test, documents). Same method and the
same `--behind`, `--no-session`, scratch `MDEDIT_SETTINGS`; A is `main` at `32d6fe6`
(`Desktop\md-wt\baseline-main`), B is `f/review-hunks` at `939d30e`. Five runs, not seven, for
time; the figures are the medians.

```
sample.rs
                             A         B      B-A   A range / B range
  first frame ms         89.80     84.00    -5.80   87.20-100.90 / 83.10-94.50
  idle private MiB       27.10     27.50    +0.40   27.00-27.50 / 26.90-27.60
  keystroke p50 ms        0.55      0.59    +0.04   0.52-0.77 / 0.58-0.68
  keystroke p95 ms        0.84      0.76    -0.08   0.64-1.49 / 0.73-1.31
  scroll p50 ms           0.43      0.48    +0.05   0.42-0.50 / 0.48-0.52
  scroll p95 ms           0.56      0.62    +0.06   0.55-0.91 / 0.60-0.79

sample.md
                             A         B      B-A   A range / B range
  first frame ms        114.40    114.00    -0.40   106.30-116.70 / 106.50-121.10
  idle private MiB       36.60     36.40    -0.20   36.50-36.70 / 36.10-36.60
  keystroke p50 ms        0.77      0.78    +0.01   0.69-0.93 / 0.77-0.82
  keystroke p95 ms        1.12      1.02    -0.10   1.07-1.34 / 0.99-1.18
  scroll p50 ms           0.59      0.63    +0.04   0.57-0.59 / 0.62-0.65
  scroll p95 ms           0.75      0.74    -0.01   0.70-0.81 / 0.71-0.81
```

Every difference is inside the two builds' own ranges and both ways. The executable of B is
9,220,096 bytes, +18,944 bytes on A (+0.21%). The second measurement agrees with the first: this
change is not on the launch path and moves none of the budgets in a way the runs can tell apart.
Still not measured: the time to draw the review itself and to make its layout; Linux and macOS;
a 100 MB result; the memory of an open review with its Preview panes.
