# Review B: the AI flows, `git diff 6cdeab2 1d503d8`

Worktree `C:\Users\kaank\Desktop\md-wt\fab-b` at `1d503d8`, read-only review. Shared crates read at
`0984eed` (`C:\Users\kaank\Desktop\md-wt\fab-shared`). Nothing was committed or pushed; the
worktree is clean again. No request went to a real provider (the tests' stand-in engine only).

## Summary

The task window (phase W) is where the faults are. R2's core (`hunks.rs`) and phase 4's reader
(`import_ai.rs`) held up: I found no panic path on a model's answer in either, and no stale answer
that is read.

The two that matter most:

1. **A crash.** `task_ui::digest_of` cuts a `String` at byte 2048 with `truncate`, which panics
   inside a character. A text file that starts with about 1,000 non-Latin letters (Cyrillic, Greek,
   Chinese, Arabic) does it. The digest is built for the footer of "File them" whenever the AI is
   on, so adding such a file ends the program in a release build.
2. **An undo that is lost.** A rewrite run at the limits the window allows (20 files of 200 KB)
   writes a record larger than the shared reader's 4 MiB cap. The files are rewritten, the record
   is written, and "AI runs" then cannot read it: no undo. The originals are still in the JSON
   file, but the program will not put them back.

Then nine places where the task window or the sort card does the wrong thing, five of them proved:
Discard and Copy result ask for the same file again instead of the next; a kept review applied
after another window is open sends a file of that window to the provider unasked; the task can be
switched while a request is out, so a filing answer becomes a "rewrite"; a write that fails leaves
a `.tmp` file and an original in the record that was never overwritten; "Keep the folders" after a
sort does not give the folders back.

One test would pass with its fix reverted (the "Requests at once" cap), proved by taking the cap
out.

**Counts:** crash 1 · data loss 1 · wrong behaviour 11 (6 proved, 5 plausible) · test gap 1 ·
nits 4.

Probes: `C:\Users\kaank\AppData\Local\Temp\claude\ddd3b02074f601ee\e3635bc8-7486-4b64-a984-53cb1ce200e6\scratchpad\fable\b-probes.patch`
(nine tests appended to `app/task_tests.rs` and `app/import_ai_tests.rs`; `git apply` in a
worktree at `1d503d8`, then
`cargo test --release -p mdedit --bin mdedit probe_b -- --nocapture --test-threads=1`).
Each probe asserts the right behaviour, so it fails now and should pass once fixed. `probe_b8`
passes: it is a check that came out sound.

---

## Findings

### 1. Crash: a digest cut inside a character · `bins/mdedit/src/task_ui.rs:682`

`digest_of` ends with `out.truncate(2048)`. `String::truncate` panics when the length is not on a
character boundary. The sample is the first 1,200 *characters* (`task.rs:225`,
`all.chars().take(DIGEST_BYTES)`), so it is up to 4,800 bytes; with the name, size, date and
heading in front, byte 2048 falls inside a two- or three-byte letter about half or two thirds of
the time.

`digest_of` is called from `TaskScreen::summary` (`task_ui.rs:609`) for the footer of "File them"
with the AI on, and from `filing_prompt` (`task_ui.rs:737`) and so from "Show what is sent" and
Run.

**Scenario.** AI connection on. "AI task on your files…", add or drop a Russian, Greek or Chinese
note longer than a page. The window draws its footer and the release build aborts (rule 11).

**Severity:** crash. **Proved** (`probe_b1`): a file of `# T` and 1,300 `ж`:

```
panicked at bins\mdedit\src\task_ui.rs:682:9:
assertion failed: self.is_char_boundary(new_len)
```

The probe calls `digest_of` on the probed entry; that the footer and the prompt reach it is from
reading the two call sites.

**Fix.** Cut at a boundary: walk back from 2048 with `is_char_boundary`, or build the sample by
bytes the way `import_ai::one_line` does (`out.len() + c.len_utf8() > max`). The test
`the_digest_is_at_most_two_kilobytes_and_holds_the_heading` uses ASCII only; give it a non-Latin
text.

### 2. Data loss: a full rewrite run's record cannot be read back, so it cannot be undone

`mdshared-app/src/runs.rs:34` (`RECORD_MAX = 4 MiB`), `:1213` (`inspect` refuses a larger file),
`:663` (`esc`), against `task_ui.rs:237-239` (`REWRITE_MAX` 200,000 bytes, `REWRITE_MAX_FILES` 20).

A record holds each original as a JSON string. Twenty files of 200,000 bytes are 4,000,000 bytes
before escaping; every `\n`, `\r`, `"` and `\` becomes two bytes. The cap is 4,194,304. CRLF text
with lines under about 40 bytes, or any text with lines under about 20, goes over it.
`rewrite_into` never looks at the size: it writes the record and then the file, every time.
Afterwards `runs::list_all` puts the record under "unread" and `prepare`/`undo` are never offered.

**Scenario.** Drop a folder of twenty large Windows-ended notes on the window, Rewrite them, apply
each. All twenty files are overwritten. "AI runs…" shows no run to undo.

**Severity:** data loss (the originals exist only inside a file the program refuses to read;
D-074's "overturned by: an in-place rewrite losing text despite the review and the undo copy").
**Proved** (`probe_b3`, through `runs::start_rewrite` and `runs::rewrite_into` with 20 CRLF files
of 199,975 bytes):

```
record bytes 4325466, listed 0, unread 1
```

**Fix.** Make it impossible to write a record that cannot be read: in `rewrite_into`, build the
JSON first and refuse the file (before touching it) when the record would pass `RECORD_MAX`; the
window then closes that record as done and starts a second one for the remaining files. Or lift
the cap for `Kind::Rewrite` to what 20 × 200 KB can reach with every byte escaped (about 8.5 MiB,
more with control characters at six bytes each) and add a test at the limit. Either way a test
that fills a run to both limits and lists it.

### 3. Wrong behaviour: Discard and Copy result ask for the same file again

`bins/mdedit/src/app/task.rs:1315-1329` (`task_review_discard`), `ai_events.rs:1012-1016` (Copy on
a file review), `task.rs:1079-1090` (`task_rewrite_next`).

Both close the review and call `task_after_review`, which calls `task_rewrite_next`, which takes
the first file that is ticked and not done. Discard marks the file neither, so the next file is
the same one. DESIGN 83: "Discard leaves a file as it was and the run goes on with the next; Copy
result copies and goes on."

**Scenario.** Rewrite two files. Discard the first review. The window sends the first file again;
its review comes back; Discard again; again. The second file is never reached, and each Discard is
a paid request. Only Apply, or closing the window, gets out.

**Severity:** wrong behaviour (and requests the owner did not ask for). **Proved** for Discard
(`probe_b2`): after `task_review_discard`, `asking.index` is 0, not 1, and `task_sent.requests` is
2. Copy result is the same call, from reading.

**Fix.** In both places mark the file `ticked = false` (not `done`) before `task_after_review`,
as `task_after_answer_skipped` does. Add the test: two files, Discard, the request out is for the
second.

### 4. Wrong behaviour: a kept review, applied later, runs another window's files

`task.rs:1294-1302` (`task_review_apply`), `:1353-1359` (`task_after_review`), `:1574-1595`
(`task_close` leaves `ai.kept`).

Escape on a file's review keeps it (`ai.kept`), with `FileReview.index` pointing into the window
it came from. Closing the window does not drop it. If the window is opened again with other files
and the kept review is shown and applied, `files[index]` of the *new* window is marked done and
unticked, and `task_after_review` sees a window and calls `task_rewrite_next` on it, whatever its
task is and with whatever is in its instruction box.

**Scenario.** Rewrite `pasta.md`; Escape on its review; Escape closes the window. Later: "AI task
on your files…" (File them) with `b.md` and `c.md`. AI › Show the last result, Apply. `pasta.md`
is written (right). `b.md` shows as done though nothing ran on it, and **the whole text of `c.md`
is sent to the provider as a rewrite with an empty instruction**, from a "File them" window where
Run was never pressed. CLAUDE.md: "only what a command is run on leaves the machine."

**Severity:** wrong behaviour; the unasked request is the serious half. **Proved** (`probe_b4`):

```
b done true ticked false, asking Some((1, "\n\n<text>\n# C\n\nprivate c\n\n</text>")), requests 0 -> 1
```

**Fix.** Give each window an id and keep it in `FileReview`; `task_review_apply`, discard and Copy
touch `self.task` and go on to the next file only when the ids match, otherwise they finish the
record and stop. Or drop a kept file review in `task_close`.

### 5. Wrong behaviour: the task can be switched while a request is out

`task.rs:529-545` (`task_act`, `Hit::Kind`), `:1405-1417` (`task_done`).

`task_key` ignores keys while `asking` is set, but the pointer path does not: the WHAT TO DO
switch is drawn and takes a click. `task_done` then reads the answer by the task the window shows
*now*, not the one that asked.

- File them asked, Rewrite them clicked: the filing JSON is taken as the rewritten text of file 0
  and shown as "Rewrite pasta.md". The review protects the file, but Apply would write the JSON
  into the note.
- Rewrite asked, File them clicked: the rewritten text is read as a filing answer ("could not be
  read").
- Anything asked, Split them clicked: `TaskKind::Split => {}` leaves `asking` set, so the window
  says it is asking until Escape.

**Severity:** wrong behaviour. **Proved** for the first (`probe_b5`, clicking the switch through
`task_press`/`task_release` at its drawn place):

```
REVIEW of "Rewrite pasta.md": result = "{\"version\":1,\"files\":[{\"id\":0,\"title\":\"Pasta\", …
```

The other two are from reading.

**Fix.** Keep the task in `Asking` and dispatch on that; and refuse `Hit::Kind` (and
`Hit::Prompt`, `Hit::Dest…`) while `asking` is set, as the keys are.

### 6. Wrong behaviour: a write that fails leaves an original in the record, and a `.tmp` beside the file

`mdshared-app/src/runs.rs:1167-1179` (`rewrite_into`), `mdshared-app/src/settings.rs:96-108`
(`write_atomic_bytes`), `task.rs:1304-1310`, `:1362-1375`, `:1579-1587`.

`rewrite_into` pushes the `Original`, writes the record, then writes the file. When the file's
write fails (read-only, locked by another program, no room) it returns the error and leaves:

- the original in the record, in memory and on disk, with a `written` digest the file never had;
- `<file>.tmp` beside the owner's file (`write_atomic_bytes` does not remove it when the rename
  fails), holding the AI's text;
- `originals.len() == 1`, so `task_close` keeps the record instead of removing an empty one, the
  end-of-run toast counts it ("1 file rewritten"), and "AI runs" lists a rewrite of a file that
  was not rewritten. Its undo reports that file as "changed since the run, kept".

The same push stays in memory when the *record's* write fails ("so nothing was"), and then rides
along with the next file's record.

**Severity:** wrong behaviour (nothing of the owner's is lost; the record and the folder are left
untrue). **Proved** (`probe_b6`, the file made read-only, on Windows):

```
blocked Some("The file could not be written: Access is denied. (os error 5)"); file "# Pasta\n\nBoil it.\n"; tmp left true; originals in memory Some(1)
records listed after close: 1 (originals Some(1))
```

**Fix.** In `rewrite_into`, pop the original and write the record again when either write fails;
in `write_atomic_bytes`, remove the temporary file on any error after it was created.
`task_rewrite_finish` should remove an empty record as `task_close` does (it only sets `Done`).

### 7. Wrong behaviour: a window parked by "Show what is sent" comes back as "File them"

`task.rs:31-38` (`task_open`), `commands.rs:2158` and `:2167` (both pass `TaskKind::File`).

`task_open` sets `t.task = kind` on the parked window. The only two callers pass `File`, so a
Rewrite or Split window that showed its request text returns as File them, with the rewrite
instruction still in the box, the Rewrite prompts still listed (`prompts` is not refreshed) and
`planned` not reset (`set_task` is bypassed). Run then sends a filing request, not the one the tab
showed.

**Severity:** wrong behaviour. **Proved** (`probe_b7`): `left: File, right: Rewrite`. The existing
test parks a File window, so it cannot see this.

**Fix.** Keep the parked window's task when one is parked; `kind` is for a new window only.

### 8. Wrong behaviour: "Keep the folders" after a sort keeps the AI's sections

`bins/mdedit/src/import_ui.rs:518-522` (`sort_back`), `mdedit-core/src/import_ai.rs:641-713`
(`settle` rewrites `note.sections`, `ticked`, `unsorted` and the order in place).

`sort_back` only sets the stage to `Off`. The plan keeps whatever an earlier sort put on it, and
nothing holds the folder's own sections to go back to.

**Scenario.** Sort a folder; look at the result; Ask again; on the new scheme press Escape, "Keep
the folders". The card is the folder card again ("N of M ticked", Add) but every file is under the
first sort's headings, and the files that sort left in Unsorted are still unticked. The Failed
stage's line "the files would still be added under their folders" is untrue in the same way.

**Severity:** wrong behaviour (Add would write headings the owner just declined; visible in the
list before Add). **Proved** (`probe_b9`):

```
before … Bread\two.md ["Bread"] · Sauces\three.md ["Sauces"] · one.md []
after  … Bread\two.md ["Pasta", "Italian"] · Sauces\three.md ["Bread"] · one.md ["Pasta"]
```

**Fix.** Keep the folder plan's notes (sections, title, ticked, order) when the first sort settles,
and put them back in `sort_back`.

### 9. Test gap: the cap on requests at once is not tested

`bins/mdedit/src/app/import_ai_tests.rs:332` (`requests_go_out_three_at_once_and_one_for_a_local_model`),
and `:375`.

Ninety files are three batches, so three are in flight with or without a cap; the local-model
half has three files, one batch. **Proved by mutation:** with the check in `sort_next_prompt`
(`import_ai.rs:192-194`) deleted, all eight tests of `import_ai_tests` pass, and the compiler warns
that `SortRun::parallel` is never read.

**Fix.** At least 121 files (four batches) for the default, asserting 3 in flight and `next == 3`;
at least two batches for the local model; and one with the setting at 2.

### 10. Wrong behaviour (plausible): a kept file review cannot be reached from the window that waits for it

`task.rs:662-667`, `input.rs:687-694`, `:943-950`, `:1446-1452`; `task.rs:1332-1340`.

Escape on a file's review keeps it and "the run waits" (DESIGN 83). Run then toasts "A review is
kept from the last file. Show it from AI › Show the last result." But while the window is open it
takes every key and every press, so that command cannot be run; the owner has to close the
window, which ends the run. From there Apply still works (finding 4 aside), but **Run again**
(`task_review_again`) sets `ai.panel = None` and then `task_rewrite_begin` returns at once because
there is no window: the result is dropped without a word.

Not proved by a test (it is about what input can reach); from reading the routing.

**Fix.** Let Run on the window show the kept review itself; make Run again on a file review with
no window keep the review and say why.

### 11. Wrong behaviour (plausible): a split answer can land on the next file's plan

`bins/mdedit/src/app/plan_write.rs:79-172` (no `cancel_split`), `task.rs:1055-1076`,
`:1044-1049`, `split_ai.rs:277-295`.

`plan_close` cancels a split that is out; `plan_write` does not. In "Split them" with an empty
instruction box: on file A's card ask the AI through the card's own panel, then Write the plan as
it stands before the answer comes. `task_plan_done(true)` opens file B's card, and `split_start`
(which would cancel) is not called because the instruction is empty. A's answer arrives,
`ai.split.id` matches, and `split_show` cuts B's document with A's line numbers.
`answer::plan` checks the ranges against B, so nothing panics; the owner sees a plan for B that
was made for A, marked as the AI's.

Not proved. **Fix.** `cancel_split` in `plan_write` once the write is under way, as in
`plan_close`.

### 12. Wrong behaviour (plausible): a file edited after it was added can never be rewritten in that window

`task.rs:1092-1097` (`task_rewrite_begin` sends `f.text`), `:139-237` (the text is read once, at
add), `:1211-1222`.

The request is made from the text read when the file was added. If the file changes on disk
afterwards, every answer's review opens blocked ("changed"), and Run again sends the same old text
again. Nothing reads the file a second time, so each try is a request that cannot be applied, and
with finding 3 Discard sends another.

**Fix.** Read the file again in `task_rewrite_begin` (the same probe), and refuse before sending
when it no longer is text.

### 13. Wrong behaviour (plausible, small): "only 20 files a run" counts files that take no part

`task_ui.rs:576-588` (`recount_limit`).

The count takes every UTF-8 text file in order, ticked or not, "already in the library" or not. With 25 files, unticking the first twenty does not free a place: the
last five still read "only 20 files a run" and cannot be ticked (`task.rs:550` needs no reason).
Switching back to File them leaves them unticked.

**Fix.** Count only the files that are ticked and have no other reason; recount on a tick.

### 14. Wrong behaviour (small): Rewrite them runs with no instruction

`task.rs:652-678`, `:1096-1097`. `task_run` does not look at the instruction for Rewrite; the
request is `"\n\n<text>…</text>"` (seen in `probe_b4`'s output). The editor's own rewrite refuses
an empty box (`ai_events.rs:913-917`). **Fix.** Refuse with the footer's sentence.

### Nits

- `import_ai.rs:526`, `:546` drop a twice-named section with `eq_ignore_ascii_case`;
  `Scheme::problem` (`:207`) compares with `to_lowercase`. A scheme with `Çay` and `çay` is read,
  then refused at "Sort N files" until the owner renames one. Use one rule in both.
- `hunks.rs:254-256`: in a subset, a deletion at the end of a text that has no final break leaves
  one (`"x\na\nb"` → `"X\na"`, only the deletion ticked, composes `"x\na\n"`). Neither text ends
  so. `fit_end` makes the two texts end alike, so it shows only here.
- `settings.rs:101-108`: the temporary name is `<file>.tmp`, created with `File::create`: an
  existing `notes.md.tmp` of the owner's is overwritten and then renamed away. On Linux and macOS
  the rename also gives the rewritten file the default mode (a `0600` note becomes `0644`) and
  replaces a read-only file. Not run on those systems.
- `prefs.rs:168-179`: "Requests at once" is kept per connection, not per preset, so a 5 set for a
  hosted OpenAI-compatible service still applies after the preset is changed to a local model,
  where the default would have been 1.

---

## Checked and found sound

- **Request ids and stale answers** (`ai.rs`, `ai_events.rs:472-641`). Ids never repeat, across
  engine restarts too. A cancelled request is taken out of `pending`, and every reader also checks
  its own id (`task.asking.request`, `split.id`, `run.id`, `sort.in_flight`), so a late `done` or
  `error` for a cancelled or replaced request is dropped. `Ended` clears `pending` and reaches the
  task, split, sort, note and run. The one hole is finding 5 (right id, wrong task).
- **Several requests at once** (`app/import_ai.rs`). Answers are kept by batch and settled once,
  in file order; an unreadable or failed batch stops new sends and lets the rest finish; Stop and
  closing the card cancel every id. The plan's notes are not reordered while a run is out, so the
  batches' indexes stay true until `settle`.
- **A hostile answer, phase 4** (`import_ai.rs:490-1105`). The JSON reader is depth-bounded (32),
  every index is `get`, numbers are checked for finite, whole and in range, names and reasons are
  cut at characters. No path, no file name and no text of the model's reaches the disk: only
  heading names and titles, through 4a's writer.
- **A hostile answer, File them** (`task_ui.rs:773-832`). `serde_json` (its own depth limit), ids
  checked against the batch, first answer wins, `with` must be a text file of the batch, sections
  through `folder_name`, each link read back before the next (`task.rs:905-940`).
- **R2's core** (`hunks.rs`). `compose` takes every index from the comparison; all ticked is the
  result and none the original, both returned as they are; a subset is whole lines. `intra_line`
  cuts at characters and graphemes, and `hunks_ui::draw` clamps its slices to the row.
  `wrap` always advances by at least one character.
- **The rewrite's guard and undo** (`task.rs:1211-1312`, `runs.rs:1143-1181`, `:2740-2750`,
  `:3065-3100`). The record is written before the file; the file is written only while it holds
  the answer's source to the byte and the byte order mark, checked in the window and again in
  `rewrite_into`; undo puts a file back only while it holds what the run wrote, and reads it again
  just before. **Proved** (`probe_b8`, passes): a CRLF file with a byte order mark, a mixed ending
  and no final break is rewritten, keeps its mark, and `runs::undo` restores it to the byte.
- **Prefs** (`prefs.rs`): `ai_parallel` parses, clamps 1..8, falls back to the default on anything
  else, and round-trips.

## Not got to

- `ai_ui.rs` and `task_draw.rs` (drawing), the AI rows of `app/modal.rs` beyond the "Requests at
  once" row, and the diff of `crates/mdedit-core/src/ai.rs` (saved prompts, `kind: file`).
- `app/review_tests.rs` and `app/task_tests.rs` were not checked test by test for "passes with the
  fix reverted"; only the parallel test was mutated. `a_file_changed_since_the_answer…` would
  still pass with the window's own check removed, because `rewrite_into` refuses too; that is two
  guards, not a gap.
- The R2 window's Apply on an open document (`ai_events.rs:1025-1064`) was read, not probed.
- `mdshared-app`'s rewrite tests, the `mdedit-ai` engine, the note card (5.8) and the split
  answer reader (`split::answer`) were out of what I read.
- Not run: the full test suite (only the nine probes and `import_ai_tests`), `cargo fmt`,
  `clippy`, the shots, the budgets. Everything ran on Windows only; findings about Linux and macOS
  file modes are from reading.
