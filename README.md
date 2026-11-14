# markdown-editor-rs

<img src="assets/mdedit.png" width="96" alt="The mdedit mark: an M and a pencil on a green tile">

`mdedit`: a light, fast text editor with syntax highlighting, the companion of
[markdown-viewer-rs](https://github.com/Kaan88/markdown-viewer-rs) (`mdview`). A Notepad
alternative for personal use that opens any text file, with the same hand-drawn interface as the
viewer.

**Status: version 0.5.0, for Windows and Linux. The owner has approved the icons and the minimap
by looking; the rest of the interface has not been explicitly approved.** The release files are built by
GitHub Actions, Windows natively on a Windows runner. On 2026-11-14 the full test suite and every
screenshot state ran on a real Windows runner; nobody has yet used it by hand on a Windows desktop.
Linux has run on KDE Plasma 6 (Wayland) and, headless, on sway; GNOME and other desktops have not
been tried.

It opens files in tabs, scrolls, selects, finds, highlights syntax, detects encodings and line
endings, edits with undo and redo, and saves in the file's own encoding and line endings without
touching any other byte. New file (Ctrl+N) opens an untitled tab that Save asks a name for.

- **Editing:** line commands (duplicate, move, delete, toggle comment, change case, sort, reverse,
  remove duplicates, join, trim, tabs to spaces), Markdown commands (bold, italic, code,
  strikethrough, heading level, link, align table), auto-pairing, and indentation that follows
  the file (tabs or spaces, detected per file). Several carets (Ctrl+click) and column selections
  (Alt+drag, Alt+Shift+arrows). Saved macros (Ctrl+Shift+R records, Ctrl+Shift+P plays).
- **Finding:** find and replace with match case, whole word and regular expressions (Ctrl+F,
  Ctrl+H), replace in the selection, find in files and open tabs (Ctrl+Shift+F, F4), and replace in
  files with a confirmation, backups and an undo. A function and heading list (Ctrl+Shift+2),
  bookmarks (Ctrl+F2, F2), folding by mouse or key (F9), a minimap (Ctrl+Shift+M).
- **Two files:** side by side (Shift+F8), compare two files or a file with its saved copy (F7 for
  the next difference), and the same file in both panes.
- **Command palette (Ctrl+P)** for tabs, files, commands, line numbers and symbols, and
  **Settings (Ctrl+,)** for the preferences.
- **With mdview:** Preview in mdview (Ctrl+Shift+V) opens the file in the viewer at the caret's
  line, and keeps the viewer on your unsaved text while you type (live preview, up to 2 MiB).
- **Window:** rounded corners (Settings > Window corners: Square, Small or Round; Round, 8 px at
  100 %, is the default). Windows 11 draws them itself; on Wayland the window draws its own shape
  and shadow; where the window is opaque (X11, KDE with a square setting) it stays square.
- Changes a file's line endings and encoding from the status bar, wraps long lines (Alt+Z),
  keeps unsaved text when the window closes and puts it back at the next start, and updates itself
  in place like mdview. There is no input-method support yet.

[docs/PLAN.md](docs/PLAN.md) holds the goals, the budgets, the phases and the feature roadmap;
[docs/DESIGN.md](docs/DESIGN.md) the design (section 16 for editing and saving).

The painter, widgets, text engine and highlighting come from
[markdown-shared-rs](https://github.com/Kaan88/markdown-shared-rs), pinned to a commit in
`Cargo.toml`.

## Download

The [releases page](https://github.com/Kaan88/markdown-editor-rs/releases/latest) has:

- **Windows:**
  - `mdedit-setup-<version>.exe`: the installer, per user. It offers mdedit under "Open with" and
    never takes a default. Pick this for a normal install.
  - `mdedit-<version>-windows-x86_64.exe`: the bare executable, no installation needed.
  - `mdedit-<version>-windows-x86_64.zip`: the same executable with LICENSE and README.md in a
    folder, for a portable copy.
  - None is code-signed.
- **Linux** (x86-64):
  - `mdedit-<version>-linux-x86_64.AppImage`: any distribution, no root. Make it executable and
    run it: `chmod +x mdedit-*.AppImage && ./mdedit-*.AppImage`.
  - `mdedit_<version>_amd64.deb` for Debian 11+, Ubuntu 20.04+ and derivatives, and
    `mdedit-<version>-1.x86_64.rpm` for Fedora, openSUSE and RHEL: pick one with a package manager.
  - `PKGBUILD` for Arch: in a folder holding it, `makepkg -si` installs the tar.gz as `mdedit-bin`.
  - `mdedit-<version>-linux-x86_64.tar.gz`: `install.sh [PREFIX]` (default `~/.local`) and
    `uninstall.sh`, on any distribution.
- `mdedit-update.txt` is the manifest the updater reads.

The Windows installed copy, the bare exe and the portable zip's exe update themselves (the folder
must be writable), and so does the AppImage. The deb, rpm, tar.gz and Arch installs do not; the
app says only the AppImage can update itself. They install a desktop file, so the taskbar icon
and "Open with" work without the AppImage's first-run registration.

The AppImage is built in an Ubuntu 20.04 container, so it needs glibc 2.31 or later. It needs the
system's libc, fontconfig, freetype and the X11/xcb libraries, and loads libcurl and the Wayland
libraries at run time. On first run it writes its desktop entry and icon into your data folder and
registers its file types so mdedit shows under "Open with"; it does not take any default.
"Make mdedit the default" is a command in Settings. To remove it, delete the AppImage, the entry
it wrote under `~/.local/share/applications` and the icon under
`~/.local/share/icons/hicolor/256x256/apps`.

## Build

```powershell
cargo build --release --workspace
cargo test --release --workspace
target\release\mdedit.exe README.md
```

On Linux the same `cargo` commands work (the binary is `target/release/mdedit`). Releases are cut
by pushing a tag `v<version>`: GitHub Actions builds and publishes the files above. See
`docs/CI.md`; `docs/RELEASE.md` has the local scripts for a dry run.

`bash tests/shots.sh <dir>` renders the interface states to PNG without a window.

The mark in `assets/` is drawn by `assets/make_icon.py` in the viewer's repository
(`python assets/make_icon.py mdedit`), which holds the shapes of both marks.
