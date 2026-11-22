# Where the code is, and how to build, test, look and measure

Stable. It changes when a crate, a pin, a command or a script changes. Commits named here are
the ones at `main` `2b407d2` (2026-11-21); the live values are in the two `Cargo.toml` files.

## Three repositories

| On GitHub (`Kaan88/…`) | What | On the owner's PC, in `C:\Users\kaank\Desktop\` |
|---|---|---|
| `markdown-editor-rs` | mdedit, this repository | `markdown-editor-ai` |
| `markdown-viewer-rs` | mdview, the Markdown viewer | `mv` |
| `markdown-shared-rs` | The crates both use | `markdown-shared-rs` |

All three are public and MIT. Anywhere else, clone all three side by side.

**What is written where, when a fact is true of more than one of them.**

- How a shared crate behaves, what it offers and what an app must opt into: in
  `markdown-shared-rs/docs/` (`FEATURES.md`, `SISTER.md` for the hand-off protocol between the
  apps, `LINUX.md`). The apps point there and do not repeat it.
- The rules of the house (CLAUDE.md here): each repository keeps its own short copy, because an
  agent starts in one of them. When a rule changes, it changes in all three.
- A decision that binds both apps is recorded in the app where it was taken, and the other app's
  log gets one line that points at it.
- The state of mdview is in mdview's own documents. This repository says only which viewer
  commit it builds against.

## This workspace

Version 1.3.0, edition 2024. Lines counted on 2026-11-21, tests included.

| Path | Lines | What it holds |
|---|---|---|
| `crates/mdedit-core` | 31,245 | The headless core: the buffer, loading and encodings, line endings, display columns, selection, find, editing, history, the line and Markdown commands, tables, typing help, snippets, folding, diff and merge, replace in files, recovery copies, preferences, the AI's prompts and limits. No window, no threads of its own |
| `crates/mdedit-highlight` | 1,184 | The highlighting worker thread and the rope adapter, over `mdshared-highlight` |
| `crates/mdedit-outline` | 1,425 | The function list and the headings of a document |
| `bins/mdedit` | 57,017 | The shell: the window loop and everything drawn |
| `bins/mdedit-ai` | 3,786 | The AI engine, a separate program. JSON lines on stdin and stdout (`proto.rs`); it holds the keys (`keys.rs`) and makes every request (`api.rs`, `cli.rs`). The editor starts it with the first request it has for it (`Ai::ask`), never at launch |
| `fixtures/` | | Small test files with exact bytes; `fixtures/make_fixtures.py` writes them and `.gitattributes` keeps git from converting them |
| `tests/` | | `shots.sh` and `shots-check.sh` (below), and scripts for the Mac runners |
| `tools/` | | `release.ps1`, `release-all.sh`, `write-manifest.sh` (releases); `compare-builds.py` (budgets) |
| `packaging/` | | The Windows installer and zip, the Linux packages, the Mac bundle |
| `shots/` | | Pictures committed for the owner to look at |
| `spikes/` | | Throwaway code from phase 0 and from the visual-editing spike, kept for reference |

### Inside `bins/mdedit/src`

Every one of its 67 files starts with a `//!` comment that says what it holds; read that before
the code. The ones a task most often needs:

| File | What |
|---|---|
| `app.rs` (5,205 lines) | `App`, the event loop, the regions and what redraws them, the frame, `--shot` |
| `app/input.rs`, `app/commands.rs` | Keys and pointer; what each command does |
| `app/modal.rs`, `overlays.rs` | Settings and dialogs; menus |
| `ui.rs` | The enums everything else is keyed on: `Region`, `Hit`, `PopupKind`, `DialogKind`, `LibAct` |
| `commands.rs` | The command table and every string the app shows (`strings::…`) |
| `args.rs` | Every command-line flag, including the ones that set up a shot |
| `docs.rs` (6,014) | Documents and tabs: load, save, reload, read-only copies |
| `view.rs`, `view/` | The text view |
| `chrome.rs`, `panes.rs`, `status.rs` | The title row and bar; the left pane's head; the status strip |
| `palette.rs`, `app/palette_events.rs` | The command palette and quick open |
| `results.rs`, `replace_ui.rs`, `app/replace_files.rs` | Find in files and its results dock; the Replace in files confirmation and report |
| `compare.rs`, `app/compare_events.rs` | Compare two files, copy a change across |
| `ai.rs`, `ai_ui.rs`, `app/ai_events.rs` | The engine's client; the instruction panel and the review; the wiring |
| `library_pane.rs`, `app/library.rs`, `app/library_write.rs` | The Notes face of the left pane; reading the library; every write to a library file |
| `preview_pane.rs`, `app/preview.rs`, `app/visual.rs`, `app/handoff.rs` | The rendered page, visual editing, the hand-off with mdview |
| `session.rs`, `recovery.rs` | The session file; recovery copies of unsaved text |
| `image_paste.rs`, `macros.rs`, `minimap.rs`, `outline.rs`, `updater.rs` | What their names say |

Tests sit beside the code: `#[cfg(test)]` modules in each file, and `app/*_tests.rs` for the ones
that drive the whole app headless.

## The shared crates, and the pins

`markdown-shared-rs` holds five crates:

| Crate | What |
|---|---|
| `mdshared-app` | Plumbing with no window: paths, the settings file, recent files, single instance and the folder lock, file watching, fuzzy matching, crash reports, the updater, the hand-off protocol, the notes scan, the notes library (`library.rs`), folder search |
| `mdshared-ui` | Everything drawn that both apps have: the `Painter` and palettes, menus, the command palette, the settings form, the single-line text field, the folder tree, pane heads and rows, scrollbars, dialog cards and buttons, tooltips and toasts, notice bars, the window shadow, corners, frost, snap, and the platform code |
| `mdshared-text` | Fonts, the text engine, glyph bitmaps |
| `mdshared-highlight` | The syntect set-up, the incremental line highlighter, Vue and Svelte |
| `mdshared-fetch` | A hardened fetcher for remote images |

**How they are pinned.** By commit, in two files:

- Root `Cargo.toml`, `[workspace.dependencies]`: the five `mdshared-*` lines, all at one `rev`
  (`f75b4c5`). `[patch.crates-io]` replaces winit and softbuffer with the owner's forks
  (`Kaan88/winit` `62fa727`, `Kaan88/softbuffer` `dd18fec`).
- `bins/mdedit/Cargo.toml`: `mdview-preview` and `mdview-render` at a commit of the viewer
  (`e2b3e31`), behind the default feature `embedded-preview`; and `mdshared-app-106`, the shared
  crate as it was at mdedit 1.0.6 (`49de4ec`), which one test uses to read a release manifest
  the way an old install would.

So a build holds two copies of some shared crates: the editor's, and the ones the viewer's
renderer was built against. That is expected.

mdview pins the shared crates too, at its own commit (`cb0ffd3` on 2026-11-21, older than the
editor's). **A change to a shared crate must therefore not change what an existing caller draws
or does unless that caller opts in.** Add a variant beside the old function
(`pane::draw_head_switches` beside `draw_head`), an option that is off unless asked for
(`Menu::access_keys(true)`), or a cargo feature (`client-margin`).

**Changing shared code.**

1. Edit it in `markdown-shared-rs`. Its tests, fmt and clippy must pass there; it has its own
   `ci`.
2. Run both apps' `tests/shots.sh` before and after and compare the folders: a shot that changed
   and should not have is the bug.
3. Commit and push there. Never force-push.
4. In the app: change the `rev` on every `mdshared-*` line, then
   `cargo update -p mdshared-ui -p mdshared-app -p mdshared-text -p mdshared-highlight -p mdshared-fetch`.
   It should only re-resolve those. If it moved a third-party crate in `Cargo.lock`, put that
   entry back.
5. Commit the pin on its own: `mdedit: pin the shared crates at …`.

While working on both at once, a local `[patch]` to the checkout on disk saves the round trip.
It must not be committed.

## Build and test

In Git Bash, in the repository. A built `.exe` is started from Git Bash too: on 2026-11-13
PowerShell refused it as unsigned (not re-checked since).

```bash
export TEMP="$PWD/target/tmp" TMP="$PWD/target/tmp"; mkdir -p target/tmp   # keep temp files off the system drive
cargo build --release --workspace
cargo test --release --workspace
cargo clippy --release --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

All four are what `ci` runs, on Linux, Windows and two Macs (`docs/CI.md`). A test must not
assume this PC: the first CI runs failed on line endings, on `\` in paths, and on a help line
that was 9 px too wide in the runner's font.

## Running a build without touching the owner's

The owner's mdedit is usually open, on `%LOCALAPPDATA%\mdedit\settings.ini`; the session, the
macros, `ai-prompts.md` and the `library` folder sit beside that file.

- `--no-session`: the window reads and writes no session. Every shot uses it.
- `MDEDIT_SETTINGS=<scratch>\settings.ini`: a settings file of your own, with everything else
  beside it. Use a fresh folder per run when measuring.
- `MDEDIT_AI_ENGINE` names another engine binary; `MDEDIT_UPDATE_MANIFEST` another update
  manifest.

## Shots

```bash
cargo build --release -p mdedit
bash tests/shots.sh target/shots/a        # renders every state into the folder
bash tests/shots-check.sh target/shots/a  # fails if a shot is missing, empty or not a PNG
```

- A shot is the app drawn into a PNG with no window: `--shot out.png`, with `--size WxH`,
  `--scale N`, `--light` or `--dark`, and flags that put the app in a state: `--menu NAME`,
  `--dialog NAME`, `--palette`, `--ai-demo KIND`, `--replace-dialog`, `--compare A B`,
  `--pref key=value`, `--hover X,Y`, `--type TEXT`. All of them are in `args.rs`.
- On one machine the set is deterministic: run it into two folders and every PNG is the same,
  byte for byte. Between machines it is not, because the fonts differ, so `ci` checks only that
  every shot exists, and uploads them (`shots-windows`, `shots-linux`, `shots-macos-…`).
- **Every new visible state gets a line in `tests/shots.sh`**, in both themes where it has
  colours of its own. Then open the PNGs and look.
- A shot proves what was drawn, not that it is right, and not that it redraws. For anything
  that changes while the user types or points, write a test that drives the app and compares
  pixels before and after.

## Measuring

```bash
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 fixtures/sample.rs
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 target/data/100mb.txt
target/release/mdedit.exe --no-session --trace --type-steps 300 --goto 20:5 fixtures/sample.rs
target/release/mdedit.exe --no-session --trace --scroll-steps 300 fixtures/sample.rs
```

`--trace` prints the stage times, the first frame, memory and CPU time. `target/data/100mb.txt`
is made by the first run of `tests/shots.sh`.

Every test window is started with `--behind` (the owner, 2026-11-22): it opens without the focus
and stays under every other window, so a run does not cover the owner's work. For
`tools/compare-builds.py` pass it as `MDEDIT_ARGS=--behind`; a build older than the flag ignores
it (with a line on stderr) and opens in front.

**The method.** Two release builds, before and after, in two worktrees. Run them in turn, not
one after the other, with a fresh settings folder per run, at least seven runs each. Report the
median and the range of each, and say what was not measured. `tools/compare-builds.py EXE_A
EXE_B RUNS FILE…` does this: it alternates the two, starts each with `--trace` three ways (until
it settles, `--type-steps 300`, `--scroll-steps 300`) and prints the medians.

The absolute first-frame number on this PC changes from day to day. Compare the two builds with
each other, not with last week.

## Commits and branches

- `main` is what is built and released. Larger pieces of work have gone on branches named
  `f/<name>` and been merged into `main`; the library redesign was committed to `main` phase by
  phase.
- Subjects: `mdedit: …` for code, `docs: …` for documents, then what changed, in plain words.
  The message ends with `Co-Authored-By: <model> <noreply@anthropic.com>`.
- A second agent in the same repository works in its own git worktree, with its own target
  folder or a shared `CARGO_TARGET_DIR`; it does not build in another agent's folder.
