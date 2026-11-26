# Settings and theme transitions on Windows

`70c2f7e` versus `4134fed`, both Windows release builds with Rust 1.99.1.
`tools/compare-builds.py` ran 31 alternating pairs on `fixtures/sample.rs`
at 1280 × 800 with `--no-session --behind --light` and fresh scratch settings.

| Measure | Before | After |
|---|---:|---:|
| First-frame median, ms | 80.80 | 82.50 |
| First-frame range, ms | 75.30–152.70 | 76.50–147.00 |
| Launches over 100 ms | 4/31 | 4/31 |
| First-frame compose median, ms | 12.43 | 12.54 |
| Idle private memory median, MiB | 27.10 | 27.00 |
| Idle private memory range, MiB | 26.60–27.50 | 26.50–27.40 |
| Keystroke p50 / p95, ms | 0.52 / 0.76 | 0.53 / 0.75 |
| Scroll p50 / p95, ms | 0.42 / 0.59 | 0.43 / 0.59 |

The paired median first-frame difference was +0.40 ms. The executable grew
by 3,072 bytes. Neither transition captures a bitmap on the measured launch path.

## Memory while the effect runs

Three alternating Windows pairs per command, with Windows Animation effects on.
The process's private bytes were sampled about every 8 ms around the delayed
`--run Settings` or `--run ThemeDark` command. These are sampled maxima, not
guaranteed peaks.

| Command | Highest active sample before, MiB | Highest active sample after, MiB | Highest post-command sample after, MiB |
|---|---:|---:|---:|
| Settings | 27.73 | 31.95 | 28.03 |
| ThemeDark | 27.61 | 31.41 | 27.50 |

Both effects temporarily exceeded the 30 MiB private-memory budget even at
1280 × 800. The theme cap limits the snapshot's size, not total process memory.
Settings still takes a temporary full-window snapshot per entrance paint; making
that cheaper remains open. Idle memory was under budget.

Raw samples: `target/sol-transition-bench.log`. Cold launch, the 100 MiB file,
active frame times, native compositor frost, Linux and macOS were not measured.
Tests verified that the completed fade drops its snapshot and requests no further
animation frame; process CPU at idle was not remeasured for this round.
