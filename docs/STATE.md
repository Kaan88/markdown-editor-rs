# State of the work

Rewritten, not appended to. Last rewritten 2026-11-22, late, the owner asleep (D-083 to D-089, D-092 and D-093 are
Claude's; D-090, D-091, D-094 and D-095 the owner's). If `git log -1` is newer, doubt "In hand" first.

## Released

- **mdedit 1.2.1**, 2026-11-21, tag `v1.2.1` at `528446a`. Fifteen assets: Windows exe, setup and
  zip, the Linux AppImage, tar.gz, deb, rpm and PKGBUILD, two macOS zips, three update manifests,
  `mdedit-ai.exe`, `mdview-engine.exe`. Record: `docs/release-log.md`.
- mdview is at 1.0.4. The shared crates are not released; the apps pin them by commit.

## On `main`, not released (the version in `Cargo.toml` is 1.3.0)

`docs/release-notes/1.3.0.md` lists it in full; in short: **the notes library** and its redesign
(phases 1, 2, 2b, 3 — the Notes pane, its menus, the keyboard, the library folder as a setting),
**phase 0** (a note is no longer read as an empty group; delete refuses a note), **phase 6a** (a
`home:` folder per group), **the notes pane on a large library** (the re-read on a worker thread,
stamps kept for the staleness check, "512 of 583 groups shown"), **visual editing** (a page
scrollbar, and `Ctrl+E` follows the tab), "Leave out the tab you were last on too", Save As
offering `.md` to an AI-made note, and the shared tick box and switch. The **split feature** is
on `main` too but is under "In hand" below, because none of it is released. New notes are named
in lower case with hyphens (D-075, D-083). 636 app tests and 543 in the core, the workspace green;
the 74 tests of the record and the move run in the shared repo.

## In hand

- **The split feature** (`ai-library-plan.md` 5.3; **no release yet**): the plan card on
  `mdedit_core::split`, the writer (`mdshared_app::runs`, a record per run in `<library>/.runs`)
  and undo. "Split into notes…" is in the tab menu, the palette, the AI menu and the Notes
  library submenu, refused with "Save {name} first" until the document is on disk and saved;
  `--plan-demo <name>` draws eight shots. Every refusal it can know is made before the first
  file (D2, D-069 to D-073), the group's file is made before the notes, and undo re-reads the
  library and removes a note only if nothing lists it. Audit 3 is closed, C1 included. **Split
  by AI** (phase 4b, D-089, DESIGN 76): "Ask the AI…" on the card, the contract in
  `mdshared_app::split::answer`, 12 shots `plan-split-ai*` (approved, D-090), never run on a real provider. Not
  built: the progress row with Cancel, per-source headings, parallel requests. **The original in
  the Recycle Bin** (phase T, D-092, DESIGN 77): approved by the owner (D-095).
- **"Move the notes…"** (`ai-library-plan.md` 5.12, phase 6b): the core is on `main` as
  `mdshared_app::moves` (50 tests; DESIGN 74), and the window on a group's row menu: the move
  card, the write through the library gate, the results, undo from "AI runs…", tabs following
  their file (DESIGN 75, D-085; 12 tests; shots `move-notes`, `move-notes-small`,
  `move-results`, both themes, **approved by the owner**, D-090). Never clicked
  through in a live window; a move across volumes never run. The library move from Settings is
  not offered. Review fixes in D-084. A moved note carries its own pictures and rewrites its
  other relative links (D-091, D-093). **Left from the review:** a crash between the library rewrites and the digest update gives a half-undo; emptied
  folders in a home outside the library stay.
- **Phase S3** (D-081, D-088) is done: `runs`, `moves`, the split checker (`split::Lines` over
  `Buffer`) and the plan card (`mdshared_ui::plan`) are shared, at shared `842ec3f`. **Phase S**
  is done too: the library tree's rows are `mdshared_app::library::tree`, shared `ecfcdf9`. All
  mdedit shots byte-identical but the clock, both times.
- **The settings redesign's shared half** is on shared `main` at `9df8bc8`:
  `mdshared_ui::settings::pages` beside the old `Form` (proved unchanged frame for frame), built
  from the approved design on `mock/settings` (D-086). Fit test at 520x360 and 1280x800, scales
  1, 1.5, 2; 52 specimen pictures (`cargo test -p mdshared-ui specimen -- --ignored`) looked at
  by Claude (the owner approved mdedit's use of them, D-090). The dead draft still sits uncommitted in the shared
  repo's own checkout; it is superseded and can be dropped.
- **mdedit's Settings on the paged form** (D-087, D-090): **merged into `main`**, approved by the
  owner by its shots. Never clicked through in a live window. mdview's half is merged too
  (mdview `29bf0ab`, D-094): the settings redesign is done.
- **Dependabot**: triage done. Left: parley PRs in shared and mdview; `regex-automata` #9 unmeasured.
- **A terminal, drawn**: `mock/terminal`, `..\mdedit-term`, 20 shots; verdict in `docs/terminal.md`.
- **A downloader window, drawn**: `mock/downloader`, `..\mdedit-dl`, 11 states in
  `shots\mock-dl\`; lessons in its README, costing in the uncommitted `ecosystem-ideas.md` 6b-6d.
- **A local-LLM workbench, drawn**: branch `mock/workbench`, worktree `..\mdedit-llm`, `--mock
  llm-<name>`, 12 states plus a real-editor `saved` shot in `shots\mock-llm\`. Holds up as a tab
  of mdedit, as `ecosystem-ideas.md` 8 said; columns under 300 px need a narrow form, five is the
  most at 1280 px; the picker has to say "one GPU" and offer one model at a time.
- **A launcher and updater for every app with a setup, drawn**: branch `mock/launcher`, worktree
  `..\mdedit-hub`, `--mock hub-<name>`, 12 states in both themes (four also at 860x600) in
  `shots\mock-hub\`. Its README lists the eight apps with installers on GitHub and their four
  manifest formats (two have none). Wants to be its own small tray app, not part of mdedit.

## Waiting for the owner

- **`docs/ai-library-plan.md`**: answered on 2026-11-22 and rewritten to match (D-074 to D-082):
  section 8 is 16 phases, 49 to 50 days, about 11 done. Open: the resizer for phase V, a new
  crate to be priced. **No release for now** (the owner, 2026-11-22): 1.3.0 is not tagged.
- **D-083 to D-089**, taken by Claude with the owner away, as asked; each says what overturns it.
- **`docs/settings-redesign.md`**: both apps' Settings as a category list, one page each, nothing
  scrolling. Designed and drawn (80 shots, `mock/settings`), 8.5 days: 5 shared, 2 mdedit, 1
  mdview, 0.5 platforms. Approved by the owner; the copy on `main` is the first round, the
  approved one is on `mock/settings`. The shared half and mdedit's are built and merged (above).
- **This arrangement of the documents** (D-058), listed in `docs\REORG.md` on `mock/ai-review`.
- The notes library has been used by the owner in a real build and its two faults fixed, but it
  is not recorded anywhere as approved.

## Next

1. `ecosystem-ideas.md` 12 gets a line pointing at `docs/terminal.md`, once the uncommitted work
   in that file (§6b-6d, someone else's) is committed or dropped.
2. Issues #7 and #8, on `f/issues-7-8` for the owner's look (D-095).
3. Then section 8's order: R2, W, V, 4a, 4, 5, drag and drop. No release.

## Known to be wrong on `main`

- **The 512-file limit.** Past 512 `.md` files in the library folder those that sort last drop
  out; the foot says how many are shown but the rest cannot be listed. Flat notes count.
- Opening Settings during an AI review discards the result.
- The AI review has no scrollbar and cuts long lines.
- The editor never reads the AI engine's version.
- **Deleting a library file orphans its notes.** It removes the list, never a note, so the folder
  keeps every note with nothing listing them and splitting that document again writes nothing
  (every note reads "already there" and comes unticked). Both fixes want the Recycle Bin: the
  question should offer to take the notes with the list, and such a row should offer *link it
  anyway*. Found by the owner.
- A pane the owner opened keeps painting the last page after every tab is closed;
  `ui.page_selected` is not cleared when the pane closes.
- **A debug build still aborts on a closed stderr** (a release build does not): something that
  runs only in debug prints with `std`'s `eprintln!`, not yet found. A plain debug `cargo test`
  fails `closed_stderr`; CI runs release and is unaffected.
- **Shared CI on Linux:** the `D:/…` library tests are fixed (shared `7a6f013`; green on the
  Macs and Windows), but `pane::tests::names_show_while_they_fit…` measured 230 px on one Linux
  run and passed on the one before: it depends on the runner's fonts.
- **The `runs*` shots draw the clock**, so two runs of `tests/shots.sh` are not byte-identical.

## Known gaps, not planned unless they get in the way

A Linux trash entry gets no deletion date (D-092); a note missing on
disk is marked at the next re-read; the Files and Functions faces have no keyboard; the notes pane
has no sort or filter; the plain preview and side-by-side have no scrollbar; no access keys in the
line-ending and encoding menus; no function list for Vue or Svelte; column selection pads short
lines on typing only; no compare arrows with wrap on; the Markdown menu needs more than 800 px.

## Not measured, not reviewed, not checked by hand

- **Budgets.** Private memory is over the 30 MiB budget with a big library (30.4 MiB at 2,000
  notes; 31.0 with 70) and the owner forgave it (D-063). The first-frame figure here moves with
  the day, so only before against after, run in turn, means anything:
  `docs/measurements/2026-11-21-since-phase-1.md`, `…-library-rework.md`.
- **Reviews.** Audit 3 (Fable, `docs/review-prompts/`) found nine live faults; all fixed on
  `main`. None of those fixes, nor anything since 1.2.0, has been reviewed.
- **Never checked by hand:** Linux blur, image paste on Linux, the AppImage and packages, the
  macOS builds, the Windows installer, and an installed copy updating itself.

## Pins, branches, worktrees

- Shared crates at `be0192d` (the settings draft above sits uncommitted in the shared checkout,
  at `b4b2182`). Viewer crates at viewer `e2b3e31`; winit `62fa727`, softbuffer `dd18fec`; mdview pins shared `be0192d`.
- Merged into `main`: every `f/…` and `release/1.2.0`. Drawings that must not merge:
  `mock/ai-review`, `mock/settings`, `mock/terminal`. `f/move-notes` is merged; its copy on
  GitHub is the pre-rebase draft (a force-push was refused) and can be deleted.
- Worktrees: the `mock/…` drawings (`..\mdedit-mock`, `-settings`, `-term`, `-dl`, `-llm`, `-hub`;
  build folders deleted).
