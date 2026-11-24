# Audit 4 fixes: paused on the owner's word (2026-11-24)

Everything is committed and pushed. Nothing is merged to any `main`. `FIXES.md` (in this folder)
has the rows the core agent touched. The AI and LIB rows below are not yet in `FIXES.md`.

## Branches and worktrees

| Repo | Branch | Worktree | Tip | State |
|---|---|---|---|---|
| shared | `fix/audit4` | `md-wt\a4-shared` | `d1330b0` | finished, green (742 + 11 recycle), 57 of 60 revert checks fail as they should |
| mdedit core | `fix/audit4` | `md-wt\a4-mdedit` | `a7e06d1` (wip) | one test fails; the newest tests never ran |
| mdedit AI | `fix/audit4-ai` | `md-wt\a4-ai` | `77d7385` (wip) | 84 targeted tests pass; whole suite, clippy and shots not run |
| mdedit LIB | `fix/audit4-lib` | `md-wt\a4-lib` | `d0436c3` | 32 targeted tests pass; whole suite and shots not run |
| mdview | `fix/audit4` | `md-wt\a4-mv` | `5943e6a` (wip) | fmt and clippy clean; full suite not re-run after small cleanups |

All three mdedit branches start from `93b4ddb`. mdedit and mdview pin shared at `f0f8159`, which
is a development pin. At merge, pin to shared `d1330b0`, merged into shared `main`.

## Shared (finished)

- **Done:** every finding in report D, with tests, except the three listed as left below.
- **Left:**
  - W11 needs `jiff`, a new dependency (rule 7).
  - W12 cannot be checked here.
  - W13: the in-place button change stays, because both released apps draw it.
- **Two shared root causes left for the owner,** because changing them changes both apps:
  - A literal `%20` in a file name always reads as a space (C6).
  - `runs::write_of` marks Done a run that wrote nothing (C3; mdedit works around it).

## mdedit core (`fix/audit4`)

Paused again on the owner's word, 2026-11-24. Every row's state is in `FIXES.md`.

- **Done, with a test that fails with its fix reverted:**
  - RB3: every library write goes through `files::replace`. This covers the pane's writers, the
    run's gate, "Add this file" (shared `put_note`) and 4a's join through `library_attempt_at`.
  - The read-only preflight.
  - F17 (BOM kept, non-UTF-8 refused), F18, F19, F20 and F3 `offers_finish`.
  - F16: a row's menu closes when the rows under it are made again and its row is another
    (`library.rs` `library_build`).
  - C12a: a move across files looks at the note's line first.
  - B11's core half: `plan_write` stops its plan's split that is out.
  - RB4/A F1, F5b, F11d and F13 from before.
- **Done, but not revert-checkable here:**
  - F14's fold passes on Windows either way; its check is CI's.
  - F8 and F4's tests pin shared's fixes.
  - F12 runs off Windows only.
- **Docs written:**
  - DECISIONS D-155 to D-182: the core's, the AI agent's, the LIB agent's, C4 as D-178
    (Waiting) and F8 as D-162 (Waiting).
  - DESIGN 70, 71, 72, 73, 75 and 80 corrected.
  - `ai-library-plan.md` row 6 corrected.
  - STATE rewritten (144 lines), and the release notes' lines added.
  - `FIXES.md` rows for A, B and C from all three agents.
- **What ran:**
  - Every revert check above, one build each.
  - The four newest tests, targeted (pass).
  - fmt and clippy on the whole workspace (clean).
  - The workspace's tests: **one fails**. They are 823 passed and 1 failed in the mdedit bin; the
    rest of the workspace did not run after it. The failing test is `library_write_tests`
    `when_the_removal_fails_the_note_is_in_the_new_place_and_the_toast_says_so` (:724).
    - It stages a failed removal with a wrong line number, which C12a's new look now refuses
      before anything is written (`59ab827`).
    - Next: restage it so the removal fails for a reason the look cannot see, without weakening
      what it asserts. For example, make the old file read-only after `library_rows`:
      `files::replace` refuses it, while the look only reads it. Then run the whole suite again.
- **Half-done: the shots.** The baseline at `main` (`402d3da`) is taken, 384 PNGs, in
  `md-wt\a4-mdedit\target\shots\base`, built from `target\base-src`. This branch's shots are not
  taken yet.
- **Next:**
  1. Take the branch's shots: `cargo build --release`, then `tests/shots.sh target/shots/a4`
     with `CARGO_BUILD_RUSTC_WRAPPER=""` and `MDEDIT_SETTINGS` in a scratch folder.
  2. Run `tests/shots-check.sh` on them.
  3. Compare the set with `base` byte for byte, and look at every changed shot in both themes.
     `library-remove-long` and its `-dark` are new; the `runs*` shots draw the clock. Say which
     shot shows what.
  4. Record the result in FIXES (A F19's row).
  5. Budgets: none should move. Library writes, the preflight, `library_build` and the cards run
     on a command or a re-read, not on launch, a keystroke or a scroll. Nothing was measured.
- **Left for others:**
  - Shared `paths::comparable` should fold a `..` after a missing folder (D-165).
  - C12b needs a shared writer that inserts a given line.
  - T3 is in shared `split.rs`.

## mdedit AI (`fix/audit4-ai`)

- **Done, with a revert check:**
  - RB1 and B1;
  - RB2's mdedit side;
  - B3, B4, B5, B7, B10, B12, B13 and B14;
  - B6 (its record half).
- **Done with a test, no revert check yet:**
  - RB3 at `task.rs` (`file_them_leaves_a_tmp_file_of_the_owners_beside_the_group_file_alone`);
  - B8 (`keep_the_folders_after_a_sort_gives_the_folders_plan_back`);
  - B11 (`a_split_out_when_a_files_plan_is_written_never_reaches_the_next_files_plan`,
    `a_split_answer_is_read_only_on_the_plan_it_was_asked_for`);
  - B n4;
  - B9, whose test is strengthened but whose mutation check has not run;
  - B n1 and B n2, whose tests have not run.
- **Next:**
  1. Run the whole suite and clippy.
  2. Run the revert checks: RB3 (`write_atomic` back in `task_file_write`), B8 (drop
     `folders_back`/`keep_folders`), B9 (delete the cap at `import_ai.rs:192-194`), B11 (each of
     `split_asked_here` and the cancel in `task_plan_done`), B n1 (`to_lowercase`), B n2 (the
     strip at the end of `compose`), B n4 (the reset in `modal.rs`).
  3. Run the shots (`import-folder-ai*` may change from B8).
- **Left:**
  - `plan_write.rs` should cancel a split itself, as `plan_close` does. That file is core's.
  - The saved-prompts file stays on `write_atomic`; it is the app's own file.
- **Decisions:**
  - B8: every AI sort starts from the folders' plan. Ask, Sort, a failure and Keep the folders
    restore it, and ticks changed on a sorted plan are dropped when asked again.
  - B11: an answer belongs to the plan screen with the same name and outline.
  - B n1: section names are one heading by ASCII case only.
  - B n4: "Requests at once" stays per connection. A switch between a local and a hosted
    provider writes the new kind's default.

## mdedit LIB (`fix/audit4-lib`)

- **Done, with tests:**
  - C1 (`a_note_let_go_over_the_editor_at_a_rows_height_is_not_moved`);
  - C2 and F15 (`a_key_while_a_note_is_held_reaches_nothing_and_the_note_held_is_the_one_moved`,
    `rows_made_again_under_a_held_note_still_move_the_note_held`,
    `a_click_on_a_note_opens_the_note_pressed_after_the_rows_were_made_again`);
  - C3 (`a_write_that_makes_no_note_keeps_the_card_and_the_answer_and_leaves_no_record`,
    `a_run_that_made_no_note_takes_its_record_and_its_empty_folders_away`; mdedit removes the
    empty record itself until shared stops writing it);
  - C12c, C12d and C12e.
- **Call sites outside its list:** `app/input.rs` and `app/commands.rs`.
- **Not run:** the whole suite, the revert checks and the shots.
- **Not started:** C4 to C10, and C12f to C12h. The planned fix for each:
  - C4: add the group's home to `Where`. In `import_plan`, refuse via `plan.blocked`, naming the
    group and its folder, and check again at write. The owner may overturn this.
  - C5: skip the library folder in the walk.
  - C6: skip files whose link would not read back, and name them on the card's header line.
  - C7: split titles and previews on `\r` too, drop control characters, cap titles at about 200
    characters, and fall back to the file name.
  - C8: add a bound of about 50,000 entries with its own message. The command stops a running
    walk. `import_take` holds the result while another card is open, called from
    `about_to_wait`.
  - C9: one `create_new` of the dry run's full text; drop `remove_own_file` and the second write.
  - C12f: `rows_of` compares with `eq_ignore_ascii_case`.
  - C12g: fold `..` before counting.
  - C12h: add a depth bound.
  - Also draw the card's `refusal` in the footer.
  - Constraint: `import::Plan` and `import::Note` are struct literals in the AI agent's tests, so
    new fields go on `Walk`, `Where` or `ImportCard`.
- **Decisions:**
  - A note write that makes nothing is a failure shown on the card.
  - A name taken during the AI call gets the next free name.
  - During a moving drag, every key but Esc is swallowed.

## mdview (`fix/audit4`)

- **Done, with tests that fail with the fix reverted:**
  - E1: idle CPU per pass is 0.7 µs, from about 110 µs.
  - E2: one of its four tests does not prove the fix.
  - E3.
  - E4: the collisions began at the 85th file, not the 240th.
  - E5: a palette keystroke takes 1.6 ms, from about 20 ms.
  - E6's message.
  - E8, E9, E10, E12.
- **Partial:**
  - E6: mdedit writing the note for mdview is a new feature.
  - E7 across batches and E11b (a late `closed`): each needs a preview protocol change in both
    apps.
- **Next:**
  1. Run the full suite: `--workspace`, plus `-p mdview-preview --features
     mdview-preview/server`.
  2. Run `tests/shots.sh target/shots/final` and compare with `target/shots/base`. The shots
     `zoom-diagram-2x` are new.
  3. Run `shots-check.sh`.
  4. Measure the budgets against main's exe: first frame, private memory and exe size. Check
     that the library lookup with `MDVIEW_TRACE=1` comes after the first frame.
  5. Put the draft `docs/audit4-handoff-top-draft.md` into HANDOFF's top section, and delete the
     draft.
- **Decisions:**
  - The zoom view closes on a theme change and stays open on a scale change.
  - Library files past the 238th share one palette header.
  - A shot looks for the library synchronously.

## After all five

Merge the three mdedit branches into one. Move both apps' pin to shared `main` holding
`d1330b0`. Run fmt, clippy, tests and shots on each repo. Write FIXES.md completely. Then one
Fable agent reviews the fixes against FIXES.md, before anything is merged to `main`.
