Unreleased: menus fade in as they open, messages rise, stack and wait their turn, and a setting to turn motion off. (Renamed to the version's own file when it is cut.)

## New

- **Menus fade in.** Menus, context menus and submenus fade in and settle into place as they
  open, in about a ninth of a second; closing is instant, and the word completion list appears at
  once as before.
- **Animations, in Settings › Appearance.** On by default. Off, nothing moves. With the system's
  animation effects off (Windows: Accessibility › Visual effects), nothing moves either.
- **Messages rise in and fade out.** A message at the bottom of the text now rises into place
  and fades away when its time is up. A message that updates (a count, a step) changes in place;
  confirmations such as "File path copied" stack, at most three, and the same one said again
  stays one — brought to the bottom, so the one you just triggered is the one in sight.
- **New waits for the lift.** When confirmations stack, the ones above lift out of the way
  before the new one rises in underneath.
- **Menu highlights fade, the command palette pops in, and notice bars slide into place.**
  Notices present on the first paint, a rename notice returning with its tab, word completion,
  and the language list stay instant. The crash notice is checked after the first paint, so
  it slides in just after launch when animations are on.
- **Settings switches slide and folder chevrons turn.** Changing Settings pages or closing
  the dialog is still instant.
- **Settings fades in when you open it.** Its card and dimmed background fade in together;
  closing it is still instant.
- **Light and dark themes crossfade.** The change takes 170 ms, including when the
  desktop changes while Theme is set to System. With a window backdrop, a very large
  window, or motion off, the theme still switches instantly.
- **Tooltip and title hover fixes.** Scrolling or middle-clicking a tab clears its tooltip
  immediately; hovering the active tab does not schedule an invisible fade.
- **Submenus follow the pointer and keyboard.** Escape or Left drops any pending hover
  change and closes the submenu that's actually on screen. Reopening a submenu shows
  current items and check marks.
- **The split preview has a scrollbar.** Click the track or drag the thumb to move
  through a long page without selecting its text.
- **Files and Functions have keyboard focus.** Use the pane's shortcut, move with the
  arrow keys, press Enter to open the row, or press Escape to go back to the editor.
- **Window placement follows the saved monitor.** If that monitor is no longer
  available, the operating system chooses where the window opens.
