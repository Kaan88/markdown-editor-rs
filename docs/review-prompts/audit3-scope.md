# Audit 3: what to review (for Fable)

Written 2026-11-21 for mdedit 1.2.0. Audit 2's findings were all fixed for 1.2.0, but nobody has
reviewed the fixes. Review them, plus the changes made after audit 2. Read `docs/HANDOFF.md`
first: "Reviews: who audited what" and "Audit 2 (Fable, 2026-11-21)", which lists each fix and its
test. Audit 2's probe tests are in `docs/review-prompts/audit2-probe-tests.patch`.

## The audit 2 fixes (release/1.2.0, merged to main for 1.2.0)

A. Unsaved text, session and updater (commit `f9a32c9`):
1. One window per settings folder: `bins/mdedit/src/instance.rs` (`claim`, the lock on
   `settings.ini.lock`, the 10 s wait, `Busy`), and its use in `app.rs` `run()`
   (`instance_path`, `_folder_lock`, `no_session_note`). Check:
   - two launches at once;
   - a launch next to a hung window;
   - a window from before 1.2.0, which holds no lock. Seen once on the owner's PC: a 1.2.0
     launch next to a running 1.1.2 got no answer to its hand-over, ran on the same session,
     took `instance.port` and renamed the 1.1.2 window's recovery copies. Why the hand-over got
     no answer is not known.
   - the restart path (`relaunch::wait_for_predecessor`).
2. `Session::read` / `read_patiently` (`session.rs`): an unreadable session is an error, never
   an empty one; the window then runs without its session. The shared `settings::write_atomic`
   now syncs before the rename (markdown-shared-rs `75ddc1a`).
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

## Open, not fixed

- The owner reports that in visual editing, the wheel over the open block's box scrolls the page
  but the box stays where it is. The test above has the box, its text area and the text view
  follow the page after small and large scrolls, and reading the paint path found nothing.
  Not reproduced; the real window was not driven. Find the cause.
- `missing_parts` checks that the files exist, not their version.

## How to report

As in audit 2: findings by area, each with a confidence and a failure scenario. Mark what was
verified by running code and what comes from reading only. Probe tests go in a patch beside this
file.
