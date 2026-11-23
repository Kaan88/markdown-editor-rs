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

Answered by the owner 2026-11-22. Confirmed: D-032, D-033, D-035 to D-039, D-041, D-042, D-044
(in its widened form, D-069 to D-072), D-045, D-046 (the `trash` crate is approved), D-047, D-051,
D-053, D-055, D-056 ("keep it out for now": mdview will have libraries, so it may come back).
Replaced: D-034 by D-074, D-040 by D-075, D-043 by D-076, D-048 by D-077, D-049 by D-078, D-050 by
D-079, D-052 by D-080, D-054 by D-081, D-057 by D-082.

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

## D-061 - 2026-11-21 - The write, the record and undo

**Decision.** (1) The record is `<library>/.runs/<UTC time>-split.json` (colons written as
hyphens, since Windows forbids them in a name) with `<same name>.done` beside it, and its text is
written and read by a small reader and writer in `mdedit_core::runs` with a depth bound, so that no
dependency is added and no file can end the process. It adds two keys to 4.4's example: the
group's `name` (the file's stem is not always the name) and `sections_added`, the section paths
the run wrote into the library file, so undo can take a heading out when nothing is under it.
(2) A note's text and the same note recomposed from the source's lines are both kept in the plan
and compared at 4.3's step 5; a writer that wrote anything else is caught there. (3) A note whose
name is taken is offered as "already there" only when the bytes are identical (3.9), and
otherwise numbered before the write, so `create_new` never has to refuse a name the plan chose
unless someone wrote to it in between. (4) Library file limits are checked before the write: the
group's file is refused at 64 library files, and its links at `MAX_ENTRIES` (2,000). (5) Undo's
question counts notes, pictures, lines and folders separately, in the words of 5.9, and a refusal
(the source not back from the Recycle Bin, or a run already undone) opens the same dialog with
Close as its only button. (6) A demonstration of the plan screen is refused with "A demonstration
of the plan screen: it writes nothing." rather than by a flag somewhere else.
**Why.** (1) 4.4 names the folder and the shape; the two keys are what this program needs to undo
a heading it wrote and to name a group whose file was renamed. (2) 4.3 step 5 asks for a
comparison with the lines it was made from, which is not the bytes written; both are in the plan,
so the check can fail. (5) A question with an Undo button that must not be pressed is a trap;
4.5's refusal is a sentence, and the dialog is where a sentence is read.
**Overturned by.** The owner, looking at the shots; 4.4 adopting the two keys; phase T, which
changes what undo does to a file (the Recycle Bin) but not which files it may touch.
**From.** The writer task of 2026-11-21 (`ai-library-plan.md` 4.1, 4.3, 4.4, 4.5, 5.9, 6.2, 6.3).

## D-062 - 2026-11-21 - The notes pane's status row

**Decision.** The "Plan ready: N notes · Show · Drop" row is one `pane::ROW_H` high at the foot
of the Notes pane, drawn by `library_pane::draw` from the plan `App` parked, and its state (the
count, and whether a plan is kept) is part of the Files region's signature in `region_sig`.
**Why.** 5.3 and 5.10 put it there; a drawn region that is not in the signature is never
repainted, and the row is the only sign a parked plan exists.
**Overturned by.** 5.10's fuller set of states, when a run can be going.
**From.** The writer task of 2026-11-21.
## D-063 - 2026-11-21 - Over 30 MiB is accepted on a library of thousands of notes

**Decision.** The private-memory budget of 30 MiB stands for the app as it is normally used, and
a library of 2,000 notes held open in the pane may cross it: measured at 30.4 MiB on `main`
against 29.4 at phase 1 (`docs/measurements/2026-11-21-since-phase-1.md`). The owner, told the
figure and its cause: "ram increase seems fine, with so many features added." The same message
left the idle-CPU figure open rather than accepted: about 0.4 % of a core, 2.25 ms of stat work
roughly once a second, and only while the Notes pane shows a library of that size.
**What is still done anyway.** The extra megabyte is phase 0 (`f00ee60`) keeping a second,
unfiltered `Library` beside the filtered one only to answer "has anything changed". That is waste,
not a trade, and it goes: the staleness check keeps the file list and the stamps it compares, not
a second copy of every group and note. The budget being forgiven does not make a redundant copy
worth keeping.
**Why.** A budget exists to stop the app growing heavy for everyone, not to forbid a large
library from costing what a large library costs. The figure is reported, not hidden, and it is
the owner's to forgive.
**Overturned by.** The figure growing on a library of ordinary size, where no feature explains
it; or idle CPU proving to cost battery on a laptop, which nobody has measured.
**From.** The measurement pass of 2026-11-21 and the owner's answer to it.

## D-064 - 2026-11-21 - Undo confines what it removes, and a run whose lines stay is not undone

**Decision.** Undo removes or writes nothing a record names until that path has passed the same
`inside` check the write makes: the library folder, or the group's folder, in one function used by
both (`inside_any` is that function for a request). The check is made once in `prepare`, before
anything is planned for removal, and a record naming a path outside those two folders is refused
whole, the way the Recycle Bin refusal is: the sentence names the path, and Close is the dialog's
only button. Separately, a run whose links could not be taken out of a library file — the ordinary
case of that file being open with unsaved changes — keeps the notes those lines name, and its
record goes to `links pending` instead of `undone`, so Undo is offered again and finishes the job
once the file can be written.
**Why.** A record is a file in `.runs` like any other, and undo deletes and writes what it names.
One that names `C:\...` or `D:\notes\work\...` was not written by this program, whose writes refuse
those paths (4.1, 6.2, 6.3), so none of its paths can be trusted, and a partial undo would be the
half-act 4.3 forbids. A per-row "kept" was rejected for that reason: it would still act on the rows
that happen to look sound in a record already known to be wrong. On the second fault, 4.5.2 exists
so that the pane never shows a link to a file that is gone: a line that stays in the file means its
note stays with it. `links pending` is already 4.4's state for "the files are dealt with but a
library file would not take its lines", and it is live, so "AI runs…" keeps "Undo this run…" where
the owner left it and the panel says "The run is not undone" with the reason.
**Overturned by.** A record that can prove its own origin — the group's folder read back from the
group's library file, which is inside the library — would let undo trust a path outside the library,
which today it must not: the group's folder is still the record's own word, as it is the owner's
word when a plan is written. Phase T's Recycle Bin would make a wrong removal recoverable, but not
the reason for the check.
**From.** The review of 2026-11-21 and faults F1 and F2.

## D-065 - 2026-11-21 - Visual editing belongs to the window, not to the one file

**Decision.** Visual editing (`Ctrl+E`) stays on when another tab is made active, and follows it:
the page shows the new document, no block is open, and it starts at that document's caret once
the tab's text has reached the page. A tab the page cannot show (not Markdown) ends the mode, and
the preview pane goes back to what it was before the mode: closed if the mode opened it, open if
the owner had it open. Showing a Markdown tab again never turns the mode on.
**Why.** The mode is a way of looking at the window - the page takes the text area - and its
pane and its commands belong to the window; a tab switch is navigation, not a request to leave
it. The owner met the other reading as a fault: switching away dropped them into side-by-side
text and page, and switching back left them there. D-017 keeps the mode's editing rule (one block
at a time); this only says whose the mode is. A pane the owner opened before `Ctrl+E` is theirs
to keep open after it, and a pane the mode opened is not left behind.
**Overturned by.** A window that edits two documents on the page at once (D-017's rule would
need a block per document), or a pane that can show two documents.
**From.** The owner's report of 2026-11-21, and the fix on `fix/visual-follow`.

## D-066 - 2026-11-21 - A write is checked before it writes, and its group's file is made first

**Decision.** "Write the notes" refuses before the first byte when anything that is known before
the write would refuse it: the group's file's name is free, the library's file cap and the group's
note cap are clear, no library file involved has unsaved changes, the group's folder is not the
library folder, the source is still the file the plan was made from (its length and SHA-256), and
a dry run of `runs::with_links_added` and `runs::check_links_added` against the group's file as it
reads now passes. The same check is what disables the button and what the card says, worked out
again when the destination changes and when a kept plan is shown. Once it passes, the group's file
is made with `create_new` before the notes; a run left "links pending" still owns it, and undo
takes an empty one away. What step 6 can still fail on is a race.

**Why.** 4.3's step 6 held two things, one a precondition (the group's file), and the other its
links; a refusal there left every note on disk and nothing listing it (audit 3's C1, C2, C3). Every
prefix of D2's order is a state one can live with, and an empty group is one of them.

**Overturned by.** The shared writer gaining a variant that places a note with no section before
the first heading, which would let C1's case be written instead of refused; or a check that needs
something only a written file can tell.

**From.** Audit 3 (`docs/review-prompts/audit3-report.md`) D2, C1, C2, C3 and 4.3.

## D-067 - 2026-11-21 - No session, no library

**Decision.** A window with no settings file has no library folder, no snippets file and no saved
prompts: `snippets_path` returns `None` unless there is a session or a settings file named by
`MDEDIT_SETTINGS`, and `library_dir` is built on it. `--no-session` therefore gives a build nothing
of the owner's to read or write, and a build that wants a library gives `MDEDIT_SETTINGS` naming a
scratch `settings.ini`, whose folder's `library` is used. A plan is refused when there is no
library folder, and the empty path is never a library root the confinement check can pass.

**Why.** CLAUDE.md rule 9 offers `--no-session` as the safe way to start a build, and until now such
a build read and wrote the owner's real library (`%LOCALAPPDATA%\mdedit\library`); a window whose
settings could not be had wrote into the default folder too. A build that can write dozens of the
owner's files is not safe, whatever flag it was given.

**Overturned by.** A separate, explicit `--library` argument that names a folder for a build.

**From.** Audit 3's A11 and A6, and rule 9.

## D-068 - 2026-11-21 - A destination carries its group's file

**Decision.** `plan_ui::Dest` holds the group's library file beside its name and folder, and the
write, the blocked check and the group menu all use that file. Nothing looks a group up by name
again; two titles that differ only in case are compared with `mdshared_app::paths::same_path`, as
the disk compares them.

**Why.** A library may hold two groups of one name; the screen matched them by name three ways
(ignoring case, exactly, and first match), so the notes went to one group's folder and the links to
another's file (C7). `HashSet<PathBuf>` told `Notes.md` from `notes.md`, which Windows does not, so
the second note was offered no other name and was left out (C5).

**Overturned by.** The library gaining a rule that a group's name is unique; it has none.

**From.** Audit 3's C5 and C7.

## D-069 - 2026-11-21 - The record holds what happened, not what was planned

**Decision.** `created` is a field of each note and each picture in a record and of the group, set
only after the run made the file (`create_new` returned Ok; `create_group` returned Ok), and
`folders` lists only the folders that did not exist when the run began, each parent it made
included, parents first. The record is written at step 1 with `created` false and no folder, and
again after the notes are written and again when the group's file is made, so a run that stops
part-way can be undone and undo knows its own files. A record with no `created` says false: it
removes no file. Undo touches a file, a folder or a group's library file only when the record says
the run made it; a note that was there already with the same bytes (3.9) is linked, counted, and
never removed, and its line comes out, and the question says "1 file was there before this run and
is kept." This replaces the sentence of D-061 (1) and plan 4.3 step 1 that the record holds "the
whole plan", and the list of `record.folders` that was every folder a note uses.
**Why.** Audit 3, A4 and D4. The record was made at step 1 from the plan, so a note that was
already there (counted as written by 3.9), a group file `create_group` refused to make, and an
empty folder that existed before were all deleted by undo, and the folders made on the way
(`Trip/Japan` for a note in `Trip/Japan/Tokyo`) were left behind. "Only files this run created"
was true of the write and not of the record undo reads.
**Overturned by.** A record format that stores the operations as they were done, one by one, in
which `created` would be implied; the phases that add operations (a text note, a recycle) will
need the same field on their own rows.
**From.** Audit 3 (`docs/review-prompts/audit3-report.md`) A4, D4.

## D-070 - 2026-11-21 - A note is removed only if no library file lists it any more

**Decision.** Undo is two steps and one rule. After the links step, the library is read again
from its files (every `.md` directly in the library folder, not only the 512 the pane keeps, a
file over the reader's cap as the reader takes it) and **a note is removed only if no library file
lists it**: the owner's edited or moved line, a line in another library file, a line the links
step could not take out all keep their note. The question says so beforehand (`held`, "1 line was
edited or moved since and is kept, with its note."), and `undo` checks again after the links step,
so what changes in between is caught. The links and the headings come out in one text,
`runs::with_removals`, used by the window's gate and the tests' gate alike, and a gate whose write
changes nothing says an error, not success; `Wrote::Nothing` is no longer `Ok`. A library that
could not be read in full (more than 4,096 library files, or one that cannot be read) is no basis
for removing a note: undo is refused, and the backstop holds every note. Finish writes the line
the record holds, not the absolute path, so what it adds is what undo looks for. This replaces
plan 4.5's two rules for lines and for files, which were independent, and the sentence of 4.5.2
that a line the owner edited "is kept" with no word about its note.
**Why.** Audit 3, A2, A5, D1, D6. The window's gate threw away the text with the links taken out
whenever no heading went with them, and read "nothing written" as success, so undo deleted every
note and left every line; the core's test gate composed the two removals correctly, so the tests
exercised a gate the program does not use. A line the owner edited was kept and its note was
deleted, by the plan's own two rules, which together produce the thing 4.5.2 exists to prevent.
Whatever causes a line to stay, the rule makes the note stay too.
**Overturned by.** A way for the library to say which notes it lists that costs less than reading
every library file (an index), which would still have to be asked after the links step and never
trusted from before it.
**From.** Audit 3 A2, A5, D1, D6.

## D-071 - 2026-11-21 - Undo is refused when the original is not the file the notes were cut from

**Decision.** Undo compares the source with the recorded length and SHA-256 always, not only for
a run that recycled it. Gone, changed, or too large to compare (a source over 64 MiB), undo does
not run: "'Japan 2025.md', the file these notes were cut from, is not where it was, so these notes
may be the only copy of its text. Undo will not remove them. Put the original back, or delete the
notes yourself if you do not need them." Close is the dialog's only button, as for the Recycle
Bin case. A second question that lets the owner go on was considered and not built. Undo's
removals stay `remove_file`; the Recycle Bin for them waits. This replaces the sentence of plan
4.5 rule 1 and decision 13 of the plan that read "while a recycled original is not back", and
D-061 (5) in so far as the refusal is for a source that was only deleted by hand.
**Why.** Audit 3, A3, D3. "Undo can never be the thing that loses the text" was written for an
original the app itself recycled, and nothing sets `recycled` before phase T; an owner who deletes
the document by hand after a split (what they do today) then presses Undo deleted the only copy
behind a question that did not mention it. With a refusal, the sentence tells them exactly what
holds. A second question needs the deletion to be recoverable to be a fair thing to offer, which
is the Recycle Bin: the plan (decision 15) puts the `trash` crate behind one function in
`mdshared-ui`, under a cargo feature so that mdview never builds it, and the shared crates are not
changed by this work, so the crate, the second question and "Undo's removals go to the bin" are
one separate task, together with phase T's recycle of the original.
**Overturned by.** The Recycle Bin for undo's removals being built: then "gone" and "changed" can
become a second question ("These notes are the only copy. Undo anyway?") because the answer is
recoverable. The owner wanting Undo for a run whose original was edited afterwards, which today is
refused; the second question is the answer to that.
**From.** Audit 3 A3, D3.

## D-072 - 2026-11-21 - A record is a file format: it reaches no further than the library folder does

**Decision.** A record must never give more power than write access to the library folder already
gives. Before anything is planned, every path a record names is checked: no `..` (a run never
writes one, and folding it by text crosses links the disk has not been asked about); inside the
library folder, not the folder itself, or inside the home the group's library file declares now,
which is read from that file when undo or Finish is asked and not from the record; the group's
file a `.md` directly in the library folder (it is written, so it is confined too); a note a
`.md`, a picture an image extension a run copies. A path that fails refuses the whole record, and
the sentence names it. An empty library folder, or one that is not absolute, is no folder at all
(and `inside` is false for it, in the write as well). Only regular files are removed
(`symlink_metadata`), only folders that are real folders and empty, and a file undo reads of a
record is read through a 64 MiB cap. A record whose `version` is past 1, or whose kind or state
this version has no word for, is not loaded and is shown as "Written by a newer version of
mdedit", nothing in the row to press; a file that is no record is shown as such; `prune` counts
and deletes only records that load, and never the others. This replaces D-064's check "the library
folder, or the group's folder" where the group's folder was the record's own word, which its
**Overturned by** paragraph named; the group's folder is now what the group's library file says
now. It replaces plan 4.4's sentence that older records are deleted without saying which.
**Why.** Audit 3 A9, A10, B2, D5. The first fix (D-064) made undo refuse a path outside the
library and the group's folder, but the group's folder came from the same record, a relative path
was joined as it stood, the group's file (which undo writes) was not confined, and nothing said
what a deletion may be of. `prune` took the newest twenty by a time that an unreadable record
does not have (0), so a finished run deleted a later version's records, newest first, with their
undo; the next phases add kinds and it can only be fixed in the version that ships first.
**Overturned by.** A signed record, or a record that proves its own origin better than being
inside a folder the owner controls; or a record format that needs a path outside the library (a
home elsewhere with its group file gone), which today cannot be undone from here and says why.
**From.** Audit 3 A9, A10, B2, D5.

## D-073 · 2026-11-22 · A note with no section goes in the group's own notes, as a variant

**Decision.** A note with no heading path is written into a library file after the group's `#`
heading and its `home:` line, after the notes already there, and before the first section heading.
In `markdown-shared-rs` these are `library::add_note_in_group` and `library::move_note_in_group`,
new functions beside `add_note` and `move_note`; `add_note` and `move_note` are unchanged, byte for
byte, because mdview pins the shared crates and rule 8 says a shared change must not change what an
existing caller writes. Every mdedit writer that can place a note with no section uses the variant:
the run's links (`runs::with_links_added`), "Add this file" (`library_put`), "New note here…"
(`library_new_note`) and "Move to…" (`library_move`). A file with no section heading takes the note
at the end exactly as `add_note` writes it.
**Why.** Audit 3 C1: `add_note` appended the note at the end of the file, which is under the last
heading, so it read back with a section and the check refused it before the write; the note of "the
rest" in a split always has no section, so a second split into a group with sections could not be
written at all, and the pane's group row had the same fault.
**Overturned by.** A group file format that gives the group's own notes a heading of their own, or
the owner deciding the old placement was right.
**From.** Audit 3 C1, `docs/review-prompts/audit3-report.md`.

## D-074 · 2026-11-22 · Rewriting files that are not open edits them in place, with the original kept for undo

**Decision.** "Rewrite them" in the AI task window overwrites each file in place, and only after
the owner has gone through that file's changes and accepted them. Before a file is overwritten its
original is written into the run's record in `.runs`, so the run's undo puts it back. Replaces
D-034 (new notes only, never the files themselves).
**Why.** The owner: new notes only "would limit ai too much", and would never do anything with an
existing library: rewriting notes already in a library left a second copy of each.
**Overturned by.** An in-place rewrite losing text despite the review and the undo copy.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-075 · 2026-11-22 · Note files are named in lower case with hyphens, everywhere

**Decision.** A note's file name is its title in lower case, words joined by hyphens
(`pasta-notes.md`), the mockup's names. It is one rule for the whole program: the split writer,
"New note here…" and every other place that names a new note. Replaces D-040 (the title as
written). Existing files are not renamed.
**Why.** The owner chose the mockup's names; D-040's own reason, one naming rule, holds if the rule
is applied everywhere.
**Overturned by.** The owner.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-076 · 2026-11-22 · A record may hold the originals of the files a run rewrote

**Decision.** Records stay in `.runs` inside the library folder and are kept for 20 runs or 90
days, as D-043 said, but a record now also holds the original text of every file the run
overwrote (D-074), and of no other file. Replaces D-043's "hold no note text".
**Why.** Undo of an in-place rewrite needs the original, and undo must travel with the library it
describes. The owner accepted that the copies sync with the library.
**Overturned by.** The owner not wanting the copies synced.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-077 · 2026-11-22 · The tinted, level preview is built with the review

**Decision.** Phase P2, the tinted and level Preview view of the mockup (3 days), moves out of
"waits for a yes" and is built with phase R2. Replaces D-048.
**Why.** The owner asked for it now.
**Overturned by.** The owner.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-078 · 2026-11-22 · Phase V, pictures the model can see, is planned

**Decision.** Phase V goes into the plan: the model may be sent pictures, at the owner's choice,
which changes what leaves the machine and is said so where it is chosen. Replaces D-049. The
resizer it needs is a new dependency and is still asked for and priced under rule 7 before it is
added.
**Why.** The owner said yes to phase V.
**Overturned by.** The owner.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-079 · 2026-11-22 · A run may add pictures to existing notes, behind a setting

**Decision.** A run may put a picture link into a note it did not write, only when an AI setting
for it is turned on (off by default), only after the owner has seen that change in the review, and
with the note's original kept in the record as D-074 keeps it. With the setting off, D-050's rule
holds: the picture is listed in the library beside the note. Replaces D-050.
**Why.** The owner: "could be useful to add just pictures via ai to library. behind an ai security
setting maybe", and left the rule to be decided. The setting, the review and the undo copy keep it
as safe as an in-place rewrite.
**Overturned by.** The owner.
**From.** The owner's answers to the plan's section 11, 2026-11-22; the rule decided by Claude.

## D-080 · 2026-11-22 · AI requests run in parallel, three at a time by default

**Decision.** A run may have several model requests in flight: a setting per connection, three by
default and one for a local model (Ollama, LM Studio). Results are still shown in the order of the
files. A model run that stops still restarts and never resumes from disk; a write still resumes.
Replaces D-052's "one request at a time".
**Why.** The owner chose parallel requests and a setting with a default of three.
**Overturned by.** Rate limits making it worse than one at a time in practice.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-081 · 2026-11-22 · All of the library and AI pieces start shared

**Decision.** Everything D-054 kept in mdedit starts in `markdown-shared-rs` too: the plan card
from the start (phase S2 goes away), the split checker, the record, the change list and the strip.
Only mdedit's own wiring stays in mdedit. Rule 8 holds: a shared change must not change what an
existing caller draws or does. Replaces D-054.
**Why.** The owner chose "share all of it"; it is the rule of 2026-11-13 applied without exceptions.
**Overturned by.** The owner.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-082 · 2026-11-22 · Drag and drop is planned

**Decision.** Drag and drop of notes and files in the library goes into the plan (3 to 4 days).
Replaces D-057 and the redesign's decision 6.
**Why.** The owner asked for it.
**Overturned by.** The owner.
**From.** The owner's answers to the plan's section 11, 2026-11-22.

## D-083 · 2026-11-22 · Only a new note's file is named in lower case with hyphens

**Decision.** D-075's rule applies where the program names a new note's file: "New note here…",
the notes a split writes, and the note of the rest. A group's library file, a folder, and a file
the owner renames keep the name as written, with only the library's rule for what a file system
takes (`folder_name`). The rule is `mdedit_core::split::note_slug`: lower case, each run of spaces
one hyphen; punctuation is left to `folder_name`. Moves to the shared crates with phase S3.
**Why.** D-075 speaks of notes. A group's file name is what the owner sees in Explorer beside its
folder, and a rename is the owner typing the name they want; changing it under them would surprise.
Kept in mdedit for now, against D-081, because the slug is one line and S3 moves the whole writer.
**Overturned by.** The owner wanting groups and renames slugged too.
**From.** Claude, building D-075 with the owner away (they asked that decisions be made and noted).

## D-084 · 2026-11-22 · A move refuses a library file it cannot rewrite, and undo of a move keeps to the library

**Decision.** Three rules for "Move the notes…" (`moves.rs`), from the review of its draft:
(1) a library file that is not UTF-8, which the reader still lists from (as cp1252, or UTF-16),
refuses the move by name before the first file moves, and stops a run that meets it, which puts
back what it moved; a file past the reader's 1 MiB lists nothing and is passed over, as the reader
does. (2) Undo of a move makes every refusal a write makes (a file or library file open with
unsaved changes, a library file that would not take its lines) before the first file goes back.
(3) A note the owner brought into the library from outside stays where the move put it when the
move is undone, listed as kept with the reason; undo writes nothing outside the library folder,
the group's folder, or the folder a library move left. D-072 holds for moves: a record's own
word that a file came from outside no longer lets undo write there.
**Why.** (1) and (2): without them notes moved and their links pointed at nothing, or undo moved
every file back and only then refused. (3): a record lives in a synced folder, and D-072 says it
gives no more power than write access to the library. Converting a file's encoding was the other
way for (1); a move is the wrong place to change how the owner's file is saved.
**Overturned by.** The owner wanting notes from outside put back by undo; then the place should
come from something other than the record (asked at undo, or confirmed by the owner).
**From.** Claude, with the owner away (they asked that decisions be made and noted); the review
of the move-notes draft, 2026-11-22.

## D-085 · 2026-11-22 · The move card: a click ticks, a tab follows its file, the demo refuses

**Decision.** Four choices in the window side of "Move the notes…" (DESIGN 75): a click anywhere
on a row toggles its tick, since the card has no preview to select for; a tab open on a moved file
follows it to its new path, after a move and after its undo (`Docs::follow_move`); Move is refused
while nothing is ticked or the plan has a `blocked` reason, and the core's refusal stays in the
card's footer; in `--move-demo` the Move button is drawn enabled and refuses when pressed, so the
shot shows the real button. The library move from Settings is not offered: the library-folder
row has no natural place for it.
**Why.** A tab left on a path that no longer exists would save a second copy where the note was.
The rest keep the card as simple as the plan card it imitates.
**Overturned by.** The owner, on looking at the shots `move-notes`, `move-notes-small`,
`move-results`.
**From.** The agent that built the window, recorded by Claude with the owner away.

## D-086 · 2026-11-22 · The paged settings form: how its keys and checks behave

**Decision.** The shared half of the settings redesign (`mdshared_ui::settings::pages`, shared
`9df8bc8`) is built from the approved design on `mock/settings` (466db6f), not from the first-round
copy of `docs/settings-redesign.md` on `main`. Its choices: the first Tab moves at once (the old
form's "first Tab only shows the ring" is dropped); Up and Down stop at a page's first and last
control; Ctrl+PageUp/PageDown stop at the first and last page; a list click acts on release; rows
have no hover fill, controls keep theirs; the fit check measures a dropdown by the option shown;
text buttons are 96 px and a longer label is reported by `fit`, not widened; disabled buttons use
the shared disabled style; page geometry is valid after the next layout or draw. The old
`settings::Form` is unchanged, proved by a frozen copy compared frame for frame.
**Why.** The smallest API that draws what the owner approved; wrapping keys surprised in the old
form, and a ring with no move cost a keystroke.
**Overturned by.** The owner, on using the paged form in either app.
**From.** The agent that built it, recorded by Claude with the owner away.

## D-087 · 2026-11-22 · mdedit's Settings on the paged form, kept on a branch

**Decision.** mdedit's half of the settings redesign is built on branch `f/settings-pages-mdedit`
(shared pinned at `7a6f013`) and is **not merged**: it changes a screen the owner uses every day,
so it waits for the owner's look. Nine pages, every old setting kept under its id. Choices taken
while building: the update button reads "Check now" or "Update" to fit 96 px; a detected program
path keeps its last 40 characters after "…"; the key's Save is live only with a key typed, Remove
only with one stored; the remove-provider icon shows only for a custom provider; the dialog reopens
on the page last shown this run; the old section names still work for `--settings-section`;
Provider and Name have no help line (today's lines do not fit at 520x360); with both tab rules off
the days row is dimmed without a reason line (4 px over at the minimum). **To do before merge:**
Model and Effort must stay dropdowns whatever their option count; today the shared rule draws two
options as a segmented "Default | Other…", so the control changes shape once a model list is
fetched. That wants a shared `Item::dropdown()` beside the old constructor.
**Why.** The mock's layout, kept at the minimum size and every scale; a control that changes shape
with its data reads as a different setting.
**Overturned by.** The owner, on looking at the `settings-*` shots or using the branch.
**From.** The agent that built it, recorded by Claude with the owner away.

## D-088 · 2026-11-22 · S3 done: the split checker and the plan card are shared

**Decision.** `mdshared_app::split` holds the checker; it reads the document through a small
trait, `split::Lines`, which mdedit-core implements for its `Buffer`, so ropey stays out of the
shared crates. A plan holds its document as an `Arc<dyn Lines>` rather than being generic over it.
`mdshared_ui::plan` holds the card (with `plan::draw` and `plan::demo`); it takes a key's name and
a menu-key flag instead of mdedit's `Key`. mdedit keeps the wiring, `headings_of` (it uses the
function list) and `--plan-demo`. The moved card tests find headings with their own fence-aware
scan. Shared `842ec3f`; all mdedit shots byte-identical but the clock.
**Why.** The smallest seam: one trait of nine methods, no new dependency, call sites unchanged
through re-exports.
**Overturned by.** A second app using the card in a way the trait or the key mapping does not fit.
**From.** The agent that moved it, recorded by Claude with the owner away.

## D-089 · 2026-11-22 · Split by AI: the contract is shared, parallel requests wait for phase W

**Decision.** Phase 4b is built (DESIGN 76). The split contract (the outline sent, the system
text, the prompt, the answer reader and checks 4 to 10 of 5.2) is `mdshared_app::split::answer`;
the `mdedit-ai` engine learns nothing of splits. Split prompts are their own list (`kind: split`);
the prompts template is unchanged. Titles are cut at 120 bytes. "At headings" recuts at the plan's
own level. **D-080's per-connection setting for parallel requests is not built in 4b**: a split of
one document makes one request, so nothing would use it; it is built with phase W, the first that
sends several. "Show what is sent" (3.10) is not built. Known faults left: the instruction panel
hides the plan instead of drawing over it, and a long header line is cut ("effort hi…").
**Why.** One request per document is what 5.2 specifies; a setting with no user is dead weight.
**Overturned by.** The owner, on looking at the `plan-split-ai*` shots or running it on a real
provider (never done).
**From.** The agent that built it, recorded by Claude with the owner away.

## D-090 · 2026-11-22 · The owner approves Settings, "Move the notes…" and Split by AI by their shots

**Decision.** The owner looked at the shots of three pieces and said "they look great": the new
Settings (32 `settings-*` shots, D-087), "Move the notes…" (`move-notes`, `move-notes-small`,
`move-results`, D-085) and Split by AI (12 `plan-split-ai*`, D-089). Settings is merged into
`main` with it. Still open: what a moved note's pictures and relative links do (5.12), and the
small faults D-089 lists.
**Why.** Rule 6: the owner approves the interface by looking at it.
**Overturned by.** The owner, on using any of the three in a real window.
**From.** The owner, 2026-11-22.

## D-091 · 2026-11-22 · A moved note carries its pictures; its other relative links are rewritten

**Decision.** "Move the notes…" carries what a note's relative links point at: a picture (or other
file) that only this note links to moves with it, to the same place relative to the note; a
target that other notes or files also use stays where it is, and the note's link to it is
rewritten to the new relative path. Undo puts the files and the link text back. Links that are
absolute, web addresses, or point at nothing are left as they are. Replaces the open question
left by D-084 and the review of the move.
**Why.** The owner chose it: nothing breaks, and a note and its pictures stay together.
**Overturned by.** The owner, on using it.
**From.** The owner, 2026-11-22, asked by Claude.

## D-092 · 2026-11-22 · The original in the Recycle Bin (phase T), as built

**Decision.** Phase T is built and merged (DESIGN 77): the box on the plan card with every reason
of 5.4, recycling as step 8 after every note is written, read back and listed and the source is
seen unchanged; undo refuses while the source is in the bin and offers "Open the Recycle Bin";
undo's own removals go to the bin, or are deleted where the drive has none. Choices: the box is
opt-in on the shared card (`Option`), so other callers draw as before; the owner's tick is kept
while the box is disabled; the one-click answer only ticks "The rest"; a system set to delete
instead of recycling counts as no bin; `trash` runs on a fresh thread because its COM start
panics on failure; on Linux a mount path that is not UTF-8 counts as no bin, because the crate
unwraps it; `trash` is pinned exactly (`=5.4.0`), one line in the lock, +30 KiB to the exe,
nothing at launch, never in mdview's build. Not built: D-071's "undo anyway", restoring from the
bin, and the "Only for one document" reason (nothing makes a plan over several files yet).
**Why.** 5.4 and D-046, kept to the letter where the crate allowed.
**Overturned by.** The owner on looking at the 18 new shots; a Linux bin that rejects entries
with no deletion date (the crate writes none without `jiff`; never tried).
**From.** The agent that built it, recorded by Claude.

## D-093 · 2026-11-22 · How a moved note carries its pictures (D-091), as built

**Decision.** Built in `mdshared_app::moves` (`carry`, `moves_links`, shared `be0192d`) and the
move card (DESIGN 74, 75). Only a file inside the note's own folder is carried, and never a `.md`
file. If any other file that might link to it cannot be read, or there are more than 10,000,
nothing is carried (the note still moves, its links rewritten). A wiki-link counts as a user by
file name. A note not saved as UTF-8, and a link to a folder, are not rewritten. On another drive
a rewritten link becomes an absolute path with forward slashes. Old builds cannot read a record
that carried files or rewrote links, so they cannot half-undo it. Not built: links from other
notes into a moved note, `<img src>` and wiki-links as links to rewrite.
**Why.** Carry only what is surely the note's own; when in doubt, rewrite rather than move.
**Overturned by.** The owner, on using it.
**From.** The agent that built it, recorded by Claude.

## D-094 · 2026-11-22 · mdview's Settings on the paged form, approved and merged

**Decision.** mdview's Settings is the shared paged form (mdview `29bf0ab`, its shared pin moved
from `cb0ffd3` to `be0192d` with all 118 of its shots byte-identical): Appearance, Reading (was
Content), Tabs, Updates. "Make mdview the default for Markdown files" became "Make mdview the
default" with the help line "For Markdown files.", to fit at 520x360. The dialog reopens on the
page last shown; a `--settings-section` switch was added for shots and takes the old group names.
The owner looked at the 20 shots and said to merge. This finishes the settings redesign.
**Why.** The design approved for both apps; rule 6.
**Overturned by.** The owner, on using it.
**From.** The owner, 2026-11-22; built by an agent, recorded by Claude.

## D-095 · 2026-11-22 · The Recycle Bin approved; issues #7 and #8 built on a branch for a look

**Decision.** The owner looked at the Recycle Bin's 24 shots (D-092) and said "all done". Before
phase R2, every part of GitHub issues #7 and #8 (from TheGed) is built on one branch,
`f/issues-7-8`, and merged only once the owner approves it: the Preview button's dropdown with a
default mode (#7); "New file" on the empty screen; plain-text paste on Ctrl+Shift+V, with
"Preview in mdview" moved to another shortcut; New, Save and AI Edit buttons on the command bar;
and "Paste rich text as Markdown", off by default, with no new dependency.
**Why.** The owner chose all four; CLAUDE.md lets a feature in only when the owner asks.
**Overturned by.** The owner, on looking at the branch.
**From.** The owner, 2026-11-22.

## D-096 · 2026-11-22 · Issues #7 and #8 approved and merged

**Decision.** The owner tried the build of `f/issues-7-8` ("preview build looked awesome") and it
is merged into `main` as built (DESIGN 78): the Preview button opens a dropdown and remembers a
default mode (`preview_mode`), and the preview entries left the View menu; "New file" on the
empty screen; Ctrl+Shift+V pastes plain text and "Preview in mdview" moved to Ctrl+Shift+L; New
and Save as icon buttons on the bar, and "AI edit" there only while the AI is on; "Paste rich
text as Markdown" (Settings › Typing, off by default) only in a Markdown or untitled tab, through
`mdshared_app::html_md`, which writes a table's rows as "a | b" lines, not a Markdown table.
**Why.** The owner's approval by looking, rule 6; the converter is shared so mdview may use it.
**Overturned by.** The owner, on using it.
**From.** The owner, 2026-11-22; built by agents, recorded by Claude.

## D-097 · 2026-11-22 · mdedit notices an mdview closed by hand

**Decision.** mdedit no longer keeps "Stop preview in mdview" lit after the owner closes mdview.
mdview now sends `?closed` (shared `Request::closed`) for every tab mdedit asked it to show, when
the tab is closed and when the window closes (waiting at most 500 ms). mdedit also checks that
its mdview is alive on its courier thread when its window gains focus and when a link menu opens,
and once more a second later; there is no idle timer. A menu open when a link ends is relisted,
or closed if nothing is left. `Menu::chosen` and `Menu::pointer` became bounds-safe. Shared pin
`09c67cd` in both apps.
**Why.** The owner's report: it never noticed, since the beginning. Rule 1 forbids CPU at idle,
so the checks ride on events.
**Overturned by.** The owner, if a closed mdview is still missed.
**From.** The owner's report, 2026-11-22; built by agents, recorded by Claude.

## D-098 · 2026-11-22 · What mdedit gives Hub: a version resource, its uninstall key, and hub.txt

**Decision.** For the owner's launcher and updater Hub (its `docs/APP-CHANGES.md`, mdedit section): `build.rs` embeds a VERSIONINFO
resource (file and product version from `Cargo.toml`) with the icon, no new dependency; `updater.rs` names the installer's entry with
its hive (`HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\{AppId}_is1`), so the shared `record_version` writes
`DisplayVersion` after the app's own update; the `publish` job writes `hub.txt` with `tools/write-hub-txt.ps1` (a copy of Hub's
script, first line naming Hub's commit) and uploads it as the sixteenth asset. `mdedit-update*.txt` stay: builds up to 1.2.1 read them.
Not done here: the `close` request (waits for a shared-crate change) and standing down when Hub manages the app (the owner's question 3).
**Why.** Hub detects by the exe's version resource first and the uninstall entry second, and checks `releases/latest/download/hub.txt`.
**Overturned by.** The owner, or Hub's own decisions D-004 and D-007 changing the format or the stand-down.
**From.** The owner, 2026-11-22 ("write pr's for installer requirements"); written by Claude.

## D-099 · 2026-11-23 · Hub's installer PRs merged; two Settings labels shortened for Linux

**Decision.** The owner said to merge the installer PRs: mdedit's `hub/installer-requirements`
(D-098, its D-096 renumbered) and mdview's, both into `main`. With them, the two Settings labels
that failed `ci` on Linux since the paged Settings (wider fonts: "needs 145 of 139", "needs 235 of
223") are shortened in both apps: the seconds under "Don't reopen tabs barely looked at" read
"Shown for less than" (each app's own label; the shared `brief::SECONDS` is left as it is, rule
8), and "Make mdedit the default for text files" became "Make mdedit the default" with the help
line "For text files.", as mdview's did (D-094). The palette's command keeps its long name.
**Why.** `ci` must be green on `main` before a release, and the owner asked for 1.3.0 to be made
ready first.
**Overturned by.** The owner, on the wording.
**From.** The owner, 2026-11-23 ("merge the installer related one", "merge the one for mdview
too"); the labels by Claude.

## D-100 · 2026-11-23 · mdview's Notes face, its "Add to the notes library", and the code written twice: yes

**Decision.** The owner said yes to the three items of `ai-library-plan.md` 7.4 and 7.5 that
waited for one: M1, a read-only Notes face in mdview's left pane (3 days); M2, "Add this file to
the notes library…" in mdview (1 day, after M1); D, the dialog skeleton, the crash bar's glue and
`frost.rs` moved into the shared crates (1.5 days). Not started: the owner then stopped all work
and asked for 1.3.0 to be made ready first. D waits for R2, whose dialog code it would cross.
**Why.** The owner's answer, 2026-11-23: "m1: yes, add it / m2: yes / d: do it."
**Overturned by.** The owner.
**From.** The owner, 2026-11-23.

## D-101 · 2026-11-23 · The owner's hand check: no blocker for 1.3.0

**Decision.** The owner tried `main` (`2e57cfa`) in a live window, on scratch settings and an empty
library, item by item from the checklist: Settings, the notes library, Split into notes and its
undo, the original in the Recycle Bin, the Preview dropdown, Ctrl+Shift+V and Ctrl+Shift+L, New
file and the bar buttons, Split by AI against a real provider, and rich paste all work. "Not
perfect but no blocker for release." Not checked: "Move the notes". Found, for the next release:
closing mdview by hand still leaves "Stop the preview" in mdedit (with the installed mdview 1.0.4,
so only the focus check could find it, and it did not: D-097 does not hold); "Add this file to
the notes library" adds the file but the tree shows it only after its group is folded and opened
again; browsing the library in the full preview keeps the preview for two or three files, then
falls back to editing; deleting a library file should say that its notes stay on disk. The
release waits only for the owner's word to tag.
**Why.** Rule 6: the owner approves by looking.
**Overturned by.** The owner.
**From.** The owner, 2026-11-23.

## D-102 · 2026-11-23 · A closed mdview is asked about more than once, and on input

**Decision.** Replaces D-097's "once more a second later". A check that finds the viewer running
is repeated after 1, 2 and 4 s (`RECHECKS`), and stops when the viewer is found gone. A key press
or a mouse press also asks (`Links::check_on_input`), at most once in 2 s and never while nothing
is linked or during the first `GRACE`; it is an event, not a timer, and costs one lock when not due.
**Why.** The owner's hand check found D-097 not holding with mdview 1.0.4. Against the real
1.0.4 exe (closed gracefully, with `--line` and `--from` as mdedit starts it) the old code did find
the viewer gone after 1 s, so the cause was not reproduced; the two changes cover the ways it can
still fail: a process that ends later than one second after its window, and a window that never
had a focus change to ask at.
**Overturned by.** The owner, if a closed mdview is still missed; then the next suspect is the
viewer staying alive after its window closes, which only mdview can fix.
**From.** The owner's report, 2026-11-23; built by an agent, recorded by Claude.

## D-107 · 2026-11-23 · The review is a list of changes with a tick each, composed in one pass

**Decision.** Phase R2 is built as `ai-library-plan.md` 5.5 describes: the review keeps the
comparison (`mdedit_core::hunks::Hunks`) and one tick per change, all ticked when it opens; Apply
puts in the text composed from the ticked changes through `ai_replace` (one edit, one undo step);
Copy result copies the same text; at zero ticked Apply reads "Nothing is ticked" and cannot be
pressed. `ai::review` and `ReviewLine` are removed; their test is kept as a test of the list's
layout. The list wraps long lines, has the shared scrollbar, tints the words that changed and dims
what is not taken. F7 and Shift+F7 step, Space ticks. The Preview's AFTER page shows the composed
text, made again 150 ms after the last tick. A change's mark on the "where" strip is its place in
the old text.
**Why.** D-077 and the plan's 5.5; the owner approved the look on `mock/ai-review`. All ticked at
the start, so that Enter does what it always did.
**Overturned by.** The owner, once they have used it.
**From.** The owner's request, 2026-11-23; the details by Claude from the plan.

## D-108 · 2026-11-23 · The tinted, level preview waits for the viewer's page rows

**Decision.** D-077 moved phase P2 (each changed block tinted and labelled in both rendered pages,
the two pages kept level) into R2. It is **not built** on `f/review-hunks`; the cheap form of the
plan ships instead (the AFTER page shows the ticked changes, headed "8 of 12 changes taken").
Amends D-077; nothing else of it changes.
**Why.** P2 needs the rectangle of every rendered block by source line in both panes. The viewer's
preview protocol (`mdview-preview`, pinned at `e2b3e31`) answers only `BlockAt` (a point to a
block's lines) and `BlockOfLine` (a line to its block's lines); it gives no rows, and the page's
layout is private to its server. A request that returns every top-level block's first line, top
and height, and an `Update` field for it, is a change to the viewer, which mdview also builds
against; by rule 8 and by the task's own rule it is not made from here. Drawing the pages in the
editor's process instead would need `mdview-core` as a direct dependency and would leave out the
mode that runs mdview in its own process.
**What would change it.** The owner's word to add that request to the viewer and move its pin;
then the tint pass and the block-to-block scroll (about three days, as the plan priced it).
**Overturned by.** The owner.
**From.** Claude, in the work of 2026-11-23; to be told to the owner.

## D-104 · 2026-11-23 · Visual editing waits for a tab still being read

**Decision.** One of the four faults of D-101: browsing the library in the full preview kept the
preview for two or three notes, then fell back to editing. A note opened from the library is a tab
still being read, and until the read ends its language is unknown, so `is_markdown()` is false and
`visual_sync` took it for a document the page cannot show. The mode now waits while the active tab
has no text and is loading: it neither follows it nor ends, and the pane stays. A read that fails
ends the mode, as a file the page cannot show does, and a block open on the document the page still
shows closes when the wait starts, so the old tab's block is not left open over the new one's text.
**Why.** The mode is the window's, and a note opened from the library is the document the mode
already shows until the read ends; ending there is the fault the owner saw. One read is cheaper
than guessing a language from the path, and a failed read is no text to show.
**Overturned by.** The owner; or a preview that can show a document whose language is not yet
known.
**From.** The owner's hand check, D-101; the block closing, from the review of the fix.

## D-103 · 2026-11-23 · The delete question: where the notes go, and a card that fits its words

**Decision.** The question before a library file is deleted tells the owner where the notes it
lists are ("The notes it lists are kept on disk."), and says nothing about them when it lists
none. Its card is no longer a fixed 460x196: the height comes from the body's own wrapped lines
(`overlays::body_height`, the same face, size and width it is drawn with), with 196 as the floor
— the height of the question that lists no notes, two lines; a question that lists notes takes a
third line and the card grows a few pixels. A long group or file name used to wrap a fourth line
through the divider into the buttons.
**Why.** The owner's hand check asked for the wording (D-101); deletion is unchanged either way.
The fixed card was found by review: the question names a group and a file, both free to be long.
A fixed height cannot hold a sentence that names things of unknown length, and sizing from the
measured body is the same rule the dialog list cards already follow (`card`'s `list`).
**Overturned by.** The owner, or a review that finds the grown card wrong on screen.
**From.** The owner's hand check (D-101) and the review of `fix/delete-library-notice`.

## D-105 · 2026-11-23 · The check made inside a grace is kept, and input does not restart the rechecks

**Decision.** Amends D-102, whose hedge the review found safe but incomplete. A check refused
because a link was still in its `GRACE` (a viewer it just started may not answer yet) was dropped,
and the focus that asked it comes once: it is now kept, and the courier waits on its own thread
until that grace ends and makes it once (`Courier::check_at`), so a viewer closed while a preview
was starting is still found. An ask made for input no longer restarts the 1, 2, 4 s rechecks
(`Check::Once` against `Check::Again`), so steady typing does not probe about once a second; only
key presses ask, not key releases or auto-repeat. D-102's "costs one lock when not due" is wrong:
when an input ask is not due nothing locks at all, and a refused ask costs one lock only while a
link is in its grace.
**Why.** The review of D-102's fix: the grace guard could swallow the one ask the owner's failing
case made, and an input ask resetting the chain made typing a steady stream of probes.
**Overturned by.** The owner, if a closed mdview is still missed; or a viewer that answers during
its grace, which would make waiting unnecessary.
**From.** The review of the fix, 2026-11-23; built by an agent, recorded by Claude.

## D-106 · 2026-11-23 · The review takes whole changes as the comparison made them

**Decision.** A tick takes a whole change of `diff::diff`, the comparison Compare uses; the review
does not re-cut the comparison's boundaries to look tidier. A line that appears on both sides can
therefore end up twice when the comparison pairs it with its neighbours: in
`fixtures/review-demo.md` the comparison ends the install paragraphs with a new "## Usage" and then
replaces the old "## Usage" with the two lines after it, so unticking the second of those two
changes (the `review-preview-partial` demo) shows "## Usage" twice in the AFTER page. Apply puts in
exactly what the list shows, and Ctrl+Z takes it back.
**Why.** D-107 keeps the comparison the user can check in Compare, and the plan's 5.5 ticks its
changes. A boundary pass (sliding each change while the line at its edge is equal on both sides)
would tidy this case but is a new behaviour for every text, with its own edge cases, and the owner,
who has not used the review yet, has not asked for it.
**What would change it.** The owner, after using it; then the pass belongs in `hunks::Hunks` alone
(never in `diff`, which Compare draws), with a test per case it re-cuts.
**Overturned by.** The owner.
**From.** Claude, finishing R2, 2026-11-23; seen in the `review-preview-partial` shot.
