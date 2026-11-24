# Audit 4: what was done with each finding

One line per finding, by report and number. Work is on `fix/audit4` in each repository, and in
mdedit also on `fix/audit4-ai` (the AI agent) and `fix/audit4-lib` (the LIB agent), none merged;
a row says which when it is not `fix/audit4`. States:

- **done (shared built)**: in `markdown-shared-rs`, where the whole workspace's tests, fmt and
  clippy passed. The revert checks of the last round (rv3) were stopped part-way.
- **done (built)**: in mdedit, written with its test; it builds with the `mdshared-*` pins on
  the shared branch's `f0f8159`, and fmt, clippy and the workspace's tests pass on Windows.
  **revert-checked** adds that its test was run with the fix taken out and failed, and says
  where. A test that runs only off Windows has not run anywhere.
- **partial: …**: what is left is said.
- **open**: not reached.

Two of mdedit's existing tests met a deliberate change in the shared branch when it was first
built, and were brought to it, not weakened:

- `plan_write_tests.rs`
  `an_undo_whose_library_file_has_unsaved_changes_keeps_the_notes_and_can_be_pressed_again`: an
  undo the gate refuses now leaves the record `UndoPending` (A F2, F3), not `LinksPending`. The
  test asserts that, and that Finish is no longer offered (`finishable()` false).
- `undo_rules.rs` `a_relative_path_with_dots_in_a_record_reaches_nothing_outside_the_library`:
  its record's group file did not exist, so it now got W2's sentence. The test asserts W2's
  sentence for that case, then writes the group file and asserts the old refusal for `../`.

## Release blockers (README order)

| # | Finding | State |
|---|---|---|
| RB1 | B1 digest cut inside a character | done (built, revert-checked by the AI agent): `task_ui.rs` `DIGEST_MAX`, cut on a char boundary |
| RB2 | B2 / D2 rewrite record over 4 MiB | done: shared `runs::rewrite_fits`, `RECORD_FULL` (shared built); mdedit rolls over to a second record, `task.rs` `task_rewrite_into`, test `a_rewrite_run_past_what_one_record_holds_goes_on_in_a_second_record` (built, revert-checked by the AI agent) |
| RB3 | D1 / B6 / A F7 / W3 `<file>.tmp`; audit 3 C9 | done (built, revert-checked) on `fix/audit4`: every library write goes through `files::replace` (`library_write.rs` `library_put_text`, read by `library_text`; `library.rs` `library_put` is shared `put_note`; the `plan_write` gate; 4a's write through `library_attempt_at`), and the plan's preflight refuses a read-only group file. Tests: `library_write_tests` `a_library_write_leaves_the_owners_tmp_file_alone_and_no_temporary_of_its_own` (fails with `write_atomic` back in `library_put_text`, the owner's file taken by the rename's write, and with `library_put`'s old code back, taken by the add's; both in the test's `read` at :27), `plan_write_tests` `a_group_file_that_cannot_be_written_leaves_the_notes_unlinked_and_the_record_says_so` (fails at :226 with `write_atomic` back), `a_read_only_group_file_is_refused_before_the_notes_are_written` (fails at :524 with the preflight's check out: the card no longer says why), `import_tests` `a_join_leaves_the_owners_tmp_file_alone_and_keeps_its_byte_order_mark` (4a; fails at :447 with `write_atomic` back). `task.rs` (File them): done by the AI agent on `fix/audit4-ai`, test `file_them_leaves_a_tmp_file_of_the_owners_beside_the_group_file_alone`, no revert check yet |
| RB4 | A F1 plan from a stale tab | done (built, revert-checked): `app/plan.rs`; the test in `plan_tests.rs` fails at :253 with the fix out, and at :280 with the byte comparison out |
| RB5 | D3 `put_note` lossy rewrite | done (shared built): `library/put.rs` refuses non-UTF-8, UTF-16 and a file changed on disk |

## Report A, mdedit core

| # | Finding | State |
|---|---|---|
| A F1 | stale tab plan | see RB4 |
| A F2 | bin refusal at undo orphans the note (= D W4) | done: new state `UndoPending` in shared `runs.rs` (shared built); mdedit `overlays.rs` "partly undone", `recycle_tests.rs` test strengthened to check the state and a second Undo (built) |
| A F3 | audit 3 C4: Finish for links pending; toast | done (built, revert-checked): `overlays.rs` `offers_finish` (test `finish_is_offered_for_a_run_whose_links_are_pending` fails at `overlays.rs:1933` with Finish for stopped and writing only), the toast "AI runs can finish it or undo it." (`commands.rs` `LINKS_PENDING_NEXT`), `docs/ai-library-plan.md` row 6 corrected; D-157 |
| A F4 | Finish does not record its headings (= D W6) | done (shared built): `runs.rs` |
| A F5 | audit 3 C6: undo removes a heading with the owner's sub-heading | done (shared built): `runs.rs` |
| A F5b | audit 3 C6: undo bins a note whose tab holds unsaved changes | done (built, revert-checked): `recycle.rs` `SparingUnsaved`, `plan_write.rs` `run_undo`, test `undo_keeps_a_note_a_tab_holds_with_changes_not_saved` (fails at `recycle_tests.rs:393` with the fix out) |
| A F6 | Move the notes offers another group's library file | done (shared built): `moves.rs`, `moves_tests.rs` |
| A F7 | write_atomic on the owner's files | see RB3 |
| A F8 | rewrite record not confined; question does not name the files | partial: shared `runs.rs` side done (shared built); mdedit shows `plan.sentence`, which names the files; test `plan_write_tests` `the_undo_question_of_a_rewrite_names_each_file_it_puts_back` passes, and cannot be revert-checked in mdedit (the sentence is shared's). D-162 names D-072 and waits for the owner |
| A F9 | crash during a write leaves notes the record does not own (= D W5) | done (shared built): notes marked created before they are written, `runs.rs` |
| A F10 | tests that do not pin what they name | done (built): F1, F2 and F14 have tests that pin them; F13's old test did not fail with its fix out, a new one does (see A F13); F4 under a section: `undo_rules.rs` `a_finished_run_under_a_section_is_undone_with_the_heading_finish_wrote` passes; it pins shared's fix and cannot be revert-checked in mdedit |
| A F11a | "their 1 lines" | done (shared built): `split/answer.rs`, `answer_tests.rs` |
| A F11b | `&& home != &req.library` waives the group-file check (= D N1) | done (shared built): `runs.rs` |
| A F11c | heading removal normalises mixed line endings | not a bug: the library writers normalise line endings on every write already |
| A F11d | `follow_move` leaves session and recents on the old path | done (built, revert-checked): `docs.rs` `follow_move`; the tests in `moving_tests.rs` fail at :298 and :335 with the fix out |
| A F12 | "Find the file…" off Windows rewrites the wrong note's link | done (built): `Picked::LibraryFile(Moving, Vec<PathBuf>)` carries the target; `ui.library.finding` removed; test `find_the_file_writes_the_link_its_picker_was_asked_for` is `cfg(not(windows))`, CI only; its revert check cannot run on this PC |
| A F13 | held question asked of the new document on a tab switch | done (built, revert-checked): `Visual::take_pending` in `app/visual.rs`, used by `preview.rs`. The unit test `a_held_question_is_not_asked_of_another_document` passed with `preview.rs` reverted; the new test `preview.rs` `a_question_held_for_the_page_is_not_asked_of_the_tab_switched_to` fails with it reverted (1b6e301) |
| A F14 | A7's check is lexical (`home: Trip/..`) | done (built): `library_write::same_folder` (shared `paths::comparable`, then `.` and `..` folded by their spelling, `folded`), used in both library_write checks, `importing.rs` and `plan_preflight`; tests `a_home_line_that_reaches_the_library_folder_by_dot_dot_is_refused`, `a_dot_dot_after_a_folder_that_is_not_there_is_folded`. On Windows both pass with the fold taken out, because Windows folds `..` already: the fold's check is CI's, on Linux and macOS. D-165. Left for shared: `comparable` should fold it, then the local fold goes |
| A F15 | finished library read rebuilds rows under a held note | done (built) by the LIB agent on `fix/audit4-lib`, with C2: the held note by its `RowId`; tests `rows_made_again_under_a_held_note_still_move_the_note_held` and others; no revert check yet |
| A F16 | row menu left open acts on another row | done (built, revert-checked): `library.rs` `library_build`, used by every re-read: a `LibraryRow` menu closes when its row is no longer the row it was opened on, and stays when it is. Test `library_write_tests` `a_row_menu_left_open_closes_when_the_rows_under_it_are_made_again` fails at :1728 with the check taken out. D-181 |
| A F17 | library file with a BOM (audit 3 C9) | done (built, revert-checked): `library_text` sets a UTF-8 BOM aside and refuses UTF-16 and non-UTF-8, `library_put_text` writes the BOM back. Tests `a_library_file_with_a_byte_order_mark_is_written_and_keeps_it` (fails at :1604 with the BOM not written back), `a_library_file_that_is_not_utf8_is_left_as_it_is_and_the_toast_says_why` (fails at :1633 with the refusal out). D-161 |
| A F18 | "Edit a library file" can write the template over `notes.md` | done (built, revert-checked): `library.rs` `first_library_file` makes `notes.md` with `files::create_new`; test `edit_a_library_file_never_writes_the_template_over_a_notes_file` fails at :1654 with `std::fs::write` back. D-161 |
| A F19 | "Take this section out" card size; comments that disagree | done (built, revert-checked): `overlays.rs` `LibraryRemove` grows from its body as `LibraryDelete` does, the body clipped above the footer; test `a_long_section_question_grows_its_card_to_its_lines` fails at `overlays.rs:2033` with the card fixed at 196; shots `library-remove-long` and `-dark` added, taken on Windows at `31371be` and looked at in both themes: the card grows to its four lines, drawn whole, the size and spacing of `library-delete-long`. The branch's whole set beside `main`'s (`402d3da`): 375 of 384 byte-identical, these 2 new, 9 changed. Of the 9, `runs`, `runs-dark`, `runs-newer`, `-newer-dark` differ only in the times, which follow the clock; `dialog-recent` only in the folder the shots were taken from; `plan-split-ai-dropped` and `-dark` (the band says "left with no lines of their own") and `runs-undo` and `-undo-dark` ("with 23 lines") take their wording from shared `f0f8159`. Nothing of this branch's own code changed a shot but the two new ones. `shots-check.sh`: 211 wanted, 0 bad. The comments: `link.rs` agrees with its code and D-105, nothing to change. D-164 |
| A F20 | crash folder for a bare or empty `MDEDIT_SETTINGS` | done (built, revert-checked): `crash.rs` `crash_dir` keeps no folder for a settings path with no folder in it; its test changed on purpose to assert that, and fails at `crash.rs:143` with the old `crash_dir`. D-163 |

## Report B, mdedit AI

| # | Finding | State |
|---|---|---|
| B1 | digest_of panics inside a character | see RB1 |
| B2 | rewrite record past 4 MiB | see RB2 |
| B3 | Discard and Copy result ask for the same file again | done (built, revert-checked by the AI agent): `task.rs` `task_review_left`, `ai_events.rs`; test `discard_leaves_the_file_and_the_run_asks_for_the_next` |
| B4 | kept review applied later runs another window's files | done (built, revert-checked by the AI agent): `FileReview.window`/`sent`, `task_window_of`, `task_rewrite_alone`; test `a_review_kept_from_a_closed_window_writes_its_file_and_sends_nothing` |
| B5 | task switch takes a click while a request is out | done (built, revert-checked by the AI agent): `Asking.task`; test `a_request_out_keeps_its_task_and_its_answer_is_read_as_that_task` |
| B6 | failed write leaves an original in the record and a `.tmp` | done (built, its record half revert-checked by the AI agent): `task_rewrite_finish` removes an empty record; test `a_rewrite_that_could_not_be_written_leaves_no_record_when_the_run_ends`; the `.tmp` half is RB3 (`task.rs`, `fix/audit4-ai`) |
| B7 | parked window comes back as File them | done (built, revert-checked by the AI agent): `task_open`; test `a_parked_rewrite_window_comes_back_as_rewrite_them` |
| B8 | "Keep the folders" after a sort keeps the AI's sections | done (built) by the AI agent on `fix/audit4-ai`: every sort starts from the folders' plan (`folders_back`, `keep_folders`); test `keep_the_folders_after_a_sort_gives_the_folders_plan_back`; no revert check yet. D-171 |
| B9 | requests-at-once test passes with its cap deleted | partial (AI agent, `fix/audit4-ai`): the requests-at-once test strengthened; its mutation check (the cap at `import_ai.rs:192-194` deleted) has not run |
| B10 | kept file review cannot be reached from the waiting window | done (built, revert-checked by the AI agent): Run shows the kept review; test `run_shows_the_kept_review_and_run_again_without_its_window_keeps_it` |
| B11 | split answer can land on the next file's plan | done (built): the AI agent's half on `fix/audit4-ai` (an answer is read only on the plan screen with the same name and outline; tests `a_split_out_when_a_files_plan_is_written_never_reaches_the_next_files_plan`, `a_split_answer_is_read_only_on_the_plan_it_was_asked_for`; no revert check yet, D-172); the core's half on `fix/audit4`: `plan_write` cancels the plan's split that is out, test `writing_the_plan_stops_its_split_that_is_still_out` fails at :1203 with the cancel taken out |
| B12 | file edited after it was added is sent as its old text | done (built, revert-checked by the AI agent): re-probe in `task_rewrite_begin`; test `a_file_edited_after_it_was_added_is_sent_as_it_is_now` |
| B13 | 20-file limit counts unticked files | done (built, revert-checked by the AI agent): `task_ui.rs` `recount_limit`; test `the_twenty_a_rewrite_takes_are_counted_among_the_ticked_files` |
| B14 | Rewrite them runs with no instruction | done (built, revert-checked by the AI agent): `REWRITE_NEEDS_INSTRUCTION`; test `rewrite_them_with_no_instruction_sends_nothing_and_says_why` |
| B n1 | `eq_ignore_ascii_case` vs `to_lowercase` for section names | done (AI agent, `fix/audit4-ai`): one rule, `eq_ignore_ascii_case`; its test has not run. D-173 |
| B n2 | hunks subset deletion at end leaves a final break | done (AI agent, `fix/audit4-ai`): `hunks.rs` strips the break a final deletion left; its test has not run |
| B n3 | `<file>.tmp` | see RB3 |
| B n4 | "Requests at once" kept per connection, not per preset | done (built) by the AI agent on `fix/audit4-ai`: a change between a local and a hosted preset writes the new kind's default (`modal.rs`); no revert check yet. D-174 |

## Report C, library writes

| # | Finding | State |
|---|---|---|
| C1 | DD drop outside the pane still moves | done (built) by the LIB agent on `fix/audit4-lib`: `row_at`, `drop_row` and `drop_place` take x; test `a_note_let_go_over_the_editor_at_a_rows_height_is_not_moved`; no revert check yet. D-177 |
| C2 | DD holds the note by row index | done (built) by the LIB agent on `fix/audit4-lib`: the held note's `RowId` in `LibraryPane` (`hold`, `held_at`, found again in `build`), every key but Esc swallowed while moving; tests `a_key_while_a_note_is_held_reaches_nothing_and_the_note_held_is_the_one_moved`, `rows_made_again_under_a_held_note_still_move_the_note_held`, `a_click_on_a_note_opens_the_note_pressed_after_the_rows_were_made_again`; no revert check yet. D-177 |
| C3 | phase 5 failed write drops the card and the answer | done (built) by the LIB agent on `fix/audit4-lib`: the card and the answer kept with the reason, the record and empty folders of a run that made no note taken away (until shared stops writing it, D-180), a taken name gets the next free one; tests `a_write_that_makes_no_note_keeps_the_card_and_the_answer_and_leaves_no_record`, `a_run_that_made_no_note_takes_its_record_and_its_empty_folders_away`; no revert check yet. D-175, D-176 |
| C4 | 4a joins a same-named group with another `home:` | open, decided: refuse it, naming the group and its folder (D-178, Waiting for the owner). Plan (LIB agent): the group's home in `Where`, refused through `plan.blocked`, checked again at write |
| C5 | 4a offers the library's own files | open (LIB agent's plan: skip the library folder in the walk) |
| C6 | 4a `%20` refuses the whole folder, naming no file | open (plan: skip files whose link would not read back and name them on the card's header line); the shared cause, `%20` read as a space, is the owner's (D-180) |
| C7 | 4a bare-CR title of many lines | open (plan: split titles and previews on `\r` too, drop control characters, cap titles at about 200 characters, fall back to the file name) |
| C8 | 4a walk cannot be stopped, no folder bound, card opens over others | open (plan: a bound of about 50,000 entries with its own message; the command stops a running walk; `import_take` holds the result while another card is open) |
| C9 | 4a new group made in two writes | open (plan: one `create_new` of the dry run's full text; drop `remove_own_file` and the second write) |
| C10.1 to C10.5 | test gaps | open |
| C11 | phase 5 undo leaves a blank line | done (shared built): `runs.rs`, the trailing blank line the add wrote goes with the link |
| C12a to C12h | smaller things | C12a done (built, revert-checked) on `fix/audit4`: a move across files looks at the note's line before the new place is written (`library_write.rs` `library_move`); test `a_move_across_files_by_a_stale_line_writes_neither_file` fails at :1750 with the look taken out. The existing test `when_the_removal_fails_the_note_is_in_the_new_place_and_the_toast_says_so` staged its half-move on a wrong line, which the look now refuses; it is restaged (`31371be`) with the old file read-only after the pane read it, so the look passes and the removal is refused. Its assertions are all kept, and it also asks that the toast names the read-only file. It fails at :742 with the half toast taken out. C12b open: keeping the line's own text across files needs a shared writer that inserts a given line (rule 8). C12c, C12d, C12e done (built) by the LIB agent on `fix/audit4-lib` (`note.rs`: one capped read, a bare CR written as LF, a refusal that can pass keeps the button). C12f (`rows_of` with `eq_ignore_ascii_case`), C12g (fold `..` before counting), C12h (a depth bound): open, LIB agent's plan |

## Report D, shared

| # | Finding | State |
|---|---|---|
| D1 | `write_atomic_bytes` takes `<name>.tmp` | see RB3 |
| D2 | record over 4 MiB | see RB2 |
| D3 | `put_note` lossy | see RB5 |
| D4 | Windows bin capacity | done (shared built): `platform/recycle.rs` reads `MaxCapacity` and refuses a larger file; test `a_file_larger_than_the_bin_is_refused` (the refusal text only; no real bin quota was tried) |
| W1 | file rewritten twice in one record half undone | done (shared built): `runs.rs` |
| W2 | group file never made, home outside: stuck, wrong sentence | done (shared built): a plain sentence, `runs.rs` |
| W3 | rename-over drops mode; `.tmp` left on failed rename | see RB3 |
| W4 | = A F2 | see A F2 |
| W5 | = A F9 | see A F9 |
| W6 | = A F4 | see A F4 |
| W7 | html_md escaping across tokens | done (shared built): `html_md.rs` holds a trailing `<` or `&`; test `escaping_looks_past_the_end_of_a_text_token` |
| W8 | html_md image alt ending in a backslash | done (shared built); test `an_image_alt_that_ends_in_a_backslash_keeps_its_image` |
| W9 | split compose promotes any ATX line | done (shared built): only the caller's headings are raised; test `only_the_callers_headings_are_raised` |
| W10 | plan card rename box takes clicks meant for Tick all | done (shared built): `plan/draw.rs` `clipped`; test `a_rename_field_half_out_of_the_list_takes_no_click_meant_for_tick_all` |
| W11 | Linux trashinfo has no DeletionDate | left: needs the `jiff` feature, a new dependency (rule 7); the owner's call |
| W12 | macOS bin through Finder | left: cannot be checked here; the other way loses Put Back |
| W13 | rule 8: `draw_dialog_button_disabled` changed in place | done (shared built): kept as changed (both released apps ship it); the plan card now calls the shared function, same pixels. Shots not taken |
| T1 | no tests for D1 to D3 | done (shared built): tests for `files::replace` and `put_note` |
| T2 | html_md escaping tests single tokens only | done (shared built): the W7, W8, N3, N4 tests |
| T3 | split's `headings_of` copies the module's rules | open: it is in shared `split.rs:1737`, not in mdedit; for the shared agent |
| N1 | = A F11b | see A F11b |
| N2 | `strip_verbatim` keeps `\\?\` past 260 | done (shared built): `paths::comparable`, used by `runs::inside`; Windows test `a_long_path_inside_the_library_is_inside_it` |
| N3 | `vbscript:` and `data:` links kept | done (shared built); test `script_and_data_links_keep_their_text_only` |
| N4 | html_md and split smaller nits | done (shared built): `<?` escaped, no marks in code, the rest's label is one comment, `line_count` of none; tests in `html_md/tests.rs`, `split.rs` |
| N5 | plan card rename scrolled out of view | done (shared built); test `the_wheel_keeps_the_name_being_typed_in_view` |
| N6 | plan card drops typed text with no key name | done (shared built); test `a_key_with_no_name_types_its_text_into_the_name` |
| N7 | `sister::Request` gained `closed` | not a bug: both apps build the struct with `..` |
| N8 | `rewrite_into` leaves the pushed original in memory | done (shared built): popped on failure |

## Report E, mdview

| # | Finding | State |
|---|---|---|
| E1 to E5 | | open |
| E6 | Add to the library while mdedit has the file open | partial: shared `put_note` refuses a file changed on disk (`PutError::Changed`, shared built); the mdview side is open |
| E7 to E11 | | open |
| E12 | notes face touches the disk; `UseDays::start` overflow | partial: `stale.rs` `saturating_add` and its test (shared built); the notes face's `is_dir` is open |
