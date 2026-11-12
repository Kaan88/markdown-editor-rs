# Spike "ime": input methods in winit 0.30.13 on Windows

Date 2026-11-12. Windows 11, winit `=0.30.13` (default-features off, `rwh_06`), softbuffer `=0.4.8`,
as in the viewer's `bins/mdv-app/Cargo.toml`. Throwaway code; the product is the findings below and
the hand-test script.

## What was done, and what was NOT

| Part | Status |
|---|---|
| Smallest window with IME on, logging, preedit/caret drawing, `set_ime_cursor_area` | Built, `cargo build --release` clean |
| Starts and exits cleanly with `--auto-exit-ms 1500` | Run 3 times, exit code 0 each, log below |
| Reading winit's Windows source to document dead keys, AltGr, text vs Commit, pitfalls | Done (file and line references below) |
| **Typing into the window: Turkish layout, dead keys, AltGr, any CJK IME** | **NOT DONE.** I cannot type into the window. Every claim about what events arrive is from reading source, not from observing. The hand-test script at the end is for the user. |
| Timing or memory numbers | Not measured. This spike's product is behaviour, not speed; nothing here claims a number. |

Run log of the auto-exit check (identical shape in all 3 runs; machine was loaded by other builds):

```
[   37.307 ms] window created, set_ime_allowed(true)
[   37.599 ms] set_ime_cursor_area pos=(10, 10) size=(14, 24)
[   37.637 ms] Focused(true)
[ 1500.076 ms] auto-exit
[ 1500.119 ms] auto-exit      <- about_to_wait runs once more after el.exit(); harmless
```

Time to window creation in the three runs was about 37 ms from process-local `Instant`, but that is
not a first-frame measurement and was taken once per run on a loaded machine, so it is only an
indication.

## The program

`src/main.rs`, about 280 lines. Run:

```
cd spikes/ime
export CARGO_TARGET_DIR=$PWD/target
cargo build --release
./target/release/ime-spike.exe [--auto-exit-ms N] [--log path]     # default log: ./ime-log.txt
```

- `set_ime_allowed(true)` right after window creation.
- Logs to stderr and the log file, each line prefixed with milliseconds since start: `KeyboardInput`
  (state, logical key, text, location, repeat, physical key, is_synthetic, plus
  `text_with_all_modifiers` and `key_without_modifiers`), `ModifiersChanged`, `Focused`, and every
  `Ime` variant (Enabled, Preedit with cursor range, Commit, Disabled), and each
  `set_ime_cursor_area` call.
- Drawing is plain rectangles: each character is a dot pattern hashed from its code point (so
  different characters look different, not readable), CJK and fullwidth take two cells. Committed text
  dark, preedit blue with an underline, block caret light blue. The caret sits after the committed
  text plus the preedit up to the preedit cursor's start.
- Text enters the buffer from `KeyEvent.text` on key press (control characters filtered; Backspace
  pops a char; Enter adds a newline) and from `Ime::Commit`. Both paths are active on purpose: if
  winit ever delivered the same text through both, the drawing would show it doubled and the log would
  show why.
- `set_ime_cursor_area(PhysicalPosition(x, y), PhysicalSize(14, 24))` is called whenever the caret
  cell moves (after every key and IME event, deduplicated).

## What winit 0.30.13 does on Windows (from source)

Files are under `winit-0.30.13/src/platform_impl/windows/`: `keyboard.rs` (`KeyEventBuilder`),
`event_loop.rs` (message handling), `ime.rs`, `keyboard_layout.rs`.

### Key events: one `KeyEvent` per key press, built from several messages

`KeyEventBuilder` (keyboard.rs, doc comment at line 50 and `process_message`) joins the messages of a
key press: one `WM_KEYDOWN`/`WM_SYSKEYDOWN`, then zero or one `WM_DEADCHAR`, then zero or more
`WM_CHAR`/`WM_SYSCHAR`. It peeks the next message with `PeekMessageW` to decide whether the event is
complete. Key release is a single `WM_KEYUP` with no text.

### Dead keys (for example ^ then a = â)

1. Dead key pressed: `WM_KEYDOWN`, then `WM_DEADCHAR`. On `WM_DEADCHAR` the builder finalizes
   immediately (keyboard.rs ~line 154). Result: a `KeyboardInput` Pressed with `text: None` and
   `logical_key: Key::Dead(Some('^'))` (the layout lookup yields `Dead`, which is not a `Character`,
   so `TextOr` does not replace it). `key_without_modifiers` converts a dead key to its plain
   `Character` (keyboard.rs ~line 560), so key bindings keep working. There is no composed text and
   no `Ime` event for this: the app must not insert anything yet.
2. Base key pressed (a): `WM_KEYDOWN`, then `WM_CHAR` with `â`. Result: Pressed with
   `text: Some("â")` and `logical_key: Character("â")` (the text replaces the layout key when it is a
   single grapheme).
3. Dead key followed by a key that does not combine (for example ^ then x): Windows sends two
   `WM_CHAR` (`^` and `x`); the builder joins them, `text: Some("^x")`, and because that is more than
   one grapheme `logical_key` falls back to the plain layout key (`Character("x")`). An app that
   inserts `text` gets both characters, which is the right result.
4. Dead key then Space gives `text: Some("^")` by the same path.
5. The release of the dead key and of `a` arrive as normal Released events with `text: None`.

winit does not turn the dead key into an `Ime::Preedit` on Windows (its own doc example in
event.rs ~line 744 shows that for macOS-style compose). On Windows dead-key composition is entirely
visible in `KeyEvent.text`, with or without IME allowed. (Not confirmed by running: see hand test.)

### AltGr (for example Turkish Q: AltGr+Q = @)

Windows models AltGr as a fake Left Ctrl press followed by Right Alt. winit handles this in three
places:

- `is_current_fake` (keyboard.rs ~line 711): on a layout with AltGr (`has_alt_graph`, detected by
  scanning for keys whose Ctrl+Alt character differs from the plain one), a Ctrl key event followed by
  a right-Alt (scancode 0xE038) event is dropped. So no fake Ctrl `KeyboardInput` is delivered for
  either press or release.
- The AltGr key itself reports `logical_key: Named(AltGraph)` and `location: Right`.
- `get_agnostic_mods` (keyboard_layout.rs ~line 272): while Right Alt is down on an AltGr layout, the
  `ModifiersChanged` state has **both Ctrl and Alt cleared**. So with AltGr+Q the app sees no Ctrl and
  no Alt in `ModifiersChanged`, and the Q press arrives with `text: Some("@")`, `logical_key:
  Character("@")`.
- In the `WM_CHAR` path (keyboard.rs ~line 234) the Ctrl test is "Ctrl held and Alt not held" on AltGr
  layouts, so AltGr does not count as Ctrl and the text is the system's (`@`).

### `KeyEvent.text` versus `Ime::Commit`

- Plain typing, dead keys, AltGr, any key that the keyboard layout itself maps to a character: always
  `KeyEvent.text`, whether or not IME is allowed. `Ime::Commit` is not involved.
- IME composition (pinyin, kana, hangul, and TSF text services that use the IMM32 composition
  messages): with `set_ime_allowed(true)`, `WM_IME_STARTCOMPOSITION` sends `Ime::Enabled`,
  `WM_IME_COMPOSITION` sends `Ime::Preedit(text, cursor)` while composing and, on a result string
  (`GCS_RESULTSTR`), a synthetic empty `Preedit("", None)` followed by `Ime::Commit(text)`;
  `WM_IME_ENDCOMPOSITION` sends `Ime::Disabled` (event_loop.rs 1513-1612). The composed text is
  carried by `Ime::Commit` only. The `WM_CHAR` messages that Windows may still generate for the result
  are ignored by the key builder when there is no pending key event (keyboard.rs ~line 170, "The
  message is probably IME") so the commit is not duplicated as `KeyEvent.text`.
- During composition the keys that the IME consumes reach the window as `WM_KEYDOWN` with
  `VK_PROCESSKEY` (0xE5). winit's layout maps that to `Key::Named(NamedKey::Process)` (keyboard_layout.rs
  line 946). The doc on `set_ime_allowed` says that during preedit the window does not get
  `KeyboardInput`; on Windows, from the source, that is not strictly so: `Process` key events can
  still arrive with `text: None`. An editor must ignore `Named(Process)` for editing commands.
  (Source reading only, to be checked by the hand test.)
- If IME is NOT allowed (`ime_allowed == false`): the `WM_IME_*` handlers send no `Ime` event
  (they check the flag), `set_ime_allowed(false)` calls `ImmAssociateContextEx(IACE_CHILDREN)` which
  detaches the input context from the window so the IME does not engage, and key events behave as plain
  key events. The window gets `Ime` events only if allowed.
- `Ime::Preedit` cursor range is in **bytes** of the UTF-8 preedit string, `(start, end)`; the spike
  converts to characters by iterating `char_indices`. `None` means hide the cursor. It can be
  `Some((n, n))` (just a caret) or a range selecting the clause being converted.

### Known pitfalls

1. **Ctrl+AltGr ambiguity.** On an AltGr layout, `ModifiersChanged` hides both Ctrl and Alt while
   Right Alt is down, and `is_current_fake` only drops a Ctrl that immediately precedes a Right Alt
   message. So Ctrl+Right-Alt (a real Ctrl) is not distinguishable from AltGr alone in
   `ModifiersChanged`, and a shortcut like Ctrl+Alt+key typed with Right Alt will not report Ctrl. For
   shortcuts, use `Left Ctrl` for Ctrl, and do not bind anything to plain AltGr+key, because
   those keys produce characters on Turkish, German, and similar layouts (AltGr+Q is `@`, a
   Ctrl+Alt+Q binding would swallow it). Left Alt+Left Ctrl on a no-AltGr layout works as Ctrl+Alt.
   Also on a layout without AltGr, `has_alt_graph` is false and Ctrl+Alt is genuinely both modifiers.
2. **Ctrl changes `text`.** When Ctrl is held (and not AltGr), `text` is recomputed from the layout
   without Ctrl (keyboard.rs ~line 245) so Ctrl+A gives `text: Some("a")`, not `U+0001`;
   `text_with_all_modifiers` has the raw control character. An editor must not insert `text` when Ctrl
   (or Super) is held; the spike inserts it anyway so the log shows it, which is a spike shortcut, not
   a recommendation.
3. **Key repeat.** Held keys produce repeated `KeyboardInput` Pressed events with `repeat: true`
   (from the lParam repeat bit), each with its own `text`. Dead keys repeat as `Dead` events with
   no text. IME composition keys do not give useful repeats.
4. **Synthetic events.** On focus gain winit synthesizes Pressed for held keys, on focus loss it
   synthesizes Released (`WM_SETFOCUS`/`WM_KILLFOCUS`), flagged with `is_synthetic`. Do not insert text
   from them (they carry none, but do not treat them as user actions).
5. **`set_ime_cursor_area` coordinates.** Windows `ime.rs::set_ime_cursor_area` converts the position
   and size with the window's scale factor (`to_physical`) and uses them as client-area coordinates
   (relative to the window's top-left of the client area, not screen). Passing `PhysicalPosition`
   avoids a second conversion; passing `LogicalPosition` is multiplied by the scale factor. Candidate
   window: `CFS_EXCLUDE` with that rectangle (so the box avoids covering the caret cell); composition
   window: `CFS_POINT` at `(x, y + height)`, i.e. the bottom-left of the area. So the rectangle must be
   the caret cell (top-left, line height), not a point, or the candidate list overlaps the line. It
   is only applied if the system has an IME (`SM_IMMENABLED`); if the IME context changes (switching
   input language), call it again, since it sets the HIMC's forms, not a persistent window property.
   The call is queued onto the window thread (`execute_in_thread`), so it is asynchronous.
6. **IME allowed changes key events.** `set_ime_allowed` sets a flag and associates or detaches the
   IMM context. With it true, an IME-enabled input language sends composition keys as
   `VK_PROCESSKEY` and the preedit/commit come as `Ime` events; with it false the same input language
   yields raw key events and no composition. Windows creates the window with IME disallowed (window.rs
   line 1161), so the app must call `set_ime_allowed(true)` (the spike does, after creating the
   window). An editor should allow IME when a text field has focus and disallow it for non-text
   focus, since toggling re-associates the context and a pending composition is discarded
   (the `Ime::Disabled` arrives via `WM_IME_ENDCOMPOSITION`; check whether a `Commit` arrives first when
   composition is interrupted: the source commits a pending result in the Preedit state, see
   event_loop.rs ~line 1586, but this was not run).
7. **Hangul order quirk.** The source comments that the Hangul IME sends `WM_IME_COMPOSITION` after
   `WM_IME_ENDCOMPOSITION`; winit handles it by committing in the end handler when state is
   `Preedit`. Korean should be tested by hand, not assumed.
8. **Messages not forwarded to `DefWindowProc`.** `WM_IME_COMPOSITION` is consumed (result 0) "to hide
   composing text drawn by IME" and `WM_IME_SETCONTEXT` masks `ISC_SHOWUICOMPOSITIONWINDOW`: the app
   must draw the preedit itself, as the spike does; the system's composition window is not shown.
9. **`PeekMessageW` in the key path.** `KeyEventBuilder` peeks at the message queue; its own doc
   warns about deadlocks if an event handler calls message-pumping functions (for example a modal
   dialog) while a key event is being built. The viewer's file dialogs run in `rfd`; keep dialogs out of
   the keyboard handler's call chain (post them for later).

## Conclusion

winit 0.30.13's Windows backend gives what a text editor needs, by design: dead keys and AltGr
produce finished `KeyEvent.text` (a dead key alone gives `Key::Dead` with no text), IME composition
gives `Preedit`/`Commit` separately from key events, and `set_ime_cursor_area` positions the
candidate box. The editor must (a) insert `KeyEvent.text` only for presses without Ctrl/Super and not
for `Process`/`Dead` keys, (b) insert `Ime::Commit` text, (c) draw the preedit itself, (d) re-send
the caret rectangle on every caret move, and (e) avoid Ctrl+Alt (and AltGr) shortcuts. None of this
is verified by running anything beyond the clean start and exit; the hand test below decides it.

## Hand test (for the user; NOT done by me)

Build and start from `spikes/ime`: `./target/release/ime-spike.exe --log ime-log.txt` and click the
window so it has focus. Keep the log file open or watch the console. Each step says what the log
should show if the source reading above is right. Anything else is a finding to report.

### A. Latin and modifiers (any layout)
1. Press and release `a`. Expect `KeyboardInput Pressed ... text=Some("a") repeat=false`, then
   Released with `text=None`. The window shows one glyph.
2. Hold `a` for a second. Expect repeated Pressed with `repeat=true`, each with `text=Some("a")`.
3. Press Left Ctrl, then `a`, then release both. Expect `ModifiersChanged CONTROL`, Pressed
   `logical=Character("a") text=Some("a") text_all_mods=Some("\u{1}")`. (The spike inserts the `a`; that
   is the spike, an editor would not.)

### B. Turkish Q layout (Win+Space or Alt+Shift to switch to Turkish Q)
Right Alt on Turkish Q is AltGr.
1. `AltGr+Q`. Expect no `KeyboardInput` for a Left Ctrl, one Pressed for the Right Alt with
   `logical=Named(AltGraph) location=Right`, `ModifiersChanged` with Ctrl and Alt both absent (state
   stays empty), then Pressed for Q with `logical=Character("@") text=Some("@")`, then releases. The
   window gets `@`. Report if you see a Control key event or `CONTROL` in the modifiers.
2. `AltGr+2` and `AltGr+7` (Turkish Q gives `²`-family and `{` characters depending on version): note
   the `text` for each. Any `text=None` for a key that types a character is a bug to report.
3. Dead key: I could not confirm which Turkish Q key is the dead `^` on this keyboard. Press
   candidates (`Shift+3`, `AltGr+3`, `AltGr+ü`, `AltGr+6`), and use the one whose Pressed event shows
   `logical=Dead(Some('^'))` (or `~`, `´`, `` ` ``, `¨`) with `text=None` and nothing drawn. Then press `a`:
   expect Pressed `text=Some("â")` `logical=Character("â")`. If Turkish Q has no dead key on your
   keyboard, switch to **US International** (add it in Settings, Language) and use `'` then `a`: expect
   `Dead(Some('´'))`, then `text=Some("á")`.
4. Dead key then a non-combining key (dead `^` then `x`): expect one Pressed with `text=Some("^x")`
   and `logical=Character("x")`. Dead key then Space: `text=Some("^")`.
5. Real Ctrl+AltGr: hold Left Ctrl, then AltGr+Q. Note whether `ModifiersChanged` ever shows `CONTROL`
   (pitfall 1 predicts it does not) and what `text` is.
6. Turkish letters `ı ş ğ ç ö ü İ`, with Caps Lock on and off, and `Shift+i`: note `text` (the
   point is that `text` is already case-mapped by Windows, not by us).
7. Confirm no `Ime::*` line appears anywhere in B (the Turkish layout has no IME); if `Ime::Enabled`
   does appear, report it.

### C. CJK IME (Settings, Language: add Chinese Simplified Microsoft Pinyin, or Japanese Microsoft IME,
and Korean as a third check). Switch with Win+Space.
1. Pinyin: with the IME in Chinese mode type `ni`. Expect `Ime::Enabled`, then `Ime::Preedit("n",
   Some((1,1)))`, `Ime::Preedit("ni", Some((2,2)))` (the exact form can differ: Microsoft Pinyin may
   insert spaces or show the cursor as a range), and `KeyboardInput ... logical=Named(Process)` events
   for the keys, `text=None`. The window shows the preedit blue and underlined, with the caret after
   it. The candidate box must appear **at the caret, just under the line**, not at the window's
   top-left. Check by typing a few lines first (Enter) and see that it follows. Then press Space (or a
   number) to select: expect `Ime::Preedit("", None)` then `Ime::Commit("你")`, then `Ime::Disabled`
   if the IME ends the session. The window shows the committed (wide, 2-cell) character, once, not
   twice. A doubled character means `KeyEvent.text` and `Commit` both delivered it: report it with the log.
2. Escape during preedit: expect Preedit cleared (`Preedit("", None)`), then `Ime::Disabled` and no
   Commit. Backspace during preedit shortens the preedit and must not delete committed text; check
   the window: if committed text shrinks, the editor must ignore `Process`/preedit-time keys.
3. Switch the IME to English/direct mode and type `ab`: expect plain `KeyboardInput` with text,
   no `Preedit`.
4. Japanese: type `nihon` (Hiragana), expect preedit `にほん` growing; press Space for conversion:
   expect `Preedit("日本", Some(range))` with a non-empty range (the clause being converted); Enter:
   Commit. Resize the window or click elsewhere mid-composition and note what arrives (`Commit` first
   or only `Disabled`).
5. Korean (hangul): type `gks`; expect preedit updates and a Commit when the syllable completes
   (pitfall 7); note any case where a character commits twice or is lost.
6. After each IME test, note whether `set_ime_cursor_area` lines in the log appear at every caret
   move (they should) and that the candidate box moved with them.

### D. Focus and misc
1. Switch away mid-composition (Alt+Tab) and back; note `Focused(false/true)`, any synthetic key
   events (`synthetic=true`), and whether the preedit is cleared.
2. Run on a second monitor with different DPI if you have one; the candidate box offset from the caret
   must stay the same number of pixels (pitfall 5).

Send back the log file (`ime-log.txt`) from steps B3, B5, C1 and C4 at least.

## What was not measured or checked

- Everything under the hand test above. No dead key, AltGr or IME event has been observed.
- Any layout other than what source reading covers; the pitfalls are from winit's own code, not
  from testing.
- TSF-only input methods that do not drive the IMM32 messages, and the emoji panel and handwriting
  panel: winit 0.30 uses IMM32 only (`ime.rs`), so these may not arrive as `Ime` events at all; not looked
  into.
- Memory, CPU, latency, first-frame time.
- Linux/macOS behaviour.
