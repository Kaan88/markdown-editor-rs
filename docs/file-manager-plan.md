# A file manager for the family: one plan

Written 2026-11-22 for the owner to approve. "You" is the owner. Nothing here is built; "mdfiles"
is a working name (question 1). The pictures are on the branch `mock/file-manager`,
`shots/mock-fm/` (standard file manager, 19 states, light and dark, six also at 860x600) and
`shots/mock-fm/` `cm-*` (the commander-style mockup); this plan names them by file. The plugin
research is its own document, `docs/commander-plugins.md`; section 14 is its verdict.

**The brief, in your words (2026-11-22).** "Don't want a commander style file manager. Maybe in
the future. Standard file manager with useful defaults and tools, menus and context menus is what
I want, split view support too."

**What I read.** `CLAUDE.md`, `docs/PLAN.md`, `docs/CODE.md`, `docs/ecosystem-ideas.md` §21 (the
file manager, priced there at 7-10 person-weeks as a two-pane keyboard app) and §5 (the photo
culler "as a mode of the file manager"), `docs/terminal.md`, the launcher mock's README and
`docs/launcher-plan.md` (as the model), `ai-library-plan.md` (as the shape). In `markdown-shared-rs`
the public items of `mdshared_ui` (`gfx`, `menu`, `rows`, `tree`, `pane`, `scroll`, `segments`,
`overlay`, `chrome`, `platform`) and `mdshared_app` (`job`, `watch`, `places`, `foldersearch`,
`files`, `fuzzy`, `sister`). `Cargo.lock` of `main`.

**What I ran.** The mockup, to make the shots (all looked at, section 15 says which). Nothing
else: no file operation, no shell API, no measurement. Every figure in sections 7 and 8 is a
**target or an estimate**, marked so; none is a measurement. Nothing was run on a folder of 100,000
files.

---

## 1. The recommendation

Build it, as **a standard, Explorer-shaped file manager with tabs and an optional split view**, on
the family's shared crates, **Windows first**, in **about 65 working days** (13 weeks), in nine
phases that each leave something usable. The first useful slice is **about 18 days** (section 11).

Three decisions carry the rest:

1. **One engine, two layouts.** The data model (a folder as a streaming, sorted list of entries),
   the file-operations core (a queue of jobs) and the tools are written once. The standard window
   is the first layout. A commander layout (`cm-main`) is a second layout on the same engine,
   about 10-12 days later, if you ask for it. That is what "maybe in the future" costs: nothing
   now, if the engine is built with two panes in mind. It is, because split view needs it.
2. **Our own context menu, the Windows one behind a button.** `fm-context` is ours (fast, themed,
   knows mdedit and mdview). Anything the shell adds (7-Zip, Git, Defender) is behind **More
   options** (Shift+F10), shown by Windows itself. Section 6 prices the alternative of drawing the
   shell's entries in our menu (+5 days, more risk) and recommends not doing it first.
3. **Safe by construction before fast.** The risk that ends a file manager is lost data
   (`ecosystem-ideas.md` §21). So the file-operations core is written first among the heavy parts,
   as its own shared crate, tested on real volumes, and nothing deletes for good without a
   question.

What it refuses is in section 2. What it costs against the budgets is in section 8.

## 2. What it is, and what it refuses to be

It is: browse folders and drives, open files, copy / move / rename / delete, with a queue that can
pause and a clear question for every conflict; tabs; **split view** (two folders side by side in
one tab, off by default, F3); details, list and icon views; preview pane; find by name and by
content; batch rename with a preview; compare and sync two folders; hashes; folder sizes; "Open
with"; the Recycle Bin; archives opened as folders (zip first).

It refuses:

- **Replacing Explorer.** It does not register itself as the default folder handler, does not
  take over Win+E, and does not change a file association (a system setting: it opens the
  system's own "Default apps" page). It is the app you open on purpose.
- **Being a shell.** No desktop, no taskbar, no tray, no network neighbourhood browser beyond
  what a path gives (`\\nas\photos` and mapped drives are paths).
- **Cloud clients.** OneDrive, Dropbox and the like are folders with placeholder files; it knows
  they are placeholders (section 7) and never makes one real by accident. It does not sync.
- **An ACL, ownership or registry editor.** Properties shows attributes and size; "Security"
  opens the system's own dialog.
- **Third-party code in the window**, by default: no plugins in the standard layout (section 14).
- **Network use.** None, except its own updates through Hub (`docs/launcher-plan.md`).
- **Thumbnails for every format.** What Windows can thumbnail (section 7), plus what the family
  already draws (images, Markdown, source). No codecs of its own.

## 3. The window, and the defaults argued

Anatomy, as drawn in `fm-main`: title row with **folder tabs**; the address row (back, forward,
up, refresh, the path as a **breadcrumb you can click to edit**, search); the command strip (**New**
as the one primary button, cut / copy / paste / rename / share / delete, sort, group, view; in
split view also Copy to other pane and Compare); the **navigation pane** (places, drives with
free-space bars, Network, Recycle Bin); the pane(s); the status bar (count, selection, free space,
view switch). Menus: a **menu bar** (File Edit View Go Tools Help) that Alt shows and View >
Menu bar keeps (`fm-menu-tools`, `fm-menu-view`); mdedit has the same one. At 860 px the Type
column goes and the navigation pane narrows; in a split window it becomes an icon rail
(`fm-main-compact`, `fm-split-compact`).

The defaults, each with the reason and what would change it:

| Setting | Default | Why |
|---|---|---|
| File extensions | **Shown** | A name is what it is. Explorer hides them and `invoice.pdf.exe` is the oldest trick there is. |
| Hidden / system files | **Hidden**; Ctrl+H toggles it for the window, and the status bar says when it is on | Matches Explorer, and the thing you wanted off. |
| Order | **Folders first, natural order** (`file2` before `file10`), case ignored, per window | Natural order is what people expect; Explorer's is natural too. |
| Sizes | **KB = 1024**, one decimal, folders "—" until computed (`fm-main`) | Windows labels 1024 as KB; the other reading ("kB (1000)") is one setting away. |
| Folder sizes | **When I ask** (Calculate folder sizes, or hover); a counting folder shows "counting" and a bar when done | A size means walking the tree: 148 MB `shots` is cheap, `target` is not. Never on open. |
| Dates | **2026-11-22 14:07** | Unambiguous, sorts as text; "System" is a setting. |
| Click | Double click opens | Single click is a setting. |
| **Remember the view of each folder** | **Only if I set it** (View > Remember for this folder) | See below. |
| On start | **Where I left off**: tabs, both panes, selection | The session is the family's pattern (`session.rs`). |
| Delete | **Recycle Bin**, with Undo; "Delete permanently" is a separate item and always asks | Section 7. |
| Confirm delete | On for permanent delete and for anything the bin cannot hold; off for the bin (it is undoable) | A confirmation you always click through teaches you to click through. |

**Remember the view per folder: decided against as the default, and argued.** Explorer remembers
per folder (the "Bags" in the registry) and then guesses a folder's *type* from its contents
("optimise for Pictures") and switches the view under you. Both are the most complained-about
behaviours of the app. A global default view plus sort, per window, is predictable. Folders you
care about (Pictures\Trip as icons, Downloads grouped by date) are set once with "Remember for
this folder", stored in the settings file keyed by path, capped (500 paths, oldest dropped) with
`mdshared_app::places::Places`, which already does that for caret positions. Reopen the question
if you find yourself setting it more than twice a week.

**Split view.** Two panes inside one tab, F3 or the button (`fm-split`): each pane has its own
path, history, view and selection, the active one carries an accent line, Tab switches, and
"Copy to other pane" / "Move to other pane" are in the strip and in the context menu
(`fm-context-split`). Dragging between panes copies (Ctrl: copy, Shift: move, default: move on
one volume and copy across, as Explorer). The address row names the split rather than repeating a
path: each pane has its own breadcrumb (`fm-split`). Split is a property of a **tab**, so two
tabs can differ. It is not the default; the first run opens one pane.

## 4. The tools

| Tool | What it does | Shot | Where it lives |
|---|---|---|---|
| **Copy queue** | Jobs run one at a time per destination drive; pause, resume, cancel, reorder; per-file conflict question with "replace if newer / skip / keep both" and "do this for the other N" (`fm-conflict`); verified copy optional; finished jobs listed with Undo where undo exists | `fm-queue`, `fm-conflict` | shared crate `mdshared-fileops` + shared queue card |
| **Batch rename** | Ordered rules (replace with regex, case, extension, counter, date taken); **live preview** with the changed part marked in red and green, collisions flagged and skipped, one undo | `fm-rename` | app; the rules engine in the shared crate (mdedit's library can use it) |
| **Compare / sync** | Two folders, by name, size, time or content hash; marks per row (newer left, newer right, only one side, differs); copy the differences either way; a sync is a plan you read first, in the same plan card the AI and the library use | `fm-compare` | shared crate (the walk and the plan), app (the view) |
| **Find** | By name and by content, in a folder and below; results as rows with the matching line (`fm-find`); skips `target`, `.git`, `node_modules` like the editor | `fm-find` | `mdshared_app::foldersearch`, extended to name search and non-text |
| **Folder sizes** | On demand, on a worker, cancelled by leaving; sizes of folders are cached for the session | `fm-main` | shared |
| **Hashes** | SHA-256 (already in the lock via `sha2`) and CRC32; on selection, in a dialog, on a worker | context menu | shared |
| **Terminal here** | Opens the family terminal if it exists (`docs/terminal.md`), otherwise Windows Terminal, otherwise `cmd` | Tools menu | app; one function |
| **Archives as folders** | zip (read first, write later) opens as a folder; extract = copy out. See price below | — | shared |
| **Preview pane** | Markdown through mdview's renderer, text and source through mdedit's highlighter, images, and a summary for several items (`fm-preview`, `fm-icons`) | `fm-preview` | `mdview-render` behind a feature, as mdedit does it |
| **Open with** | The handlers Windows lists for the type, plus mdedit and mdview first; "Choose another app" uses the system's dialog | `fm-context` | app, `SHAssocEnumHandlers` |

**Archives, priced.** zip needs the `zip` crate (new; pulls `flate2` which is in the lock, plus
`zopfli`/`bzip2` features you can leave off; **to be priced with `cargo tree` when you say yes**).
7z and rar are not in scope for the standard layout; they are the plugin question (section 14).

## 5. Menus and context menus

Four menus are drawn (all with the shared `Menu`, so they are the family's look, with access
keys and shortcut hints): the **menu bar menus** (`fm-menu-view` shows View with a checked-radio
group, check items, and Sort by as a cascade), the **file context menu** (`fm-context`), the
**background context menu** (`fm-context-bg`: New > Folder / Text file / Markdown file / Shortcut,
Paste, View, Sort, Group, Open terminal here, Properties) and the **tab** and **pane head** menus.

**What drawing taught about the shared `Menu`.** It has no cascade: `Menu<T>` is one list, so
Open with, Sort by and New are drawn as a second `Menu` placed beside the row, with the arrow
painted over the row by the caller (three shots do it). A file manager has five cascades, so the
shared menu needs a `submenu` item kind and hover-delay logic: about 2 days, in `mdshared-ui`, as
a variant beside the old one (rule 8). Menu items also lack a **caption** row ("Windows shell
extensions" in `fm-context-shell` is painted by hand).

## 6. The Windows shell context menu: whether and how

The shell menu is what puts **7-Zip, Git, Defender, "Share", cloud clients and the owner's own
tools** into Explorer's right-click. They are COM objects (`IContextMenu`, `IContextMenu2/3`)
registered under `HKCR\*\shellex\ContextMenuHandlers` and loaded **into our process** when asked.
Three options:

| | A: ours only | **B: ours + More options (recommended)** | C: shell entries drawn in our menu (`fm-context-shell`) |
|---|---|---|---|
| What you see | The family menu, nothing else | The family menu; "More options… Shift+F10" opens **Windows' own menu** at the pointer | One menu: ours, then a captioned "Windows shell extensions" part |
| Cost | 0 | **~2 days** | **~7 days** (B + 5) |
| Needs | nothing | `SHParseDisplayName`, `IShellFolder::GetUIObjectOf`, `QueryContextMenu`, `TrackPopupMenuEx`, `InvokeCommand`; COM initialised **lazily**, on first use | all of B, plus reading the `HMENU`: `GetMenuItemInfo` for text, state, bitmap, submenus; owner-drawn items; `WM_INITMENUPOPUP` for lazy submenus ("Send to", 7-Zip's); `HandleMenuMsg2` |
| Risk | none | A handler hangs or crashes: **in our process**. Explorer has the same exposure and an out-of-process shell host we do not | The same, **more often** (every right-click loads every handler), and a drawn menu must keep every special case right or the item does nothing |
| Launch path | untouched | untouched (nothing before the first use) | untouched, but each right-click waits for the slowest handler |
| Look | one | a native light menu in a dark window unless the app opts the process into dark menus (`SetPreferredAppMode`, undocumented; **unverified on this PC**) | one |

**Why B.** Our menu is instant and always right; the shell's menu is slow, themed by Windows and
sometimes wrong. B keeps the fast path ours and gives you every third-party verb **when you ask
for it**, for 2 days. C is the Windows 11 move reversed: Explorer put "Show more options" in on
purpose, because loading every handler on every right-click is slow. We should not pay that in
the common case. **Mitigation for a hanging handler** (applies to B and C): the menu is requested
on the UI thread because `HMENU`s are thread-bound, so a handler that blocks blocks the window;
we show the Windows busy cursor, never at launch, and document the exposure rather than pretend
it away. A crash inside a handler is an access violation, not a Rust panic, and ends the process;
the session file brings the tabs back (as for mdedit's own crash recovery). If this proves too
brittle, A is the fallback and loses only third-party verbs. Reopen: if you use B daily and never
use our menu's shell-less version, do C.

**Shift+right-click** shows the shell menu directly, as Explorer does.

## 7. Windows plumbing

| Subject | What happens | Decision |
|---|---|---|
| **Listing** | `std::fs::read_dir` on Windows takes the size, times and attributes from the find data, with no second call per file; a worker reads in batches and sends them to the window | `FindFirstFileExW` with `FIND_FIRST_EX_LARGE_FETCH` and `FindExInfoBasic` is the faster call, if `read_dir` is too slow in the 100k test (section 8); decide by measuring |
| **Long paths** (over 259 chars) | Rust's `std::fs` adds the `\\?\` prefix itself for absolute paths; the shell APIs (`IContextMenu`, `SHFileOperation`, drag and drop) do not take it | The app manifest sets `longPathAware`; our file code uses `std`, and anything that goes to the shell is shortened or refused with a message that says why |
| **Reserved names** (`CON`, `NUL`, trailing dot or space), case-insensitive names | New name and rename are checked before the call, with a plain message | Rename preview flags them (`fm-rename`) |
| **Junctions and symlinks** | Shown with a link badge; **copy copies the link, not the target**; delete removes the link only; "follow link" is a menu item | Recursive walks never follow a link (the editor's folder search already refuses); the delete of a directory junction uses `remove_dir`, not a recursive delete, to stay clear of the old `remove_dir_all` class of bugs |
| **Network shares** | `\\nas\photos` and mapped drives; a `stat` can block for tens of seconds on a dead server | Every filesystem call is on a worker; the pane says "waiting for \\nas…" with Cancel; never `exists()` on the UI thread; drives are listed lazily; change notification on SMB is unreliable, so the status bar offers Refresh and the view re-lists on focus |
| **OneDrive placeholders** | Files with `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` or `RECALL_ON_OPEN` are not on the PC: opening one **downloads it**, and deleting one deletes it **from the cloud** | A badge and an "online only" chip (`fm-main`, `fm-downloads`); **never** preview, hash, thumbnail or search the content of one without asking; copy asks "this will download 24 MB"; delete says what it deletes (`fm-delete`) |
| **Permissions and UAC** | We run as you, never elevated. "Access denied" is a per-item answer in the queue ("Skip / Retry / Retry as administrator") | "Retry as administrator" starts a **second, elevated copy of the job runner** (`mdfiles --job <file>`, `runas`), not the window; the window never runs elevated (an elevated window would also refuse drops from a normal one: UIPI) |
| **Recycle Bin** | The `trash` crate behind `mdshared_ui::platform::recycle` is already in use (`trash 5.4.0`, in the lock) | Used for Delete. It cannot hold files from network or removable drives, or larger than the bin; the delete dialog **says so before it happens** and asks (`fm-delete`), never silently deletes for good |
| **Copy** | A hand-written read/write loop loses alternate data streams (the "downloaded from the internet" mark), security info and sparse-ness | `CopyFileExW` (or `CopyFile2`) with a progress callback: it is also where **pause** is implemented (the callback blocks) and **cancel** (it returns). Large files use `COPY_FILE_NO_BUFFERING`. Verify is a separate, optional hash pass |
| **Move** | Same volume: `MoveFileExW`, atomic and instant, **no copy** | Different volume: copy, verify the size, then delete the source, in that order, and **only then**; a failure never deletes the source |
| **Watching** | `mdshared_app::watch` (the `notify` crate, already in the lock) watches folders and coalesces wakes | A buffer overflow (a burst of 10,000 changes) must mean "re-list this folder", not "apply events"; the debounce is 150 ms; a folder that is being re-listed keeps its selection by name |
| **Clipboard** | `CF_HDROP` plus "Preferred DropEffect" (copy or move), so Explorer and mdfiles can paste each other's cut and copy | `mdshared_ui::platform` already has clipboard text; the file formats are new (~1 day) |
| **Drag and drop** | OLE `IDropSource` and `IDropTarget`. The shared window layer has **no drag and drop of files** today | The one real new Windows layer: see the `windows` crate below |
| **Thumbnails and icons** | `IShellItemImageFactory::GetImage` (what Explorer uses; WIC, Office, PDF handlers, all of it) on 4 workers, visible rows only, an LRU of ~500 | No codec of our own. Per-extension icons are cached, so 100,000 `.rs` files cost one icon; only `.exe`, `.lnk`, `.ico` ask per file |
| **File associations** | Read through `SHAssocEnumHandlers` and `AssocQueryString`; opened with `ShellExecuteExW` | Never written. "Always use this app" is the system's own dialog |

**The `windows` crate.** Drag and drop, `IContextMenu`, `IShellItemImageFactory` and
`IFileOperation` are COM, and COM from `windows-sys` (which the family uses) means writing the
vtables by hand: about 600 lines of unsafe code and every one of them yours to get wrong. The
`windows` crate (**0.62.2, already in `Cargo.lock`, pulled by `trash`**) gives them as safe-ish
bindings. So the price of using it directly is **no new entry in `Cargo.lock`**; what it adds is
the features we turn on (`Win32_UI_Shell`, `Win32_System_Ole`, `Win32_System_Com`) and a heavier
compile of our own code. Exe size and the launch path: **not measured**; both are small because
LTO drops what is unused and nothing runs before first use. **Rule 7: I am asking, not adding.**
(question 4)

## 8. Performance: a folder of 100,000 files

Targets, not measurements; the phase that builds each says it must measure them (rule 1).

| What | Target | How |
|---|---|---|
| First frame of the window, any folder | under **100 ms**, the family's figure | The window draws before the first listing arrives; the listing is a worker |
| First rows of a 100k folder | under **150 ms** after the folder is chosen | Entries arrive in batches of 4,096; the first batch is painted unsorted, then re-sorted |
| Full listing, sorted, 100k files, warm cache | under **600 ms** | One name arena (a `String` and offsets), a 48-byte record per entry (name offset, size, times, attributes, kind); lower-case sort keys built once |
| Memory for 100k entries | about **8 MiB** over an empty window (estimate: 100k x (48 + ~24 name) bytes), with a **budget of 60 MiB private** in all, since a file manager has thumbnails the editor does not | Measured with the editor's method (`docs/CODE.md`, "Measuring"), and the budget is set when the first measurement exists |
| Scroll frame | under **4 ms** at 1280x800 | Only the visible ~40 rows are drawn (`mdshared_ui::rows` already does the arithmetic); icons and thumbnails are lookups, never calls |
| A keystroke (type-ahead, 100k) | under **8 ms** | A binary search over the sort keys, not a scan |
| Idle CPU | **zero** | No timer except a pending job and the watcher's coalesced wake |
| Window with 3 tabs, 2 panes each | no listing for a tab that is not shown | A hidden tab keeps its list but does not watch (it re-lists when shown) |

The honest unknown: **how fast `read_dir` is on 100,000 files on this PC**; the figure above for
the listing is a target from how Windows is built, not an observation. The first thing phase 1
does is measure it, on a synthetic folder, and the answer decides whether `FindFirstFileExW` is
needed.

## 9. What goes where: the app and the shared crates

Rule 8: whatever two apps could use starts shared, as a variant beside the old function. The
table says where each piece lives. "New" crates are in `markdown-shared-rs` and add **no
third-party dependency** except where marked.

| Piece | Lives in | Status | Notes |
|---|---|---|---|
| Painter, palettes, chrome, tabs, menus, segments, dialogs, scrollbars, progress | `mdshared-ui` | exists | `draw_tab` draws a document glyph on every tab; a folder tab needs a glyph argument (variant) |
| **Menu cascades and caption rows** | `mdshared-ui::menu` | **new variant**, 2 days | section 5 |
| **Table view**: columns with resize, sort arrow, multi-select, keyboard, type-ahead, inline rename, drag source | `mdshared-ui::table` | **new**, 6-7 days | `rows` has the arithmetic and refuses columns; mail will want it too (`ecosystem-ideas.md` §2 and §21 both said "the list layer is missing") |
| **Icon grid** (tiles, thumbnails) | `mdshared-ui::grid` | **new**, 2 days | the launcher mock drew one; sharing it saves the second implementation |
| Breadcrumb bar with edit mode | `mdshared-ui::crumbs` | **new**, 1.5 days | |
| Folder tree (navigation pane) | `mdshared-ui::tree` | exists | the notes pane uses it; needs drive roots and a "no rules" mode (variant) |
| **File operations**: copy, move, delete, queue, pause, conflicts, verify, undo journal | `mdshared-fileops` | **new crate**, ~12 days | headless, tested on real volumes, uses `mdshared_app::job` for the worker; no window |
| The plan card (confirm a batch: rename, sync, delete) | `mdshared-ui` (plan card, exists) | exists | mdedit's library move uses it |
| Watching, places (remembered views), fuzzy, folder search, single instance, settings file, sister hand-off, updater | `mdshared-app` | exists | `foldersearch` needs a name-only mode and binary files |
| Windows shell: context menu, clipboard files, drag and drop, thumbnails, Open with | `mdshared-ui::platform` (Windows part) | **new**, ~9 days | behind `cfg(windows)`; Linux gets `xdg-open` and the file manager's own list |
| The window, the layouts, the tools' dialogs, the settings pages | the app (`bins/mdfiles`) | new | `mdedit`'s Settings page layout is the model |

A crate for the app itself: `bins/mdfiles` in a **new repository** (`Kaan88/markdown-files-rs`,
question 2), like mdview and mdedit, pinned to the shared crates by `rev` (`docs/CODE.md`).
`mdview-render` and `mdview-preview` come in behind a feature, as mdedit does it.

## 10. Testing

- **The file-operations core** is where the project's credibility is. Tests run on a scratch tree
  made for the purpose: junctions, symlinks (needs the privilege or developer mode), long paths,
  read-only files, a locked file, a path with `CON` in it, names that differ only by case, a
  file that disappears mid-copy, a destination that fills up (a small VHD mounted in the test, on
  Windows CI only), a UNC path to a local share (`\\localhost\c$` works on the runners). Each
  test names the loss it prevents, and **fails with its fix reverted** (rule 2).
- **A property test** for sort order and for rename rules: whatever the input, the preview's
  "after" is exactly what the rename does, and no rule produces a name two files share without
  flagging it.
- **Journal tests**: kill the job runner at every step of a move (after copy, after verify,
  before delete); the source must exist after every one.
- **Shots** (rule 6): every state in this plan's list, both themes, 1280x800 and 860x600, in
  `tests/shots.sh`; and **tests that drive the app and compare pixels** for what changes while you
  point (hover, drag, resize, selection in split view).
- **Windows-only APIs** (shell menu, drag and drop, thumbnails) are tested by hand and by a smoke
  test on the Windows runner; nothing counts as working until it ran on Windows (rule 13).
- **Budgets**: `tools/compare-builds.py` extended with a 100k-file folder and a scroll test.

## 11. Phases, days, and the first useful slice

Working days, including tests and shots, estimates by judgment (no prototype). A phase ends with
something you can use.

| # | Phase | Days | What you have at the end |
|---|---|---|---|
| 0 | **Measure first**: a 100k-file folder, `read_dir` against `FindFirstFileExW`, sort time, the table's scroll; decide the listing call | 1 | The figures in section 8 become measurements |
| 1 | **Browse**: window, tabs, breadcrumb, navigation pane, **table view** (shared), streaming listing, sort, selection, keyboard, open, status bar, watching, session | 9 | A fast read-only Explorer: `fm-main`, `fm-downloads` |
| 2 | **Everyday operations**: `mdshared-fileops` (copy, move, delete to the bin, rename, new folder), conflicts, the queue (no pause yet), clipboard, Undo | 10 | **The first useful slice, with 1 and 3a: ~18 days.** You can live in it for a day |
| 3 | **Menus**: menu cascades (shared), the menu bar, the file and background context menus, Open with, **More options** (option B), properties | 6 | `fm-context`, `fm-menu-*` |
| 4 | **Views and preview**: list, icons, thumbnails (shell), groups, the preview pane (Markdown, source, image, several) | 7 | `fm-icons`, `fm-preview`, `fm-list` |
| 5 | **Split view**, drag and drop (OLE) within and between windows, copy to other pane, the queue's pause and resume | 8 | `fm-split` |
| 6 | **Tools**: batch rename 3, find 3, folder sizes 1.5, compare and sync 4, hashes 1.5, terminal here 0.5, zip as a folder 3 | 16.5 | `fm-rename`, `fm-find`, `fm-compare` |
| 7 | **Windows hardening**: long paths, network shares, OneDrive placeholders, the elevated job runner, junctions, the real-volume test set | 6 | The risks of section 13, closed or named |
| 8 | **Settings, packaging, Hub, docs**, DECISIONS, STATE, the release notes | 4 | A release candidate |
| | **Total** | **~67** | about 13 weeks, one person |

Phase 0 is inside phase 1's budget in the total (the sum is 66.5; phase 0 is the first day of
phase 1's nine); the table keeps it separate because it can end the plan early if the listing is
much slower than expected.

**First useful slice: phases 1 and 2 plus the context menu's first cut, about 18 days.** A window
with tabs, the table, the navigation pane, copy, move, rename, delete to the bin, new folder,
open, a plain context menu, session restore. No split, no tools, no thumbnails, no preview. It is
enough to find out whether the speed and the defaults are right, which is the question worth
answering before the other 49 days.

**Two-pane alternative, priced for comparison** (what `ecosystem-ideas.md` §21 priced): a
keyboard-first dual pane with no tabs, no thumbnails, no shell menu, 7-10 person-weeks. The
standard window with split view costs more (about 13) because of the shell integration, the
views and the tools; the engine it shares with the commander layout is the same.

## 12. The commander layout, later (secondary mockup)

`cm-main` draws it in the family's own look: two panels, a drive row, a command line and a
function-key bar (F3 View, F5 Copy, F6 Move, F7 Folder, F8 Delete), marked files in amber, the
cursor as a bar, no icons beyond a folder and a file. It is **not a copy of any commander's look,
names, icons or art**. On the engine of sections 9 and 11 it is about **10-12 days**: a layout,
the marking keys (Insert, `+`/`-`, `*`), the function bar, a command line that runs in the
current folder, the quick-search box, the directory hash list, and the panel-specific options.
It would be a setting ("Layout: Standard / Commander") or a second binary mode, decided when it is
asked for. The standard window is not compromised by it: nothing in sections 3-11 depends on it.

## 13. Risks

| Risk | What it would do | What we do |
|---|---|---|
| **Lost data in a move, copy or delete** | Ends the project | Section 7's order (copy, verify, then delete); the journal; the kill-at-every-step test; the bin by default; a confirmation only where it means something; **no feature that deletes in bulk without a plan card** |
| A shell handler hangs or crashes (option B/C) | The window freezes or closes | Lazy, on request; session restore; documented; fallback to option A |
| 100k files slower than the targets | The headline fails | Phase 0 measures first; the listing is streamed so the window is never waiting |
| The `windows` crate (COM) | A new direct dependency and a heavier compile | Already in the lock; asked (question 4); behind `cfg(windows)` and lazy |
| The shared table is bigger than 7 days | The first slice slips | It is the one piece that gates everything; a first version without drag and inline rename is 4 days, the rest in phase 5 |
| Thumbnails of cloud placeholders | A "view" downloads gigabytes | Never without asking (section 7); tested with a fake placeholder attribute |
| Users expect Explorer parity | Disappointment | Section 2 says what it refuses; the first run says so once |
| Linux and macOS | The shell parts are Windows only | The standard window builds on all three (the window, table, tools), shell integration is `cfg(windows)`, and Linux gets `xdg-open` and the desktop's own trash; Mac later |

## 14. Plugins for a commander layout: the verdict

Full research: `docs/commander-plugins.md` (read from primary sources on 2026-11-22; what could
not be verified is marked there). In short:

- **WCX (packers) and WDX (content) are cheap** (loader 2 days, WCX read-only 5, WDX 4), need
  **no new dependency** (`LoadLibraryW` and `GetProcAddress` from `windows-sys`, already used;
  `libloading` 0.9.1 would add `windows-link`), and fit a hand-drawn UI.
- **WFX (file systems) is large** (12-15 days; threads, credentials, timeouts, virtual folders).
  **WLX (listers) wants a native child window inside our softbuffer window**: Windows only,
  fragile (8-10 days), the `cm-lister` shot shows where it would sit.
- **In-process third-party DLLs are the risk**, not the ABI: a crash is the manager's crash
  (`panic = "abort"` does not help against an access violation), and plugins are unsigned,
  often abandoned, often 32-bit (a 64-bit process cannot load them: a 32-bit helper is another
  5-6 days).
- **Recommendation:** built-in zip, then an MC-style external-command protocol (`list` /
  `copyout`, ~5 days, out of process by nature), and **only if asked**: WCX read-only + WDX behind
  a **plugin host process** with a hash allow-list (`cm-plugins`): about **23 days** in all.
  Refuse WLX, WFX and 32-bit plugins by default. Full Total Commander compatibility: 55-65 days,
  not recommended.
- **Counts** (totalcmd.net, 2026-11-22): 847 plugins in the four types (WLX 265, WCX 217, WFX
  185 on the first page only, WDX 180); the useful ones are about fifteen (7-Zip, UnRAR, SFTP,
  MediaInfo, SQLite and a few more), most with 64-bit builds. Not verified: their licences.
- **Licensing** of the plugin API headers is **not** established; do not ship Total Commander's
  headers or an SDK in the repository until it is.

## 15. The shots

All in `shots/mock-fm/`, `-light` and `-dark`, 1280x800 unless the name says `-compact` (860x600).
README there says what each is for. Regenerate with `shots/mock-fm/make.ps1`.

## 16. Decisions to record if you approve

- A standard, Explorer-shaped file manager with tabs and an optional split view; commander layout
  later on the same engine, only when asked.
- Defaults of section 3, in particular: extensions shown, hidden files hidden, folders first in
  natural order, sizes in KB = 1024, folder sizes on request, ISO dates, Recycle Bin with Undo,
  **view remembered only on request**.
- The context menu is ours; Windows' own is behind More options (option B). **Approved by the owner, 2026-11-22.**
- `mdshared-fileops` is a new shared crate; the table, the grid and the crumbs go in
  `mdshared-ui`; menu cascades are a variant beside the old menu.
- The window never runs elevated; an elevated job runner is the only elevation.
- No plugins in the standard layout; plugin policy as section 14.

## 17. The owner's questions

1. **Name.** "mdfiles" is a working name. Something shorter that fits the other three?
2. **Where does it live?** A new repository like mdview and mdedit (recommended), or a new binary
   in this one?
3. **ANSWERED 2026-11-22: B.** (The shell context menu: B or C?) B (ours first, Windows' behind More options, 2 days) is
   recommended. C (one merged menu, +5 days, a slower right-click) only if you want one menu.
4. **May the app depend on the `windows` crate directly** for COM (drag and drop, shell menu,
   thumbnails)? It is already in `Cargo.lock` through `trash`, so it adds no new entry; what it
   adds in the exe and compile time is not measured yet. The alternative is hand-written
   vtables with `windows-sys` (about 600 lines of unsafe).
5. **zip as a folder: may I add the `zip` crate** (new, priced with `cargo tree` before it goes
   in)? Without it there are no archives in the first version.
6. **Is split view per tab** (recommended) or one per window?
7. **Remember the view of each folder: only when I ask** (recommended), or Explorer's way?
8. **Is a second, elevated job runner acceptable** for "Retry as administrator" (the window itself
   never elevates)? Or never elevate at all and say "run as administrator yourself"?
9. **Terminal here:** depends on whether the terminal in `docs/terminal.md` is built. Until then it
   opens Windows Terminal. OK?
10. **Cloud files:** first-class (badges, asking before a download, 3 days of the plan) or best
    effort (plain files, no badges)? OneDrive is what makes a naive copy surprising.
11. **Commander layout and plugins:** keep both for "the future", or put the layout (10-12 days) on
    the road map now and leave plugins out entirely?
12. **Hub:** is the file manager one more row in the launcher (`docs/launcher-plan.md`), and does
    it share an installer folder with the others?
