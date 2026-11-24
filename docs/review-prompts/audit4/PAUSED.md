# Audit 4 fixes: paused on the owner's word (2026-11-24)

Everything is committed and pushed. Nothing is merged to any `main`. `FIXES.md` (in this folder)
has every row, from all three mdedit agents, as of the core agent's pause. The LIB agent's last
round (C4 to C12h, below) is newer than its rows there.

## Branches and worktrees

| Repo | Branch | Worktree | Tip | State |
|---|---|---|---|---|
| shared | `fix/audit4` | `md-wt4-shared` | `d1330b0` | **finished**, green (742 + 11 recycle) |
| mdedit core | `fix/audit4` | `md-wt4-mdedit` | this commit (wip) | one test to restage; the branch's shots not taken |
| mdedit AI | `fix/audit4-ai` | `md-wt4-ai` | `838b748` | **finished**, green (1527), revert checks and shots done |
| mdedit LIB | `fix/audit4-lib` | `md-wt4-lib` | `ff1caf8` (wip) | `c6739bc` green (1546); its top commit not built; revert round 2 half run |
| mdview | `fix/audit4` | `md-wt4-mv` | `dd87553` | **finished**, green (704 + 19), shots and budgets done |

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
  - Shared `paths::comparable` folding a `..` after a missing folder: done in shared `d1330b0`
    (`dbd4faa`); mdedit's own fold can go once the pin moves there.
  - C12b needs a shared writer that inserts a given line.
  - T3 is in shared `split.rs`.

## mdedit AI (`fix/audit4-ai`): finished

- **Ran:** the whole workspace passed 1527, failed 0. Clippy and fmt are clean. The shots
  (384) were checked clean, and every `task-*`, `review-*` and `import-folder-ai*` shot was looked
  at in both themes.
- **Revert checks:** each fix's test fails at its own assertion with the fix taken out. That
  covers RB1, RB2, RB3 (task.rs), B3 to B14, B n1, B n2 and B n4.
- **Also fixed:**
  - `task-files-plan` showed the unplaced file ticked; it is unticked now, as the answer path
    leaves it.
  - A scheme that could not be read said "Nothing was written." twice.
- **Decisions:** B8, B11, B n1 and B n4 are written as decisions by the core agent (D-155 to
  D-182).

## mdedit LIB (`fix/audit4-lib`)

- **Ran:**
  - At `c6739bc` (all of 4a): fmt and clippy are clean, and the whole workspace passed 1546,
    failed 0. The shots were checked clean: 392.
  - Looked at in both themes: `import-folder-skipped`, `-elsewhere`, `-refused` and `-stopped`.
- **Not built:** the top code commit `f99c1f2`. It holds the `note-failed` demo and shot, the
  test `a_refusal_at_the_write_that_can_pass_leaves_the_button_on` (C12e), and DESIGN 82.
- **Done, with tests that fail with the fix reverted:** C1, C2, F15, C3 and C4.
- **Done, tests pass, revert check not run:** C5, C6, C7, C8, C10.3, C10.4, C12f, C12g and C12h.
- **Done, with no revert check possible:**
  - C9: its test is only a pin; one write and two cannot be told apart without a crash.
  - C12c: its test is a race between two reads.
- **Also done:** C10.1, C10.2 (`remove_own_file` gone), C10.5 and C12d.
- **Partial:** C12e. Its new test needs a build, a run and its revert check.
- **Next:**
  1. The two import-card text leftovers:
     - `import_ui.rs:1190`: drop " Nothing was written." from the `folder-ai-failed` demo,
       because the card's second line (`import_draw.rs:821`) already starts with it.
     - `import_draw.rs:743`: the `import-folder-ai-sorting` help says files can be renamed
       "before the files are sent" while they are being sent. Make it true for that state.
     - Look at both shots in both themes after.
  2. Build and run `f99c1f2`.
  3. Finish the revert round with `docs/review-prompts/audit4/lib-revert2.sh` on this branch.
     Delete its lines 26 to 30 first, which are done; the C4-line check on line 31 stays. Stopping
     it does not stop its children, so check `git status` after any stop.
  4. Run fmt, clippy, the whole suite and the shots. Look at `note-failed` and the other `note-*`
     shots.
- **Build setup:**
  - Every cargo command needs `--config 'build.rustc-wrapper=""' -j 4`.
  - Make `target/data` before `shots.sh`:
    - `cargo run --release -q -p mdedit-core --example gen … -- lines 100 target/data/100mb.txt`;
    - the same with `-- long 1 target/data/1mb-line.txt`.
- **Decisions** (the core agent wrote the earlier ones; these are newer than D-182):
  - **C4:** refuse to join a group of that name whose notes are kept in another folder, naming
    the group and its folder, at the plan and again at the write. A group with no folder of its
    own is joined, and the card says "with no folder of its own".
  - **C6:** a file whose link would not read back is skipped and named on the card. The rest go
    in.
  - **C5:** the library's own folder is left out of the walk, and the card says so.
  - **C8 and C12h:**
    - The walk stops at 50,000 entries or 64 folders deep, refused with the reason.
    - Running the command again stops a read in progress.
    - A finished read waits while another card is open.
  - **C9:** a new group file is written once with `create_new`, holding the dry run's text.
  - **The card's refusal:** it is said in the footer, and Add stays on.

## mdview (`fix/audit4`): finished

- **Ran:** the whole workspace passed 704, plus 19 for the preview server. fmt and clippy are
  clean.
- **Shots:** 168, all byte-identical to the baseline except the new `zoom-diagram-2x` pair, which
  was looked at. `shots-check` is clean.
- **Revert checks:** every targeted test fails with its fix reverted, except one E2 test, whose
  geometry holds either way.
- **Budgets:**
  - First frame: the same per stage as before. The PC was busy, so absolute figures say nothing.
  - Memory 25 MiB, no CPU at idle, the exe 30 KB larger.
  - The scroll frame was not measured.
- **Partial, needing a preview protocol change in both apps:** E6 (mdedit writes the note for
  mdview), E7 across batches, and E11b.
- **Decisions:**
  - The zoom view closes on a theme change and stays open on a scale change.
  - Library files past the 238th share one palette header.
  - A shot looks for the library synchronously.
- **Merge:** two of its commits are titled `wip:`. Squash them at merge.

## After all five

Merge the three mdedit branches into one. Move both apps' pin to shared `main` holding
`d1330b0`. Run fmt, clippy, tests and shots on each repo. Write FIXES.md completely. Then one
Fable agent reviews the fixes against FIXES.md, before anything is merged to `main`.
