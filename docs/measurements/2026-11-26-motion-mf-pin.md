# M-F shared pin: `09b01c1` → `1588e0f`

Windows release builds of the same mdedit code (`33e26d9`) on the two pins. The five workspace
git references move together; neither the mdview renderer pin nor a registry dependency moves.
The new cascade API is opt-in and this commit does not call it.

The first-frame comparison uses `tools/compare-builds.py` with the builds interleaved and
`fixtures/sample.rs` at 1280 × 800. Each process uses `--no-session --behind --light` and an
independent scratch `MDEDIT_SETTINGS`; the owner's settings and session are not used.

| Measure (31 alternating pairs) | `09b01c1` | `1588e0f` |
|---|---:|---:|
| First frame median, ms | 79.50 | 78.50 |
| First frame range, ms | 75.80–98.50 | 76.10–85.10 |
| Over the 100 ms budget | 0/31 | 0/31 |
| First-frame compose median, ms | 11.87 | 11.95 |
| Idle private memory median, MiB | 27.00 | 27.00 |
| Keystroke p50 / p95, ms | 0.51 / 0.67 | 0.51 / 0.66 |
| Scroll p50 / p95, ms | 0.42 / 0.54 | 0.42 / 0.54 |

Paired first-frame median difference: −0.80 ms. The five slowest were 98.5, 93.9, 85.4, 84.5,
and 83.6 ms before, versus 85.1, 84.5, 83.9, 83.7, and 82.8 ms after. The executable shrank
from 10,140,672 to 10,137,600 bytes. Idle CPU, cold start, restored sessions and Linux/macOS
were not measured. Raw results: `target/sol-mf-pin-bench.log`.

Both Windows shot runs produced 504 PNGs with all 249 required checks passing. All but
`runs.png`, `runs-dark.png`, `runs-newer.png`, and `runs-newer-dark.png` are byte-identical.
In all four, only the generated demo times changed (for example 08:23 to 08:34); the changed
regions were inspected in both themes. Raw shots: `target/shots/sol-mf-pin-{before,after}`.
