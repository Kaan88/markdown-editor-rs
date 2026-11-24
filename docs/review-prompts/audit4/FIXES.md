# Audit 4: what was done with each finding

One row per finding, by report and number. The work is on `fix/audit4` in each repository, and
none of it is merged to a `main`:

- **mdedit** `fix/audit4` holds the three mdedit agents' work: the core's, and the merges of
  `fix/audit4-ai` (`6c1867e`) and `fix/audit4-lib` (`f070775`). Its `mdshared-*` pins were on
  shared's `d1330b0` (`e5873f2`) when the fixes were reviewed, and are on `ef88814` with the
  review's fixes.
- **shared** `fix/audit4`, tip `ef88814` (the review's fixes, on `d1330b0`), on top of shared
  `main` (`0984eed`), which has not moved since the branch left it.
- **mdview** `fix/audit4`, pinned on shared's `ef88814` too (`e553ecf`; `9903fbd` when
  reviewed).

`ef88814` is a development pin: it moves to shared's `main` once shared's branch is merged
there.

The fixes were reviewed by a Fable agent before any merge (its `REVIEW.md`, with its probes,
patches and logs, is in `md-wt\review-a4fix`, outside the repositories). Its findings are fixed
or answered in the section "The review of the fixes" at the end, and every row it disagreed
with below says now what is true.

Each row gives the state, the test, the revert check and what is left.

- **done**: fixed, with the test named.
- **revert-checked**: the test was run with the fix taken out and failed; the row says who ran
  it, and where it failed when that was noted. "(shared)" is the shared agent's last round: 60
  mutations, one fix out at a time, in a copy of the tree. 57 failed as they should, and the
  three that did not are named in their rows (D4's registry read, `paths::comparable` on
  Windows, and F1, which missed its anchor once and failed on the second pass). A test that
  runs only off Windows has not run anywhere; CI runs it.
- **partial**: what is left is said.
- **left**: not fixed, and why.
- **not a bug**: looked at, and the code is right.

Two of mdedit's existing tests met a deliberate change in the shared branch, and were brought to
it, not weakened:

- `plan_write_tests.rs`
  `an_undo_whose_library_file_has_unsaved_changes_keeps_the_notes_and_can_be_pressed_again`: an
  undo the gate refuses now leaves the record `UndoPending` (A F2, F3), not `LinksPending`. The
  test asserts that, and that Finish is no longer offered (`finishable()` false).
- `undo_rules.rs` `a_relative_path_with_dots_in_a_record_reaches_nothing_outside_the_library`:
  its record's group file did not exist, so it now got W2's sentence. The test asserts W2's
  sentence for that case, then writes the group file and asserts the old refusal for `../`.

A third was restaged by the core agent for C12a: see C12a.

A fourth was restaged for RB3, and this file did not say so (the review's T4):
`plan_write_tests.rs`
`a_group_file_that_cannot_be_written_leaves_the_notes_unlinked_and_the_record_says_so` starts
from a group file that exists. A new group's file is made through the same temporary names the
test blocks, so it could no longer be made there; its two assertions about a group the run made
(`is_file`, `group.created`) became their opposites. The case it lost, links pending with a
group file this run made, has its own test now:
`links_pending_with_a_group_file_this_run_made_is_the_runs_to_take_away` (T4 below).

## Release blockers (README order)

| # | Finding | State, test, revert check, left |
|---|---|---|
| RB1 | B1 digest cut inside a character | done: `task_ui.rs` `DIGEST_MAX`, cut on a char boundary. Revert-checked by the AI agent |
| RB2 | B2 / D2 rewrite record over 4 MiB | done. Shared: `runs::rewrite_fits`, `RECORD_FULL`, test `a_rewrite_record_never_grows_past_what_can_be_read_back`, revert-checked (shared). mdedit rolls over to a second record: `task.rs` `task_rewrite_into`, test `a_rewrite_run_past_what_one_record_holds_goes_on_in_a_second_record`, revert-checked by the AI agent. Review of the fixes, S1: the record was measured in the state it had (`writing`), so one that just fit could not be read back once its undo was refused (`undo pending`, 5 bytes longer). `rewrite_fits` now measures it in the longest state word (`State::longest`). Test `a_rewrite_record_that_just_fits_still_reads_back_after_its_undo_is_refused`, revert-checked: fails with the record measured in its own state |
| RB3 | D1 / B6 / A F7 / W3 `<file>.tmp`; audit 3 C9 | done (and see the review's RB3 nit at the end: a name too long to carry `.part`). Shared: the rewrite, its undo, `moves::rewrite_note` and `put_note` write through `files::replace`, and `write_atomic_bytes` removes its temporary on a failure. Its tests are under D1. mdedit: every library write goes through `files::replace` (`library_write.rs` `library_put_text`, read by `library_text`; `library.rs` `library_put` is shared `put_note`; the `plan_write` gate; 4a's join through `library_attempt_at`; "File them" in `task.rs`). The plan's preflight refuses a read-only group file. Tests, each revert-checked: `library_write_tests` `a_library_write_leaves_the_owners_tmp_file_alone_and_no_temporary_of_its_own` (fails with `write_atomic` back in `library_put_text`, and with `library_put`'s old code back; both at :27); `plan_write_tests` `a_group_file_that_cannot_be_written_leaves_the_notes_unlinked_and_the_record_says_so` (fails at :226 with `write_atomic` back); `a_read_only_group_file_is_refused_before_the_notes_are_written` (fails at :524 with the preflight's check out); `import_tests` `a_join_leaves_the_owners_tmp_file_alone_and_keeps_its_byte_order_mark` (fails at :447 with `write_atomic` back); `task_tests` `file_them_leaves_a_tmp_file_of_the_owners_beside_the_group_file_alone` (AI agent). Left: what any rename-over loses (hard links, a Windows file's own access list, alternate streams), documented in shared |
| RB4 | A F1 plan from a stale tab | done: `app/plan.rs`. The first test, `a_tab_that_has_not_read_its_newer_file_opens_no_plan`, fails only with both halves out (the stamp check and the byte comparison), not with the stamp check alone, because its file is UTF-8 and the comparison catches it too (the review's T1); it fails with the byte comparison out. The stamp is the one guard for a tab that is not UTF-8, so each half has its own test now: `a_utf16_tab_that_has_not_read_its_newer_file_opens_no_plan` fails with the stamp check alone out, and `a_tab_told_its_file_changed_opens_no_plan` with `changed_on_disk` out |
| RB5 | D3 `put_note` lossy rewrite | done (shared): `library/put.rs` refuses non-UTF-8, UTF-16 and a file changed on disk. Tests under D3, revert-checked (shared) |

## Report A, mdedit core

| # | Finding | State, test, revert check, left |
|---|---|---|
| A F1 | stale tab plan | see RB4 |
| A F2 | bin refusal at undo orphans the note (= D W4) | done: the state `UndoPending` in shared `runs.rs`, tests `a_note_the_bin_refuses_keeps_the_run_to_be_undone_again`, `an_undo_whose_links_could_not_be_taken_out_keeps_the_notes_and_is_not_undone`, `a_rewrite_undo_that_cannot_write_its_file_is_left_to_be_undone_again`, revert-checked (shared). mdedit: `overlays.rs` "partly undone"; `recycle_tests.rs` test strengthened to check the state and a second Undo; it pins shared's fix |
| A F3 | audit 3 C4: Finish for links pending; toast | done. Shared: `State::finishable()`, and Finish refuses an undo-pending run. mdedit: `overlays.rs` `offers_finish`, test `finish_is_offered_for_a_run_whose_links_are_pending`, revert-checked: fails at `overlays.rs:1933` with Finish for stopped and writing only. The toast "AI runs can finish it or undo it." (`commands.rs` `LINKS_PENDING_NEXT`); `docs/ai-library-plan.md` row 6 corrected. D-157 |
| A F4 | Finish does not record its headings (= D W6) | done (shared): `runs.rs`, test `finish_records_its_headings_and_undo_takes_them_and_the_groups_file_out` (a note under a section), revert-checked (shared). mdedit's `undo_rules.rs` `a_finished_run_under_a_section_is_undone_with_the_heading_finish_wrote` pins it |
| A F5 | audit 3 C6: undo removes a heading with the owner's sub-heading | done (shared): `runs.rs`, test `a_heading_the_owner_wrote_under_is_kept`, revert-checked (shared) |
| A F5b | audit 3 C6: undo bins a note whose tab holds unsaved changes | done: `recycle.rs` `SparingUnsaved`, `plan_write.rs` `run_undo`, test `undo_keeps_a_note_a_tab_holds_with_changes_not_saved`. Revert-checked: fails at `recycle_tests.rs:393` with the fix out |
| A F6 | Move the notes offers another group's library file | done (shared): `moves.rs`; tests `another_groups_library_file_is_not_offered_as_a_note_to_move` (also the row's words, which had a run of spaces in the middle, fixed in `2f409f4`) and `a_request_to_move_another_groups_library_file_is_refused`, revert-checked (shared) |
| A F7 | write_atomic on the owner's files | see RB3 |
| A F8 | rewrite record not confined; question does not name the files | partial. Shared `runs.rs` side done: the question names each file, test `the_rewrite_question_names_its_files`, revert-checked (shared). The review's T6: it pinned that the names appear, not the words; `the_rewrite_question_says_what_it_puts_back_in_these_words` pins the sentence whole (shared). mdedit shows `plan.sentence` (`plan_write.rs`), which names them; `plan_write_tests` `the_undo_question_of_a_rewrite_names_each_file_it_puts_back` pins it. Left: D-162 names D-072 and waits for the owner |
| A F9 | crash during a write leaves notes the record does not own (= D W5) | done (shared): notes whose names are free are marked the run's in the first record, and the record is written again after the folders and pictures (`runs.rs`). Tests `the_first_record_owns_the_notes_whose_names_are_free`, `a_run_that_dies_before_its_notes_owns_the_folders_it_made`, `a_run_that_dies_while_it_writes_its_notes_can_be_undone` (a test-only stand-in for a crash, `runs::DIES_AT`, a constant false in a build), revert-checked (shared). It had a side effect (the review's S2): the undo of a run that died before its notes took the notes a later run of the same plan then wrote. Undo now leaves a note or picture that a later run's record, not undone, names, and the question and the report say so (`UndoPlan.later`, `UndoReport.later`). Tests `undo_of_a_dead_run_leaves_the_notes_a_later_run_wrote`, `undo_of_a_dead_run_leaves_what_a_later_run_found_there_and_listed` (shared), revert-checked four ways: the notes, the pictures, only what the run marked, only a record not undone. mdedit's report lists them (`replace_ui.rs`, test `an_undo_shows_what_a_later_run_wrote_as_kept`, revert-checked); shots `runs-undo-later` and `runs-undone-later` |
| A F10 | tests that do not pin what they name | done: F1, F2 and F14 have tests that pin them; F13's old test did not fail with its fix out, a new one does (see A F13); F4 under a section is pinned in shared (see A F4) |
| A F11a | "their 1 lines" | done (shared): the undo question's sentence in `runs.rs` counts lines in words, test `the_undo_question_counts_the_lines_in_words`, revert-checked (shared). This row named `split/answer.rs` before; that change is N4's |
| A F11b | `&& home != &req.library` waives the group-file check (= D N1) | done (shared): `runs.rs`, test `a_group_file_outside_the_library_folder_is_refused_whatever_the_home`, revert-checked (shared) |
| A F11c | heading removal normalises mixed line endings | not a bug: the library writers join every write with one line ending already |
| A F11d | `follow_move` leaves session and recents on the old path | done: `docs.rs` `follow_move`. Revert-checked: the tests in `moving_tests.rs` fail at :298 and :335 with the fix out |
| A F12 | "Find the file…" off Windows rewrites the wrong note's link | done: `Picked::LibraryFile(Moving, Vec<PathBuf>)` carries the target; `ui.library.finding` removed. Test `find_the_file_writes_the_link_its_picker_was_asked_for` is `cfg(not(windows))`. Left: it and its revert check run on CI only |
| A F13 | held question asked of the new document on a tab switch | done: `Visual::take_pending` in `app/visual.rs`, used by `preview.rs`. Revert-checked: the old unit test `a_held_question_is_not_asked_of_another_document` passed with `preview.rs` reverted; the new test `preview.rs` `a_question_held_for_the_page_is_not_asked_of_the_tab_switched_to` fails with it reverted (`1b6e301`) |
| A F14 | A7's check is lexical (`home: Trip/..`) | done: `library_write::same_folder` compares by shared `paths::comparable`, which folds a `.` or `..` after a folder that is not there, on every system (shared `dbd4faa`). mdedit's own fold (`folded`) went when the pin moved (`e5873f2`). Used in both library_write checks, `importing.rs`, `plan_preflight`, and since the review (its A F14 nit) in `note.rs`'s two checks of a note's group, which compared with `same_path`. The review found those unreachable with `home: Gone/..`; `home: Recipes/..` with `Recipes` there reaches them, and the test `a_group_whose_folder_is_the_library_folder_by_another_spelling_is_refused` fails with `same_path` back. Tests: mdedit's `a_home_line_that_reaches_the_library_folder_by_dot_dot_is_refused` and `a_dot_dot_after_a_folder_that_is_not_there_is_folded`; shared's `paths::a_dot_dot_takes_the_name_before_it_off` (revert-checked (shared): fails on any system with the fold out), `paths::a_dot_dot_after_a_missing_folder_folds_for_a_comparison` and `runs::a_dot_dot_after_a_missing_folder_is_where_it_leads`. Left: all but the shared `a_dot_dot_takes…` pass on Windows with the fold out, because Windows folds `..` already; their revert check is CI's, on Linux and macOS. D-165 |
| A F15 | finished library read rebuilds rows under a held note | done by the LIB agent, with C2: the held note by its `RowId`. Tests `rows_made_again_under_a_held_note_still_move_the_note_held`, `a_click_on_a_note_opens_the_note_pressed_after_the_rows_were_made_again`. Revert-checked by the LIB agent (the rows made again; the click). D-177 |
| A F16 | row menu left open acts on another row | done: `library.rs` `library_build`: a `LibraryRow` menu closes when its row is no longer the row it was opened on, and stays when it is. Test `library_write_tests` `a_row_menu_left_open_closes_when_the_rows_under_it_are_made_again`. Revert-checked: fails at :1728 with the check taken out. D-181. The review found seven direct `ui.library.build` calls left, so "used by every re-read" was not true. They go through `library_build` now (`importing.rs`, `library.rs`, `library_drag.rs`'s demo, `plan_write.rs` twice), and the two that take a name box out first (`library_write.rs` `library_edit_cancel`, `library.rs` after "Add this file") take the menu's row before the box goes, since taking it out moves every row under it. Test `a_row_menu_closes_when_the_name_box_above_it_goes`, revert-checked: fails with the cancel back on the old order |
| A F17 | library file with a BOM (audit 3 C9) | done: `library_text` sets a UTF-8 BOM aside and refuses UTF-16 and non-UTF-8, and `library_put_text` writes the BOM back. Tests `a_library_file_with_a_byte_order_mark_is_written_and_keeps_it`, `a_library_file_that_is_not_utf8_is_left_as_it_is_and_the_toast_says_why`. Revert-checked: they fail at :1604 with the BOM not written back, and at :1633 with the refusal out. D-161 |
| A F18 | "Edit a library file" can write the template over `notes.md` | done: `library.rs` `first_library_file` makes `notes.md` with `files::create_new`. Test `edit_a_library_file_never_writes_the_template_over_a_notes_file`. Revert-checked: fails at :1654 with `std::fs::write` back. D-161 |
| A F19 | "Take this section out" card size; comments that disagree | done: `overlays.rs` `LibraryRemove` grows from its body as `LibraryDelete` does, with the body clipped above the footer. Test `a_long_section_question_grows_its_card_to_its_lines`. Revert-checked: fails at `overlays.rs:2033` with the card fixed at 196. Shots `library-remove-long` and `-dark` were looked at in both themes: the card grows to its four lines, drawn whole, the size and spacing of `library-delete-long`. The comments: `link.rs` agrees with its code and D-105; nothing to change. D-164 |
| A F20 | crash folder for a bare or empty `MDEDIT_SETTINGS` | done: `crash.rs` `crash_dir` keeps no folder for a settings path with no folder in it. Its test was changed on purpose to assert that. Revert-checked: fails at `crash.rs:143` with the old `crash_dir`. D-163 |

## Report B, mdedit AI

All by the AI agent, revert-checked by it: each test fails at its own assertion with its fix
taken out (`77d7385`, `838b748`).

| # | Finding | State, test, revert check, left |
|---|---|---|
| B1 | digest_of panics inside a character | see RB1 |
| B2 | rewrite record past 4 MiB | see RB2 |
| B3 | Discard and Copy result ask for the same file again | done: `task.rs` `task_review_left`, `ai_events.rs`. Test `discard_leaves_the_file_and_the_run_asks_for_the_next`, revert-checked |
| B4 | kept review applied later runs another window's files | done: `FileReview.window`/`sent`, `task_window_of`, `task_rewrite_alone`. Test `a_review_kept_from_a_closed_window_writes_its_file_and_sends_nothing`, revert-checked |
| B5 | task switch takes a click while a request is out | done: `Asking.task`. Test `a_request_out_keeps_its_task_and_its_answer_is_read_as_that_task`, which waits for the answer to be taken, not for the plan. Revert-checked, both halves |
| B6 | failed write leaves an original in the record and a `.tmp` | done: `task_rewrite_finish` removes an empty record. Test `a_rewrite_that_could_not_be_written_leaves_no_record_when_the_run_ends`, revert-checked. The `.tmp` half is RB3 |
| B7 | parked window comes back as File them | done: `task_open`. Test `a_parked_rewrite_window_comes_back_as_rewrite_them`, revert-checked |
| B8 | "Keep the folders" after a sort keeps the AI's sections | done: every sort starts from the folders' plan (`folders_back`, `keep_folders`). Test `keep_the_folders_after_a_sort_gives_the_folders_plan_back`, which compares the plans whole. Revert-checked. D-171. The review's T3: that test presses "Keep the folders" after "Ask again", which has given the folders back already, so `sort_back`'s own `folders_back()` could go and it passed. Test `keep_the_folders_takes_a_sort_still_on_the_card_off_it` presses it on a card that still holds a sort, and fails with that call out. No window path reaches that state today (every way to the scheme or to a failure gives the folders back first), so the test puts the card there by hand |
| B9 | requests-at-once test passes with its cap deleted | done: the test `requests_go_out_three_at_once_and_one_for_a_local_model` has more batches than each cap (121 files with the setting at 2; 41 files for a local model) and asserts the next batch waits. Revert-checked: it fails with the cap deleted |
| B10 | kept file review cannot be reached from the waiting window | done: Run shows the kept review. Test `run_shows_the_kept_review_and_run_again_without_its_window_keeps_it`, revert-checked, both halves |
| B11 | split answer can land on the next file's plan | done. AI half: an answer is read only on the plan screen with the same name and outline, tests `a_split_out_when_a_files_plan_is_written_never_reaches_the_next_files_plan`, `a_split_answer_is_read_only_on_the_plan_it_was_asked_for`, revert-checked by the AI agent; D-172. Core half: `plan_write` cancels the plan's split that is out, test `writing_the_plan_stops_its_split_that_is_still_out`, revert-checked: fails at :1203 with the cancel taken out. Found at the merge: the core's cancel left the status line saying the split was out, and the AI agent's test failed on it (`task_tests.rs:1034`). `plan_write` now clears the status with the cancel, as `plan_close` does, and the core's test asks it too. Revert-checked: with the status line left, both tests fail (`task_tests.rs:1034`, `plan_write_tests.rs:1207`). The review's T2: on the merged tree the core's cancel stops the split before its answer can come, so the AI agent's two tests passed with the guard `split_asked_here` made always true. The guard stays (an answer must never cut one document by another's lines, whatever path a later change opens), and it has a test of its own: `a_split_asked_for_another_document_gives_this_plan_nothing` answers a split asked for one document while another's plan is open, and fails with the guard made always true |
| B12 | file edited after it was added is sent as its old text | done: the file is probed again in `task_rewrite_begin`. Test `a_file_edited_after_it_was_added_is_sent_as_it_is_now`, revert-checked |
| B13 | 20-file limit counts unticked files | done: `task_ui.rs` `recount_limit`. Test `the_twenty_a_rewrite_takes_are_counted_among_the_ticked_files`, revert-checked |
| B14 | Rewrite them runs with no instruction | done: `REWRITE_NEEDS_INSTRUCTION`. Test `rewrite_them_with_no_instruction_sends_nothing_and_says_why`, revert-checked |
| B n1 | `eq_ignore_ascii_case` vs `to_lowercase` for section names | done: one rule, `eq_ignore_ascii_case`, in `Scheme::problem` as in `read_scheme`, `settle` and the library. Test `the_reader_and_the_check_agree_on_which_names_are_one_heading` (mdedit-core), revert-checked. D-173 |
| B n2 | hunks subset deletion at end leaves a final break | done: `hunks.rs` strips the break a final deletion left. Test `a_deletion_at_the_end_of_texts_with_no_final_break_leaves_none` (mdedit-core), revert-checked |
| B n3 | `<file>.tmp` | see RB3 |
| B n4 | "Requests at once" kept per connection, not per preset | done: a change between a local and a hosted preset writes the new kind's default (`modal.rs`); between two of a kind the owner's number stays. Test `a_number_set_for_a_hosted_service_does_not_follow_to_a_local_model`, revert-checked. D-174 |

Also fixed by the AI agent, found in the shots:

- `task-files-plan` showed the file the model did not place ticked. The demo now leaves it
  unticked, as the answer path does (`TaskScreen::leave_out_unplaced`). Test
  `the_plan_shot_leaves_the_file_not_placed_unticked_as_an_answer_does`.
- A sort scheme that could not be read said "Nothing was written." twice. The unreadable-scheme
  test checks it is said once.

## Report C, library writes

By the LIB agent unless the row says the core. Its revert checks ran in two rounds, each fix
put back in turn with its test run (`docs/review-prompts/audit4/lib-revert2.sh` holds the last
round's checks); every check failed.

| # | Finding | State, test, revert check, left |
|---|---|---|
| C1 | DD drop outside the pane still moves | done: `row_at`, `drop_row` and `drop_place` take x, and the edge scroll needs the pointer over the pane. Test `a_note_let_go_over_the_editor_at_a_rows_height_is_not_moved`. Revert-checked twice: with x ignored for the row, and for the scroll. D-177 |
| C2 | DD holds the note by row index | done: the held note's `RowId` in `LibraryPane` (`hold`, `held_at`, found again in `build`); while a note is held and moving, Esc puts it back and every other key is swallowed. Tests `a_key_while_a_note_is_held_reaches_nothing_and_the_note_held_is_the_one_moved`, `rows_made_again_under_a_held_note_still_move_the_note_held`, `a_click_on_a_note_opens_the_note_pressed_after_the_rows_were_made_again`. Revert-checked: the keys let through, the rows made again, the click. D-177 |
| C3 | phase 5 failed write drops the card and the answer | done: the card and the answer are kept with the reason; the record, its `.done` log and the empty folders of a run that made no note are taken away; a name taken while the answer was out gives the next free name. Tests `a_write_that_makes_no_note_keeps_the_card_and_the_answer_and_leaves_no_record`, `a_run_that_made_no_note_takes_its_record_and_its_empty_folders_away`, `a_name_taken_while_the_answer_was_out_gives_the_note_the_next_free_name`. Revert-checked: the card, the discard, the record, the name. Left: shared `runs::write_of` still marks Done a run that wrote nothing, which mdedit works around (the owner's, D-180). D-175, D-176, D-189 |
| C4 | 4a joins a same-named group with another `home:` | done: a group of the folder's name whose `home:` is neither the folder picked nor holds it refuses the plan, naming the group and its folder; the card's group line names the group's own folder; the `home:` is read again at Add. A group with no folder of its own is joined, and the card says "with no folder of its own". Tests `a_group_of_that_name_whose_folder_is_another_refuses_the_folder_and_names_both`, `a_group_that_keeps_its_notes_in_another_folder_refuses_the_plan` (mdedit-core), `a_group_whose_folder_changed_since_the_card_opened_is_refused_at_the_write`, `a_group_with_no_folder_of_its_own_is_joined_and_the_card_says_so`. Revert-checked: the plan, the core's plan, the write, the line. Left: the owner may have such a group joined instead (D-183, which replaces D-178's interim) |
| C5 | 4a offers the library's own files | done: the folder that holds the library leaves the library folder out of the walk, and the card says so. Tests `the_library_folder_inside_the_folder_walked_is_left_out` (mdedit-core), `the_folder_that_holds_the_library_does_not_offer_the_librarys_files`. Revert-checked, both. D-184 |
| C6 | 4a `%20` refuses the whole folder, naming no file | done: a file whose line would not read back is left out and named on the card; the rest of the folder goes in. Tests `a_file_whose_line_would_not_read_back_is_left_out_and_named` (mdedit-core), `a_file_whose_name_the_library_cannot_hold_is_left_out_and_named_and_the_rest_go_in`. Revert-checked, both. Left: the shared cause, `%20` read as a space, is the owner's (D-180). D-185 |
| C7 | 4a bare-CR title of many lines | done: a title is one line (a bare CR ends one), with no control character, at most 200 characters, with the file's name if nothing is left; the preview splits on a bare CR too. Test `a_title_is_one_line_with_no_control_character_and_a_length_it_can_be_written_at` (mdedit-core). Revert-checked three times: the lines, the cleaning, the preview |
| C8 | 4a walk cannot be stopped, no folder bound, card opens over others | done: the walk stops at 50,000 entries read and the plan is refused with the reason; the command stops a read in progress; a read that ends while another card is open waits. Tests `a_walk_stops_at_its_bounds_and_its_plan_is_refused_with_the_reason` (mdedit-core), `the_command_stops_a_folder_being_read`, `a_walk_that_ends_while_another_card_is_open_waits_for_it_to_close`. Revert-checked: the bound, the plan's reason, the stop, the wait. D-186 |
| C9 | 4a new group made in two writes | done: a new group's file is made whole by one `files::create_new` of the dry run's text; `remove_own_file` and the second write are gone. Test `a_new_groups_file_is_the_dry_runs_text`. No revert check: it is a pin, since one write and two cannot be told apart without a crash between them. D-187 |
| C10.1 | DD test lets go below the rows | done: C1's test lets go at a row's y outside the pane's x, as the probe did. Revert-checked with C1 |
| C10.2 | `remove_own_file` reached by no test | done: `remove_own_file` is gone (C9), so there is nothing left to test |
| C10.3 | no test of the dry run before the group file is made | done: test `a_plan_whose_links_would_not_read_back_makes_no_group_file`. Revert-checked: fails with the dry run's check out |
| C10.4 | no test with a link to a folder | done: tests `a_junction_to_a_folder_is_not_followed` (Windows) and `a_symlink_to_a_folder_is_not_followed` (`cfg(unix)`), both mdedit-core. The junction's is revert-checked. Left: the symlink test runs on CI only |
| C10.5 | phase 5: no test of a write that fails after the record; undo checked by name only | done: C3's tests are the failed write. `undo_from_ai_runs_takes_the_note_and_its_line_out_again` now compares the group's library file byte for byte (`dabcb51`); it pins shared's C11 fix, which is revert-checked there |
| C11 | phase 5 undo leaves a blank line | done (shared), after the review. The first fix took out a blank line after the link, which was right for one shape of file only (the review's S3): an owner's trailing blank line was eaten, a link above a heading left its blank, and a file with no final newline gained one. A run's record now keeps what its links did to each library file (`Record.added`: the text before and the digest after), and while the file still holds that text, undo gives the text before back byte for byte (`with_removals_added`, the gate's `remove_links_added`). A file changed since is undone line by line, as before. Tests (shared): `undo_gives_the_library_file_back_byte_for_byte_whatever_its_shape` (eleven shapes, each with three sets of notes, the review's three among them), `undo_takes_the_lines_out_as_before_when_the_file_changed_since_the_add`, `undo_after_finish_gives_the_library_file_back_byte_for_byte`, `a_library_file_too_big_for_the_record_is_not_kept_in_it`, `what_an_add_did_is_kept_only_when_the_file_holds_exactly_it`; nine mutations, each failing its test. mdedit's gate opts in: `plan_write_tests` `undo_gives_the_owners_group_file_back_byte_for_byte` (a BOM, CRLF and a trailing blank), fails with the old `with_removals`. A gate that does not opt in (mdview has none) keeps the old behaviour |
| C12a | a move across files writes before it looks | done by the core: a move across files looks at the note's line before the new place is written (`library_write.rs` `library_move`). Test `a_move_across_files_by_a_stale_line_writes_neither_file`. Revert-checked: fails at :1750 with the look taken out. The existing test `when_the_removal_fails_the_note_is_in_the_new_place_and_the_toast_says_so` staged its half-move on a wrong line, which the look now refuses. It was restaged (`31371be`) with the old file made read-only after the pane read it, so the look passes and the removal is refused. Its assertions are all kept, and it also asks that the toast names the read-only file. It fails at :742 with the half toast taken out. D-182 |
| C12b | a drag across files rewrites the line | left: keeping the line's own text across files needs a shared writer that inserts a given line (rule 8). D-182 |
| C12c | the source read twice | done: one capped read, whose text, line count and digest are of the same bytes. Test `the_cards_text_and_digest_are_of_the_same_bytes`. No revert check: the bug is a race between two reads |
| C12d | a bare-CR source gives a note with bare CRs | done: the note is written with line feeds. Test `a_source_with_bare_carriage_returns_gives_a_note_with_line_feeds`. Revert-checked: the line feeds, and the line count |
| C12e | `note_write_refused` disables the button for good | done: every refusal at the write but "the source has changed" is a failure that can pass (`note_write_failed`): the card stays with the answer, the reason is in the footer, the button stays on. Test `a_refusal_at_the_write_that_can_pass_leaves_the_button_on`. Revert-checked. Shots `note-failed` and `-dark`. D-189 |
| C12f | section rows that differ in case (Linux, macOS) | done: `rows_of` with `eq_ignore_ascii_case`. Test `sections_that_differ_only_in_case_are_one_row` (mdedit-core). Revert-checked |
| C12g | `already` by spelling only | done: a note the group lists through `..` is in the group already (`tree::same_note`). Test `a_file_the_group_lists_through_dot_dot_is_in_it_already` (mdedit-core). Revert-checked |
| C12h | no depth bound on the walk | done: a folder 64 deep stops the walk, refused with the reason. In `a_walk_stops_at_its_bounds_and_its_plan_is_refused_with_the_reason`. Revert-checked. D-186 |

Also done by the LIB agent:

- The import card's footer draws a refusal from the write, with Add left on (D-188). Test
  `a_refused_add_is_said_in_the_footer_and_leaves_the_button_on`, revert-checked.
- While the files are being sent, the scheme's preview says so and that Stop brings the scheme
  back (D-190). Test `the_schemes_preview_says_what_can_be_done_while_the_files_are_sent`,
  revert-checked.
- The `folder-ai-failed` demo says "Nothing was written." once. Test
  `the_failed_sorts_demo_says_nothing_was_written_once`, revert-checked.
- Shots, both themes: `import-folder-skipped`, `-elsewhere`, `-stopped`, `-refused`,
  `note-failed`.

## Report D, shared

All in `markdown-shared-rs` `fix/audit4`, by the shared agent.

| # | Finding | State, test, revert check, left |
|---|---|---|
| D1 | `write_atomic_bytes` takes `<name>.tmp` | done. Tests `files::replace_leaves_the_owners_tmp_and_part_files_alone`, `replace_refuses_a_read_only_file_and_leaves_nothing_behind`, `replace_refuses_a_folder`, `settings::a_failed_save_leaves_no_temporary`, `runs::a_rewrite_that_cannot_write_its_file_leaves_no_trace`, `runs::undo_of_a_rewrite_leaves_the_owners_tmp_file_alone`, `moves::rewriting_a_moved_notes_links_leaves_the_owners_tmp_file_alone`, `put::a_bom_is_kept_and_the_owners_tmp_file_is_left_alone`. Revert-checked (shared). mdedit's side: RB3. The review's T6: `files::replace` through a link that leads nowhere returns NotFound and writes nothing, now pinned by `replace_through_a_dangling_link_writes_nothing` (Unix, so CI's; it pins what the code did, so it has no revert check) |
| D2 | record over 4 MiB | see RB2 |
| D3 | `put_note` lossy | done. Tests `a_library_file_that_is_not_utf8_is_refused_and_left_byte_for_byte`, `a_utf16_library_file_is_refused`, `a_bom_is_kept_and_the_owners_tmp_file_is_left_alone`, `a_library_file_changed_while_the_note_is_added_is_not_written_over`. Revert-checked (shared). The review's RB5 nit: a file that begins `EF BB BF FF FE` passed the UTF-16 test, which looked at the raw bytes. `files::utf16_marked` looks past a UTF-8 mark, and `put_note` and mdedit's `library_text` both use it. Tests `a_utf16_mark_is_found_after_a_utf8_one_too`, `a_utf16_library_file_behind_a_utf8_mark_is_refused` (shared), `the_writers_refuse_a_utf16_file_behind_a_utf8_mark` (mdedit), each revert-checked. Left: the stamp is the time and the size, so a write of the same size inside the clock's step is not seen; it is the stamp every caller uses |
| D4 | Windows bin capacity | done at the decision: `platform/recycle.rs` reads `MaxCapacity` and refuses a larger file. Test `a_file_larger_than_the_bin_is_refused` (needs `--features recycle`). Left: the registry read that feeds it is reached by no test, so its revert check passes; no real bin quota was tried. Also left (the review): with no `MaxCapacity` value, Windows' default, there is no check. The default is a share of the drive that Windows does not state as a fixed number, so a check would be a guess; the files that go to the bin are a split's source and its notes, text files far below any drive's share, and the bin's own answer is still read after |
| W1 | file rewritten twice in one record half undone | done: `runs.rs`. Tests `a_file_rewritten_twice_in_one_record_is_undone_to_its_first_text`, `undo_of_a_file_rewritten_twice_stops_at_an_edit_of_the_owners`. Revert-checked (shared) |
| W2 | group file never made, home outside: stuck, wrong sentence | done: a plain sentence, `runs.rs`. Test `a_run_whose_group_file_was_never_made_says_so`. Revert-checked (shared) |
| W3 | rename-over drops mode; `.tmp` left on failed rename | done: `files::replace` keeps the mode and writes through a link, and removes its temporary when the rename fails. Tests `replace_whose_rename_fails_leaves_no_temporary` (Windows), revert-checked (shared); `replace_keeps_the_files_mode` and `replace_writes_through_a_link` (Unix). Left: the two Unix tests run on CI only. See RB3 for what any rename-over still loses |
| W4 | = A F2 | see A F2 |
| W5 | = A F9 | see A F9 |
| W6 | = A F4 | see A F4 |
| W7 | html_md escaping across tokens | done: `html_md.rs` holds a trailing `<` or `&`. Test `escaping_looks_past_the_end_of_a_text_token`. Revert-checked (shared). FEATURES.md says it (`d1330b0`) |
| W8 | html_md image alt ending in a backslash | done. Test `an_image_alt_that_ends_in_a_backslash_keeps_its_image`. Revert-checked (shared) |
| W9 | split compose promotes any ATX line | done: only the caller's headings are raised. Test `only_the_callers_headings_are_raised`. Revert-checked (shared) |
| W10 | plan card rename box takes clicks meant for Tick all | done: `plan/draw.rs` `clipped`. Test `a_rename_field_half_out_of_the_list_takes_no_click_meant_for_tick_all`. Revert-checked (shared) |
| W11 | Linux trashinfo has no DeletionDate | left: it needs the `jiff` feature, a new dependency (rule 7); the owner's call |
| W12 | macOS bin through Finder | left: it cannot be checked here, and the other way loses Put Back |
| W13 | rule 8: `draw_dialog_button_disabled` changed in place | left as changed: both released apps already draw it this way, so changing it back would change what they draw. The plan card now calls the shared function, with the same pixels. No test; no shot taken in shared, and mdedit's and mdview's shots are unchanged by it |
| T1 | no tests for D1 to D3 | done: the tests under D1 and D3 |
| T2 | html_md escaping tests single tokens only | done: the W7, W8, N3 and N4 tests |
| T3 | split's `headings_of` copies the module's rules | done: `only_the_callers_headings_are_raised` passes headings that disagree with the module's own reading. Revert-checked (shared), with W9 |
| N1 | = A F11b | see A F11b |
| N2 | `strip_verbatim` keeps `\\?\` past 260 | done: `paths::comparable`, used by `runs::inside`. Test `a_long_path_inside_the_library_is_inside_it` (Windows). Revert-checked (shared) |
| N3 | `vbscript:` and `data:` links kept | done: they keep their text only. Test `script_and_data_links_keep_their_text_only`. Revert-checked (shared). FEATURES.md says it (`d1330b0`) |
| N4 | html_md and split smaller nits | done, every part: `<?` escaped and no marks inside code (`a_processing_instruction_is_escaped_and_code_holds_no_marks`); the dropped note's reason (`blank_lines_at_the_ends_are_trimmed`); the rest's label is one comment, and `line_count` of none (`the_rest_s_label_is_one_comment_whatever_the_name`); a section the owner names with only dots is dropped, as the answer reader already did (`section_names_are_one_line_and_cut_to_the_folder_length`, `00bcf4c`). Revert-checked (shared) |
| N5 | plan card rename scrolled out of view | done. Test `the_wheel_keeps_the_name_being_typed_in_view`. Revert-checked (shared) |
| N6 | plan card drops typed text with no key name | done. Test `a_key_with_no_name_types_its_text_into_the_name`. Revert-checked (shared) |
| N7 | `sister::Request` gained `closed` | not a bug: both apps build the struct with `..` |
| N8 | `rewrite_into` leaves the pushed original in memory | done: popped on a failure. Tests `a_rewrite_that_cannot_write_its_file_leaves_no_trace`, `a_rewrite_whose_record_cannot_be_written_writes_nothing`. Revert-checked (shared) |

Shared's checks at `d1330b0`, on Windows: fmt and clippy clean (also `mdshared-ui` with
`--features recycle`); `cargo test --release --workspace` 742 passed, 0 failed, 8 ignored;
`-p mdshared-ui --features recycle recycle` 11 passed, 1 ignored. Shots: none in shared. Nothing
ran on Linux or macOS.

## Report E, mdview

All in `markdown-viewer-rs` `fix/audit4`, by the mdview agent. Each fix was switched back to
the old code in turn, and its tests failed; its `docs/HANDOFF.md` has the details.

| # | Finding | State, test, revert check, left |
|---|---|---|
| E1 | every loop pass resolves the editor's tabs on disk | done: the paths are resolved once, when they arrive (`EditorTab::file`, `placed_for`), and compared by `Workspace::shows_resolved`. Test `looking_for_the_editors_tabs_on_each_turn_asks_the_disk_nothing`. Revert-checked |
| E2 | the zoom view opens at `2 x scale` the page's size | done: the view is sized in device px per raster px, and follows a change of scale. Tests `a_diagram_opens_at_twice_the_pages_size_at_every_device_scale`, `the_wheel_stops_at_the_same_sizes_against_the_page_at_every_scale`, `the_zoom_view_opens_at_twice_the_pages_size_at_every_device_scale`, revert-checked; `zooming_keeps_the_point_under_the_pointer_at_scale_two` passes either way, since its geometry holds in both units. Shots `zoom-diagram-2x-light` and `-dark`, looked at |
| E3 | the pan test passes with the pan removed | done: the drag is measured from where the wheel left it (`a_diagram_opens_at_reading_size_and_can_be_panned_zoomed_and_closed`). Revert-checked: fails when a drag does nothing |
| E4 | palette headers wrong from 240 library files up | done: files 1 to 238 have their own header, the rest go under "More library files" with the file named in each note's hint, and `G_PLAIN` is 255. The collisions began at the 85th file. Test `every_note_is_under_its_own_files_header_or_one_that_names_its_file` (512 files). Revert-checked |
| E5 | the palette resolves every note on disk when its list is built | partial: done for typing; notes are resolved once per read of the library (`Notes::resolve_notes`). Test `the_palette_asks_the_disk_nothing_about_the_notes_as_the_reader_types`. Revert-checked. Left (the review): the first Ctrl+P after each read still resolves every note on the UI thread, 1.6 ms for 400 notes. Moving it to the reading thread needs a wake of its own; at once per read it is not worth it |
| E6 | Add to the library does not know about mdedit's unsaved changes | partial. Shared `put_note` refuses a file changed on disk (`PutError::Changed`, see D3); mdview says the refusals in its own words, naming the file (`notes::not_added`). Tests `a_refused_write_says_why_and_names_the_library_file`, `a_library_file_that_is_not_utf8_is_left_as_it_is_and_the_reader_is_told_why`. Revert-checked. Left: having mdedit write the note over the sister channel, so only mdedit writes the library, is a new feature in both apps |
| E7 | a task tick can be answered against the wrong text | partial: within one batch, a tick not yet answered when a new text arrives is dropped. Test `a_tick_followed_by_a_new_text_before_its_answer_is_dropped`. Revert-checked. Left: across batches it needs a count of texts in `Update::task`, a protocol change in both apps. Also left (the review): when the editor refuses a tick it sends nothing, so `keep_top` stays set and the next text keeps the page still once rather than following the caret; the protocol has no answer that says a tick was refused, the same change |
| E8 | the zoom view stays up over another document | done: the view remembers its tab, the tab's revision and the theme (`zoom::Origin`), and closes when one changes. Test `the_zoom_view_closes_when_the_page_under_it_changes`. Revert-checked, each of the three separately |
| E9 | zoom view, small things | done: a fit can go below `MIN_ZOOM`, and a pinch over the view zooms the diagram. Tests `a_diagram_wider_than_ten_windows_still_fits_whole`, `a_pinch_over_the_zoom_view_zooms_the_diagram_not_the_page`. Revert-checked. Not a bug: the view's close button over the window's own, as every dialog's. Left: pinch and double-tap fire on macOS only, covered by headless tests only |
| E10 | preview server and wire, robustness | done: a `View` is drawn at most 8192 px a side (`wire::MAX_SIDE`), and `Fields::count` holds a count to the bytes left. Tests `a_view_larger_than_a_frame_can_carry_is_drawn_at_the_largest_one`, `a_count_larger_than_what_follows_is_refused_before_anything_is_set_aside`. Revert-checked |
| E11 | `closed` can be missed or late | partial. E11a done: a file the editor asked for that cannot be opened is told `closed` at once. Test `a_file_the_editor_asked_for_that_cannot_be_opened_is_told_closed`. Revert-checked. Left: E11b, a `closed` that arrives late after a fast second ask, needs a sequence number in both apps |
| E12 | the notes face touches the disk; `UseDays::start` overflow | done. mdview: the library is looked for on a helper thread once the window runs (`Wake::Library`), and the empty face uses the state of the last read. Tests `a_window_looks_for_the_library_after_it_is_made_not_before`, `the_empty_face_says_what_the_folder_was_at_the_last_read`, revert-checked. The review's W2 and T5: the first test called `find_library` itself, so the look in `run()` could be deleted with every test passing, and it set `MDEDIT_SETTINGS` for the whole process. `run()` now makes one call, `loop_ready`, which gives the window its proxy and starts the look; the test drives it with a test proxy (`LoopProxy::Test`) and reads the helper thread's answer. The settings file is decided when the window is made and kept on it (`App::library_settings`); the test's window has its own, and no test sets the variable. Four mutations, each failing it (`bc33a45`). Left: `run()` itself is reached by no test, and no window was launched by hand; its look cannot be lost alone, as the same line gives the window its proxy. Shared: `stale.rs` `saturating_add`, test `a_day_of_use_is_a_start_on_a_new_date`, revert-checked (shared) |

## Checks at the merge

Run on Windows, on mdedit `fix/audit4` with the three branches merged, the pin at `d1330b0` and
B11's status fix:

- fmt and clippy on the whole workspace: clean.
- The whole workspace's tests: 1571 passed, 0 failed, 14 ignored (the timing tests). Before the
  status fix, 1570 passed and the AI agent's B11 test failed (see B11).
- The shots: 396 taken, `shots-check.sh` 211 wanted, 0 bad. Beside `main`'s 384 (`402d3da`), 369
  are byte for byte the same, 12 are new and 15 changed:
  - **New, each looked at in both themes:**
    - `library-remove-long`: the core's A F19.
    - `import-folder-elsewhere`, `-refused`, `-skipped` and `-stopped`: the LIB agent's C4 to C8.
    - `note-failed`: C12e. The answer is kept, the reason is in the footer and the button is on.
  - **Changed, by a fix:**
    - `task-files-plan` and `-dark`: the AI agent's fix; the file the model did not place is
      unticked, and only its checkbox differs.
    - `import-folder-ai-failed` and `-dark`: "Nothing was written." is said once.
    - `import-folder-ai-sorting` and `-dark`: the preview's last line while the files are sent
      (D-190).
  - **Changed, with shared's new wording (`f0f8159`, the same on `d1330b0`):**
    `plan-split-ai-dropped` and `-dark` ("left with no lines of their own"); `runs-undo` and
    `-dark` ("with 23 lines").
  - **Changed, by the clock and the folder:**
    - `runs`, `-dark`, `-newer` and `-newer-dark` differ only in the times they draw.
    - `dialog-recent` differs only in the folder the shots were taken from.
- Beside each branch's own set, every difference is another branch's shot, the clock or the
  folder. The pin's move from `f0f8159` to `d1330b0` changed no shot. A second run after the
  status fix differs from the first only in the four `runs*` shots, by the clock.
- **mdview `fix/audit4`, pinned on `d1330b0` (`9903fbd`):** fmt and clippy clean. 704 passed,
  0 failed, 6 ignored, and the preview server's 19 passed. The 168 shots are byte for byte those
  of `dd87553`; `shots-check.sh` 126 wanted, 0 bad.
- **Not run:**
  - The budgets, in either app. In mdedit, audit 4's changes run on a command, a card or a
    re-read, not on launch, a keystroke or a scroll. In mdview, the shared changes after
    `f0f8159` are in code it does not call.
  - CI, on any branch.
  - Anything on Linux or macOS, and anything driven by hand in a real window.
- **Control characters:** every tracked `.md` in the five worktrees was searched. One backspace
  was found in `DESIGN.md` (`%LOCALAPPDATA%\mdedit\backup` had lost its `\b`). It came from
  `main`'s `b9d527b`, so `fix/audit4-ai` and `fix/audit4-lib` have it too. It is fixed on mdedit
  `fix/audit4`, and is still on `main`.

## The review of the fixes

The Fable agent's findings (`REVIEW.md`), each reproduced before it was fixed. Every fix has a
test that fails with it taken out: 18 mutations in shared, 13 in mdedit and 4 in mdview, one at
a time, each failing its test.

| # | Finding | State, test, revert check, left |
|---|---|---|
| S1 | RB2: a rewrite record that just fits is lost at its undo | done (shared): see RB2 |
| S2 | A F9: a dead run's undo takes a later run's notes | done (shared, mdedit's report): see A F9 |
| S3 | C11 fixed for one shape of file | done (shared, mdedit's gate): see C11 |
| W1 | "File them" writes a group file open with unsaved changes | done: `task.rs` `task_file_write` asks `library_file_free` first and reads through `library_text`, as every library writer does, writing the mark back. Tests `file_them_is_refused_while_the_group_file_has_unsaved_changes` (the file is not written, the window stays, the toast says why), `file_them_keeps_a_byte_order_mark_and_refuses_a_utf16_file`; revert-checked three ways: the refusal, the reading, the mark written back |
| W2 | E12: no test that a real window looks for the library | done (mdview): see E12 |
| T1 | RB4's stamp check pinned by no test | done: see RB4 |
| T2 | B11's `split_asked_here` pinned by no test since the merge | done, the guard kept: see B11; D-194 |
| T3 | B8's `sort_back` `folders_back()` pinned by no test | done: see B8 |
| T4 | a restaged test not listed | done: listed above, and its lost case has a test. `links_pending_with_a_group_file_this_run_made_is_the_runs_to_take_away` refuses step 6's write through a switch compiled into the tests only (`plan_write::REFUSE_LINKS`), as a race would, so the group file the run made stands with no link; Finish is offered, and Undo takes the notes and the group file away. Fails with the record not told the run made the file |
| T5 | mdview's E12 test sets `MDEDIT_SETTINGS` for the process | done (mdview): see E12 |
| T6 | the rewrite question's wording; `files::replace` on a dangling link | done (shared): see A F8 and D1 |
| nit A F16 | seven direct `build` calls | done: see A F16 |
| nit A F14 | `note.rs` compares with `same_path` | done: see A F14 |
| nit B5 | a drop or the picker adds files while a request is out | left: files are appended, so every index the request holds stays right, and the new files are not in what was asked; D-168 keeps the task, the prompt and the group, which change what was asked |
| nit RB3 | a name of 251 characters cannot carry `.part` on Windows | done (shared): such a name gets a short temporary of its own beside it. Test `a_file_with_a_name_too_long_for_the_ending_is_replaced`, revert-checked |
| nit RB5 | `EF BB BF FF FE` passes the UTF-16 test; the stamp | the first done, the second left: see D3 |
| nit D4 | no check without `MaxCapacity` | left: see D4 |
| nit RB2 | `rewrite_fits` serialises the record per file | left: a cost on a click of Apply, up to a few milliseconds for a 4 MiB record; not on launch, a key or a scroll |
| nit E5 | the first Ctrl+P after a read resolves every note | left: see E5 |
| nit E7 | `keep_top` after a refused tick | left: see E7 |
| nit E8 | `zoom::Origin` does not hold the device scale | left as decided (mdview's HANDOFF): a scale change keeps the view open at the same density, which E2 made right |
| nit E12 | the Notes switch appears a frame or more after the window | left: that is the fix (the look is off the first frame); no shot can show it |
| flake | shared `crash::tests::a_panic_writes_a_report_and_the_old_hook_still_runs` failed once in 30 runs | done: a race between tests. The panic hook is the whole process's, so any test that panicked beside it in the library's test program (a failing one, as under a mutation) wrote its report into that test's folder. The test is in its own test program now (`tests/crash_hook.rs`), where nothing else runs |

### Checks after the review's fixes

On Windows, on the pin at `ef88814`:

- **shared** `ef88814`: fmt and clippy clean; the whole workspace's tests in release: 754
  passed, 0 failed, 8 ignored; with the feature `recycle`, `mdshared-ui`'s recycle tests 11
  passed, 1 ignored. The Unix-only test `replace_through_a_dangling_link_writes_nothing` did
  not run here.
- **mdedit** `fix/audit4`: fmt and clippy clean; the whole workspace's tests in release:
  1583 passed, 0 failed, 14 ignored. The shots: 400 taken, `shots-check.sh` 211 wanted (the
  `runs*` shots are taken in a loop, which it does not count), 0 bad.
  Against the set of the merge: `runs-undo-later`, `runs-undone-later` and their `-dark` are
  new, each looked at in both themes (below); the four `runs*` shots differ by the clock only;
  every other shot is byte for byte the same.
  - `runs-undo-later`: the undo question of a run that died before its notes, saying "2 files
    are a later run's and stay."
  - `runs-undone-later`: the report after it, "Undid the run · 2 files and 2 lines", with
    "A later run's, kept (2)" listing the two notes, each "a later run wrote it or lists it".
- **mdview** `fix/audit4`: fmt and clippy clean; 704 passed, 0 failed, 6 ignored; the preview
  server's 19 passed; the 168 shots byte for byte those before; `shots-check.sh` 126 wanted, 0
  bad.
- **Not run:** CI; anything on Linux or macOS (the Unix-only tests, A F12, the fold where
  Windows folds `..` itself); the budgets (every change is on a command, a card, an undo or a
  test; none is on launch, a key or a scroll); no window driven by hand. A debug build's test
  `closed_stderr` fails on this PC, as before: icu4x's data provider prints a warning with
  `eprintln!` in a debug build, which panics on a closed stderr; the release build (CI's) does
  not print it.

## Left, all together

- **The owner's questions:** A F8's record (D-162); C4's group kept elsewhere, refused for now
  (D-183, replacing D-178's interim); the two shared root causes, a literal `%20` read as a
  space (C6) and `runs::write_of` marking Done a run that wrote nothing (C3), both D-180; W11's
  new dependency.
- **Both apps, a protocol change:** E6's other half, E7 across batches, E11b.
- **Shared:** C12b's writer that inserts a given line.
- **Not checkable here:** W12; D4's registry read and the case with no `MaxCapacity`; real bin
  quotas.
- **From the review of the fixes, left with their reasons:** B5's drop while a request is out;
  RB5's stamp; RB2's cost per file; E5's first open after a read; E7 after a refused tick (a
  protocol change); E8 and E12 as decided; `run()` in mdview reached by no test.
- **CI only (Linux and macOS):** A F12; A F14's fold where Windows folds `..` itself; W3's mode
  and link tests; C10.4's symlink test.
