# Review C: 4a (add a folder), phase 5 (Markdown note), DD (drag a note)

Scope: `git diff 6cdeab2 1d503d8` in worktree `C:\Users\kaank\Desktop\md-wt\fab-c` (detached at 1d503d8), shared crates read at `0984eed`. Read-only: nothing committed, the worktree is clean again (`git status` empty). Probes were appended to the existing test files, run, saved as a patch and reverted.

- Probes: `C:\Users\kaank\AppData\Local\Temp\claude\ddd3b02074f601ee\e3635bc8-7486-4b64-a984-53cb1ce200e6\scratchpad\fable\c-probes.patch` (10 tests; `git apply` it in the worktree to re-run).
- Run with: `cargo test --release -p mdedit --config 'build.rustc-wrapper=""' -j 2 probe_ -- --nocapture --test-threads=1` and the same with `-p mdedit-core --lib`.
- Full output of the first run: `C:\Users\kaank\AppData\Local\Temp\claude\ddd3b02074f601ee\e3635bc8-7486-4b64-a984-53cb1ce200e6\scratchpad\fable-c-probe.log`.

## Summary

No data loss and no crash found. Nothing of the owner's is copied, moved, renamed or deleted by any of the three features, the import's dry run does come before the group's file is made, and a junction loop is not followed (proved).

What is wrong is behaviour at the edges. The ones that matter most:

- **DD** writes when it should not: a note let go over the editor (outside the pane) is still moved, and the drag holds its note by row index, so rows made again under it move a different note.
- **Phase 5** loses the AI's answer when the write fails after it started (the card is gone, nothing is said, and a "done" record of nothing is left).
- **4a** joins a same-named group whose `home:` is another folder while the card says otherwise (DESIGN 81 says this is refused); offers the library's own files as notes when the folder picked holds the library; lets one file name with `%20` refuse the whole folder without naming it; and takes a multi-line "title" from a file with bare CR line ends.

Counts (12 findings): data loss 0, crash 0, wrong behaviour 9 (7 proved by a failing probe, 2 plausible from reading), test gap 1 (four gaps in it), nit 2 (1 proved).

---

## Findings

### 1. DD: a note let go outside the pane is still moved — wrong behaviour, PROVED

`bins/mdedit/src/app/library_drag.rs:29-31` and `:74-77`, `bins/mdedit/src/library_pane.rs:251-262` (`row_at`) and `:287-292` (`drop_row`).

The drop target is found from the pointer's **y only**. `row_at` checks `y` against the list's top and bottom and never looks at `x`; neither `library_drag` nor `library_drag_release` does either.

Scenario: drag "Taxes 2026", carry it right over the editor to cancel (or by overshoot), let go at a height where the pane has a row. The heading at that height is marked and the note is moved there.

Probe `probe_a_drag_let_go_over_the_editor_at_a_rows_height_still_moves_the_note` (pointer at `areas.text.cx()`, the y of the "Trips" row):

```
PROBE-D1 drop mark = Some(5)
PROBE-D1 file changed = true
PROBE-D1 toast = "“Taxes 2026” is in Personal now."
```

DESIGN 80 says "a drag let go outside the pane moves nothing and opens nothing".

Fix: give `drop_row`/`drop_place` the x as well and return `None` unless the point is inside the list rect (`area.contains(x, y)`); use it in `library_drag`, `library_drag_scroll` and `library_drag_release`.

### 2. DD: the held note is a row index, and rows can be made again under it — wrong behaviour, PROVED

`bins/mdedit/src/ui.rs:815` (`Drag::LibraryNote { row, .. }`), `app/input.rs:902` (press), `app/library_drag.rs:83-92` (release reads `rows.get(source)`), `app/library.rs:377-382` (the only guard), `app/input.rs:1422-1432` (only Esc is taken during a drag).

The drag remembers the note as an index into `ui.library.rows`. Only `library_tick` is held back, and only once `moving` is true. Everything else that rebuilds the rows still runs while the button is down: every key reaches the pane (Enter/Left/Right fold, Delete removes, F2 renames), and so does any other caller of `library_rows()` / `library.build()`. At release the row at that index is whatever is there now, and `library_move`'s own `note_at` check agrees with it because line and path are read from the same (wrong) row.

Probe `probe_rows_made_again_under_a_held_note_move_another_note`: "Two" is held; the keyboard's row is on section "A" and Enter folds it; the note is let go on "Trips".

```
PROBE-D2 rows after the key = ["Personal", "A", "B", "Two", "Three", "Trips"]
PROBE-D2 One under ["A"]
PROBE-D2 Two under ["B"]
PROBE-D2 Three under ["Trips"]
PROBE-D2 toast = "“Three” is in Personal now."
```

"Three" was moved; "Two", the note held, was not. The same hole exists without a key in the first 4 px: before `moving` is true `library_tick` may take a finished re-read, and the click or drag that follows acts on the row that slid into that index (plausible, not probed).

Fix: hold the note's identity in the drag (library file, line, path, title — what `Moving` already is for "Move to…"), and at release find the row by it or use it directly; if it is no longer in the rows, drop nothing. Also swallow every key but Esc while `moving` is true, and make the tick guard cover a `LibraryNote` drag whether or not it has moved.

### 3. Phase 5: a write that fails after it started drops the card and the answer, and says nothing — wrong behaviour, PROVED

`bins/mdedit/src/app/note.rs:464-563`, in particular `:465` (`self.note.take()`), `:540-542` (only `start_error` gives the card back) and `:544-559`.

`note_write` hands the card back only for `report.start_error`. Any failure inside `runs::write_note` after the record is made (`note_failures`, `folder_failures`, `check_failures`) falls through: the card is already taken, no toast is set, and the AI's answer is gone. DESIGN 82 says "A failure from the write itself comes back as `note_write_refused`: the card stays with the reason".

The simplest way in: the free name is chosen when the card opens (`note_target`, `free_name`) and `note_blocked` never looks at it again, so a file that takes that name while the answer is out fails the write. A home folder that cannot be made or written to, or a full disk, goes the same way.

Probe `probe_a_name_taken_while_the_answer_was_out_drops_the_card_and_the_answer`:

```
PROBE-N1 card open = false, toast = "", file = "someone else's file\n"
PROBE-N1 records = 1 (+0 unread)
PROBE-N1 record state = Done created = [false]
PROBE-N1 group = "# Recipes\nhome: Recipes\n"
```

Nothing of the owner's is harmed (the other file is untouched, no line is written). But the answer, which may have taken minutes and money, is lost, and "AI runs…" gains a record in state Done that made nothing.

Fix: in `note_write`, when `report.notes() == 0` (or any of the three failure lists is non-empty), give the card back through `note_write_refused` with the first failure's reason. Before the write, pick the free name again (`free_name`) if `card.target.path` exists, or add "the name is taken" to `note_blocked`. Consider not leaving a Done record for a run that wrote nothing (shared `write_of`: state when `written` is empty).

### 4. 4a: a group of the same name with another `home:` is joined, and the card says its folder is the one picked — wrong behaviour, PROVED

`bins/mdedit/src/app/importing.rs:159-176` (the group is found by name alone, ASCII case ignored), `bins/mdedit/src/import_ui.rs:104-117` and `:123-136` (`group_line` prints `plan.home`, the folder picked), `crates/mdedit-core/src/import.rs:440`.

DESIGN 81 lists among the refusals "a link that does not sit in the library or the group's home". No such check exists in `import_links`. A folder called `Recipes` (or `notes`, `docs`) picked anywhere joins the library's existing "Recipes" group, whatever folder that group's `home:` names, and the card reads "The group is "Recipes", which the library already has, with its folder <the folder just picked>", which is not that group's folder.

Probe `probe_a_group_of_the_same_name_with_another_home_is_joined_without_a_word` (group's home is `…/elsewhere/Recipes`, the folder picked is `library/Recipes`):

```
PROBE-I2 card still open = false, refusal = None
PROBE-I2 group file = "# Recipes\nhome: C:/…/elsewhere/Recipes\n\n- [notes](Recipes/notes.txt)\n\n## Bread\n\n- [Sourdough](Recipes/Bread/sourdough.md)\n…"
```

The group now lists notes outside its home, which is what "Move the notes" and the note card's naming assume does not happen for an imported group. D-132 says an existing group is joined with "its `home:`"; it does not say what happens when that home is another folder, so this needs the owner's word: refuse (what DESIGN 81 says), or join and say the truth.

Fix: carry the joined group's real home into the plan (`Where.group` gains it; `plan.home` is the group's, not the folder's). Then either refuse when the folder picked is neither the group's home nor inside it, or keep joining and have `group_line` name the group's real folder and say the files are listed from another one. Correct DESIGN 81 to match whichever is chosen.

### 5. 4a: the folder that holds the library offers the library's own files as notes — wrong behaviour, PROVED

`bins/mdedit/src/app/importing.rs:125-137` (`import_ready` refuses only the library folder itself), `crates/mdedit-core/src/import.rs:66-162` (the walk knows nothing of the library).

Picking the parent of the library folder (say the library is `Documents\Notes\library` and `Documents\Notes` is picked) walks into the library and takes its library files, ticked, alongside every group's notes again under a "library" heading.

Probe `probe_the_folder_that_holds_the_library_lists_the_librarys_own_files_as_notes`:

```
PROBE-I3 notes = ["library\\personal.md ticked=true", "library\\Recipes\\notes.txt ticked=true", …]
PROBE-I3 problem = None
```

Adding them makes a library file a note of another group. From then on "Delete this library file" refuses it as a note (`library_lists_as_note`), and a second import of the same folder offers the new group's own file as a note of itself. A7's reason ("would make every file in it a library file") applies to this case too.

Fix: pass the library folder to `walk` and skip it as a directory (compare with `same_path` after `canonical_or_absolute`), or refuse in `import_ready` when the library is inside the folder picked, in A7's words.

### 6. 4a: one file name the library cannot write refuses the whole folder, without naming it — wrong behaviour, PROVED

`bins/mdedit/src/app/importing.rs:353-363` (the dry run), `crates/mdedit-core/src/import.rs:614-628` (`links`); the cause is the shared reader, `mdshared-app/src/library.rs` `unwrap_path` (`%20` reads as a space, and there is no way to write a literal `%20`).

A path containing `%20` (a file saved from a browser: `my%20draft.md`, or any folder above it when the link is whole) reads back as a different path, so `check_links_added` fails for the whole plan. The card shows no problem until Add is pressed, and then says only "Recipes.md would not read back as its old notes plus the new ones." Not which file, and no way forward but to find it by hand and untick it.

Probe `probe_one_file_with_percent_20_in_its_name_refuses_the_whole_folder`:

```
PROBE-I1 problem on the card = None
PROBE-I1 card still open = true, refusal = Some("Recipes.md would not read back as its old notes plus the new ones.")
PROBE-I1 group file made = false
```

It is safe (nothing is written, the group's file is not made: the dry run does its job). The same applies to a relative path whose first character is a space, and on Linux to a name with a line break or ending in `>` after a `<`.

Fix: check each link on its own when the plan is built (`library::parse(&link.line)` gives back the same path and title), mark such a row "cannot be listed: its name has %20 in it", leave it unticked and unable to be ticked, like `already`. Then the rest of the folder can go in, and `problem()` can name the file if one is somehow ticked.

### 7. 4a: a file with bare CR line ends gives a "title" of many lines, written into the library file — wrong behaviour (minor), PROVED

`crates/mdedit-core/src/import.rs:224-249` (`title_of` uses `str::lines`, which does not split on a lone `\r`), `:261-271`, `:614-628`.

An old Mac text file (`# Shopping\rmilk\reggs\r…`) is one "line" to `lines()`, so its title is the heading plus everything after it, up to the 8 KB head, with the CRs in it. The library reader does not split on `\r` either, so the dry run passes and the line is written.

Probe `probe_a_file_with_bare_carriage_returns_gives_a_title_of_many_lines`:

```
PROBE-C2 title = "Shopping\rmilk\reggs\rand a long tail of the file"
PROBE-C2 line = "- [Shopping\rmilk\reggs\rand a long tail of the file](Old/mac.txt)"
PROBE-C2 problem = None, dry run passes = true
```

The library file then holds stray CRs. Opened in an editor that shows or normalises mixed line ends, that one line becomes several and the link breaks on the next save. There is also no length cap on a title: a single 8 KB heading line is written whole.

Fix: in `title_of` split on `['\n', '\r']`; in `links()` (or `heading_text`) drop control characters and cap the title (say 200 characters); fall back to the file's name when nothing is left. `preview_of` wants the same split.

### 8. 4a: a walk cannot be stopped, has no bound on folders, and its card opens whenever it ends — wrong behaviour, PLAUSIBLE (read, not probed)

`bins/mdedit/src/app/importing.rs:52-114`, `crates/mdedit-core/src/import.rs:78-162`.

- The walk stops at 2,000 **files taken**, not at any number of folders or entries read. A folder with few `.md`/`.txt` files and a very large tree (a drive root, a photo archive, a home folder) is walked to the end.
- `Job` has a cancel flag and the walk looks at it, but nothing in the app ever sets it: there is no Cancel for the "Reading the files in …" toast (the comment at `app/commands.rs:2363` says "unless the folder was let go of", and no code lets go of it). Until it ends, "Add a folder" answers "A folder is being read for the library already."
- When it ends, `import_take` opens the card over whatever is on screen then. `import_open` closes the palette and popups but not Settings, the note card, the plan or the task window, so two cards can be open at once.
- Reading the first 8 KB of every file will make a cloud folder (OneDrive files-on-demand) download each file.

Fix: count folders and entries read and stop with `more` at a bound (for example 50,000 entries); give the toast or the status bar a Cancel (Esc) that calls the job's cancel and clears `import_job`/`import_dir`; in `import_take` hold the card back while another card is open, or refuse the command while one is.

### 9. 4a: a new group is made in two writes — wrong behaviour (minor), PLAUSIBLE

`bins/mdedit/src/app/importing.rs:364-394`.

For a new group the file is first created with its heading and `home:` (`create_new`) and the links go in by a second write. A crash or power loss between the two leaves a group with no notes; `remove_own_file` only covers a refusal. DESIGN 81 describes the write as "one `library::rewrite`". The dry run has already produced the whole text (`new`, line 356).

Fix: for a new group, `create_new(&plan.group_file, new.as_bytes())` with the text of the dry run, then `library_changed()`; one exclusive write, no second step and no `remove_own_file` needed on that path.

### 10. Test gaps — test gap

1. `library_drag_tests.rs:141-158`, `a_drag_let_go_outside_the_pane_moves_nothing_and_opens_nothing`: it passes only because the middle of the text area is lower than the fixture's six rows. With the x ignored (finding 1) it still passes; with a longer library it would fail. It should let go at a row's y, outside the pane's x, as probe D1 does.
2. `app/importing.rs:431-438`, `remove_own_file` ("removing only a file this run made"): no test reaches it. Every refusal the tests make (`library_file_free`, the 2,000, nothing ticked) is returned before the group's file is created, so deleting the two `remove_own_file` calls fails nothing. The same goes for its "still holds exactly what was put in it" comparison.
3. The dry run before the group's file is made (`importing.rs:353-363`): no test has a plan that passes `problem()` and fails the read-back, so moving the `create_new` above the check fails nothing. Probe I1 is such a test (`group file made = false`).
4. The walk and links to folders (`import.rs:102-116`): no test has a symlink or a junction. Removing the `kind.is_symlink() && meta.is_dir()` test fails nothing. Probe C1 (a junction back to the folder, Windows) passes on the current code and would loop until the 2,000 without it.

Phase 5: nothing tests a write that fails after the record is made (finding 3), and `undo_from_ai_runs_takes_the_note_and_its_line_out_again` checks only that the group file no longer contains the name, not that it is what it was (finding 11).

### 11. Phase 5: undo leaves a blank line behind — nit, PROVED

Shared `runs::with_links_added` / `with_links_removed` through `PlanGate`; seen from `app/note.rs`.

```
PROBE-N2 before  = "# Recipes\nhome: Recipes\n"
PROBE-N2 written = "# Recipes\nhome: Recipes\n\n- [pasta notes](Recipes/pasta-notes.md)\n"
PROBE-N2 undone  = "# Recipes\nhome: Recipes\n\n"
```

The note, its line and a folder the run made all go (probe N3: a group with no `home:` line gets `library/Recipes`, and undo removes it again), but the blank line the add put before the link stays, so the library file is not byte-for-byte what it was. It is the split's own machinery and harmless to the reader; it matters only if "undo restores exactly" is meant literally.

Fix: when the removed line was the last line of the file and the line before it is blank and was added by the run, take that blank line too; or record whether the add wrote a separating blank line.

### 12. Smaller things — nit, read only

- `app/library_write.rs:992-1030` reached by a drag: across two files the destination is written first and the source line is checked second. A source line number that is stale (the file changed and the pane has not re-read; the drag holds the re-read back) lists the note in both, with the "half" toast. Checking `note_at` on the source before writing the destination costs one read. Same for "Move to…", so not new.
- A drag across files rewrites the line as `- [row label](path)`: a line that was only a path gets the file's title frozen into it, and `* ` or `+ ` becomes `- `. D-130 says "moves its line, and only its line"; within one file that is true.
- `app/note.rs:54` and `:66`: the source is read twice (the loader for the text, `std::fs::read` for the digest, the second one with no size cap). A file changed between the two gives a card whose text and digest are of different versions; the write-time check then passes against the wrong one.
- `app/note.rs:433`: a source with bare CR line ends gives a note written with bare CRs.
- `app/note.rs:565-571`: `note_write_refused` sets `blocked`, which disables the button for good, also for a refusal that could pass on a second try ("could not be read").
- `crates/mdedit-core/src/import.rs:423-432`, `:454-482`, `:655-662`: on a case-sensitive file system, sibling folders `A` and `a` can give a section row twice, and a section's box ticks both (`under` ignores ASCII case, `rows_of` does not). Linux and macOS only.
- `import.rs:405`: `already` is `same_path`, text only. A note the group lists through `..`, a link or an 8.3 name is offered again and would be listed twice.
- `import.rs:78`: the walk recurses once per folder level with no depth bound. Only a path thousands of levels deep could exhaust the walk thread's stack (which would abort, rule 11); out of reach with normal path limits.

---

## Checked and found sound

**4a**
- Nothing of the owner's is written: the only writes are `create_new` of the group's file and `write_atomic` of that same file (`importing.rs:364-383`). No copy, move, rename or delete anywhere in `import.rs`, `importing.rs`, `import_ui.rs`.
- The dry run comes before the group's file is made, on the text as it reads now, and the real write re-reads and checks again (`library_attempt_at`). Proved by I1: refused, no file made.
- `remove_own_file` removes only a file this run made and only while its bytes are still the ones it wrote.
- A group file that exists but is not a group of that name is refused twice (plan and write); `MAX_FILES` and the 2,000 are checked; an unsaved tab on the group's file refuses before anything is made.
- Links: relative inside the library, whole outside (`tree::link_path`, canonicalised on both sides); `home:` the same. Paths with spaces go in `<>` and read back; `[` `]` are stripped from titles so a title cannot break the link.
- Symlinks and junctions to folders are not followed; a junction back to the folder does not loop (proved, C1). Hidden, dot, `target`, `node_modules` are skipped.
- No panicking path on what a folder holds: names are lossy, the head is capped at 8 KB and decoded lossily, `heading_text` and `cut_bytes` slice at ASCII or checked boundaries, `civil_from_days` and `modified_day` cannot overflow on real times.
- Names: a group or subfolder ending in `#`, an empty or blank name, a name that cannot be a file stem are refused; `# C#` as a title reads back as `C` and the plan says so.
- Over 2,000 files: the walk stops and the plan is refused with the count; the card's selection, scroll, hit-testing and keys index nothing unchecked.

**Phase 5**
- The source is never written; the note is `create_new` (never over a file: N1 shows the other file untouched) and is UTF-8.
- Everything is re-checked at write time: library folder, `note_blocked`, the source by length and SHA-256.
- `outside()` / `inside_any` keep the note within the library or the group's home; a group with no `home:` gets `library/<name>/` and the link is relative (N3).
- The record is `Kind::Markdown`; undo removes the note only while its digest is the recorded one and the source is still the recorded file; an edited note is kept; the folder the run made goes (N3). `recycle_source` is for `Kind::Split` only, so a Markdown run can never bin its source.
- A late answer cannot land on the wrong card: `Done`, `Error` and `Ended` are matched on the request id (`ai_events.rs:516-520`, `:560-568`).
- `note_body`, `fit_end` and the card's keys and hits have no panicking path.

**DD**
- A drop on the note itself, on a note of the same heading, or on its own heading writes nothing (`same_place`, checked again at release).
- The write is "Move to…"'s: both files checked for unsaved tabs before either is written, each write re-reads and checks the count, the destination at 2,000 is refused, the note's file is never touched.
- Esc clears the drag, the mark and the timer before any other key handling; a press and release is still a click; the edge scroll re-arms only while the pointer stays at the edge.
- Drawing indexes nothing unchecked; the region signature carries the drag.

## Not got to

- `import_draw.rs` and `note_draw.rs` were scanned for slicing and indexing only, not read line by line; no shots were taken or looked at.
- The AI sort on the import card (the other reviewer's), `mdedit_core::hunks` (the note card's diff; a 200 KB source against a wholly different answer runs on the UI thread, not measured), `overlays.rs`' "AI runs…" row for a Markdown record, `run_show`/`run_finish` for it.
- The shared `write_of` / `undo_with` beyond what `Kind::Markdown` adds (audit 3 covered the split's path): a crash between a note's `create_new` and the record's next `close` was not re-examined.
- Linux and macOS behaviour (the async pickers, case-sensitive names) was read, not run. Nothing was run in the app itself; all proof is from tests in the worktree.
- Budgets were not measured (read-only review).
