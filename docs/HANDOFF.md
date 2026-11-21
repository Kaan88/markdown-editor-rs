# Handoff: mdedit and the shared crates

Written 2026-11-13, updated the same day after phases 2 and 3; sections 4 to 7 had a truth pass for
the release of 0.5.0 on 2026-11-14 (see "Release 0.5.0" below section 7). Read this first, then
`docs/PLAN.md` (goals, budgets, phases, feature roadmap) and `docs/DESIGN.md` (the design of
phase 1, and section 16 on editing and saving as built).

## Open: Save offers .txt for a note converted to Markdown (2026-11-21, owner)

A new note converted to Markdown is still offered `<name>.txt` by Save: `save_as`
(`app/commands.rs`) always names a new document `.txt`. It should take the extension from the
document's language (`.md` once it is Markdown). Not fixed yet.

## Release 1.2.1 (2026-11-21, released)

mdedit 1.2.1 is released: tag `v1.2.1` at `528446a`, release run 44850785598 green, manifests,
hashes and `--check-update` verified (`docs/RELEASE.md`). 1.2.0 stays published.

## Sharing with mdview (2026-11-21, branches, not merged)

The owner: the two apps are sisters, more apps may join; what they can share goes in the shared
crates. Moved to markdown-shared-rs (`mdshared-app`, at `cb0ffd3`): `brief` (the focus clock, the
rule and the `focus` lines), `folder_lock` (was mdedit's `instance.rs`), and `settings::read` /
`read_patiently` (an unreadable file is an error, not an empty one). mdedit uses them on
`f/shared-brief`; mdview has the brief setting on its `f/brief-tabs`, with Settings in
categories (as mdedit's, the card at most 640 px tall). See mdview's HANDOFF for its side.

## Tabs barely looked at, and the AI menu's Modify room (2026-11-21, released in 1.2.1)

The owner: "an option that does not reopen files that were open less n second ... this will only
count in focus time. so minimizing or switching tabs won't count."
- `Doc::focus_ms` (`Option`: `None` for a file the session has no count for, so a tab from before
  the count is never left out), `Docs::focus_tick` (called at every turn of the loop in
  `about_to_wait` with the active tab while `App::focused`; Windows sends `Focused(false)` on
  minimize), `focus_flush` before each session write, `start_focus`, `focus()`. Session
  `focus=<seconds> <encoded path>` per file tab.
- `Prefs::skip_brief` (off) and `brief_secs` (1 to 600, default 10); Settings > Session, "Don't
  reopen tabs barely looked at" / "Looked at for less than … seconds". `barely_seen` (`app.rs`)
  leaves a session tab out at start unless it was showing (`active`, `active_tab`), is pinned, has
  an unsaved entry, or has no count.
- Tests `tabs_barely_looked_at_are_left_out_at_start_when_asked`,
  `focused_time_counts_only_the_tab_in_front_of_a_focused_window`,
  `the_session_keeps_each_file_tab_s_focused_time`, prefs parse/round trip. The first and third
  fail with the pinned exemption or the session write removed.
- AI menu: a long saved prompt's name ran under its Modify button: the menu elides a label at
  its blank hint (`MENU_MODIFY_ROOM`, 74 px), narrower than the 88 px button. Widening the room
  widened the menu, which the owner did not like; the menu's button is now narrower instead
  (`MENU_MODIFY_W`, 64 px, pencil and word closer: `draw_modify(.., compact)`), the menu as wide
  as before. The rewrite panel's button is unchanged. Test
  `a_saved_prompt_s_name_stops_before_its_modify_button` (the room, the button, the word).
- Checks (Windows): fmt, clippy both ways, all tests (mdedit 412, core 514). Budgets, 15 runs of
  `sample.rs`, published 1.2.0 against this branch: first frame 95.1 / 97.1 ms, memory 27.8 / 27.6
  MiB, keystroke p50 0.55 / 0.51 ms, scroll p50 0.46 / 0.41 ms: within noise.
- Not done: the owner has not seen either; nothing run by hand.

## Release 1.2.0 (2026-11-21, released)

mdedit 1.2.0 is released: tag `v1.2.0` at `bd9cac3`, release run 44840555985 green, manifests and
downloads verified (`docs/RELEASE.md`). Open: the visual box not following the wheel as the owner
sees it (not reproduced). Next review round: `docs/review-prompts/audit3-scope.md`.

## Reviews: who audited what (2026-11-21)

- Audit 1, rounds 1 to 7 (2026-11-19 to 11-20, up to `09c5f93`, released as 1.1.2): all by Astra
  (GPT), since the owner's Fable limits were spent. No Fable audit of that code yet.
- Not audited by anyone yet: everything after round 7. That is visual editing (`f/visual-edit`
  here, with mdview `f/block-edit` `2fbb08c` and `5be9ca7`) and `f/four-fixes` (worktree
  `..\mdedit-four-fixes`: prompt Modify, About OK, cleanup days, the caret kept per file,
  custom AI providers without a key and several of them, the language switch, the .md rename offer).
- 2026-11-21: three Fable reviews asked for: (1) visual editing, (2) `f/four-fixes`, (3) a second
  opinion on Astra's audit 1 areas (recovery and session, AI keys, AI apply/review, the updater).
  Their findings and fixes go in a section "Audit 2" here.
- Audit 2's findings were all fixed for 1.2.0 (below). The fixes themselves have not been reviewed
  by Fable or anyone else; each has a test that fails with its fix reverted, except where noted.
- Seen while preparing 1.2.0: a 1.2.0 launch (with no file) next to the owner's running 1.1.2
  window did not hand over: the forward got no answer, and 1.1.2 holds no folder lock, so the
  new window ran on the same session, took `instance.port`, rewrote `settings.ini` and renamed
  the 1.1.2 window's recovery copies to its own. It was stopped by force, so it wrote nothing at
  exit; nothing was lost (the 1.1.2 window still had the text, the copies and backups stayed).
  Why that forward got no answer is not known: with scratch settings a 1.1.2 or 1.2.0 window
  took every launch (files, none, an unknown flag) in 15 to 21 ms. The lock protects only once
  every window is 1.2.0 or later.

## Audit 2 (Fable, 2026-11-21): findings, and how each was fixed

Read-only reviews; the findings are from reading code unless marked. Spot-checked against the code by
the session that asked for them (marked "verified").

**A. Second opinion on audit 1 (`main` @ `75afd67`, the 1.1.2 code).** Keys, Apply/review, SSE and
the updater held up. Drafts can still be lost:
1. Two windows on one session folder (launches close together, e.g. Explorer multi-open: no lock
   before the single-instance listener starts, `app.rs` ~3145/3389). Each exit drops the other's
   backups (`drop_stale_backups`), `Recovery::finish` clears all of `recover/`, both write
   `session.ini.tmp`. Reproduced with two real processes and a probe test. Major.
2. A session file that cannot be read at start (`Session::load`, `session.rs` ~318: any read error
   gives an empty session) leads the next exit to write the empty session and delete every kept
   draft. Probe test; verified. Also `settings::write_atomic` (shared) has no fsync before rename.
   Major.
3. "Don't save" then Cancel in a Close All / Close Others run: the first tab's recovery copy and
   session entry go while the next question is open (`commands.rs` ~1026/1044). Probe test. Minor.
4. `--exit-after-ms`, `--type-steps`, `--scroll-steps` exit without `keep_unsaved`. Minor (dev flags).
5. Tests that pass with their fix reverted: `finish(.., saved)` in the exit path (`app.rs` ~2539),
   the waiting-tab skip in `Recovery::poll` (`recovery.rs` ~447), every `Fail::hide(key)` (only the
   `sk-ant-` prefix fallback is tested), the `sse` `Watched` closed check.
6. Smaller: duplicate drafts after a crash between `keep_unsaved` and `finish`; an updater repair
   overlapping an install (`missing_parts` checks existence, not version; staged files not
   fsynced); `host_of` shows `user:pass@` from a base URL in errors.
Probe tests: `docs/review-prompts/audit2-probe-tests.patch`.

**A fixed** (branch `release/1.2.0`). Each test fails with its fix reverted, except where noted.
1. One window per settings folder: `instance::claim` (new `instance.rs`) takes a lock on
   `settings.ini.lock` (std `File::try_lock`), held while the window runs. A launch that cannot
   take it forwards its files to the window that has it, retrying every 50 ms for up to 10 s
   while that window starts listening; failing that it runs without the session (toast
   `SESSION_BUSY`), so it cannot drop the other's drafts. A running window is tried first, so a
   window from before the lock still gets the files. Tests `instance::tests::*` (two). The real
   two-process check is in the release checks below.
2. `Session::read` is an error for a file that exists but cannot be read; `read_patiently` tries
   again for 2 s; on failure the window runs without the session and says so
   (`session_unreadable`). `Session::load` is gone. Test
   `a_session_that_cannot_be_read_is_an_error_not_an_empty_session`. Shared `write_atomic` now
   syncs the file before the rename (and the folder on unix), shared `75ddc1a`; no test (a power
   loss).
3. "Don't save" counts only once the run ends: `Doc::modified()` is `!discard && has_changes()`,
   and the recovery copy, the session entry and `check_changed` use `has_changes()`. Cancel
   later brings the tab back as it was. Test
   `dont_save_then_cancel_keeps_the_first_tabs_copy_and_backup_meanwhile`.
4. The dev-flag exits go through `timed_exit()`, which keeps the unsaved text as a normal close
   does. Test `a_timed_exit_keeps_the_unsaved_text`.
5. New tests: `an_exit_whose_session_write_fails_keeps_the_recovery_copy` (`finish_session`),
   `the_recovery_look_leaves_a_waiting_tabs_copy_alone`, `a_fail_hides_its_own_key` (proto
   unit) and `an_opaque_key_echoed_in_a_streamed_error_is_hidden` (engine). The engine test
   still passes with `Fail::hide` removed, because `proto::redact` also knows the key; it fails
   only with both removed: the two are each other's backup. The `sse` `Watched` checks are
   likewise redundant with each other; the existing test fails only with both removed.
6. - A copy older than its backup with the same text covers it (`cover`'s `same_text`, read only
     for that case): a crash after the exit wrote the backups no longer opens the draft twice.
     Tests `a_copy_covers_the_backup_of_its_draft` (unit) and
     `a_crash_after_the_backups_were_written_brings_the_text_back_once` (end to end).
   - `finish` lets go of a waiting tab's copy when the user answered "Don't save" on it.
     Test `an_exit_before_the_text_was_put_back_keeps_its_copy` (extended).
   - Updater: a static `FILES` mutex in mdedit's `updater.rs`. A repair takes it with
     `try_lock` and does nothing while an update holds it; mdedit's own `spawn_install` waits
     for it, then runs shared `install` and `record_version`. Test
     `a_repair_leaves_the_files_alone_while_an_update_writes_them`. Shared staged files are now
     synced before the swap (`write_synced`, shared `8b73e01`), no test. `missing_parts` still
     checks existence only; with the overlap gone, the case Fable described (an old file
     swapped over a new one) cannot arise.
   - `host_of` drops `user:pass@`. Test `an_address_s_user_and_password_are_not_shown`.

**Release checks for 1.2.0** (Windows, this PC, merged head `37e1dd9` plus the version bump):
fmt, clippy (default and `--no-default-features`, `-D warnings`) and `cargo test --workspace`
clean (mdedit 407, core 514, mdedit-ai 44 + 12 engine). Budgets, `tools/compare-builds.py`, 15
interleaved runs of `sample.rs`, 1.1.2 against 1.2.0: first frame 109.8 / 112.2 ms (both over
the 100 ms budget today; earlier releases measured 85 to 95 ms here, so the PC is slower now),
private memory 27.6 / 27.7 MiB, keystroke p50 0.55 / 0.58 ms, scroll p50 0.44 / 0.48 ms: within
noise. Four launches at once on one scratch session, five tries: 1.2.0 leaves one window each
time (the others hand over, exit 0); 1.1.2 leaves two. Not checked: that the one window has all
four files.

**After the test build (owner, 2026-11-21):** a right-click in the visual box opens the text's
menu, not the page's (`right_press`; test `the_open_block_s_box_moves_with_the_page_it_is_on`,
fails with the fix reverted); the AI submenu's Modify button lights up at once (its hover is in
the overlay signature; no test). Not fixed: the owner sees the box stay put while the wheel
scrolls the page; the same test has the box follow the page, so the cause is not found. Next
review round: `docs/review-prompts/audit3-scope.md`.

**B fixed** on `f/visual-edit` (`2496e6d`, merged into `release/1.2.0`); see "Visual editing, B+"
below. B2 (clipping) and B3 (scroll to the caret) have no automated test and wait for the owner
to look. **C fixed** on `f/four-fixes` (`b06800a`, the four-fixes session); each fix has a test
that fails with its fix reverted.

**B. Visual editing (`f/visual-edit` + mdview `f/block-edit`).** No edit loss found.
1. The page jumps to the caret line of the Ctrl+E moment on each text or view change: the server
   keeps the last `Follow` and mdedit stops sending it (`server.rs` `go_to`). Verified. Major.
2. The box's text is not clipped to the page; it can paint over the bars when scrolled. Major.
3. A block taller than the pane: nothing scrolls the page to the caret. Major.
4. Closing the preview pane during visual editing leaves it stuck (`toggle_preview_pane`).
   Verified. Major.
5. A second quick click can get `block_at` answered from the old text (no `held` check on the
   click path). Major, narrow.
6. Keys edit at a hidden caret with no block open; Ctrl+A in a block selects the whole doc.
   Verified. Major.
7. Edits outside the block (Replace All, trim on save, reload, AI) put the box and the gap out of
   step. Major.
8. Minor: folds ignored; arrows at the first/last block; a one-frame stale `gap_y`; the
   `live::BARE` global can make parallel tests flaky.

**C. `f/four-fixes`.** No blocker; nothing reopens an audit 1 bug outright.
1. Rename leaves the recovery copy under the old path: after a crash it comes back as a detached
   Untitled draft. Major.
2. Rename is check-then-`fs::rename`, which replaces a file appearing in between. Verified. Minor.
3. Undo after the AI conversion leaves the doc Markdown and the Rename bar up. Minor.
4. A custom provider with a URL still asks the credential store when keyless; fails where none
   runs (Linux). Verified (`keys::get`). Minor on Windows.
5. Rename of a tab whose file is not on disk fails and drops the offer. Minor.
6. Minor: 1.1.2 round trip drops custom providers; migration drops a custom URL when another
   preset was chosen; keys per URL are shared and orphaned on remove; duplicate "Custom provider
   N" names; Save As keeps a chosen language; stale marks and find results after rename;
   `MDEDIT_AI_KEY` ignored for local presets.

## Visual editing, B+ (2026-11-20/21, branch `f/visual-edit`, not pushed, not released)

Ctrl+E (tab menu "Edit on the pa&ge", palette) on a Markdown tab: the preview takes the whole
text area. A click on a block opens it as its source in a box at its place on the page; the
document's own text view is drawn there (`app/visual.rs`), so undo, find and every command work.
Esc, a click anywhere outside the box, or the caret leaving the block closes it; arrows past its
ends open the next block. Approved by the owner side by side with the spike ("it's the same").

- The renderer leaves the room (mdview `set_gap`, protocol `MDVPREVIEW7`, pinned at `5be9ca7`):
  the first covered top-level block becomes empty room of the box's height, the rest take none.
- While a block is open the page gets no text (`preview_sync`, `held`): it keeps the text the
  block opened on, so it stands still while typing, as the spike did. The gap is in that text's
  lines (`Block.lines`, `Block::last`). The edits go to the page when the block closes.
- The caret leaving the block sets `Visual.pending`. `preview_sync` sends the new text first,
  then asks `block_of_line`; if the text is held for an answer, the question waits too.
- A click outside an open block only closes it (as the spike); with none open it asks `block_at`.
- The box: fill `box_fill` (page mixed 4% toward text, also the text view's background inside),
  accent border, radius 6, 10 px padding, 6 px of room above and below. No gutter (`live::bare`),
  wrap forced on.
- Test: `app::preview::tests::visual_editing_opens_a_block_in_place_follows_its_typing_and_puts_it_back`.
- Not done: scrolling the page to a block the arrows or find open off-screen; a View menu entry
  (no free letter); PLAN step 3 edge cases; step 4 budgets. Run on Windows only (tests and the
  owner's hand test).
## Four owner requests (2026-11-20, branch `f/four-fixes` off `main`, not pushed)

- **A saved prompt's pencil** puts it in the instruction box to be changed and runs nothing.
  The word is "Modify" (the owner's choice: "edit" reads as what every prompt does to the
  text). It is a framed button (pencil and word, `ai_ui::draw_modify`) in both places, so it
  reads as a control of its own and not as the row: in the Rewrite panel at each saved
  prompt's right (`ai::Hit::EditPrompt`, `ai_ui::pencil_rect`), and in the AI submenu drawn
  over each saved prompt's row after the shared menu (`ai_ui::draw_menu_modify`,
  `menu_modify_rect`; the row's hint is blank room, `overlays::MENU_MODIFY_ROOM`, so the name
  never runs under it). It takes the accent when the pointer is on it (the menu repaints when
  the pointer crosses its edge, `menu_modify_at`). The rest of the row still runs the prompt.
  A click on the menu's button is `Act::AiEditPrompt`, which opens the panel if needed
  (`App::ai_edit_saved`). The pencil now means edit only: the AI item and the
  built-in requests carry the AI mark (`chrome::SPARKLE`, a four-pointed star drawn here as
  the shared icons are), saved prompts carry play (`glyph::PLAY`, as saved macros do), in the
  menus and the palette. Rewrite then runs the text under the prompt's name while it
  is unchanged, else as "Rewrite".
  `--menu text-ai` shoots the text menu with the AI submenu open. Test `a_saved_prompts_pencil_puts_it_in_the_box_to_be_changed`.
- **About's OK with the preview pane on.** `left_press` tried the page before the dialog, and
  the OK button lies over the page. `App::on_page` is false while a dialog is open; the press,
  the hover cursor, the cursor answer and the right-click use it. Test
  `a_dialog_over_the_page_takes_the_click` (fails with the fix reverted).
- **The days before a tab is offered** are a setting: `Prefs::stale_days` (`stale_days=`, 1 to
  365, default 7), Settings > Session, "Offer them after … days" under "Offer to close tabs
  left unopened". `Docs::offer_stale_tabs` and the bar's text (`chrome::stale_label`) use it;
  the shared `stale::DAYS`/`is_stale`/`label` are no longer used here (mdview keeps its week).
  Test `the_days_before_a_tab_is_offered_are_a_setting`.
- **The caret and scroll per file** (`places.rs`, the shape of `marks.rs`): the caret's line and
  column and the top line, for the 100 files used last, `place=line,col,top;path` in the
  session. A file closed leaves its place; the session write takes the open tabs' places; a
  file read for the first time in a window gets its place back (before `pending_select` and
  `pending_goto`, so `--goto` and the viewer's hand-off still win), clamped to the text. Moving
  the caret does not change the session's signature: places are written at exit, when a file
  closes, or with any other write. Test
  `the_caret_and_scroll_come_back_when_a_file_is_opened_again_and_across_runs`.

Run on Windows: `cargo fmt`, `cargo clippy --all-targets` (no warnings), `cargo test -p mdedit`
and `-p mdedit-core`. Budgets not measured. The owner looked at the shots and test builds and approved them.

### Custom AI providers: no key needed, and more than one (same branch)

The owner: "have it be possible to use custom ai's without api key. and have more than one
custom provider possible to add."
- `mdedit_core::ai::CustomProvider` (name, base URL, model, effort); `Prefs::ai_customs` (at
  most `CUSTOMS_MAX`, 32) and `Prefs::ai_custom`, the one chosen while `ai_preset` is `custom`.
  `Prefs::ai_url`, `ai_model`, `ai_effort` and their setters read and write the chosen custom
  provider's own; presets keep sharing the connection's model as before.
- File: `ai_custom=<n>` and `ai_custom_<n>_name|url|model|effort`, and `ai_base_url` (the
  chosen custom provider's URL), all a 1.1.2 build keeps. A file with no list and an
  `ai_base_url` (from 1.1.2, or written back by it) becomes a list of one, named "Custom", with
  the connection's model and effort, whichever provider is chosen; so new → 1.1.2 → new keeps
  the chosen provider (the others are lost: 1.1.2 drops what it does not know). Gaps in the
  numbers close; a `custom` preset with no provider becomes OpenAI.
- Settings › AI: Provider lists the presets, the custom providers by name, then "Add a custom
  provider…" (adds "Custom provider N" and chooses it). A custom provider shows Name, Base URL,
  an optional API key (placeholder and help say so) and "Remove this provider". The rewrite
  panel's line names the custom provider.
- No key: the engine already sent `authorization` only with a stored key. Now Settings asks for
  a custom provider's models once the store has answered, stored or not (it waited for a stored
  key); a local preset (Ollama, LM Studio) has no key slot (`key_slot` gives `None`) and its
  runs send an empty slot, for which `mdedit-ai`'s `keys::get` answers none without asking the
  store (so they work where no credential store runs). A custom provider's key stays per URL,
  so a key stored for the old single custom URL still applies. Removing a provider clears its
  key from the store unless another provider uses that URL, so one added later on it does not
  send it unasked. A store that cannot be asked (none running, as on many Linux desktops) means
  no key for a custom slot (`keys::decide`), so the models are listed and runs go without one;
  for the presets that need a key it stays an error. `MDEDIT_AI_KEY` applies to every slot,
  a local preset's empty one too. "Add" names a provider "Custom provider N" with the first N
  free (`core_ai::new_custom_name`).
- Tests: `custom_providers_are_a_list_and_the_old_single_one_joins_it` (core),
  `custom_providers_are_added_chosen_and_removed_and_need_no_key`,
  `an_empty_slot_has_no_key_and_asks_no_store`, and `presets_and_slots`.
- Checked on Windows: the release `mdedit-ai.exe` against a stand-in server on 127.0.0.1 listed
  the models and ran a rewrite for a custom provider with no stored key; neither request carried
  an `authorization` header. Not tried against a real LM Studio or Ollama.

### Language switch, and a .md name after an AI conversion (2026-11-21, same branch, not committed)

The owner approved: a language list from the status bar, kept per file; a text file the AI makes
Markdown switches at once and is offered a `.md` name ("not intrusive and converted markdown
file has no reason not to use .md").
- `languages.rs`: the languages offered. The shared highlighter has no list, so it is what a
  fixed set of names and extensions finds through `Language::by_token`, each once, by name
  (61 tokens; the Rails and ASP variants left out). "Plain text" is no language.
- Per file: `Ready::language_set` (chosen over what the name gives). `Docs::set_language`
  recolours as Save as does (revision raised, service reopened); choosing the name's own
  language clears the choice. Kept in the place (`places.rs`): `place=line,col,top,Markdown;path`,
  so a file at its start with a choice is kept too; a name this build does not offer is dropped.
  Restored on first read, before colouring; kept across a reload; Save as keeps it unless the
  new name gives it anyway. Nothing is written to the file.
- The list is the command palette's card in a fixed mode (`palette::Mode::Languages`, opened by
  `Host::open_languages`, placeholder "Type a language"): plain text, Markdown (hint "preview"),
  then the rest by name, the current one checked. `Hit::StatusLanguage` (the status field, tip
  "Language (click to change)"; where the indentation field is left out the language field no
  longer carries its tip) and the "Language" command open it. Anchored above the field:
  `palette::anchored_window` gives the shared palette (which centres its card near the top of
  what it is given) a narrow area ending at the field, moved down once the card's height is
  known. The footer's note is painted over the shared footer (`palette::draw_note`; the shared
  one gives its text a third of the width): "Kept for this file. Back to the extension's: Plain
  text." or "From the extension: Plain text. A choice is kept for this file."
- After AI: Format as Markdown or Fix Markdown applied to the whole of a document that is not
  Markdown (direct or from the review) makes it Markdown (`App::ai_converts`,
  `ai_made_markdown`). A part of a file changes nothing. A file not named `.md`/`.markdown` gets
  the rename bar while its tab is active (`Docs::rename_offer`, `rename_bar`,
  `chrome::draw_rename_bar`): "notes.txt is Markdown now. Rename it to notes.md?", Rename / Keep.
  `Docs::rename` renames on disk and keeps the tab, its unsaved text and undo; the recent list,
  places and the file watch follow, the session writes the new path. A name that exists, or is
  open in another tab, is refused with a message and nothing is renamed.
- Visual editing (Ctrl+E) is on `f/visual-edit`, not this branch; it asks `is_markdown()` too,
  so it should follow once merged (not tried).
- Shots: `--menu language`, `--menu rename-bar`. Tests:
  `a_language_chosen_is_the_documents_and_comes_back_with_the_file`,
  `a_chosen_language_outlasts_a_reload_and_follows_save_as`,
  `a_text_file_formatted_as_markdown_is_markdown_and_offered_its_name`,
  `only_a_whole_text_file_formatted_is_switched`,
  `the_list_has_each_language_once_and_names_lead_back`, and the places and session round trips.
- Not done: the rename has no undo of its own (rename back by hand).

### Fable review of f/four-fixes (2026-11-21, same branch)

Each fix has a test that fails with the fix reverted (checked on Windows by reverting each in
turn).
1. A rename rewrites the recovery copy at once (`recovery.rs`: a slot keeps the path it was
   written for). Test `a_renamed_tab_has_its_copy_written_again`.
2. The rename never replaces a file: `docs::move_no_replace` (Windows `MoveFileExW` without
   `MOVEFILE_REPLACE_EXISTING`, windows-sys feature `Win32_Storage_FileSystem`; elsewhere a hard
   link then removing the old name, falling back to a rename after a look where links are not
   supported). The look before uses `symlink_metadata`, so a name that cannot be looked at is
   taken as taken. Test `the_rename_move_never_replaces_a_file`.
3. Undoing the AI's change back to the state before it takes the switch back: the language as
   it was and the bar (`Ready::ai_switch`, `Docs::drop_undone_switches`, run at each layout).
   A language chosen any other way, Save as or a rename clears it. Redo does not switch again.
   Test `undoing_the_conversion_takes_the_switch_back`.
4. and 10. `mdedit-ai` `keys::decide`: the override first, then the empty slot, then the
   store; a store error is no key for a custom slot. Tests
   `a_store_error_is_no_key_only_where_the_key_is_optional`,
   `the_override_is_used_for_every_slot`.
5. A tab whose file is no longer on disk is renamed by its path alone, and its stamp is that
   of nothing there, so Save writes without asking. Test
   `a_rename_follows_through_and_needs_no_file_on_disk`.
6. Migration both ways (above). Test `custom_providers_are_a_list_and_the_old_single_one_joins_it`.
7. Key cleared on remove; first free name. Tests
   `custom_providers_are_added_chosen_and_removed_and_need_no_key`, `presets_and_slots`.
8. Save as drops a chosen language when the extension changes (a copy with the same extension
   keeps it) and resets the twin pane's colours. Test
   `a_chosen_language_outlasts_a_reload_and_follows_save_as` (the twin's colours not tested).
9. A rename moves the old name's `marks` entry and the find results' rows (`Results::renamed`).
   Test `a_rename_follows_through_and_needs_no_file_on_disk`.

## Release 1.1.2 (2026-11-20, released)

mdedit 1.1.2 is released: tag `v1.1.2` at `f8dce9b`, release workflow run 44651603606 green,
manifests and assets verified (see `docs/RELEASE.md`). It carries the two round-6 fixes (reload
keeps a waiting tab editable; a due session retry is made). 1.1.1 is unchanged. The round-6 and
round-7 notes below predate this release. Round 7 came back clean, so no fixes are pending.

## Audit 1, round 7 (2026-11-20)

Astra re-reviewed `09c5f93`: no new findings, and both round-6 fixes were confirmed on Windows.
Astra ran the 25 application tests with default and with no default features; not the full
workspace, clippy, the revert checks, Linux or macOS. Merged into `main` with no release; the
fixes go out with the next version.

## Audit 1, round 6 (2026-11-20, branch `f/next`, after the 1.1.1 release)

Astra's re-review of `3e6359c` found two regressions. Both are fixed; each test was checked to
fail with its fix reverted. No release for these yet (the owner's call).

- **Reload kept the waiting tab read-only.** The tab was read-only while its recovery copy could
  not be read. Reload put the copy's text back but carried `read_only`, so `Recovery::poll`
  dropped the copy. Now `apply_load` does not carry `read_only` from a tab whose text was
  waiting; it is set again only if the text still cannot be read. Test
  `a_reload_that_puts_the_waiting_text_back_keeps_its_copy` runs Reload, the Discard dialog, the
  load on its thread, the recovery look, then a crash and the next start.
- **An expired retry with an unchanged session.** A cancelled exit puts the old list back, so
  the signature matches the one last written while `session_retry` stays set in the past, and
  `next_wake` spun the loop. Now `persist_if_changed` writes whenever a retry is due: it succeeds
  and clears, or fails and is due 5 s later. Test `a_due_retry_is_made_when_nothing_changed_since`.

Known and accepted: a legacy (id-less) backup whose file time changes while the session still
cannot be written gets a new `legacy_draft` id, so its text comes back twice. Nothing is lost.
Astra reproduced it and judged it conservative.

Astra noted that `3e6359c` itself lacks the round-5 section below; it was in the next commit,
`2dd3797`.

## Audit 1, round 5 (2026-11-19, branch `f/next`, release 1.1.1 held for it)

Astra reviewed `6cdf50e..1fe830f`: four recovery blockers, two round-4 leftovers, the updater
repair, the cross-build script and six Save prompt cases. All are fixed in `3e6359c`. Each test
below was checked to fail with its fix reverted.

- **#1: a copy locked after the start.** The backup takes the copy's place only when the copy
  has no text (gone, or not whole). On an I/O error the tab keeps waiting for the copy, which the
  exit keeps (`read_restore` now returns `io::Result<Option<String>>`). Test
  `a_copy_locked_after_the_start_is_kept_for_the_next`.
- **#2: typing into a tab whose text waits.** That tab is read-only, and an edit gets the
  `waiting_text_toast`. Test `a_tab_waiting_for_its_text_takes_no_typing`.
- **#3: an untitled backup locked at the start.** It goes in `Docs::unread_backups`, which
  `Docs::backups_needed` (the session's pruning) and `write_backups` keep. Test
  `an_untitled_backup_locked_at_the_start_is_kept`.
- **#4: exit with an unwritable session.**
  - `write_backups` only writes. A new backup never takes the name of a file already there.
  - `keep_unsaved` writes the session, and only then calls `Docs::drop_stale_backups`.
  - When the session cannot be written, the old list is restored and the Unsaved dialog asks.
  - `Recovery::finish(docs, saved)` keeps the copies of modified tabs when the exit's session
    write failed.
  - Test `an_exit_whose_session_cannot_be_written_keeps_everything`.
- **Legacy multiplication.** `recovery::legacy_draft` gives an id-less backup a stable id: FNV of
  its name, document, size and file time. Restoring uses it, and `cover` matches a copy against
  it. Test `a_legacy_backup_does_not_multiply_when_the_session_is_never_written`.
- **Session retry wake.** `App::next_wake` includes `session_retry`. Test
  `a_failed_session_write_wakes_the_window_to_try_again`.
- **Updater repair.**
  - `updater::repair` reads `repair_manifest_url()`, which is
    `releases/download/v<version>/<manifest>`, the running version's own tag.
  - `spawn_look` runs at launch whether or not a look is due, and on Check for updates.
  - A failed repair shows `strings::repair_failed`.
  - A static flag stops two repairs from running at once.
  - Test `a_repair_reads_the_manifest_of_its_own_version` checks the URL only. No download path
    is tested here; `restore_engine` is the shared crate's and is tested there.
- **Cross build.** The container's `bash -c` starts with `set -e`. Not run.
- **Save prompt.**
  - The template is created before the duplicate check.
  - It refuses when the prompts file is open with unsaved changes.
  - It refuses when the result would be over 256 KiB, or would not read back as every earlier
    prompt plus the new one.
  - `escape_line` and `unescape_line` round-trip heading and fence lines.
  - `append_prompt` keeps the file's bytes and uses its line ending.
  - Tests:
    - `a_saved_prompt_reads_back_exactly_and_the_file_is_kept` (core);
    - `a_prompt_the_template_has_is_not_saved_twice`;
    - `a_prompt_is_not_saved_under_unsaved_changes_to_the_file`;
    - `a_prompt_that_would_not_read_back_is_not_saved`.
- **Release notes.** Paste is compared with pasting a copied file, not dropping one. The notes
  also mention the repair retry and the exit fix.

Not run: Linux, macOS, the cross build, a real repair download. The owner has not yet looked at
the AI strip or the Save prompt button.

## Audit 1, round 4 (2026-11-19, branch `f/ai`)

Astra's round-4 re-review (10 findings) is fixed. Recovery identity moved from path/number to a
stable **draft id** (`draft=` in the recovery header and a `draft=<file>|<hex>` session line; older
builds skip both). Each test below was checked to fail with its fix reverted.

- #1, #9 (remap lost on an unwritten session, header rewrite): no header is rewritten any more;
  a copy covers a backup by draft id (`recovery::cover`). A failed session write is retried after
  5 s (`session_retry`). Test `app::tests::drafts_survive_starts_whose_session_is_never_written`.
  The retry itself has no unit test.
- #2 (a readable header hid a valid backup): a file tab keeps the backup as `Doc.fallback` until
  the copy's text is in. Test `a_copy_whose_text_cannot_be_read_gives_way_to_its_backup`.
- #3 (a read error deleted the only copy): `read_header_checked`/`read_checked` tell an I/O error
  from an invalid record; unreadable copies are kept by scan and exit. Test
  `recovery::tests::a_copy_that_cannot_be_read_now_is_kept`.
- #4, #5 (path case, ambiguous legacy entries): legacy matching uses `same_path`, covers only one to
  one and only when the copy is at least as new as the backup; a file tab already taking a copy
  never has it replaced. Tests `copies_and_backups_without_ids_cover_only_when_certain`,
  `two_old_entries_with_one_number_and_a_copy_all_come_back`,
  `an_old_copy_older_than_the_backup_hides_nothing`, `two_copies_of_one_file_both_come_back`.
- #6: the key is masked over the whole error body before any cut; a body over 64 KiB shows a fixed
  message; the OpenAI stream error is masked with the run's key; CLI stderr at its cap drops the
  cut line. Tests `an_error_body_loses_the_key_before_it_is_cut`,
  `a_streamed_error_loses_the_key_before_it_is_cut`, `stderr_at_its_limit_loses_the_cut_line`.
- #7: the model list read is cancellable and each page checks the job; the pre-POST cancel check is
  on the common path. Engine test `a_cancel_during_the_model_list_asks_for_nothing_more` fails only
  with both the page check and the cancellable read removed (each alone suffices).
- #8: the SSE reader counts every read as activity (`Activity`, `Watched`), `pump` consults it
  before Idle, and a closed flag stops the reader after the run ends. Tests in `sse::round4`.
- #10: the 40-character rule needs mixed case and digits. Test in `proto::tests::keys_are_hidden`.

Not run: Linux, macOS. No round-5 prompt was written; the owner deferred further review to the next
release.

## Audit 1, round 3 (2026-11-19, branch `f/ai`)

Astra's re-review of 41db024: 4 fixed, 7 partly fixed, 12 findings. All addressed; each test
below was checked to fail with its fix reverted (finding 1's fails on the old order at the first
start, where both entries named one tab).

| # | Fix | Test |
|---|---|---|
| 1 one copy hid two drafts | `bring_back` (app.rs): coverage settled first, copies restored before backups; a copy that comes back under another name rewrites its header at once (`Recovery::restore`), and a covered entry takes the copy's identity (`Restored.idents`) | `app::tests::two_drafts_survive_a_second_interruption` (three starts, Save As) |
| 2 queued failure, window asleep | `poll` wakes in 1 s while a failure is listed (count read before list); a writer that stopped is replaced | `recovery::tests::a_write_that_cannot_be_sent_is_tried_again` (asserts the dispatching poll) |
| 3 failed copy closed the review | Copy closes only on success | `ai_tests::a_closed_document_keeps_its_result_to_copy` (`CLIPBOARD_FAILS`) |
| 4 short keys, keys cut in excerpts | `keys::usable` for store and override, `set` validates before the override; `body_message` and `read_part` cut at a word's end | `keys::tests::a_short_key_is_never_used`, `api::tests::a_cut_never_leaves_part_of_a_key` |
| 5 remap only in memory | the session is written at start when an entry changed | covered by the `remapped` assertion of #1's test; the write itself is in `run`, not tested |
| 6 reader and consumer disagreed | `claude_last` / `openai_last`: the top-level type, the same events the streams end on | `api::tests::the_reader_and_the_answer_agree_on_the_end` |
| 7 heartbeats timed out as idle | `Item::Alive` every 200 ms of lines without an event; a reader whose run ended stops at its next line | `sse::round3::comments_keep_a_stream_alive`, `a_reader_of_comments_stops_after_a_cancel` |
| 8 old key reply bypassed the URL pause | `ai_settings_needs` skips key and models while `url_due` is set | `ai_tests::a_key_draft_does_not_follow_a_new_url` (A's reply drained first) |
| 9 Apply shown for a closed source | `ai_review_sync` at compose, key and action | `ai_tests::a_closed_document_keeps_its_result_to_copy` (drawn before any action) |
| 10 event name not counted | `Parser::size` = name + data | `sse::round3::an_event_name_counts_toward_the_limit` |
| 11 tests shared a folder | `file_docs` numbers its folders; the N1 test has its own | — |
| 12 model alias hidden | the prefix rule needs a 16-character run or 40 in all (`random_part`) | `proto::tests::keys_are_hidden` |
| scope | the cold model enquiry of a Claude run takes the job; a cancel ends the run before the POST | not tested (no stub for it) |

The AI app tests (`ai_tests`) now run on Windows too: the stand-in engine script runs under Git's
bash from a batch file (`MDEDIT_TEST_BASH` names another bash). Not run: anything on Linux or
macOS this round, real providers, the real credential store.

## Audit 1, round 2 (2026-11-19, branch `f/ai`)

Astra's re-review of cffebe3: 9 fixed, 6 partly fixed, 8 new findings. All addressed; each test
below was checked to fail with its fix reverted.

| Item | Fix | Test |
|---|---|---|
| N1 restored draft lost its session entry (file gone, or number taken) | `restore_unsaved` rewrites each entry to the tab it came back as | `app::tests::kept_drafts_survive_restarts_with_restore_off` |
| N2 Windows: case-different URLs share a credential | `keys::target`: custom slots stored under SHA-256 of the URL | `keys::tests::custom_endpoints_have_names_apart` |
| N3 Rewrite could send a scope the panel did not show | `Instruction.source`; changed scope is shown and needs Rewrite again | `ai_tests::rewrite_sends_only_the_scope_the_panel_showed` |
| N4 key draft followed a new custom URL; new URL's key not looked up | `key_draft_slot`; `ai.url_due` (500 ms) runs `ai_settings_needs` | `ai_tests::a_key_draft_does_not_follow_a_new_url` |
| N5 finished stream kept its reader and socket | parsing on the reader thread; it stops after the last event | `sse::tests::the_reader_lets_go_after_the_last_event` |
| N6 Run again on a closed source dropped the result; closed before the answer, no review | review kept, Run again hidden; `ai_finish` builds a closed review | `ai_tests::a_closed_document_keeps_its_result_to_copy` |
| N7 "Bearer authentication is required" redacted | `looks_like_key` after markers | `proto::tests::keys_are_hidden` |
| N8 queued jump to a missing category (shared 1cc5220) | ignored and dropped on `set_rows` | shared `settings::tests::a_jump_to_a_missing_category_is_ignored` |
| #5 failure unseen while idle; dispatch failures | `in_flight` keeps a look within 1 s; unsent writes count as failed | `recovery::tests::a_failed_write_is_tried_again`, `a_write_that_cannot_be_sent_is_tried_again` |
| #6 short keys, other providers' tokens, key cut by truncation | keys ≥ 4 chars (refused shorter), more prefixes, key hidden in the raw body | `proto::tests::keys_are_hidden`, `engine::a_key_at_the_cut_of_a_long_error_leaves_no_fragment` |
| #9 multiline event unbounded; error body not cancellable | event cap 1 MB, 8 events ahead; `sse::read_body` for error bodies | `sse::tests::a_huge_event_of_short_lines_is_refused`, `a_silent_error_body_is_cancelled_at_once` |

Still a limit: after a cancel, idle end or error, the reader thread lives until its read returns
or the 30-minute body timeout (ureq gives no way to close the socket from another thread); it
holds nothing the engine waits for.

## Audit 1 fixes (2026-11-19, branch `f/ai`)

Astra audited `f/ai` @27530b6 (and shared `f/ai` @19348d3) on Windows and found 10 problems, 3
smaller AI issues and 2 Windows validation failures. All are fixed; each has a test that fails on
the old code where one could be written (checked by reverting the fix for 2, 7 and 10). The owner's
decision on 1: closing never asks; unsaved text is kept and comes back whatever "Restore tabs at
start" says.

| # | Finding | Fix | Test |
|---|---|---|---|
| 1 | Restore off: the second exit deleted a draft | `app.rs` `restore_unsaved` puts backups back whatever `restore` says; `persist_if_changed` prunes with restore off too | `app::tests::a_kept_draft_survives_restarts_with_restore_off` |
| 2 | Apply overwrote text reloaded after the review opened | `ai_blocked` checks the range again on Apply | `ai_tests::apply_does_not_overwrite_text_reloaded_meanwhile` |
| 3 | Run again sent another tab's text | Run again activates the review's document, opens the instruction panel filled in (`Instruction.spec`), waits for Rewrite | `ai_tests::run_again_is_for_the_reviewed_document_and_asks_first` |
| 4 | Custom provider URL reused the `openai:custom` key | `key_slot(conn, preset, url)`: one slot per normalized URL | `mdedit_core::ai::tests::a_custom_endpoint_has_a_key_per_url` |
| 5 | Recovery took a failed write as written | the writer hands `(name, key)` back; `poll` clears `written`, due again after the pause | `recovery::tests::a_failed_write_is_tried_again` |
| 6 | Diagnostics could carry the key | `proto::redact` on every error event, `Fail::hide(key)` in each API call | `proto::tests::keys_are_hidden`, `engine::a_key_echoed_by_the_server_is_hidden` |
| 7 | An edit during viewer start dropped an untitled preview's `open` | `Update::over` keeps the older `open` in `Courier::put` and in the retry | `link::tests::a_newer_update_keeps_the_request_to_open` |
| 8 | `jump_to_category` / `current_category` before layout panicked (shared) | the form keeps the jump for its next layout (shared f36af6f); mdedit's `settings_jump` workaround removed | `settings::tests::a_jump_before_layout_waits_for_it` (shared) |
| 9 | A silent stream could not be cancelled; no body timeout | `sse::read` on a reader thread, 200 ms cancel checks, 180 s idle, 1 MB line / 2 MB reply caps, 30 min body timeout | `sse::tests::a_silent_stream_*`, `a_huge_line_is_refused`, `engine::a_huge_reply_is_not_used` |
| 10 | A change on disk discarded an unsaved encoding choice | `check_changed` uses `Doc::modified()` | `files_tests::a_change_on_disk_keeps_an_unsaved_encoding_choice` |
| – | A key found did not fetch the models | `Event::Key` calls `ai_settings_needs` | `ai_tests::settings_has_an_ai_group_that_asks_for_the_key` (fake engine answers `models`) |
| – | Read-only + review off: "done" toast, nothing changed | `ai_replace` returns whether it went in; the review opens blocked (`Review.blocked: Option<String>`) | `ai_tests::a_read_only_document_shows_the_result_to_copy` |
| – | A CLI whose `--version` failed counted as found | the exit status is checked | `engine::fake_cli::a_cli_whose_version_fails_is_not_found` |
| – | Windows clippy: unused `ai_start_for_test` | `#[cfg(all(test, unix))]` like its caller | clippy for x86_64-pc-windows-msvc |
| – | Shared crash test failed on Windows paths | the assertion takes `\` as `/` (shared f36af6f) | `crash` tests (shared) |

Run here (Linux): all workspace tests with default and no default features, clippy `-D warnings`
with default and no default features and for x86_64-pc-windows-msvc and aarch64-apple-darwin, fmt; same
in the shared repo. Not run: anything on Windows, real provider calls, the real credential store.

## AI rewriting (2026-11-19, branch `f/ai`, not merged, not released)

Owner's request and approved mockups; design in DESIGN.md section 68, PLAN.md amended.

- Phase 1 (shared, `markdown-shared-rs` branch `f/ai`, 19348d3): secret text fields.
- Phase 2 (`bins/mdedit-ai`, fc6955d): the engine, its four connections, keys; 18 unit and 12
  integration tests against a stub HTTP server and fake CLIs.
- Phase 3 (this commit): mdedit's side: commands, palette, text menu, the instruction and review
  panels (Changes / Preview with mdview's renderer), the bar, Settings › AI, prefs, saved prompts,
  `replace_range` as one undo step. 6 app tests with a stand-in engine script.
- Next, phase 4: package `mdedit-ai` (installer, zip, Linux `lib/mdedit`, Mac bundle, release
  workflow, updater manifest), measure the budgets with `tools/compare-builds.py`, a Windows test
  build for the owner.
- Not run: real API calls with keys, the Codex CLI, a real Claude Code rewrite, the credential
  store on Windows/macOS, anything on Windows. The owner has not seen it in the app yet.
- The shared crates are pinned to the `f/ai` commit; move the pin to `main` once that branch is
  merged there.

## Installer: no scripts under "Open with" (2026-11-18, branch `f/installer-types`)

The owner found that after installing mdedit, .bat files no longer ran: the installer offered
mdedit under "Open with" for .bat and .cmd (and other runnable types), and a new handler there can
make Windows ask how to open the file instead of running it. `mdedit.iss` now registers only
text, data and source types that nothing runs on a double-click (`Exts`, .txt first) and, at
every install, takes off what 1.0.5 and earlier wrote for `Runnable` (bat, cmd, ps1, psm1, js,
mjs, cjs, sh, bash, zsh, py, rb, pl, php, lua, html, htm): the `OpenWithProgids` value, the
`SupportedTypes` and `FileAssociations` entries, and the keys it made, when empty
(`RegDeleteKeyIfEmpty` in `[Code]`). mdedit still opens those types from its Open dialog, by
drag and drop and with the optional "Edit with mdedit" entry. Not run on Windows yet. The
self-updater swaps the exe only, so an install updated that way keeps the old entries until the
installer runs again.

.txt is associated, on the owner's word ("don't leave .txt alone, associate"): the task
"txtdefault" (checked) writes mdedit's ProgId as the default of `HKCU\Software\Classes\.txt`,
which decides where the user never chose another application. A choice made in "Open with >
Always" or Default apps (Explorer's protected `UserChoice`) wins over it and an installer cannot
change it; then (`TxtChosenElsewhere` in `[Code]`) the last page offers
`ms-settings:defaultapps?registeredAppUser=mdedit`, mdedit's Default apps page. Uninstalling
removes the default value.

## START HERE: feature complete, in maintenance (2026-11-17)

The owner agreed that mdedit is feature complete: see the status at the top of `docs/PLAN.md`
(what maintenance means, the small known gaps, what was set aside). The latest release is 1.0.4 (below).
The owner tried the right-click menu of 1.0.2 and said it works. mdview's own 1.0.1 release was
withdrawn; mdedit pins viewer commit `56dfe05` (from 1.0.4), which is on mdview's `main`.

## Tabs not opened for a week (2026-11-18, branch `f/stale-tabs`)

The owner asked for an offer to close tabs left unopened, on by default, in both applications;
the rules are `mdshared_ui::stale` (shared `f/stale-tabs` at `49de4ec`), the viewer pin is
mdview's `f/stale-tabs` (`cf965a8`), which has the same. Time counts in days of use
(`Session::use_days`, one more at a start on a new date); each tab keeps the day it was last
shown (`Doc::seen`, `seen` lines by `Pin`; none in the session counts as today, so the first
start after an update offers nothing). At start (`Docs::offer_stale_tabs`, after the pins are
put first) tabs not shown in the last 7 days of use go on a bar under the command bar
(`chrome::draw_stale_bar`, `notice::draw_offer_bar`): Close them, Keep (their count starts
again), the close button (asked again next start). Never offered: pinned, showing, in the other
pane, changed, or without a file. `App::stale_bar` checks each again before closing.
Setting `Prefs::offer_stale` (on), Settings > Session, "Offer to close tabs not opened in a week".
Test: `tabs_not_opened_for_a_week_of_use_are_offered_and_closed_or_kept` (pin_tests.rs).
Budgets against 1.0.5's code (15 interleaved runs): no change beyond noise (first frame
94.7 → 94.4 ms, idle 25.6 MiB both).

## Pinned tabs: the tab left showing comes back, and a setting (2026-11-18, `f/pins`, for 1.0.5)

The owner found that a pinned tab showing at close came back as its neighbour: the session's
`active` counted only tabs with a file, so a pinned untitled note mapped to the next file tab.
- `session.rs`: `active_tab` and `last_pinned` keep tabs by identity (`Pin::File` /
  `Pin::Untitled`); `active` stays the fallback for older sessions.
- Setting "Start on the last pinned tab" (`Prefs::start_on_pinned`, Settings under "Restore tabs
  at start"): the pinned tab used last shows at start instead.
- The pin rules moved to `mdshared_ui::pins` (shared `main` at `6dcaf7f`) so mdview has the
  same; `Docs` implements `Pinnable` and calls `pins::set`, `move_tab`, `restore`,
  `start_tab`. The viewer pin is mdview's `f/pins` (`9d3398a`), which uses the same shared rev, so
  one copy of the shared crates is built.
- Test: `the_tab_left_showing_comes_back_even_a_pinned_note_and_the_setting_picks_the_pin`.
- Budgets against 1.0.4 (15 interleaved runs, Linux xvfb): no change beyond noise (first frame
  93.6 → 93.4 ms, idle 25.6 MiB both, keystroke p95 1.86 → 1.63 ms).

## mdedit 1.0.4: the cursor over the page, and the page still after a tick (2026-11-17)

The owner tried the Windows test build of `9c543c1` and said "good"; released as 1.0.4.

The owner tried 1.0.3: "cursor remains in caret mode on the entire preview window and clicking
boxes scrolls the page to top". Viewer pin `56dfe05` (protocol `MDVPREVIEW6`, on mdview's `main`,
not an mdview release).
- Cursor: a pointer move over the page asks it what is there (`Pane::hover`, `Request::Hover`;
  only one in flight, the latest place kept in `hover_due` and sent when the answers are in).
  `Pane::pointer` holds the answer and `preview_sync` shows it: a hand over a task box, the text
  cursor over text, the arrow elsewhere (`page_cursor`). Links are not followed on the pane, so
  no hand there.
- Scroll: the server followed the caret after every new text, and the caret was off screen
  (often at line 1), so the page jumped. The text that answers a tick now leaves the page where
  it is (`Server::keep_top` in the viewer); a caret that moved still comes as its own `Follow`.
- Test: `the_cursor_is_a_hand_over_a_box_and_a_tick_leaves_the_page_still` (caret at the far end,
  page at its top: the hand is found by hovering, the click ticks, the page stays at 0).
- Measured on Linux under Xvfb, 1.0.3 against the branch, 5 interleaved runs, pane closed and
  open: within noise. The benchmark does not move the pointer. Not run on Windows.

## mdedit 1.0.3, ticking task boxes on the preview pane (2026-11-17, released)

The owner asked for it ("like in mdview ... it shouldn't save it automatically though") and for a
point release of mdedit only. Viewer commit `2dc2099` on mdview's `main` (protocol `MDVPREVIEW5`,
not an mdview release) is the new pin.
- A single click on a task box on the page: the server finds the byte of its mark in the text it
  was sent (mdview's `tasks::toggle`, refused when the text and page disagree) and answers
  `Update::task`. The pane keeps it with the document and version that text was
  (`Pane::take_task`); `App::preview_task` applies it only if the document is still at that
  version, as `EditOp::SetTask` (`blocks::set_task`: one undo step, nothing when the bytes are no
  longer `[ ]`, `[x]` or `[X]`). The document becomes modified like any edit; nothing is saved;
  the caret stays and the view does not move to it. The page redraws from the new text.
- A double or triple click on a box selects as before.
- Tests: `a_click_on_a_task_box_ticks_it_in_the_text_without_saving` (app, in the embedded
  build), `a_task_mark_flips_in_place_and_the_caret_stays` (mdedit-core), and the viewer's
  `a_click_on_a_task_box_gives_its_mark_and_selects_nothing`.
- Measured on Linux under Xvfb, `tools/compare-builds.py`, 1.0.2 against the branch, 5
  interleaved runs pane closed and open, then 7 with the pane open: within noise (the first runs
  were slightly up, including pane closed where no new code runs; the 7-run rerun came out
  slightly down). Not run on Windows.

## mdedit 1.0.2, a right-click menu on the preview pane (2026-11-16, released)

The owner asked for it after 1.0.1 ("implement right click, only build edit this time"): only
mdedit is released; the viewer is unchanged and the pin stays at `8faf059`. The owner has not
seen the menu.
- A right-click on the page (`App::preview_menu`, called from `right_press` before the text's
  menu) gives the page the keyboard without touching its selection (`Pane::focus`) and opens
  `overlays::page_menu`: Copy (enabled with something selected on the page) and Select all. Both
  run through the same commands as their keys, so they act on the page.
- `Ui::page_selected` (set in `preview_sync`: the page has the keyboard and a selection) enables
  Copy in the command palette; Cut stays tied to the text's selection and does nothing
  while the page has the keyboard.
- Test: `the_page_selects_with_the_mouse_and_copy_takes_its_text` now also opens the menu with
  and without a selection, copies from it, and checks that Cut leaves the text alone.
- Measured on Linux under Xvfb, `tools/compare-builds.py`, main against the branch, 5
  interleaved runs, pane closed and open: every figure within noise (the largest, PLAN.md's
  scroll p95 with the pane closed, 1.81 to 2.12 ms with overlapping ranges; the pane-open runs
  moved the other way). Not run on Windows.

## mdedit 1.0.1, selecting text on the preview pane (2026-11-16, released)

Released as 1.0.1 with mdview 1.0.1 (that mdview release was withdrawn later the same day) on the owner's word ("I trust you"), without a hands-on
test on Windows. Record in `docs/RELEASE.md`.


The owner asked whether the page could be selected; their answers: copy as mdview copies (the
rendered text, with HTML beside it; the source is on the left), and no click-to-jump. Viewer
branch `f/pane-select` (`5af35f5`, protocol `MDVPREVIEW4`) adds the requests `Press`, `Drag`,
`Release`, `SelectAll`, `Unselect`, `Copy`; answers carry the selection's rectangles when they
change and, after `Copy`, the text and HTML.
- A press on the page (`App::preview_press`, before the hit test, since the pane has no hit)
  starts a selection there and gives the page the keyboard (`Pane::focused`; `text_focused` is
  then false); two clicks take a word, three the block, a drag extends, past the pane's edge the
  auto-scroll timer scrolls the page (`preview_autoscroll`, the page leads). A press on the page
  does not hand the lead to the editor.
- While the page has the keyboard: Copy and Select all (their keys, the palette) act on the page
  (`C::Copy`, `C::SelectAll` in `app/commands.rs`); Escape, any other key, or a press in the
  text, a tab or the find boxes give it back and clear the page's selection; the other keys then
  act in the text as usual. Copy puts text and HTML on the clipboard (`preview_copied`).
- The rectangles are page positions, blended over the frame as mdview does (`preview_pane::draw`,
  `pal.selection` at 0.30 light, 0.42 dark). The cursor is the I-beam over the page.
- A new text clears the selection (its offsets were into the old one).
- A right-click menu on the page came in 1.0.2 (above).
- Test: `the_page_selects_with_the_mouse_and_copy_takes_its_text` (both builds; the copy goes to
  a test slot, not the system clipboard), and 3 in the viewer's `server.rs`.
- Measured on Linux under Xvfb, `tools/compare-builds.py`, main against this branch, 5
  interleaved runs: pane closed, every figure within noise; pane open, within noise except
  PLAN.md's scroll (p50 1.24 to 1.67 ms, ranges overlapping), which a second run of 7 put back
  at 1.30 to 1.29 ms (p95 1.80 to 1.84). With nothing selected the new code does no work.
  Not run on Windows.

## mdedit 1.0.0 is released (2026-11-16)

**The released state is `main`, mdedit 1.0.0, tag `v1.0.0` at `3e3f8d8`, published with mdview
1.0.0 (`94280e0`) on the owner's word, after they approved the preview pane's Windows test build.
Shared crates: `main` of markdown-shared-rs at `51e7604`. 1.0.0 holds the sections below: the
preview pane (embedded by default, synced scrolling), the format bar, pinned tabs, crash reports,
the engine shipped with mdedit; notes in `docs/release-notes/1.0.0.md`, the release record in
`docs/RELEASE.md`. All feature branches are merged; the `preview-test` workflow now runs by hand
from `main`. The merged branches (`f/preview-pane`, `f/crash-report`, `f/macos`,
`f/control-fixes`, `chore/dependabot`) can be deleted; the cloud session could not.**

## Preview pane (2026-11-16, merged, in 1.0.0, approved by the owner)

**Decisions of the owner, later the same day (these replace the two-build comparison below):**
- **The renderer is built in** (feature `embedded-preview`, now the default). The build that
  asks `mdview --preview-server` to draw stays behind `--no-default-features`, for comparison.
- **The pane works without mdview.** `mdview-engine` (formulas, diagrams, SVG) ships beside
  mdedit, looked for there first, then beside mdview. The pane, its bar button and its menu items
  are always offered; "Preview in mdview" and side by side still need mdview. Checked on Linux
  with only mdedit and the engine in a folder: math and Mermaid drawn; the engine used 2 to
  4 MB while alive and had exited 12 s later. See "Shipping the engine" below.
- **`panic = "abort"` stays.** Unwinding so the pane could catch a renderer panic was built and
  measured on a Windows runner (`tools/compare-builds.py`, 7 runs, run 43886251468): keystroke
  and scroll p50 about 0.3 ms slower (0.76 to 1.03 ms on sample.rs), first frame +0 to 6 ms,
  exe 6.53 to 9.60 MB; Linux showed nothing. The embedded build with abort matched `main` on
  every figure (run 43882888383). The owner chose abort: a renderer panic ends mdedit and crash
  recovery brings the text back.

**Shipping the engine** (same branch):
- Built from the viewer at the revision mdedit pins (the `rev` of `mdview-preview` in
  `bins/mdedit/Cargo.toml`), in the viewer's own workspace (its lockfile, its size-first engine
  profile): `packaging/engine-source.sh DIR` checks it out, then `cargo build --release --locked
  -p mdview-engine --manifest-path DIR/Cargo.toml`. On Windows `packaging/windows/build-engine.ps1`
  does both, with the static C runtime, into `target\engine\mdview-engine.exe`.
- Windows: the installer (`mdedit.iss`), the zip (`build-zip.ps1`), `tools/release.ps1`,
  `tools/release-cross.sh` and `release-all.sh` carry `mdview-engine.exe`; the manifest has
  `windows-x86_64.engine.url/sha256` (`tools/write-manifest.sh --windows-engine`), and the
  updater (`updater::APP.engine`) replaces both files.
- Linux: the AppImage has it beside mdedit in `usr/bin`; the tarball, deb, rpm and PKGBUILD put it
  in `lib/mdedit/` (mdview's packages own `/usr/bin/mdview-engine`). mdedit looks beside its exe,
  then in `../lib/mdedit/`, then beside mdview (`preview_pane::own_engines`).
- macOS: `make-bundle.sh` copies it into `Contents/MacOS` when the build folder has one; the
  release workflow builds it and checks it is there.
- An mdedit 0.7 that updates itself installs only `mdedit.exe` (its updater knows no engine).
  Shared crates `21fd49b`: such a copy still counts as installed, its next update brings the
  engine, and the launch look (`updater::restore_engine`) fetches this version's engine when it
  is missing and the manifest is of this version.
- Not run: none of the release scripts or the release workflow has run with these changes.

**Synced scrolling** (the owner asked for it beyond the caret sync; viewer `8261730`, protocol
`MDVPREVIEW2`): the side the reader moved last leads, the other follows and sends nothing back,
so they cannot set each other off. A wheel over the page makes the page lead (`Pane::scroll`);
any key, click or wheel over the editor makes the editor lead (`App::editor_leads`, from the
window events). Leading editor: its top as a fractional source line (`view::source_top`, word
wrap included) goes to the page as `Align` when it moved. Leading page: each update's `line`
puts the editor there (`view::set_source_top`). The caret sync stays as it was and goes after
the align. Lines between two blocks share the page rows between them evenly.
The two ends: the page is mostly the longer, so the same top line on both sides left the
page's end out of reach, and the editor's next scroll pulled the page back up (the owner saw
it). Since viewer `c76948a` (protocol `MDVPREVIEW3`), `Align` also carries the editor's last
screen of scrolling (`view::source_ends`: without wrap, one screen above `max_top`; with wrap,
one screen of rows above the last row at the top, as far as wrapped scrolling goes); over it
the page's top goes straight on to its bottom, and the line in each update is the exact
inverse (the page at its bottom is the editor at its end), so the lead changing sides moves
neither. A page shorter than the text stops at its bottom as before.
Tests: `scrolling_either_side_brings_the_other_to_the_same_line`,
`the_editor_at_its_end_brings_the_page_to_its_end_wrapped_or_not` (fails without the fix; both
builds).
Measured on Linux under Xvfb, pane open, 5 interleaved runs, the commit before against this:
first frame and memory the same; scroll p50/p95 sample.md 1.19/1.24 to 1.39/1.90 ms, PLAN.md
1.27/1.63 to 1.71/1.96 ms (the ranges overlap; at the end the page now moves where it used to
stay); keystroke within noise (sample.md p50 1.32 to 1.81, PLAN.md 1.30 to 1.18). Budget under
4 ms. Pane closed: the code does not run.
Measured on Linux under Xvfb, `tools/compare-builds.py`, 5 runs, the commit before against
this: pane closed, every figure within noise; pane open, the scroll frame p50/p95 grows by
0.5 to 0.8 ms (sample.md 1.07/1.18 to 1.59/1.74 ms; PLAN.md 0.96/1.17 to 1.73/1.97 ms) since the
page now moves with the text and its region is repainted; budget under 4 ms. Not run on
Windows by hand; an mdview built before this answers with the old greeting, and the renderer
build then asks to update mdview.

**Format bar** (the owner: "build that toolbar next. make it support viewers features"; the
mockup was `FormatBar.dc.html`): a row under the command bar for Markdown documents
(`format_bar.rs`), on by default, View > "Format bar (Mar&kdown)" turns it off (session key
`format_bar=0`). Groups: B, I, S, code | H1, H2, H3 | bulleted, numbered, task, quote, alert |
link, image, table, code block | math, math block, diagram, footnote, collapsible section, rule.
A button shows pressed when the caret is in what it makes (bold span, heading level, list kind,
quote). Where the window is too narrow, the last groups give way to a "…" button that opens the
Markdown menu, which now has every command; the table's row and column commands moved to their
own Table submenu beside it (the menu had more items than letters). Headings 1 to 3 have
Ctrl+1/2/3; the rest have no key.
- New edits, each one undo step, in `mdedit-core/src/blocks.rs`: `toggle_list` (any other list
  marker is replaced; all of the kind already: markers off), `toggle_quote`, `cycle_alert` (plain
  -> NOTE -> TIP -> IMPORTANT -> WARNING -> CAUTION -> plain), `insert_fence` (code, `$$`,
  ```` ```mermaid ```` with a two-node flowchart, `<details>`), `insert_rule` (blank line above, so
  not a heading underline), `footnote` (next free number, its text appended at the end, the
  caret there; nothing over 4 MB), `line_state`. In `markdown.rs`: `image_from_selection`,
  `in_span`. Inline math is `MdWrap("$")`.
- The bar's own icons (numbered list, quote, code block, sigma, diagram, footnote, details, rule)
  are in `format_bar.rs`, not in the shared glyph set, so the shared crates did not move.
- Tests: `blocks::tests` (6), `images_are_links_with_a_bang`, `format_bar::tests` (2),
  `app::format_tests` (2: shown for Markdown only, toggle, overflow; a click edits, gives the
  keyboard back to the text, shows pressed), the session key, the access-key test with the Table
  menu.
- Measured on Linux under Xvfb, `tools/compare-builds.py`, 5 runs, the commit before against
  this (bar on): the first try cost 1.2 ms per keystroke on sample.md and about 7 ms of first
  frame, from asking the rope byte by byte for five span markers several times a frame. Now one
  string scan of the caret's line (`markdown::spans_at`, checked against the edit's own span
  finder on random text), once per frame (`App::format`). After that: first frame within
  noise (sample.md 93.0 vs 92.2 ms, PLAN.md 92.3 vs 93.1, sample.rs 90.6 vs 84.0), idle memory
  equal, scroll within noise, keystroke p50/p95 on sample.md 1.26/1.54 to 1.49/1.71 ms (+0.2 ms),
  on PLAN.md and sample.rs within noise.
- Not seen by the owner; not run on Windows.

**Crash reports** (shared crates `f/crash-report`, the owner's request; mdview has the same):
- `main` installs `mdshared_app::crash` first, into `data_dir("mdedit")/crashes`. A panic (any
  thread) and, on Windows, an access violation or stack overflow write `crash-<secs>.txt`: app,
  version, message, source place, thread, time, system, and a context line of kinds and sizes
  (`app/crash.rs`: "Markdown, 48 KB; 2 tabs; preview pane open"), never text or file names.
  Five are kept. Nothing is sent.
- On the next start a bar under the update bar offers Open report (in a tab), Copy, close; each
  marks it seen (`.seen.txt`, kept).
- Checked: on Linux a real panic on the preview thread (a temporary trigger, removed) gave the
  report and the next start's bar. On a Windows runner (preview-test run 43896081143) the shared
  example wrote correct reports for a panic and an access violation (`0xc0000005 at exe+0x340d`).
  Not seen on Windows: mdedit's own bar.
- The same run compared `main` with this branch's default build (abort, standalone engine, crash
  reports) on a Windows runner, 7 runs: every figure within noise (keystroke p50 0.53 vs 0.54 ms
  on sample.rs, first frame 107.2 vs 106.8 ms on sample.md, idle memory equal).

The owner asked for a preview inside mdedit, in two versions to compare, and for it to be hidden
without mdview. Viewer side on the viewer's branch `f/preview-pane` (crate `crates/preview`,
`mdview --preview-server`, `mdview_render::set_engine_path`); the editor pins that branch's
commit.

- **One pane, two builds.** Default: `mdview --preview-server` draws the page in its own process;
  mdedit sends the text, the pane size, the caret's line and wheel scrolls over a pipe
  (`mdview_preview::wire`) and gets back the rows that changed. Feature `embedded-preview`: the
  same loop (`mdview_preview::serve`) on a thread of mdedit, with mdview's renderer built in.
  Code: `preview_pane.rs` (the pane, its threads, drawing), `app/preview.rs` (open, close, sync,
  wheel, tests).
- **Offered only while mdview is installed** (`sister::find`, looked up at settle and on focus):
  the bar button, View > "Preview in thi&s window", the tab menu's "Preview in t&his window",
  the palette and the two other preview commands are hidden, and the commands do nothing. Shown
  for Markdown, untitled and linked documents. The pane takes the split's place; a split ends.
- **The page moves only when the caret's line is out of view.** Front matter on line 1 shows
  from the top.
- An mdview too old for `--preview-server` (no greeting in 3 s) is stopped and the pane says to
  update mdview.
- `--preview-pane` opens the pane at start for `--trace`, `--type-steps` (adds a "preview" line)
  and `--shot`.

Measured on Linux under Xvfb, 1280x800, release builds:
| | main | renderer build | embedded build |
|---|---|---|---|
| First frame, no pane | 94 ms | 93 ms | 94 ms |
| Keystroke to pixels p50, pane open (3 runs, 300 keys) | 1.0-1.5 ms | 0.9-1.2 ms | 0.8-1.3 ms |
| Pane open to first page | | 23-37 ms | 19-23 ms |
| Keystroke to pane pixels p50 / p95 | | 3.7-5.0 / 5.5-10.5 ms | 2.2-2.7 / 2.8-3.9 ms |
| Private memory, pane open, all processes (762-line file) | 31.4 MiB (no pane) | 37.0 MiB (33.3 + mdview 3.7) | 36.1 MiB |
| Exe (Linux, information) | 12.57 MB | +0.06 MB | +1.71 MB |

The pane skips texts while an answer is awaited, so it shows about one keystroke in two
(embedded) or in three and a half (renderer) during fast typing; both stay inside one frame.
Not measured: scrolling the pane on its own, and the 100 MB file.

On a Windows runner (`preview-test` run 43875754540, a real window; slower than the PC, no
baseline without the pane was taken there):
| | renderer build | embedded build |
|---|---|---|
| First frame | 138-157 ms | 138-149 ms |
| Keystroke to pixels p50 / p95, pane open (3 runs x 2 files) | 3.0-3.1 / 3.6-3.8 ms | 2.3-2.4 / 2.7-2.9 ms |
| Pane open to first page | 40-50 ms (202 ms on mdview's very first start) | 20-25 ms |
| Keystroke to pane pixels p50 / p95 | 9.6-9.9 / 10.9-11.6 ms | 8.4-8.8 / 10.0-10.4 ms |
| Private memory, pane open | 36.2-36.7 MiB + mdview 5.7-6.9 MiB | 37.2-37.8 MiB |
| Exe | mdedit 6.58 MB | mdedit 7.91 MB |
There every keystroke reached the pane (300 answers of 300). The renderer build's 0.7 ms more
per keystroke in the editor itself is not explained yet (two processes on a small runner?).
The pictures of both builds are byte-identical. Nothing has been run by hand on Windows.

Downsides of embedding, as told to the owner: mdedit's `panic = "abort"` makes a renderer panic
end mdedit (the renderer build loses only the pane); larger build and exe, mdedit tied to the
viewer crates' revision and possibly drawing differently from the installed mdview; the
renderer's caches live in mdedit's memory; math and diagrams still need mdview's engine.

## After 0.7.1 (2026-11-16, in 1.0.0)

Shared crates at `d775493`. Owner's requests of 2026-11-16:
- **Input-method compositions are drawn** (underlined, the selected clause shaded) at the caret in
  the text, and in the find, palette and settings boxes: `View::draw_preedit`, `ime_preedit` in
  `app/input.rs`, shared `textfield::Preedit`. Tests in `app/ime_tests.rs`. Not yet tried with a
  real input method on any system.
- **macOS updates install themselves** (the owner changed decision 5 of the viewer's
  `docs/MACOS.md`): the zip is checked, unpacked with `ditto` and the bundle swapped whole, then
  the app restarts. `--install-update` does the same from a terminal; `tests/mac-update.sh` runs
  it in the Mac CI job against a local HTTPS server.
- **Pinned tabs**: "Pin tab" in the tab menu and the palette (`CommandId::PinTab`). Pinned tabs
  stay first, show a pin where the close button was (pressing it unpins), are passed by Close
  other tabs and Close all tabs, and Ctrl+W on one says to unpin it first. The session keeps them
  (`pin=` lines; a note without a file as `pin=#N`, restored with its unsaved text from the
  backup). Tests in `app/pin_tests.rs`. Budgets on Linux under Xvfb, 7 interleaved runs of
  `sample.rs`: first frame median 78 ms against 74 before (noise of the machine: 70 to 95 ms
  both), keystroke p50 1.08 against 1.09 ms. Not measured on Windows.

## START HERE: mdedit 0.7.1 is released (2026-11-15)

**The released state is `main`, mdedit 0.7.1, tag `v0.7.1` at `fbb3ac7`, published with mdview
0.5.1 on the owner's word. Shared crates: `main` of markdown-shared-rs at `d955a48`. 0.7.1 holds
the fixes from a hands-on test of 0.7.0 (`docs/release-notes/0.7.1.md`); the release record is in
`docs/RELEASE.md`. A plan for a macOS version of both applications is in the viewer's
`docs/MACOS.md`; the owner has given that work to someone else.** The sections below describe
0.7.0 and 0.6.0 and are kept for their detail.

Hands-on test of 0.7.0 on the Windows PC, 2026-11-15, with real mouse and keyboard (about 40
checks passed). Faults found and fixed in 0.7.1, each seen working in a real window afterwards:
- Menus opened by a right-click showed the access-key underlines at once: `right_press` built
  them with `Popup::new`; now `Popup::with_cues(.., ui.keyboard)`.
- The resize band (8 px) lay over the text's vertical scrollbar at the right edge of a window
  that is not maximized: `App::resize_band`, 3 px over a scrollbar track (`SCROLLBAR_EDGE`).
- Find in files: a document whose last match went never got its row back, and rows of edits
  discarded with "Don't save" stayed. The panel now remembers its scope and searches a tab in
  scope again, and re-reads a closed document from the disk (`results.rs`).
- Undo after Cut or Delete bookmarked lines did not bring the bookmarks back
  (`Ready::bookmark_undo`). A bookmark set after the command is lost by Redo and Undo again.
- Typing at several carets was one undo step per character (`Kind::MultiTyping` in the core).
- Text without a key name (KEYEVENTF_UNICODE) was dropped: `event_key` in `app/input.rs`.
- From the shared crates: the snap-layouts flyout on a window that is not maximized, and the
  normal size of a maximized window after side by side (see the viewer's handoff).
The test of Cut bookmarked lines with Undo leaves Cut out off Windows: the Linux CI machine has
no clipboard.

Passed in real windows: multi-caret commands, bookmarks across a restart, tables (also in a list
item), compare with the gutter arrows, column selection with padding, Vue and Svelte, find in
files with the results panel, crash recovery, a 5 MB file, and the basics. Not tested: "Compare
with file", the Files pane with an opened folder, find in open tabs, the light theme in every
menu, scales above 100 %. For the owner to judge, not changed: the bookmark click zone in the
gutter is about 5 px wide.

How the hands-on tests were done, for the next one: Sonnet workers drove the release builds
under `Desktop\md-wt` with real mouse and keyboard input from a PowerShell helper (SetCursorPos,
mouse_event, keybd_event, captures of the application's own window only), one worker at a time
because pointer and keyboard are shared. Lessons: computer use must not be used (a real Escape
key aborts the whole Claude session, twice; its open_application started the owner's installed
mdview), so Escape is posted to the window; the PC has a Turkish Q layout, so characters are
mapped with VkKeyScanEx; a click 60 ms after a pointer jump is ignored by menus, hover first;
every key and click checks that the foreground window is the build under test. The logs, the
captures and the helpers are local files in `C:\Users\kaank\Desktop\md-wt\control-tests\`
(not in any repository).

## START HERE: mdedit 0.7.0 is released (2026-11-15)

**The released state is `main`, mdedit 0.7.0, tag `v0.7.0` at `11c8d36`, published with mdview
0.5.0 the same day (the owner's instruction: finish the remaining items, then release). Shared
crates: `main` of markdown-shared-rs at `065cbdc`. What 0.7.0 added is in "After 0.6.0: gaps
closed" (its heading still says "not released": it was written before the release) and in
`docs/release-notes/0.7.0.md`; the release record is in `docs/RELEASE.md`. The owner has not
looked at any of the new interface work.** The text under the next heading describes 0.6.0 and
is kept for its detail.

Known open points: the line-ending and encoding menus have no access keys; the function list is
empty for Vue and Svelte; a column selection pads short lines on typing only, not on paste or
Enter; the compare arrows are not drawn with word wrap on; the Markdown menu ends at about 795 px
of an 800 px window, so it has no room for another item; the Linux copied-files paste and the KDE
blur were never run; the snap-layouts flyout was checked by window messages only. Not planned by
decision of the session, the owner may overrule: syntax-aware folding.

## After 0.6.0: gaps closed (2026-11-15, branch merge/next, not released)

**State: the branch `merge/next` holds everything below, on top of 0.6.0 (`main`). It is not
released and not in `main`. The owner looked at the shots and test builds and approved them.** Nothing here was seen or
used by a person: every "Not verified" line below stands. Four workers each built a set of gaps on
their own branch (`f/multicaret-more`, `f/panels-more`, `f/edit-more`, `f/adopt-shared`); the
branches were merged into `merge/next` (the access letters of the new compare and table items were
added at the last merge). Their notes lived in `docs/next/`; they are folded in here and the
folder is gone. The release notes for it are `docs/release-notes/NEXT.md`.

**Linux parts: never run.** Two pieces are in code that Windows does not compile: pasting image
files copied in a Linux file manager (`platform::clipboard_files`, `wl-paste` or `xclip`; not
compiled on the worker's PC, only by CI) and the KDE blur behind the frosted window (`sync_frost`
and `sync_surface_alpha` in `app.rs`; compiled by CI on Linux, whose Linux tests passed). Nothing
about KWin, the blur, the look, the focus rule or the paste was ever run or seen on Linux.

**Snap-layouts flyout: checked by window messages only, not by a real pointer** (see 2 below).

What was run on the Windows PC (release, `CARGO_TARGET_DIR` outside the checkout): `cargo fmt
--all --check`, `cargo clippy --locked --release --workspace --all-targets -- -D warnings`
(clean) and `cargo test --locked --release --workspace` at the merged head (304 passed and 1
ignored in the mdedit bin, 492 passed and 10 ignored in mdedit-core, the rest unchanged; no
failures). The workers ran the same on their branches. No worker or session drove a real window by
mouse or keyboard for any item below unless a line says so. Interface shots were rendered by hand
and looked at (listed per item).

### Menu fit and access letters, looked at in shots (this pass)
`--menu view` with `--compare` (the View menu while two files are compared) and `--menu
text-markdown` on `fixtures/table.md`, 1280 x 800, scale 1, light: both menus fit the 800 px
window (the View menu ends at about y 760, the Markdown menu at about y 795, 5 px to spare, so it
has no room for another item). Every item of each has its own underlined letter. View: F, l, h, W,
M, d, G, i, x (Next), u (Previous), y (Copy this change), k (Take), C (Copy this line), S, n, b, a,
Z, o, R. Markdown: B, I, C, S, H, L, A, r, n, D, u, w, g, f, e, M, v, y. This was one shot each, light
only, not a real window.

### Measurements owed by the plan (2026-11-15, this pass)
Release `mdedit.exe` from the merged head plus the one fix below, started from Git Bash with
`MDEDIT_SETTINGS` in a new empty folder per run (with `check_updates=0` in it), a PowerShell script
polling for a visible top-level window of the process (wider than 300 px); the time is from the
process's start time to that window; working set and private bytes 3.0 s after start; 5 runs each.
The same figures cross-checked with `--trace` ("first frame after"). The PC was otherwise idle.

| | Window shown (median of 5, runs) | Working set | Private bytes | Budget (docs/PLAN.md) |
|---|---|---|---|---|
| Empty document, after the fix | 50 ms (48 to 53) | 27.8 MiB | 12.5 MiB | none set for empty |
| Empty document, before the fix | about 435 ms (`--trace`, 5 runs 431 to 437) | 27.8 MiB | 12.5 MiB | none set for empty |
| `fixtures/sample.rs` (small file) | 85 ms (84 to 88) | 44.7 MiB | 27.8 MiB (27.5 to 27.9) | first frame under 100 ms: met; private under 30 MiB: met, 2 MiB to spare |
| 5 MB text file (`gen lines 5`) | 61 ms (61 to 66) | 38.8 MiB | 21.4 MiB | no budget for this size |
| Exe size | 6,481,408 bytes (6.18 MiB) | | | not a gate since 2026-11-14; was 5.8 MB at phase 3, so 0.7 MB more since |

Not measured: keystroke and scroll timings, idle CPU, the 100 MB file, memory after typing, any
run on a second machine, the effect of Defender or a cold disk cache (the runs are warm). The
numbers are from this one PC.

**A fault found and fixed here (one change, `Docs::wait_load` in `bins/mdedit/src/docs.rs`):**
when mdedit starts with no file and no restored tab, the hidden first frame waited the whole
`LOAD_WAIT` (400 ms) for a load that was never going to come, so the window appeared at about 435
ms. It has been so since phase 1. Now the wait ends at once when no tab is loading (4 lines); the
empty start shows at about 50 ms. A start with a file or a restored session is unchanged (a tab is
loading, so the wait is as before). The full tests were run after it, not a real hand start.

### Speed against size build comparison (2026-11-15)
The plan's carried-over item: the current profile (`opt-level = 3`, fat LTO, `panic = "abort"`,
the `mdedit` and `mdedit-outline` crates at `opt-level = "s"`) against `mdedit` and `mdedit-outline`
at `opt-level = 3` and `panic = "unwind"`. Built with `--config` flags into
`target-edit-size`, `Cargo.toml` unchanged. Results (same PC, same method as above, 5 runs each, medians): exe 8,759,296 bytes against 6,481,408 (2,277,888 bytes, 35 % larger). Window shown: empty document 57 ms against 50, `sample.rs` 89 ms against 85 (88 to 100 against 84 to 88), 5 MB file 60 ms against 61. Working set and private bytes the same within 1 MiB (sample.rs 45.2 / 27.7 MiB against 44.7 / 27.8). Keystroke to pixels on `sample.rs` (`--type-steps 300`, three runs each): p50 0.50 to 0.51 ms for both, p95 0.55 to 0.67 ms against 0.54 to 0.59. Scroll was not compared. The speed build is no faster in anything measured and is slower to start by a few ms, and it is 2.3 MB bigger. **Recommendation: leave the profile as it is** (the `mdedit` crates at `opt-level = "s"`, `panic = "abort"`); nothing measured argues for the change. The `Cargo.toml` profile was not changed. Not compared: the 100 MB file, scroll, idle CPU, a second machine.

### What was built, where it lives, what is not verified

**Multiple carets and bookmarks (`f/multicaret-more`).** Typing path: nothing added for a single
caret (every new branch is behind `view.extra` being non-empty or runs on its own command or
click). Looked at: the text menu and its Lines submenu (light; Lines dark rendered, not looked at).
- Select next occurrence, Ctrl+Shift+D (`multi::select_next` in core, `Ready::select_next`, command
  `SelectNextOccurrence`; text menu after Select all, palette). A caret selects the word at every
  caret; a selection adds the next exact-case match after the primary, wrapping, skipping selected
  ones; over 4096 bytes of selected text it does nothing; a toast when there is nothing to add.
  Case sensitive, not whole word. Ctrl+D stays Duplicate line. Not verified: the key in a real
  window, Turkish layout; the menu entry was seen in a shot only.
- Ctrl+click removes a caret (`multi::remove_at`, `Ready::remove_caret`, `input.rs`): a click in or
  touching a caret or selection removes it when there are two or more. Not verified: the mouse
  click itself (only the functions are tested).
- Ctrl+Backspace and Ctrl+Delete at every caret (`MultiOp::WordLeft/WordRight` in `multi::edit`):
  one undo step; `multi::edit` now merges any run of overlapping removals; the random test against
  a model passes. Short-row carets of a column selection are left alone. Not verified: in a real
  window.
- Outdent (Shift+Tab) at every caret (`LineOp::Outdent` in `multiline`): one undo step. Not
  verified: in a real window; Markdown tables with several carets (Shift+Tab in a table moves
  between cells only with one caret, as before).
- Click in the gutter toggles a bookmark (`Hit::Bookmark(line)`, the left 14 px of each shown
  line's gutter; the fold cell wins where they overlap; not registered for the second pane nor in
  band repaints). Not verified: the click in a real window (`toggle_bookmark_at` is tested), the
  cell over wrapped continuation rows, a second pane.
- Cut, copy, delete bookmarked lines (`lines::marked_lines_text`, `lines::delete_marked_lines`,
  `EditOp::DeleteBookmarked`; palette and the end of the Lines submenu, no keys; greyed with no
  bookmarks). Bookmarks are cleared after Cut or Delete; Undo brings the text back, not the
  bookmarks. Not verified: Cut and Copy against the real clipboard, the menu clicked.
- Bookmarks kept across closing a file and across runs: see `marks.rs` below (built by the panels
  worker; the multicaret worker skipped it, as a design decision of its own).

**Results panel, minimap, bookmarks (`f/panels-more`).** Cost: with the panel closed
`results_sync` returns at its first line; with it open one pass over the open tabs runs after each
event, and a search on a worker only after a tab with listed matches has been left alone 300 ms.
The session signature gained a few small values. Looked at: the panel with the keyboard in it, at a
dragged height, the folder and kinds boxes (light and dark), the minimap at 160 px, the Settings
View page. Not run: the whole `tests/shots.sh` set (its large inputs need Python, which that PC
only had as the Store alias), CPU, memory and keystroke timings.
- Keyboard in the results panel (`Results::focused`): F6 ("Go to the results"), the key that
  searched, or a click; Up, Down, Home, End, PageUp, PageDown, Enter (opens the row, keyboard back
  to the text), Escape (back to the text, panel stays open); the panel's top line is in the accent
  colour while it has the keyboard. `--results-focus` renders it. Not verified: keys and click in a
  real window; Turkish layout.
- Resizable panel: the top edge (a 5 px strip) is dragged; 110 px at least, the text keeps 140 px;
  kept in the session as `results_h` (`Areas::with_results`, `dragged_height`). Not verified: the
  drag in a real window (only compiled; the limit function is tested); a window resized while the
  panel is dragged tall.
- Results follow edits: rows of an edited document go faint ("updating..." in the title; a click or
  Enter says the document changed), and 300 ms after the last change it is searched again on a
  worker from a cheap snapshot. For Find in open tabs and for folder-search rows whose file is open.
  Stale rows are not adjusted; a file that lost all its matches gets no row back (search again).
  Replace in files still plans from disk. Not verified: in a real window; many tabs with thousands
  of matches.
- Folder and kinds for Find in files: two boxes in a 34 px row under the title after a folder
  search (folder, empty is the open folder, a relative path counts from it; kinds such as
  `*.md;*.txt`, `*` and `?`, case blind, `.md` means `*.md`, `!*.min.js` leaves names out); Enter
  searches again, Tab moves between them; kept in the session (`find_folder`, `find_kinds`).
  A missing folder gives a toast and keeps the old rows. Core: `search::NameFilter`,
  `search::in_tree_named`. No folder picker dialog. Not verified: typing in the boxes in a real
  window; a folder on a network drive; the `replace-files-*` shots changed by the extra row and
  were not compared with the earlier set.
- Minimap: already had marks (selections, find matches, bookmarks, caret row) and a column per
  split pane. New: width setting, Settings > View > "Minimap width" (48 to 240 px, default 96,
  `Prefs::minimap_width`, `live::minimap_width`, `Areas::with_minimap_width`). Not verified: the
  setting changed in a real window; an odd width at scale 1.5.
- Bookmarks across closing and runs (`marks.rs`): the lines (0-based) of the 50 latest files, at
  most 100 each, in the session as `mark=1,4;path`; put in the list when a tab closes and when the
  session is written, handed to a file the first time it is read in a window (not to a reload),
  keeping only lines that still exist. A crash loses bookmarks set since the file was opened;
  lines are not moved if the file changed on disk; they apply by path. Not verified: a real close
  and restart; Windows path case differences beyond `same_path`.

**Compare, column selection, tables, Linux file paste (`f/edit-more`).** Typing path: a
`view.column` check per edit and a pad computation only when a column selection is typed on;
compare arrows are found per visible row only while two files are compared. Looked at (light, the
dark Markdown menu too): compare with arrows, the View menu while comparing, the Markdown
submenu, a column typed past short lines. Not run: a real mouse or keyboard session, Linux.
- Compare copy arrows in the gutter (`Hit::CopyChange { other, hunk }`, the leftmost 14 px of the
  gutter at the first row of each difference; `compare::pick_hunk`, `App::compare_copy_hunk`, the
  same `EditOp::CopyLines`, one undo step; `Align::hunk_starting_at`, `hunk_ranges`). Only while
  the diff is current, not with word wrap on; the arrow hides that line's bookmark dot and a press
  on the cell copies instead of toggling it. The View menu shows the commands that start a
  comparison when nothing is compared and the ones that work on one (next, previous, copy, take,
  copy line, stop) while one runs: with the new item it was 8 px too high for 800 px. Not
  verified: the click with a real mouse (the function behind it is tested), wrapped text, a second
  pane on the left (the direction there is by `other_left`, seen in no shot).
- Copy this line to the other side (`CopyLineAcross`, `Align::partner_of`, `compare::pick_line`;
  palette and View menu while comparing, no key): the caret's line replaces its partner in a
  changed block; surplus lines and inserted or deleted blocks have no partner (greyed, a toast
  says to copy the whole change). Not verified: in a real window.
- Column selection, virtual space (`multi::column_pads`): typing on a column selection that is
  still the one made pads short rows with spaces up to the rectangle's left column, one undo step;
  only typing pads (not paste, not Enter); the caret of a short row is still drawn at the line's
  end. The test that expected `xX` on the short line `x` now expects `x X`. With word wrap the
  selection already worked on logical lines and columns (no code); a mouse drag past the end of a
  wrapped line does not extend the width, the keys do (DESIGN section 57). Not verified: with the
  mouse (the shot used `--select-column` and `--type`).
- Tables in list items (`markdown::locate`, `Row::marked`): a table whose header is `- | a | b |`
  (also `*`, `+`, `1.`, `1)`, inside a quote too) is edited with the marker and indentation kept
  by Tab, Shift+Tab, Enter, Align table and the row and column commands; a later `- | x |` line
  ends the table. Not verified: in a real window; a table in a list item nested in a deeper one
  (only the delimiter row's indentation is followed).
- Sort table by this column (`TableOp::SortColumn`; Markdown submenu and palette, no key):
  ascending, descending when already ascending; numbers when all cells parse as numbers, else text
  without regard to case; equal rows keep order; one undo step; needs two body rows. Numbers with
  thousands separators or units compare as text. Not verified: in a real window.
- Move row up and down (`TableOp::MoveRowUp`, `MoveRowDown`; Markdown submenu after Delete row,
  palette): body rows only. Not verified: in a real window.
- Pasting image files copied in a Linux file manager (`platform::clipboard_files`,
  `image_paste::parse_uri_list`; `text/uri-list`, then `x-special/gnome-copied-files`; `arboard`'s
  file list was not used because it would need a new dependency). **Not compiled and not run on
  Linux by the worker**; `parse_uri_list` is tested on Windows; with neither tool installed nothing
  is found. CI compiled it (see CI below).

**Adopting the shared crates (`f/adopt-shared`).** The shared crates are repinned from `6249cd1`
to `065cbdcc19226ecb77dd85f85ca922f53f2d3aee` (the shared branch `f/linux-frost`, with `681baf8`);
only the rev changed in `Cargo.toml` and the lock; `cargo build --locked` works.
- Vue and Svelte: `.vue` and `.svelte` open highlighted with no code of ours
  (`Language::for_path`); `OPEN_FILTER` in `bins/mdedit/src/platform.rs` gained them; the function
  list has no rule for them and stays empty. Test: `docs::tests::vue_and_svelte_files_open_with_their_language`.
  Not verified: how a real component looks; the dialog's filter in a real dialog.
- Snap-layouts flyout (`App::sync_snap` in `app.rs`, after every composed frame; `chrome::snap_button_rect`:
  the maximize button plus the shadow margin, minus the top resize band unless maximized; `None`
  while a dialog, the palette or the settings form is open). Run in a real window (Windows 11 build
  26300, 96 dpi, started from Git Bash with `MDEDIT_SETTINGS` in a new folder): `WM_NCHITTEST` at
  the maximize button answers 9 (`HTMAXBUTTON`), and 1 (`HTCLIENT`) at the close and minimize
  buttons, in the tab strip and in the top band above the button; posted client mouse messages
  (move, press, release) on the button maximize the window; posted non-client messages
  (`WM_NCMOUSEMOVE`, `WM_NCLBUTTONDOWN`, `WM_NCLBUTTONUP` with `HTMAXBUTTON`) restore and maximize
  it again. **Not verified: the flyout itself and snapping (needs a real pointer); other scales;
  touch; that the dialog case really answers `HTCLIENT` in a window.**
- Menu access keys: the main menu and its View, Macro and Recent submenus, the tab, text (Lines,
  Markdown, Convert), find-box and minimap menus read `&` marks (`.access_keys(true)`), one letter
  per item within a menu (found by a small search, preferring word starts); the Recent menu's file
  names and saved macros' names go through `escape_access` (no letter picks them). The completion,
  line-ending and encoding menus are not opted in. In `key_pressed`'s open-menu branch (after Up,
  Down, Tab, Enter, Space, Left, Right and Escape) a printable character without Ctrl or Alt picks
  the item (`access_key`, `choose(act, true)` as Enter does; a submenu opens with the keyboard on
  it); the completion list still takes typing first. Underlines show when a popup is opened by
  keyboard and are hidden when opened by mouse (`Ui::keyboard`, `Popup::cues`); any key used in an
  open menu shows them. Tests in `app/access_tests.rs` and `app/input.rs` (marks unique and
  complete for every fixed menu; the Recent menu with `R&D notes.md`; a capital works; Ctrl or Alt
  does nothing). Looked at: `docs/captures/access-keys/main-view-menu.png` (dark), and in this pass
  the View menu while comparing and the Markdown menu (above). Not verified: the menus in a real
  window by keyboard and mouse; the light theme in every menu; Windows' habit of showing underlines
  only after Alt (here: after any key).
- Frosted chrome on KDE Plasma (Wayland), non-Windows code only: `sync_frost` passes
  `mdshared_ui::frost::BlurSystem(&window)` off Windows; `sync_surface_alpha` calls
  `surface.set_premultiplied_alpha(frost.wants_alpha() || shadow.wants_alpha())` when the frost
  changes and once after the surface is made. Nothing else was needed (the app has one frame copy
  for every system and already keys out the marked pixels). **Never run or seen on Linux; only
  compiled, by CI.**
- CI of the adopt-shared branch (`gh workflow run ci.yml --ref f/adopt-shared`, run 43732981608):
  the windows and linux jobs both passed.

**Decided not to build.** Bookmarks "SKIPPED" by the multicaret worker were built by the panels
worker (above). Syntax-aware folding is not planned (folding stays by indentation). Crash
reporting, accessibility, input methods (IME), plugins, FTP and project panels and printing were
set aside by the owner earlier and are not planned.

## START HERE: mdedit 0.6.0 is released (2026-11-15)

(Since this was written, `merge/next` has gone on past it: see "After 0.6.0: gaps closed" above.)

**The released state is `main`, mdedit 0.6.0, tag `v0.6.0` at `c9d05b1`, published with mdview
0.4.0 on the owner's word; see "Release 0.6.0" below and `docs/RELEASE.md`.** The text that
follows was written while the work was in progress: nothing is in progress now, and every
branch it names is merged.

**State at the end of 2026-11-15 (read this first).** Every feature of the next-release batch is
finished and merged on `merge/next-release`, here, in the viewer and in the shared crates; the
sections below are the record of how it got there. All `mdshared-*` pins name the head of the
shared branch `merge/next-release` (`11563a3`: shared `merge/editor-next` and `merge/viewer-next`
merged, plus a taller single-page settings form for the viewer); mdview's `merge/next-release`
pins the same commit and holds the viewer side of live preview of unsaved documents, finer sync,
the window backdrop and Restart. At this head on the Windows PC: fmt and clippy `-D warnings`
clean, 806 tests pass, release build made; a system close request (`WM_CLOSE`) closed the window
in 0.3 s. CI passed on Linux and Windows at this head
(editor run 43686313531, shared run 43686315745, viewer run 43689394395). Left to the owner: looking
at the interface, a real update followed by Restart, Linux, then merging `merge/next-release`
into `main` in the three repositories, the version numbers and the tags. Later on 2026-11-15 the owner asked
for the merge: `main` was fast-forwarded to `merge/next-release` in the three repositories
(shared `11563a3`, viewer `14aedec`, editor `d56c4ee`) and pushed. Nothing was bumped, tagged or
published.


The released state is `main` (mdedit 0.5.1). Update 2026-11-15: five of the features below are
merged together on the branch `merge/next-release` (see the table and the review results below);
that branch is not merged into `main` and nothing is released. The rest of this paragraph is as
written on 2026-11-14. A large batch of next-release work was started with many
agents in parallel and then **paused by the owner** (the PC could not carry the load and the token
cost was too high). Everything is pushed; nothing below is merged into `main`, nothing is released.
The owner continues from a fresh clone on Windows. Read this section, then
`docs/next-release/SPECS.md` (the rules every feature follows and one spec per feature).

### How to continue

- One agent at a time, two at most. The owner asked for fewer agents that keep their context and
  take the next task, not a fresh agent per task, and no separate reviewer agents: the main session
  reviews every diff and every screenshot critically before a merge ("app is really good, let's
  not ruin it").
- Local builds are allowed again (the earlier "do not build locally" was lifted on 2026-11-14).
  Keep one shared `CARGO_TARGET_DIR` per repository if several worktrees are used: twenty worktrees
  with their own `target` filled the disk.
- CI: push the branch and run `gh workflow run ci.yml --ref <branch>`. Do not poll in a loop and do
  not use `gh run watch` from many agents: the account's GitHub API quota was exhausted once.
- Each feature branch carries `docs/next/<key>.md` (README text, DESIGN text, release note,
  handoff notes) instead of edits to the real docs, so branches merge without doc conflicts. The
  merge folds that file into README, DESIGN, PLAN, HANDOFF and `docs/release-notes/NEXT.md` and
  deletes it. Branches whose state is WIP may not have it yet. The five files of the branches
  merged into `merge/next-release` were folded on 2026-11-15 (DESIGN sections 57 to 61,
  `docs/release-notes/NEXT.md`, the section "Five features merged on `merge/next-release`" below);
  those of the four merged in the third session were folded the same day (DESIGN sections 65
  to 67, "Four more features merged").
- Merge order into `next`: shared branches into the shared repo's `next` first, then the app
  branch; set every `mdshared-*` pin to the shared `next` head; keep third-party crates in
  `Cargo.lock` as on `main`. Central tables (menus, commands, keys, strings, settings rows,
  `tests/shots.sh`) conflict textually: take the union. Check shortcut clashes after each merge.
- Reserved shortcuts: word completion Ctrl+Space; column selection Alt+Shift+arrows and Alt+drag;
  find in folder (viewer) Ctrl+Shift+F; presentation (viewer) F5; history (viewer) Alt+Left/Right.
- Before any release: the owner looks at the interface; a real update followed by a real Restart is
  tried by hand on Windows and from an AppImage; budgets in `docs/PLAN.md` are measured (nothing in
  this batch was measured).

### State of every branch

"CI green" means the `ci` workflow passed on Linux and Windows for that exact head. "WIP" means the
last commit is an interrupted agent's uncommitted work, committed as it was: it may not compile.
No branch below was reviewed by the main session unless it says so.

| Branch | Feature | Head | State |
|---|---|---|---|
| `merge/next-release` | the six features marked "merged" below, `f/paste-image-files`, and (third session of 2026-11-15, not seen by CI or the owner) `f/live-preview-unsaved`, `feat/restart-after-update`, `f/frosted-window`, `f/finer-sync`, merged together | see `git log` | CI green at `95edf8d` on Linux and Windows, not read for the head; reviewed by the main session; the owner tried it by hand on Windows (see "Where the session of 2026-11-15 stopped"); not merged into `main`; nothing released |
| `f/paste-image-files` | paste image files copied in the file manager as links | see `git log` (finished in the second session of 2026-11-15) | finished on Windows: clippy clean, all tests pass, tried in a real window by script (see "Second session of 2026-11-15"); merged into `merge/next-release` on the owner's word ("Merge it", 2026-11-15) without a hand test by the owner; a CI run was started at `b5185e9` (run 43651515645) and its result was not read; Linux not done |
| `next` | rounded corners (direct on `next`) | `c2ea815` | CI green; superseded by `f/rounded-corners` |
| `f/rounded-corners` | rounded corners on top of `merge/next-release`, pins shared `f/rounded-corners` | see `git log` | CI green on Linux and Windows; merged into `merge/next-release` on the owner's word (2026-11-15); never seen on a Linux desktop; see "Rounded corners, second session" |
| `feat/restart-after-update` | Restart button after an update | `886625a` | CI green; reviewed; merged into `merge/next-release` in the third session of 2026-11-15 with the bar reworded; the relaunch was tried by script, a real update and Restart never (see "Four more features") |
| `f/table-editing` | Markdown table editing help | `88d5503` | CI green; reviewed; merged into `merge/next-release` |
| `f/multicaret-line-commands` | multi-caret in the line commands | `aaba1fa` | CI green; reviewed; merged into `merge/next-release` |
| `f/compare-copy-across` | copy a change across in compare | `4d4ff2b` | CI green; reviewed; merged into `merge/next-release` |
| `f/paste-image` | paste an image into a Markdown file | `c14ebb3` | CI green; reviewed; tried on Windows by script; merged into `merge/next-release` |
| `f/autosave-recovery` | autosave and crash recovery | `4750c39` | CI green; reviewed; merged into `merge/next-release` (with the data-loss fix `867f197`) |
| `f/column-selection` | column (block) selection | `3785c7d` | CI green; reviewed; merged into `merge/next-release` |
| `f/word-completion` | word completion and snippets | see `git log` (finished in the second session of 2026-11-15) | CI green on Linux and Windows at `7b91273` (run 43652712584); merged into `merge/next-release` on the owner's word ("Merge it", 2026-11-15); the owner has not looked at the list (see "Word completion, second session of 2026-11-15") |
| `f/live-preview-unsaved` | live preview of a never-saved file (both apps) | see `git log` | finished on Windows; merged into `merge/next-release` in the third session of 2026-11-15; the viewer's side is on its branch of the same name, **not merged there** |
| `f/finer-sync` | finer scroll sync inside long blocks (both apps) | see `git log` | finished on Windows; merged into `merge/next-release` in the third session of 2026-11-15; the viewer's side (most of the work) is on its branch of the same name, **not merged there** |
| `f/frosted-window` | Window backdrop setting (Mica, Acrylic; Windows 11; off by default) | see `git log` | finished on Windows; merged into `merge/next-release` in the third session of 2026-11-15; the owner has not looked at it |

"CI green" on a feature branch may be from an earlier interrupted agent's state of the feature:
read the diff against the spec before trusting that the feature is complete.

### Review results of `merge/next-release` (2026-11-15)

The main session reviewed the diffs of the five branches before the merge.

- Autosave had a data-loss bug: after a crash an older session backup beat the newer recovery
  copy. Fixed in `867f197` (DESIGN section 61); the pruning of stale backups was then changed to
  borrow what it reads of a tab (`9f07121`).
- `next` (rounded corners) is left out. The shared test
  `rounded_paint_touches_the_margin_and_the_corner_squares_only` fails on shared `next` `e3d381b`,
  and the Round default would move KDE to the client-margin shadow route.
- `feat/restart-after-update` waits for a hand test on Windows (`f/paste-image` had one by script, see below, and was then merged).
- Three small review fixes were made in `e8dd045` (the merge of `f/table-editing`): in compare, a
  copied change takes in the diff's freshness first, so a command run right after an edit cannot
  work from line ranges of the older text; for tables, Enter at the end of an empty row ends the
  table only on its last row (in the middle it adds a row), and a row or column command refused
  inside a table says so, not that the caret is outside one.
- Budgets, measured on this Windows PC on 2026-11-15 (release builds, `main` `5cd3dff` against
  `merge/next-release` `9f07121`, 7 interleaved warm runs each, min / median): first frame of
  `sample.rs` 74.5 / 74.8 ms against 74.2 / 75.6 ms; first frame of the 100 MB file 133.5 / 136.3 ms
  against 130.8 / 135.8 ms; private memory 27.4 / 27.6 MiB against 27.2 / 27.8 MiB (small file) and
  133 MiB both (100 MB file, over its 125 MiB as before); keystroke p50 0.52 against 0.49 ms; scroll
  p50 0.45 against 0.40 ms; exe 6,121,984 against 6,228,992 bytes (+107,008). Nothing moved by more
  than noise. A recovery copy was seen written and removed on a normal exit; its CPU cost was below
  what the 15.6 ms clock of `--trace` shows. Not measured: the write of a large modified document.
- Screenshots of the green run (Windows artifact) were looked at for every shot the five added:
  nothing found. The View menu in a comparison now fills the whole height of an 800 px window
  (`menu-view-compared`); the Markdown submenu has 15 rows. The owner has not looked at them.
- `f/paste-image` was tried in a real window on Windows by script (clipboard set with PowerShell,
  Ctrl+V and Ctrl+S sent): a bitmap and a PNG-format clipboard both saved a correct 40x30 PNG
  beside the file and inserted the link; a non-Markdown file and an untitled document got nothing.
  Not tried: an image from a browser or the Snipping Tool, Linux, a person's hand. The owner then
  said to merge it: it is in `merge/next-release` (DESIGN section 62). Review notes: on Linux a
  paste with no text on the clipboard runs `wl-paste` or `xclip` on the UI thread (up to 3 s
  each); on Windows the saved PNG can carry a few padding bytes after its end. New direct
  dependency `png` 0.18, already in the build through `mdshared-text`. No shot and no app-level
  test (the only new visible state is a toast).
- `feat/restart-after-update`: a real update and Restart could not be staged on one PC. The shared
  updater refuses a download address that is not https (and then the whole manifest), and the
  newer build needs a higher version number. By hand: an installed copy of this branch's build, a
  second build with a higher version on an https host, a manifest naming it with its sha256,
  `MDEDIT_UPDATE_MANIFEST` pointing at the manifest, then Install and Restart.
- Rounded corners, a correction (owner, 2026-11-15): on Windows 11 the released window already
  has the system's rounded corners (it is undecorated but resizable, and the system rounds such
  windows). So on Windows the `next` branch only adds the setting (square, small); the work and
  the two reasons it is left out are on Linux.
- Tried by the owner on 2026-11-15: see the next section.

### Where the session of 2026-11-15 stopped

The owner stopped the session and asked for this handoff. Start from `merge/next-release`.

**The owner tried a release build of `95edf8d` by hand on Windows** (scratch settings, copies of
the fixtures) and reported, in these words: "tabbing creates new rows. right click markdown menu
works. column works. line commands work. compare works, not sure about f7. paste image works, but
not from a file browser. crash rec works."

Open from that report:

- **"not sure about f7"** was not followed up. F7 is Next difference (`App::compare_jump`, in
  0.5.0 already): the caret goes to the start of the next difference, and a toast says when there
  is none below. Nothing was found wrong in the code. Ask the owner what was seen: no movement, or
  a movement that is hard to notice (only the caret moves when the difference is already on
  screen). A thought, not agreed: show "3 of 7" in the status strip, or flash the block.
- **"tabbing creates new rows"** was taken as the feature working (Tab in the last cell of the
  last row adds a row). Not confirmed that it was meant so: if Tab adds a row from any other cell,
  that is a fault.
- **"not from a file browser":** copying an image file in Explorer puts a file list on the
  clipboard, not pixels, and the merged feature left that out. Work on it was started and is
  unfinished on the branch `f/paste-image-files` (`5c5af06`, one commit on top of
  `merge/next-release`), not agreed in detail with the owner:
  - Built: `platform::clipboard_files` (Windows, `CF_HDROP`, parsed by
    `image_paste::parse_dropfiles`; empty elsewhere), `clipboard_formats` (the one place that
    opens the clipboard, now also used by `clipboard_png`), `image_paste::is_image_file`,
    `relative_path`, `link_to_file`, `App::paste_image_files`. The file list is looked at before
    the pixels. Every file must be an image by its extension, else nothing happens.
  - Decision taken without the owner: a file is linked where it is, by its path from the
    document's folder; a file with no such path (another drive, a network path) is copied beside
    the document under its own name and linked there. Several files go in one to a line.
  - State: `cargo clippy -D warnings` clean and the editor's tests passed (232) on this PC; the
    core tests and the release build were cut short by the stop; no CI; never tried with a real
    clipboard; no docs. Linux is not done (a copied file is not looked for there).

**Second session of 2026-11-15: `f/paste-image-files` finished and merged into
`merge/next-release`.** The owner said only "resume"; the session took the one item that needed
no answer, reported, and the owner answered "Merge it" (a fast-forward). The owner has not tried
it by hand.

- Review of `5c5af06` found one gap, fixed: a file on the same network share as the document was
  copied instead of linked (`image_paste::Anchor`: a drive, a share, or the root). Nothing else
  was changed in the code.
- Run on this PC: `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo test
  --workspace` all pass (editor 232, core 446, the rest); release build 6,282,240 bytes.
- Tried in a real window on Windows by script (the file list set with PowerShell `Set-Clipboard
  -LiteralPath`, which writes `CF_HDROP` as Explorer does; Ctrl+End, Ctrl+V, Ctrl+S, Alt+F4 sent):
  three files (a subfolder with a space in the name, a sibling folder, drive D:) gave
  `![](img/my%20pic.png)`, `![](../pics/b.jpg)`, `![](far.png)` on three lines, with `far.png`
  copied beside the document byte for byte; the same D: file again in a second document gave
  `far-1.png`; an image together with a `.txt` did nothing; an image file into a `.txt` document
  did nothing.
- Not run: a copy made in Explorer by hand; the toast was not looked at; a network share (unit
  test only); a read-only document; an unsaved document; CI; Linux (not built there, and the
  feature is Windows only); budgets (the new code runs only on a paste with no text on the
  clipboard).
- Docs: README, DESIGN section 62 and `docs/release-notes/NEXT.md` are edited on the branch
  itself (no `docs/next/` file).
- Decisions still the owner's: a file is linked where it is, however far (`../../Pictures/a.png`),
  and only a file on another drive or share is copied; a list with one non-image in it pastes
  nothing and says nothing.
- A leftover of the test: the folder `D:\mdedit-paste-test` (one 185-byte PNG). Its removal by
  the session was refused by a safety check; the owner can delete it.

**Word completion, second session of 2026-11-15 (branch `f/word-completion`, merged into
`merge/next-release` by a fast-forward on the owner's word).** The owner chose it as the next
feature. The interrupted commit `0a09e78` turned out to be nearly
whole: after merging `merge/next-release` into it (three textual conflicts, both sides kept) it
compiled, clippy was clean and one test failed (a count of settings lines).

- Reviewed by the main session: `mdedit_core::complete` and `snippets`, `app/completion.rs`,
  `view/point.rs` and the wiring. Changed: the settings line count in the prefs test; "Open
  snippets file" taken out of the main menu (with it the menu no longer fitted an 800 px window
  and scrolled; the command stays in the palette); the shot `complete-above` given a window short
  enough that the list really goes above; an ignored test that measures the cost of a list.
- Run on this PC: clippy clean; all tests pass (editor 247, core 469). Budgets, release build,
  five runs: first frame of `sample.rs` 70.5 to 73.3 ms, private memory 27.5 to 28.0 MiB,
  keystroke p50 0.53 ms: as before the feature (not an interleaved comparison). Exe 6,341,632
  bytes (+59,392). A list at every limit opens in 34 ms (DESIGN section 63).
- Shots looked at by the session (light and dark where both exist): the word list, the snippet
  row, the snippet taken, the list above the caret, the Settings row, the text menu. Nothing found.
- Tried in a real window on Windows with sent keys: Ctrl+Space, Down, Enter took the second word;
  Escape closed the list and Enter was then a plain Enter; the `table` snippet was filled in over
  four stops with Tab; with `complete_auto` on, the list opened on the third letter and Tab and
  Enter took words; with it off, Tab after three letters was a Tab.
- Alt+/ did not open the list with sent keys: this PC's first layout is Turkish Q, where `/` is
  Shift+7, so the chord cannot be pressed there. It is kept for layouts with a bare `/` key
  (the reason for it is Linux, where an input method can own Ctrl+Space). The owner may want
  another second shortcut, or none.
- Not run: the owner's look at the list; a mouse click on a row in a real window (test only);
  a split view; a real `snippets.toml` by hand (tests only); Linux; CI result.
- Decisions taken without the owner: no list with several carets; the list reads the text each
  time it opens and keeps no index; snippets ship for Markdown only; `snippets.toml` beside the
  settings file; the docs are edited on the branch itself (README, DESIGN section 63, NEXT.md).

**Rounded corners, second session of 2026-11-15 (branch `f/rounded-corners` in the editor and in
the shared repository; merged into `merge/next-release` on the owner's word, "Merge it",
after CI passed on Linux and Windows for both: editor run 43653711348, shared run 43653504034).** The owner said: "test rounded corners with ci for now."

- The editor's `next` (two commits) is merged onto `merge/next-release` as `f/rounded-corners`;
  three docs conflicted and both sides were kept (the corners are DESIGN section 64 now, 63 is
  word completion on its own branch).
- Shared `f/rounded-corners` = shared `next` (`e3d381b`) plus two commits, pinned here by rev:
  1. The test that failed CI was wrong, not the drawing: it wanted an alpha above 20 at the
     outermost pixel of each corner, and the top corners sit over the faint side of a dropped
     shadow (15 at scale 1). It now checks partial alpha, no colour, no step from the margin
     pixel outside it, and the bottom stronger than the top. Shared CI green on Linux and Windows
     at `8e75bff`.
  2. A fault found in the `--frame` pictures, in 0.5.0 already and not from the rounding: the
     client-drawn shadow's edge bands kept their full strength up to the window's corners and
     met the corner tiles with a visible step (alpha 37 beside 9, 60 beside 25). The ends of each
     band now take the other edge's coverage (`shadow::client::Ends`, `c8b7d97`); a test checks
     every band against its corner tiles at each depth. The eight tiles themselves, and so the
     KWin route, are unchanged: KWin stretches the same edge tiles between the same corner
     tiles, so the step is probably there on KDE's own route too. Not looked at.
- Run on this PC: shared and editor clippy clean, all tests pass; the `--frame` shots (light,
  dark, scale 2, square) and the Settings row looked at by the session before and after the fix.
- Not run: any Linux desktop (the client route only draws there); Windows 11 with the three
  choices in a real window; the CI results of the second shared commit and of this branch.
- Still open, the owner's: with Round as the default, `Auto` takes the client-margin route on KDE
  instead of KWin's (the corners need an alpha channel there). The viewer has no branch for this
  yet.

**The owner's answers (2026-11-15, second session).** F7: "didn't enable compare", so F7 was
pressed with no comparison open, where it does nothing and says nothing (`compare_jump` returns
when there is no alignment). Not a fault; F7 inside a comparison was not reported on. Tab in
tables adds a row only from the last cell of the last row: as designed. Both questions are closed.

**Other things left as they are**

- `test/hand`: a local, unpushed branch on the Windows PC (`merge/next-release` as of `9f07121`
  plus `f/paste-image` and `feat/restart-after-update`), made for the hand tests. It can be deleted.
- The release build shown to the owner is a copy outside the repository (a scratch folder of the
  session); `target/release/mdedit.exe` is the same commit, `95edf8d`.
- One shared `target` folder was used for a build of `main` in a second worktree; the release
  build of the branch then failed on stale artifacts until `cargo clean --release -p mdedit-core
  -p mdedit -p mdedit-highlight`. Do that after building another checkout into the same folder.
- On the Windows PC the folder `Desktop\markdown-editor-rs` is an old checkout (0.4.1) with every
  tracked file deleted in its working tree; the live clone is inside it,
  `Desktop\markdown-editor-rs\markdown-editor-rs`.
- Working-tree files are CRLF on this PC (autocrlf) and LF in git; git warns, nothing breaks.

**What is left, in the order suggested to the owner**

1. Read the CI result of `merge/next-release` (pasting image
   files is in it now).
2. The owner decides: merge `merge/next-release` into `main`, the version number, the release.
   None of that was done or asked for.
3. `feat/restart-after-update` is merged (third session); a real update and Restart staged by
   hand (above) is still to do before a release.
4. Rounded corners are merged; the owner looks at them on a Linux desktop and decides the KDE
   shadow route. The shared branch `f/rounded-corners` (`c8b7d97`) is pinned by rev and is not
   the shared `main`; the viewer has no branch for the corners.
5. `f/live-preview-unsaved`, `f/finer-sync` and `f/frosted-window` are merged (third session,
   see "Four more features merged on `merge/next-release`" below). They need the mdview of the
   same release: the viewer's sides are on the viewer's branches `f/live-preview-unsaved`,
   `f/finer-sync` and `f/frosted-window` (the last with the backdrop wired into mdview and the
   Restart button merged), prepared and pushed but **not merged** into the viewer's
   `merge/next-release`, which another session was finishing at the time.
6. The owner looks at the Window backdrop in a real window (both themes, Mica and Acrylic) and
   decides the points listed under "Four more features"; blur on Wayland is not started.
7. The shared pin is the branch `merge/editor-next` of markdown-shared-rs (`f/rounded-corners`
   with the four features' shared branches merged), not the shared `main`. The viewer's
   `merge/next-release` pins another shared branch (`merge/viewer-next`); before a release the
   two have to become one.

Known small gaps in what is merged: two column-paste edge cases (a plain copied line into a
two-row column; cut and paste-back with an empty last row); a backed-up file that was deleted and
comes back as an untitled tab loses its session entry about 3 s before its recovery copy exists;
on Linux an image paste runs `wl-paste` or `xclip` on the UI thread.

### The shared repository (Kaan88/markdown-shared-rs)

`main` is `ce1345d` (what mdedit 0.5.1 pins). Branches that hold work for the above:

| Branch | Head | For | State |
|---|---|---|---|
| `next` | `e3d381b` | rounded corners (Corners setting, corner mask, client-route corner cut, DWM preference) | CI **failed** at this head; not looked into |
| `feat/restart-after-update` | `30aaa8b` | Restart button (`relaunch.rs`, `single::wait_gone`, update bar button) | CI green; reviewed |
| `feat/settings-single-page` | `ba9d342` | `Form::single_page()` for the viewer's Settings | CI green; reviewed |
| `f/folder-search` | `bfd0aca` | viewer find-in-folder searcher/results | no CI seen; WIP on the viewer side |
| `f/copy-rich-text` | `8298204` | HTML writer / clipboard formats | CI **failed** |
| `f/export-print` | `fc1c559` | export helper | no CI seen |

### Notes from the session

- 0.5.1 / 0.3.1 fixed submenus closing before the pointer reached them (see the release section
  below). The owner has not yet confirmed it by hand.
- Restart button: built and reviewed, CI green, but **no real restart was ever performed**. The
  installed-bar text was reworded when it was merged (third session of 2026-11-15).
- Viewer Settings on one page: reviewed on screenshots. Its `Cargo.lock` also moved `windows-sys`
  and `base64`; do not carry that. The Linux-only "make default" row was in no screenshot.
- Rounded corners: owner said "do rounded corners". Work is on `next` in both apps and shared; the
  apps' `next` CI is green but shared `next` CI failed, and nobody reviewed the result.
- The Linux worktrees (`~/src/md/wt/*`, `~/src/md/*-next`) and build caches are local to the Linux
  machine and are not needed: every branch is on GitHub.

## Four more features merged on `merge/next-release` (2026-11-15, third session; not in `main`, not published, not seen by CI or by the owner)

Merged in this order, one `--no-ff` merge each: `f/live-preview-unsaved`,
`feat/restart-after-update`, `f/frosted-window`, `f/finer-sync`. The notes of
`docs/next/<key>.md` are folded in: the design text is DESIGN sections 65 to 67 (the Restart
button is in the section on updates), the release notes are in `docs/release-notes/NEXT.md`.
All `mdshared-*` pins are the head of the shared branch `merge/editor-next` (`c9ba573`): shared
`f/rounded-corners` with `f/live-preview-unsaved`, `f/finer-sync`, `f/frosted-window` and
`feat/restart-after-update` merged, plus two commits made there (the bar's wording, the
backdrop's state moved in from the editor). No third-party crate moved in `Cargo.lock`.

Checked at the head on Windows 11: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test --workspace` (270 passed in mdedit's own suite, 536
in the other crates; none failed). `tests/shots.sh` against the set of the branch before
these merges (`2f4fcd0`): 184 of 196 byte-identical. The 12 that differ are the eleven Settings
shots (one more row, "Window backdrop", so the list is longer and the thumb shorter) and
`update-installed` (the bar's new sentence). Six are new: `settings-view-backdrop`,
`update-installed-plain`, `menu-view-untitled`, `menu-tab-untitled-linked-dark`,
`previewing-untitled`, `previewing-untitled-dark`. No shortcut was added by any of the four, so
there is no clash to check.

### Preview of a document that has no file (`f/live-preview-unsaved`)

Built:

- `bins/mdedit/src/link.rs`: `Target::{File, Untitled}`, `doc_name`, `Opening`, `Links::
  link_untitled`, `Links::saved`, `Links::take_older`; the courier's send returns the shared
  `Delivery`; `Docs::link_poll` turns the link of a saved document into a link to its file and
  always sends the text of a document without a file.
- `bins/mdedit/src/app/handoff.rs`: `preview_untitled`, `handed_untitled`.
- Strings: `PREVIEW_UNSAVED_TOO_LARGE`, `PREVIEW_VIEWER_OLDER`; `PREVIEW_NEEDS_FILE` is gone
  (nothing says it any more).
- Arguments for shots and scripts: `--new` (start with an untitled document in front);
  `--demo-link` now also marks an untitled document as previewed.
- Shots: `menu-view-untitled`, `menu-tab-untitled-linked-dark`, `previewing-untitled`,
  `previewing-untitled-dark`.

Decisions taken, for the owner to confirm:

- The text of an unsaved document is sent live even with "Live preview while typing" off.
- The preview commands are listed for every document without a file, not only once its language
  is set to Markdown.
- Over 2 MiB such a document is not previewed at all (a toast).
- With an older mdview the editor says so and ends the link; the viewer was started bare by then
  and stays open, empty.
- `--new` was added as a launch argument.

Tried in real windows on Windows 11 (release builds, scratch settings, driven by script): Preview
side by side on a new document started mdview and tiled the two windows; typing updated the
viewer; a relative image and link showed as missing; Save As through the real dialog turned the
viewer's tab into the file's with no second tab; typing after the save was live again; a second
untitled document was previewed beside it and its preview stopped.

Budgets, measured on this PC with `--trace` on release builds of the base (`5ccd143`) and of
this branch, run in turn: first frame of `fixtures/sample.md` 111 to 124 ms before and 112 to
124 ms after (the machine was busier than when the PLAN's figures were taken); keystroke to
pixels p50 0.73 ms, p95 0.84 to 0.88 ms before and p50 0.72 ms, p95 0.86 to 0.98 ms after;
scroll frame p50 0.61 ms before and 0.62 ms after. Nothing runs for the feature until a
document is previewed. Not measured: memory, the 100 MB file (no code on those paths changed).

Not verified:

- Linux in real windows (CI runs the tests and shots there; the unix-only test
  `preview_of_a_document_without_a_file_starts_the_viewer_bare` runs only on CI).
- A really older mdview binary: the case is covered by a test with a receiver that answers as
  the old code does, and by a shared test, not by running mdview 0.3.1.
- "Edit in mdedit" from the viewer's untitled tab in real windows (app-level tests on both sides
  only).

### The preview follows the caret inside long blocks (`f/finer-sync`)

Built:

- `bins/mdedit/src/link.rs`: `Active::column`, `Update::column`, `COLUMN_STEP`; `Linked::sent`
  is the line and the stretch of it; `Docs::link_poll` reads the caret's byte in its line.
- Tests: `link::tests::a_caret_moving_along_a_long_line_tells_the_viewer_every_stretch`,
  `app::handoff::tests::the_caret_in_a_long_line_tells_the_viewer_how_far_along_it_is` (through
  a stand-in for the viewer's window: the column on the wire, in bytes for two-byte letters, the
  text with it, and the caret put at a column the viewer names).

Decisions taken, for the owner to confirm:

- The column is a byte offset, not a character or a visual column: it is what `--goto` already
  meant, and the viewer maps bytes of the file.
- 32 bytes is the stretch: about a third of a row of the preview at its usual width.
- No shot: nothing new is visible in the editor.

Budgets, measured on this PC (Windows 11, release builds, three runs each of
`--trace --type-steps 200` and `--trace --scroll-steps 120` on the 610-line test document, no
preview running): keystroke to pixels p50 0.58 to 0.59 ms before (`merge/next-release` at
`2f4fcd0`) and 0.58 ms after, p95 0.62 to 0.64 and 0.63 to 0.66; a scroll step's own work p50
0.39 to 0.40 ms before, 0.40 ms after. No difference beyond the runs' own spread.
Nothing runs for it while no document is previewed; while one is, a pass of the loop reads one
more number from the buffer (the start of the caret's line).

Tried in real windows on Windows 11: a script started this mdedit on a 610-line
test document with `--run PreviewSideBySide`, which started the mdview of the same branch beside
it (both with scratch settings), posted arrow, Home and End keys to the editor's window and saved
each window's own picture after each. Seen: the preview follows the caret through a 300-line code
block and a 300-line paragraph, and along a 27000-byte one-line paragraph (byte 9000, End, Home);
the first update after the hand-over carries the column. Not tried in a real window: the viewer's
"Edit in mdedit" with a column (tested headless), typing with live text on a long line, and
anything by hand.

Not verified: Linux in real windows; a person's judgement of how the following feels.

### Window backdrop (`f/frosted-window`)

**Built.** As above. Shared commits `74a2273`, `9aab56b` and, on `merge/editor-next`, `c9ba573`
(the state moved to the shared crate); the editor pinned `9aab56b` on this branch before the merge (only the
`mdshared-*` entries of `Cargo.lock` changed; one new windows-sys feature, `Win32_System_Power`,
no new crate). Unit tests: the type and its keys, the key-out arithmetic (exact over the key,
opaque stays opaque, premultiplied), `backdrop_allowed`, the state machine in `frost.rs`, and four
app-level tests on composed frames (chrome marked and text not, at scale 1 and 1.5; a menu over
the chrome opaque and its shadow not; a dialog; unfocused or off: every pixel opaque).

**Setting off is unchanged.** `tests/shots.sh` before and after, compared byte for byte: 179 of
188 identical. The 9 that differ are all Settings shots and differ only because of the new row:
`settings`, `settings-dark`, `settings-narrow`, `settings-scale`, `settings-complete` (the
scrollbar thumb is a few pixels shorter) and `settings-preview`, `-dark`, `-narrow`, `-scale`
(the list is one row longer). In a real window, a capture of the new build with the setting off is
identical to a capture of the build before the feature, in both themes (window area, 0 differing
pixels).

**Numbers** (release builds, this PC: Windows 11 build 26300, 3440x1440, scale 1; `measure.sh` in
`docs/captures/frosted-window/`, 15 interleaved starts, 3 x 300 keystrokes, 3 x 300 scroll steps on a
50,000-line file; "before" is the base commit's build):

| | before | off | Mica | Acrylic |
|---|---|---|---|---|
| first frame, ms (median; min-max) | 81.7 (79.2-98.1) | 81.9 (79.6-88.2) | 83.9 (82.1-90.9) | 84.3 (80.8-94.3) |
| private memory, MiB (median) | 27.6 | 27.6 | 27.7 | 27.8 |
| keystroke to pixels, ms (p50 / p95) | 0.45 / 0.51-0.63 | 0.45-0.47 / 0.54-0.60 | 0.65 / 0.78-0.83 | 0.66 / 0.81-0.83 |
| scroll step, own work, ms (p50 / p95) | 0.42 / 0.57-0.60 | 0.41-0.42 / 0.49-0.55 | 0.41 / 0.53-0.58 | 0.41-0.42 / 0.49-0.61 |

Off is the same as before within the noise. On: about 2 ms on the first frame (the two system
calls and one more full frame), about 0.2 ms on a keystroke, nothing on scrolling. Where the
0.2 ms on a keystroke goes was not traced further; it is the cost of keying out the chrome that a
keystroke repaints. It can be looked at if the owner keeps the feature.

**Captures** (`docs/captures/frosted-window/`, taken by `capture.sh` over a striped pattern window;
screen grabs, so what the compositor put out, not what the eye judges):
`{dark,light}-{off,mica,acrylic}-{active,inactive}.png`, `dark-{mica,acrylic}-{settings,palette,menu}.png`,
`light-acrylic-menu.png`, and `before-{dark,light}-active.png` from the base build. The inactive
captures are identical to the base build's except where the small window that took the keyboard
overlaps them (the checkered rectangle at the right edge).

**For the owner to decide.**

1. The look itself, by eye, in both themes: Mica is subtle (a tint of the wallpaper; over the
   striped pattern it reads as almost flat), Acrylic is the visible one.
2. The Acrylic veil (150 of 255): lower shows more of what is behind and costs legibility.
3. Frosted only while the window is active. This follows the system, which removes the material
   from inactive windows; the chrome then switches to the flat colours.
4. The row is shown only in Windows builds, and on Windows 10 it is shown but does nothing (the
   help line says "Windows 11"). Hiding it there is a small change if wanted.
5. Mica Alt left out.
6. Nine existing Settings shots change because of the row.

**Not run.**

- Transparency effects off, battery saver, high contrast, Windows 10, Windows 11 before 22H2: no
  system setting was changed and no other machine was used. Covered only by `backdrop_allowed`'s
  unit tests and the documented behaviour of the calls. On Windows 10 the two calls are never made
  because the build check fails first.
- Maximized, snapped, full-screen, minimized and restored, a second monitor, scale factors other
  than 1 in a real window (scale 1.5 is covered headless).
- Dragging and resizing the window by hand; the spike measured resizing with route 4 and found no
  cost, the feature was not driven that way.
- Changing the setting in the dialog by hand in a real window (the captures start with the
  setting already chosen; the change path is covered by the unit tests of `Frost::sync`).
- Remote desktop and software-rendered sessions.
- Linux: nothing changes there (the row is absent, the platform calls return false); checked by CI
  only.
- The viewer: wired on the viewer's branch `f/frosted-window` (same shared code and setting row),
  not merged there; see "What is left".

### Restart after an update (`feat/restart-after-update`)

- Built on its branch earlier (reviewed then); merged here. Next to the Restart button the bar
  now reads "mdedit X is installed. Restart to use it." (shared `Updates::bar_label_with`);
  where there is no file to start (a copy that cannot update itself) it keeps "It starts the
  next time you open mdedit." and has no button. Shots `update-installed` and
  `update-installed-plain` show the two.
- Tried by script on Windows 11 with a release build and scratch settings: a window was open; a
  second process without the mark handed its file to that window and returned in 0.1 s; a second
  process started with `--restarted` (what Restart starts) waited the 5 s until the first
  window had closed, then opened its own window with the session's tabs and its file. So the new
  process does not hand its files to the window that is closing.
- **Not tried**: a real update followed by a click on Restart (it needs a second build with a
  higher version on an https host, see "Review results" above); the click itself in a real
  window (with `--update-bar installed` there is no installed file to start); Restart with
  unsaved changes and a cancelled Save question (tested headless on the branch); an AppImage.

### Decisions taken without the owner (third session)

- The order of the merges: live preview, Restart, frosted window, finer sync. The frosted branch
  could only be built against a shared head that has the Restart button's bar, so Restart went
  in before it; finer sync was finished last.
- The shared branch `merge/editor-next` was built up merge by merge while the editor's merges
  were made, and each editor merge pinned its head at the time; the final pin is `c9ba573`.
- The installed bar's sentence beside the button: "mdedit X is installed. Restart to use it."
- The backdrop's state (`Frost`: when the chrome is frosted, what the system was asked) was
  moved from the editor into the shared crate (`mdshared_ui::frost`), so mdview uses the same
  code; the editor keeps only the list of its frosted regions.
- finer sync: the column sent to the viewer is a byte in the line (what `--goto line:column`
  already meant); an update is due every 32 bytes along a line (`link::COLUMN_STEP`); on the
  viewer's side the follow step stays 48 px in both directions (an earlier session's smaller
  step upwards broke an existing test and was taken out) and the places of lines are noted at
  every parse (about 3 % of a parse, 12 bytes for a line of the file).
- The captures of the frosted window were kept, moved from `docs/next/frosted-shots/` to
  `docs/captures/frosted-window/` (3.7 MB of PNGs with the two scripts that made them). Delete
  the folder if they are not wanted in the repository.
- The viewer's sides were prepared on branches and not merged (the brief said so): see "What is
  left".

### Not verified (third session)

- Nothing was looked at by the owner. Every real-window run was a script's: posted keys and the
  windows' own pictures, a few seconds each, with scratch settings.
- CI was not started for the head; Linux was not run at all in this session.
- The three features that involve mdview were tried in real windows only with the viewer's
  feature branches' builds, never with the viewer's `merge/next-release`.
- A real update and Restart; the frosted window's list under "Not run" above; "Edit in mdedit"
  with a column in real windows.

## Five features merged on `merge/next-release` (2026-11-15, not in `main`, not published)

The notes each branch left in `docs/next/<key>.md`, folded in here; the design text is in DESIGN
sections 57 to 61, the release note in `docs/release-notes/NEXT.md`.

### Column selection

- Built: `vcol.rs` (visual columns), `multi::column` using it, `Ready::select_column/line_col`,
  drag past line end (`View::columns_past_end`), autoscroll while dragging, column-aware copy and
  Backspace/Delete/Paste in `Docs::edit`, `--select-column`, a Keyboard shortcuts row, shots
  column, column-dark, column-cjk. Alt+drag and Alt+Shift+Arrows already existed; no new shortcuts.
- Not verified: Windows and a real window with a mouse (drag, autoscroll, Alt+drag under a Linux
  window manager); release-build budgets (nothing runs unless a column selection is made; the
  keystroke path adds one comparison).
- Left out: virtual space; wrapped-row columns.

### Compare: copy a change across

- Built: mdedit-core `merge` module (take_lines, replace_lines, unit tests),
  `compare::pick`/`Align::change_at_line`/`enabled`, `App::compare_copy`, `EditOp::CopyLines`,
  commands, strings, View menu entries, palette, `--copy-change across|take` for shots, app-level
  tests, four shots (compare-copied, compare-taken-dark, menu-view-compared,
  menu-view-compared-dark).
- Decisions: no gutter arrow buttons (the compare view has no gutter between panes and the panes
  are ordinary editors; adding one would change the look); no new shortcut; next and previous
  difference already existed (F7, Shift+F7), only added to the menu.
- Not verified: Windows (CI only); release-build budgets not measured (nothing runs at startup or
  on keystroke or scroll; the code runs only on the commands, and `enabled` runs per menu or
  palette build, a hunk lookup).
- Left out: gutter arrows; copying part of a block (selection) or a single hunk inside a changed
  block.

### Multi-caret line commands

- Built: `mdedit_core::multiline` (+ `multiline/tests.rs`, about 60 tests with two and three
  carets, overlapping and adjacent selections), `*_range`/`*_pts` variants in
  `lines.rs`/`markdown.rs`, `History::fuse_since` (one undo step), shell wiring in `Docs::edit`
  (`line_edit`).
- An old test that asserted "a command other than typing leaves one caret" was changed to the new
  rule.
- Not verified: Windows, a real window, release-build budgets (nothing runs with one caret: one
  `is_empty` check).
- Left out: Outdent with several carets; MdWrap and MdLink already worked on every caret.

### Markdown table editing

- Built: `crates/mdedit-core/src/table.rs` (new; 15 unit tests including a hostile-input run), the
  refactor of the table finder in `markdown.rs` (`locate`, `lay_out`, `bounds_of`; quotes and code
  spans), `EditOp::MdTableOp`, `table_keys` in `docs.rs` (Tab, Shift+Tab and Enter hook, runs
  before the typing help), eight commands (`C::MdRowBelow` ... `MdMoveColumnRight`) in commands,
  palette, Markdown submenu (existing glyphs PLUS, DELETE, BACK, FORWARD), `fixtures/table.md` and
  four shots in `tests/shots.sh`.
- Decisions: the caret goes to the cell start, no selection of cell text; Enter in an empty row
  ends the table (otherwise a table at the end of a file could not be left with Enter); commands
  are enabled for any open document like the other Markdown commands and refuse with a toast; no
  setting (no Markdown group in Settings); a table whose header begins with a list marker
  (`- | a |`) is not recognised.
- Not verified: Windows and release-build budgets (nothing runs at startup and nothing on the
  typing path: the new code runs only for Tab, Shift+Tab, Enter and the commands, and for Tab and
  Enter only after `assist.markdown` is true; Enter in Markdown additionally locates the table
  around the caret, which is bounded to 2000 rows and returns early on a line with no pipe).
  Real-window behaviour was not tried.
- Left out: tables inside list items whose first line is the table, moving rows, sorting.

### Autosave and crash recovery

- Built: `crates/mdedit-core/src/recovery.rs` (format, atomic write, read, tests),
  `bins/mdedit/src/recovery.rs` (poll, thread, scan, restore, finish; 9 tests with real files),
  limits in `limits.rs`, `Doc::recovered`/`read_restore`/`Ready::recovered_encoding` in `docs.rs`,
  wiring in `app.rs` (start: restore and toast; `about_to_wait`: poll and timer; exit: finish),
  `strings::recovered`, `--demo-recovered N` (shot only: the toast), two shots in `tests/shots.sh`
  (recovered-light, recovered-dark).
- Decisions: recovery is separate from the session backup (different folder and format, so the
  session's code is untouched); the header holds size, time and the length, not a content hash;
  no setting and no "autosave to the real file". The thread starts on the first write and is
  joined at exit.
- Fix (data loss), `867f197`: see DESIGN section 61, "A copy wins over a session backup".
- Not verified: nothing ran on a real window or on Windows locally (CI only); an actual `kill -9`
  or power loss was not tried (the tests simulate it by dropping the store after the thread
  finished); the cost on release builds was not measured. Why it cannot move the budgets: nothing
  runs at start except one `read_dir` of a folder that normally does not exist; per event-loop
  pass the poll is a loop over the tabs comparing integers; a keystroke adds no work.
- Left out: a copy for a document over 64 MiB; text put back is lost for a file read lossily (the
  same limit the session backup has); a toast naming which documents; no content hash.

## 1. Where things are

Three public repositories under `Kaan88`, all MIT, all pushed and clean on `main`:

| Repository | What | Head on 2026-11-13 |
|---|---|---|
| `markdown-viewer-rs` | `mdview`, the Markdown viewer. Its own `docs/HANDOFF.md` describes it | `b4e1617` |
| `markdown-shared-rs` | The crates both applications use | `843b50f` |
| `markdown-editor-rs` | `mdedit`, this repository | this commit |

The editor pins the shared repository by `rev` in its root `Cargo.toml` (for 0.5.0: `7cd6461`, see "Release 0.5.0"), earlier on 2026-11-13 at
`843b50f3cc025480ee06226be1247d38df7442a2`. Three shared commits were made from this repository's
session on 2026-11-13: `a74aca4` (the updater's threads: `spawn_at_launch`, `spawn_check`,
`spawn_install`, `unix_now`), `d0ec8b8` (`Language::line_comment`) and `843b50f` (`Menu::above`,
a menu that stands on its anchor, for menus opened from a status strip). The viewer still starts
its updater threads with its own copies in `bins/mdv-app/src/app/commands.rs`; moving it to the
shared ones is a small change that was left to the viewer's own session. The editor does not yet
use the watcher's `set_folders` or the tree's `watched_dirs` (`e93719b`). The viewer pins it in `crates/core`,
`crates/render` and `bins/mdv-app`; check there which commit it is on.

On the Windows PC the three checkouts sit side by side in `C:\Users\kaank\Desktop`: `mv`,
`markdown-shared-rs`, `markdown-editor-rs`. Anywhere else, clone all three fresh. The viewer's
history was rewritten on 2026-11-12, so an older clone of it must be re-cloned, not pulled.

## 2. The user's rules

- **The goal for the editor (2026-11-12):** "as long as the app remains just as fast to open and
  work, implement useful features from notepad++." The agreed list and its order are in
  `docs/PLAN.md` under "Feature roadmap". Measure the budgets before and after each feature.
- The user approves the interface by looking at it. Screenshots and tests do not stand in for that.
- Report results plainly, including what was not run. Nothing counts as working on Windows until
  it has run on Windows.
- Commit and push as phases complete or when asked. There is no CI: run the tests yourself.
- On the Windows PC, launch a built `.exe` from Git Bash, not PowerShell (PowerShell refuses it as
  unsigned).
- **Shared first (2026-11-13):** "whatever can be shared will start shared. no need for
  duplicate work." Anything both applications could use is written in `markdown-shared-rs`,
  pushed, and pinned; the application keeps only its own names and settings.
- The user asked for cheaper models where they do the job, and on 2026-11-13 said: no Opus
  subagents; the main session does that work itself. So the headless crates with a clear
  specification go to Sonnet subagents (each in its own git worktree, so their builds do not
  break each other), and the design, the shell, the reviews and the fixes stay in the main
  session.

## 3. What was done on 2026-11-12

**Shared crates** (plan: `markdown-shared-rs/docs/PLAN.md`, steps 0 to 3 done):

| Crate | Holds |
|---|---|
| `mdshared-text` | `Bitmap`, `TextEngine`, `shared_fonts` |
| `mdshared-highlight` | syntect set-up, whole-block highlighting (viewer), `incremental` line highlighter (editor) |
| `mdshared-ui` | `gfx` (`Painter`), `theme`, `menu`, `textfield`, `tree`, `keys`, `platform`, `glyph`, `scroll`, `chrome` (tabs, caption buttons), `pane`, `overlay` (bubbles, dialog frame), `window` (resize bands) |
| `mdshared-app` | `paths`, `files`, `recents`, `settings` (key=value layer), `single` (hand-over), `watch` |

The viewer re-exports them at their old paths. After every step the viewer's 43 `--shot` renders
were compared byte for byte with renders taken before the move, and all matched; its tests pass
(319 there, 74 in the shared repository, more than the 368 before the move because tests were
added). `mdview.exe` went from 7,439,872 to 7,457,280 bytes.

Not done for the shared code: step 4 (the window loop as a framework) was put off on purpose.
Each application has its own loop; design the shared one from the two. Step 5 (a tag) waits for
that.

**Editor.** Phase 0 spikes (`spikes/*/RESULTS.md`) and phase 1, the read-only editor: tabs,
scrolling, caret and selection by keyboard and mouse, copy, find with counter, go to line, zoom,
Files pane, recent files, session restore, reload on change, single instance, syntax colours,
encodings (UTF-8, UTF-16 by BOM, legacy code pages guessed with `chardetng`) and line endings in
the status bar. Typing does nothing yet.

| Crate | Holds |
|---|---|
| `crates/mdedit-core` | Loading, encodings, line endings, the rope buffer, display columns, selection, find. Headless |
| `crates/mdedit-highlight` | The highlight worker thread over `mdshared_highlight::incremental` |
| `bins/mdedit` | The shell: window loop, text view (`view.rs`, `view/`), chrome, panes, overlays, session, `--shot` |

Measured on the PC, release build (phase 1, after the fixes):

| | Budget | Measured |
|---|---|---|
| First frame, small file | under 100 ms | 84 to 104 ms across runs, median about 90 |
| First frame, 100 MB file | under 500 ms | 138 to 199 ms warm, 271 ms cold |
| Private memory, small file | under 30 MiB | 27.5 MiB |
| Private memory, 100 MB file | 125 MiB | 133 MiB, over by 8 MiB, left as it is |
| Scroll frame, p95 | under 4 ms | 0.8 to 1.5 ms |
| Idle CPU, focused window | zero | none measured |
| Exe | under 6 MB | 5.7 MB |

**2026-11-13: phases 2 and 3.** Editing and safe saving, as `docs/DESIGN.md` section 16
describes them. Two Sonnet subagents wrote the headless parts (`buffer.rs` `replace` and `Change`,
`edit.rs`, `history.rs`; `save.rs`), the main session wrote the shell around them and
`HighlightService::edit`. Figures are in `docs/PLAN.md` under "State": a keystroke reaches the
pixels in under 1 ms on the small and the 100 MB file, and the phase 1 budgets hold.

## 4. What has not been done or checked

- **Approval by eye:** the owner has approved the icons and the minimap by looking at them.
  The rest of the interface has not been explicitly approved. (The older notes below say the
  user had not looked at the editor; the owner has since used it and raised the update look and
  the submenus, see 0.3.2.)
- Nothing interactive in the editor was tried by hand: selection, copy into another program, find,
  tabs (drag, middle click, menu), the Files pane, theme switch, reload when the file changes, a
  second launch handing over to the first (not re-checked; the owner has used the editor since,
  see the next item and 0.3.2).
- **2026-11-13, later:** the user tried the build and reported that everything worked. Three
  things they raised turned out not to be faults (Redo after Backspace, a file drop with a tab
  open, heavy shadows; the last is the About card in dark mode, which draws its shadow twice
  over the dimmed window in shared code, and the user said to leave it). While checking them the
  main session drove a real window with synthetic keys, mouse and an OLE drag: typing, Backspace,
  Delete, undo, redo, the right-click menu's Undo, Save, a dropped file, New file and Save
  through the file dialog all behaved. That is not the user's approval of the interface.
- Updates: 0.4.0 was the last release published before 0.5.0 (and 0.3.1, 0.3.2 before it, with an
  installer since 0.3.1). The bar and the About buttons were rendered with `--update-bar`; that an
  update was offered and installed by a running mdedit through the bar (not re-checked). An update
  of an AppImage by itself has not run.
- Editing basics (2026-11-13): each shortcut was pressed once by script in a real window on a
  small Rust file and gave the right text. The user has not seen the longer right-click menu.
  Move line and the two every-line commands on large or odd files ran only in the unit tests.
- New file (2026-11-13): `Doc::path` is optional. Tested by one unit test and one scripted run
  in a real window; the user has not seen the untitled tab or the changed "+" button.
- Of phases 2 and 3, by hand, before that: nothing. Typing ran only through `--type-steps` (a real window,
  scripted keystrokes) and `--type`. Never pressed by a person: every editing key, dead keys and
  AltGr on the Turkish layout, cut and paste with another program, the three questions (Save
  changes, file changed on disk, reload and lose changes), Save as and its file dialog, closing
  the window with several modified tabs.
- `ReplaceFileW`'s fallback for file systems that do not support it (network shares, FAT) has
  not run. The code for other systems than Windows was built and run on Linux from 2026-11-14
  (see "Linux: the shared Linux code adopted").
- Undo steps are capped by the text they hold (64 MiB), not by their number.
- The first frame of a Markdown file was 125 ms in one run (`fixtures/sample.md`), over the
  100 ms budget set on `sample.rs`; whether phase 1 had that was not checked (not re-checked).
- Idle CPU with the editor's window unfocused was not measured.
- The input-method hand test (`spikes/ime/RESULTS.md`: Turkish Q dead keys and AltGr, a CJK IME)
  was not done. Phase 2 depends on what it shows.
- Linux (2026-11-14, in 0.5.0): the editor builds and runs on CachyOS with KDE Plasma 6 on
  Wayland, and sway ran headless in a container; the AppImage is built in an Ubuntu 20.04
  container. File dialogs, the window identity (taskbar name and icon), downloads and the update
  check use the shared Linux code; emoji draw in colour. File dialogs and the taskbar icon were
  exercised in the driven real-window tests on KDE Plasma 6 Wayland on 2026-11-14. What is still open on Linux: GNOME and
  other desktops than those two, X11, fractional scales in a real window, two monitors with
  different scales, input methods, and the file dialogs' lack of a parent window (the window stayed responsive
  while one was open, which was seen on KDE). "Show in file manager" opens the folder only
  (not re-checked). `tests/shots.sh` needs LF line endings (a Windows checkout with autocrlf
  breaks it on Linux; `.gitattributes` covers only `fixtures/**`). The 0.5.0 test suite and
  screenshots have run on a Windows runner; nothing by hand on Windows.
- The editor's small-file first frame sits at the edge of its budget: about 60 ms of it is
  syntect start-up before the first colours.

## 5. Decisions the user has not confirmed

These were made to keep moving and are easy to undo:

1. **`panic = "abort"`** in the editor's release profile. It saved 0.9 MB and brought the exe
   under 6 MB. A panic on any thread ends the process, and there is unsaved text to lose. The
   viewer uses `unwind`. Unsaved-session restore (roadmap item 3, built 2026-11-13) is what
   keeps the text, so the abort is less costly than when this was written; still not confirmed
   by the owner.
2. **Crate names** `mdshared-*` (the shared plan called them placeholders).
3. **Step 4 put off**, as above.
4. **`mdshared-app` created** (open decision 1 of the shared plan, taken as it recommended).
5. Open in `docs/PLAN.md` and untouched: the editor's name and the word-wrap default (not
   re-checked). Markdown conveniences were built for 0.5.0 (Markdown commands, list continuation
   with Tab). An installer exists since 0.3.1 beside the bare exe.
6. Made on 2026-11-13 for phases 2 and 3: Tab inserts a tab character (since 0.5.0 there is the
   setting "Indent with spaces", and the indentation of the file is detected and followed, see
   "Findings of the driven test of 0.5.0"); the unsaved mark is a bullet before the file name; a file with replaced bytes is read-only;
   saving runs on the UI thread; the layout cache is keyed by line content; the IME stays off
   until phase 5 (not re-checked).
7. Made on 2026-11-13 for New file: Ctrl+N and the "+" beside the tabs make an "Untitled N" tab
   (the "+" opened the file dialog before); it is UTF-8 with the platform's line ending; Save
   on it is Save as, offering "Untitled N.txt"; it is not in the session, so an untitled tab
   does not come back after a restart (a modified one asks before closing).
9. Made on 2026-11-13 for the editing basics (roadmap item 1, `mdedit-core`'s `lines.rs`, by a
   Sonnet subagent, wired by the main session): Duplicate line Ctrl+D, Move line Alt+Up and
   Alt+Down, Delete line Ctrl+Shift+K, Toggle comment Ctrl+K and Ctrl+/ (the language's line
   comment, from the shared `Language::line_comment`; a language without one gets a toast),
   UPPERCASE Ctrl+Shift+U, lowercase Ctrl+U, Trim trailing spaces and Tabs to spaces (no
   shortcut). All but the two moves are in the text's right-click menu. Trim and Tabs to spaces
   refuse a file over 8 MiB: they run on the UI thread at 4 to 7 s per 100 MB.
10. v0.1.0 was published on 2026-11-13 from `2ba9912` with `tools/release.ps1`
   (`docs/RELEASE.md`): the exe and `mdedit-update.txt`. It does not hold the editing basics.
8. Made on 2026-11-13 for updates, after the user said to wire the shared updater in: mdedit
   updates itself the way mdview does (`docs/DESIGN.md`, section 17). The manifest address is
   `https://github.com/Kaan88/markdown-editor-rs/releases/latest/download/mdedit-update.txt`,
   chosen by analogy with the viewer's; `MDEDIT_UPDATE_MANIFEST` names another one. One
   executable, no engine, no installer entry. The bar sits under the command bar; the two
   controls are in About, as in mdview. This ends the plan's "no network use at all".

## 6. What to do next

1. **The owner looks at the interface.** Icons and minimap are approved by eye; the rest is
   not explicitly. The hand checks listed in section 4 (typing on the Turkish layout, dead keys
   and AltGr, cut and paste with another program, the three questions, Save as, closing with
   several modified tabs) were not re-checked.
2. **Linux is to be tested by the owner** (section 4; the owner will test it). It has been built
   and run on KDE Plasma 6 and sway only, by agents.
3. **Move the shared `main`.** The shared work lives on the branch `feat/linux` (`7cd6461` is
   pinned by 0.5.0), to become the shared `main`. A push to the shared `main` is refused from
   this PC; the owner moves it, then the `rev`s here and in the viewer can name commits on `main`.
4. The feature roadmap in `docs/PLAN.md` is built to its end. What each item left unbuilt is
   listed in its own dated section below. The list for the next release is in `docs/PLAN.md`
   ("Next release") and in "Release 0.5.0" below.
5. The Files pane could follow changes in its folders with the shared watcher's `set_folders`
   and the tree's `watched_dirs` (shared commit `e93719b`) (not re-checked).
6. Any change to shared code: edit it in `markdown-shared-rs`, run both applications'
   `tests/shots.sh` before and after, push, then move the `rev` in both.
7. Code the editor's shell copied from the viewer because it was generic but had not been moved
   (parts of the window loop, the bar, the find box wiring) is the input for step 4 of the
   shared plan.

## 7. Build and check

```bash
# in this repository; on the PC, keep temp files off the system drive's temp folder
export TEMP="$PWD/target/tmp" TMP="$PWD/target/tmp"; mkdir -p target/tmp
cargo build --release --workspace
cargo test --release --workspace        # 525 passed early on 2026-11-14; later sections give 596 (not re-run for the release)
cargo clippy --release --workspace --all-targets -- -D warnings
cargo fmt --all --check
bash tests/shots.sh target/shots/a      # deterministic; first run generates target/data. 72 PNGs when this was
                                        # written; the dated sections give 158 on 2026-11-14, then 4 more
# The script pins --scale 1.0 for shots that give none: without it the shots take the Windows
# display scale, and a changed display scale changes every picture.
```

Useful flags of `mdedit.exe` (all in `bins/mdedit/src/args.rs`): `--no-session`, `--shot out.png`
with `--size WxH --scale N --light|--dark --menu NAME --dialog NAME --find TEXT --goto L[:C]
--select A-B --type TEXT --scroll N --hscroll N --hover X,Y --bookmark L --fold L --minimap`; `--trace` prints stage times, first
frame, memory and CPU time; `--scroll-steps N` times scroll frames; `--type-steps N` types N
keystrokes at the caret and times each from keystroke to pixels (it exits without saving);
`--exit-after-ms N` opens a real window and closes it.

```bash
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 fixtures/sample.rs
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 target/data/100mb.txt
target/release/mdedit.exe --no-session --trace --type-steps 300 --goto 20:5 fixtures/sample.rs
```

**The viewer's no-pixel-change check** is the viewer's own `tests/shots.sh` (added 2026-11-13,
55 states): run it into a baseline folder at a good commit and again after a change, `cmp` every
file, then run the workspace tests (which include the four goldens) and `cargo fmt --check`.

Fixtures are written with exact bytes by `fixtures/make_fixtures.py`; `.gitattributes` keeps git
from converting them.

## Windows return when a side-by-side preview ends (2026-11-15, merged into `main` on the owner's word, not released)

Owner request of 2026-11-15 (docs/PLAN.md, "After 0.6.0 and 0.4.0", item 1). Built on the branch
`f/window-return` here, in the viewer and in the shared crates (pins: shared `6249cd1`, now shared `main`). **In `main` since 2026-11-15, CI green; not
released; the owner had not looked at it when it was merged.** The shared part and the viewer's are in
the viewer's handoff, same heading.

- `App::tiling` (`mdshared_ui::place::Tiling`) remembers where the window was when `tile_left`
  put it on the left half. `end_side_by_side` (`app/handoff.rs`) puts it back and sends the
  viewer `?unplace` from a helper thread. It runs when the last link is over (`about_to_wait`:
  something is remembered and nothing is linked, so a stopped preview, a closed document and a
  viewer that is gone all end it), and when "Preview side by side" is chosen for a document
  that is previewed while the window is still on its half: the command now gives the
  arrangement up and the preview goes on. The menu item has a check mark while arranged
  (`Links::side_by_side`). An `?unplace` from the viewer (its tab or window was closed) puts this
  window back without telling the viewer again.
- Exit while arranged (`leave_side_by_side`, `app/commands.rs`): the session gets the size and
  position from before, and the viewer is told. The exit pass of `about_to_wait` saves the size
  once (`size_saved`): it runs twice, and the second pass saved the half.
- **Fixed on the way, a fault of 0.6.0 and earlier**: after a close request that no other event
  follows (the taskbar's "Close window", a posted `WM_CLOSE`), mdedit could stay open until the
  pointer moved over it: the loop reads an exit asked for in `about_to_wait` only after the
  next event. The exit pass now posts itself one wake. Seen in a real window: open for 6 s and
  more before, gone within a second after. (The check on 2026-11-15 that found mdedit closing in
  0.3 s had another event following the close.)
- **Run on the Windows PC**: fmt, clippy `-D warnings`, `cargo test --release --workspace` (807
  pass), and the real-window runs listed in the viewer's handoff. **Not run**: the command chosen
  a second time, a stopped preview and a closed document in a real window; Linux.


## Release 0.6.0 (2026-11-15)

The next-release batch, released on the owner's word ("merge everything and make a release")
together with mdview 0.4.0; the two need each other for the preview of an unsaved document and
for the finer follow. What it holds is in `docs/release-notes/0.6.0.md` and in the sections
above. All `mdshared-*` pins name shared `main` (`11563a3`). Checked before the tag:
`cargo test --release --workspace` on the Windows PC and the `ci` run on `main` (Linux and
Windows). The owner had not looked at the interface when it was tagged; a real update followed
by Restart had not been performed.

## Release 0.5.1 (2026-11-14)

One fix, reported by the owner with a screen recording of 0.5.0: a submenu closed as soon as the
pointer left its item, so nothing in it could be chosen with the mouse. The pointer crossed the
menu's 5 px padding (no row hovered there) and `Popup::pointer` took that for another item. Now the
submenu stays while the pointer is off the rows, and another row closes or swaps it only after a
rest of `SUBMENU_DWELL` (`Popup::rested`, called from the timers). The earlier driven tests missed
it because the driver jumps the pointer between points; the new unit tests walk it px by px.

Verified by the unit tests on CI (Linux and Windows) only: the owner asked for no local builds, so
no real window was driven. The owner checks it in the released build.

## Release 0.5.0 (2026-11-14)

mdedit 0.5.0, for Windows and, for the first time, Linux. Version 0.4.1 was never published, so 0.5.0
holds everything dated 2026-11-13 and 2026-11-14 below: command palette, settings, compare two
files, replace in files, the same file in both panes, saved macros, fold by mouse, auto-pairing
and indentation that follows the file, line commands, Markdown commands, "Preview in mdview" with
live preview, the window shadow, and the Linux port.

**How releases are built (updated 2026-11-14, later).** Published releases come from GitHub Actions:
`.github/workflows/release.yml` runs on a tag `v<version>` (`docs/CI.md`). Windows is built natively
on a Windows runner with the repository's PowerShell scripts, so the published Windows files are
not the Wine cross-builds; the local cross path (`tools/release-all.sh`, "Releasing from Linux" in
`docs/RELEASE.md`) remains for testing. Linux is built in an Ubuntu 20.04 container. `ci.yml` runs
fmt, clippy with warnings as errors, the tests and the screenshot suite on Linux and on Windows for
every push to `main`; no test is skipped.

**The files.** Nine assets, each named `mdedit-<v>-...` below with `<v>` the version:
`mdedit-<v>-windows-x86_64.exe`, `mdedit-setup-<v>.exe`, `mdedit-<v>-windows-x86_64.zip` (portable),
`mdedit-<v>-linux-x86_64.AppImage`, `mdedit-<v>-linux-x86_64.tar.gz` (with `install.sh` and
`uninstall.sh`), `mdedit_<v>_amd64.deb`, `mdedit-<v>-1.x86_64.rpm`, an Arch `PKGBUILD` (a `-bin`
package of the tar.gz) and `mdedit-update.txt` (one manifest with a `windows-x86_64` and a
`linux-x86_64` entry). The AppImage is x86-64, built in
an Ubuntu 20.04 container, so it needs glibc 2.31 or later. It needs the system's libc,
fontconfig, freetype and the X11/xcb libraries, and loads libcurl and the Wayland libraries at run
time. It updates itself. On first run it writes its desktop entry and icon into the user's data
folder and registers its file types without taking defaults; "Make mdedit the default" is a command
in Settings. Which copies update themselves: the Windows installed copy and the bare exe (the zip's exe too, in
a writable folder) and the AppImage. The deb, rpm, tar.gz and PKGBUILD installs do not (the app says
only the AppImage can update itself); they install a desktop file, so the taskbar icon and "Open
with" work without the AppImage's first-run registration. The local cross-builds (cargo-xwin; Inno
Setup under Wine) were checked under Wine only; how they are made: `docs/RELEASE.md`, "Releasing
from Linux".

**Pins.** The shared crates are the branch `feat/linux` of `Kaan88/markdown-shared-rs` at `7cd6461`,
to become its `main` (a push to the shared `main` is refused from here; the owner moves it).
Two forks are carried on Linux and resolved for all platforms through `[patch.crates-io]`:
`Kaan88/winit`, branch `md-0.30.13` (a content margin for the client-drawn shadow) and
`Kaan88/softbuffer`, branch `md-0.4.8` (ARGB buffers on Wayland).

**Windows is reviewed (2026-11-14, the owner asked for it to be marked so).** The full test suite and
every screenshot state (167 for mdedit; 70 for mdview) ran on a real Windows runner, with both apps
built natively, and the native setups were checked for their files and icon. That review does not
cover: the installer wizard clicked through by a person, taskbar and Explorer icons, SmartScreen
with unsigned files, and interactive use on a Windows desktop. The last builds run by hand on Windows
itself were mdedit 0.4.0 (and mdview 0.2.2). 0.4.1 was built on Windows but never published.

**Found and fixed through that Windows run.** In the viewer on Windows a bare check mark and heart drew
as empty boxes (the shared font fallbacks were applied only off Windows; now on every system, see
the viewer's HANDOFF). Emoji with the variation selector are in colour on Windows (seen in the
runner's shots), which was unverified before.

### Decisions taken without the owner

The owner said on 2026-11-14: use your best judgement for open questions and make a note. These
were taken that way, or agreed on that day where it says so.

- **The exe size is reported, not a gate** (the owner agreed on 2026-11-14). Launch time is the
  first budget. The build profile was left as it is until a build compiled for speed with
  unwinding can be compared on real Windows.
- **Window shadow.** On KDE the window uses KWin's own protocol. Other Wayland desktops get the
  client-drawn margin (patched winit and softbuffer). X11 gets none by default, because a tiled
  X11 window would draw its shadow over its neighbour.
- **A second fork (softbuffer)** was made beside the approved winit fork, because stock softbuffer
  cannot present a transparent margin on Wayland.
- **Checked menu items have a check-mark column of their own** before the icons: a tinted icon
  alone did not read as "on".
- **Indentation is detected per file and followed.** A driven test found tabs being inserted into
  space-indented files.
- **Submenus stand beside their parent**, and a wheel turn over an open menu is swallowed (it
  closed the whole menu before).
- **Live preview.** The viewer returns to the saved file when the editor is gone (it checks every
  5 s while a tab is live). Text is sent only while the document is modified. The cap is 2 MiB.
- **Versions.** mdedit 0.5.0 and mdview 0.3.0, as feature releases; 0.4.1 and 0.2.3 were never
  published.
- **The AppImage keeps the default squashfs compression.** It adds about 18 ms to start-up; only
  an uncompressed image (2.5 times the size) was measurably faster (numbers in the viewer's
  HANDOFF, section 12).
- **The Windows files were first cross-built and Wine-checked** (the owner's choice, "find a way to
  cross compile windows"). The published ones are now built natively by the release workflow.

### Not verified

- By hand on Windows: the installer wizard clicked through by a person, taskbar and Explorer icons,
  SmartScreen with unsigned files, interactive use on a Windows desktop. (The test suite and the
  screenshots ran on a Windows runner; see above.)
- GNOME and other desktops than KDE Plasma 6 and sway (sway ran headless), and X11.
- Fractional display scales in a real window on Linux.
- Two monitors with different scales.
- The toast when the editor closes during a live preview: the viewer's return to the file was
  seen, the toast was not.

Verified in the driven real-window tests on KDE Plasma 6 Wayland, 2026-11-14: file dialogs (the
window stayed responsive while one was open) and the taskbar icon (both apps showed their own icon
in the panel). An AppImage updated itself end to end: the offer at launch, Update clicked, the file
replaced with a matching hash, the newer version reported at the next start, the old copy cleaned
up.

### Next release (agreed with the owner 2026-11-14)

- Copy a change across in compare.
- Finer sync with the viewer inside long paragraphs and code blocks.
- Live preview for a document that has never been saved.
- Multi-caret support in the line commands (sort, join, headings, table alignment).
- Carried over: the speed-versus-size build comparison on Windows, and a hand run on a Windows
  desktop of this release.
- For consideration, not agreed (owner, 2026-11-14): rounded window corners. Default radius 8
  logical px, scaled with the display, square when maximised, tiled or fullscreen, with a setting
  that includes 0. Reasons: 8 sits between KDE Breeze (about 5 px, from bug reports) and GNOME
  libadwaita (about 12 px, from a 2021 commit and forum estimates), and matches the 8 px radius the
  apps' own dialogs use. There is no standard way to ask a Linux desktop for its radius. On Windows
  11, ask the system for its standard rounding (the DWM corner preference) instead of drawing. On
  KDE it needs the client-margin shadow route, because KWin's shadow route leaves the window opaque
  and square; that is why it was not squeezed into this release.

The same list is in `docs/PLAN.md`.

## Findings of the driven test of 0.5.0, fixed (2026-11-14, not published; in 0.5.0)

One commit per item. Design: DESIGN.md section 56 (indentation) and the notes below.

1. **Indentation follows the file.** `mdedit-core::indent::detect` (pure, tested), run when a
   document is read, restored or saved under a new name, never while typing; `Prefs::assist_for`;
   setting "Detect indentation from the file"; status field "Spaces: 4" / "Tabs" (dropped when the
   strip is crowded, the tooltip then sits on the language field). Enter and the Markdown list Tab
   continue a line's own kind. New and compared tabs now take the preferences (they were made with
   the defaults until a setting changed).
2. **Icons.** Shared pin 090d4da: the Lines submenu has SORT, SORT_DESC, SORT_CASE, REVERSE,
   DEDUPE, JOIN; "Make mdedit the default" has CHECK (not visible in a shot: it is listed only
   from an AppImage). Every `--menu` shot was looked at: the only items left without an icon are
   the Markdown submenu's Bold, Italic, Code, Strikethrough, Heading level and Align table (no
   shared glyph fits; they stay empty rather than borrow one).
3. **Wheel.** While a menu, submenu or the palette is open a wheel turn over a menu that does not
   scroll, or outside all of them, does nothing; over one that scrolls it scrolls it. The palette
   takes the wheel only over its card. Same rule in the viewer.
4. **In selection after Escape.** Escape in the find box only takes the keyboard out of it, the
   replace strip stays open, so Ctrl+H found `replace_open` set and skipped capturing the
   selection. `capture_replace_scope(already_open)` now captures a multi-line selection every time
   Replace is asked; a caret or one-line selection with the strip open leaves the range alone.
5. **Palette footer.** Shared `d44bc14`: no count while a message row ("Go to line 25") stands in
   for results and there are no entries.
6. **Folder to a second launch.** The shared request listener dropped anything that was not a file
   (shared `9222f96` lets requests name folders; the paths-only receivers still get files only).
   `mdedit <dir>` and `--folder` go over; `App::open` already made a folder the Files pane's. A first
   launch with a directory argument made a (failed) tab of it; it now takes it as its folder,
   `--folder` winning, and shows the Files pane.
7. **Live dot without an edit.** Text is sent only while the document is modified; the link is no
   longer seeded with a version; going back to the saved text (undo) sends `live_off` once.
8. **Edit in mdedit opening a second window.** Not a bug: `--no-session` neither listens on nor
   forwards to the instance channel (`app.rs`, `session_path` is `None`), so a test window started
   that way takes no request. A normal window listens (`listen_requests_waking`, keyed by the settings
   path: a window started with another `MDEDIT_SETTINGS` than the viewer's launch is another
   instance). Test: `a_request_arrives_over_the_instance_port` (goto, from, place).
9. **Window shadow on KDE.** Measured on this machine (KDE Wayland, release binary,
   `MDEDIT_SETTINGS` scratch, `--no-session`, over the shared `shadow` example's white backdrop,
   one `spectacle` shot): grey 248 of the backdrop darkens to 212 5 px outside the editor's left edge
   (225 at 10 px, 241 at 20 px, 247 at 30 px, 248 at 35 px) and to 229 at 5 px above the top edge; the viewer binary over
   the same backdrop gives the same curve. The KWin route attaches. Only left and top edges were
   over the backdrop, right and bottom were not measured.
10. **Report-only.** (a) `Resized` sets `self.maximized = window.is_maximized()` every time, so a
    window-manager unmaximise updates the glyph. (b) The 6 px band: winit's `RESIZE_BAND` (6) widens
    the input region, shared `edge_at` has no lower bound (`x < EDGE`), so 4 px outside is inside
    both; nothing in code limits it to 1 px. On the default KDE route there is no client margin and
    no band at all (KWin owns the area outside). Not reproduced without input.

NOT run: Windows, X11, GNOME, anything interactive beyond the one screenshot above.

**Correction (2026-11-14).** The "unexplained modified tabs and stray newlines" seen in the driven
test were the owner typing into the test window.

## Emoji presentation, a stray column, the exe size (2026-11-14, after the first 0.5.0 work, not published; in 0.5.0)

**Emoji.** Shared pin moved to `bbb1c7a`; `View::build` (`bins/mdedit/src/view.rs`, the one place
that makes a layout: lines, long-line chunks, line numbers and the metrics samples all go through
it) calls `mdshared_text::push_emoji_runs(&mut b, text, &stack)` after the default and run styles,
with the same family string. Nothing changes on Windows (the helper is empty there). `fixtures/emoji.txt`
(made by `make_fixtures.py`), shots `emoji`, `emoji-dark`, `emoji-caret` (caret after a keycap,
`--goto 3:11`). Looked at: the first line in colour, the bare characters on the second as text
glyphs, the caret right after the keycap. Cost (a scratch benchmark of `build`, 3000 runs, not
committed): ASCII line 14.7-17 us with and 14.8-15.1 us without; a line with emoji about 29 us
against 20 us. No window needed, so no keystroke trace in the sway container.

**The black column.** Cause: with the second pane at the left and its minimap beside it, the minimap
draws the dividing line at the right of the pane, but `split_inner` still cut the text by one pixel
on that side (as if the region drew the line), and nothing painted that pixel (x=543, y 84..773 in
`compare-saved`). Fix: `split_inner(rect, other_left, px, minimap)` keeps the whole region in that
one case; the three callers pass `areas.minimap_other.is_some()`. Test
`a_split_view_leaves_no_pixel_unpainted` paints the screen with a sentinel, composes in full and
finds none left, over same file or two files, the second pane at either side, minimap on or off, the
share 0.3/0.5/0.7 (a pane too narrow for a minimap goes without, so one side only), scale 1 and 1.5
(it failed on exactly that column before the fix). Shots: only `compare-saved` changed (that column
now the page colour, and the second pane's scrollbars one pixel further right). The alpha prefill in
`compose` stays as a guard.

**The exe size.** Built with the documented cross build at commits of the first-parent history
(`dcf7c7d` is on the `main` side; `features-files` came in with the merge `afb3766`): dcf7c7d 5,892,096;
afb3766 (merge: command palette, compare, replace in files) 6,058,496 (+166,400); eedd8be (one
file in both panes) 6,072,832 (+14,336); 58d27ee (pin a3e2ef0: sister, place, shadow) 6,079,488
(+6,656); 340c153 (hand-off with mdview) 6,107,648 (+28,160); e2aae5b (live preview) and d3d3643
6,111,744 (+4,096, then 0). So three quarters of the 220 KB is the palette, compare and replace in
files branch, not the hand-off. Cause checked: `Cargo.lock` and the dependency features are the same
at dcf7c7d and d3d3643 apart from the shared pin; a link map of the unstripped build has no wayland,
zbus, x11rb or xkbcommon symbol, so nothing Linux-only reaches the Windows exe. `.text` +155 KB,
`.rdata` +10 KB. Symbol sizes (linker map, dcf7c7d against afb3766, `.text`): replace-in-files UI
23 KB and its worker 10.5 KB; the palette 13.8 KB plus the shared palette 10.4 KB and fuzzy match
6.3 KB (they were in the shared crate already but unused, so not linked before); the diff for compare
13.8 KB and compare 5 KB; the app's event code for the three +48 KB. That is about 3,500 new lines
of non-test code at `opt-level = "s"`: honest code, no fix, the release profile untouched.

## Window shadow routes, settings shots, DESIGN, timings (2026-11-14, after the hand-off work; not published; in 0.5.0)

Part A is commit `1d96351` (shadow), part B the commit after it. Design: `docs/DESIGN.md` sections
54 (hand-off, live preview) and 55 (shadow). The shadow is the viewer's adoption copied
(`~/src/md/view` `7734d6a`, merge `5dad417`; `markdown-shared-rs/docs/LINUX.md` "Window shadow").
Shared pin is now `99f810c` everywhere; winit and softbuffer come from the two patched forks
(`[patch.crates-io]`), and `mdshared-ui` has `client-margin`.

**What was built.** `Shadow::prepare`, `attach_mode(.., Mode::from_env())`,
`set_premultiplied_alpha(true)` when `wants_alpha()`, `set_min_content_size`. `App.size` is the
content size; `Resized`, `ScaleFactorChanged`, `CursorMoved` and the saved window size convert; the
present path puts the content at the margin offset (rationale per line in the commit message and
DESIGN 55). The edge test has no lower bound (shared `edge_at`; tests for x < 0). Placement
(`--place`, side by side) hands `place::set` a window-geometry rectangle; on KDE that is KWin's
`frameGeometry` on both routes, elsewhere on Wayland `set` returns false and the toast says so, so
the margin cannot make it wrong there. Not handled: forced `MD_SHADOW=client` on X11, where
`place::set` (shared) subtracts outer minus inner and the inner size includes the margin; Auto
never picks that route.

**Alpha finding.** Shared `Bitmap` fills, blends and blits keep or write 255, and the minimap's
bitmap is cleared (255) before it is blitted. `Bitmap::new` is all zero, though, so a pixel no region
paints stays at alpha 0. A temporary scan of all 158 shots found exactly one: `compare-saved`
(minimaps on in a compare view), column x=543, y 84..773, black in the shot before and after. Cause
(not fixed, left as drawn): the other pane's rectangle is cut by one device pixel on the divider
side (`split_inner`) although its minimap, not the divider, is at that side. Fix made: `compose`
sets alpha 255 when it allocates `screen` (colours unchanged); test `a_composed_frame_is_opaque`.
Proposed for shared: `Bitmap::new` should initialise alpha to 255 as its doc comment says.

**Proofs run (this machine, headless: no window on the host display, no pointer, no keys).**
- `cargo test --release --workspace`: 596 passed, 13 ignored (593 before; new: saved size over five
  restarts at six scales, resize band outside the content, opaque frame; plus the ignored cost
  test below). Clippy `-D warnings` and `fmt --check` clean.
- `tests/shots.sh`: the 158 shots of the build before part A and after it are byte-identical
  (`cmp`); part B adds 4 (`settings-preview*`), the 158 are still identical.
- Windows cross build (`tools/release-cross.sh --no-installer`, Docker, cargo-xwin): compiles with
  `--locked`; exe 6,111,744 bytes after part A, and the same 6,111,744 at `d3d3643` (different
  hashes, section padding). **That is over the 6,000,000-byte budget and was before this work**
  (the 0.5.0 release exe was 5,892,096; the hand-off work in between grew it). Not run on Windows.
- Live window, sway headless in Docker (image `mdedit-sway`, built in
  `~/.cache/md-linux-test/edit-sway/` from archlinux with sway, swaybg, grim, imagemagick;
  `WLR_BACKENDS=headless WLR_LIBINPUT_NO_DEVICES=1 WLR_RENDERER=pixman`, 1600x1000 output with a
  light solid background, `--cap-add SYS_NICE` because the sway binary has file capabilities;
  scripts `run.sh`, `inner.sh`, `steps.sh`; the release binary and `fixtures/` mounted read-only;
  `mdedit --no-session sample.rs`; screenshots with `grim`, outputs in
  `~/.cache/md-linux-test/edit-sway/out/`). Client route (Auto on sway) against `MD_SHADOW=none`:
  | state | `swaymsg get_tree` rect, client | rect, none | content crop vs none (`compare -metric AE`) |
  |---|---|---|---|
  | start | 160,100 1280x800 | same | 0 differing pixels |
  | `resize set 900 620` | 350,190 900x620 | same | 0 |
  | fullscreen | 0,0 1600x1000 | same | 0 (the whole output) |
  | fullscreen off | 350,190 900x620 | same | 32 px: the caret (2x19 at 61,84), blink phase |
  The rectangle equals the content size, the shadow shows around it (e.g. 10 px left of the window
  the background goes 224 to 214; none: 224), no stale margin after the resize, fullscreen has none.
  Screenshots: `client-initial.png`, `client-resized.png`, `client-full.png`, `client-back.png`, and
  the `none-*` set, in the same folder. Looked at: `client-initial.png`, `client-resized.png`,
  `client-full.png`.

**Part B.**
- Shots looked at (`~/.cache/md-linux-test/edit-base/shots-b/`): `settings`, `settings-narrow`,
  the new `settings-preview`, `-dark` (not opened), `-scale`, `-narrow` (the Preview section scrolled
  into view with the new `--settings-section Preview` shot flag), `menu-view-md-linked-dark`,
  `menu-view-rs-linked`, `menu-tab-md-linked-dark`, `previewing`. Nothing was off: the help line
  of "Live preview while typing" fits whole in all four layouts, the nav marks Preview, the
  entries and their checks are aligned, the status strip says "Previewing". The first screen of
  the existing settings shots never shows the new row (it is below the fold), hence the new shots.
  The help-line test already iterated every row; it now also asserts the live preview row exists with
  a help line.
- DESIGN 54 (hand-off, live preview) written against SISTER.md.
- Timings, in the sway container (the only way to get a window headless); before-build (`d3d3643`)
  and after-build interleaved, 8 rounds each, `sample.rs`, 300 keystrokes, 200 scroll steps; min /
  median:
  | | before | after (client shadow) | after, `MD_SHADOW=none` |
  |---|---|---|---|
  | window shown (ms) | 72.7 / 77.0 | 71.8 / 74.6 | 72.4 / 74.7 |
  | private memory (MiB) | 19.1 / 19.3 | 19.2 / 19.25 | 19.1 / 19.1 |
  | keystroke p50 (ms) | 0.55 / 0.56 | 0.49 / 0.50 | 0.48 / 0.49 |
  | keystroke p95 (ms) | 0.63 / 0.75 | 0.54 / 0.86 | 0.51 / 0.53 |
  | scroll p50 (ms) | 0.50 / 0.50 | 0.44 / 0.45 | 0.43 / 0.44 |
  No regression; the after builds are a little faster on keystroke and scroll (the pinned shared
  code and a changed link differ too, so it is not credited to the shadow). The Linux binary
  grew 12,033,952 to 12,075,104 bytes. Numbers are from sway headless (pixman, no GPU), not
  from the KDE session.
- Cost of the live preview's send on the UI thread: test `live_send_cost_on_the_ui_thread`
  (`app/handoff.rs`, ignored; run with `cargo test --release -p mdedit live_send_cost --
  --ignored --nocapture --test-threads=1`). A 193,500-byte document in 8,400 lines, 300 edits, a
  listener standing in for the viewer: `link_poll` with a send is 0.3 microseconds p50 and p95
  (max 1.4 to 3), with nothing due 0.07 microseconds; the edit itself 1.6 microseconds. The
  rope snapshot and the slot hand-over are what it does; the sends were real (the stand-in
  received updates with the whole 193,500 bytes; the courier coalesces, 3 of 300 arrived).

**NOT run.** Anything on the host display (nothing was opened there), the pointer or keyboard
(so: the edge resize from the outer band, a click through the margin, `drag_resize_window`,
focus changes with the real compositor), the Windows exe, GNOME (Mutter), X11 and Xwayland, KDE's
own route in a window (the KWin route is the old code and the shots show it unchanged), fractional
scales, `--place` and side by side with the client route (no desktop to place on in sway; KDE's
script not run). The owner has not looked at the shadow or the settings shots.

## Hand-off with mdview (2026-11-14, parts 1-4; not published; in 0.5.0)

Built: shared pin 1e39bc1; receiving (`--from`, `--follow`, `--place`, `--goto L[:C]`, `--run`, `--demo-link`; requests
from a second launch go to the running window); "Preview in mdview" (Ctrl+Shift+V, View and tab menus, palette) and
"Preview side by side"; linked mode (`bins/mdedit/src/link.rs`: at most 10 updates a second, newest wins, a courier
thread so a hung viewer never blocks the window, nothing runs unless a link exists); live preview of unsaved text
(setting "Live preview while typing", on by default; the whole text and caret line, never over 2 MiB, one toast).
Ctrl+Shift+V still pastes in the find box. Tests: 593 pass, clippy and fmt clean.

Ran for real (KDE Wayland, DP-2, both apps running): side by side placement (editor 0,0 1720x1396, viewer beside it);
a scripted edit shows in the viewer unsaved with the live dot in its tab. When running two apps on one machine for
this, each needs its own settings folder and BOTH variables for the other app: `MDVIEW_SETTINGS` and
`MD_SISTER_MDVIEW_SETTINGS` for the viewer, `MDEDIT_SETTINGS` and `MD_SISTER_MDEDIT_SETTINGS` for the editor (the viewer's
liveness check looks the editor up by the second one and otherwise ends the link after 5 s).

NOT done: part 5 (window shadow adoption), timings (keystroke with a link, send cost), the `--goto 20` second-launch
check, regenerating the shots (settings shots will differ after the new row), DESIGN.md section. NOT run: Windows,
X11, GNOME, anything interactive.

## One file in both panes of the split (2026-11-14, after the first 0.5.0 work, not published; in 0.5.0)

Design: `docs/DESIGN.md` section 53 (written before the code). Commits on `main`: `a778d7f` (the
flaky test), `b115477` (the design), `2c306c3` (the refactor, no behaviour change), `eedd8be` (the
feature), `1938963` (tests); the docs commit after them.

- **The flaky test** `a_change_on_disk_does_not_replace_unsaved_text`: `open()` starts a load on a
  thread and `load_now()` replaces it. When the first thread had finished, its result sat in the
  queue, and `wait_load` woke on it although `apply_load` then dropped it (old revision), so the
  test went on before the real load had finished. `wait_load` now drops results of replaced loads
  and closed tabs before it decides to wait. A new test reproduces it deterministically; the old
  one passed 200 runs twice under a full `cargo build` load.
- **Refactor (`2c306c3`)**: `ViewState` holds what one way of looking at a document is (carets,
  scroll, folds, colours, find mark, wrap cache); `Ready.view` is the pane with the keyboard,
  `Ready.twin` the other. Proof: 147 of 147 shots byte-identical to the build of `f6c92d5`, 555
  tests, clippy and fmt clean.
- **The feature**: "Show this file in the other pane" (View menu, tab menu, palette; no key: none
  was obviously free). Both panes keep their own caret, selection(s), scroll, folds and minimap;
  an edit, Undo, Redo, Replace all or a macro in either is mapped through in the other
  (`Change::pieces`, `Change::map_offset` in core); the tab appears once, with the second-pane
  line under it running the width of the tab; closing the tab closes both views; unsplitting (or
  opening another tab for the keyboard) keeps the view that has the keyboard (or leaves the
  document in the other pane with its view); "Compare with the other pane" is greyed for one
  file; the session keeps a same-file split as the existing `split=` line (the path is the active
  file's) and restores it with a second view. Highlight requests carry a `slot`, so the two views
  of one document get their own colours from the one parse.
- **Decisions made without the owner**: folds per view (not shared); find marks and the match list
  shared, the current match per view; bookmarks shared; the unfocused view's caret is not drawn;
  the focus keys swap the two views and the panes' sides, as for two tabs; "Compare with file" or
  "with saved" end the same-file split first; flags for shots: `--split-same`, `--other-goto`,
  `--other-select`, `--other-fold`, `--other-scroll`; the three-way difference of the view-menu
  and tab-menu shots is the new row only.
- **Tests**: 568 pass, 12 ignored (554 before this work; 14 new: 2 core `Change` mapping, 1
  highlight slots, 1 `wait_load`, 5 in `docs.rs` (mapping through undo/redo, replace all and
  several carets, folds, find per view, closing/unsplitting) plus 1 for another tab taking the
  keyboard, 4 in `files_tests.rs` (commands and menus, a macro, the session, closing), and an
  extended session test).
- **Shots**: 151 now. The 147 earlier ones are byte-identical to the baseline except the six that
  list the new command (`menu-view`, `menu-view-checked`, `menu-view-checked-dark`,
  `menu-view-compare`, `menu-main-view`, `menu-tab`). New: `same-file-split` (selection in the
  left pane, a fold and another place in the right, minimaps, light), `same-file-split-dark`
  (scrolled differently, minimaps), `same-file-split-left` (second pane at the left, word wrap,
  Markdown, 900x600), `same-file-split-find` (find marks in both, dark, minimaps). I looked at
  all four and the two menu shots.
- **Measured** (this machine was loaded by other work: load average 16 to 29, so medians are
  noisy and the minima are the fairer comparison; 10 interleaved runs, base = `f6c92d5` build):
  | | base min / median | now min / median |
  |---|---|---|
  | `sample.rs` window shown (ms) | 88 / 95 | 82 / 97 |
  | `sample.rs` private memory (MiB) | 20.6 / 20.7 | 20.6 / 20.65 |
  | `sample.rs` keystroke p50 (ms) | 0.54 / 0.57 | 0.51 / 1.04 (one pair of noisy runs) |
  | `sample.rs` scroll p50 (ms) | 0.47 / 0.55 | 0.46 / 0.49 |
  | `50k.rs` (quieter second batch) shown | 70 / 90 | 76 / 86 |
  | `50k.rs` keystroke p50 / p95 | 0.47 / 0.51 and 0.49 / 0.94 | 0.47 / 0.50 and 0.49 / 0.65 |
  | `50k.rs` scroll p50 | 0.45 / 0.46 | 0.45 / 0.46 |
  | `100mb.txt` shown / private / keystroke p50 / scroll p50 | 182 / 124.4 MiB / 0.63 / 0.65 | 181 / 124.4 MiB / 0.69 / 0.74 |
  50,000-line file in both panes with the second at line 25,000, typing in the first (300
  keystrokes): p50 0.66 min / 0.69 median, p95 0.71 / 1.14, private 23.1 MiB (one pane: 22.1);
  with both minimaps: p50 1.10 / 1.12, p95 1.14 / 1.53, scroll p50 0.65 / 0.68. All far under
  the 8 ms budget. Colouring the second pane's far position costs the parse up to there (about 1.8
  s of CPU to line 40,000), the same as `--goto 40000` already did in one pane.
- **Not run**: everything interactive (no mouse, no keyboard in a real window: the menu and tab
  entries were clicked by no one, the tab underline was seen only in shots); Windows (nothing of
  this ran there); the owner has not looked at it. Not built: a key for the command; the second
  view's caret drawn while unfocused; folds shared as an option.

## Typing, Markdown, settings, shadow (2026-11-14, 0.5.0, not published; in 0.5.0)

One machine (CachyOS, Wayland); nothing run on Windows, nothing driven by hand: tests, `--shot`
and `--trace` only. Design: DESIGN.md sections 43 to 48. Commits: A `4149108`, line commands
`a2fb6eb`, typing help `d24e906`, Markdown `2f23d38`, settings `76ffe89`, then shadow and 0.5.0.

- Sent-back items: in-selection replace governs counter, current match, marks and minimap; fold
  chevron without tooltip, centred in the gap; the fold-box tooltip clip is in the shared crate
  (`Bubble::tip`/`Bubble::hint` in mdshared-ui overlay.rs size the box from a fractional width;
  `w.ceil()` there is the proposed fix; not rebuilt or verified).
- Keys: Ctrl+J, Ctrl+B, Ctrl+I, Ctrl+Shift+H, Ctrl+, were free; Link has none.
- Review round (same day): submenu placement (`sub_side`), indent with spaces for blocks (`indent_with`/`outdent_with`), settings help lines fitted, shared pin `9c1290b`, replace box aligned with the find box (DESIGN 49).
- Timing (sample.rs, 300 keystrokes, noisy machine): before p50 0.51-0.62 ms, p95 0.76-1.52 ms;
  after p50 0.54-0.90 ms, p95 1.09-1.30 ms; first frame 74-100 ms both; private 20.6 MiB.
- Tests: 525 pass with `--release`; clippy and fmt clean. Shots in `~/.cache/md-linux-test/b4`.
- Not run: anything interactive, Windows, Wayland shadow (KWin), the settings dialog by hand, the
  AppImage "make default" row, "Check now".
- Commit trailers from here on name Claude Sonnet 5.5 (the earlier ones in this series, Fable 5.2).

## Command palette, compare two files, replace in files: the interface (2026-11-14, branch `features-files`, not merged at the time, not published; in 0.5.0)

Wired three finished pieces of headless logic into the window, on the branch `features-files` (from
`79e70ec`). Done on one machine (CachyOS, Wayland); nothing run on Windows; nothing driven by hand
(only tests, `--shot`, and `--trace --exit-after-ms`/`--type-steps`/`--scroll-steps`). DESIGN 50
(palette), 51 (compare in the window), 52 (replace in files in the window).

**Built**
- **Command palette** (`Ctrl+P`): `palette.rs` (sources), `app/palette_events.rs` (the host).
  Open tabs, recent and the folder's files (worker, 5,000 cap, kept until the folder changes), `>`
  commands with hints and greyed ones, `:` go to line, `@` symbols, `?` the prefixes.
  `--palette <query>`.
- **Compare two files**: `compare.rs` (`Align`, `Compare`, the worker with a 150 ms debounce),
  `app/compare_events.rs` (commands, F7/Shift+F7, the per-frame checks), changes in `view.rs`
  (filler rows, tints, marks inside lines), `minimap.rs` (tints, no text on fillers), `docs.rs`
  (`Ready::align`, `read_only`, `Doc::title`, `Docs::open_copy`), core `fold.rs` (`Folds::suspend`).
  Compare with the other pane, with a file, with saved (a read-only "name (saved)" tab at the left);
  in the View menu, the tab menu and the palette. `--compare A B`, `--compare-saved`,
  `--next-difference N`. No merge arrows.
- **Replace in files**: `replace_ui.rs` (state, workers, dialog and report drawing),
  `app/replace_files.rs` (the steps); a replacement box and "Replace all…" in the results panel, the
  confirmation dialog (tick boxes, preview, backups, skipped files), a progress line with Cancel, the
  report and "Undo replace". `--results-replace`, `--replace-dialog`, `--replace-report` for shots.

**Decisions made without the owner**
- F7 and Shift+F7 for next and previous difference; no existing binding used them. `Ctrl+P` for the
  palette (it was unbound).
- The saved copy is at the left, the edited text at the right. The View menu has the three start
  commands and Stop comparing; the difference keys are in the palette and on F7 only. The tab menu
  has Compare with file… and Compare with saved.
- While comparing, the Word wrap item still flips the setting but does nothing until the comparison
  ends; fold commands do nothing (folds are kept and follow edits, and come back).
- The marks inside a line are not drawn while the diff is out of date; the strip says "Comparing…".
- Difference colours are fixed per theme in `compare.rs` (the shared palette has none).
- A greyed command in the palette cannot be chosen; the file list is not built for the empty query.
- Replace in files: Enter replaces and Escape cancels in the dialog; replacing with an empty text
  is allowed; the preview shows the first lines of the file, not a diff of all of it; a new search
  or a replace ends the previous report and its undo.
- A fixture pair, `fixtures/compare-a.rs` and `compare-b.rs` (shots), and `shots.sh` makes a fresh
  folder for the replace shots (the report shot really replaces).

**Checks run**: `cargo test --release --workspace` 517 passed before (the branch point), 542 after
(25 new: 17 in `app/files_tests.rs`, of them 9 for the palette, 1 for replace in files and 7 for
compare; 5 in `compare.rs`; 1 each in `fold.rs`, `args.rs` and `replace_ui.rs`); clippy `-D warnings` and `fmt --check`
clean. `tests/shots.sh`: 98 shots before, 117 after; the 19 new ones were looked at (palette x8,
compare x7, replace in files x4). Of the 98, 86 are byte-identical and 12 differ, all because of
what was added: `menu-main`, `-dark`, `menu-main-view`, `menu-main-macro-dark`, `menu-tall-main`,
`menu-tall-main-end` (the main menu has "Command palette…"), `menu-view`, `menu-view-checked`,
`-dark` (the Compare items), `menu-tab` (the two compare items), `dialog-shortcuts` (new commands),
`files-pane` (the fixtures folder holds two more files).

**Measured on Linux (window shown, `sample.rs`, 10 interleaved runs of each, branch point against
the branch)**: window shown median 73.6 ms before and 72.8 ms after (range 68.5 to 100.2 and 67.5
to 99.3); private memory 20.6 to 20.8 MiB both; idle CPU 80 ms both; keystroke p50 0.47 ms both;
scroll step p50 0.42 ms both (6 runs each); the exe grew 11,476,744 to 11,683,112 bytes. With
compare on (two 63 and 72 line files): keystroke p50 0.41 to 0.42 ms, scroll p50 0.34 to 0.35 ms;
two 50,000 line files with 27 differences: keystroke p50 0.42 to 0.43 ms (p95 up to 0.71),
scroll p50 0.40 ms. A first build of the compare work scrolled 0.07 ms slower (paint 0.27 to 0.34):
bisected to `Docs::sync_wrap` taking `ready_mut()` and writing the fold flag on every frame;
it now writes only when the value changes and the figures are the same as before. Not confirmed
from the trace that the compare was on while the `--type-steps` keystrokes were made (it starts
when both files are read, before the steps, but I did not print it).

**Not run**: anything interactive. Nothing was clicked or typed by hand: the palette (opening,
keys, clicking a row, the wheel), the compare commands from the menus and the file dialog
(Compare with file…), F7, scrolling and typing in a compared pane by hand, the minimap click in
compare mode, dragging the divider while comparing, the replace dialog's clicks, tick boxes and
preview under the pointer, Cancel while a large replacement runs, Undo replace by click. Windows:
nothing; the blocking dialog path for Compare with file… (`#[cfg(windows)]`) was written the way
Open files is and has not been built. A real replacement of many files, and a compare of two very
different large files (the approximate case), were not timed in a window. The shots and the tests
stand in for these only as far as they go; the owner has to look at the interface.


After the merge of `origin/main` (dcf7c7d) into `features-files`: 554 tests pass in the workspace
(153 mdedit, 335 core, 30, 14, 21 and 1 in the other crates), clippy `-D warnings` and `cargo fmt
--check` clean; the shot set is 147 (128 on main plus 19 new). 13 of main's 128 differ: the six
menu shots with the View and tab menus' Compare items (menu-main*, menu-view*, menu-tab, menu-tall-*),
dialog-shortcuts (new keys), files-pane (the two new fixtures) and dialog-recent (the path of the
checkout it was rendered in). One test, `a_change_on_disk_does_not_replace_unsaved_text`, failed
once in the first full run and passed in five later runs; it was not looked into. Timings were
measured before the merge, not again after it.

## Gaps closed: icons, chevrons, split, macros, find memory, title row (2026-11-14, still 0.4.1, not published; in 0.5.0)

Done on one machine (CachyOS, Wayland), nothing run on Windows, nothing driven by hand: only
tests, `--shot`, and `--trace --exit-after-ms`. Merges first: `3daf35d` (core-edit) and `b4f26e7`
(core-files, with the core's `fuzzy.rs` removed). Shared pin `d6e6cf1` (later `9c1290b`).

- **Icons**: every menu item has its glyph, the replace strip's toggles are icons, shots for the
  menus that had none (`--menu` names added; see `tests/shots.sh`). DESIGN 11.
- **Fold chevrons** (DESIGN 37): `10f8d25`. Unit tests for the hit cell and the click.
- **Split view** (DESIGN 38): divider, ratio/side/file in the session, unfocused-pane click,
  scrollbars, Shift+wheel, tab underline. `f579149`.
- **Macros** (DESIGN 39): `macros.txt`, named, bounded. `1d65863`.
- **Find memory and replace in the selection** (DESIGN 40, 41): `c44cdf8`.
- **Title row** (DESIGN 42): `a65b61e`.

**Same file in both panes: what would have to move.** Not built (the owner said leave it). The
caret and selection (`sel`, `extra`), scroll (`top`, `top_row`, `left`), folds (`folds`), find marks
current (`find.current`) and the colours (`colors`, `map_colors`, keyed in the highlight service
by `DocId`, with `peek` for the second pane) are fields of `docs.rs` `Ready`. A second view of the
same document would need these in a per-view struct (one per pane), with the buffer, history and
find matches staying in `Ready`; the highlight service would be keyed by (document, view), the
minimap plan already is per pane (`map_lines`, `map_plan`), and `Region::Other` would draw a view,
not a tab. `Docs::other` is a tab index today; it would become a view of any tab.

**Decisions made without the owner**
- Fold chevrons: hit area is the right 24 px of the gutter row, not the whole gutter.
- Up/Down in find and replace boxes take the history only when it is not empty (DESIGN 40).
- "In selection" is on by default only when the selection spans more than one line; the match
  counter still counts the whole file; the toggle shows only while a range is kept.
- Macros: playing a saved one makes it the "last" one that the play keys run.
- Title row: deferred drag off Windows only; the 4 px slop is a guess.
- Second pane's tab marker: a 2 px muted underline, chosen to stay quieter than the active tab.

**Measured on Linux (window shown, `sample.rs`, 10 interleaved runs each)**: before 70 to 92 ms
(median about 82), after 68 to 86 (median about 80), so no change; private memory 20.6 to 20.8 MiB
before and after; keystroke p50 0.52 ms before, 0.49 after; scroll step p50 0.44 ms both. The exe
grew 11,409,136 to 11,476,832 bytes (this Linux build, all of the work above). Macro and find
history loading: macros are read on first use; the session's histories are 40 short strings. A
launch with a session holding full histories was not timed.

**Checks run**: `cargo test --release --workspace` 516 passed (404 at the start of the work),
clippy `-D warnings` and `fmt --check` clean; `tests/shots.sh`: every shot without a menu, a
replace strip, fold, minimap-fold or split was byte-identical to before each part; new shots were
looked at one by one.

**Not run**: anything interactive. Not run by hand or seen: dragging the divider, the clicks in the
second pane, the fold chevron hover and click, Up/Down in the boxes, the in-selection toggle with
the mouse, the macro prompt and dialog, the title row double click on Wayland (it needs the
mouse; only the decision is unit tested). Nothing was built or run for Windows, including the
deferred title drag's `cfg`. The fold-box tooltip in `fold-box-hover` is cut ("Open the block
(F...") by the shared bubble width; not investigated.

## Linux: the shared Linux code adopted (2026-11-14, still 0.4.1, not published; in 0.5.0)

Run on one machine: CachyOS, KDE Plasma on Wayland, cargo 1.99, build dir
`~/.cache/md-linux-test/edit`. Shared pin moved to `84de734a1aaecc50cca80b23bacee127b6778ef3` (now `d6e6cf1`, see the 2026-11-14 icons section)
(branch `feat/linux`, not yet on shared `main`). Everything below is `cfg(not(windows))` unless
said; **nothing was built or run for Windows**. The Windows code paths are the old ones.

- **Repaint**: `present` copies the whole frame unless `buffer.age() == 1` (was `== 0`).
  Wayland swaps two buffers (age 2) and a resized buffer keeps its age. Not seen on screen: the
  glitched toolbar on maximise was not re-tried (no driven tests on this run).
- **File dialogs**: Open files, Open folder and Save as use the shared `pick_*_async`; the helper
  thread's `done` only posts `Wake::Dialog(Picked)`, handled on the UI thread (`dialog_done` in
  `app/commands.rs`). `dialog_busy` ignores a second request. Save as inside a flow: `save_as`
  returns `SaveAs::Waiting`, `Picked::Save` carries the document id and the `After`; a result calls
  `request(after)` (as the Save button did), a cancel, a failed write or a closed tab calls
  `cancel_pending`. Windows keeps the blocking calls (`platform::pick_files`, `pick_save`).
  `rfd` is no longer a direct dependency off Windows. **Not run**: no dialog was opened (the
  owner's PC); the flows are untested beyond compiling. Known gap: while a Save as dialog of a
  close flow is open the window is live, and another close request made meanwhile would have its
  question cleared by the cancel.
- **Fonts**: the editor's `EXTRA_FALLBACKS` is gone; Windows calls `mdshared_text::add_fallbacks`
  on its font copy, elsewhere `shared_fonts()` has it. Code font string off Windows ends
  `..., Noto Color Emoji, monospace`. Looked at in `md-light.png`: the emoji is a coloured face.
- **Window identity**: `window::identify(attrs, "mdedit", assets/mdedit.png)`;
  `desktop_entry::ensure(..)` once in `run`. Run with `$APPIMAGE` set and `XDG_DATA_HOME` on a
  scratch folder: `mdedit.desktop` and the icon were written, the entry looks right. New command
  `MakeDefault` ("Make mdedit the default for text files", no key), in the main menu after
  "Keyboard shortcuts", only off Windows, only when `$APPIMAGE` names a file, hidden once
  `is_default`. Seen in a `--shot` with `$APPIMAGE` set; the command itself was **not run**
  (it would edit the owner's `mimeapps.list`).
- **Updates**: `--check-update` reaches GitHub on Linux (the manifest says 0.4.0, so "nothing
  newer"; a 404 and a refused connection give `Status(404)` and `Os(7)`). With `$APPIMAGE` set the
  launch check ran (`update_checked` written to the settings). A local http manifest is refused
  ("not an https URL"), so the bar and Update were **not** run with a real offer. The wording
  "Only an installed copy can update itself." (shared `NOT_INSTALLED`) is acceptable on Linux but
  says nothing about AppImage; left to shared.
- **`--trace`** on Linux: memory from `/proc/self/status` (`RssAnon` as private, `VmHWM` as peak)
  and CPU time from `/proc/self/stat` (user + system, 10 ms ticks). Unit-tested on sample text.
- **Not done** (fixed later the same day, see the section above): double click on the title row did not maximise (a likely fix is to call `drag_window` only on the first pointer move with
  the button down, so that a double click never starts the compositor's move grab). "Show in file
  manager" still opens the folder only.
- **Checks**: `cargo test --release --workspace` 404 passed (403 before plus one);
  clippy `-D warnings` and `fmt --check` clean. `tests/shots.sh` before (HEAD `7eb4b9b` with the
  old pin) and after: 74 shots, 66 byte-identical; the 8 that differ are exactly those with an
  emoji (md-light, utf16le, utf16be, outline-dark, split, minimap-dark, minimap-split, tabs).
- **Budgets on Linux (information)**: window shown 72 to 78 ms on `sample.rs`; private memory
  20.6 to 20.8 MiB (peak 49 MiB); 100 MB file: loaded 115 ms, window shown 190 ms, private
  124.5 MiB; keystroke p50 0.48 ms, p95 0.57 ms (300 keystrokes).

## Menus taller than the window scroll (2026-11-13, 0.4.1, not published; in 0.5.0)

Found in a real window 665 to 760 logical px tall: the main menu rose to the window's top edge
over the caption buttons and lost "Exit". It depends on the window's height, not the scale.

- **What existed:** nothing scrolled. `mdshared_ui::menu::Menu::place` clamped a menu into the
  window with a 4 px margin and cut its height (`h.min(height - 8)`), so the rows below were
  unreachable and the menu covered the title row.
- **Shared** (branch `fix/tall-menus` of `markdown-shared-rs`, from `chore/finalize`, head
  `f49edd3`; the shared `main` was not moved): `Menu::place_in` / `draw_in` take the area a menu
  may use. A menu pushed up to fit stops at the area's top (below the title row); one taller
  than the area is as tall as fits and scrolls: `scroll_by` (wheel), `step` keeps the highlighted
  row in view (and wraps as before), a thin scrollbar (`scroll::scrollbar`) shows the position,
  hits use the placed, scrolled rows and only inside the menu, `row_rect` and `scroll` give the
  placed row. `scroll_to_end` is for shots. `place` and `draw` are the old calls (the whole
  window). A menu that fits and was opened inside the title row (the tab's own menu opens at the
  pointer) stays where it was opened. Four unit tests.
- **Editor:** `bins/mdedit/src/app.rs` gives both menus `menu_room(title, w, h)` (the window below
  `areas.title`; the pre-placement for the damage rectangle must use the same room, or the two
  calls clamp the scroll differently) and the overlay signature includes the scroll;
  `app/input.rs` `wheel` scrolls the menu under the pointer when it is taller than its room and
  closes the menus as before otherwise; `--menu main-end` and two shots (`menu-tall-main`,
  `menu-tall-main-end`, 1000x520) in `tests/shots.sh`, now 74 PNGs.
- **Ran:** `tests/shots.sh` before and after: the 72 existing PNGs byte-identical; the two new ones
  looked at. `cargo test --release --workspace` 404 passed (the editor has no new test of its
  own: the logic is the shared one), clippy `-D warnings` and fmt clean. Real window at the
  display's scale 1.0, 1000x560: the main menu below the title row with Exit reachable; the
  wheel over it scrolled it to Exit; "About mdedit" clicked opened its dialog; Down 20 times
  wrapped to the first row, Up from it to Exit with the menu scrolled to the end; Right on Macro
  opened its submenu inside the window; the encoding picker opened above the status strip inside
  the window. Measured (release, small file): exe 5,679,616 bytes (5,678,592 before), window shown
  about 77 to 85 ms (one cold run 236 ms), private memory 27.8 MiB, keystroke p50 0.48 ms.
- **Not run:** another display scale (the display was at 1.0), a context menu or submenu taller
  than the window, a scrollbar drag (the bar only shows the position), a touch pad's pixel wheel,
  Linux. The user has not seen it.

## Clean-up of the three repositories (2026-11-13, no release)

Headless work, asked by the owner to leave the code finished. No version change.

- **Shared** (`chore/finalize` at `24865c593279c977d610ba10c4df4e0ada9a371b`, on top of
  `1c3710a`; the shared `main` was not moved): `cargo clippy --workspace --all-targets -- -D
  warnings` passes. `chunks_exact(N)` became `as_chunks::<N>()`, `% 2 == 0` became
  `is_multiple_of`, `Gfx` got `Default`, a test's `clone()` into a slice became `from_ref`, and
  five drawing or button functions and one test type got an `#[allow]` with a reason. New
  `mdshared_ui::menu::row_span(items, i)` gives a row's offset below the menu's top and its
  height, with a test tying it to the rows `Menu::place` lays out. Shared tests and fmt pass.
- **Editor:** all five `rev`s moved to `24865c5`. The editor had no mirrored menu numbers in
  its code (its cascade follows the pointer, or the menu's top for the keyboard); only the test
  helper `row` in `ui.rs` used `4.0` and now uses `row_span`.
- **Debug tests.** `a_request_longer_than_one_slice_is_answered` asked 5,000 lines of an
  unoptimized highlighter, which is about a hundred times slower and worse than linear (1,000
  lines took 1 s, 2,000 took 3.4 s). A debug build now asks 1,000 lines (it passes in under
  2 s); release still asks 5,000. The whole workspace ran once in debug: 404 passed, none failed,
  the `mdedit` test binary 29 s in all (no single test timed; none could pass 30 s).
  The "four `mdedit-highlight` tests fail in debug" note in section 4 was from before and
  is gone.
- **`first_line`** in `platform.rs` is now `#[cfg(any(windows, test))]`: only the Windows
  clipboard reads UTF-16. Not compiled for Linux: the target is installed but its system
  libraries (fontconfig, X11 `pkg-config`) are not on this PC; the cfgs were read instead.
- Checks: `cargo test --release --workspace` 404 passed; clippy `-D warnings` and fmt clean;
  72 shots byte-identical to a baseline from the commit before; exe 5,678,592 bytes. The
  first comparison differed in every picture because the Windows display scale had changed
  between the two runs (1280x800 against 2240x1400 pixels); `tests/shots.sh` now pins
  `--scale 1.0`, and the baseline was rebuilt from the old code with the pinned script.

## Minimap tested in a real window, and 0.4.0 (2026-11-13)

The owner asked for the left-out runs and then a release. Windows, launched from Git Bash,
scratch `MDEDIT_SETTINGS` files, mouse and keys by `SetCursorPos`, `mouse_event`, `keybd_event`,
a screenshot or a file check for each. No defect was found, so no code was changed (the version
went to 0.4.0 and nothing else).

- **Run, with what was seen:** (2) split view of two 50,000-line files: a press and a 60 px drag
  on the second pane's column scrolled only that pane (10,492 against 5,246 in the other), and
  the keyboard went to it (tab and status line); (3) folds (Shift+F9, thousands) and word wrap
  together: a drag moved the band and the text with wrapped lines in view; (4) 50k.rs beside
  100mb.txt in a split with both columns: a drag on each column, ten wheel notches, letters
  typed in each file and a click between them took 3.1 s of scripted steps in all, no stall; (5) the Files pane open, then with the results panel too,
  the window narrowed to 660 px (column hidden) and widened (back), the results panel closed
  (column full height again); (6) the right-click menu: Escape closed it, Down and Enter hid the
  minimap; (7) 453 carets (a column selection extended by the wheel over a long range), scrolled,
  and typed into (all 453 lines took the letters), no stall; (8) idle CPU: 0 to 15.6 ms of CPU
  time per 20 s with the window idle and the column on, the same as the exe built before the
  minimap (0 to 31 ms per 20 s; the clock ticks in 15.6 ms steps); (9) the main menu by keyboard:
  Down to View, Right opened the submenu with its first item lit, Down four times and Enter
  toggled Minimap, Escape and Left each closed only the submenu, Escape again closed the menu,
  Right on "Open recent" showed the scratch settings' recent file, a click on an item in the View
  submenu toggled it; (10) the installer: see below.
- **Not possible or not run:** (1) a scale other than 100 %: `--scale` exists only for `--shot`;
  in a window the scale is the display's, and the owner's display settings were not changed (the
  1.5 and 2.0 shots stay the only evidence). (10) The installer was **not** run interactively: an install of mdedit (0.3.2, `AppId`
  C58A9AB1-...) already exists on this PC, so the wizard would have upgraded it in place; the
  silent variant was run instead, with no tasks, into `target\insttest` (`/VERYSILENT /TASKS=""
  /DIR=...`): the 0.4.0 setup installed the exe, icon and uninstaller there, the installed exe ran
  and Ctrl+Shift+M worked in it, the uninstaller removed the folder and the uninstall key. So the
  wizard's pages, and that the last page now offers only "Launch mdedit", were not seen on screen;
  the script (`mdedit.iss`) has one `[Run]` entry and no Default apps task. **A slip:** the
  uninstall key of the existing install is the same key, so the test install replaced it and the
  test uninstall deleted it, and the Start Menu shortcut with it; I had meant to export the key
  first and the export failed (Git Bash rewrote the `/y`). The existing install was put back by
  running the 0.3.2 setup silently into its own folder with all three tasks: its exe is the same
  file (SHA-256 5872957b...a026, 5,651,968 bytes), the uninstall key says 0.3.2 with its location,
  and the Start Menu and desktop shortcuts, the context-menu entry and "Open with" are there. It
  was not the exact original key (the unins000 files are rewritten), and its task choices were
  inferred from what was registered. Linux was not run.
- **Not run besides:** a drag on the column with folds, wrap and a split all at once; the first
  press of the keyboard menu on a high-DPI display; thousands of carets.
- **Measured** (the 0.4.0 exe, 5,678,592 bytes; budgets: 6,000,000, 100 ms, 30 MiB, 8 ms, 4 ms):
  sample.rs shown 75.4 to 76.5 ms off and 77.0 to 79.1 ms on; private memory 27.7 to 27.9 MiB off
  and 27.5 to 28.0 MiB on; keystroke p50 0.47 ms off, 0.59 to 0.60 ms on; scroll step p50 0.41 off,
  0.44 to 0.45 on; 50k.rs 0.48 and 0.44 off, 0.68 and 0.62 to 0.65 on; 100mb.txt 0.54 to 0.55 and
  0.55 to 0.56 off, 0.77 to 0.78 and 0.76 to 0.77 on. Tests 404 passed, clippy and fmt clean,
  72 shots, all byte-identical to the last run but `dialog-about` (it shows the version).

## Minimap, finished (2026-11-13, after the owner saw it: "looks great. finish it. and make it togglable")

First part of the day: the section below. This one builds what it left out. Design in
`docs/DESIGN.md` section 28 (updated). Not released, version unchanged, nothing pushed elsewhere.

- **Toggle.** Ctrl+Shift+M (free: no command had it; Ctrl+Alt is AltGr and was avoided), shown in
  the View menu and the shortcuts dialog. A right click on a column opens a one-item menu,
  "Hide minimap" (`PopupKind::Text`'s machinery, `overlays::minimap_menu`). Recordable in a macro,
  as Word wrap is (nothing to change: `recordable` leaves view toggles in; a test says the two
  agree).
- **Marks:** selection (and several carets), the caret's row (keyboard pane only), find matches at
  the right edge, bookmarks at the left edge. Drawn from the data the editor keeps, only for the
  window's lines, never a search of its own. See the design section.
- **Colour:** 78 % kept (60 % was grey). Compared side by side in a light and a dark shot.
- **Word wrap:** one row per line still. The band's top is exact to a wrapped row and its bottom
  edge counts the last visible wrapped row whole, so **the band can be up to one wrapped row too
  tall**; the scroll fraction is the editor's own (top line over lines less one).
- **Second pane:** each pane has its column (`Region::MinimapOther`, `Hit::MinimapOther`); the
  map request is per document (the worker already had a slot per document). The restructuring was
  the pattern already there (one more region, one more hit, arrays of nine); nothing beyond it.
  A drag holds the document, not the pane (`Drag::Minimap { doc }`), so it survives the panes
  changing roles when a press on the second column gives that pane the keyboard. The panes' two
  columns have the same top and height, which is what a drag after the swap relies on for the one
  event before the next frame has laid the areas out again.
- **Measured** (release, quiet PC, runs alternated; budgets: exe 6,000,000 bytes, shown 100 ms,
  memory 30 MiB, keystroke p50 8 ms, scroll step p50 4 ms):

  | | before the minimap | after, off | after, on |
  |---|---|---|---|
  | exe bytes | 5,651,968 | 5,678,592 | same exe |
  | shown, sample.rs (ms) | 73.7 to 77.5 (a first run at 93.7) | 76.4 to 86.8 (first two runs 119.5 and 104.1, machine busy) | 75.5 to 76.4 |
  | private memory, sample.rs (MiB) | 27.5 to 28.0 | 27.3 to 27.8 | 27.6 to 28.1 |
  | keystroke p50, sample.rs (ms) | 0.52 to 0.53 | 0.51 to 0.52 | 0.63 |
  | scroll step p50, sample.rs (ms) | 0.45 to 0.46 | 0.44 | 0.48 |
  | keystroke p50, 50k.rs (ms) | 0.53 | 0.52 | 0.71 to 0.72 |
  | scroll step p50, 50k.rs (ms) | 0.48 to 0.49 | 0.46 | 0.66 to 0.67 |
  | keystroke p50, 100mb.txt (ms) | 0.60 to 0.61 | 0.59 to 0.61 | 0.79 to 0.81 |
  | scroll step p50, 100mb.txt (ms) | 0.59 to 0.75 | 0.59 to 0.61 | 0.81 to 1.02 |

  Shown with the split and both columns (sample.rs and sample.md): 77.7, 84.0, 96.5 ms (the 96.5
  was the first run); memory 27.8 to 27.9 MiB. Idle CPU with the column on: CPU time at idle and
  at exit equal (no timer). The 1 MB single-line file with the column on: shown 53.7 ms, 16.5 MiB,
  CPU time equal at idle and exit; the 100 MB file: shown 142.8 ms. Typing with the split open
  cannot be timed (`--type-steps` has no split); it was typed in a real window (below).
- **Tests:** 404 passed, 0 failed (400 before this part): the new pure parts (selection columns,
  byte to column, wrapped scroll range and inverse, `Areas::with_minimap` for two panes), the
  recordable toggle. Clippy `-D warnings` and `fmt --check` clean.
- **Shots:** 72 now. New: `minimap-marks` (selection, find, bookmarks, caret row), `minimap-wrap`
  (`--wrap`), `minimap-scale-150`. The 64 older ones are byte-identical to the run before the
  minimap but `menu-view`, `menu-main-view` and `dialog-shortcuts` (they list the command and its
  key). The five minimap shots of the first part changed (caret row, 78 % colour, a column for
  each pane in `minimap-split`). Looked at: all new ones, `dialog-shortcuts`, and a scale 2.0
  shot (rows and columns are whole pixels at 1.5 and 2.0; the ink is 3 px tall and 2 px wide at
  1.5, 4 and 2 at 2.0).
- **Run in a real window** (Windows, scale 1.0, Git Bash launch, `MDEDIT_SETTINGS` scratch files,
  mouse and keys by `SetCursorPos`, `mouse_event`, `keybd_event`, window screenshots): split view
  with two columns (Shift+F8); letters typed in the left pane, a click on the right pane's
  column (the pane took the keyboard; letters typed went into its file), the wheel over that
  column (the pane scrolled, its band followed); the right-click menu, "Hide minimap" (columns
  and the setting gone); Ctrl+Shift+M (back); the window made 700 wide (both columns hidden),
  980 (both back) and 1280; Shift+F9 on the 50,000-line file (thousands of folds), a press and a
  drag on the column; word wrap on in a 760 px window, a drag on the column and the wheel (the
  band moved with wrapped lines in view); the results panel open (`--find-in-tabs`) with the
  column above it, and the find marks at the column's right edge; a plain `.txt` file (a press
  centred the view); the 1 MB single-line file (no stall, typed in it).
- **Not run:** scale other than 1.0 in a real window (shots at 1.5 and 2.0 only); Linux; a drag
  on the second pane's column (a press and the wheel were run); a drag with folds and wrap
  together; two large files in a split; typing with the split timed; the keyboard-only paths of
  the right-click menu (Escape, Enter); the column with the Files pane open; a selection of
  many carets over a long window (the code sorts them and looks at the window's lines only; no
  test with thousands); idle CPU in a real window by a tool (only `--trace`). The owner has seen
  the first part, not these additions.

## Minimap (2026-11-13, asked by the owner: "a minimap like Sublime Text")

`docs/DESIGN.md` section 28 has the design as built. Built to a written design from the main
session; not released, version not changed (still 0.3.2), nothing pushed to other repositories.

- **What it is.** View > Minimap (no key) shows a 96 px column at the right edge of the text with
  the document in miniature: one row per line that shows (folds skipped, word wrap ignored), a run
  of non-whitespace one coloured rectangle, the editor's colours at 60 %, a translucent band over
  the rows in view (8 %, 14 % over the column), the column scrolling in proportion to the editor
  in long files. Press outside the band: the editor centres on that line, a drag goes on from
  there; press inside it: a drag keeps the grab offset; the wheel scrolls the text. Off by
  default; remembered in the session (`minimap=1`); `--minimap` for shots and timing runs.
- **Decisions made without the owner.**
  - **The worker has a second request slot.** `peek` is one slot per document and the minimap
    wants another range of the document the editor is viewing, so `mdedit-highlight` gained
    `HighlightService::map` and a `map` flag on `Colored` (two tests). The worker serves view and
    peek first, then the map in 2 ms slices, then the background parse. This is in this
    repository's crate; nothing in the shared repository changed.
  - **Split view: only the pane with the keyboard has the column.** It was one call to
    `with_minimap` on the `text` area after the split, so it was not harder than giving none. The
    other pane keeps its full width. With the keyboard pane on the left, the column sits between
    the two panes. Under 320 px of text beside it (a pane in a narrow window) it hides.
  - **A run that crosses a colour change is split there**, not drawn in the colour of its first
    character (`fn main() {` is several colours in the editor).
  - **With word wrap** the band is the lines from the view's top line to the one at its bottom and
    the window's fraction is taken over those lines; without wrap it is exactly the editor's
    scroll fraction.
  - **Where the band is after a press outside it.** In a file longer than the column's rows the
    band does not land under the pointer (the column scrolls with the editor). The drag then moves
    the band by the pointer's movement from where it stands. Putting the band under the pointer
    instead would send the view elsewhere than the line that was clicked.
  - The first frame does not wait for the minimap's colours; a `--shot` does.
  - A line is read up to 512 bytes; wide characters count one column; the ink is cached as a
    bitmap per window, and typing draws the window again (no per-line patching).
- **Measured** (release, same quiet PC, runs alternated; budgets: exe 6,000,000 bytes, window
  shown 100 ms, memory 30 MiB, keystroke p50 8 ms, scroll step p50 4 ms):

  | | before | after, off | after, on |
  |---|---|---|---|
  | exe bytes | 5,651,968 | 5,666,304 | same exe |
  | shown, sample.rs (ms) | 74.8 to 75.3 (3 runs) | 74.6 to 76.3 (5 runs) | 77.1 to 82.4 (3 runs) |
  | private memory, sample.rs (MiB) | 27.8 | 27.5 to 27.7 | 27.8 to 28.0 |
  | keystroke p50, sample.rs (ms) | 0.51 to 0.52 | 0.47 | 0.57 to 0.59 |
  | scroll step p50, sample.rs (ms) | 0.44 to 0.45 | 0.40 | 0.43 |
  | keystroke p50, 50k.rs (ms) | 0.52 | 0.47 to 0.49 | 0.66 to 0.67 |
  | scroll step p50, 50k.rs (ms) | 0.46 | 0.43 | 0.61 to 0.62 |
  | private memory, 50k.rs (MiB) | 29.4 | 29.5 | 29.6 to 29.9 |
  | keystroke p50, 100mb.txt (ms) | 0.59 | 0.53 to 0.54 | 0.74 |
  | scroll step p50, 100mb.txt (ms) | 0.58 to 0.59 | 0.54 to 0.55 | 0.72 to 0.73 |
  | shown, 100mb.txt (ms) | 131 to 136 | 135 to 161 | 131 to 142 |

  The first build with the minimap on showed the window at 92 to 101 ms (sample.rs) and 110 to 118
  ms (50k.rs) because the first frame waited for the minimap's colours; that wait was removed (see
  above) and the table is of the final exe. Idle CPU with the minimap on: the CPU time at idle
  and at exit were equal on sample.rs and 100mb.txt (no timer was added); on 50k.rs the CPU time
  grows over the run as it does without it (the background parse). The 50k.rs file is already at
  29.4 MiB without the minimap (the 30 MiB budget was set on the small file); the minimap adds 0.1
  to 0.5 MiB (its bitmap is 96 x 700 px).
- **Tests.** `cargo test --release --workspace`: 400 passed, 0 failed (382 before): the arithmetic
  (marks, segments with tabs and cut-off, window and proportional scroll, pointer y to row, row to
  line with folds, drag and press) in `minimap.rs`, `Areas::with_minimap` in `ui.rs` (hide under
  320, split, results panel), the session and the argument, two for the worker. Clippy
  `-D warnings` and `fmt --check` clean. `tests/shots.sh` makes 69 shots (five new: `minimap`,
  `minimap-dark`, `minimap-long`, `minimap-fold`, `minimap-split`); the 64 earlier ones are
  byte-identical to a run made before the change except `menu-view` and `menu-main-view`, which
  list "Minimap"; two runs of the whole set were identical. I looked at the five new shots,
  `menu-view` and the real-window screenshots.
- **Run in a real window** (Windows, scale 1.0, launched from Git Bash with `MDEDIT_SETTINGS` on a
  scratch file and a copy of the 50,000-line file; mouse by `SetCursorPos` and `mouse_event`,
  screenshots of the window): the menu button, View, Minimap (turned on, `minimap=1` written);
  a press outside the band (the view went to the line at the pointer, 121 to 157 for a press on
  line 139); a drag after it (to line 13,000); a press inside the band and a drag of 40 px (the
  band moved 40 px); the wheel over the column (3 notches, 9 lines); a click in the text and
  typing 60 letters, a line break and 30 letters (the two long lines show in the column at once);
  Minimap off (full-width text, `minimap` gone from the file); on again; the window closed and
  started again on the same settings file (the column is there).
- **Not run:** scale other than 1.0 (the shots at 1.5 were not made for the minimap, only the
  unit arithmetic covers it); Linux; the split view in a real window (shot only), a drag with
  word wrap or folds in a real window (shots only for folds; wrap not even that); a window narrow
  enough to hide the column (unit test only); the results panel open with the column; a file that
  is not coloured at all in a real window (100mb.txt was only timed); the first frame with the
  column on a slow disk; the column on the 1 MB single-line file; a minimap request racing an
  edit in a real window beyond the typing above; idle CPU in a real window with the column on
  (only the `--trace` figures); typing with the split view open. **What is not built:** selection
  and find marks, bookmarks and the caret in the column, one row per wrapped row, the second pane's
  column, a width setting, a key for the command. (Of these, the marks, the second pane's column
  and the key were built later the same day: see "Minimap, finished" above.)

## Update look at every launch, and submenus on mouse-over (2026-11-13, 0.3.2, asked by the owner)

- **Update look.** The owner saw the launch look find nothing while the manual check found 0.3.1.
  Cause: `check_due` allowed the launch look once in 24 h, so an earlier look the same day
  suppressed it. Shared commit `1c3710a1ae2fed59585e37b2d4108c8d2802ac43` (branch
  `fix/update-look-every-launch` of `markdown-shared-rs`, on top of the old pin `fce8c8e`) makes it
  true at every launch with the setting on and removes `CHECK_EVERY`. All five `rev`s are moved to
  it. **That branch is pinned and the shared `main` was not moved.** Nothing in the editor used
  `CHECK_EVERY` and no test asserted "once a day"; only a doc comment in `session.rs` changed.
  Copies of 0.3.1 and older still look once a day, so they find 0.3.2 at launch only when their
  day is up, or by the manual check.
- **Submenus: a cascade.** `Popup` (`ui.rs`) now holds an optional `sub` popup and a `dwell`.
  Resting the pointer on "Open recent ›", "View ›" or "Macro ›" for 300 ms (`SUBMENU_DWELL`) opens
  the submenu beside the main menu, which stays; a click or Right opens it at once (Enter too, with
  the first item lit). The submenu sits level with the item when the pointer opened it, else at the
  menu's top, on the parent's left when there is room. Another item of the parent closes it (and
  starts that item's dwell); Escape or Left closes only the submenu; a click outside, the wheel or
  a right click close both; a click on a submenu item runs it and closes both. Up, Down and Enter
  act on the submenu while one is open. No other menu has a "›" item (checked: tab, text, find,
  line-ending and encoding menus), so none changed; the code is generic over `Act::submenu`. The
  dwell is a deadline in `Popup`, read by `timers()` like the tooltip's; with no menu open, or no
  item being rested on, there is no deadline and the loop waits. Built as a cascade because the
  popup code needed one extra field, not a rewrite (about 300 lines with tests).
- **Tests and shots.** 4 unit tests in `ui.rs` (`popup_tests`: dwell starts and keeps its
  deadline, a plain item or leaving cancels it, another item closes the submenu, the submenu keeps
  the parent lit, outside both changes nothing) place real menus headless. `--menu main-view` and
  `--menu main-macro` are new shot states; `tests/shots.sh` gains `menu-main-view` and
  `menu-main-macro-dark` (64 PNGs). I looked at those two and at `menu-main`.
- **Measured** (release): exe 5,651,968 bytes (budget 6,000,000); window shown 77 to 83 ms on
  quiet runs (95 to 109 ms in runs when the machine was busy; the published 0.3.1 exe showed the
  same spread in turns with it); private memory 27.3 to 27.8 MiB; keystroke p50 0.54 ms against
  0.50 ms for the 0.3.1 exe run in turns with it (0.47 was measured on a quieter day; budget 8).
  `cargo test --release --workspace` 382 passed, 0 failed; clippy `-D warnings` and `fmt --check`
  clean.
- **Run.** Update look: the release exe copied to `target\launchtest\` with `MDEDIT_SETTINGS` on a
  scratch file and `update_checked` one minute old: after the run the file said `update_checked` =
  the launch time (moved to now). The published 0.3.1 exe in the same setup left it at one minute
  old (the control). Hover, in a real window on Windows (Git Bash launch, cursor moved with
  `SetCursorPos`, screenshots of the window): the menu button clicked; after 100 ms on "View" no
  submenu; after 500 ms the View menu beside the main menu; the pointer into it kept View lit;
  to "Macro" the View menu closed and Macro opened; to "Go to line" it closed; a click on the text
  closed both menus.
- **Not run:** the keyboard paths (Right, Left, Escape, Enter into a submenu) in a real window
  (code only, no test); a click on an item inside a submenu in a real window; "Open recent ›" in
  a real window (the scratch settings had one recent entry, not hovered); scale 1.5 or 2; Linux;
  a submenu on a narrow window (it falls to the parent's right); idle CPU with a menu open was not
  measured (by reading the code the deadline is cleared when it fires); an update installed from
  the launch look. The owner has not seen any of it.

## More languages and an installer (2026-11-13, 0.3.1, asked by the owner)

- **Languages.** Looked up by file extension through `Language::for_path` before the change:
  coloured were Rust, C, C++, C#, Java, Go, Python, Ruby, PHP, JavaScript (also `.ts`, `.tsx`,
  `.jsx`, read with the JavaScript grammar), SQL, Bash, Batch, HTML, CSS, JSON, YAML, XML,
  Markdown, Makefile, Lua, Perl, R, Diff, Scala, Haskell, Erlang, Clojure, LaTeX, OCaml, Groovy
  and, from the hand-written scanners, TOML, Kotlin, Swift, PowerShell and Dockerfile. Not
  coloured: SCSS, Less, INI, `.env`, `.conf`, `.cfg`, CMake, Dart, Zig, Vue, Svelte, Elixir,
  Protobuf, Julia, Nim, Terraform, VB, F#, `.csv`, `.log`, `.txt` (the last three are plain text
  on purpose).
- **Added** in `markdown-shared-rs` `mdshared-highlight` `scan.rs`: TypeScript (`.ts`, `.tsx`,
  `.mts`, `.cts`; `.js` and `.jsx` stay on the JavaScript grammar), Dart, Zig, SCSS, Less, INI
  (`.ini`, `.cfg`, `.conf`, `.env` files, `.editorconfig`, `.gitconfig`) and CMake
  (`CMakeLists.txt`, `.cmake`). **They are hand-written line scanners, not `.sublime-syntax`
  files**: the existing extra languages (Kotlin, Swift, PowerShell, Dockerfile, TOML) are scanners
  too, a sublime syntax needs a second syntax dump or a YAML parse at start-up, and no licence
  text had to be taken from anyone. Nothing was downloaded. They are approximations: no
  regular-expression literals or JSX tags in TypeScript, no string interpolation colours except
  escapes, selectors in SCSS and Less are plain. Vue and Svelte were not added. `line_comment`
  covers them (`//` for TypeScript, Dart, Zig, SCSS, Less; `#` for INI and CMake). The viewer's
  fenced code blocks pick the new languages up when it moves its pin (`ts` fences change from the
  JavaScript grammar to the scanner).
- **Shared commit:** branch `feat/more-languages`, commit `fce8c8e71a9cbf0bab366bf7795f0e3860f9be3f`,
  pinned by the editor. **The shared `main` was not moved** (it is still at `870d620`); merging the
  branch is the owner's call. `cargo test --workspace --release` in the shared repository: the
  `mdshared-highlight` tests (31) and the rest pass; clippy with `-D warnings` is clean for
  `mdshared-highlight` but fails in `mdshared-app` (five warnings in test code, not touched here).
- **Installer.** `packaging\windows\mdedit.iss` is the viewer's `mdview.iss` adapted (same
  `[Setup]` choices: per-user, no elevation, `{localappdata}\Programs\mdedit`, no Start menu folder
  page, lzma2, modern wizard; same tasks wording and registry layout). The extension list is one
  line of the script. The wizard asks: where to install; Tasks page with "Register mdedit for text
  and source files" (checked), "Add Edit with mdedit to the context menu of all files" (off),
  "Create a desktop shortcut" (off); the last page offers Launch mdedit and, if associations were
  chosen, opening Settings at Default apps (off). **The viewer's installer lacks** the context-menu
  task and does not ask for whom to install (it is per-user only, so mdedit's does not either).
  `tools\release.ps1` builds it through `packaging\windows\build-installer.ps1` and uploads
  `mdedit-setup-<version>.exe` as the third asset; the manifest still names the bare exe.
- **Updater and an installed copy.** The shared updater (`mdshared-app` `selfupdate.rs`,
  `updater.rs`) does not look at how the app was installed: it renames the running exe aside to
  `.old` and moves the new one into the exe's own folder, and removes `.old` later. The per-user
  install folder is writable, so an installed mdedit updates in place; the uninstaller removes the
  files it installed by name, and the updated exe is the same name. An update leaves the installer's
  version number in "Apps" stale. Not tested: an update applied to an installed copy.
- **Measured** (release build, this PC): exe 5,650,432 bytes (budget 6,000,000; 5,637,632 before);
  installer 4,277,023 bytes; window shown 78.1 to 81.5 ms over five runs; private memory 27.5 to
  27.9 MiB; keystroke p50 0.47 ms. `cargo test --release --workspace` 378 passed, 0 failed;
  clippy `-D warnings` and `fmt --check` clean.
- **Run:** silent installs into a folder under `target` with `/VERYSILENT /CURRENTUSER` and
  `/TASKS=` of nothing, `assoc`, `assoc,contextmenu` and `contextmenu`: the keys appeared only with
  their task (the ProgId, `Software\mdedit\Capabilities` with 64 extensions, `Applications`,
  `RegisteredApplications`, `OpenWithProgids`; the `*\shell\mdedit` key with its command) and the
  uninstaller removed all of them and the folder. The installed exe answered `--check-update`
  with "nothing newer". A shot of a `.ts` file shows keywords, strings, comments and types in colour
  and "TypeScript" in the status strip.
- **Not run:** the installer's wizard clicked through by a person (only silent installs), "Open
  with" and Default apps as Windows draws them, a double click or the context-menu entry on a file,
  upgrading over an older install, the 62 shots (not re-rendered, so any shot of a `.ts`, `.ini`
  or `.conf` file may have changed), the new languages on Linux, an update applied to an installed
  copy, anything of the interface by the owner. The installer is not code-signed, so SmartScreen
  may warn. The unit tests colour a small sample per language; real files of each were not tried.

## Our own icons (2026-11-13, asked by the user: the same look on every system)

The user approved mockups (stroke 1.25 on a 16 px grid, round ends) and named no icon to
change. The shared `main` is at `870d620` with them; the user asked for them on main.

- How: `mdshared-ui` `glyph.rs` holds each of the 38 icons as SVG path data; `Gfx::icon`
  strokes it with `zeno` (the rasterizer `swash` already brings in) and keeps the mask per
  size; `Painter::icon` has the same signature as before, so neither app's code changes.
  Segoe Fluent Icons, the Fluent System Icons subset for Linux and `Face::Icon` are gone.
  Icons are grey antialiased now, where the font glyphs had ClearType colour fringes.
- A dot is a small circle whose stroke closes over its middle, and the dashed frame of
  Select all is drawn dash by dash, so there is one way of drawing and no fill.
- `cargo test -p mdshared-ui icon_sheet -- --ignored` in the shared repository writes
  `target/icons.png` and `icons-150.png`: every icon at 8.5 to 32 px in both themes.
- Measured after, on Windows: exe 5,637,632 bytes (142 KB more: the stroker and the path
  parser); window shown 76 to 82 ms; private memory 27.4 to 27.6 MiB; keystroke p50
  0.47 ms. 378 tests with `--release` (in a debug build
  `a_request_longer_than_one_slice_is_answered` runs out of its 60 s; whether it did before this change was not checked), clippy and fmt clean. All 62 shots differ from before, as they must.
- Looked at by me: the sheet, `menu-main`, `files-pane`. Not looked at: the other shots.
  The user has not seen the icons in a real window.
- Not done: the viewer still pins the commit before; it needs only its pin moved (the names
  and `Painter::icon` are unchanged) and new baselines for its shots.

## Split view (2026-11-13, the rest of roadmap item 10: the roadmap is done)

- Shift+F8 ("Two files side by side" in the View menu) shows two tabs beside each other:
  the active one, and the tab after it in a second pane at the right. F8, a click on the
  second pane, or a click on its tab gives it the keyboard: the panes change roles and each
  keeps its tab and its side. Any other tab chosen shows in the pane with the keyboard.
  Shift+F8 again, or closing the second pane's tab, ends the split.
- **It is two files, not two views of one file.** The user was told the difference on
  2026-11-13 and said: "leave as is, note it." So one file in both panes is not planned;
  do not build it unless asked. A
  document's caret, scroll, folds and colours belong to the document (`docs.rs` `Ready`);
  showing one document twice needs them taken out of it into a view of their own, which
  was not done. The same file cannot be shown in both panes.
- The pane without the keyboard has no caret and no line band. The wheel scrolls it (by
  lines; not sideways). Its scrollbars show but cannot be dragged, and a first click on it
  only gives it the keyboard: it does not place the caret.
- The halves are equal and cannot be resized; under 520 px of text width one pane shows.
  Nothing in the tab row marks the second pane's tab. The split is not in the session.
- How: `Docs::other` is the second pane's tab and `Docs::activate` swaps the roles when
  that tab is activated. The pane is a seventh region (`Region::Other`,
  `Areas::with_split`), drawn by the same `View::draw` with its hits replaced by one
  `Hit::Other`. Its colours are asked with the new `HighlightService::peek`, which the
  worker answers without making that document the one it parses in the background.
- Run in a real window on Windows by keys: Shift+F8, a letter and Ctrl+S, F8, a letter and
  Ctrl+S, F8, a letter, Shift+F8, F8 (the toast), a letter; and a click on the second pane
  followed by a letter. Each letter was in the right file. Seen in shots only (`split`,
  `split-dark`): the look, word wrap in a pane, colours in both. Not run: the wheel over
  the second pane, closing its tab, a split with the Files pane or the results panel open,
  a narrow window, two large files.
- Measured after: exe 5,495,808 bytes; window shown 77 to 79 ms (six runs; two of three
  runs right after the build were 85 and 90); private memory 27.7 MiB; keystroke p50
  0.48 ms; scroll step p50 0.55 ms. Typing with the split open was not timed (`--split`
  works for `--shot` only). 378 tests, clippy and fmt clean; the shots are byte-identical
  to the build before but `menu-view` and `dialog-shortcuts`; 62 shots.

## Linux at `4280e9d` (2026-11-13, last run of the day)

The same agent, same VM, the editor at `4280e9d` with its own pin of the shared branch.

- Builds (one warning, `first_line`); 377 tests pass (378 on Windows); `tests/shots.sh`
  gives 62 PNGs. Looked at: `split`, `split-dark`, `menu-macro`, `fold`: icons, both panes
  and colours are right. The emoji of `sample.md` draws as a box there (no colour emoji
  font on the VM, most likely; not looked into).
- In the real window, by file bytes: a macro recorded and played twice; a macro played to
  the end of the file from the menu (six lines, each changed once, none added); split view
  by Shift+F8 and F8 (each letter in the right file); a click on the second pane then a
  letter (in that pane's file, at the caret that pane had); copy read back with xclip. By
  screenshot: "Recording" in the status strip, the wheel scrolling the second pane.
- Window shown at 52 to 56 ms, keystroke p50 0.73 ms. The binary is 10,920,416 bytes
  (10,025,128 at `f0c75a6`, 9,572,392 at `4023958`); why it grew by 0.9 MB in a day when
  the Windows exe shrank was not looked into.
- Not done: Wayland; the other 58 shots were not looked at.

## Linux in a real window, and room in the exe (2026-11-13, later)

A Sonnet agent drove the editor on the VM (`dietpi@192.168.1.113`, Debian 13, LXDE and
Openbox on X display `:0`) with xdotool, checking saved files' bytes. Its scripts and work
files are in `~/mdedit-test/` there; it installed xdotool, xclip, wmctrl, scrot and zenity.

- Worked, by file bytes or screenshot: typing, Enter, Backspace, Delete, undo, redo, save;
  find, replace, replace all, go to line; a second launch handing its file to the first
  window (not with `--no-session`, which also turns the hand-over off); Ctrl+Tab, Ctrl+N, the
  Save changes question; reload when the file changes on disk; click, double click, wheel,
  the right-click menu, the main menu, the theme button; word wrap, the function list, find
  in files; move, resize and maximise through the hand-drawn title row (Openbox adds no
  title bar of its own), minimise and close; the folding keys. Idle CPU over 20 s: one tick
  focused, none unfocused. Window shown at 68 ms, keystroke p50 0.71 ms, the 100 MB file
  read in 141 ms. `--trace` prints no memory or CPU figures on Linux.
- Found and fixed in the shared repository, branch `feat/linux-desktop`, commit `1e244ec`
  (on top of `b74e8cb`; everything in it is under `cfg(not(windows))`):
  - Copy, cut and paste did nothing: the clipboard off Windows was a stub. It now uses
    `arboard`, with one clipboard kept for the life of the process. Checked on X11 with
    xclip both ways and between two editor windows. Not checked on Wayland.
  - Every icon was a box. Off Windows the icons now come from a 7 KB subset of Microsoft's
    Fluent UI System Icons (MIT, the licence is beside the font) built into the binary, with
    a second code point for each name in `glyph.rs`. They are a little lighter and smaller
    than Segoe's. Seen in pictures and sensible: those of the bars, menus, panes and
    dialogs. Not seen in any picture: ZOOM, DELETE, LINK, POWER, SELECT_ALL, FULLSCREEN,
    BACK_TO_WINDOW, IMAGE, COPY, CUT, PASTE, OPEN_WITH, MOON, WARN.
- The editor pins that commit (`1e244ec3ed5b62d9e38e2a894d4f3d242d11b6c8`). It is the
  shared `main` since the user said to push it (2026-11-13; a fast-forward from `b74e8cb`).
  The viewer's pin was not touched.
- File dialogs: nothing appeared on the VM, because `rfd` there needs either the desktop
  portal or zenity and the VM had neither. With zenity installed Ctrl+O opens its file
  chooser; choosing a file and Save as were not tried. The editor says nothing when no
  dialog can be shown.
- Not fixed: a double click on the title row does not maximise on Linux; `first_line` in
  `platform.rs` is unused there (one warning).
- With the newer shared code the Windows exe was 6,008,832 bytes. `Cargo.toml` now builds
  `mdshared-highlight` (the glue and the scanners, not syntect) and **the shell crate
  `mdedit` itself** for size: with fat LTO the shell is where every crate's generic code
  is made, and the exe is 5,490,688 bytes. Measured after, on Windows: window shown 75 to
  77 ms; first colours 51 to 52 ms (50 before); keystroke p50 0.46 to 0.50 ms, 0.58 ms on
  the 100 MB file, 0.66 ms in Markdown with wrap; scroll step p50 0.53 ms; the 100 MB file
  read in 101 to 103 ms (96 to 98 was measured earlier in the day, not side by side). All
  60 shots are byte-identical to the build before; 377 tests pass.

## Macros (2026-11-13, the first half of roadmap item 10)

`bins/mdedit/src/macros.rs`; `App::play_macro` in `app/commands.rs`. Written in the main
session.

- Ctrl+Shift+R starts recording and stops it; Ctrl+Shift+P plays the macro. "Play it to
  the end of the file" has no key. The three are in a new "Macro" entry of the main menu,
  which replaces it in place as "View" does. While recording, the status strip says
  "Recording".
- A macro is the keys pressed while recording (key, modifiers, the text typed), given again
  to `key_pressed`. So it holds typing, caret moves, the editing commands and what was typed
  in the find box. Not in it: the mouse, what was chosen from a menu, and the commands that
  open a dialog or change tabs, files or the look (`macros::recordable`). At most 10,000
  keys.
- "To the end of the file" plays again for as long as a run leaves the caret on a later
  line than the run before, and stops on the empty line after the last line break; at most
  100,000 runs. A macro that does not end one line further down runs once. It runs on the UI
  thread and cannot be stopped; the window waits.
- A question or menu that a key of the macro opens ends the playback.
- There is one macro, kept in memory: not saved, not named, gone when the editor closes.
  Each key is its own undo step, as when typed; one Undo does not take a whole run back.
- A search in a file over the size searched at once (`SYNC_FIND_BYTES`) runs on a worker, so
  a macro that searches there would go on before the search is done. Not tried.
- Decision made without the user: "Manage recent files…" left the main menu to make room
  (it is still in "Open recent"), since the menu was as tall as an 800 px window.
- Run in a real window on Windows: Ctrl+Shift+R, Home, "- ", End, ";", Down, Ctrl+Shift+R,
  Ctrl+Shift+P twice, Ctrl+S (three lines changed); and a macro played to the end of the
  file from the menu (a click on the menu button, then arrow keys and Enter), on a file
  with and one without a last line break: every line changed once. Not run: recording over
  the cap, a macro with a search, playback on a large file, the "Recording" field (no shot
  has it).
- Measured after: exe 5,984,256 bytes; window shown 77 to 78 ms; private memory 27.5 to
  28.0 MiB; keystroke p50 0.49 ms. 377 tests, clippy and fmt clean; the shots are
  byte-identical to the build before but `menu-main` and `dialog-shortcuts`; `menu-macro`
  is new (60 shots).

## Code folding (2026-11-13, the rest of roadmap item 9)

`docs/DESIGN.md` section 27. Written in the main session on the core that was already there.

- F9 folds the block the caret's line heads, or else the nearest block around it (the caret
  then goes to the end of that block's first line); on a folded block's first line it opens
  it. Shift+F9 folds every block that starts at the left margin; Ctrl+F9 opens all. The three
  are in the View menu. A block is the lines below a line that are indented more than it, so
  a closing brace stays in view under the folded line.
- A folded line has a grey box with three dots after its text and an arrowhead between its
  number and the text. Neither can be clicked: folding is by key and menu only.
- Up, Down, the page keys, the wheel, the scrollbar and a click go by the lines that show.
  A caret that comes into a fold opens it (Right at the end of a folded line, Ctrl+End, a
  find, go to line, a bookmark, a result of find in files). Several carets move by lines of
  the document, so they open the folds they enter.
- Folds follow edits. An edit that adds or removes lines inside a fold or in its first line
  opens it, and so does a line break typed at the very start of its first line. Undo does not
  fold again. A reload opens them all; they are not in the session.
- Colours are asked for the lines from the view's top to its bottom with the hidden ones
  between, at most 10,000 (`view::shown_lines`): under a larger fold the text is plain until
  the fold is scrolled out at the top.
- The highlight worker fault noted before is fixed: a request that could not be finished in
  one 8 ms slice was started again for ever. Each try now gets twice the time of the one
  before, up to 2 s (`mdedit-highlight`, test `a_request_longer_than_one_slice_is_answered`).
- With folds the caret's blink and moves repaint the whole text region, as with word wrap.
- Run in a real window by keys on Windows: go to line 18, F9, a letter, Down, a letter,
  Shift+F9, Ctrl+End, a letter, Ctrl+F9, Ctrl+S; the saved file had the letters at the end of
  line 17, of line 24 and of the file. Seen in shots only (`fold`, `fold-dark`): the marks,
  folds with word wrap. Not run in a window: the wheel, the scrollbar and clicks with folds,
  the menu's entries, Fold all on a large file (the core looks at the first 200,000 lines).
- The exe went to 6,005,248 bytes with it, so ten more crates are built for size
  (`icu_locale_core`, `icu_provider`, `icu_locale_fallback`, `litemap`, `yoke`, `writeable`,
  `tinystr`, `potential_utf`, `utf8_iter`, `zeno`): 5,978,624 bytes.
- Measured after: window shown 73 to 77 ms (four runs); private memory 27.3 to 28.3 MiB;
  keystroke p50 0.47 ms, the same with two folds on screen. Scroll steps were not measured
  again. 376 tests, clippy and fmt clean; of the 57 earlier shots all but `menu-view`,
  `dialog-shortcuts` and `dialog-about` are byte-identical (the first two list the new
  commands; why the About one differs from the morning's render was not looked into).

## Function list, and v0.2.0 (2026-11-13, half of roadmap item 9)

- Ctrl+Shift+2 (and "Function list" in the View menu) shows the active document's headings,
  functions and types in the left pane, in the Files tree's place; Ctrl+Shift+1 or the folder
  button in its title row brings the tree back, and each command hides the pane when its own
  list is the one showing. A click on a row goes to its line; the row of the caret's place is
  marked and scrolled into view. `bins/mdedit/src/outline.rs`.
- The list comes from the new crate `mdedit-outline` (written by a Sonnet agent to a given
  interface): line-wise rules for Markdown, Rust, Python, the C family, shell, TOML/INI, Ruby,
  Lua and CSS, no parser, so it misses things (parameter lists over several lines, methods of
  a JavaScript class) and other languages show "No list for this kind of file". It is a crate
  of its own so that it is built for size. A worker thread is given the text after every
  change while the list shows and gives up a list it is making when a newer text comes.
- The exe went to 6,050,816 bytes with it. Most of that was not the rules but
  `std::sync::mpsc`: each type sent on a channel costs about 15 KB. The outline and results
  workers and the load and find results of `Docs` now use a list or a slot under a `Mutex`
  (with a `Condvar` where something waits) and the exe is 5,984,768 bytes.
- Measured after: window shown 77 to 79 ms; keystroke p50 0.47 ms, and 0.56 ms while the
  function list shows; scroll step p50 0.40 ms; the 100 MB file loads in 98 ms.
- Not run in a real window: the function list at all (two shots only: `outline`,
  `outline-dark`), so not its clicks, its wheel, its updates while typing or the pane switch.
  The load and find results were run by the tests and the shots only since the change from
  channels.
- Folding came after this release: see "Code folding" above.
- **v0.2.0** was published from this state so that the updater can be tried from 0.1.0
  (`docs/RELEASE.md`).

## Find in files (2026-11-13, roadmap item 8)

- Ctrl+Shift+F looks for the find box's text, with its options, in every file of the Files
  pane's folder, or of the active file's folder when no folder is open. "Find in open tabs"
  (no key) looks in the tabs' text as it is now. Both are in the find box's right-click menu.
  What is found shows in a panel under the text (`bins/mdedit/src/results.rs`): a row for each
  file, a row for each match with its line number and the line, the match marked. A click on a
  match, or F4 and Shift+F4, opens the file and selects the match. The panel's close button
  (and the command "Close the results", no key) closes it and stops a search still running.
- The core is `mdedit_core::search` (`in_buffer`, `in_tree`), written by a Sonnet agent to a
  given interface and reviewed here. A folder is walked on a worker thread: files first, then
  subfolders, by name; `.git`, `node_modules`, `target` and names starting with a dot are
  skipped, as are files over 16 MiB, files with NUL bytes and files that cannot be read (the
  title row says how many). It stops at 20,000 matches, 2,000 of one file. Files are read with
  the editor's own loader, so encodings are detected the same way.
- A folder search reads files from disk: a tab with unsaved changes is searched as it was
  saved, and the match is then selected at the same offsets in the changed text, clamped to
  it, so it can be off. "Find in open tabs" skips tabs not read yet and tabs over 64 MiB and
  says "more were left out"; it runs while the window waits.
- Not built: replace in files, a way to choose the folder or file kinds, keyboard focus in
  the panel (arrows, Enter), a resizable panel (it is 230 px, less in a low window, none
  below about 450 px of window height), Escape to close it, results that follow edits.
- Run in a real window by keys: "ring" typed in the find box, Ctrl+Shift+F with
  `fixtures/sample.rs` open (34 matches in 2 files, 1 file skipped), F4 three times (the
  other file opened in a tab with the match selected), a click on a row, Shift+F4. Not run in
  a window: "Find in open tabs" (two shots only: `results`, `results-dark`), the menu entries,
  the wheel and the scrollbar of the panel, closing it, a search of a large folder, cancelling.
- The exe went over the budget with this (6,041,600 bytes), so fifteen more crates are built
  with `opt-level = "s"`: `icu_segmenter`, `icu_normalizer`, `icu_properties`, `softbuffer`,
  `memmap2`, `icu_collections`, `zerovec`, `zerotrie`, `tracing`, `tracing-core`, `indexmap`,
  `linked-hash-map`, `smol_str`, `miniz_oxide` and `aho-corasick`. Tried and
  taken back: `read-fonts` and `skrifa` save 55 KB but the window shows about 5 ms later;
  `bit-set`, `bit-vec`, `crc32fast` and `base64` save 2.5 KB and seemed to cost 2 ms.
- Measured after: exe 5,979,648 bytes; keystroke p50 0.48 ms; scroll step p50 0.40 ms;
  private memory 27.8 MiB; the 100 MB file loads in 96 ms. Window shown: 79 to 81 ms, and
  the same for the build before this feature run in turns with it (the machine was about
  2 ms slower than when 77 to 78 ms was measured). Find speed with `aho-corasick` built for
  size was not measured apart from the core's tests passing.

## Bookmarks (2026-11-13, roadmap item 7)

- Ctrl+F2 sets or removes a bookmark on the caret's line (also "Toggle bookmark" in the
  text's right-click menu); F2 and Shift+F2 go to the next and the previous one, going round
  at the ends; "Clear bookmarks" has no key and is in no menu (the shortcuts dialog lists it,
  nothing runs it yet). A bookmarked line has a dot left of its number.
- Bookmarks follow edits (`lines::shift_marks` in the core, called from `Ready::changed`,
  so Undo and Redo move them too) and a reload. A bookmark on a line that is joined into
  another goes to the joined line; pressing Enter at the start of a bookmarked line leaves the
  bookmark on the empty line above the text, not on the text.
- They are per tab and are not kept: closing the tab or the editor loses them.
- Not built: a click in the gutter to toggle one, cut/copy/delete of bookmarked lines.
- Run in a real window by keys: two bookmarks set, a line typed above them, F2 twice and
  Shift+F2 with a letter typed at each stop; the saved file had the letters on the right
  lines. The dot was seen in a shot only (`bookmarks`); the menu entry was not clicked.
- Measured after: exe 5,975,040 bytes; window shown 77 to 78 ms; keystroke p50 0.48 ms.
  The text menu is now 590 px tall: it fits an 800 px window with nothing to spare.

## Multiple carets and column selection (2026-11-13, roadmap item 6)

`docs/DESIGN.md` section 25. A Sonnet subagent wrote the core module (`multi.rs`, 17 tests and
a timing test); the shell part was written in the main session.

- Ctrl and a click adds a caret. Alt and a drag selects a column; Alt+Shift and an arrow
  makes or extends one from the keyboard. Escape, a plain click, the page keys or any command
  that is not listed below goes back to one caret.
- At every caret: typed text, Enter (a plain line break, no indent), Tab (a tab character),
  Backspace, Delete, paste, cut, and the caret keys with or without Shift. Each edit is one
  undo step, typed characters included (they do not merge into one step as with one caret),
  and Undo leaves one caret.
- Copy puts the selections one to a line, in text order. Paste gives one line to each caret
  when the clipboard has as many lines as there are carets, else the whole text to each.
- A column selection has no virtual space: on a line shorter than the column the selection
  ends at the line's end, and a drag cannot go past the text of the line under the pointer.
  Columns are display columns (a tab counts to its stop).
- With word wrap on, Up and Down move several carets by lines, not rows.
- With several carets the status strip says how many, the current-line band and the bracket
  boxes are off, and every caret move repaints the whole text region.
- Not built: Ctrl+D style "select next occurrence", Ctrl+click on a caret to remove it, Ctrl+
  Backspace and Ctrl+Delete at every caret (they go back to one caret), line commands at
  every caret.
- Run in a real window: Alt+Shift+Down three times, typing, Right, Backspace, Shift+End,
  Ctrl+C, Escape, Ctrl+End, Ctrl+V (four lines came out), a click and two Ctrl+clicks, typing,
  Ctrl+Z (one step), Ctrl+S; the saved file was checked. That run found that Copy kept only
  the last selection; fixed. Not run in a window: Alt and a drag (the script cannot drag),
  a column over tabs or wide characters, many thousands of carets (core timing only: 100,000
  carets, an insert 75 ms, its undo 68 ms), carets with word wrap on. No shot shows several
  carets: `--shot` has no switch for them.
- Measured after: exe 5,967,872 bytes (32,128 under the budget); window shown 78 to 80 ms;
  private memory 27.3 to 28.1 MiB; keystroke p50 0.48 ms; scroll step p50 0.54 ms; the 52
  shots are byte-identical to the build before.

## Room in the exe (2026-11-13)

`Cargo.toml` also builds `fontique`, `bincode`, `flate2`, `yazi` and `mdshared-ui` with
`opt-level = "s"`. The exe went from 5,987,328 to 5,948,416 bytes (51,584 under the budget).
All 52 shots are byte-identical to the build before. Measured after, three runs each: window
shown 77 to 80 ms (78 to 79 before); keystroke p50 0.48 ms (0.79 before); scroll step p50
0.54 ms on the 100 MB file (0.84 before); with wrap on in long paragraphs 1.20 ms (1.58
before). Why painting got faster was not looked into, and which of the five crates did it
was not separated. Tried and not kept: `encoding_rs` for size saves another 50 KB but reads a
100 MB Windows-1254 file in 551 ms instead of 508; `url`, `idna` and `fdeflate` save nothing.

## Word wrap (2026-11-13, the last of roadmap item 4)

View menu, "Word wrap", or Alt+Z; off by default, kept in the session (`wrap=1`), `--wrap`
for shots. Roadmap item 4 is done with it.

- A line longer than the view goes on in rows below: a row ends after the last space or tab
  that fits, a word longer than a row is cut between characters. The line number stands
  beside the first row only. There is no sideways scrolling.
- Rows are found from column counts (`display::wrap_rows` in the core), with the same
  estimate of widths the view already used for long lines, not from shaped text. Glyphs from
  fallback fonts that are wider than the estimate can run past the right edge and be cut
  there. 16 px are kept free at the right for the caret and the scrollbar.
- Up, Down, Page Up and Page Down go by rows and keep the column in the row. Home and End
  still go to the line's start and end, not the row's. A click and a drag find the row under
  the pointer.
- The scroll position is a line and a row in it. The vertical scrollbar's thumb shows the
  top line's number, not rows: it moves unevenly through a file whose lines differ much in
  length, a document that is one long line has no scrollbar (wheel and keys scroll it), and
  the last line can be scrolled up to the view's top.
- The caret's blink and moves repaint the whole text region with wrap on (without it, only
  the lines involved).
- A document read with a line longer than 262,144 characters is shown unwrapped though wrap
  is on (`WRAP_LINE_MAX`); nothing says so on screen. The figure is the one from reading the
  file: a longer line pasted later is wrapped, at about 20 ms a keystroke for each 1 MB of
  that line.
- Run in a real window by keys and one click on a file of long paragraphs: Alt+Z, Page Down,
  Ctrl+End, Up three times and typing, a click in the middle of a paragraph and typing,
  Ctrl+Home, Down, End. Not run in a window: the scrollbar's thumb with wrap, a selection
  drag that scrolls, resizing the window with wrap on, scale 1.5 and 2, tabs and wide
  characters at a row's end (unit tests only), the View menu's entry (the key was used).
- Measured after: exe 5,987,328 bytes (12,672 under the budget: the next feature needs
  room found for it); window shown 78 to 79 ms; private memory 27.6 to 27.8 MiB; keystroke
  p50 0.79 ms with wrap off (unchanged), 0.79 ms with wrap on in `sample.rs`, 1.58 ms with
  wrap on in paragraphs of up to 3,700 characters (1.42 ms there with wrap off); scroll step
  p50 0.83 ms on the 100 MB file with wrap on (0.84 off). Shots: `wrap`, `wrap-dark`.

## Encoding and line endings from the status strip (2026-11-13, roadmap item 5)

The last two fields of the status strip are buttons now. Roadmap item 5 is done.

- Line endings: the field opens a menu standing on it (Windows (CRLF), Unix (LF), Macintosh
  (CR), the document's own checked). Choosing one rewrites every line break as one undo step
  (`lines::convert_line_endings`), so the tab shows as modified and Undo brings the old breaks
  back. Refused with the same toast as Trim and Tabs to spaces above 8 MiB
  (`ALL_LINES_MAX_BYTES`); the core function took about 5.4 s on 100 MB, 3.3 million lines.
- Encoding: the menu has "Save in" UTF-8, UTF-8 with BOM, UTF-16 LE, UTF-16 BE and the system's
  code page (the current one checked), then "Reopen as" the same five, then "Reopen, detecting
  the encoding".
  - Save in changes nothing on disk: it sets the encoding the next Save writes and marks the
    tab modified (`Doc::encoding_changed`; it is not an undo step). When the text has a
    character the encoding cannot hold, it is refused with a toast naming the character and
    its line.
  - Reopen as reads the file again as that encoding whatever its bytes suggest
    (`LoadOptions::forced`, kept in `Doc::forced` so every later reload of the tab uses it,
    until "Reopen, detecting the encoding"). With unsaved changes the Discard question comes
    first (`After::ReadAs`); the choice is applied only once it is answered. The choice is
    not kept in the session: after a restart the file is detected again.
  - A file read with replacement characters (lossy, read-only) can still be reopened as
    another encoding; its Save in entries and its line-ending field are off.
  - Only those five encodings are offered. The other code pages the detector can choose
    (`Encoding::Legacy` of any `encoding_rs` page) cannot be picked by hand yet.
- Run in a real window by clicks: CRLF to LF, Save, then Save in UTF-16 LE, Save; the file's
  bytes were checked (FF FE, LF breaks). Not run in a window: Reopen as (covered by a `docs.rs`
  test and the core tests), the Discard question before a reopen, the refusal toast, a file near
  8 MiB, the menus at scale 1.5 or 2.
- The shared `Menu::above` is additive (a menu made without it is placed as before). The
  viewer's check was not run for it and the viewer's `rev` was not moved: its checkout belongs
  to another session.
- Measured after: exe 5,965,824 bytes; window shown 78 to 86 ms; private memory 27.6 to
  27.8 MiB; keystroke p50 0.78 ms. Shots: `menu-eol`, `menu-encoding`.

## View aids, first part (2026-11-13, roadmap item 4)

Zoom, go to line, the matching bracket and the other occurrences of the selected word are in;
the current line was already banded, and Show whitespace is in. Word wrap has its own section
above.

- Show whitespace: a dot in each space, a dash with a tick in each tab, and at each line's end
  a down arrow for LF, a return arrow for CRLF, a left arrow for CR (drawn with the text font
  and its fallbacks). Off by default, kept in the session (`whitespace=1`), `--whitespace` for
  shots. Other blank characters (no-break space and the like) get no mark.
- The main menu has a "View" entry that replaces it in place, as "Open recent" does: Files
  pane, Show whitespace, Zoom in, Zoom out, Reset zoom. "Files pane" left the main menu for it.
  Clicked through in a real window (the menu button, View, Show whitespace); the marks
  themselves were seen in shots.

- Matching bracket: with nothing selected, the bracket after the caret (else the one before
  it) and its partner get a grey box. `()`, `[]`, `{}` by nesting of the same kind, looked for
  within 64 KiB each way, strings and comments not told apart. There is no "go to matching
  bracket" command.
- Selected word: when the selection is one whole word (letters, digits, `_`, up to 128
  bytes), its other whole-word occurrences on the visible lines are tinted, case sensitive,
  on lines up to 8192 bytes. Neither aid has a setting to turn it off.
- Both were seen in shots only (`bracket`, `twins-dark`), not in a real window.

- Zoom: 50 to 400 % in steps of 10, kept in the session (`zoom=`), shown in the status strip
  when it is not 100. Ctrl and the wheel, Ctrl+- and Ctrl+0 were run in a real window. Zoom in
  by key is the shared "Ctrl++" (Ctrl+= on a US layout), which no key gives on a Turkish Q
  layout; that binding was not run. `--zoom N` for shots.
- Go to line: Ctrl+G types in the find box's place ("12", "12:5" or "12,5"); Enter goes there
  and gives the keyboard back to the text, Escape leaves. Run in a real window by keys.
- The main menu is as tall as an 800 px window now, so the zoom commands are not in it; the
  next menu entry needs a submenu or a shorter menu.
- `Cargo.toml` also builds `mdshared-app`, `winit` and `chardetng` for size: the exe is
  5,933,056 bytes with these features, window shown 78 to 82 ms, keystroke p50 0.77 ms.

## Unsaved text survives a close (2026-11-13, roadmap item 3)

`docs/DESIGN.md` section 19. Written in the main session.

- Decisions made without the user: closing the window (or Exit) no longer asks about unsaved
  changes when there is a session; the text is kept in `backup\` beside `settings.ini` and put
  back at the next start, the tabs marked modified. Closing a tab still asks. With
  `--no-session` closing asks as before. The backups are written only at close: a crash or a
  power cut loses unsaved text as it did before.
- A file tab gets its unsaved text back as one edit over the file as it is on disk now, so
  Undo gives the file's text. If the file changed on disk between the two runs nothing says
  so, and saving overwrites it.
- Run in a real window by script with `MDEDIT_SETTINGS` on a scratch file: typed into a file
  and a new document, Alt+F4 (no question, two backups, the file untouched); started again
  (both tabs back and modified), Ctrl+S on the file's tab, Alt+F4 (the file had the text, one
  backup left); started again, Ctrl+Z in the new document, Alt+F4 (no backup left).
- Measured after: exe 5,987,328 bytes (12 KB under the budget: the next feature will need
  room found for it), window shown 79 ms, private memory 27.6 to 27.8 MiB, 42 shots unchanged,
  268 tests, clippy and fmt clean.
- Not run: a large unsaved file (the text is written and read on the UI thread); a backup that
  cannot be written (the code then asks as before); a file deleted between the runs (the code
  restores it as a new document); closing before a restored tab was read (unit test only).

## Find and replace (2026-11-13, roadmap item 2)

`docs/DESIGN.md` section 18 describes it. A Sonnet subagent wrote the core part (whole word,
regular expressions, `replace.rs`); the shell part was written in the main session.

- Decisions made without the user: the strip sits under the command bar and opens with
  Ctrl+H; Replace all is Ctrl+Shift+Enter (Ctrl+Alt is AltGr); Replace all refuses files over
  24 MiB because it runs on the UI thread (1.4 to 2.2 s per 100 MB with dense matches); the
  option toggles are text ("Aa", "W", ".*"), not icons; the options are not kept in the
  session.
- The exe went to 6,061,056 bytes with the regex search. `Cargo.toml` now builds
  `regex-syntax`, `png`, `mdshared-fetch` and some cold crates with `opt-level = "s"`, which
  gives 5,977,088. `regex-syntax` at that level also parses syntect's patterns: the first
  colours of `fixtures/sample.rs` came at 55 to 57 ms, which was not compared with a build
  without it.
- Measured after: window shown 79 to 84 ms, private memory 27.8 to 28.0 MiB, keystroke p50
  0.78 ms, 42 shots identical across two runs, 266 tests, clippy and fmt clean.
- Run in a real window by script (keys only): Ctrl+H, a query, Tab, a replacement, Enter twice,
  Ctrl+Shift+Enter, Escape twice, Ctrl+Z, saving after each step; the saved files had 2, then
  29, then 2 replacements, so Replace all was one undo step.
- Not run: no click on the strip's buttons or toggles, nor on the menu's options; a regular
  expression was only used in tests and shots, not typed in a window; Replace all on a file
  near 24 MiB; the user has not seen the strip.
- Not built: "find in all open tabs" waits for the results panel of find in files (item 8);
  replace within the selection (the core takes `within`, the shell always passes the whole
  document); Undo with a box of the bar focused still does nothing.

## Next release work (branch next)

### Rounded window corners (base `8019a35`)

Built in `mdshared_ui::corners` and `shadow` (shared branch `next`) and wired here; DESIGN section
57 is the specification.

- Built: the "Window corners" setting (Square, Small, Round; default Round), kept as `corners=` in
  the session; Windows: the DWM preference, applied at creation and on a change; Wayland client
  margin: the anti-aliased corner cut in `present` (only the touched corner squares), the shadow
  under the cut; `--frame` shots; tests for the setting, the args, the mask and painter (shared)
  and the frame picture.
- Decisions: radii 0, 4, 8 only (they are exact on Windows too: do not round, small, round; a 12
  would have been drawn as 8 there); on KDE the default (Round) now uses the client margin instead
  of KWin's protocol, because KWin's route is opaque (the plan's own note); Square keeps KWin's
  route. A change from Square to Round on KDE applies at the next start (the help line says so).
  The row is hidden where nothing can round (X11 without the margin). The shadow tiles were not
  re-rendered for the rounded shape: the difference is about one alpha step (computed, not seen).
  The Windows 11 preference used to be set to Round unconditionally in `style_window`; that call
  is now the setting's.
- NOT verified (no local builds by the owner's rule; CI only): any real window on any desktop.
  Not seen: the corner in a real Wayland window, on KDE with the route change, the shadow under the
  corner, a tiled or maximized window, switching the setting at run time, Windows 11 DWM rounding
  or Windows 10 staying square, fractional scales other than in the unit tests. The startup and
  keystroke budgets were not measured: the change adds a `Shadow::rounds()` check to `present`
  and nothing else on the keystroke and scroll path; with Square or without a margin nothing runs.
  The KDE route change does move the default KDE window from KWin's shadow to the client-drawn one
  (bigger buffer by the 36 px margin each side), which was not timed.
