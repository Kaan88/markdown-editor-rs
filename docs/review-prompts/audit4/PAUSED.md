# Audit 4 fixes: where the branches stand after the review of the fixes (2026-11-24)

The file kept its name from the owner's pause; the pause is over. Everything is committed and
pushed, and nothing is merged to any `main`. `FIXES.md` (in this folder) has every finding of
reports A to E with its state, its test, its revert check and what is left, and, under "The
review of the fixes", every finding of Fable's review of them (`md-wt\review-a4fix`), fixed or
left with why. This file says where each branch stands, what ran and what comes next.

## Branches and worktrees

| Repo | Branch | Worktree | Tip | State |
|---|---|---|---|---|
| shared | `fix/audit4` | `md-wt\a4-shared` | `ef88814` | **finished**, the review's fixes in; green (754 + 11 recycle) |
| mdedit | `fix/audit4` | `md-wt\a4-mdedit` | see `git log` | **merged**: the core's work, `fix/audit4-ai` (`6c1867e`) and `fix/audit4-lib` (`f070775`); then the review's fixes; pins shared `ef88814`; green (1583, 400 shots) |
| mdedit AI | `fix/audit4-ai` | `md-wt\a4-ai` | `838b748` | **finished**, merged |
| mdedit LIB | `fix/audit4-lib` | `md-wt\a4-lib` | `cabb09f` | **finished**, green (1549, 394 shots), merged |
| mdview | `fix/audit4` | `md-wt\a4-mv` | `bc33a45` | **finished**, the review's W2/T5 in; pins shared `ef88814`; green (704 + 19), shots unchanged |

All three mdedit branches started from `93b4ddb`. Shared `main` is `0984eed`, the base of its
`fix/audit4`, and has not moved. `ef88814` is a development pin in both apps (it was `d1330b0`
at the merge); when shared's branch is merged to its `main`, both pins move to the commit that
holds it.

## What the merge did and found

- **Conflicts:** one, in `import_tests.rs`, where both branches added tests at the end; both are
  kept. The docs (`FIXES.md`, DECISIONS, STATE, DESIGN) merged without a conflict.
- **The pin:** mdedit moved from `f0f8159` to `d1330b0`. Shared's `paths::comparable` now folds
  a `..` after a folder that is not there, so mdedit's own `folded()` in `library_write.rs` is
  gone; its tests pass on Windows (where they passed either way; their check is CI's).
- **One fault the merge found:** the core's B11 fix, `plan_write` stopping the plan's split that
  is out, left the status line saying the split was out. The AI agent's test for "Split them"
  failed on it (`task_tests.rs:1034`). `plan_write` now clears the status with the cancel, as
  `plan_close` does, and the core's test asks it too. With the status left, both tests fail.
- **Decisions:** the LIB agent's, as D-183 to D-190. C2's key rule was already D-177. D-183
  (C4) replaces D-178's interim and is still the owner's to overturn.
- **Control characters:** every `.md` in the three repositories was searched. One backspace was
  found in `DESIGN.md` (`%LOCALAPPDATA%\mdedit\backup`, from `main`'s `b9d527b`) and fixed here.
  It is still on `main`.

## What the review of the fixes found, and what was done

Fable's review (`md-wt\review-a4fix\REVIEW.md`, its patches and probes in `evidence\`) was
reproduced finding by finding before each fix. Its three faults are fixed in shared, each with
tests that fail with the fix out: a rewrite record measured in its longest state (S1, D-191),
undo leaving what a later run names (S2, D-192), and undo giving the library file back byte for
byte from what the add recorded (S3, D-193). "File them" writes the group file as every library
writer does (W1, D-195). mdview's library test drives the window's own start (W2/T5). The
tests it asked for (T1 to T6) are in; of the nits, A F16, A F14, RB3 and RB5's mark are done, the
crash test's flake was a race and is fixed, and the rest are left with why (D-198). Mutations:
18 in shared, 13 in mdedit and 4 in mdview, each failing as it should. The `FIXES.md` rows the
review disagreed with now say what is true.

## What ran at the merge, on Windows

- **mdedit `fix/audit4`:** fmt and clippy on the whole workspace, clean. The whole workspace's
  tests: 1571 passed, 0 failed, 14 ignored (the timing tests). The shots: 396 taken,
  `shots-check.sh` 211 wanted, 0 bad. What changed against `main` and why is in `FIXES.md`,
  "Checks at the merge".
- **mdview `fix/audit4` on `d1330b0`:** fmt and clippy clean; 704 passed, 0 failed, 6 ignored;
  the preview server's 19 passed; the 168 shots byte for byte those of `dd87553`;
  `shots-check.sh` 126 wanted, 0 bad.
- **Not run:** the budgets, in either app. In mdedit, every audit 4 change is on a command, a
  card or a re-read, not on launch, a keystroke or a scroll. In mdview, the shared changes after
  `f0f8159` are in code it does not call. CI has not run on any of the branches. Nothing ran on
  Linux or macOS, and nothing was driven by hand in a real window.

## What ran after the review's fixes, on Windows

- **shared `ef88814`:** fmt and clippy clean; 754 passed, 0 failed, 8 ignored, and the feature
  `recycle`'s 11.
- **mdedit `fix/audit4` on `ef88814`:** fmt and clippy clean; 1583 passed, 0 failed, 14 ignored;
  400 shots, `shots-check.sh` 211 wanted, 0 bad. New: `runs-undo-later` and `runs-undone-later`,
  each in both themes; the other `runs*` shots differ by the clock only.
- **mdview `fix/audit4` on `ef88814`:** fmt and clippy clean; 704 passed, 0 failed, 6 ignored;
  19; the 168 shots byte for byte; `shots-check.sh` 126 wanted, 0 bad.
- **Not run:** as at the merge, CI, Linux, macOS, the budgets and a window by hand.

## Each agent's part, in short

- **Shared:** every finding in report D, with tests, except W11 (needs `jiff`, rule 7), W12
  (cannot be checked here) and W13 (the in-place button change stays: both released apps draw
  it). Two shared root causes are the owner's, since changing them changes both apps: a literal
  `%20` read as a space (C6), and `runs::write_of` marking Done a run that wrote nothing (C3;
  mdedit works around it).
- **mdedit core:** report A; RB3's library writes through `files::replace`, with a BOM kept and
  non-UTF-8 refused; the read-only preflight; C12a; B11's core half; the docs.
- **mdedit AI:** report B, each test failing at its own assertion with its fix out; the
  `task-files-plan` shot's unplaced file unticked; "Nothing was written." said once.
- **mdedit LIB:** report C. Every revert-checkable fix was checked, in two rounds
  (`lib-revert2.sh` holds the last). C9 and C12c have no revert check: one is a pin, the other a
  race. Shots looked at in both themes: `import-folder-skipped`, `-elsewhere`, `-refused`,
  `-stopped`, `note-failed`, `import-folder-ai-sorting`, `-ai-failed`.
- **mdview:** report E. Every targeted test fails with its fix out, except one E2 test whose
  geometry holds either way. Budgets measured at `dd87553`: first frame the same per stage,
  memory 25 MiB, no CPU at idle, the exe 30 KB larger; the scroll frame not measured. E6, E7
  across batches and E11b need a protocol change in both apps.

## Next

1. Shared's `fix/audit4` to its `main`, both apps' pins to that commit, and the apps'
   branches to theirs. The review of the fixes is done and its findings are fixed or left with
   why. mdedit's branch has six `wip:` commits (`a7e06d1`, `77d7385`, `f99c1f2`,
   `b76a6ec`, `ff1caf8`, `6c1b059`) and mdview's two (`81dbe84`, `5943e6a`); what to do with them
   is decided at that merge.
2. CI on each branch, once, and its result looked at once.
