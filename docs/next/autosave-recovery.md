# Autosave and crash recovery (key: autosave-recovery)

## README

Feature list: **Crash recovery.** Unsaved text survives a crash, a kill or a power loss: a copy of
every changed document is kept in the background, and the next start puts it back as modified
tabs ("Recovered 2 unsaved documents"). Nothing is written to your files. No shortcut and no
setting.

## DESIGN

**Store.** The folder `recover` beside the session file (so `MDEDIT_SETTINGS` moves it; none with
`--no-session` or in a shot). One file per modified document, `<run tag>-<document id>.rec`
(`mdedit_core::recovery`): a header (magic line, `path`, `untitled`, `encoding`, `eol`, `disk`
= size and modification time of the file when it was read, `bytes`) then `---` and the text.
A copy is valid only when the text after the header has exactly `bytes` bytes.

**When written.** `Recovery::poll` (`bins/mdedit/src/recovery.rs`) runs once per pass of the
event loop and compares each tab's buffer revision and encoding with what it last saw (integer
compares; no work with no modified tab). A changed document is written 3 s after its last change
(`RECOVERY_IDLE_MS`), and at the latest 30 s after the first change not yet written
(`RECOVERY_MAX_MS`), so continuous typing writes about twice a minute. The write is on a thread
that starts at the first write (none at launch): the UI thread takes an O(1) rope snapshot and a
header and sends them; the thread writes `<name>.tmp`, syncs, renames over the old copy. Jobs
for the same name that queue up are coalesced to the newest.

**Removed** (by a job to the thread) when the tab is saved, undone to the file's text, closed
(also "Don't save"), or discards its changes; all copies are removed on a normal exit
(`Recovery::finish`, joined first so no write comes after), together with the folder when empty.
Exception: a tab whose restored text was not yet put back keeps its copy (it is in no other
place); the next start offers it again.

**Limit.** A document over `RECOVERY_MAX_BYTES` (64 MiB) gets no copy (a copy it already had is
removed): rewriting it every half minute would cost more than it is worth. Above it, a crash
loses the unsaved text, as before; a normal close still keeps it through the session's backup.
Read-only tabs (a comparison's saved side) and documents read lossily get none.

**Normal exit vs crash.** The session already keeps unsaved text on a normal close
(`write_backups`, section 19). That is unchanged and is not shared with the recovery store
except that both are put back by the same path (`Doc::restore`, `put_back`). Because a normal
exit deletes the recovery files, what is found at the start is from a run that did not exit
normally. Recovery does not depend on "Restore tabs at start".

**At start** (only when the folder has files; one directory listing otherwise): files that are
not whole and `.tmp` files are deleted. For a copy whose file still exists the tab is added (or
the session's tab for that path is used) with `restore` set to the copy, loaded at once like a
session backup; when the file is read, the text goes over it as one edit (Undo gives the file's
text), and the encoding the copy names is applied when the text can be written in it. A copy of
a document without a file, or whose file is gone, opens as an untitled document with its text.
If the tab already has a session backup, the backup stays the only one and the copy is deleted.
The copy is renamed to this run's name for that tab at once, so a second crash before the next
write loses nothing. One toast "Recovered N unsaved documents" (singular for 1).

**File changed since.** If the file's size or time differs from the `disk` in the header, the
text is put back as modified, the tab's stamp is set to the copy's (so Save ends in the existing
"The file changed on disk" question) and the changed-on-disk banner shows; Reload drops the
recovered text. The file on disk is never written by recovery.

**Edge cases.** Two windows sharing a session folder are not possible (single instance); a
crash during the exit's cleanup can leave a copy beside a session backup (handled: backup wins).
Header values with line breaks are escaped; a header over 64 KiB is refused.

## Release note

Unsaved text now survives a crash or a power loss: mdedit keeps a recovery copy of changed
documents in the background and puts them back, as modified tabs, at the next start.

## Handoff

Built: `crates/mdedit-core/src/recovery.rs` (format, atomic write, read, tests),
`bins/mdedit/src/recovery.rs` (poll, thread, scan, restore, finish; 9 tests with real files),
limits in `limits.rs`, `Doc::recovered`/`read_restore`/`Ready::recovered_encoding` in `docs.rs`,
wiring in `app.rs` (start: restore + toast; `about_to_wait`: poll and timer; exit: finish),
`strings::recovered`, `--demo-recovered N` (shot only: the toast), two shots in `tests/shots.sh`
(recovered-light, recovered-dark).

Decisions: recovery is separate from the session backup (different folder and format, so the
session's code is untouched); the header holds size/time and the length, not a content hash
(the length catches a cut-short copy; a hash would need the text twice); no setting and no
"autosave to the real file". The thread starts on the first write and is joined at exit.

Not verified: nothing ran on a real window or on Windows locally (CI only); an actual
`kill -9` / power loss was not tried (the tests simulate it by dropping the store after the thread
finished); the cost on release builds was not measured. Why it cannot move the budgets: nothing
runs at start except one `read_dir` of a folder that normally does not exist; per event-loop pass
the poll is a loop over the tabs comparing integers; a keystroke adds no work.

Left out: a copy for a document over 64 MiB; text put back is lost for a file read lossily (the
same limit the session backup has); a toast naming which documents; no content hash.
