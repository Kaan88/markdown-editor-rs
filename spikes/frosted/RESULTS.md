# Spike "frosted": a system backdrop (Mica, Acrylic) behind the editor's chrome on Windows 11

Date 2026-11-15. Windows 11 Pro 26H2, build 26300.9974 (an Insider build), one screen at scale 1.0,
dark system theme, transparency effects on, on AC power, battery saver off. winit and softbuffer are
the app's forks at the app's commits (`Kaan88/winit` 89ace86, `Kaan88/softbuffer` dd18fec), through
the same `[patch.crates-io]`. Throwaway code; the product is the findings below.

## The answer in short

- **It works, and it is a small feature, not a change to how the window is presented.** Two calls
  winit already has: `with_transparent(true)` on the window attributes (winit then calls
  `DwmEnableBlurBehindWindow` with an empty region) and the backdrop attribute
  (`DWMWA_SYSTEMBACKDROP_TYPE`; winit's `with_system_backdrop` / `set_system_backdrop`). After that,
  every pixel softbuffer presents with alpha 0 shows the backdrop, alpha 255 is opaque, and values
  between blend (premultiplied).
- **softbuffer's GDI path carries alpha as it is.** `BitBlt` from its 32-bit DIB section copies the
  top byte to the window, and DWM honours it once the window is "transparent". No ARGB path, no
  layered window, no `DwmExtendFrameIntoClientArea`, no swap chain. The softbuffer fork needs no
  change on Windows.
- **It costs nothing that could be measured in the process**: first frame and private memory are the
  same with and without, within the run-to-run noise (table below). The blur is done by DWM.
- **The work is in the Painter, not in the window**: chrome backgrounds written with alpha 0 (or a
  tint), and text that sits on them drawn with greyscale antialiasing and premultiplied alpha.

## What was done, and what was NOT

| Part | Status |
|---|---|
| Spike window: undecorated, resizable, winit + softbuffer, hidden first frame as the app does | Built (`cargo build --release` clean), run |
| Route 1: backdrop attribute only | Run. Does not work: transparent areas are black |
| Route 2: `DwmExtendFrameIntoClientArea(-1)` + attribute | Run. Works, but DWM draws its own caption buttons |
| Route 3: route 2 without winit's undecorated-shadow marker | Run. Same as route 2 |
| Route 4: `with_transparent(true)` + attribute | Run. Works cleanly. **The route to use** |
| Layered window, ARGB softbuffer path, DirectComposition swap chain | **Not tried**: route 4 works, the task said to stop at the first that works well |
| Mica, Mica Alt, Acrylic; dark and light; inactive window; a tint; alpha ramp | Run, screenshots in `shots/` |
| First-frame time and private memory, 20 runs per variant, interleaved | Run, numbers below |
| Resize: 60 programmatic size changes per run, 3 runs per variant | Run, numbers below |
| **Dragging and resizing by hand (flicker, lag as the eye sees it)** | **NOT DONE.** No hand touched the window. Hand-test script at the end |
| **Transparency effects turned off, battery saver on** | **NOT RUN.** The settings were only read (both in the "effects on" state). What happens is from documentation |
| Maximized, snapped, second monitor, scale other than 1.0, high contrast | Not run |
| Windows 10, Windows 11 21H2/22H2 release builds | Not run (this PC is on 26H2 Insider). From documentation |
| Linux (Wayland, X11) | Not run, no code. Notes from documentation at the end |
| DWM's own cost (dwm.exe memory, GPU time) | Not measured: it is outside the process |
| Real text | The mock draws bars, not glyphs. Nothing here shows how real antialiased text reads on a backdrop |

A screenshot of a backdrop is a screen capture (GDI `CopyFromScreen`) of what the compositor put
out. It is close to what the eye sees but it is not proof of it, and it says nothing about motion.
The owner judges the look on the screen.

## The program

`src/main.rs`, about 540 lines. A mock editor: a title row with three tabs and caption boxes, a
side column with rows, a status strip (these three are the "chrome"), and an opaque page of pseudo
text. Pixels are written as premultiplied ARGB.

```
cd spikes/frosted
export CARGO_TARGET_DIR=C:/Users/kaank/Desktop/md-wt/target-frosted
cargo build --release
$CARGO_TARGET_DIR/release/frosted-spike.exe --dark --blur --backdrop mica      # Esc closes
```

| Argument | Meaning |
|---|---|
| `--backdrop mica\|alt\|acrylic\|none\|auto` | `DWMWA_SYSTEMBACKDROP_TYPE` 2, 4, 3, 1, 0 |
| `--blur` | winit `with_transparent(true)` (route 4) |
| `--extend` | `DwmExtendFrameIntoClientArea` with margins -1 (route 2) |
| `--no-marker` | without winit's `with_undecorated_shadow(true)` |
| `--opaque` | chrome in its flat colour at alpha 255 (the baseline) |
| `--tint N` | the flat chrome colour at alpha N (0..255) over the backdrop |
| `--alpha0` | the opaque parts written as `0x00RRGGBB`, as code unaware of alpha would |
| `--dark`, `--light` | colours, and `DWMWA_USE_IMMERSIVE_DARK_MODE` through winit's theme |
| `--inactive`, `--top` | shown without activation; always on top |
| `--pattern` | a colourful opaque window, to put behind the spike window |
| `--resize-test` | 60 size changes, each drawn and presented, times logged |
| `--auto-exit-ms N`, `--log path`, `--pos x,y`, `--size w,h` | |

The title row drags the window, a 6 px band at the edges resizes it (winit's `drag_window` and
`drag_resize_window`, as the app does).

`shot.ps1` runs one variant over the pattern window and saves the capture and the log;
`measure.sh` does the timing runs.

## Question 1: can the window show a backdrop, and what must the surface be

| Route | Result | Screenshot |
|---|---|---|
| 1. Attribute only | The call succeeds (`hr 0`, reads back 2) and nothing shows: alpha-0 pixels are black | `shots/r1-mica-plain-dark.png` |
| 2. + `DwmExtendFrameIntoClientArea(-1)` | The backdrop shows, alpha blends. But DWM draws the system's minimize, maximize and close glyphs in the top right corner, under the app's own (the window still has `WS_CAPTION \| WS_SYSMENU`) | `shots/r2-mica-extend-dark.png` |
| 3. Route 2 without the 1 px non-client strip | The same, glyphs included | `shots/r3-mica-extend-nomarker-dark.png` |
| 4. `with_transparent(true)` + attribute | The backdrop shows, alpha blends, no system glyphs, shadow and corners unchanged | `shots/b-mica-dark.png` and the other `b-*` |

What route 4 showed, each read off the captures:

- **Alpha is carried exactly.** The ramp at the bottom of the side column runs the accent colour
  `4ea1ff` from alpha 0 to 255 over a backdrop of `1e2121`. Measured: alpha 0 gives `1e2121`, 63
  gives `2a4158`, 127 gives `366190`, 191 gives `4281c7`, 253 gives `4da0fd`. Those are the
  premultiplied "over" values to within one step. So antialiased edges can be composed over the
  backdrop.
- **Everything opaque must carry alpha 255.** With `--alpha0` the page and the active tab vanish
  into the backdrop (`shots/b-mica-dark-alpha0.png`). The app already writes 255 on every pixel
  (the test at `bins/mdedit/src/app.rs`, "Every pixel of a composed frame has alpha 255"), which is
  why the window can be made transparent without anything changing while the feature is off.
- **Without a backdrop the transparent parts are plain see-through**, not blurred
  (`shots/b-nobackdrop-dark.png`, `--backdrop none`). So transparent pixels may only be written
  when a backdrop is known to be in force; otherwise the chrome keeps its flat colour.
- winit's `set_transparent` at run time only sets a flag on Windows; the DWM call is made at window
  creation. So either the window is always created transparent on Windows (measured: no cost, see
  below) and the setting only changes what is painted and which backdrop type is set, or the shared
  crate makes the one `DwmEnableBlurBehindWindow` call itself.

## Question 2: which parts could be frosted

The mock: frosted title row, side column and status strip around an opaque page
(`shots/b-mica-dark.png`, `b-mica-light.png`, `b-acrylic-*.png`).

- **Title row, tab strip, side panes, status strip: yes.** The backdrop is one per window and lies
  behind the whole of it, so any region of the window can show it by being transparent.
- **The text area: no**, as the plan says. It stays opaque.
- **Menus and dialogs: no.** They are drawn inside the same window, on top of the page. A backdrop
  shows what is behind the *window* (Mica: the wallpaper; Acrylic: the windows behind), never the
  app's own page under the menu. A frosted menu would need the menu to be a window of its own, which
  is a different piece of work.
- **Mica is close to a flat colour**: dark `1d2121` against the mock's flat `2b2b2b`, light `ecf6f6`
  against `f3f3f3`, with a slow tint from the wallpaper. Mica does not show windows behind (the
  pattern window is invisible through it). Text on it should read as on a flat colour.
- **Mica Alt** is stronger: dark `000e0f` on this wallpaper (nearly black), light `d3dddd`.
- **Acrylic shows the blurred colours of whatever is behind**, strongly
  (`shots/b-acrylic-dark.png`: red, blue and orange patches behind grey text). Bare, it hurts
  the chrome's text. With the chrome colour laid over it at alpha 128 it calms down
  (`shots/b-acrylic-dark-tint128.png`, `b-acrylic-light-tint128.png`).
- **What the Painter would have to do** (not tried here, the mock has no glyphs): text is blended
  against a known colour today, and LCD (subpixel) antialiasing needs that known colour: a pixel
  has one alpha, not three. Text on a frosted area has to be greyscale-antialiased and written
  premultiplied. Selection and hover highlights on frosted areas become half-transparent fills
  (the selected row in the mock is the accent at alpha 70).

## Question 3: what it costs

### First frame and memory

20 runs per variant, interleaved (run 1 of each, run 2 of each, ...), release build, window 900 x 560
at scale 1, hidden first frame then shown and presented again, as the app does. Time is from the
start of `main`. Another session was building on the machine at times, which is where the maxima
come from.

| Variant | First frame presented, ms (median, min to max) | Plus one DWM composition, ms (median) | Private at first frame, MiB (median) | Private after 1.2 s | Working set |
|---|---|---|---|---|---|
| Opaque, no backdrop (baseline) | 31.57 (28.49 to 41.75) | 35.88 | 5.23 | 5.23 | 16.87 |
| Mica (route 4) | 31.34 (28.43 to 41.36) | 37.59 | 5.23 | 5.23 | 16.87 |
| Mica Alt (route 4) | 31.10 (28.75 to 43.37) | 37.54 | 5.23 | 5.23 | 16.87 |
| Acrylic (route 4) | 30.60 (28.68 to 63.95) | 36.06 | 5.22 | 5.23 | 16.87 |
| Mica, route 2 (extend) | 31.07 (28.81 to 45.06) | 37.40 | 5.23 | 5.23 | 16.87 |

No difference outside the noise in time; none at all in memory. "Plus one DWM composition" is the
time at which `DwmFlush` returned after the first frame; its 1.5 ms spread between variants is
within one refresh interval and within the spread between runs. Transparent window with the chrome
painted opaque and no backdrop (the "feature off" state if the window is always created
transparent): 6 runs, 28.1 to 30.8 ms, 5.22 to 5.28 MiB.

These are the spike's numbers, not the app's: the app's budgets (first frame under 100 ms, private
under 30 MiB) are not shown to hold by this, only that the backdrop adds nothing measurable to a
window of this kind.

### Resizing

60 programmatic size changes (`request_inner_size`, growing then shrinking by up to 240 x 120 px),
each drawn and presented, 3 runs per variant:

| Variant | Draw + present, mean ms | Until DWM composed it, mean ms (max over the 3 runs) |
|---|---|---|
| Opaque | 0.54 to 0.59 | 5.67 to 6.11 (7.66) |
| Mica | 0.53 to 0.57 | 5.66 to 5.80 (8.20) |
| Mica Alt | 0.59 to 0.63 | 5.64 to 5.89 (9.42) |
| Acrylic | 0.55 to 0.56 | 5.92 to 5.93 (9.45) |
| Mica, route 2 | 0.53 to 0.59 | 5.78 to 5.92 (7.75) |

The same. This is not the modal resize loop a mouse drag runs, and a timing says nothing about
flicker. **Dragging and resizing by hand was not done.**

### Corners, shadow, border

`shots/zoom-corners.png` (top-left and bottom-right corners at 4x; opaque dark, Mica, Acrylic,
route 2, opaque light, Mica light): the rounded corners, the shadow and the 1 px top line are the
same with and without the backdrop. The backdrop is clipped to the rounded corners by DWM.

### Dark and light

`DWMWA_USE_IMMERSIVE_DARK_MODE` (which the shared `style_window` already sets) picks the dark or
light material: `b-mica-dark.png` against `b-mica-light.png`, and the same for Alt and Acrylic.

### Inactive window

DWM switches to a flat colour by itself, with no repaint by the app:

| | Active (this wallpaper) | Inactive |
|---|---|---|
| Mica, dark | `1d2121` | `202020` (`shots/b-mica-dark-inactive.png`) |
| Acrylic, dark | blurred content | `545454` (`shots/b-acrylic-dark-inactive.png`) |

The inactive Acrylic grey is much lighter than the app's dark chrome; with a tint over it, less so.
Light theme inactive was not captured.

### Transparency effects off, battery saver

Read only: `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\EnableTransparency`
is 1 on this PC; `GetSystemPowerStatus` says AC power, battery saver off. **Not run with either
changed.** Microsoft's documentation says Mica and Acrylic fall back to a solid colour when
transparency effects are off, in battery saver and on low-end hardware, and the inactive case
above shows DWM doing that kind of fallback itself. Whether the fallback colour is acceptable next
to the app's page is unknown. The safe rule costs one registry read: if `EnableTransparency` is 0,
paint the chrome opaque and do not rely on DWM's fallback.

## Question 4: fallbacks (from documentation, not run)

- **Windows 11 before 22H2 (build 22621):** `DWMWA_SYSTEMBACKDROP_TYPE` does not exist. 21H2
  (22000) has an undocumented attribute for Mica only. Use the flat colour.
- **Windows 10:** no Mica, no supported backdrop for Win32 windows. The undocumented
  `SetWindowCompositionAttribute` (accent state "acrylic blur behind", from 1803) is what other
  apps use; it is undocumented and is reported to lag while the window is dragged on some builds.
  Recommendation: flat colour on Windows 10. Because a transparent window without a backdrop is
  see-through, the app must decide by the Windows build (22621 or later) and by the result of the
  attribute call, and paint the chrome opaque otherwise.
- **Wayland:** KDE has `org_kde_kwin_blur` (plasma-wayland-protocols): a blur object per surface
  with a region, which fits "chrome only" directly. A cross-compositor protocol,
  `ext-background-effect-v1` (wayland-protocols, staging), does the same job; which compositors
  ship it has to be checked when the work starts. GNOME (Mutter) offers clients no blur. Hyprland
  blurs transparent surfaces by the user's own configuration, without a protocol. Elsewhere: flat
  colour. The window needs an ARGB surface, which the softbuffer fork gives on Wayland only where
  the client-drawn margin is in use; under KWin's shadow route the surface is opaque today, so
  that route would need alpha switched on too.
- **X11:** under KWin, the `_KDE_NET_WM_BLUR_BEHIND_REGION` window property, on a window with an
  ARGB visual. Other window managers: nothing a client can ask for (picom blurs by the user's
  configuration). Flat colour.

## Recommendation

Do it as a small feature, Mica first, on Windows 11 22H2 and later, chrome only.

The smallest change:

1. **`mdshared-ui` (window set-up, Windows):** create the window with `with_transparent(true)`;
   next to `style_window`, set `DWMWA_SYSTEMBACKDROP_TYPE` (2 Mica, 4 Mica Alt, 3 Acrylic, 1 none)
   from the setting, and report whether a backdrop is in force: build 22621 or later, the call
   returned success, `EnableTransparency` is 1. The apps add one line each at window creation.
2. **Painter (`mdshared-ui`):** a "frosted" flag for chrome surfaces. When set, their background is
   written with alpha 0 (or the flat colour at a tint alpha), fills on them are premultiplied, and
   text on them is greyscale-antialiased and premultiplied. When not set, everything is as today.
   This is the real work and the part the owner has to judge by eye.
3. **softbuffer fork:** nothing on Windows. (The fork's `set_premultiplied_alpha` is a no-op there;
   the top byte already goes through.)
4. **winit fork:** nothing.
5. **Setting:** off, Mica, Mica Alt, Acrylic; off by default until looked at. Acrylic only with a
   tint.

Risks to check when building it, in this order: real text on Mica (greyscale against today's LCD
text beside it on the page); a hand drag and resize; transparency effects off; light theme
inactive; maximized and snapped windows; a release build of Windows 11 (this PC is on an Insider
build).

## Hand test for the owner

```
cd C:\Users\kaank\Desktop\md-wt\edit-frosted\spikes\frosted
C:\Users\kaank\Desktop\md-wt\target-frosted\release\frosted-spike.exe --dark --blur --backdrop mica
```

Then the same with `--backdrop alt`, `--backdrop acrylic`, `--backdrop acrylic --tint 128`,
`--light`, and `--opaque` (no `--blur`, no `--backdrop`) for comparison. Esc closes.

1. Drag the window by the title row across the wallpaper and over other windows: does the frosted
   area lag behind, tear or flash?
2. Resize from the right and bottom edges, fast: do the newly uncovered parts flash a wrong colour
   before the frame arrives? Compare with `--opaque`.
3. Click another window: the chrome goes flat. Is the jump acceptable in dark and in light?
4. Maximize is not wired in the spike; snap with Win+Left and look at the edges.

## Files

- `src/main.rs`, `Cargo.toml`, `Cargo.lock`: the spike.
- `shot.ps1`, `measure.sh`: the capture and timing scripts.
- `shots/`: `a-opaque-{dark,light}` (baseline), `r1`, `r2`, `r3` (routes that were set aside),
  `b-{mica,alt,acrylic}-{dark,light}`, `b-*-inactive`, `b-acrylic-*-tint128`, `b-nobackdrop-dark`,
  `b-mica-dark-alpha0`, `zoom-corners`.
- `logs/`: the log of each capture run, and `measure-summary.txt`.
