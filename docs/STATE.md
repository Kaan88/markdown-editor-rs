# State of the work

Rewritten, not appended to. Last rewritten 2026-11-23, while 1.3.0 is being made ready (D-096 to
D-100). If `git log -1` is newer, doubt "In hand" first.

## Released

- **mdedit 1.2.1**, 2026-11-21, tag `v1.2.1` at `528446a`. Fifteen assets: Windows exe, setup and
  zip, the Linux AppImage, tar.gz, deb, rpm and PKGBUILD, two macOS zips, three update manifests,
  `mdedit-ai.exe`, `mdview-engine.exe`. Record: `docs/release-log.md`.
- mdview is at 1.0.4. The shared crates are not released; the apps pin them by commit.
- **Hub**, the launcher and updater, is built as its own app (`Kaan88/mdhub`, `Desktop\hub`; its
  `docs/STATE.md` is its record; 0.1.0 notes written, no tag).

## On `main`, not released: 1.3.0 (the version in `Cargo.toml`)

`docs/release-notes/1.3.0.md` lists it in full. In short:
- **The notes library**: the Notes pane, its menus and keyboard, the library folder as a setting,
  a `home:` folder per group (6a), the re-read on a worker thread, phase 0's protections.
- **Split into notes** (R1, 5.3): the plan card, the writer with a record per run in
  `<library>/.runs`, undo, "AI runs…". **Split by AI** (4b, D-089). **The original in the Recycle
  Bin** (T, D-092). **Move the notes** (6b, D-085), carrying their pictures (D-091, D-093).
- **Settings as pages** in both apps (D-086, D-087, D-094), every page fitting at 520x360.
- **Issues #7 and #8** (D-096): the Preview dropdown and default mode, "New file" on the empty
  window, plain paste on Ctrl+Shift+V (mdview preview moved to Ctrl+Shift+L), New, Save and AI
  edit on the bar, rich paste as Markdown (off by default).
- **A closed mdview is noticed** (D-097): mdview sends `?closed`; mdedit checks on focus and when
  a menu opens; nothing runs while idle.
- **For Hub** (D-098): a version resource in `mdedit.exe`, the uninstall key with its hive, and
  `hub.txt` as the release's sixteenth asset; the workflow step has never run in a release.
- Visual editing's scrollbar and `Ctrl+E` following the tab; "Leave out the tab you were last on
  too"; new notes named in lower case with hyphens (D-075, D-083).

The workspace's tests: 1338 pass, fmt and clippy clean (2026-11-23, Windows). Two Settings labels
were shortened so `ci` passes on Linux again (D-099).

## Making 1.3.0 ready (the owner, 2026-11-23: "Ready 1.3.0 first")

- **Budgets: no change against 1.2.1.** `docs/measurements/2026-11-23-1.3.0-against-1.2.1.md`:
  `sample.rs` 94.1 ms and 27.3 MiB in both; `sample.md` 121.5 ms and 36.4 MiB against 120.4 and
  36.6, over the first-frame and memory budgets in 1.2.1 already.
- **`ci` on `main`**: red on Linux since the paged Settings (two labels too wide); fixed in
  `ee18685`, not yet seen green.
- **The owner's hand check**: `target\for-owner\9-release-1.3.0\CHECKLIST.md`. Settings, the
  split and its undo, the Recycle Bin, the move, the dropdown and a closed mdview were approved by
  pictures or tested in code and never used in a live window. Split by AI was never run against a
  real provider: try it or leave it out of the notes.
- **Not reviewed**: nothing since 1.2.0 has had an outside review (audit 3's fixes included).
- **mdview 1.0.5** is on its `main` too (`docs/release-notes/1.0.5.md` there): Settings as pages,
  `?closed`, the version resource and `hub.txt`. Its `ci` on Linux had the same label fault,
  fixed in `e41ea3f`, not yet seen green.
- Then the tag `v1.3.0` (`docs/RELEASE.md`), only when the owner says so.

## In hand

- **Phase R2**, the review with ticks and the tinted, level preview (`ai-library-plan.md` 5.5,
  D-077): started and stopped by the owner on 2026-11-23 before anything was written. Nothing
  of it exists.
- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.

## Waiting for the owner

- The hand check and the tag (above).
- **`docs/ai-library-plan.md`** section 8, in order after R2: W, V, 4a, 4, 5, drag and drop.
  Open: the resizer for phase V, a new crate to be priced first (rule 7).
- **The apps that may join**: `docs/ideas/README.md` lists each plan and drawing (terminal, file
  manager, task manager, downloader, workbench, and the quickest first release).
- **Dependabot**: open PRs (mdedit #9 regex-automata; mdview #2 to #6; shared #2 to #7) and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Best left until
  after the release.
- The notes library has been used in a real build and its two faults fixed, but it is not
  recorded anywhere as approved.

## Next

1. `ci` green on both `main`s; the owner's hand check; then the tags, when the owner says.
2. Then R2, and section 8's order. M1, M2 in mdview; D after R2.

## Known to be wrong on `main`

- **The 512-file limit.** Past 512 `.md` files in the library folder those that sort last drop
  out; the foot says how many are shown but the rest cannot be listed. Flat notes count.
- Opening Settings during an AI review discards the result.
- The AI review has no scrollbar and cuts long lines (R2 ends this).
- The editor never reads the AI engine's version.
- **Deleting a library file orphans its notes.** It removes the list, never a note, so the folder
  keeps every note with nothing listing them and splitting that document again writes nothing
  (every note reads "already there" and comes unticked). Found by the owner.
- A pane the owner opened keeps painting the last page after every tab is closed;
  `ui.page_selected` is not cleared when the pane closes.
- **The move**: a crash between the library rewrites and the digest update gives a half-undo;
  folders emptied in a home outside the library stay.
- **A debug build still aborts on a closed stderr** (a release build does not): something that
  runs only in debug prints with `std`'s `eprintln!`. A plain debug `cargo test` fails
  `closed_stderr`; CI runs release and is unaffected.
- **Shared CI on Linux:** `pane::tests::names_show_while_they_fit…` measured 230 px on one Linux
  run and passed on the one before: it depends on the runner's fonts.
- **The `runs*` shots draw the clock**, so two runs of `tests/shots.sh` are not byte-identical.
- The uninstall entry of an installed copy says the version it was installed at (1.0.6 on the
  owner's PC); from 1.3.0 an update the app installs itself records the new one (D-098).

## Known gaps, not planned unless they get in the way

A Linux trash entry gets no deletion date (D-092); a note missing on disk is marked at the next
re-read; the Files and Functions faces have no keyboard; the notes pane has no sort or filter; the
plain preview and side-by-side have no scrollbar; no access keys in the line-ending and encoding
menus; no function list for Vue or Svelte; column selection pads short lines on typing only; no
compare arrows with wrap on; the Markdown menu needs more than 800 px. Split by AI has no progress
row with Cancel and no headings per source; the Recycle Bin cannot restore an original (D-071).

## Not checked by hand

Linux blur, image paste on Linux, the AppImage and packages, the macOS builds, the Windows
installer, an installed copy updating itself, and `hub.txt` read by Hub from a real release.

## Pins, branches, worktrees (tidied 2026-11-23)

- Shared crates at `09c67cd` in both apps. Viewer crates at viewer `e2b3e31`; winit `62fa727`,
  softbuffer `dd18fec`.
- Branches here: `main`; the drawings `mock/ai-review`, `mock/settings`, `mock/terminal`,
  `mock/downloader`, `mock/workbench` (local only), `mock/launcher`, `mock/file-manager`,
  `mock/task-manager` (also on GitHub), never to be merged; `archive/doc-reorg-draft` (local:
  the D-058 rearrangement as drafted); on GitHub, `spike/frosted-window` and `chore/dependabot`.
  Every merged branch was deleted, here and on GitHub, in all three repos.
- Kept as found, local, unpushed: the shared repo's `archive/settings-draft-uncommitted`, Hub's
  `wip/install-uncommitted` (69 lines in `hub-core` and a second `hub.ico`).
- Worktrees: none but `main`. The folders three finished sessions still hold under
  `.claude\worktrees\` are empty.
