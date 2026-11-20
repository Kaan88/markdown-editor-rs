# mdedit · request for round 7 re-review

Please re-review **Kaan88/markdown-editor-rs, branch `f/next`**, the commit titled "mdedit: fix the
findings of the sixth audit round". It is one commit on top of `main` at `9cbd9eb`, which is the
published 1.1.1 plus its release record. Compare it against your Round 6 report on `3e6359c`.
The shared crates are unchanged (`5a481b9`).

Read first: `docs/HANDOFF.md`, section "Audit 1, round 6", which is in this commit. Each test
named there was checked to fail with its fix reverted.

## What changed, by finding

1. **Reload kept the recovery-only read-only state** (`docs.rs`, `apply_load`). A reload no
   longer carries `read_only` from a tab whose text was waiting (`doc.restore` set). It is set
   again only if the text still cannot be read. The only other read-only tabs are `open_copy`
   tabs, which never wait for text, so they keep it as before.
   - Test `app::tests::a_reload_that_puts_the_waiting_text_back_keeps_its_copy`:
     1. lock the copy, load the tab, unlock;
     2. `request(After::Reload)`, check the Discard dialog, `answer(0)`;
     3. wait for the threaded load;
     4. run the recovery look (`poll` at t0 and t0 + 60 s, then `flush`);
     5. drop the app, and start again: the copy's text comes back.
2. **An expired retry with an unchanged signature** (`app.rs`, `persist_if_changed`). When
   `session_retry` is due, the session is written even if the signature matches the one last
   written. A success clears the retry; a failure sets it 5 s ahead.
   - Test `app::tests::a_due_retry_is_made_when_nothing_changed_since`: a session saved, then
     made unwritable, Exit requested and cancelled, the retry made due (still failing, then
     writable).

## Your round-6 answers, taken as given

- Waiting until restart is acceptable; finding 1 makes the protected state consistent.
- Unreferenced backups stay until the next successful exit, and nothing else changed there.
- A legacy backup whose file time changes, while the session still cannot be written, comes back
  twice. This is recorded in HANDOFF as known and accepted; nothing is lost.

## What I ran (native Windows x64)

Workspace tests with default and with no default features: 1007 passed, 14 ignored each.
All-target clippy `-D warnings` for both, and `cargo fmt --check`. A revert check for both tests.

Not run: Linux, macOS, real repair downloads, the release workflow.
