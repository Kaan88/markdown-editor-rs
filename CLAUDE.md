# mdedit

A light text editor in Rust: a Notepad alternative that opens any text file, with syntax
highlighting. The companion of the Markdown viewer `mdview`. winit + softbuffer, every control
drawn by hand, built on crates it shares with the viewer. Windows first; Linux and macOS are
built and released as well.

On GitHub it is `Kaan88/markdown-editor-rs`, whatever this folder is called on disk. Its sisters
are `Kaan88/markdown-viewer-rs` (mdview; on this PC `..\mv`) and `Kaan88/markdown-shared-rs` (the
shared crates; `..\markdown-shared-rs`).

## Read in this order

1. This file. It is short on purpose and it is kept true.
2. `docs/STATE.md`: what is released, what is on `main` and not released, what is being built,
   what is next, what is known to be wrong. Trust it over anything older.
3. `docs/DECISIONS.md`: what has been decided, when, why, and what would change it. Look there
   before you decide anything. A decision is never re-made silently: add an entry that names the
   one it replaces.
4. Then what the task needs:
   - `docs/PLAN.md`: what the app is, what it is not, the budgets.
   - `docs/CODE.md`: where the code is, how the shared crates are pinned, how to build, test,
     take shots and measure.
   - `docs/DESIGN.md`: how each part is built, one section per part. Read its first screen.
   - `docs/CI.md` and `docs/RELEASE.md`: the workflows, and how a release is cut.
   - `docs/ai-library-plan.md`: the plan the library and AI work is built from.
   - `docs/ecosystem-ideas.md`: the apps that may join these three; the plans written for some of
     them are in `docs/ideas/` (its `README.md` is the index).

`docs/archive/` is history. Nothing in it is current. Do not start there, and do not act on it.

## What it is, and what it refuses to be

- Fast to open and light, before anything else. The owner's rule (2026-11-12): "as long as the
  app remains just as fast to open and work, implement useful features from Notepad++."
- Not an IDE: no language servers, no build or debug, no plugins, no terminal. Not a WYSIWYG
  Markdown editor: Markdown is edited as text, and visual editing edits one block at a time, as
  its text.
- No network use, except its own updates, and the AI when the owner turns it on. The AI is off
  by default; only what a command is run on leaves the machine, only to the provider chosen; and
  the request is made by the separate `mdedit-ai` engine, never by the editor.
- Feature complete since 2026-11-17. A new feature goes in only when the owner asks for it.

## The rules, each one learned the hard way

1. **The budgets are measured, before and after.** First frame under 100 ms, private memory
   under 30 MiB, a keystroke under 8 ms, a scroll frame under 4 ms, no CPU at idle, and nothing
   new on the launch path. The figures and the method are in `docs/PLAN.md` and `docs/CODE.md`.
   If you did not measure, say so, and say why the change cannot move them.
2. **A test per behaviour, and it fails with its fix reverted.** Never weaken, skip or ignore an
   existing test.
3. **fmt and clippy are clean before a push**: `cargo fmt --all --check` and
   `cargo clippy --release --workspace --all-targets -- -D warnings`.
4. **There is CI.** The workflow `ci` runs on every push to `main` and every pull request, on
   Linux, Windows and two Macs: fmt, clippy, the tests, the screenshot suite. Run the tests
   yourself first, then let it confirm. Start a run once and do not poll it; `gh run watch` once
   used up the account's API quota. Wait, then look once.
5. **A release is cut by pushing the tag `v<version>`** after `ci` is green on `main`
   (`docs/RELEASE.md`). Not by a local script. A version that failed is not used again.
6. **The owner approves the interface by looking at it.** A shot is how you show it, not proof
   that it is right. Every new visible state goes into `tests/shots.sh`; look at each one, in
   both themes; say which shot shows what. A shot can be right by accident: the rename box of
   the notes pane drew nothing while typing, and its shot looked fine.
7. **No new dependency without asking**, saying so plainly, and pricing it: what it adds to
   `Cargo.lock`, to the exe, to the launch path.
8. **Shared first.** The owner (2026-11-13): "whatever can be shared will start shared. no need
   for duplicate work." Code two apps could use is written in `markdown-shared-rs`. A change
   there must not change what an existing caller draws or does unless that caller opts in: add
   a variant beside the old function. Run both apps' `tests/shots.sh` before and after, push,
   then move the `rev` in each app on purpose. Never force-push there.
9. **Never run a build on the owner's settings.** The owner's own mdedit is usually open on this
   PC, on `%LOCALAPPDATA%\mdedit\settings.ini`. Start a build with `--no-session` **and**
   `MDEDIT_SETTINGS` naming a `settings.ini` in a scratch folder: no session means no library, no
   snippets and no saved prompts at all — nothing of the owner's is read or written, and
   "Split into notes…" is refused for want of a library folder — and `MDEDIT_SETTINGS` is the one
   settings file such a build reads, with its library beside it. A development build once took
   over a running window's session.
10. **Other agents may be working in this checkout.** `git status` before you edit. Changes you
    did not make are someone's work: leave them.
11. **The release build aborts on a panic** (`panic = "abort"`): one panic on any thread ends the
    program. Anything read from outside, a file or a model's answer, is parsed with no path that
    can panic.
12. **Commit and push as phases complete, or when asked.** Subjects read `mdedit: …` or
    `docs: …` and say what changed in plain words. The message ends with a `Co-Authored-By` line
    naming the model that wrote it.
13. **Report plainly**: what was run, what was not, what was not verified. Nothing counts as
    working on Windows until it has run on Windows.
14. `spikes/` is throwaway code, kept for reference. So is the branch `mock/ai-review`.

## Using the shared toolkit (`Kaan88/markdown-shared-rs`)

This app is built on the owner's toolkit, pinned by commit in `Cargo.toml` and `bins/mdedit/Cargo.toml`. Its widgets are pieces
that have to be wired together, and an audit of every app on it (2026-11-27) found each one had
wired some of them halfway: a text field with a caret but no drag, no triple click or no
right-click menu; a menu in English in a Turkish app; drags that stayed held after Alt+Tab.

1. **Look before writing.** Before writing input, menu, caret blink, timer, drag, scroll or
   dialog code, read the toolkit's `docs/CATALOGUE.md` at the pinned commit (its "I want to ..."
   index first). Use what is there. If it almost fits, change it in the toolkit, additively as
   its `AGENTS.md` says, rather than copy it here.
2. **Wire a text field whole.** Use `textfield::FieldInput`, or at a pin older than it
   `textfield::Pointer` with `TextField::context_menu`: every press, move and release, the right
   press, the keys, its deadline, and its menu drawn last. Never `TextField::press` alone: it
   places a caret and nothing else (the toolkit deprecates it from `FieldInput` on). The
   toolkit's settings forms, and at a pin that has them its palette and plan screen, have
   `pointer_press` routes that give their fields the same; use those.
3. **Every held press can be lost.** On Windows winit sends no button release when the window
   loses the mouse in the middle of a drag. On `Focus(false)` (or the host's `Event::PressLost`,
   at a pin that has it) call `cancel` (or `let_go`) on every widget that can hold a press. Do
   not fake it with a release far off screen: a release acts on what is under the pointer.
4. **The edit menu speaks the app's language.** Give it the app's words (`EditLabels`,
   `TextField::context_menu_labelled`, `set_edit_labels`).
5. **Move the pin on purpose.** When it moves, read the toolkit's commits since the old pin for
   deprecations and new opt-ins, act on each warning (do not allow them wholesale), run this
   repository's checks, and send the toolkit a pull request for the sibling revision table in its
   `docs/CATALOGUE.md`.
6. **A widget that is hard to wire right is the toolkit's bug.** Fix it there, or add a roadmap
   row there (`docs/ROADMAP.md`), and say so in this app's pull request.

## Keeping these documents true

- `docs/STATE.md` is **rewritten**, never appended to: when a line stops being true, replace it.
  If it passes 150 lines, something in it is history and goes.
- `docs/DECISIONS.md` is **appended to**, never edited: a decision that changes gets a new entry
  that names the old one.
- When a piece of work is done: update STATE.md; add its decisions; write or correct the
  `docs/DESIGN.md` section for what was built; add the line to the release notes.
- Do not write a dated story of the session anywhere. `git log` is the story, and
  `docs/archive/` holds the old ones.
- This file changes only when a rule or a pointer changes.
