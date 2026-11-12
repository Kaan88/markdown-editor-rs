# Handoff: mdedit and the shared crates

Written 2026-11-13. Read this first, then `docs/PLAN.md` (goals, budgets, phases, feature roadmap)
and `docs/DESIGN.md` (the design of phase 1, and section 13 on what phase 2 adds).

## 1. Where things are

Three public repositories under `Kaan88`, all MIT, all pushed and clean on `main`:

| Repository | What | Head on 2026-11-13 |
|---|---|---|
| `markdown-viewer-rs` | `mdview`, the Markdown viewer. Its own `docs/HANDOFF.md` describes it | `b4e1617` |
| `markdown-shared-rs` | Four crates both applications use | `2caa930` |
| `markdown-editor-rs` | `mdedit`, this repository | this commit |

The viewer and the editor each pin the shared repository by `rev`
(`09576bc19cc5d9ff6bbe128bb1e699b6cc51d8e0`): the viewer in `crates/core`, `crates/render` and
`bins/mdv-app`, the editor in its root `Cargo.toml`. `2caa930` only changed the shared
repository's README and plan, so the pin was not moved.

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
- The user asked for cheaper models where they do the job. On 2026-11-12 the mechanical moves,
  the spikes, the headless crates and the checks ran on Sonnet; the design, the shell, the
  reviews and the fixes on Opus.

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

## 4. What has not been done or checked

- **The user has not looked at either application since these changes.** The editor was opened on
  their screen once; no feedback yet. Nothing about the interface is approved.
- Nothing interactive in the editor was tried by hand: selection, copy into another program, find,
  tabs (drag, middle click, menu), the Files pane, theme switch, reload when the file changes, a
  second launch handing over to the first.
- The viewer's first-frame time and memory were not re-measured after the move to shared crates,
  and its installer was not rebuilt. The installed `mdview` on the PC is the build from before
  the move.
- Idle CPU with the editor's window unfocused was not measured.
- The input-method hand test (`spikes/ime/RESULTS.md`: Turkish Q dead keys and AltGr, a CJK IME)
  was not done. Phase 2 depends on what it shows.
- Linux: nothing in the shared crates or the editor has been built there.
- The editor's small-file first frame sits at the edge of its budget: about 60 ms of it is
  syntect start-up before the first colours.

## 5. Decisions the user has not confirmed

These were made to keep moving and are easy to undo:

1. **`panic = "abort"`** in the editor's release profile. It saved 0.9 MB and brought the exe
   under 6 MB. A panic on any thread ends the process, which matters more once there is unsaved
   text. The viewer uses `unwind`. Revisit before phase 3, or with unsaved-session restore.
2. **Crate names** `mdshared-*` (the shared plan called them placeholders).
3. **Step 4 put off**, as above.
4. **`mdshared-app` created** (open decision 1 of the shared plan, taken as it recommended).
5. Open in `docs/PLAN.md` and untouched: the editor's name, one installer or two, word-wrap
   default, Markdown conveniences.

## 6. What to do next

The user paused the work right before this step. When they say to continue:

1. **Phase 2, editing core, and phase 3, safe saving**, as `docs/PLAN.md` defines them.
   `docs/DESIGN.md` section 13 says where editing fits. The order used for phase 1 worked: a
   design with compiling stubs and three packages with no file overlap, the two headless packages
   in parallel (edit operations and undo in `mdedit-core`; saving with byte-exact round-trip
   tests), then the shell by one writer, then an independent check, a review by lens, fixes, and a
   re-check.
   - Done when: the keystroke budget (under 8 ms, typical 2 to 4) holds on the small and the
     100 MB file; a scripted edit run matches a reference buffer; round trips over UTF-8, BOM,
     UTF-16, Windows-1254, CRLF, LF and mixed files change no byte they should not.
   - A typing trace flag is needed, like `--scroll-steps`, so the keystroke time can be measured
     without a person.
2. Then the feature roadmap in `docs/PLAN.md`, in its order.
3. Any change to shared code: edit it in `markdown-shared-rs`, run the viewer's check (section 7),
   push, then move the `rev` in both applications.
4. Code the editor's shell copied from the viewer because it was generic but had not been moved
   (parts of the window loop, the bar, the find box wiring) is the input for step 4 of the shared
   plan.

## 7. Build and check

```bash
# in this repository; on the PC, keep temp files off the system drive's temp folder
export TEMP="$PWD/target/tmp" TMP="$PWD/target/tmp"; mkdir -p target/tmp
cargo build --release --workspace
cargo test --release --workspace        # 138 passed on 2026-11-12
cargo clippy --release --workspace --all-targets -- -D warnings
cargo fmt --all --check
bash tests/shots.sh target/shots/a      # 33 PNGs, deterministic; first run generates target/data
```

Useful flags of `mdedit.exe` (all in `bins/mdedit/src/args.rs`): `--no-session`, `--shot out.png`
with `--size WxH --scale N --light|--dark --menu NAME --dialog NAME --find TEXT --goto L[:C]
--select A-B --scroll N --hscroll N --hover X,Y`; `--trace` prints stage times, first frame,
memory and CPU time; `--scroll-steps N` times scroll frames; `--exit-after-ms N` opens a real
window and closes it.

```bash
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 fixtures/sample.rs
target/release/mdedit.exe --no-session --trace --exit-after-ms 2500 target/data/100mb.txt
```

**The viewer's no-pixel-change check.** The script and its baseline lived in a session scratch
folder on the PC and are not in any repository. To recreate it: in the viewer, at a commit known
to be good, render a fixed set of `--no-session --shot` states (both themes, each `--menu`, each
`--dialog`, the empty state, panes, find, focus mode, page widths, scales 1.0, 1.5 and 2.0, a few
`--hover` positions; 43 were used) into a baseline folder; after a change, render again and `cmp`
every file, then run the workspace tests (which include the four goldens) and `cargo fmt --check`.
Putting that script into the viewer's repository would be a useful first task.

Fixtures are written with exact bytes by `fixtures/make_fixtures.py`; `.gitattributes` keeps git
from converting them.
