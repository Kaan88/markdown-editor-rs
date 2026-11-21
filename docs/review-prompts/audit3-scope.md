# Audit 3: what to review (for Fable)

Written 2026-11-21 for mdedit 1.2.0. Audit 2's findings were all fixed for 1.2.0, but nobody has
reviewed the fixes. Review them, plus the changes made after audit 2. Read `docs/HANDOFF.md`
first: "Reviews: who audited what" and "Audit 2 (Fable, 2026-11-21)", which lists each fix and its
test. Audit 2's probe tests are in `docs/review-prompts/audit2-probe-tests.patch`.

The document was written before the notes library existed. Since `v1.2.1` a whole feature has been
merged and nobody has read it: the notes library, its redesign phases, the writers it moved into
the shared crates, and a handful of smaller changes. They are in "Merged since 1.2.1" below, and
they carry the faults that matter most. 1.3.0 has no tag at the time of writing (the version is
1.3.0 in `Cargo.toml`, with `docs/release-notes/1.3.0.md`), so code and notes may still move.

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
migration. Its section in HANDOFF lists each fix with its test.

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
the keyboard.

### Where a fault would be worst

Look here first, and with the will to break it rather than to follow the happy path:

- **Anything that writes or deletes a file of the owner's.** `library_delete_file` (the one
  delete), `library_rename_file` (the one rename of a file), `library_new_note` (creates a note
  and may take it back), `library_attempt` / `library_write` and `library_put` (rewrite a library
  file). The notes themselves are never written by any of these, and that is the property to
  attack.
- **Anything that decides a path.** `library_dir` (preference, or the settings folder),
  `link_path` and `written_target` (relative or absolute, and how it is spelled), `resolve` and
  `unwrap_path` (how a written path becomes a real one), `folder_name` and `file_stem_for` (a
  name, made safe), `library_own_file` (what may be deleted), `same_note` and
  `canonical_or_absolute` (whether two spellings are one file).
- **Anything that parses text a model or another program wrote.** `library::parse` and its
  helpers `heading`, `home_line`, `note_line`, `unwrap_path`, `looks_like_path`;
  `Text::new`/`note_parts` in the writers; `title_of`/`first_heading` (which reads the first
  bytes of a note). A malformed line must be left alone, never half-understood.

### The notes library: the folder, the reader and the format

A library is a folder of Markdown files, one file per group; the file's first `#` line is the
group's name, its other headings are sections (they nest), and each list item holding a link or a
path is a note. Notes stay where the owner keeps them; one that is gone is listed as missing.
The folder is `library/` beside `settings.ini`, or the folder the preference names.

Files: `crates/mdshared-app/src/library.rs` (`read`, `library_files`, `parse`, `title_of`,
`first_heading`, `Entry`, `Group`, `Library::changed`), `bins/mdedit/src/app/library.rs`
(`library_dir`, `library_load`, `library_rows`), `bins/mdedit/src/docs.rs` (`hold_library`,
`forget_library`). `docs/library-redesign.md` §9 holds the decisions.

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
- more than 64 `.md` files directly in the folder: those that sort last are dropped in silence
  (`MAX_FILES`), and the delete guard below reads the same truncated list.

### The library writers in the shared crate

The writers are small text-in/text-out functions: they take a library file's text, the line a row
was read from, and give the new text, or `None` when the line is not what they expect (the app
turns that into a quiet no-op). The app re-parses before and after every write.

Files: shared `library.rs`: `remove_note`, `rename_note`, `set_note_path`, `rename_section`,
`add_section`, `remove_section`, `set_group_name`, `move_note`, `section_line`, and for the
`home:` line `set_group_home`, `clear_group_home`, `new_group`, `new_group_with_home`. Inside:
`Text` (per-line `Row`, fence state, line ending), `after_notes`, `heading_line`, `place`,
`note_parts`.

Try to break:
- a line number that moved between the read and the write: `remove_note` checks only that the
  line is a note, so a line inserted above it by another program means a different note goes.
  The app's `note_at` narrows this to the same path, not to the same text;
- a heading written twice: `heading_line` takes the first and matches case-insensitively, so a
  rename of the second leaves two headings that read as one section;
- a file with no final newline, or mixed line endings: the whole file is rewritten with one
  ending (CRLF if any line had CRLF) and gains a final newline;
- `move_note` to the heading the note is already under: it removes the line and re-places it, so
  a note can jump above the prose that followed it;
- a section removal where the last line of the section is the end of the file: check the blank
  lines it leaves (`remove_section` trims trailing blanks only at the end of the file).

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
refuses, compares and writes atomically. `library_dir` is a preference (empty means `library/`
beside `settings.ini`; a relative path is taken from the settings folder).

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
  between the read and `write_atomic` is lost;
- a library file open in a tab with no changes: the refusal checks `modified()` only, so the pane
  writes under the tab, and the tab's next save (from its older text) puts it back;
- the checks compare counts (and, in a few, one name or line), so a write that keeps the count
  but puts a line where it does not belong passes: renaming a section to a name another section
  already has, or a move within one file that lands under the wrong duplicate heading. The plan's
  F is about the import's many links, and this gate is what it will use;
- the refusal to write over a modified tab compares the paths as text (`library_file_free`), not
  canonicalised as `library_can_add` does, so the same file open through a link or a different
  spelling of the drive letter is not seen, and the write lands under that tab;
- "Delete this library file…" from the group row: the doc comment above `library_row_menu` still
  says nothing in it deletes a file on disk, and the menu has the item.

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
- renaming the library file: the group's note folder keeps the old name, so the next "New note
  here…" opens a second folder beside it (the `home:` line that would anchor the folder is not
  written yet, see below);
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
  one, the keyboard cannot) collides with another heading's key;
- a rename while the library changes underneath: `library_tick` drops the box when the rows move,
  and a key on the same turn may commit first; type a name while another program saves the
  library every second and see which wins;
- Delete on a group does nothing (by design), Delete on a section asks and takes the notes under
  it (`starts_with` counts nested ones, `remove_section` removes the whole subtree);
- Space, Tab, Backspace and Insert are deliberately swallowed in the pane (the catch-all arm of
  `library_key`); check that no shortcut is lost with them.

### A note is not a group, and Esc keeps an AI result (f00ee60)

A note made from the pane now goes in a folder of its own named for the group's file, with a link
relative to the library folder; a file that only names a note is not shown as a group; and Escape
on an AI review keeps its result, which "AI: Show the last result" brings back.

Files: `bins/mdedit/src/app/library_write.rs` (`library_new_note`, `free_name`),
`bins/mdedit/src/library_pane.rs` (`without_notes`, `listing`, `link_path`),
`bins/mdedit/src/docs.rs` (`library_read`, `hold_library`, `ai_kept`), `bins/mdedit/src/ai.rs`
(`Ai::kept`), `bins/mdedit/src/app/ai_events.rs` (`ai_keep_review`, `ai_drop_kept`,
`ai_show_kept`), `bins/mdedit/src/status.rs`, `bins/mdedit/src/commands.rs` (`AiShowResult`).

Tests: `app/library_leak_tests.rs`, `app/ai_kept_tests.rs` (including
`the_command_is_offered_only_while_a_result_is_kept`).

Try to break:
- a new note whose folder already exists (made by hand, or left by a rename of the library file):
  `free_name` avoids a name clash in the folder but nothing notices a folder that is already
  there;
- the note is created with `std::fs::write` and then linked; if the link write fails it is
  removed, but if the program is killed in between, the note is left unlisted in the folder, and
  the cleanup's `remove_file` would delete whatever is at that path by then;
- Escape, then start a rewrite and cancel it: `ai_drop_kept` runs as soon as the engine accepts
  the run, so the kept result is gone though no new result came;
- Escape, close the document, then show the kept result and Apply: `ai_review_sync` and
  `ai_blocked` must block it, leaving Copy only;
- a kept result held while the AI connection is turned off or the key is cleared.

### The `home:` line and the one name rule (shared f75b4c5, 074e462; pinned by 2b407d2, fcf3130)

A group may name the folder its notes belong in with one `home:` line, read like a note's path
(relative to the library folder unless absolute); and one function, `folder_name`, turns a group,
section or note name into a file or folder name (the characters Windows forbids and `%`, no
trailing dot or space, the 22 device names, 64 characters). The app uses the name rule; it does
**not** write a `home:` line yet, and `note_folder` is used only by tests: phase 6a is not built.

Files: shared `library.rs` (`home_line`, `Parsed::home`, `Group::home`, `set_group_home`,
`clear_group_home`, `new_group_with_home`, `note_folder`, `folder_name`, `RESERVED_NAMES`),
`bins/mdedit/src/app/library_write.rs` (`file_stem_for`), `Cargo.toml` (the rev pin).

Try to break:
- an older reader and a file with a `home:` line: it must list the same notes as today (it does:
  `home:` is not a heading and not a note), but a reader that takes `home:` as a note would
  change the collection: check mdview and any second app;
- a `home:` line written in `<>` with `%20`, with a Windows separator, with `..`, and a second
  `home:` line (prose): only the first counts, and `clear_group_home` removes every one;
- the name rule on the edge cases: `CON`, `con.txt`, `COM7 `, `a.`, `..`, `%20`, 64 emoji (64
  characters can exceed a 255-byte name on Linux and macOS), and two names that come out the
  same (`folder_name` says telling them apart is the app's job; check `free_name` for notes and
  `library_new_group`'s `exists()` for groups);
- `note_folder` and `new_group_with_home` have no caller outside the tests: if phase 6a starts
  using them, the empty `home` (`Path::new("")`) and a heading path that sanitises to the same
  folder twice are the cases to try.

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

### The audit 2 fixes that moved into `mdshared-app` (e412d72, 2c23a71; shared cb0ffd3)

The focus clock and the brief rule, the folder lock, and the patient settings read are now in the
shared crates, used by mdedit and mdview. Compressed in the audit 2 section above for that
reason. Check that the move did not change behaviour, not just that it compiles.

Files: `crates/mdshared-app/src/folder_lock.rs`, `crates/mdshared-app/src/settings.rs`
(`read`, `read_patiently`, `write_atomic`), `crates/mdshared-app/src/brief.rs`,
`bins/mdedit/src/app.rs` (where the lock is taken), `bins/mdedit/src/session.rs`.

Try to break:
- two launches on one settings folder (the lock is per folder, and the loser forwards its files
  then exits);
- a lock left by a process that crashed, and a launch beside a 1.2.0 window that holds no lock;
- a settings file that exists but cannot be read (open it exclusively in another program), and a
  file that is being written: `read_patiently`'s 2 s;
- `write_atomic` on a full disk and on a read-only folder: the temporary file must not be left
  behind, and the old file must survive.

## Known faults, from the library plan (§2)

`..\mdedit-mock\docs\ai-library-plan.md` §2 is a reading pass over this code (it is in the
mock-up repository, beside this one). Do not spend time finding these again: confirm the fix, or
say what is left.

- **A. A note made from the pane was also a group, and "Delete this library file…" deleted it.**
  Fixed by `f00ee60`: a new note goes in a folder of its own named for the group's file
  (`library_new_note`), a file that only names a note is not shown as a group
  (`without_notes`), and delete refuses a file the library lists (`library_lists_as_note`).
  Tests `a_note_made_with_new_note_here_does_not_become_a_group`,
  `delete_never_removes_a_file_the_library_lists_as_a_note`.
- **B. The 64-file limit hides real library files.** Not fixed: `MAX_FILES` and the silent
  truncation are as they were. The scenario is rarer now (new notes no longer land in the
  folder), but a hand-made flat `.md` still counts, and the delete guard reads the same
  truncated list.
- **C. Links to notes inside the library folder were absolute.** Fixed: `library_pane::link_path`
  writes a relative target for a file inside the folder. Tests
  `a_note_made_inside_the_library_folder_is_linked_without_a_drive_and_survives_a_move`,
  `a_link_to_a_file_outside_the_library_folder_stays_absolute`.
- **D. Esc threw an AI result away.** Fixed: `ai_keep_review` keeps it and
  "AI: Show the last result" brings it back. Tests in `app/ai_kept_tests.rs`.
- **E. The review cannot be read past the first screen.** Not fixed: `ai_ui::changes` still has
  no scrollbar, no wrapping, and no way from change to change.
- **F. The write checks count, they do not compare.** Partly fixed: every write re-parses, and
  some closures now check more than a count (the group's name, the renamed title, the line's
  path), but no closure compares the exact set of notes, and the gate enforces only what the
  closure checks. A twenty-three-link write with one dropped and another duplicated would pass a
  count check, and that is what the import will ask of it. The shared writer tests
  (`every_writer_reads_back_as_the_library_it_wrote`) check the writers, not the gate; mdedit's
  `a_writer_with_nothing_to_do_writes_nothing_and_says_nothing` checks the no-op.
- **G. File names from titles.** Fixed for what the app writes: `folder_name` is the one rule
  (22 device names, `%` replaced, no trailing dot or space, 64 characters) and `file_stem_for`
  uses it (shared `074e462`, pinned by `fcf3130`). Tests
  `every_name_that_is_written_goes_through_the_one_rule`,
  `a_note_named_with_a_percent_is_written_under_a_name_the_reader_reads_back`. Not fixed for what
  the app reads: `unwrap_path` still turns `%20` into a space, so a link to a file whose name
  really holds `%20` is wrong.
- **H. The engine's version is never read.** Not fixed: `bins/mdedit/src/ai.rs` finds the hello
  line by `line.contains("\"ev\":\"hello\"")`, and `proto::Request` has no
  `deny_unknown_fields`.
- **I. The library is read on the UI thread.** Not fixed; and while the pane shows,
  `library_tick` stats the folder every `RECHECK` (800 ms). Worth measuring at 500 and 2,000
  notes before the import is built on it.
- **J.** The plan overturns three earlier decisions (whether a split note starts with
  `# <title>`, where a converted file goes, the record folder's name). Decisions, not faults;
  phases 6a and later are not built yet.

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

## How to report

As in audit 2: findings by area, each with a confidence and a failure scenario. Mark what was
verified by running code and what comes from reading only. Probe tests go in a patch beside this
file.

## Unverified

Written from the repositories and their documents; nothing was built or run.

- Whether 1.3.0 ships as one release: the version is 1.3.0 and `docs/release-notes/1.3.0.md`
  exists, but there is no `v1.3.0` tag in this checkout.
- The test counts. `docs/HANDOFF.md` says 511 at `8522c82`; `f00ee60` adds tests and no later
  document records a total.
- Whether mdview writes or reads a `home:` line yet (mdedit does not write one).
- Whether phases 2b and 3 are meant to be reviewed as one change: both are in `8522c82`, while
  `docs/library-redesign.md` §10.3 still says "in hand" for both.
- The release notes' "Nothing deletes a file from your disk" (1.3.0, the pane bullet) against the
  group menu's "Delete this library file…", added in phase 2b; whether the notes are due an
  update is the owner's call.
- Whether the AI review panel's missing scrollbar (`ai_ui::changes`) is scheduled; nothing in the
  documents says so.
- The cost of `library_tick` on a large library: no measurement was found, only the 800 ms
  interval.
