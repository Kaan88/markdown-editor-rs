# State of the work

Rewritten, not appended to: when a line here stops being true, replace it. Last rewritten
2026-11-21, with `main` at `487e184`. If `git log -1` shows something newer than that, the lines
under "In hand" are the first to doubt.

## Released

- **mdedit 1.2.1**, 2026-11-21, tag `v1.2.1` at `528446a`. Fifteen assets: Windows exe, setup and
  zip, the Linux AppImage, tar.gz, deb, rpm and PKGBUILD, two macOS zips, three update manifests,
  `mdedit-ai.exe`, `mdview-engine.exe`. Record: `docs/release-log.md`.
- mdview is at 1.0.4. The shared crates are not released; the apps pin them by commit.

## On `main`, not released (the version in `Cargo.toml` is 1.3.0)

Release notes so far: `docs/release-notes/1.3.0.md`.

- **The notes library.** Library files in a `library` folder (one file per group, a heading per
  section, a link or a path per note); `Ctrl+Shift+N` opens a note by name; "Add this file to
  the notes library…"; the library as a tree in the left pane (`Ctrl+Shift+3`).
- **The library redesign, phases 1, 2, 2b and 3** (`ddb03d4`, `3453a7a`, `8522c82`): a Notes
  button on the bar; Files / Functions / Notes switches in the pane's head; the pane's face comes
  back at start; right-click menus; rename in place; move to a section; find a moved file; the
  library folder as a setting; the pane by keyboard; a note's folder on its row; empty states
  with buttons; "Show in the notes library"; "Delete this library file…".
- **Phase 0, the faults that could lose a note** (`f00ee60`, `fcf3130`): a note made from the
  pane is no longer read as an empty group, and "Delete this library file…" refuses any file the
  library lists as a note; a note inside the library folder is linked by a path relative to it;
  groups, sections and notes take one name rule, the shared `folder_name` with its 22 device
  names; Esc on an AI review keeps the result, and "AI: Show the last result" brings it back.
- **Phase 6a, a folder per group** (`487e184`): a new note goes in the folder its group's
  `home:` line names, one folder deeper for each heading of its section; a group without that
  line gets one, named after its file, in the same write; "Set the group's folder…" writes the
  line and "Clear the group's folder" removes it, and no file on disk moves either way.
- "Leave out the tab you were last on too" (Settings › Session).
- A scrollbar for the page in visual editing; Save As offers `.md` to a note the AI turned into
  Markdown.
- **One tick box and one switch**: the Replace-in-files confirmation draws its ticks with the
  shared `mdshared-ui` `check`, and the AI review its Changes / Preview switch with `segments`;
  a screen test pins each to the pixels the private drawing made.
- mdedit is at 558 tests (the app's own unit tests; 1196 in the workspace).

## In hand

- **The plan screen** (`ai-library-plan.md` 5.3), committed on `main` and not pushed: the card
  that shows a proposed set of notes, built on `mdedit_core::split`, with no AI in it. It opens
  from `--plan-demo <name>` (eight shots in `tests/shots.sh`) and from `App::plan_open_active`,
  which no menu or command calls yet. It writes nothing: "Write the notes" only counts that it
  was pressed. Not built: the Recycle Bin box, "Ask the AI…", the list's per-source headings,
  the status row's "Plan ready · Show · Drop" (`plan_show` and `plan_drop` exist, nothing draws
  them), and `replace_ui::draw_confirm` is not converted. Next: the writer, the record and undo.
- **Mockups** of three AI screens (the split plan, the rewrite review with ticks, the AI task
  window): branch `mock/ai-review`, worktree `..\mdedit-mock`, behind `--mock`, shots in
  `shots\mock\`. A drawing, not an implementation. Not to be merged.

## Waiting for the owner

- **`docs/ai-library-plan.md`**: the plan for everything after phase 3 (33.5 days in thirteen
  phases, and six optional ones). One question blocks work: the `trash` crate for the Recycle
  Bin. Whether 1.3.0 is tagged now that phase 0 is in, or waits for more of the plan, is also
  the owner's. Its section 11 lists 26 decisions taken for the owner; they are entries D-032 to
  D-057 of `docs/DECISIONS.md`, marked as waiting.
- **`docs/settings-redesign.md`**: the Settings dialog of both apps redesigned: a category list
  down the left, one page per category, nothing scrolling. Designed and drawn (72 mockups in
  `shots\mock-settings\` of the worktree `..\mdedit-settings`, branch `mock/settings`), costed at
  8.5 days, not built. The owner has approved the shape, dropping the dialog's footer and Close
  button, and rewording the Tabs page so it fits at the minimum; a symmetry and sizing pass is
  being drawn. Unanswered: the two settings it names as candidates to drop (Minimap width, Start
  on the last pinned tab), so both stay.
- **This arrangement of the documents** (D-058). The list of what moved, was rewritten and was
  removed is `docs\REORG.md` in the worktree `..\mdedit-mock`; it is deleted once looked at.
- The interface of the notes library has been used by the owner in a real build, and two faults
  they found were fixed in `8522c82`. It is not recorded anywhere as approved.

## Next

1. The plan's order: the shared pieces, then the plan card with Split at headings, then Split by
   AI. Its section 12 has the sequence and the cost.
2. **Audit 3** (Fable), against `docs/review-prompts/audit3-scope.md`, which now covers everything
   merged since 1.2.1: the notes library, its redesign phases, and phase 0 and 6a.
3. **1.3.0** is no longer held by phase 0; whether to tag it now is under "Waiting for the owner".

## Known to be wrong on `main`

Found 2026-11-21 while writing the plan (its section 2 has three of them). Phase 0 fixed the
faults that could lose a note or break a library that moves; these four are still true.

- **The 64-file limit.** More than 64 `.md` files directly in the library folder, and those that
  sort last drop out of the pane without a message. Notes flat in the folder count against it.
- Opening Settings during an AI review discards the result.
- The AI review has no scrollbar and cuts long lines.
- The editor never reads the AI engine's version.

## Known gaps, not planned unless they get in the way

- A note that goes missing on disk is marked at the next re-read or Refresh, not at once.
- The Files and Functions faces of the pane have no keyboard.
- The notes pane has no sort command and no filter box.
- The plain preview pane and side-by-side have no scrollbar.
- No access keys in the line-ending and encoding menus; an empty function list for Vue and
  Svelte; a column selection pads short lines on typing but not on paste or Enter; no compare
  arrows with word wrap on; the Markdown menu has no room left in an 800 px window.

## Not measured, not reviewed, not checked by hand

- **Budgets** were last measured for phase 1 (`d4e3ad6`, 2026-11-21): first frame 89.7 to 91.2 ms
  (medians of 7), 27.6 to 27.9 MiB, no change from before. Phases 2, 2b and 3 have no recorded
  measurement. The first-frame figure on this PC moves with the day (about 90 ms on some, about
  111 on others), so only before against after, run in turn on the same day, means anything.
- **Reviews.** Audit 1 (Astra, seven rounds) covered up to 1.1.2. Audit 2 (Fable) found faults
  that were all fixed for 1.2.0; the fixes have not been reviewed. Audit 3 was scoped
  (`docs/review-prompts/audit3-scope.md`) and there is no record that it ran. Nothing since 1.2.0
  has been reviewed by anyone: that is the whole notes library.
- **Never checked by hand:** the KDE blur on Linux; pasting image files copied in a Linux file
  manager; the AppImage and the Linux packages on a desktop; the macOS builds on a desktop; the
  Windows installer; an installed copy updating itself and restarting.

## Pins, branches, worktrees

- Shared crates at `c603b23`. The viewer's crates (`mdview-preview`, `mdview-render`) at viewer
  `e2b3e31`. winit fork `62fa727`, softbuffer fork `dd18fec`. mdview itself pins the shared
  crates at `cb0ffd3`, which is older.
- Every local `f/…` branch and `release/1.2.0` is merged into `main`. `mock/ai-review` is not
  and should not be.
- Worktrees on this PC: `..\mdedit-mock` (the mockups), `..\md-wt\p1-before` and `p1-after` (the
  phase 1 budget builds), `..\mdedit-four-fixes` (merged, prunable).
