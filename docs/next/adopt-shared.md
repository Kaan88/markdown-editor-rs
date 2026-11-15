# Adopting the shared crates' new features (branch f/adopt-shared)

Shared crates repinned from `6249cd1` to `065cbdcc19226ecb77dd85f85ca922f53f2d3aee` (the branch
`f/linux-frost`, which contains `681baf8`: Vue and Svelte highlighting, menu access keys, the
snap-layouts flyout). Only the rev changed in `Cargo.toml` and the lock; every third-party version
in `Cargo.lock` is as it was (`cargo build --locked` works, no `cargo update`).

Run (Windows, release, `CARGO_TARGET_DIR` outside the worktree): `cargo fmt --all`,
`cargo clippy --locked --release --workspace --all-targets -- -D warnings` (clean), `cargo test
--locked --release --workspace` (all passed: 299 in the mdedit bin, the rest unchanged).

**Linux: the changes in section 4 were never run or seen on Linux. They are in code that Windows
does not compile, so they were written without a compiler and checked only by CI (see the end).**

## 1. Vue and Svelte
- Built: `.vue` and `.svelte` open highlighted with no code of ours (`Language::for_path`). The
  editor has no fixed list of language names (the status bar shows `Language::name`); the only
  list is the open dialog's filter, `OPEN_FILTER` in `bins/mdedit/src/platform.rs`, which gained
  `vue` and `svelte`. The function list (`mdedit-outline`) has no rule for them and stays empty.
  Test: `docs::tests::vue_and_svelte_files_open_with_their_language`.
- Not verified: how a real component looks in the editor; the dialog's filter in a real dialog.
- Release note: Vue and Svelte files are syntax highlighted and listed in the Open dialog's filter.

## 2. Snap-layouts flyout
- Built: `App::sync_snap` (`bins/mdedit/src/app.rs`) runs after every composed frame and calls
  `mdshared_ui::snap::set_maximize_button_rect(&window, rect)` when the rectangle changed (device
  px key, so a new size, maximize/restore or scale gives a new call). `chrome::snap_button_rect`
  works the rectangle out: the maximize button (`win_button_rect(title.right(), 1)`), plus the
  client-drawn shadow margin, minus the top resize band (`window::EDGE`) unless maximized; `None`
  while a dialog, the palette or the settings form is open (their backdrop covers the title row).
  The app has no full-screen or distraction-free mode, and no menu reaches the title row. Test:
  `chrome::tests::the_snap_rect_is_the_maximize_button_below_the_resize_band`.
- Run, in a real window started from Git Bash with `MDEDIT_SETTINGS` in a new folder: the app
  starts; `WM_NCHITTEST` at the maximize button's screen point answers 9 (`HTMAXBUTTON`) and
  1 (`HTCLIENT`) at the close button, at the minimize button, in the tab strip and in the top
  band above the button; posted client mouse messages (move, press, release) on the button
  maximize the window (placement 1 to 3); posted non-client messages (`WM_NCMOUSEMOVE`,
  `WM_NCLBUTTONDOWN`, `WM_NCLBUTTONUP` with `HTMAXBUTTON`) restore it and maximize it again.
  Windows 11 build 26300, 96 dpi.
- Not verified: the flyout itself and snapping (needs a real pointer); other scales; touch; that
  the dialog case really answers `HTCLIENT` in a window.
- Release note: On Windows 11, hovering the maximize button shows the snap-layouts flyout.

## 3. Menu access keys
- Built: the main menu and its View, Macro and Recent submenus, the tab, text (Lines, Markdown and
  Convert submenus), find-box and minimap menus read `&` marks (`.access_keys(true)`), one letter per
  item within a menu; marks were found by a small search for the first free letter, preferring word
  starts. The Recent menu's file names and the saved macros' names go through `escape_access` (they
  have no mark, so no letter picks them, and a `&` in a name is drawn). The completion menu, the
  line-ending and encoding menus are not opted in (completion because typing continues; the other
  two because their labels come from enums and a letter would have no meaning).
- Keys: in `key_pressed`'s open-menu branch, after Up, Down, Tab, Enter, Space, Left, Right and
  Escape, a printable character without Ctrl or Alt (`access_char`: the typed text if it is one
  character, else the bare key) calls `access_key` on the open submenu, else on the menu, and acts
  with `choose(act, true)` exactly as Enter does (a submenu opens with the keyboard on it). Before,
  such a character in an open menu did nothing, so nothing is lost. The completion list still takes
  typing first.
- Underlines: `Ui::keyboard` (true before any input, set by every key press, cleared by a mouse
  press) decides when a popup opens; mouse-opened menus hide them (`set_access_visible(false)`),
  and any key used in an open menu shows them (`Popup::show_cues`). `Popup::cues` mirrors the state
  because the shared menu has no getter.
- Tests (`app/access_tests.rs`, `app/input.rs`): marks are unique and complete for every fixed menu;
  data labels; the Recent menu with `R&D notes.md`; a letter picks and opens a submenu, a capital
  works, a letter nothing has or a disabled item leaves the menu; Ctrl or Alt does nothing;
  completion stays opted out and typing goes on; underlines follow keyboard and mouse.
- Looked at: `docs/captures/access-keys/main-view-menu.png` (`--shot ... --menu main-view`, dark): the
  underlines are visible on the first letters and on `n` of "Ope**n** recent", `y`, `h`, `k`, `u`, `i`.
- Not verified: the menus in a real window by keyboard and mouse; light theme; the underline on
  every label; Windows' habit of showing underlines only after Alt (here: after any key).
- Release note: Menus show underlined access keys; press the letter to pick an item.

## 4. Frosted chrome on KDE Plasma (Wayland), non-Windows code only
- Built (`bins/mdedit/src/app.rs`): `sync_frost` passes `mdshared_ui::frost::BlurSystem(&window)`
  off Windows (Windows still `WindowSystem(hwnd)`); `sync_surface_alpha` calls
  `surface.set_premultiplied_alpha(frost.wants_alpha() || shadow.wants_alpha())` whenever the frost
  changes and once after the surface is made, next to the existing shadow call. The app already pins
  the patched softbuffer (`Cargo.toml`) and had that call. The third change the shared notes list,
  keying the marked pixels out and alpha `0xFF` elsewhere, needed nothing: the app has one frame
  copy for every system, it already calls `frost::key_out` while `frost.key()` is `Some`, and the
  bitmap's alpha is always 255 apart from the marks.
- **Never run or seen on Linux. Only compiled, by CI on Linux.** Nothing about KWin, the blur, the
  look or the focus rule is verified.
- Release note: On KDE Plasma (Wayland) the "Window backdrop" setting blurs the desktop behind the
  chrome.
- CI (`gh workflow run ci.yml --ref f/adopt-shared`, run 43732981608): the windows and linux jobs both passed, so the non-Windows code compiled and the Linux tests passed. Nothing was run or seen in a Linux window.
