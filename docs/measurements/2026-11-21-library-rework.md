# The notes pane on a large library, measured 2026-11-21

The three faults in `2026-11-21-since-phase-1.md` were fixed together: the shared pin moved to
`fb7777c` (a 512-file cap, with `Library::omitted`; paths compared part by part), the notes pane
reads a changed library on a worker thread (`mdshared_app::job`), and the second whole `Library`
kept only to answer "has anything changed" is now a slim copy of the file list and stamps.
Windows only; release builds; this PC, the same evening.

## The builds

- **Before**: `a9a694e` with the shared pin at `c603b23` (64 files).
- **After**: the same tree with the pin at `fb7777c` and the rework (512 files).

Both were built with `cargo build --release -p mdedit`; the two exes are 8,837,120 and 8,843,776
bytes.

## Private memory

The method of the first document: a fresh folder under `%TEMP%\opencode\meas`, `MDEDIT_SETTINGS`
naming its `settings.ini`, seeded with `[mdedit]` / `files=1` / `pane=notes` / `check_updates=0`
and a directory junction `library` into the library; `--trace --size 1280x800 --exit-after-ms
2500 fixtures/sample.rs`; the run checked to have written back `files=1`, `pane=notes`. Six runs
per build, medians with min-max.

`md-wt\data\lib-large` is 70 group files and 2,000 notes. The before build reads 64 of them (its
cap); the after build reads all 70, so this pair is not like for like:

| | Before, 64 of 70 groups | After, 70 of 70 groups |
|---|---|---|
| Private at idle, MiB | 31.50 (31.0-31.8) | 31.20 (30.8-31.4) |
| Private at the first frame, MiB | 28.8 (28.5-29.2) | 28.7 (28.3-29.0) |

To compare the same library on both builds, `lib-60` is a copy of `lib-large` with the ten last
groups removed: 60 files, 1,714 notes, under both caps. This is the slim copy's saving alone:

| | Before | After |
|---|---|---|
| Private at idle, MiB | 31.50 (31.1-32.0) | 30.90 (30.5-31.4) |

The owner forgave the memory figure on a 2,000-note library (D-063), so the point here is not the
megabyte but that the copy answered nothing; the 0.55 MiB is what it cost.

## The event-loop cost of a library change

A temporary test in `app::library_pane_tests` (removed before the commit) opened the real 70-file
library through the folder preference, saved `group-01.md` again six times and timed
`App::library_tick` with `Instant::now()` around it, in a release test build:

```
cargo test --release -p mdedit measure_library_change_tick -- --nocapture
```

The rows were checked to gain exactly one on each run, so the timed work is the one measured in
the first document as `read()` at 38.8 ms plus the row rebuild:

| | Before, one pass | After, the pass that starts the read | After, the pass that takes the result |
|---|---|---|---|
| Six runs, ms | 40.3, 40.4, 40.8, 44.8, 46.3, 48.9 | 2.33, 2.35, 2.41, 2.50, 2.56, 2.57 | 5.78, 5.80, 5.88, 5.94, 6.00, 6.18 |
| Median, ms | 42.8 | 2.46 | 5.91 |

The 2.3-2.6 ms start is the unchanged stat check (`Library::changed`, 2.25 ms on this library)
plus the thread; the 5.8-6.2 ms pass is the result being checked, filtered and turned into rows.
The 38.8 ms read itself is on the worker thread and no longer blocks a frame; the worst pass is
under two scroll frames instead of ten.

## What was not measured

- The first frame on this PC is not comparable with the first document's (that evening it read
  380-460 ms for both builds against 410-440 in the first pass); the memory figures above are the
  ones this work can move, and both builds were run in turn.
- The row rebuild was timed inside the tick in both builds but not separated from
  `without_notes` in the "after" pass.
- macOS and Linux: no desktop here.
- A shot of the omitted row: the shot runner has no library (a shot never reads the user's
  folder), so the row is pinned by its label and signature tests, not by a picture.
