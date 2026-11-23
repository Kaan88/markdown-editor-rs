# Audit 4: what was done with each finding

One line per finding, by report and number. Work is on the branch `fix/audit4` in all three
repositories, not merged. States:

- **done (shared built)**: in `markdown-shared-rs`, where the whole workspace's tests, fmt and
  clippy passed. The revert checks of the last round (rv3) were stopped part-way.
- **done (not built)**: in mdedit, written with its test, never compiled. mdedit cannot build
  until its `mdshared-*` pins are moved to the shared branch's commit.
- **partial: …**: what is left is said.
- **open**: not reached.

## Release blockers (README order)

| # | Finding | State |
|---|---|---|
| RB1 | B1 digest cut inside a character | done (not built): `task_ui.rs` `DIGEST_MAX`, cut on a char boundary |
| RB2 | B2 / D2 rewrite record over 4 MiB | done: shared `runs::rewrite_fits`, `RECORD_FULL` (shared built); mdedit rolls over to a second record, `task.rs` `task_rewrite_into`, test `a_rewrite_run_past_what_one_record_holds_goes_on_in_a_second_record` (not built) |
| RB3 | D1 / B6 / A F7 / W3 `<file>.tmp`; audit 3 C9 | partial: shared `files::replace` (unique temp name, keeps the mode, refuses read-only, no temp left; shared built). Left: mdedit's library writes still use `write_atomic`. Switch `library_write.rs:166` and `:642`, `library.rs:506`, `task.rs:935`, `importing.rs`, the `plan_write` gate to `files::replace` |
| RB4 | A F1 plan from a stale tab | done (not built): `app/plan.rs`, test in `plan_tests.rs` |
| RB5 | D3 `put_note` lossy rewrite | done (shared built): `library/put.rs` refuses non-UTF-8, UTF-16 and a file changed on disk |

## Report A, mdedit core

| # | Finding | State |
|---|---|---|
| A F1 | stale tab plan | see RB4 |
| A F2 | bin refusal at undo orphans the note (= D W4) | done: new state `UndoPending` in shared `runs.rs` (shared built); mdedit `overlays.rs` "partly undone", `recycle_tests.rs` test strengthened to check the state and a second Undo (not built) |
| A F3 | audit 3 C4: Finish for links pending; toast | partial: `overlays.rs` `offers_finish` + test, toast "AI runs can finish it or undo it." (`commands.rs` `LINKS_PENDING_NEXT`), `plan_write_tests.rs` assertion changed to the new words (not built). Left: `docs/ai-library-plan.md:350` still says "Try again / Undo"; decision entry |
| A F4 | Finish does not record its headings (= D W6) | done (shared built): `runs.rs` |
| A F5 | audit 3 C6: undo removes a heading with the owner's sub-heading | done (shared built): `runs.rs` |
| A F5b | audit 3 C6: undo bins a note whose tab holds unsaved changes | done (not built): `recycle.rs` `SparingUnsaved`, `plan_write.rs` `run_undo`, test `undo_keeps_a_note_a_tab_holds_with_changes_not_saved` |
| A F6 | Move the notes offers another group's library file | done (shared built): `moves.rs`, `moves_tests.rs` |
| A F7 | write_atomic on the owner's files | see RB3 |
| A F8 | rewrite record not confined; question does not name the files | partial: shared `runs.rs` side done (shared built). Not checked: whether mdedit's undo question names the files |
| A F9 | crash during a write leaves notes the record does not own (= D W5) | done (shared built): notes marked created before they are written, `runs.rs` |
| A F10 | tests that do not pin what they name | partial: F1, F2 and F14 have tests that pin them; F4's under-a-section case not checked |
| A F11a | "their 1 lines" | done (shared built): `split/answer.rs`, `answer_tests.rs` |
| A F11b | `&& home != &req.library` waives the group-file check (= D N1) | done (shared built): `runs.rs` |
| A F11c | heading removal normalises mixed line endings | not a bug: the library writers normalise line endings on every write already |
| A F11d | `follow_move` leaves session and recents on the old path | done (not built): `docs.rs` `follow_move`, tests in `moving_tests.rs` |
| A F12 | "Find the file…" off Windows rewrites the wrong note's link | done (not built): `Picked::LibraryFile(Moving, Vec<PathBuf>)` carries the target; `ui.library.finding` removed; test `find_the_file_writes_the_link_its_picker_was_asked_for` is `cfg(not(windows))`, CI only |
| A F13 | held question asked of the new document on a tab switch | done (not built): `Visual::take_pending` in `app/visual.rs`, used by `preview.rs`; test `a_held_question_is_not_asked_of_another_document` |
| A F14 | A7's check is lexical (`home: Trip/..`) | partial: `library_write::same_folder` (via shared `paths::comparable`) used in both library_write checks, `importing.rs` and `plan_preflight`; test `a_home_line_that_reaches_the_library_folder_by_dot_dot_is_refused` (not built). Left: shared `comparable` must fold a `..` that follows a missing folder, because on Linux and macOS `std::path::absolute` keeps it |
| A F15 | finished library read rebuilds rows under a held note | open (plan: put the drag guard inside `library_read_take`, for any `LibraryNote` drag) |
| A F16 | row menu left open acts on another row | open (plan: close a `PopupKind::LibraryRow` menu in `library_accept`) |
| A F17 | library file with a BOM (audit 3 C9) | open |
| A F18 | "Edit a library file" can write the template over `notes.md` | open |
| A F19 | "Take this section out" card size; comments that disagree | open |
| A F20 | crash folder for a bare or empty `MDEDIT_SETTINGS` | open |

## Report B, mdedit AI

| # | Finding | State |
|---|---|---|
| B1 | digest_of panics inside a character | see RB1 |
| B2 | rewrite record past 4 MiB | see RB2 |
| B3 | Discard and Copy result ask for the same file again | done (not built): `task.rs` `task_review_left`, `ai_events.rs`; test `discard_leaves_the_file_and_the_run_asks_for_the_next` |
| B4 | kept review applied later runs another window's files | done (not built): `FileReview.window`/`sent`, `task_window_of`, `task_rewrite_alone`; test `a_review_kept_from_a_closed_window_writes_its_file_and_sends_nothing` |
| B5 | task switch takes a click while a request is out | done (not built): `Asking.task`; test `a_request_out_keeps_its_task_and_its_answer_is_read_as_that_task` |
| B6 | failed write leaves an original in the record and a `.tmp` | done (not built): `task_rewrite_finish` removes an empty record; test `a_rewrite_that_could_not_be_written_leaves_no_record_when_the_run_ends` (the `.tmp` half is RB3) |
| B7 | parked window comes back as File them | done (not built): `task_open`; test `a_parked_rewrite_window_comes_back_as_rewrite_them` |
| B8 | "Keep the folders" after a sort keeps the AI's sections | open |
| B9 | requests-at-once test passes with its cap deleted | open |
| B10 | kept file review cannot be reached from the waiting window | done (not built): Run shows the kept review; test `run_shows_the_kept_review_and_run_again_without_its_window_keeps_it` |
| B11 | split answer can land on the next file's plan | open |
| B12 | file edited after it was added is sent as its old text | done (not built): re-probe in `task_rewrite_begin`; test `a_file_edited_after_it_was_added_is_sent_as_it_is_now` |
| B13 | 20-file limit counts unticked files | done (not built): `task_ui.rs` `recount_limit`; test `the_twenty_a_rewrite_takes_are_counted_among_the_ticked_files` |
| B14 | Rewrite them runs with no instruction | done (not built): `REWRITE_NEEDS_INSTRUCTION`; test `rewrite_them_with_no_instruction_sends_nothing_and_says_why` |
| B n1 | `eq_ignore_ascii_case` vs `to_lowercase` for section names | open |
| B n2 | hunks subset deletion at end leaves a final break | open |
| B n3 | `<file>.tmp` | see RB3 |
| B n4 | "Requests at once" kept per connection, not per preset | open |

## Report C, library writes

| # | Finding | State |
|---|---|---|
| C1 | DD drop outside the pane still moves | open (plan: `drop_row`/`drop_place` take x and refuse a point outside the list) |
| C2 | DD holds the note by row index | open (plan: keep the held note's `RowId` in `LibraryPane`, find its row at release, drop nothing when gone; swallow keys but Esc while moving; tick guard for any `LibraryNote` drag) |
| C3 | phase 5 failed write drops the card and the answer | open |
| C4 | 4a joins a same-named group with another `home:` | open. Decided (owner's question, flagged): refuse it, as DESIGN 81 says, with a message naming the group and its folder; decision entry still to write |
| C5 | 4a offers the library's own files | open |
| C6 | 4a `%20` refuses the whole folder, naming no file | open |
| C7 | 4a bare-CR title of many lines | open |
| C8 | 4a walk cannot be stopped, no folder bound, card opens over others | open |
| C9 | 4a new group made in two writes | open |
| C10.1 to C10.5 | test gaps | open |
| C11 | phase 5 undo leaves a blank line | done (shared built): `runs.rs`, the trailing blank line the add wrote goes with the link |
| C12a to C12h | smaller things | open |

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
| T3 | split's `headings_of` copies the module's rules | open |
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
