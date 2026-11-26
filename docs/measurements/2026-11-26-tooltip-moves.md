# A pointer move through the tooltip, 2026-11-26

**What was measured.** The cost of `App::update_tip`, which runs on every pointer move now that
the tooltip's wait counts from the pointer coming to rest (`Tooltip::hover_at`, D-257): the
whole of what a move adds to a `CursorMoved` event, not the event's other work and not a frame.

**Method.** A test that is a measurement, `tip_move_cost` in `bins/mdedit/src/app/motion_tests.rs`
(`#[ignore = "a measurement, not a test"]`), release build, on this PC (Windows 11, the owner's):

```bash
cargo test --release -p mdedit -j 8 --config 'build.rustc-wrapper=""' \
    -- --ignored --nocapture tip_move_cost
```

It builds a still window with a bar button's tooltip showing, then makes 20,000 moves within
that button (case A: the pointer moves, the control does not — the warm path, where
`chrome::tooltip` makes one `String` with a `format!` and `Tooltip::hover` clones the shown
tip's text once) and 20,000 repeats of the same control at the same place (case B: skipped,
`Ui::tip_point`). Each call is timed with `Instant::now()` around it; the median and the 10th
percentile of each are printed. Seven runs were made in a row; the figures did not vary.

**The figures**, per pointer move, seven runs, all identical:

| case | median | p10 |
|---|---|---|
| A: within one control (one `String`, one clone) | 200 ns | 200 ns |
| B: same control, same place (skipped) | 0 ns | 0 ns |

`Instant` on Windows advances in about 100 ns steps, so 200 ns is two steps and "0 ns" is below
one: B is the cost of two comparisons, and A's 200 ns is the resolution's floor for anything
measurable, not a ceiling of the real work.

**The skip**, which the second column of case B measures: a move that changes neither the
control nor the pointer's place returns before `chrome::tooltip` is asked for a text and before
anything is cloned (`App::update_tip`, `Ui::tip_point`). It does not help a real move that stays
on one control (case A), which must still pass the text through `hover_at`.

**Not measured.** A whole `CursorMoved` event or a frame in a real window (the pointer was
driven from a test); the same on Linux and macOS; the cost when no tooltip is showing (lower: no
clone of a shown text); and memory.
