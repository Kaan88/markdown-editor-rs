# Frosted window (Windows 11)

Feature key `frosted-window`. Editor branch `f/frosted-window` (from `origin/merge/next-release`),
shared branch `f/frosted-window` (from the shared rev that base pinned). Off by default.

## README

Feature list:

- **Window backdrop** (Windows 11, off by default): Settings > View > Window backdrop lets the
  system's Mica or Acrylic material show through the title row and tab strip, the bar, the side
  pane and the status strip. The text, menus and dialogs stay solid.

No shortcut, no command.

## DESIGN

**The setting.** `backdrop = off | mica | acrylic` in the settings file (`Prefs::backdrop`,
default `off`); the row "Window backdrop" sits under Theme in Settings > View and exists only in a
Windows build. `--pref backdrop=mica` works like every other preference.

**What is frosted.** The regions Title (title row and tab strip), Bar, Files (the side pane) and
Status. The text area, the minimap, the results panel, menus, the command palette, tooltips and
dialog cards stay opaque. A dialog's dimming and the soft shadows of menus fall on the frosted
chrome and darken it, as they do on the flat chrome.

**When it is frosted.** All of these must hold, otherwise the chrome is today's flat colours:

- the setting is Mica or Acrylic;
- Windows build 22621 or later (22H2: the first with `DWMWA_SYSTEMBACKDROP_TYPE`);
- "Transparency effects" is on (`HKCU\...\Themes\Personalize\EnableTransparency`, missing counts
  as on);
- high contrast is off (`SPI_GETHIGHCONTRAST`);
- battery saver is off (`GetSystemPowerStatus`);
- the system accepted the backdrop calls;
- the window has the keyboard. The system shows no material behind an inactive window, so an
  inactive window paints its flat chrome itself: pixel for pixel what it shows with the setting
  off.

These are read when the setting changes and each time the window gains the keyboard, so turning
transparency effects off takes hold the next time the window is activated. With the setting off
the cost is one comparison per frame: no system call, and the window is never made transparent.

**How.** Route 4 of the spike (`spikes/frosted/RESULTS.md` on `spike/frosted-window`):
`DwmEnableBlurBehindWindow` with an empty region makes the window's alpha channel count, and
`DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE)` puts Mica (2) or Acrylic (3) behind it. Both are
called on the live window when the setting is first turned on; nothing changes at window creation.

The Painter is unchanged and the chrome is drawn opaque exactly as today. After a frosted region is
drawn, the alpha byte of its pixels is set to a mark (254). Everything drawn later with opaque
spans (menu, palette, dialog card, tooltip) writes alpha 255 and so clears the mark pixel by pixel;
blended spans (shadows, dimming) leave it. When a frame is copied to the window, marked pixels (and
only in the copy) are "keyed out": the key colour, the palette's plainest chrome colour (the
darkest of title/bar/pane in the dark theme, the lightest in the light one), becomes transparent,
and every other colour gets the smallest alpha that still reproduces it over the key, premultiplied.
Over a backdrop equal to the key the result is exactly today's pixels; over another backdrop each
pixel moves by `keep * (backdrop - key)`, so text keeps its subpixel rendering and its contrast
against its surroundings.

**Veil.** Mica is close to a flat colour, so the key is fully transparent (veil 0). Acrylic shows
what is behind the window, so the chrome keeps a veil of its own colour over it (150 of 255);
without it text over a busy window behind does not read well.

**Mica Alt is left out**: in the dark theme it is close to black and makes the chrome heavier than
the text area; it adds nothing the other two do not.

**Where the code is.** Shared: `mdshared_app::backdrop` (the type, keys, row texts),
`mdshared_ui::frost` (mark, key colour, veil, key-out, and the state `Frost`, tested against a
stand-in system: moved there from the editor when the branch was merged, so that mdview uses the
same one), `mdshared_ui::platform::backdrop_available` / `backdrop_allowed` / `set_backdrop`.
Editor: `bins/mdedit/src/frost.rs` (which regions), `App::sync_frost`, the mark in the region loop
of `compose`, the key-out in `present`.

**Shots.** A `--shot` has no window, so it never shows the material. `tests/shots.sh` gained
`settings-view` and `settings-view-dark` (the View section, which had no shot, with the new row
under Theme on Windows) and `settings-view-backdrop` (the row set to Mica; the chrome in it is flat,
which is the point: no window, no frost). On Linux these three show the section without the row.

## Release note

- Windows 11: an optional frosted window. Settings > View > Window backdrop (Mica or Acrylic) lets
  the system material show through the bars and the side pane. Off by default.

## Handoff

**Built.** As above. Shared commits `74a2273`, `9aab56b` and, on `merge/editor-next`, `c9ba573`
(the state moved to the shared crate); the editor pinned `9aab56b` on this branch before the merge (only the
`mdshared-*` entries of `Cargo.lock` changed; one new windows-sys feature, `Win32_System_Power`,
no new crate). Unit tests: the type and its keys, the key-out arithmetic (exact over the key,
opaque stays opaque, premultiplied), `backdrop_allowed`, the state machine in `frost.rs`, and four
app-level tests on composed frames (chrome marked and text not, at scale 1 and 1.5; a menu over
the chrome opaque and its shadow not; a dialog; unfocused or off: every pixel opaque).

**Setting off is unchanged.** `tests/shots.sh` before and after, compared byte for byte: 179 of
188 identical. The 9 that differ are all Settings shots and differ only because of the new row:
`settings`, `settings-dark`, `settings-narrow`, `settings-scale`, `settings-complete` (the
scrollbar thumb is a few pixels shorter) and `settings-preview`, `-dark`, `-narrow`, `-scale`
(the list is one row longer). In a real window, a capture of the new build with the setting off is
identical to a capture of the build before the feature, in both themes (window area, 0 differing
pixels).

**Numbers** (release builds, this PC: Windows 11 build 26300, 3440x1440, scale 1; `measure.sh` in
`docs/next/frosted-shots/`, 15 interleaved starts, 3 x 300 keystrokes, 3 x 300 scroll steps on a
50,000-line file; "before" is the base commit's build):

| | before | off | Mica | Acrylic |
|---|---|---|---|---|
| first frame, ms (median; min-max) | 81.7 (79.2-98.1) | 81.9 (79.6-88.2) | 83.9 (82.1-90.9) | 84.3 (80.8-94.3) |
| private memory, MiB (median) | 27.6 | 27.6 | 27.7 | 27.8 |
| keystroke to pixels, ms (p50 / p95) | 0.45 / 0.51-0.63 | 0.45-0.47 / 0.54-0.60 | 0.65 / 0.78-0.83 | 0.66 / 0.81-0.83 |
| scroll step, own work, ms (p50 / p95) | 0.42 / 0.57-0.60 | 0.41-0.42 / 0.49-0.55 | 0.41 / 0.53-0.58 | 0.41-0.42 / 0.49-0.61 |

Off is the same as before within the noise. On: about 2 ms on the first frame (the two system
calls and one more full frame), about 0.2 ms on a keystroke, nothing on scrolling. Where the
0.2 ms on a keystroke goes was not traced further; it is the cost of keying out the chrome that a
keystroke repaints. It can be looked at if the owner keeps the feature.

**Captures** (`docs/next/frosted-shots/`, taken by `capture.sh` over a striped pattern window;
screen grabs, so what the compositor put out, not what the eye judges):
`{dark,light}-{off,mica,acrylic}-{active,inactive}.png`, `dark-{mica,acrylic}-{settings,palette,menu}.png`,
`light-acrylic-menu.png`, and `before-{dark,light}-active.png` from the base build. The inactive
captures are identical to the base build's except where the small window that took the keyboard
overlaps them (the checkered rectangle at the right edge).

**For the owner to decide.**

1. The look itself, by eye, in both themes: Mica is subtle (a tint of the wallpaper; over the
   striped pattern it reads as almost flat), Acrylic is the visible one.
2. The Acrylic veil (150 of 255): lower shows more of what is behind and costs legibility.
3. Frosted only while the window is active. This follows the system, which removes the material
   from inactive windows; the chrome then switches to the flat colours.
4. The row is shown only in Windows builds, and on Windows 10 it is shown but does nothing (the
   help line says "Windows 11"). Hiding it there is a small change if wanted.
5. Mica Alt left out.
6. Nine existing Settings shots change because of the row.

**Not run.**

- Transparency effects off, battery saver, high contrast, Windows 10, Windows 11 before 22H2: no
  system setting was changed and no other machine was used. Covered only by `backdrop_allowed`'s
  unit tests and the documented behaviour of the calls. On Windows 10 the two calls are never made
  because the build check fails first.
- Maximized, snapped, full-screen, minimized and restored, a second monitor, scale factors other
  than 1 in a real window (scale 1.5 is covered headless).
- Dragging and resizing the window by hand; the spike measured resizing with route 4 and found no
  cost, the feature was not driven that way.
- Changing the setting in the dialog by hand in a real window (the captures start with the
  setting already chosen; the change path is covered by the unit tests of `Frost::sync`).
- Remote desktop and software-rendered sessions.
- Linux: nothing changes there (the row is absent, the platform calls return false); checked by CI
  only.
- The viewer is not wired; everything it needs is in the shared crates.
