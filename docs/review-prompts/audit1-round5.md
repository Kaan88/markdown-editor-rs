# mdedit 1.1.1 · request for round 5 review

Please review **Kaan88/markdown-editor-rs, branch `f/next` @ `1fe830f`**. Your last review (round 4)
was of `6cdf50e`. Everything from `6cdf50e..1fe830f` is in scope:

| Commit | What |
|---|---|
| `fdc19db` | Fixes for your round-4 findings |
| `23ea30c` | Release packaging: the AI engine in every package, format-2 update manifest |
| `0e87337` | Merge of `f/ai` into `main` (no changes of its own) |
| `4b9e90c`, `17126c7` | CI fixes: Unix clippy in a recovery test, wider timing margins in SSE tests |
| `1df9009` | New: image path paste, AI activity strip, Save prompt |
| `1fe830f` | Version 1.1.1 (1.1.0 was never published) and its release notes |

The shared crates are pinned at `5a481b9`. Nothing has been released. 1.1.1 is published only
after this review.

Read first: `docs/HANDOFF.md`, section "Audit 1, round 4". It maps each round-4 finding to its fix
and test.

## 1. Round-4 fixes (`fdc19db`)

Recovery identity moved from path/number to a stable **draft id**: `draft=` in the recovery header
and a `draft=<file>|<hex>` session line. Older builds skip both. Each test named below was checked
to fail with its fix reverted.

- **#1, #9: remap lost on an unwritten session; header rewrite.**
  - No header is rewritten any more. A copy covers a backup by draft id (`recovery::cover`).
  - A failed session write is retried after 5 s (`session_retry`).
  - Test `app::tests::drafts_survive_starts_whose_session_is_never_written`. The retry itself has
    no unit test.
- **#2: a readable header hid a valid backup.**
  - A file tab keeps the backup as `Doc.fallback` until the copy's text is in.
  - Test `a_copy_whose_text_cannot_be_read_gives_way_to_its_backup`.
- **#3: a read error deleted the only copy.**
  - `read_header_checked` and `read_checked` tell an I/O error from an invalid record.
  - Unreadable copies are kept by scan and at exit.
  - Test `recovery::tests::a_copy_that_cannot_be_read_now_is_kept`.
- **#4, #5: path case; ambiguous legacy entries.**
  - Legacy matching uses `same_path`. It covers only one to one, and only when the copy is at
    least as new as the backup.
  - A copy already taken by a file tab is never replaced.
  - Tests:
    - `copies_and_backups_without_ids_cover_only_when_certain`
    - `two_old_entries_with_one_number_and_a_copy_all_come_back`
    - `an_old_copy_older_than_the_backup_hides_nothing`
    - `two_copies_of_one_file_both_come_back`
- **#6: keys in diagnostics.**
  - The key is masked over the whole error body before any cut.
  - A body over 64 KiB shows a fixed message.
  - The OpenAI stream error is masked with the run's key.
  - CLI stderr at its cap drops the cut line.
  - Tests:
    - `an_error_body_loses_the_key_before_it_is_cut`
    - `a_streamed_error_loses_the_key_before_it_is_cut`
    - `stderr_at_its_limit_loses_the_cut_line`
- **#7: cancelling the model list.**
  - The read is cancellable, and each page checks the job.
  - The pre-POST cancel check is on the common path.
  - Test `a_cancel_during_the_model_list_asks_for_nothing_more`. It fails only with both the page
    check and the cancellable read removed; either one alone is enough.
- **#8: SSE activity.**
  - The reader counts every read as activity (`Activity`, `Watched`), and `pump` checks it before
    calling Idle.
  - A closed flag stops the reader after the run ends.
  - Tests in `sse::round4`. `17126c7` widened their timings after the Intel Mac runner failed one;
    the assertions are unchanged.
- **#10: long model names.** The 40-character rule needs mixed case and digits. Test in
  `proto::tests::keys_are_hidden`.

## 2. Release packaging (`23ea30c`)

`mdedit-ai` is installed beside the exe in:
- the Windows zip and installer;
- the AppImage (`usr/bin`);
- the Mac bundle (`Contents/MacOS`, signed with the bundle).

In the deb, rpm, tarball and Arch packages it goes in `lib/mdedit/`.

There are two Windows update manifests:
- `mdedit-update.txt` stays format 1 (exe and engine). 1.0.6 and older read it, and so does 1.1 on
  Linux.
- `mdedit-update-2.txt` adds `windows-x86_64.mdedit-ai.url/sha256` and is read by 1.1 on Windows.

The updater (`updater.rs`) asks for `mdedit-ai` as an extra. A 1.0.6 install that updates gets
`mdedit-ai` at its next start (`missing_parts`). `tests/release_manifest.rs` reads format 1 with
the 1.0.6 crate (`mdshared-app` at `49de4ecf`) and with the new one. The 1.0.6 crate must refuse
format 2.

Please check in particular:
- that a 1.0.6 install cannot be stranded: updated to 1.1.1 but without `mdedit-ai`, or offered a
  manifest it misreads;
- what happens when the `mdedit-ai` download or its hash check fails after the exe has already been
  replaced.

## 3. New in 1.1.1 (`1df9009`)

These are new features, not audit fixes. Their tests are new tests of new code. No revert check
was done.

- **A pasted image path becomes a link.**
  - Text that is only the full path of an existing image file can be bare or in double quotes, as
    Explorer's Copy as path gives it, with surrounding whitespace or a newline.
  - Pasted into a saved Markdown document, it goes through `paste_image_files`, the same as a
    dropped file: a relative link, or on another drive a copy beside the document. Anything else
    pastes as text.
  - The change is `image_paste::image_path_in_text` and `Commands::paste`. Test
    `a_pasted_image_path_is_an_image`.
  - Assumption to check: the reported bug was "Copy as path" text. Explorer's Ctrl+C on a file
    puts a file list on the clipboard, and that already linked from any folder.
- **AI activity.**
  - While `docs.ai_status` is set, a 2 px accent segment crosses the top of the status bar
    (`status::ai_strip`), stepped every 50 ms in `timers()`, and the status text uses the accent
    colour.
  - The step (`ai_phase`) is in the status region's signature, so only that region redraws.
  - No test. Please check that the timer stops when a run ends or fails, and that it does not keep
    the window awake when idle.
- **Save prompt.**
  - The instruction panel has a **Save prompt** button, shown when the field is not empty, and
    Ctrl+S.
  - `ai_save_prompt` appends to `ai-prompts.md`. It creates the file from its template if there is
    none, and writes with `write_atomic`.
  - The name is the first words, at most 40 characters (`core_ai::prompt_name`), with " 2", " 3"
    for duplicates, case-insensitive.
  - A line that would read as a `## ` heading or a fence gets a leading backslash
    (`core_ai::append_prompt`).
  - Text identical to a saved prompt is not saved again.
  - Tests `a_prompt_saved_from_the_box_reads_back` and
    `ai_tests::the_instruction_is_saved_as_a_prompt`.
  - Please check:
    - a prompts file with CRLF line endings, or with no final newline;
    - a file changed in the editor but not yet saved: the prompts file may be open in a tab;
    - the escaped lines: they read back with the backslash kept, which is what goes to the model.

## What I ran (native Windows x64)

Workspace tests with default and with no default features, all-target clippy `-D warnings` for both,
and `cargo fmt --check`: all pass on `1fe830f`'s code. CI (Windows, Linux, macOS arm64 and Intel)
passed on `main` @ `17126c7`, which has everything above except `1df9009` and `1fe830f`.

Not run: the new features by hand in the app (the owner has not looked at them yet), real
providers or CLIs, the real credential store, an update from 1.0.6 through the real manifests, and
the release workflow itself.
