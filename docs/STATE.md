# State of the work

Rewritten, not appended to: when a line here stops being true, replace it. Last rewritten
2026-11-22, when the Claude credits ran out mid-work: two agents died and left drafts in two
trees, unreviewed. If `git log -1` shows something newer than that, the lines under "In hand"
are the first to doubt.

## Released

- **mdedit 1.2.1**, 2026-11-21, tag `v1.2.1` at `528446a`. Fifteen assets: Windows exe, setup and
  zip, the Linux AppImage, tar.gz, deb, rpm and PKGBUILD, two macOS zips, three update manifests,
  `mdedit-ai.exe`, `mdview-engine.exe`. Record: `docs/release-log.md`.
- mdview is at 1.0.4. The shared crates are not released; the apps pin them by commit.

## On `main`, not released (the version in `Cargo.toml` is 1.3.0)

Release notes so far: `docs/release-notes/1.3.0.md`.

`docs/release-notes/1.3.0.md` lists it in full; in short: **the notes library** and its redesign
(phases 1, 2, 2b, 3 — the Notes pane, its menus, the keyboard, the library folder as a setting),
**phase 0** (a note is no longer read as an empty group; delete refuses a note), **phase 6a** (a
`home:` folder per group), **the notes pane on a large library** (the re-read on a worker thread,
stamps kept for the staleness check, "512 of 583 groups shown"), **visual editing** (a page
scrollbar, and `Ctrl+E` follows the tab), "Leave out the tab you were last on too", Save As
offering `.md` to an AI-made note, and the shared tick box and switch. The **split feature** is
on `main` too but is under "In hand" below, because none of it is released. 635 app tests and 566
in the core, the workspace green.

## In hand

- **The split feature** (`ai-library-plan.md` 5.3; **no release yet**): the plan card on
  `mdedit_core::split`, the writer (`mdedit_core::runs`, a record per run in `<library>/.runs`)
  and undo. "Split into notes…" is in the tab menu, the palette, the AI menu and the Notes
  library submenu, refused with "Save {name} first" until the document is on disk and saved;
  `--plan-demo <name>` draws eight shots. Every refusal it can know is made before the first
  file (D2, D-069 to D-073), the group's file is made before the notes, and undo re-reads the
  library and removes a note only if nothing lists it. Audit 3 is closed, C1 included. Not
  built: the Recycle Bin, the progress row with Cancel, "Ask the AI…", per-source headings.
- **Mockups** of three AI screens (plan, rewrite review, task window): branch `mock/ai-review`,
  worktree `..\mdedit-mock`, `--mock`, shots in `shots\mock\`. A drawing; not to be merged.
- **Two agents died mid-work on 2026-11-22** when the Claude credits ran out. What they
  left is uncommitted and unreviewed: a stranger's draft, to be read, run, and never committed as
  finished. **Nothing has been built since they stopped**, so neither tree is known to compile.
  - **"Move the notes…"** (`ai-library-plan.md` 5.12) died part-way: branch `f/move-notes`,
    worktree `..\md-wt\move-notes`, last commit `0c41787`, nothing of its own committed. It left
    a new `crates/mdedit-core/src/moves.rs` (2,545 lines), `moves_tests.rs` (1,316), one line in
    `lib.rs` and 118 changed lines in `runs.rs`. The window side (`plan_ui`, `plan_write`, the
    group row's menu) was not reached.
  - **The settings half** died earlier still, after only the proof that the old form is
    unchanged: `crates/mdshared-ui/src/settings/frozen.rs` (1,734 lines, the form copied whole),
    `unchanged.rs` (255, drives both and compares every pixel in both themes) and five lines in
    `settings.rs`. **Uncommitted in the shared repo's own checkout, on `main` at `b4b2182`.**
    The categorised form was not written: no page host, no row kinds, no page-fit test;
    `unchanged.rs` links `super::pages`, which does not exist.
- **Dependabot cleared** per `docs/dependabot-triage-2026-11-22.md`: the actions group and
  `windows-sys` 0.61.2 merged (measured before and after: no change past noise, 1.5 KB smaller),
  `rfd`, `chardetng` and `parley` closed here with reasons, the open-PR limit raised to 10. The
  parley PRs in the shared repo and mdview are still open. `regex-automata` 0.4.19 (#9) arrived
  after and is green, not measured, not merged.
- **A terminal for this ecosystem, drawn**: branch `mock/terminal`, worktree `..\mdedit-term`,
  `--mock term-<name>`, 20 shots in `shots\mock-term\`, numbers in `numbers.txt`. Output in one
  block per command — command, exit code, duration, output — each collapsible, searchable,
  openable in mdedit, or savable as a note. `ecosystem-ideas.md` 12 says **Don't**; the owner
  asked for that to be reconsidered in a document of its own: **`docs/terminal.md`**, which says
  *possible*, as a third app, for the block version only (10-17 pw, an estimate), and waits for
  the owner. Established by drawing: blocks need OSC 133 marks from the shell, so `plain-*.png`
  (no marks, one stream) is the case to survive; a block costs ~46 px; only failure deserves a
  mark; and "no terminal" in the charter makes this a third app or a written exception.

- **A downloader window over NewsQueue and Inbound, drawn**: branch `mock/downloader`, worktree
  `..\mdedit-dl`, `--mock dl-<name>`, 11 states in both themes in `shots\mock-dl\` (README says
  what each shows). One table, a Source chip per row; the details pane is the only place the
  engines differ, so its tabs follow the row; an absent engine dims its rows and veils the pane as
  "last reported". Not drawn: Files and Log tabs, Remote files, TorBox, first run. Costing is in the
  uncommitted `ecosystem-ideas.md` 6b-6d.
- **A local-LLM workbench, drawn**: branch `mock/workbench`, worktree `..\mdedit-llm`, `--mock
  llm-<name>`, 12 states plus a real-editor `saved` shot in `shots\mock-llm\`. Holds up as a tab
  of mdedit, as `ecosystem-ideas.md` 8 said; columns under 300 px need a narrow form, five is the
  most at 1280 px; the picker has to say "one GPU" and offer one model at a time.

## Waiting for the owner

- **`docs/ai-library-plan.md`**: its 26 decisions were answered on 2026-11-22: 17 confirmed, the
  `trash` crate approved (phase T is unblocked), nine replaced by D-074 to D-082. The plan's
  phases and days do not yet reflect the nine: rewrites in place with an undo copy in the record,
  lower-case-hyphen note names everywhere (the split writer and "New note here…" already built do
  not do it), P2 with R2, phase V (its resizer still to be asked for), pictures into existing
  notes behind a setting, parallel requests, everything shared, drag and drop. **No release for
  now** (the owner, 2026-11-22): 1.3.0 is not tagged.
- **`docs/settings-redesign.md`**: both apps' Settings as a category list, one page each, nothing
  scrolling. Designed and drawn (80 shots, `mock/settings`), 8.5 days: 5 shared, 2 mdedit, 1
  mdview, 0.5 platforms. Approved by the owner; the shared half died with the credits (above) and
  the app halves are not briefed.
- **This arrangement of the documents** (D-058), listed in `docs\REORG.md` on `mock/ai-review`.
- The notes library has been used by the owner in a real build and its two faults fixed, but it
  is not recorded anywhere as approved.

## Next

1. `ecosystem-ideas.md` 12 gets a line pointing at `docs/terminal.md`, once the uncommitted work
   in that file (§6b-6d, someone else's) is committed or dropped.
2. Clear the dependabot queue per the triage (above): merge two after a rebase, close three, raise
   the PR limit.
3. Then read the two dead agents' trees and decide per tree. "Move the notes…" has 2,545 lines of
   model and 1,316 of tests nobody has run — run them first; if they pass, the window side is a
   well-specified job. The settings half needs a restart on `pages`.
4. Fold D-074 to D-082 into `ai-library-plan.md` (phases, days, section 11), and rename notes to
   lower case with hyphens in the writers already built (D-075).
5. The plan's order afterwards: the shared pieces, then Split by AI (section 12). No release.

## Known to be wrong on `main`

Found 2026-11-21 while writing the plan (its section 2 has three of them). Phase 0 fixed the
faults that could lose a note or break a library that moves; these are still true.

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

- **A development build writes crash reports into the owner's folder.** `app::crash_dir` is
  `%LOCALAPPDATA%\mdedit\crashes` whatever `MDEDIT_SETTINGS` says, against rule 9; three reports
  from a mockup build of 2026-11-22 sit there now.
- **mdedit dies when stderr is a closed pipe**: `eprintln!` panics ("failed printing to stderr",
  os error 232) and the release build aborts. Seen when a shell closed the pipe of a `--trace` run.

## Known gaps, not planned unless they get in the way

Undo removes with `remove_file`, not the Recycle Bin (waits for `trash`, D-071); a note missing on
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

- Shared crates at `b4b2182` (whose checkout holds the settings draft above). Viewer crates at
  viewer `e2b3e31`; winit `62fa727`, softbuffer `dd18fec`; mdview still pins `cb0ffd3`.
- Merged into `main`: every `f/…` and `release/1.2.0`. Drawings that must not merge:
  `mock/ai-review`, `mock/settings`, `mock/terminal`. `f/move-notes` holds the dead draft.
- Worktrees: `..\mdedit-mock`, `..\mdedit-settings`, `..\mdedit-term`, `..\md-wt\move-notes`,
  `..\md-wt\p1-before`/`p1-after`, `..\mdedit-four-fixes` (prunable).
