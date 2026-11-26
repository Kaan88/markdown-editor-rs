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

It builds a still window with a bar button's tooltip showing, then makes **200,000** moves within
that button (case A: the pointer moves, the control does not — the warm path, where
`chrome::tooltip` makes one `String` and `Tooltip::hover` clones the shown tip's text **twice**,
once for what it remembers of the earlier frame and once for its "what shows changed"
comparison: **three allocations a move**) and 200,000 repeats of the same control at the same
place (case B: skipped, `Ui::tip_point`).

**Each batch is timed as a whole and divided.** The first version timed every call with
`Instant::now()` on both sides and reported medians, and that was wrong for two reasons: Windows'
`Instant` advances in steps of about 100 ns, so one call reads as one or two steps, and the
earlier "0 ns when skipped" was that step hiding a real cost, not a free path. Seven runs were
made in a row; the figures did not vary by more than a few nanoseconds.

**The figures**, per pointer move, seven runs, the median of the seven:

| case | median | range over the seven |
|---|---|---|
| A: within one control (one `String`, two clones) | 215 ns | 212 to 220 ns |
| B: same control, same place (skipped) | 25 ns | 25 ns |

**The skip**, which the second row measures: a move that changes neither the control nor the
pointer's place returns before `chrome::tooltip` is asked for a text and before anything is
cloned (`App::update_tip`, `Ui::tip_point`). It does not help a real move that stays on one
control (case A), which must still pass the text through `hover_at`.

**Not measured.** A whole `CursorMoved` event or a frame in a real window (the pointer was
driven from a test); the same on Linux and macOS; the cost when no tooltip is showing (lower: no
clone of a shown text); and memory.
