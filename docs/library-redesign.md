# The notes library: redesign plan

Written 2026-11-21 for the owner to approve or reject. Nothing in it is built. Every statement
about how the app behaves today comes from the code, with the file and line.

Read before writing this: `CLAUDE.md`; `docs/HANDOFF.md` ("The notes library", "The notes library
in the left pane, and nested headings"); `docs/PLAN.md` (budgets, "Notes library");
`docs/DESIGN.md` §11; `markdown-shared-rs/crates/mdshared-app/src/library.rs` (all 794 lines);
`mdshared-ui/src/{pane,menu,tree}.rs`; mdedit's `library_pane.rs`, `app/library.rs`, `palette.rs`,
`overlays.rs`, `panes.rs`, `chrome.rs`, `commands.rs`, `app/commands.rs`, `app/input.rs`, `ui.rs`,
`app/replace_files.rs`, `replace_ui.rs`; `bins/mdedit-ai/src/{proto,keys,api}.rs`, mdedit's
`ai.rs` and `app/ai_events.rs`.

---

## 1. What the library is for

The library is a short, hand-kept index of the notes worth coming back to, wherever they sit on
disk. It is not a file browser (the Files pane is) and not a search (Find in files is). Three
things must be fast:

1. **Open a note by name** — Ctrl+Shift+N, type three letters, Enter. Already works
   (`C::Library`, `commands.rs:387`; the list is built in `palette.rs:656-692`).
2. **Keep a note found while working** — one command from the open document, no dialog chain.
   Already works (`library_add`, `app/library.rs:92-115`).
3. **Browse what exists** — the tree in the left dock, folded where you left it
   (`library_pane.rs:94-151`).

What it must not become: a second file manager, a tag system, a database, or anything that scans
in the background. The format is Markdown the owner can open and edit in this very editor; that
is the feature, not an implementation detail. Nothing may run at launch: the library is read only
when it is first shown (`app/library.rs:19-20`), and that must stay true (first-frame budget,
`docs/PLAN.md:317`).

---

## 2. How other tools do this, and what fits here

| Tool | What it does | What it teaches mdedit |
|---|---|---|
| **VS Code Explorer** | Right-click: New File, New Folder, Reveal in File Explorer, (sep) Cut/Copy/Paste, Copy Path, Copy Relative Path, (sep) Rename `F2`, Delete `Del`. Rename is an inline box in the tree with the stem pre-selected. | Copy the menu **order**: create → reveal → clipboard → rename → destructive last, separated. Copy inline rename and `F2`. |
| **Obsidian file explorer** | New note, New folder, Rename, **Move file to…** (a picker, no dragging needed), Reveal in system explorer, Delete (to system trash). | "Move to…" as a picker is the cheap, keyboard-reachable alternative to drag and drop. Deletes go to the trash, never `unlink`. |
| **Obsidian Bookmarks pane** | A second pane, separate from the file tree, holding links to files, folders, headings and searches, in groups you make. Stored in `.obsidian/bookmarks.json`. | A curated pane *beside* a filesystem tree is a normal, accepted shape. Reject the JSON: our index is Markdown and that is better. |
| **Zed project panel** | Same menu as VS Code plus full keyboard: arrows, Enter opens, `F2`, `Del`, and the panel takes focus. | Keyboard in the pane is table stakes. Ours has none at all today. |
| **Sublime sidebar** | New File, Rename, Delete, **Reveal in Side Bar**, Find in Folder. | "Reveal in Side Bar" — a command that scrolls the pane to the active file — is cheap and missing here. |
| **Notepad++ Folder as Workspace** | A docked folder, no project file; per-file Open, Rename, Delete, Copy path. | The owner's reference app (`docs/PLAN.md:26`). Minimal is acceptable; what matters is that right-click does *something*. |
| **Logseq / Dendron** | Hierarchy encoded in the filesystem (`proj.area.topic.md`); refactor commands rename files and rewrite every link. | A structured folder only works if rename-and-fix-links is a real subsystem. That is weeks, not days. Reject at this depth. |
| **Apple Notes / Bear** | Folders (one per note) plus tags (many per note). No user-visible filesystem. | One note in several places is useful — and our link model already allows the same note in two groups for free. Do not break that by making groups exclusive folders. |
| **Zotero collections** | Items live once in storage; a collection holds references. The menu has **"Remove Item from Collection"** and **"Move Item to Trash"** as two separate, differently-worded items, and only the second asks. | The closest model to what mdedit already is, and the answer to the most dangerous question in this redesign: two verbs, two wordings, one confirmation. Copy it exactly. |

Conventions adopted from the above: `F2` renames, `Delete` removes from the library (safe),
`Enter` opens, arrows navigate, `Left`/`Right` fold and unfold, `Shift+F10` and the Menu key open
the row's menu, right-click selects the row under the pointer first, destructive items last and
separated, "Copy path" and "Reveal in File Explorer" always present. Multi-select and drag and
drop are discussed in §3 and are **not** in the first cuts.

---

## 3. The redesign

### 3.1 The toolbar button

Today the bar draws Open, Reload, Recent, Files, Preview on the left and More, Theme, Find
prev/next on the right (`chrome.rs:896-938`). There is no library button.

- Add `BarItem::Library` to `ui.rs:78-89`, drawn immediately after `BarItem::Files` inside the
  `if plan.panes` block (`chrome.rs:901-917`), glyph `BOOKMARK` (the library's icon everywhere
  else: `overlays.rs:60,168,240`, `library_pane.rs:222`), label `"Notes"`, lit when
  `ui.panes.files && ui.library.on`. Click → `C::ToggleLibraryPane` (`app/commands.rs:68-78`).
- Tooltip: "Notes library (Ctrl+Shift+3)", built by the `with(...)` helper at `chrome.rs:268-277`.
- `plan_width` gains `labelled(t.library, l) + GAP` in the `plan.panes` branch
  (`chrome.rs:144-148`), and `BarText` a `library` field (`chrome.rs:118-125`, filled in
  `bar_text`, `chrome.rs:446`).
- **Narrow widths:** the button follows the Files button exactly. Labels drop at step 2 of
  `bar_plan` (`chrome.rs:169-172`), so it becomes an icon; the whole `panes` group — Files,
  Notes and the Preview slot — disappears at step 4 (`chrome.rs:178-183`). At that width the pane
  is still on Ctrl+Shift+3 and in the View menu (`overlays.rs:240`). No new degrade step.
- **The honest cost:** a labelled button is `44 + text` px (`chrome.rs:132-134`); "Notes" is
  roughly 37 px at 13 px UI, so the full-label plan needs about **81 px more** than today. The
  window width at which labels drop rises by that much. At 1280×800 (the shot size) nothing
  changes; near 1100 px the labels go sooner. `bar_degrades_in_order` (`chrome.rs:1147`) and
  `bar_fits_at_the_window_minimum` (`chrome.rs:1167`) must be re-run — I have not built, so this
  is arithmetic, not measurement.
- The function list gets **no** bar button. It is per-document, it has Ctrl+Shift+2, and three
  pane buttons on one bar is clutter.

### 3.2 Context menus in the pane

There are none today: `right_press` (`app/input.rs:896-975`) handles tabs, the minimap, the text
area and the find box, and falls through on everything else (`app/input.rs:972`). The Files pane
has none either. Two new popup kinds are needed, `PopupKind::LibraryRow(usize)` and
`PopupKind::LibraryEmpty` (`ui.rs:422-448`), built by a new `overlays::library_row_menu`, with
one arm in `right_press`. A right-click sets the pane's cursor to the row under the pointer
first, as the text menu moves the caret (`app/input.rs:932-941`).

**Every write obeys the rules `library_put` already set** (`app/library.rs:130-170`): a library
file open in a tab with unsaved changes is never written (toast `library_unsaved`); the new text
is re-parsed and compared with the old before it is written; the write is
`settings::write_atomic`. Those three lines are the safety model of this whole document.

**Group row** (a library file; `Kind::Group`, `library_pane.rs:25`):

| Item | Key | What it does on disk |
|---|---|---|
| &New note here… | | Asks for a name in the inline box; creates `<name>.md` in the group file's folder with `# <name>`, adds a link line to this group, opens it in a tab |
| &Add the open document | | `library_put(file, &[])` — today's code path. Greyed when the active tab has no file or is already listed (`app/library.rs:103-112`) |
| New &section… | | Inline box; appends `## <name>` at the end of the file |
| — | | |
| &Rename group | `F2` | Rewrites the file's first `# heading` (that heading *is* the group name, `library.rs:78-79,159-164`). The file name is untouched |
| Rename the library &file… | | Renames `<x>.md` → `<y>.md`. Changes sort order only (`library_files` sorts by name, `library.rs:139`) |
| — | | |
| &Edit this library file | `Ctrl+Enter` | Opens **this one file** in a tab (see §3.5) |
| &Copy path / Reveal in File &Explorer | | The library file's path. Same code as the tab menu (`overlays.rs:387-401`) |
| — | | |
| Remove this &group… | | Confirmation dialog, then deletes the `.md` file. The dialog says "the N notes themselves are not touched" and names them by count |

**Section row** (`Kind::Section`):

| Item | Key | What it does on disk |
|---|---|---|
| &New note here… | | As above, link written under this heading path via `add_note` (`library.rs:461`) |
| &Add the open document here | | `library_put(file, section)` |
| New &subsection… | | Writes a heading one level deeper, under this one |
| — | | |
| &Rename section | `F2` | Rewrites that one heading line, keeping its `#` count, so the nesting is unchanged |
| &Move section to… | | Group/section picker (the palette, `library_places`, `palette.rs:696-729`); cuts the heading and everything under it to the target, re-levelled |
| — | | |
| &Go to this heading | | Opens the library file at that line |
| — | | |
| Remove this &section… | | Confirmation naming how many links go with it. Refuses silently on nothing |

**Note row** (`Kind::Note { path, missing }`):

| Item | Key | What it does on disk |
|---|---|---|
| &Open | `Enter` | Nothing. Today's click behaviour (`app/library.rs:55`) |
| Open in the other &pane | | Nothing (`C::ShowInOtherPane`) |
| — | | |
| &Rename in the library | `F2` | Rewrites **the link's text only** (`[Title](path)`). The file on disk keeps its name. Label says so |
| &Move to… | | The group/section picker; removes the line here, `add_note`s it there. Two writes, one undo step, refused unless both files are writable and unopened-dirty |
| &Find the file… | | Only when `missing`: a file picker, then the link's target is rewritten. The repair for a note moved in Explorer |
| — | | |
| &Copy path / Reveal in File &Explorer | | The note's path |
| Go to its &line in the library file | | `Entry.line` already exists for this (`library.rs:69-70`) |
| — | | |
| Remove from the &library | `Delete` | Deletes that one line. No question asked: it is one line, and Undo-in-the-file is a keystroke away once the file is open |
| Delete the file on &disk… | `Shift+Delete` | **See the question in §8.1.** Default: this item does not exist |

**Empty space** (`PopupKind::LibraryEmpty`):

New &group…  ·  &Add the open document…  ·  —  ·  &Edit a library file…  ·  Reveal the library
&folder  ·  —  ·  Re&fresh

**Rename: inline, not a dialog.** The app has no text-entry dialog at all (`DialogKind`,
`ui.rs:577-591`, is buttons and one ticked list), so a rename dialog would be the first of its
kind and cost at least as much as doing it properly. Inline is what VS Code, Obsidian and Zed do.
Mechanically: a `TextField` (`mdshared_ui::textfield`) held by `LibraryPane`, drawn over the row,
plus a new `BarField` variant with arms in `field()`, `field_mut()`, `focus_field()` and `hit()`
(`ui.rs:36-56, 848-925`) and key routing in `app/input.rs`. `Enter` commits, `Esc` cancels, a
click elsewhere commits. The same field serves five items (rename group, rename section, rename
note, new group, new section, new note) — one widget, six uses.

### 3.3 The head buttons, and switching the three faces

Today the dock has three faces, one at a time (`app/commands.rs:1750-1763`): the Files tree
(`panes.rs:20`), the function list (`outline.rs:239`), the library (`library_pane.rs:159`). Their
heads differ only in a word — "FILES", "FUNCTIONS", "NOTES" (`commands.rs:508,568`,
`outline.rs:239`) — and each draws one 28 px button at the same place, `r.right() - 38`:

- Files pane: `glyph::FOLDER_OPEN` → `Hit::FilesFolder` → `C::OpenFolder`, the folder picker
  (`panes.rs:24-29`, `app/commands.rs:108`).
- Library pane: `glyph::FOLDER` → `Hit::LibraryFiles` → `C::ToggleFiles`, back to the tree
  (`library_pane.rs:164-169`, `app/commands.rs:116`).

Two near-identical folder icons, same pixel, two unrelated meanings. The owner is right.

**Proposal.** Replace the head's word with three face switches at its left, and make the
right-hand button belong to the face shown:

| Face | Head switch | Right-hand button |
|---|---|---|
| Files | folder icon, lit when shown | `FOLDER_OPEN` → Open folder… (unchanged) |
| Functions | `FUNCTIONS` icon | none |
| Notes | `BOOKMARK` icon | `PLUS` → the empty-space menu (New group / New note / Add this file) |

The lit switch is `pal.text` with a 2 px `pal.accent` underline; the others `pal.muted`.
`Hit::PaneFace(Face)` replaces `Hit::LibraryFiles` and `Hit::OutlineFiles`, which are retired
(`ui.rs:210,213`).

**Why switches in the head and not a dropdown or three bar buttons:** `draw_head` already owns
the top 40 px of the pane and writes one short label at `r.x + 16` (`pane.rs:15-26`); replacing
that label with three 28 px hit targets is a small, local change in one shared function, and it
is where every editor named in §2 puts the same control. A dropdown costs a popup and a click
more for the commonest switch. Three bar buttons was rejected in §3.1.

**Width.** `PANE_MIN` is 160 px (`ui.rs:18`); the default is 300 (`session.rs:769`). Three
labelled switches ("Files", "Functions", "Notes") plus the action button do not fit in 160 px.
So the head degrades exactly as the bar does: labels while the pane is wider than about 210 px,
icons with tooltips below that. That is the `plan.labels` pattern already in
`bar_plan` (`chrome.rs:159-206`), applied to a 40 px head.

The faces stay on Ctrl+Shift+1/2/3 (`commands.rs:361-367`) and in the View menu
(`overlays.rs:236-241`); the bar's Notes button opens the pane when it is closed, which the head
switches cannot do. That is why both exist.

### 3.4 Drag and drop inside the tree

**Not in the first cuts; probably not at all.** The mechanism is not alien — tab reorder is
already a hand-rolled drag with a movement threshold (`Drag::Tab { index, x, moving }`,
`ui.rs:667-672`) — but a tree drop needs: a press threshold, an insertion line drawn between
rows, auto-scroll at the pane's top and bottom edges while held, drop-validity rules (a note into
a group or section; a section into another group is a multi-line move; a group into itself must
be refused; a section into its own descendant must be refused), the two-file write when the drop
crosses groups, and `Esc` to abandon mid-drag. With tests at this project's bar that is **3 to 4
person-days**, and it delivers what "Move to…" delivers for about a tenth of that, while being
unreachable from the keyboard.

Recommendation: ship "Move to…" (§3.2), use the pane for a fortnight, and only then decide. If it
is built later, notes only, never sections.

### 3.5 What "Edit the library" should do

Today `library_edit` (`app/library.rs:190-219`) collects every group file and opens all of them:

```rust
for file in files {
    self.open(&file);
}
```

The last `open` wins the active tab, so with `personal.md` and `work.md` in the folder the owner
lands in `work.md` — exactly the complaint. With the 64-file limit (`library.rs:42`) it can open
64 tabs.

It should open **one** file, chosen in this order:

1. the group of the pane's cursor row, if the pane is showing;
2. the group of the note in the active tab, if that file is listed;
3. the only group, when there is only one;
4. otherwise the group picker (the palette, the shape of `library_groups`, `palette.rs:685-692`),
   and open the one chosen.

Keep the existing behaviour for an empty folder: create `library/notes.md` from `TEMPLATE` and
open it (`app/library.rs:203-215`). Rename the menu item to "Edit a library file…"
(`overlays.rs:176`), and add "Edit this library file" to the group row's menu, which is where it
is actually wanted.

### 3.6 The rest of what "finished" means

- **Empty states.** Today the pane writes one grey line, `"No notes yet"` or "There is no folder
  for the notes library" (`library_pane.rs:179-187`, `commands.rs:507,509`). Give it the Files
  pane's treatment: a line plus a real button (`panes.rs:41-49`), here two — "New group…" and
  "Add this file".
- **Keyboard in the pane.** None exists: `Hit::LibraryRow` and `Hit::FilesRow` are reachable only
  by click (`library_pane.rs:197`, `panes.rs:96`). The results panel is the only focusable list
  and shows how (`ui.results.set_focus(true)`, `app/commands.rs:118-122`;
  `replace_dialog_key`, `app/replace_files.rs:138-162`). Add: a cursor row, `Up`/`Down`,
  `Left` fold-or-parent, `Right` unfold-or-first-child, `Enter` open, `F2`, `Delete`,
  `Shift+F10`/Menu for the row menu, `Esc` back to the text, and Ctrl+Shift+3 focusing the pane
  when it already shows. Without this the context menus are mouse-only, and the project has an
  access-key test (`app/access_tests.rs:78`) that will want every new menu marked.
- **A note's folder.** Rows show the title alone. Add a dimmed, right-aligned folder name, as
  `recent_menu` does (`overlays.rs:91-102`). It answers "which `taxes.md` is this" for the price
  of one `text()` call per visible row.
- **"Show this file in the notes library"** on the tab menu and in the palette: opens the pane and
  scrolls to the note. Sublime's "Reveal in Side Bar". Cheap.
- **Sort order.** Groups are in file-name order (`library.rs:139`), notes in written order. Keep
  it: the file is the truth and the owner edits it by hand. Offer "Sort the notes in this section
  by name" as a one-off command that **rewrites the file**, not a view setting — a view-only sort
  makes the pane disagree with the file it is showing.
- **Search inside the pane.** A one-line filter box in the head that narrows rows to matching
  titles and keeps their parents. Worth having, but late: Ctrl+Shift+N already searches titles and
  file names across every group (`palette.rs:656-692`), so the pane's filter is for browsing, not
  finding.
- **The pane at start.** It is not restored (HANDOFF, "Not done"). The session already keeps
  `files` and `files_w` (`session.rs:264,558`, DESIGN §11); add which face was showing. One line
  each side.
- **Pinned / recent notes:** no. See §7.

---

## 4. The filesystem question

The owner's question, restated: *my notes are disorganised outside mdedit; should the library own
a structured folder, and what does that cost?*

Today nothing is moved or copied: a library file holds link lines, a relative path resolves
against the library folder (`library.rs:389-400`), and a note that is not on disk is shown as
missing rather than dropped (`library.rs:166`, `library_pane.rs:235`). The only writer in the
whole shared module is `add_note` (`library.rs:461`).

### (a) Keep links only

- **`mdshared_app::library` changes:** add text-in/text-out writers beside `add_note` —
  `remove_note(text, line)`, `rename_note(text, line, title)`, `rename_section`, `add_section`,
  `remove_section`, `set_group_name`, `new_group(name)`. Each is verified by re-parsing, as
  `library_put` already verifies `add_note` (`app/library.rs:157-170`). About 250 lines plus
  tests in the existing `library::tests` module.
- **Cost:** 1.5 person-days. It is a prerequisite for §3.2 in *every* option, so it is not
  really a cost of this option.
- **What breaks:** nothing. mdview reads the same files; the format is unchanged.
- **Notes shared with mdview:** unaffected. mdview does not read the library yet (HANDOFF, "Not
  done"), and when it does it reads the same text.
- **Rename/move/delete outside the app:** the note becomes `missing` and stays visible; the repair
  is "Find the file…" (§3.2), one picker.
- **Failure modes:** two notes with the same file name are fine — the paths differ and the titles
  may repeat. No operation spans two files except "Move to…", which is two atomic writes; if the
  second fails the note is in neither place, so it must write the *target* first and the removal
  second, and say so if the removal fails.

### (b) A managed folder the library owns

- **Changes:** a home folder; `move_in(path)` that moves a note under `home/<group>/` and rewrites
  the link as a relative path; renames that move files; deletes that go to the recycle bin;
  collision naming (`taxes-2.md`); a rule for files sitting in the home folder that no group
  lists (shown how? the library then *is* a file browser, which is the Files pane's job); and
  some way to notice changes made outside the app.
- **Cost:** 6 to 9 person-days. The move engine itself is small; the cases are not — cross-volume
  moves are copy-then-delete and not atomic, a tab open on a moving file holds a stale path, and
  each needs a test.
- **Plus a dependency:** deleting to the recycle bin needs the `trash` crate or a `windows`-crate
  `IFileOperation` call, with nothing equivalent on Linux without a freedesktop trash
  implementation. `docs/PLAN.md` treats new dependencies as the owner's decision.
- **What breaks:** every path to those notes held anywhere else — Explorer shortcuts, mdview's
  recents, and wiki-links, which `mdshared_app::notes` resolves against a vault root
  (`notes.rs:10-14`). Moving files changes which wiki-links resolve.
- **Failure modes:** a half-finished move across volumes (copied but the link not rewritten, or
  the reverse) needs a journal, not just an atomic write; two same-named notes landing in one
  folder; a note open in a tab while it moves — `Docs::check_changed` will see the file gone and
  mark the tab missing (DESIGN §11, "Changes on disk").

### (c) Hybrid: a group may declare a home folder

- **Changes:** one new line kind in a library file, e.g. `home: D:/notes/work`, read by `parse`.
  The format tolerates it already — anything that is not a heading or a note line is ignored
  (`library.rs:18-19, 307-320`) — so an mdview that does not know the line keeps working.
  A note added to a group with a home may be moved into it by an **explicit** "Move the file
  here" item, never automatically; a note whose path is already under the home is written as a
  relative link.
- **Cost:** 2 days for the format, reader and writer; plus about 2 days for the single-file move
  with its cases. Four days for something useful. No watcher, no bulk import, no delete.
- **What breaks:** nothing by default. A group with no `home:` behaves exactly as today, which is
  the only reason this option is interesting: it can be tried on one group.
- **Failure modes:** the same as (b) but one file at a time, with the owner having asked for each
  one.

### Recommendation

**(a) now, (c) later if the owner still wants it, never (b) wholesale.** The owner's actual pain —
"it is disorganised outside of mdedit" — is an *import* problem, not a *storage* problem: what is
missing is a way to find out what those files are and get them indexed (§5), not a new place to
keep them. Option (a) plus the AI import tidies the index; option (c) then lets the owner move the
files for one group at a time, with an undo, after they have seen what the groups should be. Going
straight to (b) spends a week building a move engine for a layout nobody has decided on yet.

**What I need from the owner to be sure:** whether the files should physically move at all, or
whether a tidy index over a messy disk is enough; and if they should move, where that folder lives
and whether it is synced (OneDrive, Dropbox — a sync client renaming or re-creating files under
the app changes every failure mode above).

---

## 5. AI for library management

### 5.1 What the engine can do today — read this before judging the design

`bins/mdedit-ai` speaks one JSON object per line (`proto.rs:1-6`). The requests are exactly:
`hello`, `detect`, `models`, `run`, `cancel`, `key_set`, `key_clear`, `key_has`
(`proto.rs:17-59`). The events are `hello`, `delta`, `done`, `error`, `models`, `detected`, `key`.

`Run { id, conn, model, effort, system, prompt }` (`proto.rs:31-42`) is the whole of it: one
system string, one prompt string, one answer. `done` carries the finished text
(`proto.rs:232`); `delta` carries a **character count only** (`proto.rs:228`). There is no tool
use, no JSON mode, no multi-turn, no attachment, and the engine never touches the filesystem
except the OS credential store for keys (`keys.rs:1-6`). Connections: the Claude Code and Codex
CLIs, the Claude API, and any OpenAI-compatible endpoint (`proto.rs:61-86`).

On mdedit's side, `ai_start` (`app/ai_events.rs:294-356`) builds one run with
`core_ai::SYSTEM` — which is rewrite-shaped ("Reply with the rewritten text only",
`crates/mdedit-core/src/ai.rs:294-299`) — and `core_ai::prompt`, which wraps the text in `<text>`
tags (`mdedit-core/src/ai.rs:311-313`). One run at a time (`Ai::run: Option<Run>`, `ai.rs:399`).
The cap on what is sent is `AI_TEXT_MAX = 200_000` bytes (`app/ai_events.rs:15,302`). It is off
unless the owner turns it on (`ai_can_start`, `app/ai_events.rs:234-241`; `docs/PLAN.md:298-300`).

**Consequence: no new engine request is needed.** Organising a folder is a sequence of ordinary
`run` calls with a different `system` and a prompt the app builds — and the answer is text the app
must validate itself, exactly as it already validates a rewrite by re-parsing before writing.

Two small additions are worth making, neither required:

1. **An optional `json: true` on `run`** (`proto.rs:31-42`, `VERSION` bump at `proto.rs:15`), so
   the Claude API and OpenAI-compatible endpoints can be asked for strict JSON. The CLI
   connections cannot honour it, and the app must validate the text either way, so this is a
   nicety.
2. **Sequential batches, not concurrency.** `Ai::run` is a single `Option` and the status strip
   assumes one run. Leave it so: 13 runs one after another for 500 files is a minute or two of
   waiting with a progress line, and a queue in `Ai` is a refactor with no user-visible gain.

### 5.2 Reading the folder, and the batching

- **The scan** runs on a worker, as Find in files does (`mdedit_core::search`, used from
  `results.rs:12`), with the caps `mdshared_app::notes::Config` already defines for exactly this
  job — `max_files`, `max_file_bytes`, `max_total_bytes`, `max_depth`, `max_entries`
  (`notes.rs:27-41`). Hidden folders skipped, symlinks not followed.
- **Per file a digest, never the file:** the path relative to the scanned folder, size, modified
  date, and the first run of text — the first heading if there is one, then up to **1200 bytes**
  of body, whitespace collapsed, 2000 bytes hard cap. A title and a first paragraph classify a
  note; the rest does not pay for itself.
- **Arithmetic** (an estimate, not a measurement): 2000 bytes ≈ 500 tokens, so ~550 tokens a file
  with its metadata. The existing 200 KB prompt cap would hold about 90 digests. I propose
  **40 files a batch** (~25 KB of prompt, ~6 K tokens in, ~2 K out), so a batch that fails is
  cheap to retry. **500 files = 13 runs**, roughly 80 K input and 26 K output tokens for the whole
  folder. The app should show the owner *files and runs* before it starts; it cannot honestly show
  a price.
- **Two passes, because batch 2 must not invent "Finance" where batch 1 said "Money":**
  - *Pass 1, categories.* Path + title + 120 bytes for every file, in as many runs as the cap
    needs, asking for a proposed set of **at most 8 groups with at most 4 sections each**. If pass
    1 took more than one run, one short merge run reconciles the lists.
  - *Pass 2, assignment.* Batches of 40 digests, with the fixed category list sent in every batch.
    Anything the model names outside that list is rejected by the validator and becomes
    `Unsorted`.
- **Conversion to Markdown is separate and off by default.** 500 `.txt` files would be 500 runs
  and real money, and most of them need nothing but a `.md` name and a `# Title`. Three levels:
  *none* (default — the library links any file, `library.rs:339-359`), *name only* (no model call
  at all), *full* (one run per file through the existing `core_ai::FORMAT` path,
  `mdedit-core/src/ai.rs:302`, only for files the owner ticks).

### 5.3 What the model must return

One JSON object per batch and nothing else:

```json
{"version": 1,
 "files": [
   {"id": 17, "group": "Work", "sections": ["Clients"],
    "title": "Acme contract notes", "confidence": 0.8, "why": "invoices and delivery dates"}
 ]}
```

- `id` is the index the app gave that file **in that batch**. Paths never come back from the
  model, so a hallucinated path can never reach the filesystem. This is the single most important
  safety property of the design.
- `group` and `sections` must be members of the list that was sent; otherwise `Unsorted`.
- `title` is capped at 120 characters and stripped of newlines and brackets before it can enter a
  link line (`link_line` already strips `[` and `]`, `library.rs:410`).
- `confidence` orders the preview and nothing else. `why` is shown as a row hint and is never
  written to disk.
- **The validator** takes the first balanced `{…}` in the reply (models add prose whatever they
  are told — `fit_end`, `mdedit-core/src/ai.rs:317`, exists for the same class of problem),
  checks `version`, checks every `id` is in this batch and unique, and drops everything else. A
  batch that will not parse is retried once with a stricter instruction; a second failure marks
  those files `Unsorted` and the run carries on. One bad batch never aborts the operation.

### 5.4 The preview, before anything is written

Reuse the shape that Replace in files already proved: plan on a worker → a ticked list the owner
confirms → apply on a worker → a report → one undo (`app/replace_files.rs`, whole file;
`replace_ui.rs:312-330`; `mdedit_core::replace_files::{apply, revert}`,
`replace_files.rs:682,744`). Nothing touches disk before the button.

The plan is too big for a dialog, so it goes in the **results panel**, which already has rows,
scroll, selection, a focus model and a report mode (`results.rs`): a tree of group › section ›
file, every row ticked, each file showing its proposed title and its first line as the hint.
Controls: untick a file, untick a whole group, "only above 0.6", and a row menu item "Put this one
somewhere else…" using the same group/section picker as §3.2. Buttons: **Add N notes to the
library**, **Cancel**, and, when conversion is on, a tick "Also convert the N ticked text files".

### 5.5 What is written, in what order

1. **The record first**, before anything else: `library/.organise/<timestamp>.json`, holding every
   intended change. Without the record there is no undo.
2. **Conversions**, if asked, one file at a time: `name.md` written **beside** the original, never
   over it; a name that exists gets `-2`. The `.txt` is not touched, not moved, not deleted.
3. **Library files**, one `write_atomic` each: every `add_note` for that group applied to the text
   in memory, the result re-parsed and checked before the write (`app/library.rs:157-170`). A
   group file that does not exist yet is created from `TEMPLATE` (`library.rs:51-55`).
4. **The record is closed** with what each step actually did, and a hash of each file written.
5. The pane rebuilds; the panel shows the report: N notes into M groups, K files converted,
   L skipped and why.

### 5.6 Undo, and the safety measures

**Undo** is one command — "Undo the last library change", in the Notes library menu, enabled while
a record exists. It removes exactly the lines that were added, matched by path and line text
rather than line number, and deletes exactly the `.md` files the conversion created, and only
where the file is byte-identical to the hash in the record. Anything changed since is left alone
and named in the report. That is the same contract as
`mdedit_core::replace_files::revert`, which refuses a file whose backup no longer matches
(`replace_files.rs:744-768`).

The checklist, all of which must hold:

- **Off unless the AI connection is on** (`app/ai_events.rs:241`). The menu item is greyed with
  "Turn on the AI connection in Settings" otherwise.
- **Nothing is read outside the folder the owner picked.** Depth cap, entry cap, per-file byte cap
  (1 MiB), total-bytes cap, hidden folders skipped, symlinks not followed.
- **A hard cap of 500 files a run**, refused above it with the count, so a stray pick at `C:\`
  cannot start a thousand-dollar job.
- **Only digests leave the machine**, and the confirmation shows how many bytes and how many runs
  that is — the project's rule is that only the text a command is run on goes
  (`docs/PLAN.md:298-300`), and the owner must be able to check it.
- **Nothing is overwritten.** Conversion writes new names only.
- **Nothing is deleted and nothing is moved** in option (a). The word "organise" means "indexed",
  and the confirmation must say so in those words.
- **Every library write** goes through the re-parse check and `write_atomic`; a library file open
  with unsaved changes stops the run and names it (`app/library.rs:130-139`).
- **Offline or no key:** the engine answers `error` with kind `network` or `auth`
  (`proto.rs:91-97`) and the run stops with the existing bar and its "Try again"
  (`ai.rs:300-313`). The plan made so far is kept, so a retry only re-runs the missing batches.
- **Nonsense from the model:** per-batch validation, one retry, then `Unsorted`.
- **Cancel** stops between batches; the engine's `cancel` op already exists (`proto.rs:43-45`).

### 5.7 Build first, and the gold-plating

**First:** the scan, the digest, pass 1, pass 2, the validator, the plan in the panel, the link
writes, the record and the undo. That is the whole of the value. No conversion.

**Gold-plating, and why:**

- Per-file conversion runs — expensive, and most files do not need it. Phase 5, ticked files only.
- "Learn the style of my existing library" — a longer prompt for a gain nobody can measure.
- A watcher that offers to file new files as they appear — surprise plus cost per run.
- Confidence thresholds beyond one checkbox.
- Embeddings or a local model for classification — a new dependency and a new failure mode to
  save a few cents a run.

**"Modify existing libraries"**, which the owner also asked for, mostly exists already and should
be said plainly: a library file *is* Markdown, so opening it and running **AI: Rewrite with
instruction** on it ("group these links by topic, keep every link exactly as written") goes
through the review panel that already shows before and after and applies or discards
(`ai.rs:343-365`). The only thing missing is a saved prompt for it and a check that the result
still parses as the same set of links — about a day, after §3.2's writers exist. A bespoke
"reorganise my library" feature on top of that would be gold-plating.

---

## 6. The plan

Person-days for one maintainer working with AI agents, tests included, at this project's bar
(a test per behaviour, each failing with its fix reverted; fmt; clippy both ways; the budgets
re-measured where the launch path or a frame is touched). Ordered so the cheap wins land first
and the product is coherent after every phase.

| # | What ships | Days | Needs an answer first |
|---|---|---|---|
| 1 | "Edit the library" opens one file; toolbar Notes button; the dock's three head switches | 3 | No |
| 2 | Context menus in the pane, the shared writers behind them, inline rename | 6 | **Yes** (Q1) |
| 3 | Keyboard in the pane, empty states, folder shown per note, "Show in the library", pane face restored at start | 3 | No |
| 4 | AI organise: scan, digests, two passes, validator, plan panel, link writes, record, undo | 8 | **Yes** (Q3, Q5) |
| 5 | Conversion to Markdown, ticked files only | 3 | **Yes** (Q4) |
| 6 | Hybrid home folder (option c) and "Move the file here" | 4 | **Yes** (Q2) |
| 7 | Drag and drop in the tree | 3–4 | **Yes** (Q6) |

**Phase 1 — the three complaints that cost almost nothing (3 days).**
Touches `app/library.rs` (`library_edit`, 190-219), `chrome.rs` (`BarItem`, `plan_width`,
`bar_plan`, `bar_text`, the draw), `ui.rs` (`BarItem`, `Hit`), `panes.rs`, `outline.rs`,
`library_pane.rs` (heads), `mdshared-ui/src/pane.rs` (`draw_head` gains the switches),
`app/commands.rs` (the `Hit::PaneFace` arm), `overlays.rs:176` (the renamed item).
New tests: `edit_the_library_opens_one_file_and_says_which`; the bar degrade tests extended with
the new button; head-switch hit tests at 160 px and 300 px; the access-key test for the renamed
item. Owner sees: `--shot` of the bar with the Notes button and of the dock's head in all three
faces, light and dark.

**Phase 2 — context menus (6 days).**
The shared writers in `markdown-shared-rs/crates/mdshared-app/src/library.rs` (`remove_note`,
`rename_note`, `rename_section`, `add_section`, `remove_section`, `set_group_name`, `new_group`),
each text-in/text-out with tests in `library::tests`. Then mdedit: `PopupKind::LibraryRow` and
`LibraryEmpty` (`ui.rs`), `overlays::library_row_menu`, the `right_press` arm
(`app/input.rs:916`), the `Act` variants, the handlers in `app/library.rs`, the inline field
(`LibraryPane` + a `BarField` variant + key routing), and the group/section picker reusing
`library_places` (`palette.rs:696`). New tests: one per write, each checking the file's text and
that the pane re-reads; a library file open with unsaved changes refuses; a failed re-parse writes
nothing; the access-key test for both new menus. Owner sees: `--menu library-row`,
`--menu library-empty`, and a shot mid-rename.

**Phase 3 — keyboard and polish (3 days).**
`library_pane.rs`, `panes.rs`, `app/input.rs`, `session.rs`, `overlays.rs` (the tab menu item).
Tests: cursor movement and folding by key; `Enter` opens; `F2` starts the rename; the face is
restored from a session line. Owner sees: a shot with the cursor on a row and the pane focused,
and the new empty state.

**Phase 4 — AI organise, first cut (8 days).**
A new `bins/mdedit/src/app/organise.rs` and `crates/mdedit-core/src/organise.rs` (the digest, the
prompt builder, the validator — all pure and testable without a network), the worker scan, the
plan in `results.rs`, the writes in `app/library.rs`, the record, the undo. Tests: the digest of a
sample folder; the prompt is under the cap at 40 files; a good reply plans correctly; a reply with
prose around it still parses; an unknown group becomes `Unsorted`; a duplicate `id` is dropped;
nonsense twice marks the batch `Unsorted` and the run continues; the plan writes nothing until the
button; undo removes exactly what was added and refuses a changed file; the run refuses with the
AI connection off, over the file cap, and with a library file open and dirty. Owner sees: the plan
panel over one of their own folders, and the library tree after Add.

**Phase 5 — conversion (3 days).** One run per ticked file through the existing rewrite path, the
new `.md` beside the original, the record extended, the report. Owner sees a `.txt` and its new
`.md` in compare.

**Phase 6 — hybrid home folder (4 days).** `library.rs` format and writers, the "Move the file
here" item, the collision rule, the open-tab case. Owner sees one group with a home folder and a
note moved into it.

**Phase 7 — drag and drop (3–4 days), only if the owner asks for it after using phase 2.**

Budgets: phases 1 to 3 add nothing to the launch path (the library is still read on first use,
`app/library.rs:19-20`) and nothing per frame beyond a few more `text()` calls in a pane that is
closed by default, so first frame, memory and keystroke should not move; they must still be
measured before and after phase 1 and phase 3, as `CLAUDE.md` requires. Phase 4 runs on a worker
and must be measured for idle CPU with a plan on screen.

---

## 7. What I would not do

- **An index file or database for the library.** The Markdown *is* the index, and the owner can
  edit it in this editor. A cache would be a second truth. `Library::changed()` already makes
  re-reading cheap (`library.rs:114-122`).
- **Tags.** Sections already nest to any depth, and a note can be listed in two groups today for
  free. A tag namespace would add a second model and a second UI with nothing new to offer.
- **A file watcher on the library folder.** `library_tick` re-stats at most every 800 ms and only
  while the pane shows (`app/library.rs:61-81`, `library_pane.rs:19`). A watcher is a thread and a
  launch-path risk against the first-frame budget, for a folder of at most 64 small files.
- **Automatic AI filing of new files.** Surprise, and every run costs money.
- **Multi-select in the pane, in the first cuts.** Every multi-row write is a transaction across
  several library files. Ship the single-row writes, use them, then decide.
- **A fourth dock region for the library.** The dock has three faces and one splitter; a new
  region means `Region`, `Areas` and the repaint signatures (`ui.rs:93-122, 1117`) and a layout
  the budgets would have to be re-proved against.
- **Sending whole files to the model.** Digests classify as well, cost about a tenth, and leak
  less.
- **Pinned or recent notes in the pane.** Ctrl+Shift+N with fuzzy matching and the tab session
  already cover "the note I keep coming back to".
- **Letting the AI move or delete files in the first cut.** It may propose; the app writes links
  and new files only.

---

## 8. Questions for the owner

1. **May the pane delete a file on disk?** That means a confirmation and a new dependency (the
   `trash` crate, or a `windows`-crate call, with nothing equivalent on Linux). If you do not
   answer, the pane will only ever remove links and never touch your files. *Blocks the last item
   of phase 2.*
2. **Should notes physically move into a folder the library owns, or is a tidy index over a messy
   disk enough?** If they should move: where does that folder live, and is it synced (OneDrive,
   Dropbox)? *Blocks phase 6; see §4.*
3. **Is sending about 2 KB of each scanned file to your chosen AI provider acceptable, and is 500
   files a run the right hard cap?** *Blocks phase 4.*
4. **For conversion: a new `.md` beside the original with the original kept — right?** Or should
   the originals go to an `originals/` folder (which is a move, and brings §4's failure modes with
   it)? *Blocks phase 5.*
5. **Which folder should the first AI run be tried on?** It wants to be a real but small one,
   30 to 80 files.
6. **Do you want drag and drop in the tree**, knowing it is 3 to 4 days and that "Move to…" does
   the same job in a tenth of that and works from the keyboard? *Blocks phase 7.*
7. **Three pane faces: head switches (my proposal, §3.3), a dropdown, or three toolbar buttons?**
   This one only needs a look at the phase 1 screenshots.
