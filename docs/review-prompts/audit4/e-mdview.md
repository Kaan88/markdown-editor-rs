# Review E: mdview since v1.0.0 (`git diff v1.0.0 8612e74`), read-only

Worktree `C:\Users\kaank\Desktop\md-wt\fab-mv` at 8612e74. Nothing was committed or pushed; the
two probe files and the one `mod` line I added were removed and `git status` is clean again.

## Summary

No crash was found. The preview server took over 150,000 hostile requests (NaN, infinities,
`u32::MAX`, empty and multi-byte documents, a gap set under a changing text) without a panic.
The zoom math has no panicking path that a real window can reach.

Three things are worth acting on:

1. **Budget:** while a tab the editor asked for is open, every pass of the event loop resolves
   that tab's path on disk. Measured: 131 µs per pass with three such tabs, 0 with none.
2. **Zoom at other scales:** the view opens at `2 x scale` times the page's size, not twice. On
   a 200 % display it opens at 4x, with every raster pixel stretched over 2 device pixels.
3. **Add to the notes library** writes a library file without knowing whether mdedit has it
   open with unsaved changes. The shared `put_note` says the caller must ask first.

Also: one test whose pan assertion passes with the pan removed, a palette header mix-up from
240 library files up, and a narrow race on task-box ticks that mdedit's own check mostly
defuses.

Counts: data loss 1 (plausible, not proven), crash 0, wrong behaviour 6, test gap 1, nit 4.

One fact that differs from the brief: mdview's release profile is `panic = "unwind"`
(`Cargo.toml:16`), not `abort`. A panic on the main thread still ends the window, and one in
`--preview-server` still ends the preview, so the rule holds in effect.

## What was run

- `cargo test --release -p mdview-preview --features server --test probe_fable` (my probe, 4
  tests): all passed, no panic.
- `cargo test --release -p mdview probe_` (my probe, 4 tests): passed; their printed figures are
  quoted below.
- Not run: the full test suite, `tests/shots.sh`, the app in a window, anything off Windows.
  The engine was not built in this worktree, so the three engine-dependent tests would have
  skipped; I did not exercise a real diagram hit-test.
- Probe sources and logs are beside this file: `e-probe-preview.rs`, `e-probe-app.rs`,
  `probe1.log`, `probe2.log`.

## Findings

### 1. Every loop pass resolves the editor's tabs on disk — wrong behaviour (budget), proven

- `bins/mdv-app/src/app.rs:1774` (`about_to_wait` calls `tell_editor_closed`),
  `bins/mdv-app/src/app/commands.rs:962-976` (`editor_closed`),
  `bins/mdv-app/src/workspace.rs:1744` (`shows` calls `canonical_or_absolute`).
- `editor_closed` runs on every pass of the loop while `editor_shown` is not empty, which is
  the whole time "Preview in mdview" is in use. For each remembered file it calls
  `Workspace::shows`, which calls `std::fs::canonicalize` (open the file, ask its final path,
  close it). So every mouse move, wheel step, redraw and animation tick costs one file open per
  tab the editor asked for.
- Probe output: `editor_closed per call: none asked 0.00 us, three asked 131.45 us` (local SSD,
  release, 5,000 calls). About 44 µs per tab per pass. Against the 4 ms scroll frame that is
  1 to 3 %; on a network share or a sleeping disk each call can take milliseconds or block.
  HANDOFF says "Nothing is remembered, checked or sent for tabs the user opened", which is
  true, but gives no figure for tabs the editor opened.
- Fix: do not touch the disk here. Either store the canonical path in `editor_shown` when the
  editor asks and compare it with `same_file` against `tab.path` (already canonical), or have
  `Workspace` count tab removals and only run the check when that count changed.

### 2. The zoom view opens at `2 x scale` the page's size — wrong behaviour, proven

- `bins/mdv-app/src/zoom.rs:36` (opens at `zoom: 1.0`), `zoom.rs:125-132` (draws
  `r.w * scale` device px), against `crates/render/src/layout/media.rs:181` (the page draws
  `image.width / 2` device px).
- `zoom` is logical px per raster px, and the raster is made at `2 x scale`. So one logical px
  per raster px is `scale` device px per raster px. Probe output:
  - scale 1: page 500 logical px, zoom opens at 1000 (2x), each raster px on 1 device px
  - scale 1.5: opens at 1500 (3x), each raster px on 1.5 device px
  - scale 2: opens at 2000 (4x), each raster px on 2 device px
- On a 150 % or 200 % display the diagram opens already blurred and larger than meant, and
  `MAX_ZOOM` 4.0 is 8 device px per raster px. The file header, HANDOFF and the tests all say
  "twice the size the page showed"; the tests and shots only cover scale 1.0. HANDOFF lists
  "other device scales" as not verified.
- Fix: keep the view's size in device px per raster px (open at 1.0 = `1 / scale` logical),
  and scale `MIN_ZOOM`/`MAX_ZOOM` the same way. Add the scale-2 case to
  `zoom::tests` and a `zoom-diagram` shot at `--scale 2`.

### 3. The pan test passes with the pan removed — test gap, proven

- `bins/mdv-app/src/app/tests.rs:3825`:
  `assert!(view.pan.0 < 0.0 && view.pan.1 < 0.0)` after a drag of (-100, +30).
- The wheel step before it already leaves the pan negative on both axes. Probe output, with no
  drag at all: `pan after the wheel, no drag: (-64.0, -40.0)`. So the assertion holds whether or
  not `Drag::Zoom` moves anything; the drag even moves y the other way (+30).
- HANDOFF's revert check covered the click arm, the wheel and the press's source, not the pan
  in `App`. (`zoom::tests` does cover `pan_by` itself; the gap is the pointer path in
  `input.rs`.)
- Fix: read the pan before the drag and assert it moved by exactly (-100, +30).

### 4. Palette headers wrong from 240 library files up — wrong behaviour, by reading

- `bins/mdv-app/src/palette.rs:144`: `G_LIBRARY.saturating_add(g as u8)` with `G_LIBRARY = 16`.
- Groups 240 to 255 all saturate to id 255, and `g as u8` wraps at 256, so group 256 gets id 16
  again. Their notes are listed under another library file's header, and `set_groups` gets
  duplicate ids. The shared reader keeps up to 512 library files (`MAX_FILES`), so this is
  reachable. Not run.
- Fix: a wider group id, or stop giving each file its own header past the range.

### 5. The palette resolves every note on disk when its list is built — wrong behaviour (budget), plausible

- `bins/mdv-app/src/palette.rs:337` (per note), also 303 and 315 (per tab, per recent);
  `bins/mdv-app/src/app/modal.rs:34` (the library read on the first Ctrl+P).
- `files()` runs when the card opens and each time the query goes between empty and not empty,
  so the first typed character pays for it. It calls `canonical_or_absolute` for every note of
  the library. At the 44 µs measured in finding 1, 200 notes is about 9 ms and the reader's cap
  of 2,000 entries per file far more, against the 8 ms keystroke budget. I did not measure this
  with a library.
- The first Ctrl+P also reads the whole library on the UI thread, even if the Notes face was
  never used. That is a recorded decision (Decision 5); I found no figure for it.
- Fix: resolve each note's path once, in `Note::of` at open or when the library is read.

### 6. Add to the library does not know about mdedit's unsaved changes — data loss, plausible

- `bins/mdv-app/src/notes.rs:197` calls `put::put_note`, whose doc says: "Whether the file is
  open with changes not saved is the caller's to ask first." mdview does not ask and has no way
  to.
- Scenario: `Personal.md` is open in mdedit with unsaved edits. In mdview, "Add this file to
  the notes library" rewrites `Personal.md` on disk and says the note "is in Personal now".
  mdedit then holds a buffer older than the file. If the owner keeps mdedit's version and
  saves, the added line is gone with no word from either app. What mdedit does on an outside
  change with unsaved edits I did not check.
- A smaller version of the same: `put_note` reads the file, then writes it whole, with no check
  that it did not change in between. A save by mdedit in that window is overwritten. The window
  is milliseconds.
- Reading is sound: `library::read` uses `read_stable` and both apps write atomically, so the
  face never sees half a file.
- Fix: when mdedit is running, ask it to add the note over the sister channel (it owns the
  library and knows its buffers); write directly only when it is not. At least compare the
  file's stamp before the write with the one taken at the read.

### 7. A task tick can be answered against the wrong text — wrong behaviour, plausible, narrow

- `crates/preview/src/server.rs:163-171` (`Press` stores `task` and `keep_top`), 97-116 (`Text`
  clears neither `task`).
- The offset in `Update::task` is into the text the server held when it took the `Press`. If
  the editor sends a new `Text` before it reads that answer (typing, or an AI rewrite streaming
  into the buffer, while a box is clicked), mdedit pairs the offset with `sent.version`, the
  last text it sent (`mdedit preview_pane.rs:473`), which is the newer one. Its version check
  then passes and the offset is applied to a text it was not computed for.
- mdedit's `set_task` only writes where it finds `[ ]`, `[x]` or `[X]` around the offset
  (`mdedit-core blocks.rs:514-523`), so the usual result is a click that does nothing; hitting a
  different box needs the shift to land exactly on one. Not reproduced.
- Related: `keep_top` stays set until the next `Text`. If the tick is dropped as above, or the
  `Text` in the same batch was the typed one, the page skips following the caret once.
- Fix: count `Text` requests in the server and send that count with `Update::task`; mdedit keeps
  the count per text sent and compares. Clear `task` and `keep_top` when a `Text` arrives after
  the `Press` in `apply`.

### 8. The zoom view stays up over another document — wrong behaviour, minor, proven

- `bins/mdv-app/src/app/commands.rs:858-915` (`handed_over`) and every other path that changes
  the showing tab without going through the keyboard or pointer.
- Probe output: `after the editor opened math.md: zoom open = true, active tab = "math.md"`.
  A request from mdedit, a live text, or a file reload changes the page under the view; the
  view keeps showing a diagram of a document no longer shown until Esc.
- Fix: close the view when the active tab or its document changes (`close_zoom` from
  `handed_over` and the reload path), or leave it and say so in HANDOFF as decided.

### 9. Zoom view, small things — nit

- The title bar cannot be dragged and the window buttons cannot be used while the view is up:
  `hit_at` returns only overlay hits (`app.rs:1273`). Probe: the window's close button position
  hits `ZoomClose` (the view's button sits over it), the title bar hits nothing.
- macOS: `PinchGesture` (`app.rs:1699`) does not look at `ui.zoom`, so a pinch zooms the page
  under the view, not the diagram. By reading; not run.
- `0` cannot fit a raster more than ten times the window (`MIN_ZOOM` 0.1, `zoom.rs:14`, clamped
  in `fit`), so "shows the whole diagram" fails for a very wide one.
- A theme change while the view is open leaves the old theme's raster on the new sheet colour.

### 10. Preview server and wire, robustness — nit

- `crates/preview/src/server.rs:136`: `View` width and height are not capped, though the wire's
  own limit is an 8K by 8K frame. A width and height of a few hundred thousand make
  `Bitmap::new` ask for tens of gigabytes. This predates 1.0.0 and needs a broken editor; I did
  not run it.
- `crates/preview/src/wire.rs:551`: `Vec::with_capacity(n)` for up to 16.7 million rectangles
  (268 MB) before any of them is read. This is on the editor's side of the wire. Cap `n` by the
  bytes left in the frame.

### 11. `closed` can be missed or late — nit, plausible

- `bins/mdv-app/src/app/commands.rs:884`: an open from mdedit that fails (`ws.open` is `Err`) is
  never remembered, so mdedit is never told its preview has no tab.
- A `closed` is sent on a helper thread. If the owner asks for the same file again at once, the
  open can arrive before the earlier `closed` is delivered, and mdedit ends a preview that is
  showing. Not reproduced.

### 12. The notes face touches the disk where it need not — nit

- `bins/mdv-app/src/notes.rs:187`: `empty()` calls `tree::empty_state`, an `is_dir`, from inside
  `draw` on every repaint of an empty face.
- `notes.rs:100` and `app.rs:281`: at launch mdedit's `settings.ini` is read and the library
  folder is checked with `is_dir`. HANDOFF records this as intended. A library folder on an
  unreachable share would hold the first frame; I did not measure the launch.
- Shared crate: `stale::UseDays::start` adds 1 to a count read from the session; a
  `use_days=4294967295 …` line panics a debug build (a release build wraps).

## Checked and found sound

- **Preview server, hostile input** (versions 4 to 7): `Press`, `Drag`, `Release`, `Hover`,
  `BlockAt`, `BlockOfLine`, `Gap`, `Follow`, `Scroll`, `Align`, `SelectAll`, `Copy`, `Unfollow`
  with NaN, infinities, extremes and `u32::MAX`, over empty, one-character, emoji, nested-list,
  table, details, footnote and HTML documents; a selection made and then a shorter text; a gap
  set and then the text replaced; views of 0x0, 1x1 and odd scales. No panic.
- **Protocol accounting:** every batch is answered once with its count (`server.rs:642-663`), a
  frame or an ack, so mdedit's `waiting` cannot stick. Unknown kinds are skipped; the version in
  `MAGIC` guards that.
- **Wire:** every new field round-trips, a short frame is an error not a panic, `Some(vec![])`
  and `None` stay distinct for the selection and the block.
- **Gap:** `Gap::lead` and `covers` index with `get`; non-finite and negative heights become 0;
  an empty or reversed line range covers nothing.
- **Zoom math:** `clamp` cannot be given `min > max` (that branch is only taken when the image
  is larger than the area); non-finite wheel notches are refused; huge notches saturate at the
  limits; the view is clamped again on every resize (`app.rs:701`).
- **Idle CPU:** the zoom view, the notes face and the stale-tab bar add no timer. The preview
  server blocks on `recv`. The only per-pass work added is `focus_tick` (a compare) and
  finding 1.
- **Click to diagram:** the source text is taken with `get`, a multi-click never opens the
  view, a drag that selects does not open it, and an evicted raster is drawn again or reported.
- **Notes face reading while mdedit writes:** `read_stable` plus atomic writes on both sides.
  Row indices from the face are looked up with `get`.
- **A build never reads the owner's library:** `settings_file` needs an explicit
  explicit `MDEDIT_SETTINGS`, or a real session with no scratch `MDVIEW_SETTINGS`
  (`notes.rs:49-62`).
- **`?closed` tests:** each of the four fails by construction if its part is removed. A missing
  file keeps its identity through `canonical_parent_join`, so a deleted file is not reported as
  closed.
- **Unfollow test** fails with `Unfollow` reverted (the new text would go back to line 1).
- **Session parsing** of the new lines has no panicking path. **Version resource** test reads
  the built exe. In CI the engine binary is built by `cargo test --workspace` (the engine crate
  has an integration test), so the engine-dependent tests run there.

## Not got to

- Settings as pages beyond a skim of the diff and the shared form's bounds checks.
- Pins, stale tabs and brief tabs in depth (diff read once; no probes).
- The selection rectangles' geometry, the Align/Sync math and `Update::apply` (mostly older
  than 1.0.0).
- A real diagram hit-test at several scales and widths (needs the engine built).
- `library_tests.rs` for revert-proofing, the shots, `tools/write-hub-txt.ps1`, the release and
  CI workflow changes, the updater change.
- Anything in a real window, and anything on Linux or macOS.
