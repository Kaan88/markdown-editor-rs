# State of the work

Rewritten, not appended to: when a line here stops being true, replace it. Last rewritten
2026-11-21, with the plan's guards: a write is checked before it writes, and a window without a
settings file has no library. If `git log -1` shows something newer than that, the lines under
"In hand" are the first to doubt.

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
- **The notes pane on a large library** (`fb7777c` shared pin): a library file saved in another
  window is read on a worker thread (`mdshared_app::job`), so the event loop no longer stalls for
  the read; the rows and the keyboard's cursor stay while it runs, and a result whose folder
  moved under it is dropped. A folder with more `.md` files than the reader keeps (now 512) says
  at the pane's foot how many groups it shows ("512 of 583 groups shown"). The staleness check
  keeps the file list and their stamps, not a second full library (`docs/measurements/2026-11-21-library-rework.md`).
- "Leave out the tab you were last on too" (Settings › Session).
- A scrollbar for the page in visual editing; Save As offers `.md` to a note the AI turned into
  Markdown.
- **Visual editing follows the tab.** `Ctrl+E` stays on across tabs: another Markdown tab shows
  its page, with no block open and at the place that tab's text was; a file the page cannot show
  ends the mode; and the preview pane goes back to what it was before the mode — closed if the
  mode opened it, open if it was open. Closing the tab the mode is on follows the next one.
- **One tick box and one switch**: the Replace-in-files confirmation draws its ticks with the
  shared `mdshared-ui` `check`, and the AI review its Changes / Preview switch with `segments`;
  a screen test pins each to the pixels the private drawing made.
- mdedit is at 629 tests (the app's own unit tests).

## In hand

- **The plan screen** (`ai-library-plan.md` 5.3), of which the writer, the record and undo are in
  (**no release yet**, `main`): the card that shows a proposed set of notes, built on
  `mdedit_core::split`, with no AI in it. It opens from `Split into notes…` — the tab's menu, the
  palette, the AI menu and the Notes library submenu, on the document in front, disabled with
  "Save {name} first: the notes are made from the file on disk." until that document has a file
  and no unsaved changes — and from `--plan-demo <name>` (eight shots). "Write the notes" writes:
  `mdedit_core::runs` makes the folders, the notes and the group's library file in the order of
  4.3, writes one record per run in `<library>/.runs`, and every library file is updated through
  the window's own `library_attempt` gate. The report takes the results panel with "Undo this
  run", and `Undo` asks first. It refuses unless the original is still the file the notes were cut
  from (D-071), removes a note only if no library file lists it (D-070), only one this run made
  (D-069), only inside the library or the group's home (D-072); a later version's record is
  never pruned.
  "AI runs…" (the AI menu and the Notes library
  menu) lists the records of the runs, with Show the notes, Undo this run and Finish. The Notes
  pane draws "Plan ready: N notes · Show · Drop" at its foot while a plan is parked. Every refusal
  it can know before the first file is made before it (D2: the group's file's name, both caps, an
  unsaved library file, the source's length and SHA-256, and a dry run of the links), the group's
  file is made before the notes, the plan reads the library where it opens and refuses without a
  library folder (`--no-session` has none unless `MDEDIT_SETTINGS` names one), a group's folder
  cannot be the library folder, and a destination carries its group's file. A note with no section
  into a group that already has sections is written in the group's own notes, above its first
  section (audit 3 C1). Not built: the Recycle Bin (step 8, phase T), the progress row
  "Writing… 14 of 23" with Cancel, "Ask the AI…", the list's per-source headings, and
  `replace_ui::draw_confirm` is not converted.
- **Mockups** of three AI screens (the split plan, the rewrite review with ticks, the AI task
  window): branch `mock/ai-review`, worktree `..\mdedit-mock`, behind `--mock`, shots in
  `shots\mock\`. A drawing, not an implementation. Not to be merged.
- **Being built right now, by three agents, none sharing a tree.** Each commits on its own
  branch or repository and reports; none of it is reviewed or merged.
  - **"Move the notes…"** (`ai-library-plan.md` 5.12), branch `f/move-notes`, worktree
    `..\md-wt\move-notes`, from `0c41787`. The plan card's second user and the first that is not
    a split: a group's notes into its folder, the notes to match renamed headings, or the whole
    library when the library folder changes. Moved never copied-and-deleted, nothing overwritten,
    the link rewritten in the same write as the move, a record so it can be undone.
  - **The settings redesign, the shared half** — `markdown-shared-rs` on `main`, to be pushed by
    the agent. A variant beside the single-page form, not a rewrite, because both apps draw
    settings with the old one (rule 8). Its point is the page-fit test that fails naming the page
    and the pixels over.
  - **A triage of the five dependabot pull requests**, open since 2026-11-16 with `ci` red:
    `rfd` 0.15→0.18, `chardetng` 0.1→1.0, `windows-sys` 0.59→0.61, `parley` 0.12→0.13, and the
    actions group. Read and report only, to
    `%TEMP%\opencode\dependabot-triage.md`; nothing merged. `parley` is text layout and sits on
    the launch path.
- **A terminal for this ecosystem, drawn**: branch `mock/terminal`, worktree `..\mdedit-term`,
  behind `--mock term-<name>`, 20 shots in `shots\mock-term\`, layout numbers in
  `numbers.txt`. Output divided into one block per command — command, exit code, duration,
  output — which can be collapsed, searched, opened in mdedit or saved into the notes library.
  `ecosystem-ideas.md` 12 says **Don't**; the owner has asked for that verdict to be reconsidered
  as *possible* in a document of its own that points at these shots. **That document is not
  written yet**, and it is the first thing to do. What the drawing established: blocks need the
  shell to emit OSC 133 marks, so `plain-*.png` (no marks: one undivided stream, every block
  action dead) is the case the product has to survive; a block costs about 46 px of overhead;
  only failure should carry a mark; the charter says "no terminal", so this is a third app or a
  written exception; and the settings form cannot hold two apps' categories at the minimum size.

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

1. **The terminal document**, as the owner asked: a document of its own that upgrades
   `ecosystem-ideas.md` 12 from "Don't" to possible and points at `shots\mock-term\` on
   `mock/terminal`. `ecosystem-ideas.md` itself has someone else's uncommitted work in it, so
   its pointer waits until that is committed or dropped.
2. Review and land the three pieces being built, in this order: "Move the notes…", the shared
   settings half (then brief the app side, which is 2 days for mdedit and 1 for mdview), the
   dependabot triage.
3. The plan's order: the shared pieces, then the plan card with Split at headings, then Split by
   AI. Its section 12 has the sequence and the cost.
4. **1.3.0** is no longer held by phase 0; whether to tag it now is under "Waiting for the owner".

## Known to be wrong on `main`

Found 2026-11-21 while writing the plan (its section 2 has three of them). Phase 0 fixed the
faults that could lose a note or break a library that moves; these are still true.

- **The 512-file limit.** More than 512 `.md` files directly in the library folder, and those
  that sort last drop out; the pane's foot now says how many groups it shows of how many there
  are, but the rest cannot be listed. Notes flat in the folder count against it.
- Opening Settings during an AI review discards the result.
- The AI review has no scrollbar and cuts long lines.
- The editor never reads the AI engine's version.
- **Deleting a library file orphans its notes.** "Delete this library file…" removes the group's
  list and never a note, so the folder keeps every note with nothing listing them; splitting the
  same document again then writes nothing, because every note reads "already there" with the
  same bytes and comes unticked. Two fixes, both wanting the Recycle Bin: the question should
  name what it leaves and offer to recycle the notes with the list, and an "already there with
  the same text" row should offer *link it anyway*. Found by the owner, 2026-11-21.
- A pane the owner opened keeps painting the last document's page once every tab is closed, and
  `ui.page_selected` is not cleared when the pane closes.

## Known gaps, not planned unless they get in the way

- Undo removes with `remove_file`, not the Recycle Bin: waits for the `trash` crate (D-071).
- A note that goes missing on disk is marked at the next re-read or Refresh, not at once.
- The Files and Functions faces of the pane have no keyboard.
- The notes pane has no sort command and no filter box.
- The plain preview pane and side-by-side have no scrollbar.
- No access keys in the line-ending and encoding menus; an empty function list for Vue and
  Svelte; a column selection pads short lines on typing but not on paste or Enter; no compare
  arrows with word wrap on; the Markdown menu has no room left in an 800 px window.

## Not measured, not reviewed, not checked by hand

- **Budgets.** The full pass since phase 1 is `docs/measurements/2026-11-21-since-phase-1.md`:
  the first frame could not be compared (the machine moved fourfold that evening); private
  memory rose to 30.4 MiB with the 2,000-note library, which the owner then forgave (D-063,
  2026-11-21). The notes-pane rework was measured after it, same method
  (`docs/measurements/2026-11-21-library-rework.md`): private 31.0 MiB with the 70-file library;
  on one 60-file library 31.50 MiB before against 30.90 after; and a library change's
  event-loop pass 40.3–48.9 ms before against 2.3–2.6 ms to start the read and 5.8–6.2 ms to
  take its result. The first-frame figure on this PC still moves with the day, so only before
  against after, run in turn, means anything.
- **Reviews.** Audit 1 (Astra, seven rounds) covered up to 1.1.2. Audit 2 (Fable) found faults
  that were all fixed for 1.2.0; the fixes have not been reviewed. Audit 3 (Fable) has run:
  `docs/review-prompts/audit3-report.md`, 21 probes against `6cdeab2`. Everything it found is
  fixed and on `main`: the Undo question (A1), the four-byte name that ended the program (B1, in
  the shared crates), the plan guards (D2 and A6, A7, A8, A11, C2, C3, C5, C7), C1 (the shared
  `add_note_in_group` writes a note with no section above the group's first section), and undo's
  own rules (D1, D4, A2-A5, A9, A10, B2). Audit 3's own fixes have not been reviewed, and nothing
  since 1.2.0 has been reviewed by anyone but Fable: that is the whole notes library.
- **Never checked by hand:** the KDE blur on Linux; pasting image files copied in a Linux file
  manager; the AppImage and the Linux packages on a desktop; the macOS builds on a desktop; the
  Windows installer; an installed copy updating itself and restarting.

## Pins, branches, worktrees

- Shared crates at `b4b2182`. The viewer's crates (`mdview-preview`, `mdview-render`) at viewer
  `e2b3e31`. winit fork `62fa727`, softbuffer fork `dd18fec`. mdview itself pins the shared
  crates at `cb0ffd3`, which is older.
- Every local `f/…` branch and `release/1.2.0` is merged into `main`. The mock branches are not
  and should not be: `mock/ai-review` (the AI review screens), `mock/settings` (the settings
  redesign), `mock/terminal` (a terminal for the ecosystem, drawn to be looked at, not built).
  `f/move-notes` is "Move the notes…", being built.
- Worktrees on this PC: `..\mdedit-mock`, `..\mdedit-settings` and `..\mdedit-term` (the three
  mockup branches), `..\md-wt\move-notes` (the move), `..\md-wt\p1-before` and `p1-after` (the
  phase 1 budget builds), `..\mdedit-four-fixes` (merged, prunable).
