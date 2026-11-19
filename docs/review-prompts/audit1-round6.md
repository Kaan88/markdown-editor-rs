# mdedit 1.1.1 · request for round 6 re-review

Please re-review **Kaan88/markdown-editor-rs, branch `f/next`**, commit `3e6359c` ("mdedit 1.1.1:
fix the findings of the fifth audit round"). Compare it against your Round 5 report on `1fe830f`.
Shared crates are unchanged (`5a481b9`). Nothing is released; 1.1.1 waits for this review.

Read first: `docs/HANDOFF.md`, section "Audit 1, round 5". It maps each finding to its fix and test.
**Every test named there was checked to fail with its fix reverted.** The exceptions are the
updater download path and the cross-build script; see below.

## What changed, by finding

### Release blockers

1. **A copy locked after its header was accepted.**
   - `apply_load` (`docs.rs`) takes the older backup only when the copy has no text (gone, or not
     whole).
   - On an I/O error the tab keeps `restore`, `fallback` and `recovered`, so `Recovery::finish`
     keeps the copy, and the next start covers the backup with it again.
   - `read_restore` now returns `io::Result<Option<String>>`. For a backup, NotFound is
     `Ok(None)`.
   - Test `a_copy_locked_after_the_start_is_kept_for_the_next`: lock after `bring_back`, load,
     unlock, exit, next start. The text comes back.
2. **Edits while a backup waits.**
   - A tab whose text waits is set `read_only`. `edit_in` refuses with a toast that says why.
   - `read_only` cannot be turned off by the user.
   - `write_backups` writes no backup for a tab whose waiting text is a recovery copy without a
     fallback (the copy is kept). Otherwise it reuses the waiting backup as before.
   - Test `a_tab_waiting_for_its_text_takes_no_typing`.
3. **An untitled backup that cannot be read at the start.**
   - `restore_unsaved` records it in `Docs::unread_backups`.
   - `Docs::backups_needed` (the pruning, which replaces the inline prune in
     `persist_if_changed`) and `write_backups` keep it listed. So the file is never removed.
   - Test `an_untitled_backup_locked_at_the_start_is_kept`.
4. **Session not committed before cleanup.**
   - `write_backups` only writes, and a new backup avoids every name already in the folder
     (`.tmp` too).
   - `keep_unsaved` then writes the session, and only after that calls `drop_stale_backups`. When
     the session write fails, it puts the previous list back and returns false, so the Unsaved
     dialog asks instead of exiting.
   - At the final exit, `Recovery::finish(docs, saved)` keeps the copies of tabs still modified
     when `save_session` failed.
   - Test `an_exit_whose_session_cannot_be_written_keeps_everything`: the session path is a
     folder, and an old backup `1.txt` of another draft is listed. The test also asserts that
     `finish(_, false)` keeps the copy and `finish(_, true)` removes it.

### Round-4 leftovers

- **Legacy multiplication.**
  - `recovery::legacy_draft(file, entry)` is FNV-1a over the backup's name, the document's path
    or number, and the file's size and modified time. It is stable while the file is unchanged.
  - `restore_unsaved` gives an id-less entry's tab that id.
  - `cover` matches a copy with an id against an id-less backup by it (`(Some, None)` arm). The
    pre-1.1 path matching for id-less copies is unchanged.
  - Test `a_legacy_backup_does_not_multiply_when_the_session_is_never_written` runs four starts
    with the same unwritten session, each writing a copy and dying. Each start has one draft.
- **Session retry wake.** `about_to_wait` now takes `App::next_wake`: the minimum of the timers,
  the recovery look, the results deadline and `session_retry`. Test
  `a_failed_session_write_wakes_the_window_to_try_again`.

### Updates and packaging

- **The repair** (`updater.rs`):
  - It reads `repair_manifest_url()`: `releases/download/v<running version>/<manifest name>`.
    `MDEDIT_UPDATE_MANIFEST` still overrides it.
  - `spawn_look` replaces the shared `spawn_at_launch` and `spawn_check`. It runs `clean_up` at
    launch; the repair when a part is missing (at launch, due or not, and on Check for updates);
    and then the look.
  - A failed repair posts `Answer::Repaired(Err)`, and the window shows a toast that says it
    will try again.
  - A static flag keeps the launch and a manual repair from writing the same files at once.
  - Tested: the URL, and that nothing is fetched when nothing is missing. **Not tested:** a
    repair download end to end. `restore_engine` itself is the shared crate's.
- **Cross build.** The container's `bash -c` starts with `set -e`, so a failed `mdedit-ai` build
  stops before the engine build, and a cached exe is never packaged. **Not run** (it needs
  docker).

### Save prompt (`ai_events.rs`, `mdedit-core/src/ai.rs`)

| Case | Now |
|---|---|
| Prompts file open with unsaved edits | Refused with a toast; nothing is written. |
| Heading or fence in the instruction | `escape_line` adds one backslash before a `## ` at column 0 or a fence, counting any backslashes already there. `parse_prompts` takes one off outside fences (`unescape_line`). It reads back exactly, so the duplicate check finds it. A hand-written fence with no backslash is untouched. |
| Missing prompts file | The template is created and loaded before the duplicate check. |
| Over 256 KiB | Checked on the new text; refused. |
| File ends inside a fence | The new text is parsed. It is written only if it reads back as every earlier prompt plus this one. |
| CRLF, no final newline, trailing whitespace | The file's bytes are kept; what is added uses CRLF when the file has any. |

Tests:
- `a_saved_prompt_reads_back_exactly_and_the_file_is_kept` (core);
- `a_prompt_the_template_has_is_not_saved_twice`;
- `a_prompt_is_not_saved_under_unsaved_changes_to_the_file`;
- `a_prompt_that_would_not_read_back_is_not_saved`, which covers the open fence, the size and
  CRLF.

Behaviour change: a line in a saved prompt that starts with backslashes before `## ` or a fence
now reads with one backslash fewer. In Markdown that backslash is an escape anyway.

### Release notes

The paste is now compared with pasting a copied file. The notes also mention the repair retry
and the new exit behaviour.

## What I ran (native Windows x64)

Workspace tests with default and with no default features: **1005 passed, 14 ignored** each. All-
target clippy `-D warnings` for both, and `cargo fmt --check`. The revert check for every new test
listed above.

Not run: Linux, macOS, the cross-build script, a real repair download, the release workflow, real
providers. The owner has not yet looked at the AI strip or the Save prompt button.

## Please check in particular

- A tab whose text waits is read-only until the next start; nothing reads it again during the
  run. Is that acceptable, or should a retry be required?
- `keep_unsaved` failing on the session leaves the newly written backups unreferenced until the
  next successful exit removes them. Could they confuse a start in between?
- Whether `legacy_draft` can collide or change in a way that splits a draft. For example, a
  backup file whose time changes without its text changing.
- `spawn_look` replaces the shared launch function. Is anything from `spawn_at_launch` missing?
