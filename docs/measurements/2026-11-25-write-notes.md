# Write notes (`f/ai-workflow`, phases V3 to V5 and the sent card) against `main`

Measured 2026-11-25 on the Windows PC with `tools/compare-builds.py`, 7 interleaved runs, every
window `--behind` (`MDEDIT_ARGS=--behind`), `--no-session`, `MDEDIT_SETTINGS` naming a scratch
`settings.ini` (one file for all the runs). A is `main` at `37cd0b9`, built in a second worktree;
B is `f/ai-workflow` at `2eeb36a` (V4, V5 and the sent card merged). The fixes after it touch no
launch or idle path.

**Verdict.** No change on the budgets. Nothing of Write notes is on the launch path: no new
dependency, no startup work, no timer. Pictures are measured when added to the task window, the
cards are built when shown, and the write runs when asked. The exe is 10,033,664 bytes against
9,583,616 (+439 KB): Write notes, its two cards and its write.

```
Medians of 7 interleaved runs. A = mdedit-before.exe (Temp), B = mdedit-after.exe (Temp)

sample.rs
                             A         B      B-A   A range / B range
  first frame ms         87.80     86.60    -1.20   84.50-90.10 / 84.80-97.70
  idle private MiB       27.00     27.10    +0.10   26.90-27.20 / 26.90-27.20
  keystroke p50 ms        0.52      0.52    +0.00   0.51-0.53 / 0.51-0.58
  keystroke p95 ms        0.80      0.74    -0.06   0.67-0.89 / 0.64-0.99
  scroll p50 ms           0.42      0.42    +0.00   0.42-0.42 / 0.42-0.43
  scroll p95 ms           0.54      0.61    +0.07   0.53-0.70 / 0.53-0.69
```
