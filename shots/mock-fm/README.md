# mdfiles: a file manager (mockups, branch `mock/file-manager`, throwaway)

Drawn with mdedit's own painter, fonts, palette and shared widgets (`bins/mdedit/src/mock.rs` and
`mock/fm*.rs`, `mock/cmd.rs`, reached only by `--mock fm-<name>` and `--mock cm-<name>`).
Regenerate with `shots/mock-fm/make.ps1` after `cargo build --release -p mdedit`; every run is
`--behind --no-session` with `MDEDIT_SETTINGS` in `%TEMP%\mock-fm-settings`. Light and dark at
1280x800; `*-compact-*` is 860x600. The plan is `docs/file-manager-plan.md`; the plugin research
is `docs/commander-plugins.md`. Every folder, file, size and date is made up ("you" is nobody).
"mdfiles" is a working name.

## The standard file manager

| File | Shows | Why |
|---|---|---|
| `fm-main-*` | A project folder, details view: folder tabs, breadcrumb, command strip with one primary button (New), navigation pane with places and drives (free-space bars), three files selected, folder sizes as bars (one "counting", one not asked for), the status bar | The screen the owner lives in. Extensions shown, ISO dates, folders first |
| `fm-downloads-*` | Downloads grouped by day; an "online only" chip on a OneDrive placeholder, a badge on its icon | Group, and the cloud state that makes a copy or a delete surprising |
| `fm-split-*` | Split view: two panes, each with its own breadcrumb head; the active one has an accent line and the stronger selection; "Copy to other pane" and Compare appear in the strip | Split is a property of the tab, off by default (F3) |
| `fm-list-*` | The list view: names flowing in columns | The third view beside details and icons |
| `fm-icons-*` | Pictures as thumbnails with the preview pane open (image, its facts), a video tile, a folder that is online only | Icons, thumbnails, the preview pane |
| `fm-preview-*` | A Markdown file's preview, in mdview's renderer | The family's renderer in a pane |
| `fm-menu-tools-*`, `fm-menu-view-*` | The menu bar (Alt, or View > Menu bar) with Tools open, and View with its radio group, check items, and "Sort by" cascading | Full menus, as the owner asked; shortcut hints and access keys |
| `fm-context-*` | Right-click on three files: Open, Open with (cascade), Edit in mdedit, View in mdview, cut/copy/paste, copy as path, Copy to/Move to, Rename 3 files, Compress, Checksums, Properties, Delete; a last item "More options" (Shift+F10) | Option B of plan section 6: ours first, Windows' own behind a button |
| `fm-context-shell-*` | The same menu with the shell's entries (7-Zip, Git, Terminal, Defender, Share) appended under a disabled caption | Option C of plan section 6, to show what it would look like and how long it gets (21 rows) |
| `fm-context-split-*` | The menu in split view: "Copy to other pane F5" appears | |
| `fm-context-bg-*` | Right-click on the empty part of the folder: New (cascade: folder, text, Markdown, shortcut, zip), Paste, View, Sort, Group, Terminal here | |
| `fm-queue-*` | The transfers card: a running copy with speed and time left, a move waiting for the same drive, a copy to a NAS paused by the user and resumable, what finished today with Undo; the strip at the foot of the window is the collapsed form | Pause, resume, reorder, one job at a time per drive |
| `fm-conflict-*` | A file exists at the destination: both side by side with size, time and Newer/Older; Replace / Replace only if newer / Skip / Keep both; "do this for the other 11" | The conflict question, which is where file managers lose data |
| `fm-rename-*` | Batch rename: three ordered rules on the left, before and after on the right with the changed part marked red and green; one collision flagged and skipped; the button says how many will happen | The preview is the feature |
| `fm-delete-*` | Move to the Recycle Bin, with a warning that one file is online only (deleting removes it from the cloud) and that network and oversized items are deleted for good | The risk named before it happens |
| `fm-compare-*` | Two folders compared: a mark column (newer left/right, only here, differs, same), a strip with the counts and Copy to right / Synchronise | The compare mode |
| `fm-find-*` | Find in files and names: options bar, rows with the matching line and the match highlighted, the preview pane open | |
| `fm-settings-*` | Options > Browsing: the defaults as settings, with the reason in one line each | The defaults, argued in plan section 3 |

## The commander-style mockup (secondary)

Our own design, not a copy of any commander's look, names, icons or art.

| File | Shows |
|---|---|
| `cm-main-*` | Two panels, a drive row, a command line, the function-key bar (F1-F10), marked files in amber, tabs of panel pairs |
| `cm-vfs-*` | Left: a zip opened as a folder by a WCX plugin; right: an SFTP server as a folder by a WFX plugin, with a WDX column (Dimensions); a chip says which plugin type draws each panel |
| `cm-lister-*` | F3 on a database: lister tabs, the last one a WLX plugin's; a note that it runs in the host process |
| `cm-plugins-*` | The plugin manager: type, file, what it handles, state (allowed, 32-bit needs a helper, new file, hash changed), and the page of the selected one with hash, signed, limits |

## What drawing taught

1. **The shared `Menu` has no cascade.** Open with, Sort by, New and Group are second `Menu`s
   placed beside the row, with the arrow painted over by the caller. A file manager has five. A
   `submenu` item kind (and a caption row, for "Windows shell extensions") is needed in
   `mdshared-ui`, as a variant beside the old one.
2. **`chrome::draw_tab` draws a document glyph on every tab.** Folder tabs look wrong until it
   takes a glyph (a variant).
3. **`rows` refuses columns**, as its own comment says, and every state of this mockup is a
   table. The table view (columns, sort arrow, resize, multi-select, type-ahead, drag) is the
   first shared piece to build; mail will want the same one.
4. **Split view needs its own heads.** One address bar cannot name two folders, so each pane has
   a breadcrumb head and the address row says "Split view" (`fm-split`). The active pane needs
   two cues (the accent line and the stronger selection), because with two similar lists
   "which one is Delete going to act on" is the whole question.
5. **Folder sizes cannot be a column that fills itself.** Three states had to be drawn: not
   asked for, counting, and a number with a bar. The bar is what makes a size column readable
   at a glance (`shots` is 148.6 MB; `target` is not asked for).
6. **The shell's context menu is long.** Option C makes it 21 rows and it no longer fits under
   the pointer; it is pushed up over the command strip. That is a drawn argument for option B.
7. **The cloud state wants three places**: a chip in the row, a badge on the icon (small icons
   cannot carry the chip), and a sentence in the delete dialog. One of them is never enough.
8. **At 860 px** the Type column goes and the navigation pane becomes an icon rail in split
   view; the preview pane is 240 px instead of 300. A split window at 860 is two 290 px panes
   with only Name, Date and Size: usable, no more.
9. **The commander layout is the same table twice** with a different frame (function bar,
   command line, drive row). Drawing it took a fraction of the standard window, which is the
   argument for one engine and two layouts.
10. **A plugin's presence must be visible**: a chip for its type next to the path, a column that
    says it comes from a plugin, a state (allowed, new, hash changed) in the manager. Without
    them a panel that hangs on a remote server looks like a bug in the manager.

## Not drawn

Properties and the Hashes dialog, the Recycle Bin view (Original location, Date deleted,
Restore), the elevated-job prompt, a network share that does not answer, Open with's "Choose
another app" (the system's own dialog), the tab and pane head menus, drag and drop in flight.
