# Audit 3: what to review (for Fable)

Written 2026-11-21 for mdedit 1.3.0. This is the second writing of it: the first was at `df562e8`
and said phase 6a was not built. Everything below is `main` at `a9a694e`; the commits are named
rather than a sha, because the tree moved while this was written.

Read `docs/STATE.md` first ("Not measured, not reviewed, not checked by hand"), then
`docs/DECISIONS.md` D-059: one reviewer, this document as the brief, kept up to date as work lands.
Audit 2's findings were all fixed for 1.2.0, but nobody has reviewed the fixes. Review them, plus
everything merged since 1.2.1. Audit 2's probe tests are in
`docs/archive/review-prompts/audit2-probe-tests.patch`, and audit 1's rounds are beside it. The old
`docs/HANDOFF.md` is now `docs/archive/HANDOFF-2026-11-13-to-2026-11-21.md`.

The document was first written before the notes library existed. Since `v1.2.1` a whole feature has
been merged and nobody has read it: the notes library, its redesign phases, the shared writers it
moved, the split model, the plan screen, and now the writer with its record and undo. 1.3.0 has no
tag at the time of writing (the version is 1.3.0 in `Cargo.toml`, with
`docs/release-notes/1.3.0.md`), so code and notes may still move.

## The audit 2 fixes (release/1.2.0, merged to main for 1.2.0)

A. Unsaved text, session and updater (commit `f9a32c9`):
1. One window per settings folder. This was mdedit's `instance.rs` in 1.2.0; it is
   `mdshared-app`'s `folder_lock` since `2c23a71` (shared `cb0ffd3`), so the code checks are in
   the shared entry at the end of the new section. Compressed here because of that move; what it
   must still do is `claim`, the lock on `settings.ini.lock`, the 10 s wait, `Busy`, and its use
   in `app.rs` `run()` (`instance_path`, `_folder_lock`, `no_session_note`). Check:
   - two launches at once;
   - a launch next to a hung window;
   - a window from before 1.2.0, which holds no lock. Seen once on the owner's PC: a 1.2.0
     launch next to a running 1.1.2 got no answer to its hand-over, ran on the same session,
     took `instance.port` and renamed the 1.1.2 window's recovery copies. Why the hand-over got
     no answer is not known.
   - the restart path (`relaunch::wait_for_predecessor`).
2. `Session::read` / `read_patiently`: an unreadable session is an error, never an empty one; the
   window then runs without its session. Compressed as well: since `2c23a71` these are
   `mdshared-app::settings::read` and `read_patiently` (shared `cb0ffd3`), and mdedit's
   `session.rs` calls them. The shared `settings::write_atomic` syncs before the rename
   (markdown-shared-rs `75ddc1a`).
3. `Doc::modified()` against `has_changes()` (`docs.rs`): "Don't save" counts only once the
   run ends. Check every caller of both for the wrong choice.
4. `timed_exit()` (`app/commands.rs`) and `finish_session()` (`app.rs`).
5. `Fail::hide` and `proto::redact` back each other up: the engine test fails only with both
   removed. The same holds for the `sse` `Watched` checks.
6. Smaller:
   - `recovery::cover`'s new `same_text` (read in `bring_back`, `app.rs`): an older copy with
     the same text as its backup covers it.
   - `Recovery::finish` skips the copies of discarded tabs.
   - The updater's `FILES` mutex and mdedit's own `spawn_install` (`updater.rs`); the shared
     `write_synced` (markdown-shared-rs `8b73e01`). `missing_parts` still checks only that the
     files exist.
   - `host_of` (`bins/mdedit-ai/src/api.rs`).

B. Visual editing (commit `2496e6d`, with mdview `e2b3e31` on `f/block-edit` adding
`Request::Unfollow`). The fixes are in `app/visual.rs`, `app/preview.rs`, `preview_pane.rs`
(`gap_due`, `unfollow`), `app.rs` (the box clipped to the page) and `app/commands.rs` (the
edits gate, Select All).

C. `f/four-fixes` (commit `b06800a`): renaming (the recovery copy, a move that never replaces),
undo after an AI conversion, providers without a key, Save As and the language, settings
migration. Its section in the archived handoff lists each fix with its test.

## Changed after audit 2 (in 1.2.0)

- In visual editing, a right-click in the open block's box opens the text's menu (AI and the
  rest), not the page's (`right_press`, `app/input.rs`).
- The Modify button of a saved prompt in the AI submenu lights up at once: its hover is in the
  overlay signature (`app.rs`, `osig`). No test.
- Test `the_open_block_s_box_moves_with_the_page_it_is_on` (`app/preview.rs`).

## Merged since 1.2.1 (on main, not reviewed)

Everything below is code nobody has read since `v1.2.1`. The commits are named so a reading can
be checked against them. One feature runs through most of it: the notes library, a folder of
Markdown files that list notes kept anywhere, shown in the left pane and written by menus and by
the keyboard; and, at the end, a plan screen that cuts a document into notes and writes them.

### Where a fault would be worst

Until today nothing in this program created, wrote or deleted a file on the owner's behalf beyond
one note at a time. **Write the notes** now creates a folder per section, a file per note, a record
and its `.done` file in one press, and **Undo this run** removes files again. The centre of gravity
has moved from reading text to deleting files. Look here first, with the will to break it rather
than to follow the happy path:

- **Undo, which deletes files, and decides which from a JSON file.** `runs::prepare` and
  `runs::undo` (`crates/mdedit-core/src/runs.rs`) turn a record into a list of paths and remove
  the ones whose SHA-256 matches what the record says was written. Two attacks:
  - the record is a file in `<library>/.runs`, in a folder the owner may have put in OneDrive or
    on a share, and nothing proves this program wrote it. `resolved` (`runs.rs:266`) takes an
    absolute path as it stands, and neither `prepare` (`:1836`) nor `undo` (`:1960`) asks whether
    a path is inside the library or the group's folder — unlike the write, whose `outside`
    (`:1501`) does. A record naming `C:\Users\kaank\Documents\taxes.md` with that file's real
    digest is listed by "AI runs…" and deleted by Undo, behind a question that says the run wrote
    it. `folders[]` goes to `remove_dir` anywhere, and `group.file` to `remove_file` wherever a
    file holds exactly a `# …` line and a `home:` line.
  - the line that decides is the digest, so a record whose digest is wrong keeps everything, and
    one whose digest is right removes a file the run never wrote. Ask how a record could be
    forged, and what Undo should refuse on principle rather than by hash.
- **Undo that says it is done when it is not.** `undo` marks the record `undone` (`runs.rs:2032`)
  whether or not a library write was refused; the note removals a few lines above happen anyway.
  Keep the group's library file open with unsaved changes, press Undo on a run of twenty notes,
  and the gate refuses the link removal, the notes are deleted all the same, the pane lists lines
  whose files are gone, and Undo cannot be pressed again ("This run is undone already").
- **Anything that parses a file that anything could have written.** `Record::from_json`,
  `parse_json` and `Reader` in `runs.rs`. The size cap (4 MiB) and `JSON_DEPTH` 32 hold; the
  *values* are not checked. A `notes[]` entry naming a folder, a `ranges` entry that runs past
  the source's length (kept as it stands), a `heading_shift` of 300 (clamped to 8), a `line`
  that is not a link line, a `started` that is not a stamp (the list is sorted by that string),
  a `version` of 10^9 (cast to `u32`), `instruction` holding anything. `parse_json` also accepts
  trailing bytes after the value. Try each field wrong, then `undo`, `finish` and `prune` on the
  result.
- **Anything that decides a path, now that paths decide folders and file names.**
  `folder_name` (the one name rule), `note_folder` (one folder per heading), `Dest::new` and
  `retarget` (the names a plan offers), `link_path` (relative to the library), `written_home`,
  `resolved`, `library_own_file`, `same_note`/`same_path`. Attacks: a heading named `..`, `CON`,
  `50%20off`, or 64 emoji (`folder_name` caps 64 *characters*, which is up to 256 bytes, and the
  note's own name still has to fit beside up to three of them); two sections that sanitise to the
  same folder; a title of 120 bytes (`SPLIT_TITLE_MAX`) under three sections; a group whose
  `home:` is outside the library, such as `D:\notes\work`.
- **The plan screen's "already there".** `plan_ui::fingerprint` (`plan_ui.rs:97`) is FNV-1a over
  the bytes plus the length (D-060 4). A file in the group's folder that collides is drawn
  "already there" and unticked, so the note is silently not written and the row says the opposite
  of what happened. FNV-1a is linear in the last byte, so a collision is arithmetic, not a
  search: take a note's text, keep its length, and choose its last bytes to hit the hash.
- **The one gate every library write goes through.** `App::library_attempt`
  (`bins/mdedit/src/app/library_write.rs:125`) reads, changes, checks and writes atomically, with
  no comparison of the stamp it read — the old two-window race, now on the path that writes dozens
  of links at once. And `plan_blocked` (`bins/mdedit/src/app/plan.rs:103`) checks the group file's
  tab once, when the screen opens, with an exact path compare, and nothing re-checks it when the
  destination changes.
- **The split model's invariant.** `mdedit_core::split`: every line of the document ends in
  exactly one note or in "the rest", and the ranges come from headings. `Plan::check` states it
  and verifies it, and only the tests call it — a release build runs the plan without it, so a
  key sequence that breaks the invariant stays broken and is written to disk.

For the library itself the parser items below are still where to start: a `#tag` becoming a
group's name, an indented code block read as a note, a `home:` line forty lines down.

### The notes library: the folder, the reader and the format

A library is a folder of Markdown files, one file per group; the file's first `#` line is the
group's name, its other headings are sections (they nest), and each list item holding a link or a
path is a note. Notes stay where the owner keeps them; one that is gone is listed as missing.
The folder is `library/` beside `settings.ini`, or the folder the preference names.

Files: `crates/mdshared-app/src/library.rs` (`read`, `library_files`, `parse`, `title_of`,
`first_heading`, `Entry`, `Group`, `Library::changed`), `bins/mdedit/src/app/library.rs`
(`library_dir`, `library_load`, `library_rows`), `bins/mdedit/src/docs.rs` (`hold_library`,
`forget_library`). `docs/ai-library-plan.md` §5 and `docs/archive/library-redesign.md` §9 hold the
decisions.

Try to break:
- text nobody here wrote: `#tag` on the first line becomes the group's name and clears every
  section below (`heading` takes a level-1 line with no space after `#`); `#1 priority` does the
  same;
- an indented code block (four spaces) is not a fence: a `C:\x\y.md` inside one is read as a
  note, and can then be renamed or removed;
- `- https://example.com/a.md`: `looks_like_path` calls it a note, and `resolve` turns its
  slashes into separators and makes a path of it;
- a `home:` line in a paragraph on line 40, nowhere near the heading: the first such line
  anywhere in the file is the group's folder;
- a note genuinely named `50%20off.md`: `unwrap_path` reads `%20` as a space, and the writer has
  no way to escape it;
- more than `MAX_FILES` `.md` files directly in the folder: those that sort last are dropped in
  silence, and the delete guard below reads the same truncated list. The number is 64 in the
  shared crate mdedit pins (`c603b23`); the shared repository has it at 512 with a message, in a
  commit that is **not** pinned here (`ecef629`), so 64 is the code under review.

### The library writers in the shared crate

The writers are small text-in/text-out functions: they take a library file's text, the line a row
was read from, and give the new text, or `None` when the line is not what they expect (the app
turns that into a quiet no-op). The app re-parses before and after every write.

Files: shared `library.rs`: `remove_note`, `rename_note`, `set_note_path`, `rename_section`,
`add_section`, `remove_section`, `set_group_name`, `move_note`, `add_note`, `section_line`, and for
the `home:` line `set_group_home`, `clear_group_home`, `new_group`, `new_group_with_home`,
`link_line`. Inside: `Text` (per-line `Row`, fence state, line ending), `after_notes`,
`heading_line`, `place`, `note_parts`, `written_target`.

Try to break:
- a line number that moved between the read and the write: `remove_note` checks only that the
  line is a note, so a line inserted above it by another program means a different note goes.
  The app's `note_at` narrows this to the same path, not to the same text;
- a heading written twice: `heading_line` takes the first and matches case-insensitively, so a
  rename of the second leaves two headings that read as one section;
- a file with no final newline, or mixed line endings: the writers keep each line's ending
  (`a_crlf_file_stays_crlf`), but the whole file gains a final newline;
- `move_note` to the heading the note is already under: it removes the line and re-places it, so
  a note can jump above the prose that followed it;
- a section removal where the last line of the section is the end of the file: check the blank
  lines it leaves (`remove_section` trims trailing blanks only at the end of the file);
- `add_note` with a title that holds `](`: `link_title` strips the brackets, `written_target`
  wraps a space in `<>`, and nothing escapes a `%20` in the title.

### Phase 1: one file to edit, the Notes button, the face switches (ddb03d4, shared 163a403)

"Edit a library file" now opens one file rather than all of them; the toolbar has a Notes button;
the pane's head has Files / Functions / Notes switches; the face the pane showed comes back at
start (`pane=` in the session). The shared `draw_head` is untouched, so mdview draws as before.

Files: `bins/mdedit/src/app/library.rs` (`library_file_to_edit`, `library_edit`),
`bins/mdedit/src/ui.rs` (`PaneFace`, `pane_face`, `set_pane_face`, `Hit::PaneFace`),
`bins/mdedit/src/panes.rs` (the head, `draw_head_switches`), `bins/mdedit/src/chrome.rs` (the
bar's Files and Library buttons), `bins/mdedit/src/app.rs` (`restore_pane`),
`bins/mdedit/src/session.rs` (`pane`), `crates/mdshared-ui/src/pane.rs` (`Switch`,
`switch_rects`, `switch_labels_fit`, `draw_head_switches`).

Try to break:
- labels at about 211 px and icons alone to `PANE_MIN` 160: `switch_labels_fit` measures the
  labels, so try a display scale and a pane width where a label is a pixel too wide;
- `session.files` now means "the pane was open, whatever the face": a session from 1.2.0 (no
  `pane=`) with the pane open must open Files, and a shut pane must stay shut; `--files` and a
  folder argument must win over the restored face;
- opening a folder while the Notes face shows must go through `set_pane_face(Files)` (the fault
  found on the way in phase 1): check every path that sets a folder;
- a click on a switch while the name box is open: the box is dropped, and the click must not also
  reach the row underneath.

### Phase 2: row menus, rename in place, one gate for every write, the folder as a setting (3453a7a)

Right-click menus on a group, a section, a note and the empty space; a single-line name box drawn
over the row (new group, new section, new note, rename); "Move to…", "New note here…", "Find the
file…", "Go to its line", "Remove the section?"; and every write goes through one function that
refuses, re-reads, compares and writes atomically. `library_dir` is a preference (empty means
`library/` beside `settings.ini`; a relative path is taken from the settings folder).

Files: `bins/mdedit/src/app/library_write.rs` (whole file: `Wrote`, `note_at`, `free_name`,
`file_stem_for`, `library_file_free`, `library_attempt`, `library_write`, `library_act`,
`library_row_act`, `library_rename_*`, `library_new_*`, `library_remove_*`, `library_move`,
`library_set_note_path`), `bins/mdedit/src/overlays.rs` (`library_row_menu`,
`library_empty_menu`), `bins/mdedit/src/app/input.rs` (`right_press`),
`bins/mdedit/src/ui.rs` (`BarField::Library`, `Hit::LibraryField`),
`crates/mdedit-core/src/prefs.rs` (`library_dir`).

Try to break:
- two windows on one library folder: `library_attempt` reads, changes, checks and writes with no
  comparison of the stamp it read to the stamp it writes over, so the other window's save
  between the read and `write_atomic` is lost. This is now on the plan writer's path too;
- a library file open in a tab with no changes: the refusal checks `modified()` only, so the pane
  writes under the tab, and the tab's next save (from its older text) puts it back;
- the checks compare counts (and, in a few, one name or line), so a write that keeps the count
  but puts a line where it does not belong passes: renaming a section to a name another section
  already has, or a move within one file that lands under the wrong duplicate heading. The plan's
  link writes are the exception and compare sets (`check_links_added`);
- the refusal to write over a modified tab compares the paths with `same_path`, which folds case
  and nothing else (`..`, a second separator, a UNC spelling and a junction still differ), while
  `library_can_add` canonicalises. Open the same file through a link or another spelling of the
  drive and the write lands under that tab;
- "Delete this library file…" from the group row and the doc comment above `library_row_menu`.

### The delete guard and the two-file move (8522c82, f00ee60)

Delete (phase 2b) removes only a regular `.md` directly in the library folder, compared by
canonicalised path, and refuses a file the library lists as a note. A move between two library
files (phase 2) writes the target first and the removal second, and says the note is in both
places when the removal fails.

Files: `bins/mdedit/src/app/library_write.rs` (`library_own_file`, `library_lists_as_note`,
`library_ask_delete`, `library_delete_answer`, `library_delete_file`, `library_rename_file`,
`library_move`), `bins/mdedit/src/library_pane.rs` (`listing`, `without_notes`, `link_path`),
`bins/mdedit/src/docs.rs` (`hold_library`).

Try to break:
- the group file that lists a note sorts past the 64-file cap: `library::read` never sees the
  list, so neither `without_notes` nor `library_lists_as_note` finds the note; its file shows as
  an empty group, and deleting that group row deletes the note. This is the plan's fault A
  reopened by fault B;
- a junction or symlink into the library folder: `library_own_file` canonicalises both sides, so
  decide what should happen there, and check a UNC path and a drive letter in another case;
- deleting a library file that is itself listed as a note *and* has notes of its own: it stays a
  group and can be deleted, taking its own list with it. The dialog says the notes are not
  touched, which is true, but the list is gone;
- deleting a library file that is open and clean in a tab: the tab keeps a path that no longer
  exists, and its next save recreates the file;
- renaming the library file: the group's `home:` line keeps the old folder name, so the next "New
  note here…" opens the folder the line names and not a second one — check that it does, now
  that phase 6a writes the line, and that a group without the line behaves as it did;
- a move where the second write finds the line changed: the note is in both, the toast says so;
  check the toast's group names and that a `Wrote::Nothing` is not silent;
- `library_rename_file` checks `to.exists()` and then calls `std::fs::rename`, which on Windows
  replaces a file that appears in between (the same shape as audit 2's C2, still here).

### Phase 2b and 3: the name box, the keyboard, the folder on the row, empty states (8522c82)

The name box drew nothing as it was typed because the pane's frame signature did not include the
box; it now does. The pane also takes the keyboard, shows each note's folder, has empty states
with buttons, and "Show in the notes library" finds the open file in the tree.

Files: `bins/mdedit/src/app.rs` (`region_sig`, `Region::Files`; the box's text, caret, selection,
focus, blink and cursor row), `bins/mdedit/src/app/library.rs` (`library_key`, `library_row`,
`library_focus`, `library_show_file`, `library_tick`), `bins/mdedit/src/library_pane.rs`
(`rows`, `build`, `id_at`/`RowId`, `set_cur`, `follow`, `restore_cur`, `is`, `key_of`, `left`,
`right`, `jump`, `folder_fit`, `elide_left`, `empty_state`, `draw`), `bins/mdedit/src/keys.rs`
(`Key::Menu`), `bins/mdedit/src/macros.rs`.

Try to break:
- the cursor by identity: `RowId::Note` compares the `PathBuf` exactly while the open-note
  highlight uses `same_note` (which folds `.` and `..`); a re-read after another program saves
  the file with a different spelling moves the cursor off the note to its heading;
- two notes with the same path in two groups: `follow` takes the first row that matches under
  that heading, so check which one a command lands on;
- `key_of` joins headings with U+0001: a heading containing that character (a model can write
  one, the keyboard cannot) collides with another heading's key. The plan screen's rows and the
  record's sections take headings from a document too;
- a rename while the library changes underneath: `library_tick` drops the box when the rows move,
  and a key on the same turn may commit first; type a name while another program saves the
  library every second and see which wins;
- Delete on a group does nothing (by design), Delete on a section asks and takes the notes under
  it (`starts_with` counts nested ones, `remove_section` removes the whole subtree);
- Space, Tab, Backspace and Insert are deliberately swallowed in the pane (the catch-all arm of
  `library_key`); check that no shortcut is lost with them.

### A note is not a group, and Esc keeps an AI result (f00ee60)

A note made from the pane goes in a folder of its own named for the group's file (and, since
`487e184`, one folder deeper per heading); a file that only names a note is not shown as a group;
and Escape on an AI review keeps its result, which "AI: Show the last result" brings back.

Files: `bins/mdedit/src/app/library_write.rs` (`library_new_note`, `free_name`, `remove_empty_folders`),
`bins/mdedit/src/library_pane.rs` (`without_notes`, `listing`, `link_path`),
`bins/mdedit/src/docs.rs` (`library_read`, `hold_library`, `ai_kept`), `bins/mdedit/src/ai.rs`
(`Ai::kept`), `bins/mdedit/src/app/ai_events.rs` (`ai_keep_review`, `ai_drop_kept`,
`ai_show_kept`), `bins/mdedit/src/status.rs`, `bins/mdedit/src/commands.rs` (`AiShowResult`).

Tests: `app/library_leak_tests.rs`, `app/ai_kept_tests.rs` (including
`the_command_is_offered_only_while_a_result_is_kept`).

Try to break:
- a new note whose folder already exists (made by hand, or left by a rename of the group's
  heading): `free_name` avoids a name clash in the folder but nothing notices a folder that is
  already there;
- the note is created with `std::fs::write` (`library_write.rs:690`) and then linked: the name is
  found by `free_name`, not created exclusively, so two windows or a program that takes the name
  in between means the note is replaced. If the link write fails the note is removed, but if the
  program is killed in between, the note is left unlisted in the folder, and the cleanup's
  `remove_file` would delete whatever is at that path by then. The plan's writer does this with
  `files::create_new` and does not have this hole; the pane still does;
- the folders made for it are taken away on a refused link (`remove_empty_folders`, deepest
  first): make one of them non-empty in between and check it is kept;
- Escape, then start a rewrite and cancel it: `ai_drop_kept` runs as soon as the engine accepts
  the run, so the kept result is gone though no new result came;
- Escape, close the document, then show the kept result and Apply: `ai_review_sync` and
  `ai_blocked` must block it, leaving Copy only;
- a kept result held while the AI connection is turned off or the key is cleared.

### Phase 6a: a folder per group, set or cleared (487e184)

A note made under a heading is written in the folder its group's `home:` line names, one folder
deeper for each heading of its section; a group without the line gets one, named for the group, in
the same write. "Set the group's folder…" writes the line from a folder picker and "Clear the
group's folder" removes it. No file on disk moves either way. This is the half of phase 6a the
editor needed; the AI half is not built.

Files: `bins/mdedit/src/app/library_write.rs` (`library_new_note`, `library_choose_group_home`,
`library_group_home_picked`, `library_clear_group_home`, `library_new_group`,
`remove_empty_folders`), `bins/mdedit/src/overlays.rs` (the group's menu items),
`bins/mdedit/src/app/commands.rs` (`Picked::GroupHome`), `bins/mdedit/src/commands.rs` (the
toasts), `bins/mdedit/src/ui.rs`, shared `library.rs` (`home_line`, `Parsed::home`, `Group::home`,
`set_group_home`, `clear_group_home`, `new_group_with_home`, `note_folder`, `folder_name`,
`RESERVED_NAMES`), tests `app/library_write_tests.rs`, `app/library_leak_tests.rs`.

Try to break:
- a group whose heading and file name differ: the folder is named from the *heading*
  (`parsed.name.as_deref().unwrap_or(&own)`). Rename the heading after notes exist and the next
  "New note here…" opens a second folder; rename the file and the `home:` line and the folder
  stay as they were. Decide which is right, and make the row say it;
- a `home:` line written absolute, with `..`, as a UNC path, with `%20`, in `<>`, or with a drive
  letter in another case; a second `home:` line in prose (only the first counts, and
  `clear_group_home` removes every one);
- a `home:` line that names a file, or a folder the window cannot write: `plan_blocked` refuses
  the first for the plan screen, but "New note here…" does not — `create_dir_all` fails and the
  note is not made; check nothing is left half-made and the toast says which;
- a folder the run creates and then has to take away: `made` is collected by walking up while
  `exists()` is false, and `remove_empty_folders` runs deepest first — a folder made by someone
  else in the same second must not go;
- clearing the line: the notes stay in the folder they were in, and the next note goes to a
  folder named for the group again, so one group has notes in two places. Is that what the pane
  says? The row shows the folder the *line* names;
- the name rule on the edge cases: `CON`, `con.txt`, `COM7 `, `a.`, `..`, `%20`, 64 emoji (64
  characters can exceed a 255-byte name on Linux and macOS), and two names that come out the same
  (`folder_name` says telling them apart is the app's job; check `free_name` for notes,
  `library_new_group`'s `exists()` for groups, and `retarget` for the plan);
- the `home:` line and an older reader: it is not a heading and not a note, so a library file
  with it lists the same notes; check the reader that resolves a relative `home:` against the
  library folder, and one that holds an absolute path when the library is moved.

### The group's folder on its row, and links that fold their dots (e4c59aa, 932b714, 34b5a2d)

A group's row and its menu say where its new notes go, dimmed at the right like a note's folder
(`Row::home`, `folder_fit`, `elide_left`, `library_row_menu`), and the folder's name comes from the
one rule. A link's `.` and `..` are folded by text before the path is resolved (`lexical`,
`link_path`), so a library that has not been written yet still gets a relative link; and the pane's
tooltips take the keys the platform prints.

Files: `bins/mdedit/src/library_pane.rs` (`Row::home`, `folder_fit`, `elide_left`, `link_path`,
`lexical`), `bins/mdedit/src/overlays.rs` (`library_row_menu`), `app/library_write.rs`
(`library_row_act`), `app.rs`, tests `app/library_pane_tests.rs`, `app/library_write_tests.rs`.

Try to break:
- a home path longer than the row, on another drive, or with a `..` in it; two groups whose
  `home:` names the same folder;
- a pane at `FOLDER_MIN_PANE` (210) and at `PANE_MIN` (160): the folder is dropped first; check
  the title still elides from the left (`elide_left`) and the row still reads;
- `link_path` with a base that does not exist yet, a note that leaves the library and comes back
  (`a/../b.md`), a note inside a junction, and a base and note spelled with different separators;
- the key cue in the tooltip: `Ctrl` on Windows and Linux, `⌘` on a Mac — a test can only assert
  the one it runs on (`the pane tooltips take the keys this platform prints`).

### The split model: a document cut into notes (6f9eb6b)

`crates/mdedit-core/src/split.rs`: a `Plan` is the document's text (a `Buffer` snapshot), the
headings the caller found, and a level. Each note is a range of lines, a title, the section path it
sits under, a tick; a note's text is the source's, byte for byte, except that its headings are
**promoted, never added, never stripped** (a note that starts at a heading has that heading made
level 1 and the rest raised with it; a note with no heading gets `# <title>` and a blank line, and
`title_added` says so). The rest is every non-blank line in no ticked note, as labelled runs, and
is computed from what is ticked. `Plan::check` states the invariant and verifies it.

Files: `crates/mdedit-core/src/split.rs` (whole file: `plan`, `cut`, `compose`, `bytes_of`,
`rest`, `rest_lines`, `new_note_at`, `merge_with_above`, `move_to_section`, `rename`, `warnings`,
`check`, `atx`, `Fence`, `blank_words`), `crates/mdedit-core/src/limits.rs` (`SPLIT_MAX_NOTES`
2,000, `SPLIT_TITLE_MAX` 120 bytes, `SPLIT_SECTION_MAX` 64 bytes, `SPLIT_MAX_SECTIONS` 3).

Try to break:
- every line exactly once: a document that ends without a line break; two headings on
  consecutive lines (a one-line note); a heading as the last line; a document of blank lines
  only; a note that is one line; `merge_with_above` across a section heading (the heading joins
  the note above, as designed — check the preview and the bytes); `new_note_at` at line 0, past
  the end, on a blank line, inside a fence (it moves to the line after the block and the row says
  so);
- two readers of a heading. `headings_of` (`plan_ui.rs:451`) uses the outline, `mdedit_outline`'s
  `symbols`, to say where notes start; `atx()` in `split.rs` says what the title is and how far
  the `#`s move. Find a line where they disagree — `#tag`, `####### x`, three spaces of
  indentation, a tab, a setext heading, a heading inside an HTML comment, front matter — and
  look at the note's title, its `heading_shift` and the `#`s taken away;
- composing: a last line with no break (a break is added); a CRLF document, and one with lone
  `\r` breaks (`blank_words` and `diff::line_count` count the lines, and a disagreement here
  makes every blank-line count wrong); a heading longer than 120 bytes (the title is cut, the
  heading line is not); a heading holding a control character (`clean` turns it into a space in
  the title only);
- the rest's labels: `<!-- lines a-b of NAME -->` is written with the document's own file name,
  unescaped. On Linux and macOS a file may be named `a --> b.md`, which ends the comment early
  and puts a line of Markdown into the note;
- caps: past 2,000 notes the cut stops and the lines stay in the rest (`capped`); a fourth
  section level is dropped from the path silently; a title of 120 bytes can exceed a 255-byte
  file name once it is a note's name in a deep folder;
- the keys that change the plan: `set_level` makes the plan again and throws every rename, tick
  and merge away (by design, but it is one button); `move_to_section` with two names that
  sanitise alike; `sections()` order is first appearance, so the menu's order follows the
  document, not the tree.

### The plan screen: a card that writes nothing (d3b1391)

The card of `docs/ai-library-plan.md` 5.3: the notes a split would make, read and changed, the
coverage strip, the preview of the row's whole text, a level switch, a destination menu, warnings
that never block, and a **Write the notes** button. Escape on a changed plan parks it, and the
Notes pane's foot draws "Plan ready: N notes · Show · Drop". It opens from "Split into notes…" on
the document in front, which must have a file and no unsaved changes, and from `--plan-demo`.

Files: `bins/mdedit/src/plan_ui.rs` (state, events, layout, `widths`/`strip_bands`, `Dest`,
`Target`, `retarget`, `fingerprint`, `fs_reader`), `bins/mdedit/src/plan_draw.rs` (drawing),
`bins/mdedit/src/plan_demo.rs` (the shots' documents), `bins/mdedit/src/app/plan.rs` (opening,
closing, parking, input), tests `app/plan_tests.rs`, `plan_ui.rs`'s own tests, DECISIONS D-060 and
D-062, `tests/shots.sh` (the eight `--plan-demo` shots, `--plan-parked`, both themes).

Try to break:
- the strip: 2,000 one-line notes (every band pinned to 2 px, the plain-proportion fallback), two
  notes with a blank-only gap (the gap joins the note before it), width 1 px, a window resized
  while the thumb is held, `band_row` at the left and right edges, and a tick — a tick must move
  no band but its own (`unticked_or_not_a_note_keeps_its_columns…`);
- `retarget` and its disk cache: the `Reader`'s answers are kept until the destination changes, so
  change the folder on disk while the screen is open; a file deleted after it was found "already
  there" stays unticked; two ticked notes whose names collide after `folder_name`; a note whose
  name is a folder;
- the keys: Escape parks a changed plan and drops an unchanged one; Enter writes; Space ticks; F2
  renames; F7 and Shift+F7 walk the rows with a mark; the Menu key; a key while the name field is
  open goes to the field; a click outside the card; the wheel over the list and over the preview;
- the name field over a row: its text, caret, selection, blink and cursor row must be in the
  overlay signature (`PlanScreen::sig`, folded into `osig` in `app.rs` at lines 1377 to 1379,
  with the caret's blink while a field is open), or it draws nothing as it is typed — the phase
  2b fault, once more, on a new box;
- the row's path and the destination (`target_text`, `dest_text`): they always begin `library\`,
  and for a group whose folder is outside the library they print the whole absolute path after it
  (see F4 below);
- a plan opened from a document that has changed on disk since: nothing compares the file with
  the `Buffer` the plan was made from, so the notes are the old text (see the writer's section);
- the disabled button: a demonstration, a group file open with changes, a folder that is a file,
  a read-only folder — the reason is the tooltip and the foot, and the button must not act.

### The writer, the record and undo (e4099c2, 6cd39b8)

"Write the notes" writes in the order of `docs/ai-library-plan.md` 4.3: the record in
`<library>/.runs`, the folders, the pictures, the notes, the read-back, the library file's links
through the window's own gate, the record closed, and (not built, phase T) the Recycle Bin. Every
file is created by `mdshared_app::files::create_new`, which refuses a taken name and renames a
temporary onto it. Undo asks first, takes the links out, then removes only files whose SHA-256 is
the recorded one, then the group's file when only its heading and `home:` line are left, then the
folders while they are empty; the record is marked `undone` and stays. "AI runs…" lists the
records, with Show the notes, Undo this run and Finish.

Files: `crates/mdedit-core/src/runs.rs` (whole file: `Record`, `Run`, `State`, the JSON reader and
writer, `create`, `close`, `list`, `prune`, `write_at`, `with_links_added`,
`check_links_added`, `with_links_removed`, `with_headings_removed`, `prepare`, `undo`, `finish`,
the `Gate` trait), `bins/mdedit/src/app/plan_write.rs` (`PlanGate`, `plan_write`,
`plan_request`, `open_notes_of`, `open_runs`, `run_undo_ask`, `run_undo`, `run_finish`,
`run_show`, `runs_demo`), `bins/mdedit/src/replace_ui.rs` (`Outcome::run`, `Outcome::undone`),
`bins/mdedit/src/overlays.rs` (`DialogKind::Runs`, `RunUndo`), `bins/mdedit/src/ui.rs`
(`Runs`), `bins/mdedit/src/args.rs` (`--dialog runs|run-undo|run-undo-recycled`,
`--plan-parked`), `bins/mdedit/src/library_pane.rs` (the Plan ready row), `bins/mdedit/src/palette.rs`,
`bins/mdedit/src/commands.rs` (`SplitIntoNotes`, `AiRuns`), tests `app/plan_write_tests.rs`,
`crates/mdedit-core/src/runs.rs`'s 18, `tests/shots.sh` (`runs*`, `plan-ready`), DECISIONS D-061.

Try to break:
- the order, and every prefix of it: stop the process between each step (a record read back by
  `load`, a run left `writing` or `links pending`, then Finish); a run whose record was never
  closed; a run undone twice; a run whose source was moved since;
- create-only: a name taken between `retarget` and step 4, a note that is a folder, a folder that
  is a file, a read-only folder, a full disk, a path past `MAX_PATH`, a name that only differs in
  case from one already there (Windows and a case-sensitive filesystem differ here);
- step 5, the read-back: `verify` compares the file with `from_source`, which is the same text
  the plan composed, so it can only fail on a bad disk — and a note that was **already there**
  with the same bytes (`already`, step 4's `AlreadyExists` arm) is not read back at all;
- the library step: `add_links` refuses at `MAX_ENTRIES` (2,000) before the gate; a group file
  with a duplicate of a planned line; a group file that gained a line since the plan was made; a
  group file open in a tab with changes (the write is refused *after* the notes are written); a
  group file that is read-only between the check and the write;
- the record: a name already in `.runs` (99 tries, then refused), a clock that goes backwards, a
  `.runs` folder holding another program's JSON (`prune` deletes anything that fails to load once
  it is past the twentieth, with `started_secs` 0), a record of 4 MiB, a record with trailing
  garbage;
- undo: a note changed since (kept), a line edited since (kept), a file deleted since (gone), the
  group file with the owner's own prose (kept), a folder the owner filled (kept), the source in
  the Recycle Bin (refused — nothing sets `recycled` before phase T, so only a hand-made record
  reaches it), a run whose group file was renamed since (no group text → every link counts as
  gone while the notes are still removed);
- `finish`: notes written and not linked, one of them edited since (its digest differs, so it is
  neither linked nor reported), the section headings the run added (`sections_added` is not used
  by `finish`, so the links land under headings that may not exist);
- the cost: the write runs on the UI thread, between frames, with no `Job` and no Cancel
  (`plan_write::plan_write` calls `runs::write` and returns). Time a 200-note plan, a group file
  of 2,000 notes, a folder tree on a slow disk, and a plan whose source is on a share; the
  window is not redrawn or answered while it runs.

### One tick box and one switch from the shared crates (5688e6a)

The Replace in files confirmation draws its ticks with `mdshared_ui`'s `check`, and the AI review
its Changes / Preview switch with `segments`; the pinned shared rev moved to `c603b23` for them,
which also brings `mdshared-app::job` (a worker with progress and cancel) and `files::create_new`.
`create_new` is used by the writer; `job` has no caller — `replace_ui::Job` is still the one the
replace panel runs on.

Files: `bins/mdedit/src/ai_ui.rs` (the switch), `bins/mdedit/src/replace_ui.rs` (the ticks),
`bins/mdedit/src/app/widget_pixels_tests.rs` (two tests that redraw the old private drawing beside
the new one and compare pixels), `Cargo.toml`/`Cargo.lock` (the rev), `crates/mdshared-ui`'s
`check` and `segments`.

Try to break:
- the two pinned tests draw one state of each: an unticked and a ticked box, and a switch with
  the second segment. Hover, pressed, disabled, the focus ring, the dark theme and a scale that
  is not 1 are not pinned — compare them by eye against Replace in files and the review;
- a click on the tick at the edge of its box, and a click on the switch's gap;
- a scale of 1.25 or 1.5: the check's geometry is in logical pixels and its stroke rounds at the
  device scale;
- the keyboard: the tick box is drawn by the panel and its hit is the panel's, so check Space and
  Enter still reach it.

### The documents were reorganised (322fa92, ec87d64, a9a694e)

No code. `docs/HANDOFF.md` is gone and `docs/STATE.md` is rewritten rather than appended;
`docs/DECISIONS.md` is the log (D-058 to D-062 are new: the documents, this review, the plan
screen, the write and undo, the pane's status row); `docs/ai-library-plan.md` now lives in this
repository (it was `..\mdedit-mock\docs\ai-library-plan.md`), and `docs/library-redesign.md` is
archived. `docs/measurements/2026-11-21-since-phase-1.md` is the budgets measured again.

Try to break: a pointer in `CLAUDE.md` that leads nowhere (`docs/PLAN.md`'s budgets, `docs/CODE.md`'s
shot script); the plan's §2 list against the code (below); and the measurement document's method —
the ordering of A and B, the seeded session, the junction it uses for the library, and the
sentence that says the first-frame number is the machine.

### The visual-editing scrollbar (31f773a)

The page in visual editing had no scrollbar; the bars drawn inside an open block's box measured
the whole document and could not move. Now the page has a real bar and the box has none, so the
page scrolls and the box rides with it. The plain preview and side-by-side still have no bar.

Files: `bins/mdedit/src/preview_pane.rs` (`Pane::scroll_to`, `extent`, `scroll_due`, and `draw`
with its new `bar` argument), `bins/mdedit/src/app/input.rs` (`scroller`, the drag, the press
that must not open a block), `bins/mdedit/src/view.rs` (`Geom::vbar`, `hbar`, `live::bare()`),
`bins/mdedit/src/app.rs` (`page_bar`, `Region::Preview`), `bins/mdedit/src/app/preview.rs` (the
two tests).

Try to break:
- drag the thumb fast to either end: only the latest target is kept (`scroll_due`) because the
  renderer's `Scroll` is relative, so the final position must match the pointer with no overshoot
  and no drift after letting go;
- click the track above and below the thumb, wheel over the page, then drag: both `hover_due` and
  `scroll_due` wait on the same `waiting` count;
- the owner's open report: the wheel over the open block's box scrolls the page but the box stays
  where it is. `the_open_block_s_box_moves_with_the_page_it_is_on` says it follows in the test;
  drive a real window and find what the test does not model (a second pane, a scaled display, a
  wheel over the box rather than over the page);
- a press on the bar over an open block's box: the `Hit::ScrollBar(Scroller::Page)` check must
  come before `visual_press`, and the About dialog's OK must still win;
- a window resized while the thumb is held, and a document shorter than the pane (no bar, no
  hit).

### Save As offers the language's extension (d1e75f3)

A new document used to be offered `<name>.txt` whatever its language; a note the AI made Markdown
is now offered `.md`, and a language with no extension of its own keeps `txt`.

Files: `bins/mdedit/src/app/commands.rs` (`offered_save_name`, `save_as`),
`bins/mdedit/src/languages.rs` (`extension_of`, `TOKENS`, `PLAIN_EXT`), tests
`a_new_note_made_markdown_is_offered_a_md_name` (in `app/ai_tests.rs`) and
`a_languages_extension_leads_back_to_it_and_plain_text_is_txt`.

Try to break:
- a language whose first matching token reads back as another language (`extension_of` takes the
  first `TOKENS` that leads back to the same name): check every language, and two languages that
  could take the same extension;
- a tab renamed in the bar to `Budget.md` and never saved: `Doc::name()` returns the title and the
  offer becomes `Budget.md.md`;
- a document with a path (unchanged), a document with no language, an unsaved document in a
  folder the window cannot write to, and the AI conversion that sets Markdown after the name was
  chosen.

### `brief_last`: the tab you were last on counts like the rest (4628bf2, shared e7f6e61, f7a60a0)

A second switch under "Don't reopen tabs barely looked at" makes the rule count the tab that was
showing when the window closed, so a file glanced at and left on screen is not reopened. Off by
default; pinned tabs, unsaved text and tabs with no count still come back.

Files: `crates/mdedit-core/src/prefs.rs` (`brief_last`, parse and round trip),
`bins/mdedit/src/app.rs` (`barely_seen`), `bins/mdedit/src/app/modal.rs` (`BRIEF_LAST`, the row
and its help), `bins/mdedit/src/session.rs` (the pref lines), shared
`crates/mdshared-app/src/brief.rs` (`LAST`, `LAST_HELP`, `left_out`).

Try to break:
- the switch alone: with "Don't reopen…" off it must do nothing (test asserts this);
- the showing tab is known twice, by `session.active` (an index) and by `active_tab` (a path):
  after tabs are closed or reordered, check the index still names the tab that was in front, and
  that a tab which is pinned is exempt whichever way it is found;
- a tab with no focus count (`None`) is always kept, even with the switch on;
- a session written with `brief_last=on` and read back (the prefs travel in the session, not only
  in `settings.ini`);
- the new help line fits the settings form at 1280x800 (`every_help_line_fits_in_every_layout`).

### The audit 2 fixes that moved into `mdshared-app` (e412d72, 2c23a71; shared cb0ffd3, now c603b23)

The focus clock and the brief rule, the folder lock, and the patient settings read are now in the
shared crates, used by mdedit and mdview. Compressed in the audit 2 section above for that
reason. The pin has moved on since (`c603b23`, for the library, the widgets, `files::create_new`
and `mdshared-app::job`), so check that the move did not change behaviour, not just that it
compiles — and that the two later pins did not either.

Files: `crates/mdshared-app/src/folder_lock.rs`, `crates/mdshared-app/src/settings.rs`
(`read`, `read_patiently`, `write_atomic`), `crates/mdshared-app/src/brief.rs`,
`crates/mdshared-app/src/files.rs` (`create_new`, `stamp`), `bins/mdedit/src/app.rs` (where the
lock is taken), `bins/mdedit/src/session.rs`.

Try to break:
- two launches on one settings folder (the lock is per folder, and the loser forwards its files
  then exits);
- a lock left by a process that crashed, and a launch beside a 1.2.0 window that holds no lock;
- a settings file that exists but cannot be read (open it exclusively in another program), and a
  file that is being written: `read_patiently`'s 2 s;
- `write_atomic` on a full disk and on a read-only folder: the temporary file must not be left
  behind, and the old file must survive;
- `create_new` on a name that appears between its second check and its rename: the doc comment
  admits the window is not closed, and the writer relies on it.

## Known faults, from the library plan (§2)

`docs/ai-library-plan.md` §2 is a reading pass over this code. Do not spend time finding these
again: confirm the fix, or say what is left.

- **A. A note made from the pane was also a group, and "Delete this library file…" deleted it.**
  Fixed by `f00ee60`: a new note goes in a folder of its own named for the group
  (`library_new_note`), a file that only names a note is not shown as a group (`without_notes`),
  and delete refuses a file the library lists (`library_lists_as_note`). Tests
  `a_note_made_with_new_note_here_does_not_become_a_group`,
  `delete_never_removes_a_file_the_library_lists_as_a_note`.
- **B. The 64-file limit hides real library files.** Not fixed in the code under review:
  `MAX_FILES` is 64 at shared `c603b23`, which is what `Cargo.toml` pins, and `library_files`
  truncates in silence; the delete guard reads the same truncated list. The shared repository has
  it at 512 with a message in `ecef629`, which is not pinned. So: still open here, fixed further
  along the road.
- **C. Links to notes inside the library folder were absolute.** Fixed: `library_pane::link_path`
  writes a relative target for a file inside the folder, and `runs::Request`'s notes use the same
  function. Tests
  `a_note_made_inside_the_library_folder_is_linked_without_a_drive_and_survives_a_move`,
  `a_link_to_a_file_outside_the_library_folder_stays_absolute`.
- **D. Esc threw an AI result away.** Fixed: `ai_keep_review` keeps it and
  "AI: Show the last result" brings it back. Tests in `app/ai_kept_tests.rs`.
- **E. The review cannot be read past the first screen.** Not fixed: `ai_ui::changes` still has
  no scrollbar, no wrapping, and no way from change to change; `docs/STATE.md` lists it as known
  to be wrong.
- **F. The write checks count, they do not compare.** Partly fixed, and this is the part that
  matters most now: the plan's links go through `runs::check_links_added` /
  `check_links_removed`, which compare the exact set of notes, each under its section (D-061),
  and that is the check the twenty-three-link import will use. Every other library write still
  passes a count closure to `library_attempt` (`library_new_note`, `library_new_section`,
  `library_remove_note`, `library_remove_answer`, and the renames), and the gate enforces only
  what the closure checks.
- **G. File names from titles.** Fixed for what the app writes: `folder_name` is the one rule (22
  device names, `%` replaced, no trailing dot or space, 64 characters) and `file_stem_for` uses
  it (shared `074e462`, pinned by `fcf3130`), as do `retarget` and the plan's group file. Tests
  `every_name_that_is_written_goes_through_the_one_rule`,
  `a_note_named_with_a_percent_is_written_under_a_name_the_reader_reads_back`. Not fixed for what
  the app reads: `unwrap_path` still turns `%20` into a space, so a link to a file whose name
  really holds `%20` is wrong; and the 64 is characters, not bytes.
- **H. The engine's version is never read.** Not fixed: `bins/mdedit/src/ai.rs` finds the hello
  line by `line.contains("\"ev\":\"hello\"")`, and `proto::Request` has no
  `deny_unknown_fields`.
- **I. The library is read on the UI thread.** Not fixed, and now measured: a tick is 0.28 ms
  (200 notes) or 2.25 ms (64 files, 1,832 notes) of listing and stats, and a real re-read is
  38.8 ms — ten scroll frames, in the event loop. `docs/STATE.md` and the measurement document
  carry the numbers and the two budgets they break.
- **J.** The plan overturns three earlier decisions (whether a split note starts with
  `# <title>`, where a converted file goes, the record folder's name). Decided: D-036 (promote,
  never add), D-029/D-032 (a folder per group and per section, and nothing outside it), D-043
  (`.runs`). What remains is that phases after 6a's editor half are not built: the Recycle Bin,
  the progress row with Cancel, "Ask the AI…", the import and the rewrite.

## Faults found while reading (this pass)

Not from the plan's list; found by reading `df562e8..a9a694e`. None of them has a test, and
nothing here was run.

- **F1. Undo removes files at paths the record names, with no check that they are the run's.**
  `crates/mdedit-core/src/runs.rs`: `prepare` (line 1836) and `undo` (line 1960) take `notes[].path`,
  `pictures[].path`, `folders[]` and `group.file` from the record, resolve an absolute path as it
  stands (`resolved`, line 266) and remove what the digest matches. The write confines every path
  (`outside`, line 1501; `inside_any`, line 1491); undo does not. A record that anything else wrote
  into `<library>/.runs` chooses which file of the owner's a press of Undo deletes. Fix: refuse a
  record whose paths are outside the library or the group's folder, as the write does, and test it.
- **F2. Undo marks the run undone even when the library write was refused.** Same file, line 2032:
  `marked.state = State::Undone` runs whether or not `report.failures` holds a refusal, and the
  notes were already removed above. The owner is left with a library that lists notes whose files
  are gone, and Undo refuses to run again. Fix: leave the record as it was when any step refused,
  and say which.
- **F3. Undoing a run that added a section heading rewrites a CRLF library file with LF.**
  `runs.rs`, `with_headings_removed` (lines 1678-1687): `out.lines()` then `join("\n")` loses every
  `\r`, and a final `\n` is added. The shared writers keep each line's ending
  (`a_crlf_file_stays_crlf`), and the gate's check compares parsed notes, so nothing catches it.
  Whole-file rewrite of the owner's file for a change of one heading.
- **F4. The plan screen shows every note's path under `library\`.** `bins/mdedit/src/plan_ui.rs`:
  `target_text` (line 890) and `dest_text` (line 907) prefix `library` and fall back to the whole
  path when the note is not under the root, so a group whose `home:` is `D:\notes\work` reads
  `library\D:\notes\work\….md`. Display only, but it is the row the owner reads before a write of
  dozens of files.
- **F5. A run that cannot start closes the plan.** `bins/mdedit/src/app/plan_write.rs`, line 96:
  `self.plan = None` happens before `report.start_error` is looked at, so a start error (the source
  is gone, a folder is outside the library) clears the card and the owner's ticks and renames with
  it; nothing was written. The refusal on `plan_request` puts the screen back; this path does not.

## Open, not fixed

- The owner reports that in visual editing, the wheel over the open block's box scrolls the page
  but the box stays where it is. The test `the_open_block_s_box_moves_with_the_page_it_is_on`
  has the box, its text area and the text view follow the page after small and large scrolls, and
  reading the paint path found nothing. Not reproduced; the real window was not driven. The
  scrollbar work (`31f773a`) changed the paint and hit path around it, so try again there. Find
  the cause.
- `missing_parts` checks that the files exist, not their version.
- The plan's faults B, E, H and I, and the `%20` half of G, are still open (above).
- The delete guard reads the library through the 64-file cap, so a note listed by a file past
  the cap still shows as an empty group and deleting it deletes the note. Should the guard read
  every `.md` in the folder?
- Opening Settings during an AI review discards the result (`docs/STATE.md`, "Known to be
  wrong").
- The two budgets the measurement document finds unmet: private memory 30.4 MiB against 30 with
  a 2,000-note library restored, and CPU at idle while the Notes face shows a large library
  (2.25 ms a tick). The document names the cause of the memory one (the phase-0 unfiltered read
  kept beside the filtered one) and records no fix yet.
- F1 to F5 above.

## How to report

As in audit 2: findings by area, each with a confidence and a failure scenario. Mark what was
verified by running code and what comes from reading only. Probe tests go in a patch beside this
file.

## Unverified

Written from the repository and its documents at `a9a694e`; nothing was built or run for this
document, and mdedit's own tests were not run.

- **Whether 1.3.0 ships as one release.** Still open: there is no `v1.3.0` tag in this checkout,
  and `docs/STATE.md` puts "tag it now that phase 0 is in, or wait" under "Waiting for the owner".
- **The test totals.** Answered as far as a checkout can: `docs/STATE.md` records 592 of the
  app's own unit tests and 1,274 in the workspace at `487e184`; a count of `#[test]` in this
  tree, which is not the same as `cargo test`'s total, gives 596 in `bins/mdedit/src` and 1,300
  in the workspace. `e4099c2` adds 17 in `mdedit_core::runs` and 6 window tests, and `6cd39b8`
  adds one more. No number in the documents is current, and none was produced by running the
  suite.
- **Whether `note_folder` has a caller.** Answered: three. `library_write::library_new_note`
  (the pane's new note, phase 6a), `plan_ui::retarget` (each row's file) and
  `plan_write::plan_request` (the rest's note). It is no longer test-only.
- **Whether mdview reads a `home:` line.** Answered for the shared reader: mdview pins shared
  `cb0ffd3`, and `crates/mdshared-app/src/library.rs` does not exist at that commit — the notes
  library came later — so a `home:` line cannot reach mdview through the shared crate. Whether
  mdview has a library of its own was not checked: there is no checkout of it on this PC.
- **Whether phases 2b and 3 are one change.** Not a question for the reviewer: both are in
  `8522c82`, `docs/STATE.md` lists them together, and the reviewer reads the commit.
- **The release notes' deletes.** Answered: `docs/release-notes/1.3.0.md` now names both — "the
  one thing that does delete is 'Delete this library file…'", and the split bullet says Undo
  removes only the files whose contents are still exactly what it wrote. Read the undo sentence
  again against F2, which is the case where they are not.
- **Whether the AI review panel's missing scrollbar (`ai_ui::changes`) is scheduled.** Still
  open; nothing in the documents says so.
- **The cost of `library_tick` on a large library.** Answered: 0.28 ms and 2.25 ms a tick, 4.1 ms
  and 38.8 ms a re-read, in `docs/measurements/2026-11-21-since-phase-1.md`. What is still
  unmeasured is the row rebuild and repaint that follow a re-read, isolated from the rest of the
  tick.
- **macOS and Linux.** No run of any of this on either: the writer's `create_new` rename, the
  name rule against a 255-byte limit, the folders of phase 6a, the plan screen's fonts and
  widths. No budget has ever been measured on either.
- **The owner has not seen the plan screen or the runs dialogs.** The shots exist
  (`plan-split*`, `plan-split-gaps`, `plan-split-rest`, `plan-split-warn`, `plan-split-again`,
  `plan-split-rename`, `plan-split-many`, `plan-split-small`, `plan-ready`, `runs`,
  `runs-undo`, `runs-undo-recycled`, both themes), and `docs/STATE.md` records no approval of
  them or of the notes library's interface.
- **The budgets.** As `docs/measurements/2026-11-21-since-phase-1.md` measures them, A =
  `d4e3ad6` (phase 1) against B = `d3b1391` on this PC, 8 runs a configuration: the first frame
  is **not verified** (every build on the machine read 410 to 440 ms that evening, the installed
  1.2.1 included; A and B are within each other's ranges); private memory is **not met** with the
  2,000-note library restored — 30.4 MiB against 30, +1.05 MiB since phase 1, paid by the
  phase-0 unfiltered read; CPU at idle is **not met** while the Notes face shows a large library
  — a 2.25 ms check about every 800 ms, not a timer of its own; keystroke (p95 1.48 ms) and
  scroll (p95 1.35 ms) are met. The commits after `d3b1391` (the writer, undo, the measurement
  itself) are not in those numbers, and a fix for the memory is not recorded in the document.
