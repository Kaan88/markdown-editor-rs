# Review A: split / runs / plan / move / undo / recycle, the notes pane, the fixes since v1.3.0, Settings and the rest

mdedit at `1d503d8`, worktree `C:\Users\kaank\Desktop\md-wt\fab-a`, shared crates as pinned
(`0984eed`, read in `~/.cargo/git/checkouts/markdown-shared-rs-82e4091941a49b93/0984eed`).
Nothing committed or pushed. Left in the worktree, uncommitted: the probes
(`crates/mdedit-core/tests/fable_probe.rs`, `fable_base.inc`,
`bins/mdedit/src/app/fable_probe_tests.rs`) and one `mod` line in `bins/mdedit/src/app.rs`.

`runs.rs`, the checker of `split.rs` and the plan card are no longer in this repository: they
moved to `mdshared-app` / `mdshared-ui`. I reviewed them there, because that is where audit 3's
fixes are.

## Summary

- Audit 3's fixes hold, with three exceptions: **C4, C6 and C9 are not fixed and no decision
  says they were left**, and **A8 is fixed for a kept plan but not for a tab that has not read
  the file again**.
- One finding can take text away from where the owner expects it (F1): a plan made from a stale
  tab writes notes of the old text and, with the box ticked, puts the newer document in the
  Recycle Bin. Proven. It can be got back from the bin.
- No crash found. No way found for undo to delete a note a line still lists, a file a run did
  not make, or anything outside the library and the group's declared home.
- 20 findings: data loss 1, crash 0, wrong behaviour 11, test gap 1, nit 7. Five are proven by
  probes; the rest are from reading.
- The notes pane, the post-1.3.0 fixes and Settings were read by two parallel passes of this
  review; they ran no probes, so their findings (F12 to F20) are from the code only.

## What was run

- Probes, all pass, each asserting the fault at a line marked `FAULT`:
  - `cargo test --release -p mdedit-core --test fable_probe probe_` : 4 passed.
  - `cargo test --release -p mdedit --bin mdedit fable_probe` : 1 passed.
- Existing suites of this slice, unchanged, all pass: `undo_rules` (27), `app::plan_write_tests`
  (25), `undo_rules_tests` (5), `undo_button_tests` (2), `moving_tests` (9), `recycle_tests`
  (11), `plan_tests` (7).
- Not run: the whole suite, fmt, clippy, the shots, the app itself, anything on Linux or macOS.
  **No fix was reverted to see its test fail**; "the test" in the table below means I read it
  and it forces the audit's situation.

---

## Findings

### F1. A plan made from a tab that has not re-read its file writes the old text and recycles the new document — data loss (recoverable from the bin), proven

- **Where.** `bins/mdedit/src/app/plan.rs:46-75` (`plan_open_active`): the notes come from
  `ready.buffer`, the length and SHA-256 from `std::fs::read(&path)`. Nothing compares the two.
  `plan_write.rs:432-441` then finds the disk equal to the digest that was taken from the disk.
- **Scenario.** Trip.md is open and unmodified. Another program (a sync client, another editor)
  saves a newer Trip.md. Before the tab has reloaded (the watcher has not been heard, the reload
  is still running, or the drive sends no events), "Split into notes…", tick the Recycle Bin
  box, Write. The notes hold the old text; the record holds the new file's digest; step 8 sees
  "unchanged" and bins the newer document, whose added text no note holds. Without the box:
  notes that are not the document, and undo allowed.
- **Proof.** `probe_a_plan_made_from_a_stale_tab_writes_the_old_text_and_recycles_the_new`: the
  note reads `alpha`, `three.md` does not exist, the bin holds the text with `ALPHA REWRITTEN`
  and `## Three`.
- **Also plausible, not run.** A tab marked "changed on disk" whose owner undoes back to the
  saved state has `modified() == false` and still holds the old text.
- **Fix.** In `plan_open_active`, refuse (or reload first) when `doc.is_loading()`,
  `doc.changed_on_disk`, or `stamp(path) != ready.info.stamp`. Better: take the digest from the
  bytes the tab was loaded from, and let the preflight compare the disk with that.

### F2. A note the Recycle Bin refuses at undo is left unlisted, and the run is marked undone — wrong behaviour (orphaned notes), proven

- **Where.** shared `runs.rs:3036-3041`: the state is `Undone` whenever no *link* step failed;
  `bin_failures`, and notes kept as changed in between, are not counted.
- **Scenario.** Undo a split while one note is open in another program, so the bin refuses it.
  Its line is taken out, the group's file is removed once it is down to its heading and `home:`,
  the note stays on disk, the record says "undone", and "Undo this run…" is refused from then
  on. The report's title is "Undid the run".
- **Proof.** `probe_a_bin_refusal_orphans_the_note_and_the_run_is_marked_undone`.
- **Fix.** Keep the record live while `bin_failures` is not empty. A second undo then works: the
  lines read as gone, nothing lists the note, and it is removed.

### F3. Audit C4 is not fixed: "links pending" still has no way forward — wrong behaviour, read

- **Where.** `bins/mdedit/src/overlays.rs:1599-1605` offers Finish for `Stopped | Writing` only.
  `plan_write.rs:183-186` still ends the toast "Try again / Undo", and
  `plan_write_tests.rs:179` asserts those words.
- **What it means.** A run whose library file could not be updated at step 6 (only a race now,
  after D-066) can only be undone. No DECISIONS entry and no line of STATE says C4 was left.
- **Fix.** Offer Finish for `LinksPending` of a split or Markdown run, after F4; reword the
  toast. A run left `links pending` by a failed *undo* must not be offered Finish.

### F4. Finish adds headings the record never learns of, so undo leaves them and the group's file — wrong behaviour, proven at the core

- **Where.** shared `runs.rs:3188-3197`: `finish` closes a clone of the record with only its
  state changed; `sections_added` stays as the failed write left it, empty.
- **Scenario.** A run stops before its links, Finish, later Undo: the notes and lines go, the
  `##` headings stay, `only_heading_and_home` is false, so the empty group's file stays too.
  Reachable today for a run left "writing"; on the main path once F3 is fixed.
- **Proof.** `probe_finish_then_undo_leaves_the_headings_and_the_groups_file`.
- **Fix.** `finish` works out the headings the file lacks, as `write_of` does
  (`runs.rs:1729-1743`), and records them when the gate succeeds.

### F5. Audit C6 is not fixed: undo takes out a heading the owner wrote under — wrong behaviour, proven

- **Where.** shared `runs.rs:2073-2112` (`with_headings_removed`): "nothing under it" means no
  note entry; the owner's own sub-heading or prose does not count.
- **Scenario.** After a split the owner adds `### My own plans` under the run's `## Tokyo`.
  Undo removes `## Tokyo`; the sub-heading is left under whatever section came before it.
- **Proof.** `probe_undo_takes_a_heading_the_owner_wrote_under`.
- **Fix.** Remove a heading only when the next non-blank line is a heading of the same or a
  higher level, or the end of the file.
- **Also from C6, read.** Undo still bins a note whose tab holds unsaved changes, because the
  disk matches; the text survives in the tab. The gate has `free` for a move and nothing for
  undo.

### F6. "Move the notes…" offers, ticked, to move another group's library file out of the library — wrong behaviour, plan proven, write read

- **Where.** shared `moves.rs:530-575` (`plan_group`): a listed path directly in the library
  folder is a `FileKind::Note` row, "not in the group's folder", ticked. `preflight`
  (`moves.rs:2350-2354`) checks only that the *destination* is not the library folder.
- **Scenario.** Group "Index" lists `[Recipes](Recipes.md)`, which is the group file of
  "Recipes". Move the notes of Index: `Recipes.md` goes to `library/Index/Recipes.md`, the group
  Recipes leaves the pane and every note it lists is unlisted. Undo puts it back.
- **Proof.** `probe_move_the_notes_offers_to_move_another_groups_library_file`: the row is ready
  and ticked. I did not run the write.
- **Fix.** In `plan_group`, a file directly in the library folder that reads as a group with
  entries of its own is blocked ("it is a library file"), or at least comes unticked. A flat
  note may still move.

### F7. `write_atomic` is now used on the owner's own files — wrong behaviour off Windows, read

- **Where.** shared `settings.rs:96-117`, called by `runs::rewrite_into` (`runs.rs:1178`),
  `undo_rewrite` (`runs.rs:3078`) and `moves::rewrite_note` (`moves.rs:1305`).
- **What goes wrong.** The file is replaced by a new one made with `File::create`. On Linux and
  macOS it gets the default mode: a rewritten `0600` note becomes readable by others, a script
  loses its executable bit, hard links are split. `<name>.tmp` beside the target is truncated if
  the owner has a file of that name and is left behind when the rename fails (audit C9, still
  there, and now in any folder a rewrite or a move touches).
- **Fix.** Give the temporary file the target's permissions before the rename; make it with
  `create_new` under a name that cannot be the owner's; remove it on failure.

### F8. A rewrite's record is not confined, and no decision says D-072 is replaced for it — wrong behaviour (design), read

- **Where.** shared `runs.rs:2740-2752`, `3056-3097`; the test
  `a_rewrite_record_is_not_confined_to_the_library_its_files_are_anywhere` asserts it.
- **What it means.** D-072: "A record must never give more power than write access to the
  library folder already gives." A `rewrite` record names any path, and undo writes the record's
  `text` there while the file's digest equals the record's own `written`. Whoever can write into
  a synced `.runs` and knows a file's exact bytes can have "Undo this run" replace it. The
  question says "Puts back the original of 1 file" and does not name the file.
- **Fix.** A DECISIONS entry that names D-072; name the files in the question.

### F9. A crash during a write leaves notes the record does not own — nit, read

`runs.rs:1520-1553`, `1706`: `created` is false for every note until the record is written
again after step 5; `plan_write.rs:155-163` sets `group.created` only after `runs::write`
returns. A process that dies in between leaves notes and a group file that undo will not remove;
Finish links them, and a later undo keeps them as "there before this run". The `.done` journal
has the truth and nothing reads it.

### F10. Tests that do not pin what they name — test gap

- `split_into_notes_needs_the_file_on_disk_to_be_what_is_in_the_tab` covers a modified tab only
  (F1).
- `undo_with_a_bin_hands_it_the_notes_and_keeps_one_it_refuses` (shared) and
  `a_note_the_bin_refuses_at_undo_is_kept_and_listed_apart` assert the note is kept and listed
  apart, not the record's state nor that anything still lists it (F2).
- `a_finished_run_is_undone_with_its_lines_and_its_notes` uses a note under no section, the one
  shape with no heading (F4).
- `a_home_line_that_names_the_library_folder_makes_no_note_there` covers `home: .` only (F14).

### F11. Smaller — nit

- `runs.rs:2331`: "their 1 lines" is still there.
- `runs.rs:1945`: `&& home != &req.library` still waives the group-file check.
- `runs.rs:2103-2107`: removing a heading rejoins the whole file with one line ending, so a
  file with mixed endings is normalised by undo.
- `docs.rs:3464`: `follow_move` changes the tab's path only; the session and the recent files
  still name the old one, which `Docs::rename` updates.

### From the pass over the fixes since v1.3.0 (read only)

**F12. "Find the file…" can rewrite the wrong note's link off Windows — wrong behaviour.**
`app/library_write.rs:1035-1074`: `ui.library.finding` is set before the `dialog_busy` test, and
`Picked::LibraryFile` carries only paths. With A's picker open (the window is live on Linux and
macOS), "Find the file…" on B sets `finding` to B and opens nothing; the file chosen in A's
picker is written as B's link. Fix: carry the target in the result, as `GroupHome` and
`NoteFile` do.

**F13. A held question can be asked of the new document on the frame of a tab switch — nit.**
`app/preview.rs:117-160`, `app/visual.rs:479-486`: `preview_sync` takes `v.pending` and asks it
after sending the new tab's text; `follow_tab`, which clears it, runs afterwards. A narrow
window; nothing is lost.

**F14 is below. F19. Drawing and comments — nit.** "Take this section out"
(`overlays.rs:1043-1047`) keeps the fixed 460x196 card D-103 found too small for the delete
question, and the grown delete card is still capped at the window's height. `link.rs:566-567`
and D-102 ("costs one lock") disagree with the code, which matches D-105.

### From the pass over the notes pane, the library and Settings (read only)

**F14. Audit A7's check is lexical — wrong behaviour, low.** `app/library_write.rs:576-582`,
`696-702`: `same_path(&home, &root)` does not fold `..`, links or junctions, so `home: Trip/..`
passes and "New note here…" writes into the library folder. Fix: compare canonical paths.

**F15. A finished library read rebuilds the rows under a held note — wrong behaviour.**
`app/commands.rs:2362` calls `library_read_take` directly; the drag guard is only in
`library_tick` (`app/library.rs:380`). The release then moves whatever note now has the row's
index. A line moves; no file is touched.

**F16. A row menu left open acts on whatever row now has its index — wrong behaviour.**
`app/library_write.rs:210`, `221`: `PopupKind::LibraryRow(i)` survives a re-read; "Remove from
the library" then removes another note's line with no question. Older than the diff; the
threaded read changes the timing.

**F17. A library file with a BOM — nit (audit C9, not fixed).** `app/library_write.rs:154`
reads with `read_to_string`; "Rename" and "Set the group's folder…" do nothing without a word,
and `library_new_note` drops the `home:` line it meant to write (`:743`).

**F18. "Edit a library file" can write the template over `notes.md` — nit.**
`app/library.rs:574-577`: `std::fs::write` when the folder read as empty; use `create_new`.

**F20. The crash folder for a bare or empty `MDEDIT_SETTINGS` — nit.** `app/crash.rs:28-33`
gives `crashes` in the working folder, where `snippets_path` refuses the same value.

---

## Audit 3, finding by finding

| | Now | The test |
|---|---|---|
| A1 | Fixed: Undo is button 0; a refused undo's Close only closes | `undo_button_tests.rs` clicks both drawn buttons |
| A2 | Fixed: `runs::with_removals` in both gates; `Wrote::Nothing` is an error | `undo_rules_tests.rs:62`, `:129`, through the window |
| A3 | Fixed: the source is compared always (D-071) | `undo_rules.rs:400`, `:421` |
| A4 | Fixed: `created` per note, picture and group; folders only those made (F9 is the crash window) | `undo_rules.rs:454-634` |
| A5 | Fixed: one rule, re-read after the links step; Finish writes the recorded line | `undo_rules.rs:231-336` |
| A6 | Fixed: the plan and "AI runs…" load the library; an empty root is no folder | `plan_write_tests.rs:940`, `:969` |
| A7 | Fixed where picked and for `home: .`; lexical (F14) | covers `home: .` only |
| A8 | Fixed for a kept plan and a lossy file; **not for a stale tab (F1)** | `plan_write_tests.rs:850`, `:900` |
| A9 | Fixed: `Reach`, no `..`, the home from the library file, extensions, regular files | `undo_rules.rs:698-977`; rewrite records are exempt (F8) |
| A10 | Fixed: prune counts only records that load; newer ones are listed | `undo_rules.rs:993`, `:1062` |
| A11 | Fixed: no session, no library | `plan_write_tests.rs:923` |
| B1 | Fixed in the shared crate, by bytes (`paths.rs:96-107`) | not looked for |
| B2 | Fixed: `read_regular` with a 64 MiB cap | `undo_rules.rs:877`, `:908` |
| C1 | Fixed (D-073) | `plan_write_tests.rs:624` |
| C2, C3 | Fixed: refused in `plan_preflight`; the group's file is made first | `:445`, `:546`, `:583`, `:692`, `:746` |
| C4 | **Not fixed (F3)** | the test asserts the old toast |
| C5, C7 | Fixed (D-068) | `:771`, `:811` |
| C6 | Bin refusals are listed apart; **the heading and the unsaved tab are not fixed (F5)** | none |
| C8 | Not looked at | |
| C9 | **Not fixed (F7, F17)** | none |

## Checked and found sound

- **The split's write** (`runs::write_of`, `plan_write.rs`): every note by `create_new`; a taken
  name is only read; the group's file is made first and taken away again only while it holds
  exactly what was put in it; a start error leaves nothing.
- **Undo of a split**: refused for a source gone, changed or too large, and for a library not
  read in full; a note goes only when the run created it, it is unchanged, it is a `.md` inside
  the reach, and no library file lists it after the links step; a failed link step keeps every
  note; folders go only when the run made them and they are real and empty.
- **Step 8** (`recycle_source`): only a whole, closed, done split whose source still has the
  recorded digest. F1 is upstream of it.
- **`recycle.rs`**: `Taken` is never the bin undo uses; a test window cannot reach the system's
  bin; the Windows refusal code slices only ASCII the system returned.
- **The move machine** (shared `moves.rs` 917-3149): renames never replace; a cross-volume copy
  is read back before the old copy goes, and that only after the library names the new place; a
  refused library file rolls everything back and what stays is recorded; undo confines every
  path, keeps files changed since, and refuses before the first file goes back. No slicing or
  indexing of outside data that can panic.
- **`moving.rs`**: the card stays with the reason when nothing moved; tabs follow moved and
  carried files; the demo refuses.
- **The record reader**: the new `originals` and `moved` parts have no indexing and no unwrap.
- **From the two parallel passes:** the closed-mdview courier (no idle timer, the chain stops at
  three rechecks, an input ask starts none); `?closed`; D-104's wait and its three tests; the
  delete card's measure; `dialog_busy` cleared on every path; D-067; the threaded library read;
  the delete guard; the pane's two-file move; the prefs round trip; `modal.rs` page index and
  clamps; `chrome.rs`; `args.rs`, `keys.rs`, `updater.rs`, `build.rs`, `closed_stderr.rs`,
  `version_resource.rs`.

## Not got to

- The shared plan card (`mdshared-ui/src/plan.rs`, 3,096 lines) beyond `retarget`; its drawing;
  `move_draw.rs`; `move_ui.rs` beyond its outline.
- `moves.rs` before line 917 other than `plan_group`, `plan_library` and `carry`;
  `moves_links.rs` (the link rewriting itself). `plan_library` has no caller in the app.
- `app/split_ai.rs`: only that its write goes through `plan_write`.
- The shared `library.rs` writers (`add_note_in_group`, `remove_note`, `set_note_path`): taken
  on their tests.
- The shared paged settings form, `html_md.rs`, `bins/mdedit/src/commands.rs`.
- Anything on Linux or macOS: F7 and F12 are from the code.
- Whether a library file with a BOM lets undo's plan and the window's gate disagree (the plan
  decodes, the gate reads the BOM as text). The check would refuse and keep the notes; not run.
- Audit C8 (the costs on the UI thread).
