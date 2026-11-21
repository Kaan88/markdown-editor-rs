# Audit 3: report

mdedit at `6cdeab2` (worktree `C:\Users\kaank\Desktop\md-wt\audit3`), shared crates as pinned
(`c603b23`). Read only: I changed nothing in the worktree or the main checkout, and `git status`
in the worktree is clean.

## In short

1. **In "Undo this run?", Cancel undoes the run and Undo only closes the question** (with the
   mouse). Proven.
2. **Undo deletes the notes and leaves every one of their lines in the library** whenever no
   section heading goes with them: any run whose notes are under no section, or that went into
   sections that were already there. Proven through the window. The fix for F2 will not catch it,
   because nothing is refused.
3. **Undo deletes the only copy of the text** when the owner has deleted the original since. It
   looks at the original only when `recycled` is set, which nothing sets. The delete is
   `remove_file`, not the Recycle Bin. Proven.
4. **A file or folder name ends the program**: `beş.md`, `D:\maç\…`, `C:\Users\Zoé\…`. It is in
   the shared crate and in every release since 1.0.0. Proven with the real binary. mdview calls
   the same function; I did not run it.
5. "Split into notes…" from the main menu, before anything has read the library, writes the whole
   run into the folder the program was started in, with the confinement check void. Proven.
6. A family: refusals that could be known before the first file is made are found after the notes
   are written (a note under no section into a group that has sections; a section called `C#`; a
   group file whose name is taken; the two caps; an unsaved tab). The run is left "links pending",
   and nothing offers a way forward: Finish is not offered for that state, and "Try again" does
   not exist.
7. The write side held. I tried to make "Write the notes" overwrite, move, rename or delete a file
   of the owner's and could not: every note is created by `create_new`, a taken name is never
   written to, folders are only created, and the library file goes through a check that compares
   sets. The claim of plan 4.1 fails in undo, not in the write.
8. Three of the faults are faults of the plan I wrote, not of the code built from it. They are in
   section 4.

Twenty-one probes were run; each passes on `6cdeab2` and asserts the fault. Section 8 lists them.

## How this was checked

- **Read in full:** `crates/mdedit-core/src/runs.rs`, `split.rs`; `bins/mdedit/src/plan_ui.rs`
  (not its tests), `app/plan.rs`, `app/plan_write.rs`, `app/plan_write_tests.rs`,
  `app/library_write.rs`; the shared `library.rs`, `files.rs`, `paths.rs`, `folder_lock.rs` and
  `settings.rs` at `c603b23`. **Read in part:** `app/library.rs`, `library_pane.rs` (the path
  helpers), `overlays.rs` (the runs dialogs, the menus), `replace_ui.rs` (`Outcome::run`,
  `undone`), `app/commands.rs`, `app/input.rs`, `docs.rs` and `app.rs` where the above lead.
- **Run:** a copy of the tree (`git archive` of `6cdeab2`) in
  `%TEMP%\opencode\audit3-build`, with two probe files added and one `mod` line in `app.rs`;
  `cargo test --offline` with its target folder in `%TEMP%\opencode\audit3-target`. And one run
  of the debug binary, headless (`--no-session --shot`), on two scratch files.
- **Not run:** mdedit's own suite, any window by hand, anything on macOS or Linux. The owner's
  settings and library were never opened.

Each finding says **proven** (a probe ran) or **read** (from the code only).

---

## 1. Faults that can lose, delete or corrupt

Ordered by what would hurt most.

### A1. "Undo this run?": Cancel undoes the run, Undo closes the question

- **Where.** `bins/mdedit/src/overlays.rs:1446-1461` draws `labels[n]` with `Hit::DlgButton(n)`
  and `labels = [("Cancel", false), ("Undo", true)]`: Cancel is button 0, Undo is button 1.
  `bins/mdedit/src/app/commands.rs:193-203` runs the undo on `n == 0`; any other button falls to
  `close_dialog()`. Every other question puts its action first (`overlays.rs:1068-1118`), which
  is what the handler assumes. `RunUndo` is not in `DialogKind::is_confirm` (`ui.rs:712-721`), so
  nothing shared catches it.
- **Reach.** "AI runs…" → "Undo this run…", or "Undo this run" on the report; click Cancel. Enter
  and Esc are right (`app/input.rs:1455-1463`), which is why a keyboard test passes.
- **How bad.** The one button that means "do not delete" deletes. With A3 it is the permanent
  loss of the only copy. And an owner who clicks Undo sees the question close and nothing happen.
- **Do.** Put the action first as the other questions do, and make `RunUndo` a confirm kind. Test
  it by clicking both drawn buttons.
- **Proven.** `n23_cancel_in_undo_this_run_undoes_the_run`,
  `n23_undo_in_undo_this_run_only_closes_the_question`: the button at x=676 deletes the notes and
  marks the record undone; the button at x=780 leaves both.

### A2. Undo deletes the notes and leaves all their lines, when no section heading goes with them

- **Where.** `bins/mdedit/src/app/plan_write.rs:57-68`. The closure ends with
  `runs::with_headings_removed(&without, headings, &base)`, which is `None` when it removed no
  heading (`runs.rs:1690`), so the text with the links taken out is thrown away.
  `library_attempt` reads `None` as nothing to do (`library_write.rs:138-140`), and line 66 maps
  `Wrote::Nothing` to `Ok(())`. `runs::undo` then counts the lines as removed (`runs.rs:1976`),
  deletes the notes (`1981-1994`) and marks the record undone (`2031-2036`).
- **Why the tests pass.** The core's test gate composes the two steps correctly
  (`runs.rs:2228-2229`, `.unwrap_or(without)`), so the core tests exercise a gate the program
  does not use. The one window test of undo (`plan_write_tests.rs:105`) splits a document with a
  `#` above its notes, the one shape where a heading does go.
- **Reach.** Split a document whose notes have no section: every note at `#` (the level a plan
  opens at when `#` occurs twice), or only `##` headings with no `#` above. Or split into a group
  that has those sections already. Or add a note of your own under the run's heading. Then Undo.
- **What is left.** N files deleted; N lines in the library file, all missing; the record
  "undone", so Undo is refused from then on; a report titled "Undid the run · N files and N
  lines".
- **How bad.** The end state of F2, on the main path, with no refusal. "Leave the record as it
  was when a step refused" does not catch it.
- **Do.** One function in `runs.rs` that makes both removals and gives `Some` when either changed
  anything, used by both gates. A removal that comes back `Wrote::Nothing` is an error, not
  success. And A5's rule as the backstop.
- **Proven.** `n1_undo_of_notes_under_no_section_deletes_them_and_leaves_their_lines` (window),
  `n1_the_windows_gate_drops_the_link_removal…` (the closure).

### A3. Undo deletes the only copy when the original is gone, and does not say so

- **Where.** `runs.rs:1842-1857`: `prepare` looks at the source only when
  `record.source.recycled` is set, and nothing sets it before phase T. The removals are
  `std::fs::remove_file` (`runs.rs:1989`, `2003`).
- **Reach.** Split. Delete or move the original by hand, which is what an owner who wants it gone
  does today, because the Recycle Bin step is not built. Later, Undo (or Cancel, A1). The
  question is the usual one: "Removes 1 note this run wrote, their 1 lines in Trip, and 1 folder
  if it is empty."
- **How bad.** Text is lost for good, behind a question that does not mention it.
- **Whose fault.** Mine. Plan 4.5 rule 1 says "undo can never be the thing that loses the text"
  and then covers only a source the app itself recycled.
- **Do.** `prepare` compares the source with the recorded length and digest always. Gone or
  different: refuse, or ask a second question that says it ("The original is not where it was.
  These notes are the only copy."). And send undo's removals to the Recycle Bin now, not at
  phase T.
- **Proven.** `a_undo_deletes_the_only_copy_when_the_original_is_no_longer_there`.

### A4. Undo deletes files and folders the run did not create

- **Notes that were already there.** `runs.rs:1280-1287` counts a file with the same bytes as
  written. The record is made at step 1 from the plan (`1174-1187`) and lists it with its digest,
  so undo deletes it (`1859-1869`, `1981-1994`). The same for a picture (`1247-1254`), which has
  no caller yet. Reach:
  tick an "already there" row (Space, or Tick all), write, undo. The file from the earlier run,
  or the owner's own copy, goes, and the earlier run's line stays. The module's first paragraph
  (`runs.rs:4-9`, "only while the file is byte for byte the one it wrote") is not true of these.
- **The group's file.** `group.created` stays `true` in the record when `create_group` failed
  because the file was there (C2). Undo then removes that file if it holds only a heading and a
  `home:` line (`runs.rs:1928-1930`, `2011-2021`).
- **Folders.** `record.folders` is every folder the notes use (`plan_write.rs:206-216`), not the
  folders the run made. An empty folder that was there before goes. Folders made on the way
  (`Trip\Japan` for a note in `Trip\Japan\Tokyo`) are not listed, so they and `Trip` stay after a
  complete undo.
- **Do.** The record holds what happened, not what was planned: `created` per note, picture,
  folder and group file, set only when this run created it, written at step 7. Undo touches
  nothing else.
- **Proven** at the writer for notes and folders (`n3_…`, `n5_…`), with the request built as
  `plan_request` builds it; ticking the row, and the group's file, are **read**.

### A5. Undo keeps a line and deletes its note

- **Where.** `runs.rs:1898-1924`. A line the owner edited is kept and listed. A line the owner
  moved to another section or group is not found and is counted as "gone". In both cases the
  note stays in `plan.notes` and is deleted. The suite asserts it:
  `undo_keeps_a_note_the_owner_changed_and_a_link_the_owner_edited` (`runs.rs:2622-2665`) expects
  `Two.md` removed and `[Two, renamed]` still in the file.
- **Reach.** After a split, rename a note in the pane (F2) or move it to another section, both
  the natural next thing to do. Then Undo. Renamed: the question says "1 line was edited since
  and is kept" and not that its note goes. Moved: nothing is said.
- **A third way in.** `finish` builds its links from `note.path`, the whole path
  (`runs.rs:2099-2105`); the write uses `note.link_path` (`1041-1048`). After Finish no line
  matches its record, every line reads as edited, and undo deletes every note and leaves every
  line. It is also plan fault C again for a finished run. Today Finish is offered only for a run
  left "writing" (C4); once it is offered for "links pending", as it has to be, this is on the
  main path.
- **Whose fault.** Mine. Plan 4.5 gives the rule for lines and the rule for files as two rules,
  and gives the reason for their order as "so the pane never shows a link to a file that is
  gone". With an edited line, the two rules produce exactly that.
- **Do.** Couple them. After the link step, read the library again, and delete a note only if no
  library file lists it any more. One rule, and F2, A2, the three cases here and "the group's
  file was renamed" all become harmless whatever caused them. Keep the fixes for each cause too.
- **Proven** for the moved line and for Finish (`d1_…`, `n2_…`); the edited line is asserted by
  the existing test.

### A6. "Split into notes…" before the library is read writes into the working folder

- **Where.** `app/plan.rs:60` takes `self.docs.library.dir`, which is the empty path until
  something calls `library_load` (`docs.rs:1583-1586`, `1631-1632`). `app/commands.rs:2017-2019`
  opens the plan without loading. The palette (`palette_events.rs:25`) and the tab's menu
  (`input.rs:973`) happen to load first. The main menu's Notes library submenu
  (`commands.rs:1315-1318`, `overlays.rs:178`) and the AI menu (`commands.rs:1323-1326`,
  `overlays.rs:825`) do not.
- **Why nothing stops it.** `inside_any` (`runs.rs:1491-1498`) passes everything:
  `canonical_or_absolute("")` is `""`, and every path starts with the empty path.
- **Reach.** Start mdedit with the pane on Files or shut. Open a document. Main menu → Notes
  library → Split into notes… → Write the notes.
- **What happens.** The folders, the notes, `<name>.md` and `.runs` are made in the folder the
  process was started in. The screen says `library\…`. The pane then reads the real library and
  shows none of it. Undo from the report resolves the record against `library_dir()` and says
  "Nothing of this run is left to remove." When the working folder is the document's own, the
  group file's name is the document's, `create_new` refuses it, and the notes stay unlinked
  beside the document.
- **Also.** `open_runs` lists `self.docs.library.dir` (`plan_write.rs:295-296`) while
  `run_undo_ask`, `run_undo` and `run_finish` resolve against `library_dir()` (`463`, `482`,
  `305`): one record, two folders.
- **How bad.** Nothing is lost. But files are written outside the library by the feature whose
  first promise (plan 3.3 and 6.2) is that it cannot.
- **Do.** `plan_open_active` and `open_runs` load the library and refuse without a folder;
  `write_at` refuses an empty or relative `library`; `inside_any` is false for one.
- **Proven** for the write: `n19_split_into_notes_before_the_library_is_read…` (run alone: it
  changes the working folder). What undo then says is **read**.

### A7. A group whose folder is the library folder turns its notes into library files

- **Where.** `library_write.rs:552-571` accepts any folder. `library_pane.rs:615-618` gives the
  whole path when the folder is the base itself, so the `home:` line names the library folder.
  `runs.rs:1495` lets a note sit directly in it. The picker opens in the library folder for a
  group with no folder (`library_write.rs:525-534`), so "Select Folder" at once is this case.
- **What happens.** "New note here…" and the plan write `.md` files directly in the library
  folder, where the reader takes each for a library file. That is the plan's fault A again, and
  each one counts toward the 64 of fault B, which is the case where the delete guard is blind
  and "Delete this library file…" deletes a note.
- **Do.** Refuse a `home:` that is the library folder, where it is picked, where it is read and
  in `plan_blocked`. In `inside_any`, a note's parent must not be the library folder.
- **Proven.** `n22_…`: the note is `library\Idea.md`, and `library::read` lists it as a group.

### A8. A kept plan writes the text it was made from, after the document has changed

- **Where.** `app/plan.rs:137-145` reopens the kept screen as it is. `runs.rs:1141-1152` reads
  the source and takes its digest when the write starts; the notes come from the `Buffer` the
  plan kept (`plan_write.rs:159`).
- **Reach.** Split, change a tick, Esc (the plan is kept). Edit and save the document. Show.
  Write the notes.
- **What happens.** Notes cut from the old text at the old line numbers. The record holds the new
  file's digest beside the old ranges. Nothing on screen says so.
- **How bad.** Today, notes that are not the document. With phase T it is the loss the design
  exists to prevent: step 8 compares the source with the record's digest, which here is the
  digest of the file as it is now, so it would recycle a document the notes do not hold.
- **Also stale in a kept plan.** `blocked` (computed once, `plan.rs:95`); the "already there"
  answers (the disk cache, `plan_ui.rs:607-618`); the library folder, if the setting changed. And
  `plan_open` drops a kept plan without a word when another opens (`plan.rs:33`).
- **The same family.** `plan_open_active` (`plan.rs:40-58`) does not look at `info.lossy`. Plan
  3.11: "A file with bytes that are not text is refused." Save As refuses such a document
  (`docs.rs:3279`); the split takes it, and the notes hold U+FFFD where the bytes were.
- **Do.** Take the source's length and digest when the plan is made and carry them in the
  `Request`. `write_at` refuses to start when the file differs, and `plan_show` says so. Refuse a
  lossy document.
- **Proven** (`n20_…`); the lossy document is **read**.

### A9. F1 is wider than described

- **Relative paths.** `resolved` (`runs.rs:266-277`) joins a relative path as it stands. A record
  holding `../taxes.md`, with no absolute path in it, is listed by `prepare` for removal. A test
  for `is_absolute` does not close F1. **Proven** (`n13_…`).
- **"Or the group's folder" is circular.** `group.home` comes from the same record. Take the home
  from the group's own library file as it reads now.
- **`group.file` is also written.** It is where `remove_links` writes, through `write_atomic`.
  Confine it to a `.md` directly in the library folder.
- **If the fix reuses `inside_any`,** a record's path becomes a way to end the program (B1),
  until B1 is fixed.
- **The principle the scope asked for.** A record must never give more power than write access to
  the library folder already gives. So: fold the dots, then confine every path to the library
  folder or to a home the group's library file declares now; refuse the whole record otherwise
  and say so in the list; delete only regular files (`symlink_metadata`), and only `.md` and the
  picture types a run writes.

### A10. A finished run deletes the records it cannot read

- **Where.** `runs.rs:973`: a record that does not load gets `started_secs` 0, sorts last, and is
  deleted once there are more than twenty. `Kind::parse` knows only `split` (`127-132`), and
  `version` is read and never compared (`693`).
- **Reach.** The next phases add kinds. A 1.3.0 window on a library a later version also writes
  (two PCs on one OneDrive library, or one PC before and after an update) deletes the newer
  version's records and `.done` files the next time it finishes a run, newest first. Their undo
  goes with them.
- **How bad.** Small today. But it can only be fixed in the version that ships first.
- **Do.** Prune only what loads. Show what does not as "written by a newer version", with nothing
  enabled. Refuse `version > 1` in `from_json`.
- **Proven.** `n6_…`. The scope knows the mechanism for "another program's JSON"; the case that
  matters is this program's own next version.

### A11. `--no-session` does not keep a build off the owner's library

- **Where.** `app.rs:3546-3552` (no session path); `app/completion.rs:70-80` (`snippets_path`
  falls back to the data folder); `app/library.rs:17-29`.
- **What it means.** A window started with `--no-session` reads and writes
  `%LOCALAPPDATA%\mdedit\library`: the owner's real library whenever the setting is at its
  default. CLAUDE.md rule 9 gives `--no-session` as the safe way to start a build. Since today
  such a build can write dozens of files there, delete a library file and undo a run.
- **The same fallback for a user.** A window that could not have its session (`app.rs:3590`,
  `3606`) never read the library-folder setting, so its pane shows, and a split writes into, the
  default folder and not the one the owner chose.
- **Do.** No session, no library, unless `MDEDIT_SETTINGS` names one. Until then, say in rule 9
  that a build needs `MDEDIT_SETTINGS`.
- **Read.** I did not start a window. The one binary I ran was a shot, which has no library.

---

## 2. Crashes

### B1. A file or folder name ends the program

- **Where.** Shared `crates/mdshared-app/src/paths.rs:85-87`, `usable_without_verbatim`:
  `stem.len() == 4 && (stem[..3]…)`. `len` is bytes; `stem[..3]` panics when byte 3 is inside a
  character. It is reached from `canonical_or_absolute` (`paths.rs:24-35`) on Windows, for every
  component of the canonical path, whether or not the file exists.
- **Which names.** A component whose stem is four bytes with a character across byte 3: two ASCII
  letters and a two-byte one (`beş`, `taş`, `dağ`, `maç`, `koç`, `acı`, `clé`, `thé`, `así`,
  `Maß`, `Zoé`); two two-byte letters (`üç`, and any two-letter name in Cyrillic, Greek, Hebrew
  or Arabic); one ASCII and one three-byte character; one emoji.
- **Reach.** Opening such a file, or any file under such a folder, by any route
  (`Docs::open` → `add_pending`, `docs.rs:1907`, `1723`; `app/handoff.rs:50`; the folder
  argument, `app.rs:3657`). Save As to such a name: the panic is before the write
  (`docs.rs:3263`). Renaming a tab to one: the panic is after the file was moved (`docs.rs:3480`,
  `3490`). "Write the notes" with a note or section called so (`library_pane.rs:612-613`,
  `runs.rs:1492`). A right-click in the Notes pane when the library lists such a note, there or
  missing (`library_write.rs:83-88`). "New note here…" in a group called so, after the note file
  is made (`library_write.rs:690`, `696`).
- **How bad.** `panic = "abort"`: the process ends. It has been there since the shared commit
  `1720df5` (2026-11-12): every tag from `v1.0.0` to `v1.2.1` pins a revision that holds it, and
  so does the shared `HEAD` (`fb7777c`). mdview re-exports the function
  (`mv\crates\core\src\files.rs:10`). For a user whose profile folder is `C:\Users\Dağ`, no
  document of their own can be opened.
- **Do.** Compare bytes: `let b = stem.as_bytes(); b.len() == 4 &&
  (b[..3].eq_ignore_ascii_case(b"COM") || b[..3].eq_ignore_ascii_case(b"LPT")) &&
  b[3].is_ascii_digit()`. A test with `beş`, `üç`, `a日` and an emoji. Shared first (rule 8),
  then both apps. And turn on `clippy::string_slice` for the workspace and the shared crates:
  rule 11 is checked by eye today, and this is what the eye missed.
- **Proven**, three ways. The function alone (`%TEMP%\opencode\probe-verbatim\probe.rs`). Through
  the code (`n15_…` in both probe files). And the real binary:
  `mdedit --no-session --scale 1.0 --shot ok.png bes.md` exits 0 and writes the picture;
  the same with `beş.md` exits 101 with `paths.rs:86:21: end byte index 3 is not a char boundary;
  it is inside 'ş'`. Opening a file and writing a plan were run; the other routes above are
  read. mdview was not run.

### B2. A record can name a file that hangs the window or runs it out of memory

- **Where.** `runs.rs:1830-1833` (`digest_of`), `1845`, `2082`: `std::fs::read` of a path taken
  from the record, whole, on the UI thread, with no cap.
- **Reach.** A record naming a very large file, or a device (`CON`, a pipe, `/dev/zero`): "Undo
  this run…" reads it. F1's cause with another effect; confinement removes most of it.
- **Do.** Confine first; then refuse a file larger than a note can be before reading it.
- **Read.**

### Nothing else found that panics

I read for panics, line by line: the JSON reader, `from_json`, and the clock functions of
`runs.rs`; the shared library parser and every writer; `split.rs`; the non-test code of
`plan_ui.rs`; the runs dialogs' drawing (`overlays.rs:1329-1464`, `when` at `1495-1504`).
`plan_draw.rs` I only searched for slicing, `unwrap` and `expect`: none. A hostile 4 MiB record of
`[0,0,0,…]` costs about 64 MiB for a moment; nothing worse.

---

## 3. Wrong, with nothing lost

The first four leave a run half-done: notes on disk, nothing listing them.

### C1. A note under no section cannot be linked into a group that has a section

- **Where.** Shared `library.rs:715-722`: `add_note` with no section appends at the end of the
  file, which is under the last heading. `check_links_added` (`runs.rs:1553-1587`) rightly
  refuses. The comment on `with_links_added` (`runs.rs:1536-1539`) holds only for a file with no
  section yet. The note of "the rest" always has no section (`plan_write.rs:185-201`).
- **Reach.** Split a second document into a group an earlier split gave sections to, with the
  rest ticked, or with any note under no section. One such note fails every link of the run:
  "The notes are written, but Trip.md could not be updated…". Finish fails the same way.
- **In the pane (read, not run).** The same writer, behind checks that only count: "New note
  here…", "Add this file" and "Move to…" on a group's own row put the note under the group's
  last section (`library_write.rs:706-708`, `916-928`; `app/library.rs:386-399`).
- **Do.** In the shared writer, a note with no section goes before the first section heading. It
  changes what an existing caller writes, so by rule 8 it is a variant beside `add_note`.
- **Proven.** `n16_…` in both files.

### C2. What could be refused before the first file is refused after the last

- A new group whose file name is taken: `create_group` runs at step 6 (`runs.rs:1319-1333`,
  `1434-1462`). Reach: rename a group in the pane (its heading changes, its file does not), then
  split a document of that name again. The screen looks groups up by name, takes it for a new
  one, says nothing against writing, writes the notes, and is refused.
- The 64 library files (`runs.rs:1435-1450`), the 2,000 notes (`plan_write.rs:30-37`), and a
  library file with unsaved changes (checked once when the screen opens, `plan.rs:103-123`, not
  when the destination changes, `plan_ui.rs:1007-1011`).
- "New group…" on the screen is always a new group, whatever is typed (`plan_ui.rs:1469-1473`).
- **Do.** See D2.
- **Proven** for the file name (`n7_…`); the rest **read**.

### C3. A section or group called `C#` does not read back

- **Where.** Shared `library.rs:350-356` takes every trailing `#` off a heading. `split.rs`
  (`Atx::text`, `1278-1286`) keeps `C#`, as CommonMark does. The writer writes `## C#`, the
  reader reads `C`, the check refuses. For a group, `create_group` refuses at step 6
  (`runs.rs:1451-1461`), after the notes.
- **Reach.** A document with a `# C#` or `# F#` above its notes, or one called `C#.md`.
- **Do.** One reader of a heading for both crates. Until then, the dry run of D2 catches it
  before anything is written.
- **Proven.** `n17_…`.

### C4. "Links pending" has no way forward

- **Where.** `overlays.rs:1403-1406` offers Finish for `Stopped | Writing` only. The toast
  (`plan_write.rs:110-113`) ends "Try again / Undo", and neither is a control. `State::Stopped`
  is never set by anything.
- **So** a run refused at step 6 can only be undone. And `run_finish` says nothing when it added
  nothing (`plan_write.rs:318-326`), while `finish` marks the run done whenever no write was
  refused, even when it linked nothing because every note had changed or gone
  (`runs.rs:2113-2118`).
- **Whose fault.** Partly mine: plan 4.5 lists "stopped" and "writing" for Finish and forgets the
  state 4.3 step 6 creates.
- **Read.**

### C5. Two titles that differ only in case are given one file

`plan_ui.rs:721` keeps the names taken in a `HashSet<PathBuf>`, which tells `Notes.md` from
`notes.md`; the disk does not. The second note is offered no other name, fails at step 4 as "a
file of that name is there with other contents", and is left out. **Proven** (`n21_…`, Windows).
Compare with `same_path`.

### C6. What undo reports

- A note that could not be deleted is listed as "changed since, kept" (`runs.rs:1989-1993`,
  `replace_ui.rs:401-410`), after its line was removed.
- Undo deletes a note whose tab holds unsaved changes, because the file on disk still matches.
  The text survives in the tab only.
- `with_headings_removed` looks for notes under a heading and nothing else
  (`runs.rs:1662-1668`): the owner's prose or own sub-heading under a heading the run made loses
  that heading.
- **Read.**

### C7. The destination is a name, looked up three ways

`plan.rs:80` (ignoring case), `plan.rs:116` (exactly), `plan_write.rs:146` (ignoring case, the
first match). With two groups of one name, the notes go to the folder of the one chosen and the
links to the file of the first. `plan_blocked` also treats the Windows read-only attribute of a
folder as "cannot be written" (`plan.rs:108`), which it does not mean. Carry the group's file in
`Dest`. **Read.**

### C8. Costs on the UI thread

The scope knows the write has no worker and no Cancel. To add: `finish` reads and parses the
group's file once per note (`runs.rs:2088-2095`); `prepare` runs twice per undo and `undo` hashes
again, so every note is read three times. **Read.**

### C9. Smaller

- A library file with a BOM: the gate reads with `read_to_string` (`library_write.rs:134`), the
  reader strips the BOM, so "Rename" and "Set the group's folder…" quietly do nothing on it.
- `write_atomic` (shared `settings.rs:95-98`) creates `<name>.tmp` beside the target with
  `File::create`, which truncates a file of that name if the owner has one, and leaves it behind
  when the write fails.
- `split.rs` promotes any `#` line outside a code fence, so a `## x` inside an HTML comment or a
  `<pre>` loses a `#`. A note started at line 1 of a document with front matter gets its
  `# title` above the front matter.
- `plan_request` skips a ticked note without a word when it has no target
  (`plan_write.rs:156-158`). I could not reach it: every change goes through `refresh`. It should
  be an error.
- In the shared reader, a `##` heading above the file's first `#` heading stays open for `parse`
  and is closed for `heading_line`, so the reader and the writer place a note differently in a
  file shaped like that. Only a file edited by hand is.
- `docs/release-notes/1.3.0.md:7` says undo "asks first… takes the links out of the library, then
  removes only the files whose contents are still exactly what it wrote". Until A1, A2 and A4 are
  fixed, each of the three is untrue.

---

## 4. Where the design is wrong

The first three are mine.

- **D1. Undo's two rules have to be one** (plan 4.5; A5). Never delete a note that a library file
  still lists.
- **D2. Check first, then write** (plan 4.3; C1 to C3). Step 6 holds two things: making the
  group's file, which is a precondition, and adding the links. Before step 1, with nothing
  written: is the group file's name free, are the two caps clear, is the library file free of
  unsaved changes, is the source still the file the plan was made from, and does a dry run of
  `with_links_added` and `check_links_added` on the file as it is now pass? Then make the group's
  file before the notes. An empty group is a state one can live with, and undo already removes
  it. What is left for step 6 to fail on is a race.
- **D3. Undo and the original** (plan 4.5 rule 1; A3). "Undo never loses the text" has to hold
  for an original the owner removed, not only one the app recycled.
- **D4. The record holds the plan, not what happened** (A4). Write `created` after the fact.
- **D5. A record is a file format** (A9, A10, B2). Check the version, never delete what cannot be
  read, confine after normalising, cap what is read.
- **D6. One contract, two gates, and the tested one is the fake** (A2). Put the composition in
  the core, where the tests are.

---

## 5. Not to my taste

Kept apart, as asked. None of these is a fault.

- Enter is "Undo" in a question about deleting. It matches the other questions.
- "their 1 lines" (`runs.rs:1770`).
- A toast that ends "Try again / Undo" reads as two buttons.
- The last condition of `outside` (`runs.rs:1527`): `&& home != &req.library` waives the check on
  the group's file when the folder is the library. No caller reaches it.
- Step 5 compares a string with itself (`plan_write.rs:163`). `Plan::check` is O(lines); once per
  write, in release, is affordable, and it is the check the step's name promises.
- `plan_ui.rs` and `app/plan.rs` still open by saying the screen writes nothing.
- `Record::group_name` is the file's stem and `group.name` is the heading; the list shows one and
  the question the other.

---

## 6. Read, and nothing found

- **The write, against plan 4.1.** A taken name is never written to: `create_new` refuses before
  a byte, and the `AlreadyExists` arm only reads. Folders are only created. The library file is
  rewritten only through the set check. The record and its `.done` are the run's own. What is
  left is the rename window `create_new`'s own comment admits. Of the seven operations, Picture,
  Text note and Recycle have no caller.
- **Names that become paths.** `folder_name` and `note_folder`: separators, `%`, trailing dots
  and device names are replaced; `..` comes out as `Untitled`. I found no title or section that
  leaves the group's folder.
- **The split's invariant.** Notes are sorted and never overlap, and the rest is computed from
  what is ticked, so it holds by construction. The outline and `atx()` follow the same rules; I
  found no line they read differently.
- **The JSON reader**, for panics: none.
- **"Already there" by FNV-1a.** A collision leaves a note unwritten and says "already there";
  the writer compares bytes, so nothing is overwritten.
- **The delete guard** (`library_own_file`, `library_lists_as_note`): as the scope describes it.
  Nothing new beyond the 64 and A7.
- **The library write gate** (`library_attempt`): it reads, changes, checks and writes with no
  stamp comparison, as the scope says. The window is the time of one parse. Nothing beyond that,
  apart from A2's reading of `Wrote::Nothing`.
- **`barely_seen`** (`app.rs:2452-2467`): pinned and unsaved tabs come back either way.
- **`folder_lock.rs`, `settings::read` and `read_patiently`** at `c603b23`: nothing.

## 7. Not read

Send the next reviewer here. Nothing in this report speaks for it.

- The audit 2 fixes: the session and the lock in `app.rs` `run()`, `modified()` against
  `has_changes()`, `timed_exit`, `finish_session`, recovery, the updater, `host_of`; visual
  editing; `f/four-fixes`.
- The Notes pane's keyboard and drawing (`library_key`, the rows, `follow`), and the phase 1
  switches.
- The visual-editing scrollbar, and the owner's open report about the box.
- Save As and the language's extension; the shared `check` and `segments`; `mdshared-app::job`.
- `plan_draw.rs` beyond the search; `plan_demo.rs`.
- The measurement document's method.
- `same_path` at the pin compares two lowercased strings, so separators and `..` decide. The fix
  (`fb7777c`) is on the shared main and not pinned. `undo` and `finish` use it to match a
  record's paths against the library's. In the flows I followed, both sides are built from the
  same strings, so they agree. A link or a `home:` line spelled by hand with a doubled separator
  or a `.` is the case I did not follow.
- macOS and Linux. B1 is Windows only; C5 was run on Windows only.

## 8. The probes

`%TEMP%\opencode\audit3-probe-tests.patch` adds two files and one `mod` line. It applies to
`6cdeab2` (`git apply --check`). Every test passes there and asserts the fault at a line marked
`FAULT`; turned round, each is the test for its fix.

- `crates/mdedit-core/tests/audit3_probe.rs`, 11 tests:
  `cargo test -p mdedit-core --test audit3_probe` — A2 (the closure), A3, A4 (notes, folders),
  A5 (moved line, Finish), A9, A10, B1, C1, C3.
- `bins/mdedit/src/app/audit3_probe_tests.rs`, 10 tests:
  `cargo test -p mdedit --bin mdedit audit3_probe_tests` — A1 (two), A2, A7, A8, B1, C1, C2, C5;
  and A6 alone, because it changes the working folder:
  `cargo test -p mdedit --bin mdedit n19_split -- --ignored --test-threads=1`.

Left in `%TEMP%\opencode`: `audit3-build` (the copy, 11 MB), `audit3-target` (2.8 GB, can go),
`probe-shot`, `probe-verbatim`, `shared-c603b23` (the shared files as pinned), `audit3-notes.md`
(my working notes).

## 9. The order I would fix in

1. A1: one line, and it is the button that says no.
2. B1: in the shared crate, for both apps, and a release of each.
3. A2 with D1: the composition, then the rule that makes the class harmless.
4. A3: look at the original; the Recycle Bin for undo's removals.
5. A6: load the library; an empty library is refused by the writer itself.
6. D2: the checks before step 1. It closes C1, C2 and C3 together.
7. A4 and D4, A9, A10, before the record's format is in the field.
8. A7, A8, A11, C4, C5.

On the five already found: F2's fix does not cover A2. F3 is in the only path through which undo
writes the library file today, so it and A2 are one change. F1 needs A9's four points. I have
nothing to add to F4 and F5.
