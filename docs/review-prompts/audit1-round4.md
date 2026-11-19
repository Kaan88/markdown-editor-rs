# mdedit AI rewriting · request for round 4 re-review

Please re-review **Kaan88/markdown-editor-rs, branch `f/ai`**, the commit after `41db024` titled
"mdedit: fix the findings of the third audit round of AI rewriting". Compare it against your Round 3
report on `41db024`. The shared crates are unchanged since round 3 (`1cc5220`). The 1.1.0 release
work (shared pin move, format-2 manifest, packaging) is **not** in this commit and is out of scope.

Read first: `docs/HANDOFF.md`, section "Audit 1, round 3". It maps each finding to its fix and test.
Every test named there was checked to fail with its fix reverted, except finding 5's startup session
write and the cancellable model enquiry (neither has a direct test; see below).

## What changed, by finding

1. **One recovery file hid two drafts.** Startup restore is now `bring_back` (`bins/mdedit/src/app.rs`):
   - which copy covers which backup is settled before any number is given out;
   - recovery copies are restored **before** session backups, so they keep their numbers;
   - a copy that comes back under another identity (file gone, number taken) rewrites its header to
     that identity at once (`Recovery::restore`, `bins/mdedit/src/recovery.rs`);
   - a backup the copy covers takes the copy's new identity (`Restored.idents`).

   Test: `app::tests::two_drafts_survive_a_second_interruption` covers your scenario: three starts
   with no new recovery write between them, then Save As on the covered tab.
2. **Queued failure, window asleep.** `Recovery::poll` reads the in-flight count, then the failure
   list, and wakes in 1 s if either is non-empty. A writer whose send failed is dropped and started
   again on the next write. The test now asserts the deadline the dispatching poll returns.
3. **Failed clipboard copy closed the review.** Copy closes the review only after a successful
   copy. Test seam: `ai_events::CLIPBOARD_FAILS`, used only in tests.
4. **Credentials in diagnostics.**
   - `keys::usable` applies the 4-character minimum to store entries and to `MDEDIT_AI_KEY`;
   - `keys::set` validates before the override short-circuit;
   - `body_message` cuts its 200-character excerpt at a word boundary, and shows a fixed message
     when no whole word fits;
   - `read_part` drops the word at a 64 KiB cut before redaction.

   Not done: the broader "structured provider errors over the prefix list" suggestion. Prefix-based
   masking is still the fallback.
5. **Remap only in memory.** When `bring_back` changes an entry, `run` writes the session at once,
   before the event loop, so before any recovery write. The `remapped` flag is asserted in #1's test.
   The write itself is not unit-tested.
6. **Claude reader and consumer disagreed.** `claude_last` and `openai_last` (`bins/mdedit-ai/src/api.rs`)
   classify by the top-level JSON type. They stop on exactly the events the stream consumers end on
   (Claude: `message_stop` and `error`; OpenAI: `[DONE]` and an error object).
7. **Heartbeats timed out as idle.** The SSE reader sends `Item::Alive` at most every 200 ms for lines
   that produce no event; `pump` refreshes its idle clock on it. A reader whose consumer has gone
   (after a cancel) stops at its next line, through `try_send` → `Disconnected`.
8. **Older key reply bypassed the URL pause.** `ai_settings_needs` skips the key lookup and model
   discovery while `url_due` is set; the timer calls it again. Replies are still cached under their
   own slot. The test drains A's reply before the timer fires.
9. **Apply visible after the source closed.** `ai_review_sync` marks the review closed before compose,
   before key handling and before any review action. The test composes before pressing anything.
10. **Event limit excluded the name.** `Parser::size()` is name plus data; the 1 MiB check uses it.
11. **Shared test directory.** `file_docs` numbers each call's folder; the N1 test uses its own name.
12. **Model alias hidden.** The prefix rule now also needs a run of 16 or more characters between
    `-`, `_` and `.`, or 40 characters in all (`random_part`, `proto.rs`).

**Cancellation scope note.** The cold model enquiry of a Claude run now takes the run's job, so a
cancel ends its error-body read. The run checks for a cancel before the rewrite POST. Traced, not
tested.

## Also new

`ai_tests` now runs on Windows. The stand-in engine script runs under Git's bash from a generated
`.cmd`, found at `%ProgramFiles%\Git\bin\bash.exe`; `MDEDIT_TEST_BASH` overrides it. Please confirm
it works on your Windows setup without your earlier launcher adaptation.

## What I ran (native Windows x64, Rust 1.99.1)

Workspace tests with default and no default features, and all-target clippy `-D warnings` for both
feature sets: results are in the commit message. Also `cargo fmt --check`.

Not run: Linux and macOS this round, real providers or CLIs, the real credential store, the
30-minute body timeout.

## Please check in particular

- Recovery ordering edge cases: two found copies with the same untitled number; a found copy for a
  file that reappears; a copy that cannot be read (its entry is now treated as uncovered and the
  backup restored).
- Whether rewriting a recovery header at startup can lose text if that write fails. It logs and
  keeps the old header.
- That the `Alive` throttle cannot starve a slow multiline event of idle refreshes.
- Any remaining path where a diagnostic is shortened before the exact-key mask.
