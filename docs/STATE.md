# State of the work

Rewritten, not appended to. Last rewritten 2026-11-23, after mdedit 1.3.0 and mdview 1.0.5 were
released. If `git log -1` is newer, doubt "In hand" first.

## Released

- **mdedit 1.3.0**, 2026-11-23, tag `v1.3.0` at `ebe939d`. Sixteen assets, `hub.txt` the new one.
  Record: `docs/release-log.md`; notes: `docs/release-notes/1.3.0.md`. In short: the notes
  library and its pane; Split into notes by headings or by AI, with a record per run and undo; the
  original into the Recycle Bin; Move the notes with their pictures; Settings as pages; issues #7
  and #8 (closed); a closed mdview noticed (D-097, which the owner's check found does not hold:
  below); a version resource and `hub.txt` for Hub (D-098). Checked by the owner by hand before
  the tag (D-101; "Move the notes" not tried); budgets unchanged against 1.2.1
  (`docs/measurements/2026-11-23-1.3.0-against-1.2.1.md`).
- **mdview 1.0.5**, 2026-11-23, tag `v1.0.5` at `0a9bc7c`, fourteen assets: Settings as pages,
  `?closed` to mdedit, a version resource and `hub.txt`. Its note line was published as "(not
  released yet)" and corrected on the release the same hour, with the owner's yes; copies that
  saw it first keep it until the next release. Recorded in mdview's `docs/HANDOFF.md`.
- The shared crates are not released; the apps pin them by commit.
- **Hub**, the launcher and updater, is built as its own app (`Kaan88/mdhub`, `Desktop\hub`; its
  `docs/STATE.md` is its record; 0.1.0 notes written, no tag).

## On `main`, not released

- **A file added to the notes library shows in the pane's tree at once** (branch
  `fix/library-add-refresh`, merged): the note and its line were written, but the pane's rows were
  made again only when a group or a section was folded and opened, so the add looked lost. The rows
  are made again as it is written, with what was folded above the note opened, and a name being
  typed in the pane's box goes with them, its keyboard back in the text.
- **Browsing the library in the full preview** (visual editing, branch `fix/preview-browse`,
  merged, D-104): a tab still being read no longer ends the mode; `visual_sync` waits for it.
  Otherwise nothing but documents since `v1.3.0`. Nothing since 1.2.0 has had an
  outside review (audit 3's fixes included).

## In hand

- **Phase 4a, "Add a folder to the library…"** (`ai-library-plan.md` 5.7, without the AI):
  built and tested on `f/p4a`, worktree `md-wt\p4a`, not merged. A folder the owner picks becomes
  a group of links to its `.md`, `.markdown` and `.txt` files under their subfolders' headings
  three deep, written in one library file; nothing of theirs is copied, moved or renamed
  (D-110 to D-112, DESIGN 79). Not asked for by the owner, who stopped at R2 and said the section
  8 order stands; it is here because the brief for it was already given.
- **Phase R2**, the review with ticks and the tinted, level preview (`ai-library-plan.md` 5.5,
  D-077): started and stopped by the owner on 2026-11-23 before anything was written. Nothing
  of it exists.
- **M1, M2 and D** (D-100): the owner said yes; not started. D waits for R2.
- **mdview: click to zoom a large diagram**, asked for by the owner on 2026-11-23 for later;
  written up at the top of mdview's `docs/HANDOFF.md`. Not designed, not priced.

## Waiting for the owner

- **`docs/ai-library-plan.md`** section 8, in order after R2: W, V, 4a, 4, 5, drag and drop.
  Open: the resizer for phase V, a new crate to be priced first (rule 7).
- **The apps that may join**: `docs/ideas/README.md` lists each plan and drawing (terminal, file
  manager, task manager, downloader, workbench, and the quickest first release).
- **Dependabot**: open PRs (mdedit #9 regex-automata; mdview #2 to #6; shared #2 to #7) and a
  `chore/dependabot` branch in each repo whose `dependabot.yml` was never merged. Left until
  after the release; it is after it now.
- The notes library has been used in a real build and its two faults fixed, but it is not
  recorded anywhere as approved.

## Next

1. The owner's four faults from the hand check (D-101), for the next release: the add and browsing
   the library in the full preview, merged; a closed mdview, on `fix/closed-mdview` (first: whether
   it is missed with mdview 1.0.5 too); the notice before deleting a library file, on
   `fix/delete-library-notice`.
2. Then R2, and section 8's order. M1, M2 in mdview; D after R2. Phase 4a is built on `f/p4a`
   and waits for the owner's word to merge it.

## Known to be wrong on `main`

- **A closed mdview is still missed** (the owner's hand check, D-101): closing mdview by hand
  leaves "Stop the preview" in mdedit, even after mdedit's window has the focus again (tried with
  mdview 1.0.4, which sends no `?closed`). D-097's focus check does not hold; not yet looked into.
- **The 512-file limit.** Past 512 `.md` files in the library folder those that sort last drop
  out; the foot says how many are shown but the rest cannot be listed. Flat notes count.
- Opening Settings during an AI review discards the result.
- The AI review has no scrollbar and cuts long lines (R2 ends this).
- The editor never reads the AI engine's version.
- **Deleting a library file orphans its notes.** It removes the list, never a note, so the folder
  keeps every note with nothing listing them and splitting that document again writes nothing
  (every note reads "already there" and comes unticked). Found by the owner, who asks that the
  question at least say the notes stay on disk.
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
- Worktrees: `main` (`Desktop\markdown-editor-ai`); `md-wt\baseline-main`, `closed-mdview`,
  `delete-library-notice`, `library-add-refresh`, `preview-browse`, `review-hunks` and `p4a`;
  `hub-wt\app-prs2\mdedit`; and a session's detached scratchpad under `Temp`. The folders three
  finished sessions still hold under `.claude\worktrees\` are empty.
