# The quickest idea to a first release, and its plan

Written 2026-11-23 for the owner to approve. "You" is the owner. Nothing here is built, no source
file was changed, nothing was committed. Hub is built and mail was never asked for, so both are
left out.

**The pick: tasks over Markdown, as a Tasks view in mdview, released as mdview 1.0.5.** About
**10 plan days**; about a working week on the calendar if you look at the drawing and at the
build within a day each time. The runner-up is the local-LLM workbench in mdedit.

**What I read.** `docs/ecosystem-ideas.md` (with its uncommitted 6b to 6d), `STATE.md`,
`RELEASE.md`, `CLAUDE.md`, the headings of `DECISIONS.md`; the plans in the worktrees
(`terminal.md`, `terminal-plan.md`, `file-manager-plan.md`, `task-manager-plan.md`,
`launcher-plan.md`: recommendation, phases, risks and questions of each, not every section);
the workbench mock's README and `numbers.txt`; Hub's `docs/STATE.md`, `docs/RELEASE.md`, its file
list and `git log`; mdview's `docs/PLAN.md` status, the head of `docs/HANDOFF.md`,
`docs/release-notes/1.0.5.md`, `crates/core/src/tasks.rs`; in the shared crates
`foldersearch.rs` (its head and types) and `rows.rs` (its API); in mdedit `bins/mdedit/src/ai.rs`
(the `Ai` state), `bins/mdedit-ai/src/{main,proto,api}.rs` by grep, `preview_pane.rs` by its API.

**What I ran.** `git log`, `git status`, `wc`, `grep`, and one GET to `localhost:1234/v1/models`
and `localhost:11434` to see whether a local model server is up (LM Studio answered with one chat
model; Ollama did not answer). No build, no test, no app started, no measurement.

**What I did not check.** `file-manager-questions.md`, `file-manager-context-menu.md`,
`commander-plugins.md` and the downloader and terminal mock READMEs were not read; the mock shots
were not looked at; `docs/PLAN.md`, `CODE.md` and `DECISIONS.md` of mdedit were read by grep only;
mdview's `docs/UPDATE.md` (its release record) and `folder_search.rs` were not read, only sized.
Every day figure below is an estimate by judgment.

---

## 1. What decides "quickest"

Typing is not the limit here. Hub's plan priced Windows at 29 days; its 44 commits run from
13:57 to 20:48 on 2026-11-22, one afternoon. A day later Hub's own `STATE.md` still says
"Released: nothing", "Seen live: nothing", the installer never built or run, `ci` and `release`
never run. So the calendar is made of four things, and the comparison counts them:

1. **Your looks.** An interface is approved by you looking at it. Each look is a wait.
2. **Open questions** that only you can answer before code starts.
3. **The hand run on Windows**, and what it needs (a real installer, UAC, a second PC, a model
   server).
4. **The release vehicle.** A feature in a released app uses a workflow that has published
   several releases. A new app needs a repository, `ci`, a release workflow, an installer, a
   manifest and `hub.txt`, none of them proven for it: Hub shows that this is where a new app
   stops.

## 2. The comparison

Days are plan days at this repository's bar. "Earlier figure" is what the idea's own document
says; "checked" is what the code or the plan changed.

| Idea | Earlier figure | First release, as I price it | Drawn? | Waiting on you | Vehicle | Verdict |
|---|---|---|---|---|---|---|
| **Tasks over Markdown** | 2-3 pw | **about 10 days** (section 5) | No | One go-ahead, one look at a drawing, two defaults | mdview 1.0.5: notes already started, 8 commits since 1.0.4, no freeze | **Pick** |
| **Local-LLM workbench** | 2-3 pw as a feature | 12 to 16 days, then a frozen release | Yes, 12 states | The mock is not recorded as approved; "No release for now" on mdedit (STATE, 2026-11-22) | mdedit 1.3.0, which also carries the split, the move and Split by AI, none clicked through live | **Runner-up** |
| Feed reader | 3-5 pw | 20 to 25 days plus a new app's release | No | Everything | New repository, the fifth window | No |
| Hardware panel | 3-4 pw | 15 to 20 days plus a new app's release | No | The X G6 CLI is not in these repositories; `ClaymoreControl-1.5` and `Gamma22Tray` already sit on the Desktop | New app; Hub's tray and Run key could be copied | No |
| Task manager | 56 days, slice 18 | 18 days to the slice, then packaging, signing, a Defender test | Yes, about 40 states | 14 questions, among them "build it at all?" and code signing | New app | No |
| Secrets vault | 4-6 pw | 20 to 30 days | No | Everything; KDBX writing is "experimental" in the crate named | New app, security software | No |
| Terminal | 37-45 days | 37 to 45 days | Yes, 20 shots | **Nothing**: all seven questions answered | New app `mdterm`; ConPTY unknown until its spike | No, but see below |
| Downloader window | about 15 pw | 60 to 100 days | Yes, 11 states | The data model, left open by you | New app that ships a .NET Host | No |
| File manager | about 65 days, slice 18 | 18 days to a slice you can try; a release is the 65 | Yes, 19 states | 11 of 12 questions | New app; lost data is the risk | No |

**The terminal** is the only new app with every question answered, and you said nothing comes
before it. It is not the quickest: seven to nine weeks, with ConPTY unproven until phase 0. The
pick does not take its place. The Tasks view is a week in a different repository and can run
beside the terminal's three-day spike.

### Why tasks, from the code

- **The scan exists.** `mdshared_app::foldersearch` walks a folder on its own thread by the Files
  tree's rules, with caps (16 MiB a file, 2,000 hits, 200 a file), `Progress` and `Done` events
  and cancel (`foldersearch.rs`, 668 lines). mdview already has a window part over it
  (`bins/mdv-app/src/folder_search.rs`, 488 lines).
- **The tick exists and is released.** `mdview-core`'s `tasks.rs` (238 lines): `markers(bytes)`
  finds every box and skips fenced code; `toggle_in_file(path, index, expected)` reads the file
  again, refuses when its boxes are not the ones expected, refuses UTF-16 and read-only files,
  and writes one byte. The setting "Tick task boxes without asking" is there.
- **The list exists.** `mdshared_ui::rows` (virtual rows, keyboard, scroll thumb), `segments`
  for the filter, the Files and Outline panes as the model.
- **Sharing costs nothing new.** `mdview-core` already depends on `mdshared-app`, so the scanner
  can start in the shared crate, as rule 8 asks, and mdedit can take it later.
- **No dependency, no network, no engine.** Rule 7 has nothing to price.
- **The release is one tag.** mdview's `release.yml` has published 1.0.0 to 1.0.4.

So `ecosystem-ideas.md`'s "about 80% as a feature" holds for the first release below. Its 2-3 pw
is for Today, Upcoming, By file **and** a board with recurrence in mind; the first release here
is smaller.

### Why the workbench lost

Its mock is drawn and an LM Studio server answers on this PC, which puts it ahead on looks. It
lost on four counts, three of them found in the code:

1. **mdedit's release is frozen by you**, and 1.3.0 would carry work "never clicked through in a
   live window" (the move) and "never run on a real provider" (Split by AI). A release of the
   workbench is a hand pass over all of that first.
2. **The editor runs one AI request at a time.** `Ai` holds `run: Option<Run>` and
   `running()` is `run.is_some() || split.is_some()` (`bins/mdedit/src/ai.rs`). The engine does
   run jobs in parallel (`Jobs`, `mdedit-ai/src/main.rs`), but the editor's side of N runs is new.
3. **The preview pane shows one page.** `preview_pane::Pane` is one viewer and one document.
   Rendered Markdown in three to five columns needs several, or plain text; the mock's README
   says its Markdown is "a stand-in".
4. **A tab that is not a document** was not found in `docs.rs` by grep (not read through). The
   mock draws the workbench as a tab.

The engine's `delta(id, chars)` has no token counts, as `ecosystem-ideas.md` 8 says; that part
was right. With one chat model loaded in LM Studio today, a side-by-side hand run also needs a
second model fetched. My figure is 12 to 16 days against the earlier 10 to 15, and then the
freeze. If you lift the freeze anyway, it is the next quickest and its drawing is done.

## 3. The first release: what is in

A **Tasks view** in mdview over the folder open in the Files pane.

- Every `- [ ]` and `- [x]` line of the folder's Markdown files, found as mdview's page finds
  them (list markers, quotes, fenced code skipped), listed under its file.
- A filter: Open, Done, All; and a text box that narrows the list.
- Sections by date when a task carries one: Overdue, Today, Upcoming, No date. Read only, in the
  one syntax of question 2. This part can be cut (section 8).
- A click on a task opens its file at that line. A click on its box ticks it in the file: one
  byte, refused when the file changed, with the reason in a bar. "Tick task boxes without
  asking" is obeyed as on the page.
- The list follows the folder: a file saved elsewhere is read again.
- Honest edges: no folder open, nothing found, still reading, "2,000 of more shown", a file too
  large or unreadable counted in the foot.

## 4. What is out, and said so

- A board, drag and drop, priorities, projects, recurrence.
- Reminders and anything that runs while mdview is closed.
- Writing or changing a date, editing a task's text, adding a task. mdview stays a viewer; the
  one write is the tick it already makes.
- The same view in mdedit. The scanner is shared so that it is cheap later; it needs mdedit's
  release freeze lifted, so it is not in this plan.
- A library-wide view apart from the open folder; saved filters; sorting other than file order
  and date.
- Hub's `hub.txt` in mdview's release (question 4).

## 5. The phases

A day is a day at this repository's bar: a test per behaviour that fails with its fix reverted,
fmt and clippy clean, shots looked at in both themes, budgets measured before and after.

| # | Phase | Days | Done when |
|---|---|---|---|
| 0 | **Read, measure, draw** | 1.5 | Below |
| 1 | **The shared scanner** | 1.5 | Below |
| 2 | **The view** | 2.5 | Below |
| 3 | **The tick from the list** | 1 | Below |
| 4 | **Dates** (can be cut) | 1.5 | Below |
| 5 | **Budgets, review, the hand run** | 1.5 | Below |
| 6 | **Release 1.0.5** | 1 | Below |
| | **Total** | **10.5** | 9 without phase 4 |

### Phase 0: read, measure, draw (1.5 days). Ends at your look.

- Read `folder_search.rs`, `panes.rs` and `workspace/` in mdview and decide where the view sits:
  a third face of the left pane beside Files, or a panel as the folder search is. Write the
  choice and its reason into mdview's HANDOFF.
- Check that `HANDOFF.md`'s heading "Settings as pages ... not merged" is stale (mdedit's
  D-094 says it is merged at `29bf0ab`); 1.0.5 must be cut from a `main` that is known.
- Measure, on a scratch copy of a large notes folder with a scratch `MDVIEW_SETTINGS`, the time
  of one `foldersearch` walk with a matcher that matches task lines. This sets the cap and tells
  whether the list fills while you watch.
- Draw the view behind `--mock`, as the other drawings were: main list, filter on Done, the date
  sections, a refused tick, no folder, nothing found, still reading, the cap reached; both
  themes, 1280x800 and 520x360. About 16 shots.
- **Done:** the shots are in a folder you can open, each one looked at, a list says which shows
  what, and you have said yes, no, or what to change. Nothing after this phase is merged before
  that.

### Phase 1: the shared scanner (1.5 days). Can run beside phase 0's drawing.

- `mdshared_app::tasks`, new and additive: the marker scan moved from `mdview-core` (which keeps
  its names by re-export, so nothing in mdview changes), returning for each box its byte, its
  state, its line number and its text; the date reader of phase 4 lives here too.
- A folder scan on the `foldersearch` walker. The walker's matcher sees one line; a task inside a
  code fence is known only from the lines before it, so this is a second entry beside
  `search_tree`, not a change to it.
- **Tests:** the existing `tasks.rs` tests move with the code and stay green unweakened; a file
  with a box inside a fence, a quote, a nested list, a BOM, CRLF, UTF-16 (refused); the caps; a
  cancelled scan; 100,000 random and mutated inputs with no panic (rule 11).
- **Done:** shared `ci` green; both apps' `tests/shots.sh` byte-identical before and after
  (mdedit's clock shots aside); the `rev` moved in mdview in a commit of its own.

### Phase 2: the view (2.5 days)

- The list on `rows`: file headers, task rows, the box, the text cut with an ellipsis, counts;
  the filter on `segments`; the text box; keyboard (arrows, Enter opens, Space ticks, Escape
  leaves); the wheel and the thumb; open at the line.
- Scanning starts when the view is first shown, never at launch, and stops when it is hidden.
- **Tests:** one per behaviour named above, on a scratch folder; the view's hit test without a
  window; "a hidden view starts no thread".
- **Shots:** every state of phase 0's drawing, now real, in `tests/shots.sh`, both themes, both
  sizes; each compared with the drawing by eye.
- **Done:** tests green, shots looked at and listed.

### Phase 3: the tick from the list (1 day)

- A tick calls `toggle_in_file` with the states the list holds for that file. If the file
  changed, the tick is refused, the bar says why, and the file is read again. An open tab of
  that file shows the new state through the watcher it already has.
- **Tests:** a tick writes exactly one byte; a file changed between scan and tick is left alone;
  read-only and UTF-16 are refused with their text; with "without asking" off the question is
  shown; two ticks in a row on one file both land.
- **Done:** each test fails with its guard removed, checked one guard at a time.

### Phase 4: dates (1.5 days, can be cut)

- Read a date in the syntax of question 2; sections Overdue, Today, Upcoming, No date. Today is
  the local date: `GetLocalTime` through `windows-sys` on Windows, `localtime_r` through `libc`
  elsewhere, both already in the lock (to be confirmed for mdview's lock in phase 0).
- **Tests:** the reader on valid, invalid and impossible dates (2026-02-30), a date inside a
  link or code span left alone; the sections with the date passed in, never read from the clock.
- **Done:** tests green; the shots drawn with a fixed date so two runs are byte-identical.

### Phase 5: budgets, review, the hand run (1.5 days)

- **Budgets,** release build, runs in turn before against after: first frame, private memory at
  3 s with the view never opened (must not move), memory with the view open on the large folder,
  CPU over 10 s idle with the view open (0), the scan's time. Written into HANDOFF.
- **Review** by an agent that did not write the code: every guard removed one at a time, each
  must fail a test (as was done for Hub).
- **Hand run on Windows, on a build started through Bash with `--no-session` and a scratch
  settings file:** open a folder, open the view, filter, tick, untick, tick a file that is open
  in mdedit with unsaved changes, tick while a sync tool rewrites the file, a folder of
  thousands of notes, both themes, 100% and 150% scale. Then you use it once on a real folder.
- **Done:** the figures are written, the review's list is closed, you have used it.

### Phase 6: release 1.0.5 (1 day)

Section 10.

## 6. The critical path, and what runs beside it

Your yes (question 1), then the drawing, then **your look**, then phase 2, 3, 5, then **your
use of the build**, then the tag. Two waits for you; everything else is agent time.

- Beside the drawing: phase 1, which draws nothing.
- Beside phase 2: phase 4's reader and its tests in the shared crate.
- Beside phase 3: the release notes, the hand-run list, the before figures of the budgets.
- Not in parallel: the shared `rev` move (one commit, alone), and anything in mdview's `app`
  files by two agents at once.

If both looks come within a day, the calendar is five to seven working days. If phase 1 is
started before your look and the answer is no, 1.5 days are lost and the shared scanner stays
unused; that is the price of the parallel start.

## 7. Reused, and built

| Reused as it is | From |
|---|---|
| The folder walk, its rules, caps, events, cancel, the worker | `mdshared_app::foldersearch` |
| The box scan, the one-byte write, its refusals | `mdview-core::tasks` (moved, re-exported) |
| Virtual rows, keyboard, scroll thumb | `mdshared_ui::rows`, `scroll` |
| The filter switch, the text box, the bar | `mdshared_ui::segments`, `textfield`, `notice` |
| Watching the folder | `mdshared_app::watch` |
| The pane, its splitter, the Files face as the model | mdview `panes.rs` |
| The question before a tick, and its setting | mdview `workspace` |
| Release, manifests, installer, `ci` | mdview's workflows |

| Built | Where | Days |
|---|---|---|
| The task model (line, text, state, date) and the folder scan | `mdshared_app::tasks` | 1.5 |
| The date reader and the sections | `mdshared_app::tasks` | 1 of phase 4 |
| The view: rows, headers, filter, keys, states | mdview | 2.5 |
| The tick from a list row | mdview | 1 |
| The local date | mdview or `mdshared_app` | 0.5 of phase 4 |

## 8. Risks, and when to stop

| Risk | Why it is real | What the plan does |
|---|---|---|
| **You did not ask for it.** | mdview is "feature complete, in maintenance"; a feature goes in only when you ask. Tasks is the one idea here nobody drew or followed up | Question 1 comes first; 1.5 days are spent before your look, no more |
| **A tick damages a file.** | It is the only write, into files you care about, from a list that may be seconds old | `toggle_in_file` reads again and refuses on any disagreement; phase 3's tests; the hand run against an unsaved mdedit and a sync tool |
| **The scan breaks a budget.** | A folder of thousands of files, a watcher that fires in bursts | Nothing at launch, nothing while hidden, the walker's caps, one rescan per burst; measured in phase 5 |
| The syntax of dates becomes a promise | Whatever is read first is what you will write | Read only, one syntax, question 2; phase 4 can ship later |
| Moving `markers` to the shared crate changes mdview | A released behaviour | Re-export, the tests move unweakened, shots byte-identical |
| 1.0.5 also carries Settings as pages | Approved by shots; whether clicked through live is not recorded for mdview | It joins the hand run of phase 5 |
| Linux and macOS | The view is drawn the same, but nothing of it is run by hand there | `ci` runs tests and shots on all three; said in the notes as it is for every release |

**Stop or change course when:**

- You look at the drawing and do not want it: stop. Cost 1.5 days (3 if phase 1 ran beside it).
  The workbench is next, if the freeze is lifted.
- The view open on the large folder puts private memory over 30 MiB, or the view closed moves
  any launch figure: fix within a day or lower the caps; if neither works, stop before phase 3.
- The hand run shows a tick that lands wrongly or a refusal that fires on ordinary files: ship
  the list **read only** (a click opens the file; the tick stays on the page, as today). That is
  still a release.
- Phase 4 is not done when phase 5 is: release without dates, as 1.0.5, and dates as 1.0.6.

## 9. What you decide

| # | Question | My default |
|---|---|---|
| 1 | Build the Tasks view in mdview and release it as 1.0.5? | Yes. It is a feature in a maintained app, so it needs your word |
| 2 | Which date is read? | `due:2026-12-01` after the task's text, and the calendar-emoji date of Obsidian's Tasks plugin; nothing is written |
| 3 | What is scanned? | The folder open in the Files pane, and nothing when none is open |
| 4 | Does 1.0.5 also carry Hub's `hub.txt` (Hub's `docs/APP-CHANGES.md`)? | No: that is Hub's change, decided there; about half a day if you want it in |

Nothing else needs you before code. Where the view sits is shown by the drawing.

## 10. Releasing it

mdview is released the family's way, as `docs/RELEASE.md` here describes for mdedit: a pushed
tag, never a local script. mdview's own record is `docs/UPDATE.md` in `..\mv`, which I did not
read; where it differs from the steps below, it wins.

**Before**

1. Everything is on mdview's `main`; nothing known to be wrong holds it.
2. The budgets of phase 5 are measured and written down.
3. `docs/release-notes/1.0.5.md` is finished. Its first line is what the update bar shows every
   user; today it reads "mdview 1.0.5 (not released yet): Settings as pages." and must be
   rewritten for them, naming the Tasks view.
4. 1.0.5 has not been used as a version. (HANDOFF says "the next mdview release is 1.0.2 or
   later" after the withdrawn 1.0.1; 1.0.5 is free.)
5. Only mdview is released (D-014). The shared crates are pinned by commit and not released.
   mdedit is not touched.

**Cutting it**

1. Set the version in `[workspace.package]` and, as the earlier releases did, `pkgver` in
   `packaging/arch/PKGBUILD`; commit, push, wait for `ci` on `main`. Start the run once and look
   once; do not poll.
2. A dry run of `release` with `publish` unchecked, if mdview's workflow has it as mdedit's
   does (not checked).
3. `git tag v1.0.5 && git push origin v1.0.5`.

**After**, against the live release:

- it is not a draft or a pre-release and names the assets `docs/UPDATE.md` lists;
- `releases/latest/download/mdview-update.txt` (and any other manifest mdview publishes) names
  1.0.5;
- every file a manifest names downloads with the SHA-256 the manifest gives;
- the published exe's `--check-update` answers "nothing newer", and 1.0.4's answers that 1.0.5
  is available;
- on this PC, the installed 1.0.4 is updated from its own bar and opens with the Tasks view.
  This last step has never been done for any release of either app (`RELEASE.md`, "Not
  verified"); do it this time, because it is five minutes and it is the step users take.

Then: the record in `docs/UPDATE.md`, mdview's HANDOFF and PLAN status, and one line in this
repository's `STATE.md` that mdview is at 1.0.5.

## 11. If you say no to tasks

The order after it, by the same count: the **workbench** once mdedit may be released (12 to 16
days; decide a tab or a full-window panel first, and plain text or rendered columns); then the
**terminal**, which waits on nothing but its three-day spike and is the only new app you have
already answered every question for.
