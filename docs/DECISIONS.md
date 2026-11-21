# Decisions

What has been decided about mdedit: what it is, what it will and will not do, and how the work
is done. How a part is *built* is in `docs/DESIGN.md`; this file is for the choices that someone
could otherwise make again, differently, without knowing.

**The rules of this file.**

- It is appended to, never edited. An entry's text does not change after it is written.
- A decision that changes gets a **new entry** that says which one it replaces. The only thing
  ever added to an old entry is one line: "Replaced by D-0xx".
- Numbers are never reused.
- Every entry has the date, the decision, why, what would overturn it, and where it came from.
- "Waiting" means it was taken on the owner's behalf and the owner has not said yes. When they
  answer, add a line with the date ("Confirmed", or "Replaced by D-0xx"). Until then it is the
  default, and it is built on.
- Before deciding anything, search this file. If it is here, follow it or bring it to the owner.

Sources are named as they are after the reorganisation of 2026-11-21: the old plan is
`docs/archive/PLAN-2026-11-12-to-2026-11-20.md`, the old handoff is
`docs/archive/HANDOFF-2026-11-13-to-2026-11-21.md`, the library redesign is
`docs/archive/library-redesign.md`.

## By subject

| Subject | Entries |
|---|---|
| What the app is and is not | D-001, D-002, D-003, D-004, D-012, D-013, D-017 |
| Speed, size, the build | D-001, D-005, D-008, D-009 |
| The network and the AI | D-007, D-015, D-023, D-030, D-032 to D-035, D-049, D-052, D-055 |
| The shared crates and mdview | D-006, D-019, D-020, D-054, D-056 |
| Releases | D-014, D-016 |
| The notes library | D-018, D-021, D-022, D-024 to D-031, D-036 to D-046, D-050, D-051 |
| The review of changes | D-047, D-048, D-053, D-059 |
| Left out on purpose | D-003, D-010, D-013, D-026, D-057 |
| The look of the window | D-011 |
| These documents | D-058 |

---

## D-001 · 2026-11-12 · Speed comes first

**Decision.** A feature goes in only if first frame, keystroke time, scroll frame, memory and
idle CPU stay within the budgets of `docs/PLAN.md`, measured before and after.
**Why.** The owner: "as long as the app remains just as fast to open and work, implement useful
features from Notepad++."
**Overturned by.** The owner.
**From.** The old plan, "State (2026-11-13)".

## D-002 · 2026-11-12 · Its own program, drawn by hand

**Decision.** `mdedit` is a separate binary, not a mode of mdview. Pure Rust, no webview, no
toolkit: winit for the window and input, softbuffer for the pixels, every control drawn by hand.
It opens any text file. It looks and behaves like the viewer.
**Why.** The owner wanted "an editor as fast and pleasant as the viewer: a Notepad alternative
with syntax highlighting" (the viewer's plan, 2026-11-11).
**Overturned by.** The owner.
**From.** The old plan, "What it is".

## D-003 · 2026-11-12 · Not an IDE

**Decision.** No language servers, no build or debug, no plugins, no terminal.
**Why.** It is a light editor; each of those is a different program.
**Overturned by.** The owner. (Word completion from the open files was added in 0.6.0 and is not
a step towards this.)
**From.** The old plan, "What it is not".

## D-004 · 2026-11-12 · Not a WYSIWYG Markdown editor

**Decision.** Markdown is edited as text; the rendered page is the viewer's job.
**Why.** Typing into rendered text needs a second editor core.
**Overturned by.** Amended by D-017, which adds visual editing one block at a time and keeps the
rest.
**From.** The old plan, "What it is not".

## D-005 · 2026-11-13 · The buffer, long lines and highlighting, chosen by measurement

**Decision.** The buffer is `ropey`. Lines up to 4 KB are shaped whole, longer ones in visible
chunks of about 1 KB. Highlighting is syntect with a stored state every 16 lines, run in
time-boxed slices on a worker.
**Why.** Phase 0's spikes: a 100 MB file loads in about 75 ms, edits and lookups take
microseconds (`spikes/*/RESULTS.md`).
**Overturned by.** A measurement that says otherwise.
**From.** The old plan, "State (2026-11-13)".

## D-006 · 2026-11-13 · Shared first

**Decision.** Anything both applications could use is written in `markdown-shared-rs`, pushed,
and pinned; the application keeps only its own names and settings.
**Why.** The owner: "whatever can be shared will start shared. no need for duplicate work."
**Overturned by.** The owner.
**From.** The old handoff, "2. The user's rules".

## D-007 · 2026-11-13 · It updates itself

**Decision.** mdedit looks for and installs its own updates the way mdview does, through the
shared updater, from a manifest on the latest GitHub release. One setting turns it off.
**Why.** The owner said to wire the shared updater in. This ended the plan's "no network use at
all".
**Overturned by.** The owner.
**From.** The old handoff, "5. Decisions the user has not confirmed", item 8; the old plan, "What
it is not".

## D-008 · 2026-11-14 · The exe size is not a gate

**Decision.** The size is reported with each release, and only a sudden large jump needs
explaining. The gates are, in order: launch time, then keystroke, scroll, memory and idle CPU.
**Why.** The owner, 2026-11-14.
**Overturned by.** The owner.
**From.** The old plan, "State (2026-11-13)".

## D-009 · 2026-11-15 · The release profile stays as it is

**Decision.** `panic = "abort"`, fat LTO, one codegen unit, and most crates compiled for size.
A panic on any thread therefore ends the program.
**Why.** Compared on Windows: a build with the editor compiled for speed was 8.76 MB against
6.48 MB and no faster to start or to type in.
**Overturned by.** A measurement that shows a speed gain worth the size.
**From.** The old plan, "State (2026-11-13)" and "Next release".

## D-010 · 2026-11-15 · Folding stays by indentation

**Decision.** No syntax-aware folding.
**Why.** Considered and not planned.
**Overturned by.** The owner.
**From.** The old plan, "Feature roadmap".

## D-011 · 2026-11-15 · A frosted window, behind the chrome only

**Decision.** The backdrop shows behind the title row, the tab strip, the Files pane and the
status strip. The text area stays opaque. There is a setting, and it includes "off". Every
surface keeps its flat colour as the fallback.
**Why.** Text is blended against a known colour and reads worse over a shifting backdrop; the
system does the blur, so the app pays nothing per frame.
**Overturned by.** The owner.
**From.** The old plan, "Later (owner request 2026-11-15)".

## D-012 · 2026-11-17 · Feature complete, in maintenance

**Decision.** Every roadmap is built. From here: fix bugs found in daily use; keep dependencies
current (Dependabot opens pull requests every Monday, `ci` runs on each, they are merged by hand
once it passes and the budgets are measured; the sister crates and the winit and softbuffer
forks are moved by hand). A new feature goes in only when the owner asks for it, under the same
rules as before.
**Why.** The owner agreed on 2026-11-17 that mdedit is feature complete.
**Overturned by.** The owner asking for a feature, one at a time. They have, three times since:
D-015, D-017, D-018.
**From.** The old plan, "Status".

## D-013 · 2026-11-17 · Set aside

**Decision.** Not built: screen-reader accessibility, plugins, printing, the FTP and project
panels.
**Why.** Set aside by the owner. Accessibility matters more for an editor than for a viewer and
is still not planned: a hand-drawn interface has no screen-reader support.
**Overturned by.** The owner; each can be reopened.
**From.** The old plan, "Status" and "Risks".

## D-014 · 2026-11-17 · Release only the app a change affects

**Decision.** A release of one app is not followed by a release of its sister unless the sister
changed.
**Why.** mdview 1.0.1 carried no change of its own and was withdrawn.
**Overturned by.** A change that both apps must ship together.
**From.** The old plan, "Status".

## D-015 · 2026-11-19 · AI rewriting, on three conditions

**Decision.** mdedit can send text to a model and put the answer back: local Claude Code and
Codex, the Claude API, OpenAI-compatible APIs with provider presets, model and effort settings.
The conditions: the connection is **off by default**; when it is on, **only the text a command
is run on** goes, and only to the provider chosen; and **the editor makes no request itself**,
the separate `mdedit-ai` engine does.
**Why.** The owner asked for it on 2026-11-19. The conditions are what keep "no network use"
true for everyone who does not turn it on.
**Overturned by.** The owner.
**From.** The old plan, "Status" and "What it is not"; `docs/DESIGN.md` section 68.

## D-016 · 2026-11-19 · A version number is used once

**Decision.** A version that failed before it was published, or that was published, is never
used again. The next attempt takes the next number.
**Why.** 1.1.0 failed in CI on the Intel Mac and the version moved to 1.1.1; when 1.1.1 needed
two fixes it stayed as published and 1.1.2 followed.
**Overturned by.** Nothing: installed copies compare version numbers.
**From.** `docs/release-log.md`, the sections for 1.1.1 and 1.1.2.

## D-017 · 2026-11-20 · Visual editing, one block at a time

**Decision.** The window can show the rendered page; a click on a block turns only that block
into its Markdown source, in place, edited as plain text; Ctrl+E swaps to the whole source and
back. One buffer, no second editor core. Typing straight into rendered text stays out.
**Why.** The owner asked for it and approved the spike: "fast, jumps are not jarring"
(`spikes/blockedit/RESULTS.md`).
**Overturned by.** The owner. Amends D-004.
**From.** The old plan, "Proposed (owner request 2026-11-20)". Released in 1.2.0.

## D-018 · 2026-11-20 · A notes library that moves nothing

**Decision.** Quick access to notes scattered anywhere. A `library` folder; each `.md` file in
it is one group; a heading is a section; a note is a link or a path. Nothing scans in the
background (no `folder:` lines). Quick open (`Ctrl+Shift+N`) searches titles and file names, not
the text of the notes. Both apps read the same files, through the shared crates.
**Why.** The owner asked for it on 2026-11-20 and answered the two open questions that way.
**Overturned by.** The owner.
**From.** The old plan, "Notes library".

## D-019 · 2026-11-21 · Sisters, and more may join

**Decision.** What the apps can share goes in the shared crates, with future apps in mind.
**Why.** The owner: the two apps are sisters, more apps may join. Restates D-006.
**Overturned by.** The owner.
**From.** The old handoff, "Sharing with mdview (2026-11-21)".

## D-020 · 2026-11-21 · A shared function another app calls is not changed

**Decision.** When mdedit needs a shared function to do more, the function gets a variant beside
it; the old one keeps its signature and draws exactly what it drew.
**Why.** `mdshared_ui::pane::draw_head` draws mdview's panes too. The head switches went into
`draw_head_switches`, and mdview was untouched until it wants them.
**Overturned by.** Both apps moving their pin in the same change, on purpose, with both apps'
shots compared.
**From.** The library redesign, section 9, the first correction.

## D-021 · 2026-11-21 · No deleting the owner's files from the notes pane

**Decision.** Links only. "Reveal in File Explorer" is the way out to a real delete.
**Why.** A destructive action and a new dependency for one menu item nobody had missed.
**Overturned by.** The owner asking for it; then the item and the dependency, half a day.
**From.** The library redesign, section 9, decision 1. Taken for the owner. Narrowed by D-031.

## D-022 · 2026-11-21 · Links only, no managed folder

**Decision.** No folder the library owns, no `home:` line.
**Why.** The pain was thought to be an unindexed disk, not a storage layout.
**Overturned by.** The owner using it.
**From.** The library redesign, section 9, decision 2. Taken for the owner.
**Replaced by D-029**, the same day.

## D-023 · 2026-11-21 · What an AI import may send

**Decision.** Digests go to the provider: 2 KB a file, at most 500 files a run. The confirmation
names the bytes and the number of runs before anything leaves the machine.
**Why.** Only what a command is run on is sent (D-015), and the owner must be able to check it.
**Overturned by.** The owner; a smaller cap is a constant.
**From.** The library redesign, section 9, decision 3. Taken for the owner.

## D-024 · 2026-11-21 · A converted file is a new file beside its original

**Decision.** Conversion to Markdown writes a new `.md` beside the original; the original is
never touched.
**Why.** Moving originals brings a managed folder's failure modes into a feature that has none.
**Overturned by.** The owner. D-032 proposes a different place for the new file and is waiting.
**From.** The library redesign, section 9, decision 4. Taken for the owner.

## D-025 · 2026-11-21 · The AI import is developed on a generated folder

**Decision.** Not on the owner's files; the real folder is picked in the app when they try it.
**Why.** It keeps their data out of a development loop.
**Overturned by.** Nothing.
**From.** The library redesign, section 9, decision 5. Taken for the owner.

## D-026 · 2026-11-21 · No drag and drop in the notes pane

**Decision.** "Move to…" instead.
**Why.** Three to four days for what a menu item does in a tenth of that, and it cannot be
reached from the keyboard.
**Overturned by.** The owner; then phase 7 of the redesign as written.
**From.** The library redesign, section 9, decision 6. Taken for the owner.

## D-027 · 2026-11-21 · Files / Functions / Notes are switches in the pane's head

**Decision.** The left pane's three faces are chosen by switches in its head, and the face comes
back at start.
**Why.** It is where other editors put this control, and it ended two folder icons that meant
different things.
**Overturned by.** The owner after seeing it; a dropdown or bar buttons, a day.
**From.** The library redesign, section 9, decision 7 and the third correction. Built in phase 1.

## D-028 · 2026-11-21 · Where the library lives is a setting

**Decision.** `library_dir`. Empty means `library/` beside `settings.ini`; a relative path is
taken from the settings folder.
**Why.** A fixed place is wrong for a folder kept in OneDrive or beside the notes themselves.
**Overturned by.** Nothing foreseen.
**From.** The library redesign, section 9, the second correction. Built in phase 2.

## D-029 · 2026-11-21 · A folder per group, and per section

**Decision.** New notes go into a folder of their group, and a folder of their section inside
it. It is the default, not an option. A `home:` line under the group's heading names the folder;
nothing that exists is moved; a group can be pointed at a folder the owner already has.
**Why.** The owner, after using phases 1 and 2: "I still don't like the idea of having all files
in one folder."
**Overturned by.** The owner. **Replaces D-022.**
**From.** The library redesign, section 10.1.

## D-030 · 2026-11-21 · A model that splits a document returns line ranges, never text

**Decision.** One document and one instruction become many notes. The model answers with line
ranges and titles; the notes are made of the document's own lines; a ticked plan is shown before
anything is written.
**Why.** The owner: "I want ai to be able to also take one document and split organize as I ask
it to. a long single document can be a library on its own with one prompt with many files."
Ranges, so that nothing a model invents can reach a file.
**Overturned by.** Nothing foreseen for the ranges; the rest is the owner's.
**From.** The library redesign, section 10.2.

## D-031 · 2026-11-21 · A library file may be deleted from the pane

**Decision.** "Delete this library file…" on a group's row deletes that library file, after a
question. It deletes only a regular `.md` directly in the library folder.
**Why.** The owner found there was no way to delete one. D-021 still holds for the owner's own
documents; a library file is the app's own index.
**Overturned by.** The owner. **Narrows D-021.** Its guard is not enough as built: see
`docs/STATE.md`, "Known to be wrong", and section 5.0 of `docs/ai-library-plan.md`.
**From.** The old handoff, "Phase 3" (`8522c82`).

---

## D-032 to D-057 · 2026-11-21 · The library and AI plan

Taken for the owner in `docs/ai-library-plan.md`, section 11, in the same order (D-032 is its
decision 1). **All are waiting.** The plan has the full reasoning for each.

| # | Decision | Why | What would overturn it |
|---|---|---|---|
| D-032 | An AI run creates files only inside the library folder or a group folder the owner set, and changes or deletes nothing elsewhere. Replaces decision 4 of the redesign's section 9 (a converted file beside its original) | With a folder per group there is a right place for a new note, and one sentence covers every feature | The owner wanting results beside their sources |
| D-033 | The kind of a file task is chosen by the owner (file, split, rewrite), never by the model | What gets written must not be the model's choice | Nothing I can think of |
| D-034 | A task that changes text, run on files that are not open, writes new notes and never the files themselves | A bad result over many files with no undo | A real need for in-place fixes of many files, with backups |
| D-035 | Splitting and adding a folder each open with a plan made without a request; the AI is a second step on the same screen | Works with the AI off, instant, a baseline, and the machinery is proven on a deterministic input first | The no-AI plans proving useless in practice |
| D-036 | A note that starts with a heading has its headings promoted so that one is level 1; nothing is added and nothing is stripped. A note with no heading gets `# title` and a blank line | The mockup's duplicate heading; a note should be a document | The owner preferring lines exactly as they were (then: no change at all, and no title line) |
| D-037 | "The rest" is one note of labelled runs, off by default, defined as every non-blank line outside a ticked note | Makes the Recycle Bin rule reachable without a dozen scrap notes | The owner wanting a note per run |
| D-038 | Lines in no note are drawn plainly; the warning colour appears only when it has a consequence | Gaps are the normal case | The owner missing gaps they cared about |
| D-039 | Notes are written as UTF-8 whatever the source's encoding | The library's reader reads UTF-8 only | The reader learning other encodings |
| D-040 | A note's file is named by its title as written (the rule "New note here…" uses), not in lower case with hyphens | One naming rule in the program | The owner preferring the mockup's names, everywhere |
| D-041 | `home: <path>` exactly, relative to the library folder by default; a group without one gets it with the first note the app creates there | Run: the other spellings are read as notes. Relative keeps a synced library working | A reader that takes other forms safely |
| D-042 | Links to notes inside the library folder are relative to it | The library-folder setting exists so the folder can move | Nothing |
| D-043 | Records live in `.runs` inside the library folder, hold no note text, and are kept for 20 runs or 90 days | Undo must travel with the library it describes, and must not be a second copy of your notes | The owner not wanting them synced |
| D-044 | Undo never removes a note while a recycled original is not back, and removes only files that are byte for byte what the run wrote | Undo must not be able to lose text | Nothing |
| D-045 | The first cut of undo after a recycle asks the owner to restore the original; the program does not pick it out of the bin | Rare, and easy to get wrong | It being needed often |
| D-046 | The Recycle Bin uses the `trash` crate, without its default features, behind one function in `mdshared-ui` under a cargo feature, offered only on a local fixed drive | One new line in the lock against three back ends of our own; the feature keeps it out of mdview | The owner saying no to the dependency (question 1) |
| D-047 | A partial rewrite is composed in one pass and applied as one edit; `merge::Splice` per change is not used for it | One undo step, and the text is needed anyway | Nothing |
| D-048 | The Preview view ships with the ticks in its cheap form; the tinted, level one waits | Three days for something the Changes view may make unnecessary | The owner asking for it after using the ticks |
| D-049 | Pictures are placed by their names; the model does not see them. Seeing them is a separate phase | The engine has no attachments, and sending pictures changes what leaves the machine | The owner saying yes to phase V |
| D-050 | A picture is put into a note only if the run writes that note; otherwise it is listed in the library beside it | The owner's own files are not edited by a run | Nothing |
| D-051 | Pictures are copied beside the notes by default; the originals are never moved | A library that travels should carry its pictures | Disk use; the box is there to untick |
| D-052 | One request at a time; a model run restarts and never resumes from disk; a write resumes | Rate limits; no digests on disk; a half-written library must be finishable | A provider where parallel requests are free and wanted |
| D-053 | Esc in the review and on a plan closes and keeps; only Discard and Drop throw away | A result that took minutes must not be lost to a key | Nothing |
| D-054 | The format, the folder rule, the leaves and the list arithmetic start in the shared crates; the plan card is shaped in mdedit and moved on a date (phase S2); the split checker, the record, the change list and the strip stay in mdedit | The owner's rule of 2026-11-13, "whatever can be shared will start shared", applied to everything but mdedit's own policy and the pieces nobody else could use (7.1) | The owner wanting those shared as well; a second app that compares texts or splits documents |
| D-055 | The engine's protocol stays free of task words; the JSON contracts live in `mdedit-core` | Keeps the engine usable by anything | Nothing |
| D-056 | mdview gets no AI and nothing that writes the owner's documents | It is a reader, and its network sentence is worth keeping | The owner deciding otherwise, knowingly |
| D-057 | Drag and drop stays out | As the redesign decided; nothing here changes the reasons | A drag layer existing for another reason |

## D-058 · 2026-11-21 · How these documents are kept

**Decision.** `CLAUDE.md` is the entry and is always true. `docs/STATE.md` is the state of the
work and is rewritten, never appended to. This file is appended to, never edited. `docs/PLAN.md`,
`docs/CODE.md`, `docs/DESIGN.md`, `docs/CI.md` and `docs/RELEASE.md` are reference and change
when what they describe changes. `docs/release-log.md` gains a section per release.
`docs/archive/` is history and is not read for the current state. No dated story of a session is
written anywhere: `git log` is that.
**Why.** The owner: "have it rewrite and organize docs around this so that future sessions makes
no mistakes." The handoff had grown to 3,895 lines with four "START HERE" headings; `CLAUDE.md`
said there was no CI when there was; one document needed a section that overrode four others.
**Overturned by.** The owner. The list of what was moved, rewritten and removed is
`docs/REORG.md`.
**From.** The reorganisation of 2026-11-21. Waiting for the owner to look at it.

## D-059 — 2026-11-21 — Fable reviews the code

**Decision.** Fable is the sole reviewer of mdedit's code. A review is run after a major merge,
against `docs/review-prompts/audit3-scope.md`, which is kept up to date as work lands: a section
for each merge since the last review, naming its files, its tests and what to try to break.
**Why.** Audit 1 was Astra's, because the owner's Fable limits were spent; audit 2, by Fable,
found faults with a test for each, and those fixes have not been reviewed since. One reviewer
with one brief keeps a single record of what has been read, and a scope corrected as work lands
is never again out of date before the review starts.
**Overturned by.** The owner naming another reviewer; Fable's limits making a review impossible.
**From.** The reviews of 2026-11-21 ("Reviews: who audited what" in the old handoff) and the
audit-3 scope of `df562e8`, which covers everything merged since 1.2.1.

## D-060 - 2026-11-21 - The plan screen: what 5.3 left open

**Decision.** (1) The disabled primary button on the plan screen is drawn as
`replace_ui::dialog_button` draws a disabled one (radius 6, the plain field fill and line, the
faint label), not as `overlay::draw_dialog_button_disabled` does (radius 5, a 35 % accent fill).
Two screens, one look. (2) The options row has no height until something is put in it: it holds
the Recycle Bin box (phase T) and the pictures box (5.6), neither of which exists. (3) The
coverage strip gives every note at least 2 px, ticked or not, so that a tick changes the pixels
of its own note and no other; a stretch of blank lines between two notes is not a gap, it joins
the note before it. (4) The notes a folder already holds with the same bytes come unticked, and
the same names with other bytes are numbered (3.9); "the same" is length and FNV-1a, read from
disk at most 8 MiB a file. (5) The model gained `Plan::has_text`.
**Why.** (1) `draw_dialog_button_disabled` is to be made to match mdedit later, and a screen
drawn with it now would differ from Replace until then. (3) A strip that moved when a note was
ticked would make a tick look like an edit of the document; and the blank lines each note was
trimmed of are not something to read as left out.
**Overturned by.** The owner, looking at the shots; the shared button once it matches.
**From.** The plan-screen task of 2026-11-21.