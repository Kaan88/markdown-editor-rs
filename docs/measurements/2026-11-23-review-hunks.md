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
