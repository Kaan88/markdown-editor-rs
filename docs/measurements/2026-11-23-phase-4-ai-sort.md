# Phase 4 (sort a folder by AI) against its base, before merging it

Measured 2026-11-23 on the Windows PC with `tools/compare-builds.py`, 3 interleaved runs, every
window `--behind`, a scratch settings file (`MDEDIT_SETTINGS`), `--no-session`. A is the base the
branch was cut from, `main` at `4c95da0` (worktree `md-wt\p4-mainbase`); B is `f/p4`, which is
that base with phase 4's two requests, the scheme and the settled plan.

**Verdict.** No change. The AI step adds a command, two stages of the import card, a settings row
and code that runs only when **Ask the AI** or **Sort N files** is pressed -- nothing on the launch
path, no thread at start, no new dependency -- and the figures agree: every delta is inside the
two builds' own ranges. `sample.rs` is over the first-frame budget in both builds already, by the
same amount (PLAN.md's 100 ms; these builds sit at 85-87 ms).

**The exe** grows from 9,353,728 to 9,392,640 bytes (+38,912, +0.42 %), which is the prompts, the
answer reader and checks, the scheme's editing and the card's drawing; it is smaller than phase
4a's +97 KB because the card and the walk were already there.

```
Medians of 3 interleaved runs. A = mdedit.exe (release), B = mdedit.exe (release)

sample.rs
                             A         B      B-A   A range / B range
  first frame ms         87.00     85.10    -1.90   85.30-91.70 / 83.20-111.40
  idle private MiB       27.00     26.80    -0.20   26.80-27.20 / 26.70-27.10
  keystroke p50 ms        0.56      0.54    -0.02   0.52-0.59 / 0.53-0.54
  keystroke p95 ms        0.90      0.77    -0.13   0.74-0.99 / 0.75-0.79
  scroll p50 ms           0.43      0.43    +0.00   0.43-0.45 / 0.43-0.44
  scroll p95 ms           0.61      0.67    +0.06   0.57-0.66 / 0.61-0.69
```

**Not measured, and why.** What the card costs while it is open: typing a scheme name and
scrolling its list are driven by neither `--type-steps` nor `--scroll-steps`, so no figure exists
for them; the request itself is the engine's, not the editor's, and no request was made to a real
provider in these runs. The 3-run set was used because this change has no launch-path code; a 7-run
set as phase 4a's would tighten the ranges, not change the verdict.
