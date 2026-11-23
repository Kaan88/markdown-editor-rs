# Review D: markdown-shared-rs, c603b23..0984eed

Read-only. Worktree `C:\Users\kaank\Desktop\md-wt\fab-shared` is clean again (`git status` empty); probes were removed.

## Summary

Three proved data-loss bugs, all in code that writes the owner's files:

1. A rewrite (and a moved note's link rewrite, and `put_note`) silently destroys a file of the owner's named `<file>.tmp` beside the one written.
2. A rewrite run whose originals pass 4 MiB writes a record that no longer loads, so Undo is gone for every file of that run.
3. `put_note` rewrites a library file that is not UTF-8 with every non-ASCII letter replaced by U+FFFD.

No panic path was found in the record parser, the moves machine, html_md, split, cf_html or the plan card. The moves machine (rename, cross-volume copy, rollback, undo) read as sound. Two more proved wrong behaviours in undo (a file rewritten twice is half undone; a run whose group file was never made can be neither undone nor finished).

Counts: data loss 3 proved + 1 unverified; crash 0; wrong behaviour 13; test gap 3; nit 8.

Parts were reviewed by two delegated reviewers (html_md/split, and mdshared-ui); their findings are marked [delegated] and I did not re-run their probes.

## Findings: data loss

### D1. `write_atomic_bytes` takes the owner's `<name>.tmp` — proved
- Where: `crates/mdshared-app/src/settings.rs:96-108`, now reached from `runs.rs:1178` (`rewrite_into`), `runs.rs:3078` (`undo_rewrite`), `moves.rs:1308` (`rewrite_note`), `library/put.rs:141` (`put_note`).
- The temporary is `<path>.tmp`, opened with `File::create` (truncates), then renamed onto the target. This was written for the app's own settings files. It is now used on any text file the owner rewrites and on notes.
- Scenario: a folder holds `notes.txt` and `notes.txt.tmp`. The owner runs an AI rewrite on `notes.txt`.
- Probe output: `P2 rewrite_into = Ok(()); notes.txt.tmp exists = false`. The file is gone, not in any record, and Undo does not bring it back (`P2 after undo: restored = 1; notes.txt.tmp exists = false`).
- Fix: use the temporary of `files::create_new` (`create_temporary`: a name that did not exist, tried in turn), then rename over the target; remove the temporary on every failure.

### D2. A rewrite record over 4 MiB cannot be read back — proved
- Where: `runs.rs:34` (`RECORD_MAX`), `runs.rs:1211-1214` (`inspect`), `runs.rs:1143-1181` (`rewrite_into` has no size check). The record holds each original's whole text; a file may be up to `DIGEST_MAX_BYTES` (64 MiB).
- Scenario: one rewrite task over several files (mdedit keeps one record per task, `task.rs:1247-1272`), originals adding up to more than 4 MiB once JSON-escaped; or one file over 4 MiB.
- Probe output: after the fourth 1 MiB file, `record on disk = 4481568 bytes, loads = false`; at the end `list_all: 0 runs, 1 unread (newer flags: [false])`, and `f0.txt now holds: "gone\n"`. Every `rewrite_into` returned `Ok`, and mdedit's toast says "Its original is in AI runs, where Undo brings it back."
- The originals are still in the JSON on disk, so a hand recovery is possible; the program offers none.
- Fix: refuse in `rewrite_into` when the record would pass the cap (before the file is written), and have the caller start a new record; or lift the cap for kind `rewrite`.

### D3. `put_note` rewrites a non-UTF-8 library file lossily — proved
- Where: `library/put.rs:122-141`. `files::decode` returns `(text, invalid)`; the flag is dropped, and the lossy text is written back whole. The read-back check compares the lossy text with itself, so it passes.
- Scenario: a library file saved as Windows-1254 with `- [Çağrı](<cagri.md>)`; mdview adds a note (`fab-mv/bins/mdv-app/src/notes.rs` calls it with no encoding check).
- Probe output: `put_note = Ok("New"); old title bytes still there = false; U+FFFD bytes in file = 3`, file now `- [�a�r�](<cagri.md>)`.
- A UTF-16 file is also re-saved as UTF-8 with no BOM (not lossy, but not asked for). `moves::read_library` already refuses a non-UTF-8 file (D-084); this path does not.
- Fix: return `PutError` when `decode` reports replaced input; keep the BOM/encoding or refuse UTF-16 too.

### D4. Windows: a file larger than the Recycle Bin's capacity — not verified [delegated]
- Where: `crates/mdshared-ui/src/platform/recycle.rs:164-231`, `:68`. The `trash` crate sets `FOF_NO_UI | FOF_ALLOWUNDO | FOF_WANTNUKEWARNING`; the only check afterwards is that the file is no longer at its path, which a permanent delete also satisfies.
- Not tested (needs the bin's quota changed). Fix: read `MaxCapacity` from the `BitBucket\Volume\{guid}` key beside `NukeOnDelete` and refuse a larger file.

## Findings: wrong behaviour

### W1. One file rewritten twice in a record is half undone, and the record says undone — proved
- Where: `runs.rs:2741-2749` and `runs.rs:3065-3090`.
- Probe output: after `zero → one → two`, undo leaves `file = "one\n", state = Undone`, and a second undo is refused with "This run is undone already."
- Reach: mdedit marks each task file `done`, so it probably rewrites each file once per record; I did not prove it cannot happen.
- Fix: undo the originals of one path newest first, in a loop, or refuse a second `rewrite_into` of the same path in one record.

### W2. A run whose group file was not made can be neither undone nor finished — proved
- Where: `runs.rs:2525-2571` (`Reach::of` reads the home from the group's file as it is now), `runs.rs:1826-1842` (`create_group`).
- Scenario: a new group with its folder outside the library; the group file is not created (512 library files, a disk error, or a crash between steps 5 and 6). The notes are on disk in the home.
- Probe output: `write: notes written 1, group failure Some("The library already has 512 files."), state Some(LinksPending)`; undo and finish both answer "…\Notes\Trip is outside the library and the group's folder, so this run is not undone."
- Nothing is lost, but the record is stuck and the sentence is wrong about why.
- Fix: when the group's file does not exist and the record says the run was to create it, either let Finish create it first, or say that plainly.

### W3. `write_atomic_bytes` on the owner's files drops what a rename-over drops — plausible, not run
- Where: same as D1. On Linux and macOS the new file has the default mode, so a rewritten script loses its executable bit and a 0600 file becomes 0644; hard links are broken; on Windows, alternate streams and a file's own ACL go.
- A failed rename (target open in another program on Windows) leaves `<file>.tmp` behind: `settings.rs:108` returns without removing it.
- Fix: copy the permissions of the target onto the temporary before the rename; remove the temporary on error.

### W4. Undo marks the run undone when the bin refused a note — by reading
- Where: `runs.rs:3036-3041`. The state looks only at link failures. A note the bin would not take (open in another program on Windows) has had its line removed, the group file may be removed, and the record becomes `Undone`, so undo cannot be pressed again. The note stays on disk, unlisted; it is reported in `bin_failures`.
- Fix: keep the state live while `bin_failures` is not empty.

### W5. A crash during steps 2 to 5 of a write leaves files the record does not own — by reading
- Where: `runs.rs:1597-1706`. The record is written at step 1 with `created: false` everywhere and not again until after step 5. Pictures and notes written before a crash are on disk, and undo then treats them as files that were there before (never removed). The `.done` journal names them, but nothing reads it.
- The module comment says every prefix can be lived with; it can, but not undone. Fix: close the record after the pictures and after every N notes, or mark `created` before writing and let undo's digest check decide.

### W6. Headings a run wrote are not recorded when the links are written late — by reading
- Where: `runs.rs:1744-1747` sets `sections_added` only when `add_links` succeeds in the same call; `finish` (`runs.rs:3131-3199`) adds the links later and never sets it. Undo then leaves the empty headings, and so keeps a group file it made (`only_heading_and_home` is false).

### W7. Pasted HTML: escaping looks inside one text token only — proved [delegated]
- Where: `html_md.rs:534-545`. `&lt;<span>script&gt;alert(1)</span>` becomes `<script>alert(1)`; `&amp;<span>copy;</span>` becomes `&copy;`. Syntax-highlighted markup outside `<pre>` has this shape.
- Fix: decide at the next text token, or re-check the finished line in `block()`.

### W8. Pasted HTML: an image alt ending in a backslash breaks the image — proved [delegated]
- Where: `html_md.rs:933-952`. `alt="back\"` gives `![back\](a.png)`.

### W9. `split`: `compose` promotes any ATX-looking line, `bytes_of` counts only the caller's headings — proved [delegated]
- Where: `split.rs:1247` vs `split.rs:1290-1299`. With `## not a heading` inside `<pre>` and headings passed in that do not include it, the note loses a `#` on that line and `check` says "note 0 counts 34 bytes and writes 33".
- Not verified whether mdedit's outline ever disagrees with `atx()`.

### W10. Plan card: the rename box takes clicks meant for "Tick all" — proved [delegated]
- Where: `plan/draw.rs:543-560`, `:1003`; `plan.rs:1332`. The field's hit spot is not clipped to the list.

### W11. Linux: a trashed file's `.trashinfo` has no `DeletionDate` — by reading [delegated]
- Where: `crates/mdshared-ui/Cargo.toml:16` leaves out the `trash` crate's `jiff` feature; the spec requires the key. Not run on Linux.

### W12. macOS: the bin is reached by `osascript` telling Finder — plausible [delegated]
- Where: `platform/recycle.rs:63` (crate default `DeleteMethod::Finder`). Needs the Automation permission; a refusal makes every recycle an `Err`. Nothing is lost.

### W13. Rule 8: `overlay::draw_dialog_button_disabled` was changed in place [delegated]
- Where: `overlay.rs:220-229`. Corners 5 → 6, and a primary lost its tinted fill. One existing caller, `fab-a/bins/mdedit/src/ai_ui.rs:586`. The old test was inverted (`assert_ne` → `assert_eq`). No before/after shot was taken.

## Findings: test gaps

- T1. Nothing tests `write_atomic_bytes` against a neighbour named `<file>.tmp`, a record past `RECORD_MAX`, or `put_note` on a non-UTF-8 file (D1 to D3 all pass the suite).
- T2. `html_md` escaping tests use single text tokens only [delegated].
- T3. `split`'s test helper `headings_of` (`split.rs:1737`) copies the module's own rules, so no test passes headings that disagree with them [delegated].

## Nits

- N1. `runs.rs:1945`: `if !inside_any(&req.group.file, req) && home != &req.library` lets a group file anywhere through when the home is the library folder.
- N2. `paths.rs` `strip_verbatim` keeps the `\\?\` form for a path of 260 or more, so `runs::inside` reads an existing long path as outside the library. Fails safe (refused).
- N3. `html_md.rs:1136-1141`: only `javascript:` links are dropped; `vbscript:` and `data:` links are kept [delegated].
- N4. `html_md.rs:540` `<?` not escaped; `:708` marks inside `<code>`; `answer.rs:392-396` wrong reason for a dropped note; `split.rs:645` file name raw inside an HTML comment; `split.rs:83` `n - 1`; `split.rs:778-790` a `..` section kept [delegated].
- N5. Plan card: a rename scrolled out of view goes on unseen (`plan.rs:1544`) [delegated].
- N6. Plan card: typed text is dropped when the key has no name (`plan.rs:1786-1797`) [delegated].
- N7. `sister::Request` gained the public field `closed`; both apps build with `..`, none is affected [delegated].
- N8. `rewrite_into` leaves the pushed original in the in-memory record when the record or the file could not be written (`runs.rs:1167-1179`); harmless at undo (the digest equals the original's).

## Checked and found sound

- `runs` JSON reader: depth-bounded, every index through `get`, surrogates and bad UTF-8 handled, numbers clamped; `from_json` for all four kinds; a newer version, unknown kind or state is listed and never deleted (`prune` skips what does not load).
- `stored`/`resolved` on both platforms; `Reach` (no `..`, library folder or the home the group file declares now, extension lists); `canonical_or_absolute` for a path that does not exist yet.
- Split/Markdown write order: nothing is overwritten (`create_new` everywhere), an existing file with the same bytes is never marked as the run's, folders recorded only when made.
- Split undo: source digest check first, links before files, library re-read before each removal, digest re-check at removal, a held or edited line keeps its note, folders removed only when empty and real.
- Rewrite: refused when the file changed since the review, BOM kept, record written before the file, undo only while the file holds what the run wrote.
- Moves: preflight (digests, taken names, path length, kinds, name never changes), record before the first rename, cross-volume copy read back and the old copy removed only after the lines are written, rollback of lines then files, undo by what is on disk, carried files only with their note, `reach_of` on a hostile record, `with_edits`/`apply_edits`/`plan_edits`, `moves_links::rewrite` (no slicing off a boundary).
- `library/put.rs` `dir_for`, `pref_in`, `dir_from_settings`: no panic path.
- [delegated] html_md and split: no panic; 6,000-document fuzz of line accounting; hostile HTML timings. Plan card at 13 window sizes, stale indexes, `menu.rs`, `cf_html.rs`, `platform.rs`, `recycle.rs` refusals, `settings/pages.rs` indexes.

## Not done

- `moves_tests.rs` and the tests in `runs.rs` were not checked one by one for "fails with its fix reverted"; `library.rs`'s diff (the `place_in_group` writers) and `library/tree.rs` were only skimmed.
- The window's rename (`rename_no_replace` in mdedit) and the apps' gates are outside this diff and were not read.
- Nothing was run on Linux or macOS: W3, W11 and W12 are from reading. D4 was not tested. The real Recycle Bin test was not run.
- No `tests/shots.sh` run, so W13 has no picture.
- The full test suites were not run; only the probes.
