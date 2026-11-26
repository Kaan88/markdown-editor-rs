# Launch with a restored stale tab — 2026-11-26

Windows release builds, before at `3ab2f8c` and after at `0966ac0`. This is the real
windowed session-restore path, not the usual `--no-session` startup comparison. Each process
got a fresh `MDEDIT_SETTINGS` folder with only two generated/test files listed, one of them
last seen 18 days of use earlier. Update checks were disabled. No personal session, settings,
or reports were used.

The first-frame and idle-memory traces use the method in `docs/CODE.md`: 1280 × 800, light
theme, alternating build order, `--behind --trace --exit-after-ms 1500`. The two settled
reference images were checked against the same session with the stale offer turned off so
the fixture could not silently benchmark an ordinary launch.

31 interleaved pairs with Windows Animation effects on:

| Measure | Before | After |
|---|---:|---:|
| First frame median, ms | 83.6 | 82.6 |
| First frame range, ms | 78.7–116.3 | 79.6–102.0 |
| First frames over 100 ms | 1/31 | 1/31 |
| First-frame compose median, ms | 12.28 | 12.66 |
| Traced damage frames per launch | 8 | 4 |
| Full-height damage frames after first frame | 5 | 0 |
| Idle private memory median, MiB | 27.2 | 27.2 |

The paired median first-frame difference was −0.5 ms. The five slowest first frames were
116.3, 98.2, 88.9, 87.4 and 87.0 ms before, versus 102.0, 89.5, 88.3, 86.8 and 86.3 ms
after. Every before run drew five extra full-height damage frames (6.70 ms median combined
traced frame time); no after run did. The restored stale banner is full-height from the
first paint in the regression test, with no animation deadline. The static before and after
reference images are pixel-identical. The 100 ms launch budget is not a hard bound.

Executable size: 10,139,648 → 10,140,672 bytes (+1,024 bytes); SHA-256 prefixes:
`7f4441448b81` → `48a5b3e4dc35`. The run did not measure cold
launch, idle CPU, normal typing or scrolling, or any other UI transition. It did not change
the Windows motion setting; the app's motion-off paths were tested separately.
Raw logs and images: `target/sol-stale-launch/`.
