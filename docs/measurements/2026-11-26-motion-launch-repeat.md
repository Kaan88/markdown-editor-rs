# Motion launch repeat — 2026-11-26

This repeats the [seven-run check](2026-11-26-motion-x3-md.md) after its 121.4 ms sample.
Windows release builds, `tools/compare-builds.py`, `fixtures/sample.rs` at 1280 × 800, fresh
scratch settings per process, `--no-session --behind --light`. Each comparison alternates the
build order for 31 rounds. The slow samples below come from the normal launch runs, not from
the typing or scroll benchmarks.

## X3 and M-D in isolation

| Build | Median, ms | Range, ms | Over 100 ms | Five slowest, ms |
|---|---:|---:|---:|---|
| `9b7551f` (before) | 88.5 | 81.9–113.8 | 4/31 | 113.8, 113.6, 113.0, 101.8, 98.8 |
| `5a13dc7` (X3 and M-D) | 91.0 | 80.1–134.4 | 8/31 | 134.4, 115.1, 111.6, 107.8, 106.2 |

The median of the paired differences was +0.9 ms (17 slower, 14 faster); the difference
between the two medians was +2.5 ms. The slow tail recurred. This batch alone does not tell
whether X3/M-D caused it. Private memory was 27.0 MiB on both builds; keystroke p50 was
0.59 ms on both; scroll p50 was 0.46 → 0.47 ms.

## Current build and first-frame stages

Another 31 alternating `9b7551f` / `0bacb93` rounds, with the saved `5a13dc7` binary also
launched after each pair as a cross-check (not order-balanced against the other two).

| Build | Median, ms | Range, ms | Over 100 ms | Five slowest, ms |
|---|---:|---:|---:|---|
| `9b7551f` | 82.6 | 76.8–124.8 | 1/31 | 124.8, 93.3, 93.3, 89.6, 88.1 |
| `5a13dc7` | 83.5 | 79.6–117.1 | 2/31 | 117.1, 102.0, 92.1, 91.0, 90.6 |
| `0bacb93` (tooltip and title review) | 83.0 | 78.0–97.0 | 0/31 | 97.0, 92.0, 90.8, 87.3, 86.3 |

| First-frame trace median, ms | `9b7551f` | `5a13dc7` | `0bacb93` |
|---|---:|---:|---:|
| Window created (elapsed) | 22.0 | 22.2 | 22.3 |
| First frame presented hidden (elapsed) | 76.1 | 77.0 | 75.8 |
| Window shown (elapsed) | 81.9 | 82.8 | 82.3 |
| Layout | 0.62 | 0.63 | 0.61 |
| Paint | 1.85 | 1.83 | 1.80 |
| Compose | 12.66 | 12.66 | 12.43 |

Current versus baseline: paired median +0.9 ms; idle private memory 27.1 → 27.0 MiB,
keystroke p50 0.55 → 0.54 ms, scroll p50 0.43 → 0.43 ms. The baseline's 124.8 ms sample
was already at 39.7 ms at window creation and 114.1 ms at hidden present; the intermediate
build's 117.1 ms sample reached those stages at 29.8 and 108.4 ms.

**Conclusion.** No repeatable shift in the ordinary first-frame rendering stages was found
from X3/M-D. On this path they add only frame bookkeeping: no new startup I/O, worker, or
active transition. Settings must be open and X3 needs a shown popup, palette, or notice
before those animations run. The slow tail is real on both sides, and this does not establish
a hard 100 ms bound. No cold launch, restored session with a notice, idle CPU, or 100 MB load
was measured. Raw samples and phases are in `target/sol-launch-repeat-5a13dc7.log` and
`target/sol-launch-repeat-0bacb93.log`.

The follow-up [stale-tab restore measurement](2026-11-26-motion-stale-launch.md) covers the
launch path this comparison omitted.
