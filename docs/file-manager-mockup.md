# The mockup, from the simple file manager to the commander look

All 26 states in order, each in light and dark (left light, right dark). Everything is drawn by mdedit's own painter behind `--mock`; names, sizes and dates are made up. Context: `docs/file-manager-plan.md` (plan), `docs/commander-plugins.md` (plugin research), `shots/mock-fm/README.md` (what drawing taught). Regenerate with `shots/mock-fm/make.ps1`.

The path from simple to commander is one engine with two layouts: parts 1 to 4 are the standard window, part 5 is the same table twice with a different frame (function bar, command line, drive row). Plugins appear only in part 5.


---

## Part 1: the simple file manager


### 1. One folder, details view

The everyday window: folder tabs, breadcrumb, one primary button (New), places and drives on the left, three files selected, folder sizes as bars (`spikes` is counting, `target` not asked for). Extensions shown, ISO dates, folders first.

| Light | Dark |
|---|---|
| ![fm-main light](../shots/mock-fm/fm-main-light.png) | ![fm-main dark](../shots/mock-fm/fm-main-dark.png) |


### 2. Grouped, with a cloud file

Downloads grouped by day. `tax-papers.zip` is an OneDrive placeholder: a chip and a badge say it is not on this PC.

| Light | Dark |
|---|---|
| ![fm-downloads light](../shots/mock-fm/fm-downloads-light.png) | ![fm-downloads dark](../shots/mock-fm/fm-downloads-dark.png) |


### 3. List view

Names flowing in columns, for a folder of many small files.

| Light | Dark |
|---|---|
| ![fm-list light](../shots/mock-fm/fm-list-light.png) | ![fm-list dark](../shots/mock-fm/fm-list-dark.png) |


### 4. Icons and thumbnails, with the preview pane

Pictures as thumbnails, a video tile, a folder that is online only. The preview pane shows the selected image and its facts.

| Light | Dark |
|---|---|
| ![fm-icons light](../shots/mock-fm/fm-icons-light.png) | ![fm-icons dark](../shots/mock-fm/fm-icons-dark.png) |


### 5. Markdown preview

The preview pane renders Markdown with mdview's renderer.

| Light | Dark |
|---|---|
| ![fm-preview light](../shots/mock-fm/fm-preview-light.png) | ![fm-preview dark](../shots/mock-fm/fm-preview-dark.png) |


### 6. Small window (860x600)

Type column gone, navigation pane narrower. Same window, nothing else changes.

| Light | Dark |
|---|---|
| ![fm-main-compact light](../shots/mock-fm/fm-main-compact-light.png) | ![fm-main-compact dark](../shots/mock-fm/fm-main-compact-dark.png) |


---

## Part 2: menus


### 7. Menu bar, Tools menu

Alt (or View > Menu bar) shows File Edit View Go Tools Help. Tools holds batch rename, compare, sync, find, folder sizes, checksums, queue, terminal here.

| Light | Dark |
|---|---|
| ![fm-menu-tools light](../shots/mock-fm/fm-menu-tools-light.png) | ![fm-menu-tools dark](../shots/mock-fm/fm-menu-tools-dark.png) |


### 8. View menu with a cascade

Radio group for the view, check items for extensions and hidden files, and Sort by opening a second menu.

| Light | Dark |
|---|---|
| ![fm-menu-view light](../shots/mock-fm/fm-menu-view-light.png) | ![fm-menu-view dark](../shots/mock-fm/fm-menu-view-dark.png) |


### 9. Right-click on files (option B)

Our menu, with Open with as a cascade and a last row More options (Shift+F10) that opens Windows' own menu.

| Light | Dark |
|---|---|
| ![fm-context light](../shots/mock-fm/fm-context-light.png) | ![fm-context dark](../shots/mock-fm/fm-context-dark.png) |


### 10. The merged alternative (option C, not chosen)

Windows' entries appended under a caption: 21 rows, which is why B was chosen.

| Light | Dark |
|---|---|
| ![fm-context-shell light](../shots/mock-fm/fm-context-shell-light.png) | ![fm-context-shell dark](../shots/mock-fm/fm-context-shell-dark.png) |


### 11. Right-click on empty space

New (folder, text, Markdown, shortcut, zip), Paste, View, Sort, Group, terminal here.

| Light | Dark |
|---|---|
| ![fm-context-bg light](../shots/mock-fm/fm-context-bg-light.png) | ![fm-context-bg dark](../shots/mock-fm/fm-context-bg-dark.png) |


---

## Part 3: split view


### 12. Split view

Two panes in one tab, each with its own path, history and view. The active pane has an accent line and the stronger selection. Copy to other pane and Compare appear in the strip.

| Light | Dark |
|---|---|
| ![fm-split light](../shots/mock-fm/fm-split-light.png) | ![fm-split dark](../shots/mock-fm/fm-split-dark.png) |


### 13. Context menu in split view

Copy to other pane (F5) joins the menu.

| Light | Dark |
|---|---|
| ![fm-context-split light](../shots/mock-fm/fm-context-split-light.png) | ![fm-context-split dark](../shots/mock-fm/fm-context-split-dark.png) |


### 14. Split at 860x600

The navigation pane becomes an icon rail; two panes of about 290 px with Name, Date, Size.

| Light | Dark |
|---|---|
| ![fm-split-compact light](../shots/mock-fm/fm-split-compact-light.png) | ![fm-split-compact dark](../shots/mock-fm/fm-split-compact-dark.png) |


---

## Part 4: tools


### 15. Transfers: the copy queue

One running, one waiting for the same drive, one paused and resumable, finished jobs with Undo. The strip at the foot is the collapsed form.

| Light | Dark |
|---|---|
| ![fm-queue light](../shots/mock-fm/fm-queue-light.png) | ![fm-queue dark](../shots/mock-fm/fm-queue-dark.png) |


### 16. A conflict

Both files side by side; Replace, Replace only if newer, Skip, Keep both; one answer for the other 11.

| Light | Dark |
|---|---|
| ![fm-conflict light](../shots/mock-fm/fm-conflict-light.png) | ![fm-conflict dark](../shots/mock-fm/fm-conflict-dark.png) |


### 17. Delete, with the sharp edges named

Recycle Bin by default, with a warning that an online-only file is deleted from the cloud and that network or oversized items cannot go to the bin.

| Light | Dark |
|---|---|
| ![fm-delete light](../shots/mock-fm/fm-delete-light.png) | ![fm-delete dark](../shots/mock-fm/fm-delete-dark.png) |


### 18. Batch rename with a preview

Ordered rules on the left; before and after on the right, changes marked red and green, a collision flagged and skipped.

| Light | Dark |
|---|---|
| ![fm-rename light](../shots/mock-fm/fm-rename-light.png) | ![fm-rename dark](../shots/mock-fm/fm-rename-dark.png) |


### 19. Compare folders

Mark column (newer either side, only here, differs, same) and a strip with counts, Copy to right and Synchronise.

| Light | Dark |
|---|---|
| ![fm-compare light](../shots/mock-fm/fm-compare-light.png) | ![fm-compare dark](../shots/mock-fm/fm-compare-dark.png) |


### 20. Find by name and content

Rows with the matching line and the match highlighted, preview pane open.

| Light | Dark |
|---|---|
| ![fm-find light](../shots/mock-fm/fm-find-light.png) | ![fm-find dark](../shots/mock-fm/fm-find-dark.png) |


### 21. Options > Browsing

The defaults as settings, each with its reason.

| Light | Dark |
|---|---|
| ![fm-settings light](../shots/mock-fm/fm-settings-light.png) | ![fm-settings dark](../shots/mock-fm/fm-settings-dark.png) |


---

## Part 5: the commander look


### 22. Two panels, command line, function keys

Our own design: dense rows, marked files in amber, drive row, command line, F1-F10 bar. Not a copy of any commander.

| Light | Dark |
|---|---|
| ![cm-main light](../shots/mock-fm/cm-main-light.png) | ![cm-main dark](../shots/mock-fm/cm-main-dark.png) |


### 23. Archives and servers as folders (plugins)

Left: a zip opened by a packer plugin (WCX). Right: an SFTP server by a file-system plugin (WFX), with a content-plugin column (WDX). A chip names the plugin type.

| Light | Dark |
|---|---|
| ![cm-vfs light](../shots/mock-fm/cm-vfs-light.png) | ![cm-vfs dark](../shots/mock-fm/cm-vfs-dark.png) |


### 24. A lister plugin

F3 on a database; the last tab is a viewer plugin (WLX) running in the plugin host.

| Light | Dark |
|---|---|
| ![cm-lister light](../shots/mock-fm/cm-lister-light.png) | ![cm-lister dark](../shots/mock-fm/cm-lister-dark.png) |


### 25. The plugin manager

Type, file, what it handles, state (allowed, 32-bit needs a helper, new file, hash changed) and the selected plugin's hash and limits.

| Light | Dark |
|---|---|
| ![cm-plugins light](../shots/mock-fm/cm-plugins-light.png) | ![cm-plugins dark](../shots/mock-fm/cm-plugins-dark.png) |


### 26. Commander at 860x600

The same layout in a small window.

| Light | Dark |
|---|---|
| ![cm-main-compact light](../shots/mock-fm/cm-main-compact-light.png) | ![cm-main-compact dark](../shots/mock-fm/cm-main-compact-dark.png) |
