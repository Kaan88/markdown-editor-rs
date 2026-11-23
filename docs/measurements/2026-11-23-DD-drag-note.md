# Phase DD (dragging a note) against `main` `9d08066`, before the merge

Measured 2026-11-23 on the Windows PC with a copy of `tools/compare-builds.py`'s method: two
release builds interleaved, 7 runs, each window `--behind`, `--no-session`, a fresh scratch
settings folder per start (`MDEDIT_SETTINGS`), `fixtures/sample.rs`. A is `origin/main` `9d08066`
(`deps: Bump regex-automata`), B is `f/pdd` over it (the drag of a note in the notes pane).

**Verdict.** No change. The change adds nothing to the launch path (the library is read on first
use, never at start, and the pane is closed by default) and nothing per frame outside the notes
pane; every figure is within the noise of the other.

```
Medians of 7 interleaved runs. A = mdedit-baseline.exe, B = mdedit.exe

sample.rs
                             A         B      B-A   A range / B range
  first frame ms        431.60    431.40    -0.20   401.60-572.50 / 401.20-573.20
  idle private MiB       26.70     26.80    +0.10   26.30-27.10 / 26.30-26.90
  keystroke p50 ms        0.97      0.94    -0.03   0.91-1.11 / 0.90-1.12
  keystroke p95 ms        1.39      1.22    -0.17   1.09-1.85 / 1.09-1.88
  scroll p50 ms           0.79      0.71    -0.08   0.72-0.94 / 0.69-0.91
  scroll p95 ms           1.18      1.06    -0.12   0.92-1.54 / 0.89-1.48
```

**What is not comparable.** The first frame is 431 ms for both builds here, against 94 ms
measured for 1.3.0 on the same PC earlier the same day
(`2026-11-23-1.3.0-against-1.2.1.md`): several builds of other work ran on this PC throughout
this session, so the absolute figure is the machine's load, not the build's. The two builds are
equal to 0.2 ms, which is what this change can be asked to show.

**Not measured.** The 100 MB document and `sample.md`; the pane's own cost while it shows (the
change draws two extra shapes only during a drag, and the drag is not on the launch or frame
path). `--library-demo` renders the pane without reading anything.
