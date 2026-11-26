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
On 2026-11-26 the owner accepted the theme fade's brief overshoot (D-277):
about 31.4 MiB at 1280 × 800 and 100% scale. The 170 ms fade keeps its
full-window snapshot, may exceed 30 MiB only while it runs, and frees the
snapshot as soon as it ends. This exception is for the theme fade only.
Opening Settings during a theme fade can hold both copies at once; the overlap
was not measured. At `4134fed`, Settings took a temporary full-window snapshot
per entrance paint. Idle memory was under budget.

Raw samples: `target/sol-transition-bench.log`. Cold launch, the 100 MiB file,
active frame times, native compositor frost, Linux and macOS were not measured.
Tests verified that the completed fade drops its snapshot and requests no further
animation frame; process CPU at idle was not remeasured for this round.

## Review follow-up: Settings card only

`dbeb294` stopped copying the whole window for the Settings entrance. The card
and shadow are copied only on intermediate frames; outside them the dim is
adjusted in place. At 1280 × 800 and native 100%, six runs (three per theme)
sampled at intervals of about 3 ms peaked at **29.25 MiB**, versus **31.95 MiB**
in the original Settings samples above. The sampled active range was
28.46–29.25 MiB, and all six runs stayed under 30 MiB. The theme copy was not
changed. Concurrent Settings and theme fades were not measured.

For 125% and 150%, a scratch release build forced the painter scale and
corresponding physical window size; desktop DPI and installed apps were
untouched. This measures rendering and memory at the given sizes, not OS
DPI-change handling. Each row summarizes six runs (three per theme).

| Scale | Physical size | Pre-open range (MiB) | Active range (MiB) | Post-entrance range (MiB) |
|---|---|---:|---:|---:|
| 100% (native) | 1280 × 800 | 26.45–27.18 | 28.46–29.25 | 27.03–27.83 |
| 125% (forced) | 1600 × 1000 | 31.02–31.84 | 33.82–34.61 | 31.59–32.39 |
| 150% (forced) | 1920 × 1200 | 36.62–37.15 | 40.44–40.97 | 37.23–37.77 |

At both higher scales the pre-open samples were already over 30 MiB. Raw
samples: `target/sol-settings-peak-100.log` and `target/sol-settings-peak-scaled.log`.

The final release build (`dbeb294`) was compared in another 31 interleaved pairs
against the original pre-transition build (`70c2f7e`), not against `4134fed`.
First-frame median was 77.00 → 78.00 ms (+0.40 ms paired); ranges were
73.50–96.70 → 73.80–103.20 ms, with one final run over 100 ms and none on the
baseline. Its cause is not established. First-frame compose median was
11.86 → 11.82 ms. Idle private memory was 27.00 MiB on both builds; keystroke
p50/p95 was 0.51/0.62 → 0.51/0.65 ms and scroll was 0.41/0.50 → 0.41/0.51 ms.
Three additional 100% light-theme samples raised the Settings sampled maximum
to **29.31 MiB**; the unchanged theme fade reached **31.31 MiB** in this repeat.
Raw samples: `target/sol-transition-review-bench.log`.
