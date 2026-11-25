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



**Decision.** Phase 5.8 in mdedit: a text file picked from a group's row becomes a new note in that
group's folder, written by the AI from the file's text. Its decisions, made as they were needed:
1. The card sends the whole file (capped at 200 KB, the editor's own loader, no NUL bytes and no
   byte it could not read) and one instruction, `ai::FORMAT`; the note is Markdown made from that
   text, and the record keeps the instruction and what was sent.
2. The note is always a new file, named from the source's file name (`pasta notes.txt` →
   `pasta-notes.md`, then `pasta-notes 2.md`), never a rename and never a replacement; the source
   is not touched. The note is UTF-8 whatever the source was, since Markdown is its text.
3. The run is `Kind::Markdown` in the shared record (mdshared-app), so "AI runs…" names it, undo
   removes the note and its line with the split's own machinery, and undo refuses when the original
   is no longer the file the note was made from (D-300's rule, kept whole).
4. Every refusal is made before the request and again before the write, in the card: AI off, no
   library folder, the group folder being the library folder, the group's file unsaved, the 2,000
   note cap, a line that would not read back, a source changed while the answer was out.
**Why.** The plan (5.8) asks for exactly this, and the app's rule is that a destructive path is
checked twice; the source is the owner's own file and the note is new, so nothing about it can be
lost. `Kind::Split` would make undo read line ranges that do not exist in an answer that is prose,
and "AI runs…" would call it a split.
**Overturned by.** The owner; a model answer that must be written into the source instead of a new
note; a record format that can hold a note's text without a kind of its own.
**From.** `docs/ai-library-plan.md` 5.8 and 5.10; the review that asked for a kind, not a reuse.
## D-120 ΓÇô 2026-11-23 ΓÇô A Markdown note from a file: the text, not the file, and always a new note

**Decision.** Phase 5.8 in mdedit: a text file picked from a group's row becomes a new note in that
group's folder, written by the AI from the file's text. Its decisions, made as they were needed:
1. The card sends the whole file (capped at 200 KB, the editor's own loader, no NUL bytes and no
   byte it could not read) and one instruction, `ai::FORMAT`; the note is Markdown made from that
   text, and the record keeps the instruction and what was sent.
2. The note is always a new file, named from the source's file name (`pasta notes.txt` ΓåÆ
   `pasta-notes.md`, then `pasta-notes 2.md`), never a rename and never a replacement; the source
   is not touched. The note is UTF-8 whatever the source was, since Markdown is its text.
3. The run is `Kind::Markdown` in the shared record (mdshared-app), so "AI runsΓÇª" names it, undo
   removes the note and its line with the split's own machinery, and undo refuses when the original
   is no longer the file the note was made from (D-300's rule, kept whole).
4. Every refusal is made before the request and again before the write, in the card: AI off, no
   library folder, the group folder being the library folder, the group's file unsaved, the 2,000
   note cap, a line that would not read back, a source changed while the answer was out.
**Why.** The plan (5.8) asks for exactly this, and the app's rule is that a destructive path is
checked twice; the source is the owner's own file and the note is new, so nothing about it can be
lost. `Kind::Split` would make undo read line ranges that do not exist in an answer that is prose,
and "AI runsΓÇª" would call it a split.
**Overturned by.** The owner; a model answer that must be written into the source instead of a new
note; a record format that can hold a note's text without a kind of its own.
**From.** `docs/ai-library-plan.md` 5.8 and 5.10; the review that asked for a kind, not a reuse.

## D-130 · 2026-11-23 · A dragged note moves its line, and only its line

**Decision.** In the notes pane, a note row dragged onto a group row or a section row is moved
there: its line is taken out of its library file and written under the heading it was dropped on,
by the writer "Move to…" already uses. Nothing else is dragged - not a group (a library file whose
order is its file name) and not a section (a block of lines, and a cross-file rewrite) - and no
file of the owner's is copied, moved, renamed or deleted. A plain click still opens the note;
Esc abandons a drag; a hold at the pane's edge scrolls the list. A file dropped from the file
manager is not taken by the pane: winit's Windows drop handler receives the drop point and
discards it (drop_handler.rs, _pt), so the window cannot know which heading was under the
pointer, and a guess would file the owner's file in the wrong place. Implements D-082 with this
scope.

**Why.** The design of record for drag and drop (the redesign's 3.4, archived) says "notes only,
never sections", and the library's rule is that the pane writes lines, never files. A drag is the
mouse path to what "Move to…" already does from the keyboard, so it adds no new kind of write.

**Overturned by.** The owner saying what a group or a section dropped on another group should do,
or wanting files from Explorer added to the library (which needs the drop point read from the
platform, not winit).

**From.** D-082; the archived redesign 3.4; phase DD built 2026-11-23.

## D-131 · 2026-11-23 · An import writes links in one library file and touches nothing else

**Decision.** "Add a folder to the library…" (5.7, phase 4a) is a plan of links, not an import of
files: the folder the owner picks becomes the group, each file it takes becomes one list line in
that group's library file, and no file of the owner's is copied, moved, renamed or deleted. The
files are listed where they are. A link inside the library folder is written relative to the
library, so the library can move as a whole; one outside it is written by its whole path.
**Why.** The library is a list of what the owner has, not a second copy of it: a copy would drift
from the file it was made from, and a move would break every other program that has the path. The
owner's rule for the whole redesign (`library-redesign.md` 9) is links only.
**Overturned by.** The owner; or a library that is meant to own the files it lists.
**From.** `docs/ai-library-plan.md` 5.7; the phase 4a brief.

## D-132 · 2026-11-23 · The group of an import is the folder, and an existing group is joined

**Decision.** The group's name is the folder's own name (a folder with no name is refused), and
its library file is named after it -- `library/Recipes.md` for `…/Recipes`. The group's `home:` is
the folder, relative to the library when the folder is inside it. When the library already has a
group of that name the plan joins it: its library file, its `home:` and its notes, and the 2,000
counts from what it holds. A file the group already lists is shown as such, comes unticked, and
is never written a second time -- `Tick all` and a heading's box leave it alone.
**Why.** The folder is where the notes are, so a group that names anything else would be a lie;
and adding a folder the library half has must not make a second group with the same name (the
pane would show two, and one of them could not be removed).
**Overturned by.** The owner; or a library where two groups may share a name.
**From.** D-041 (`home:`), D-260's heading rules; the phase 4a brief.

## D-133 · 2026-11-23 · Subfolders three deep are headings, and what a folder cannot name is refused

**Decision.** A note is filed under its subfolders as headings, three levels at most; a fourth
folder is folded into the third, so nothing is lost and no heading is made from a name the library
would read differently. A tick box that is only partly ticked is drawn with a dash: neither an
empty box nor a tick tells the truth about a heading with some of its notes in. A group or
subfolder whose name ends in `#`, or whose heading would read back as nothing, refuses the whole
plan with the sentence and asks for the folder to be renamed -- the library's reader strips a
closing run of `#`, so `C#` comes back as `C` and the note would be filed under a heading the
card did not promise. A plan is refused, never half written: the check is made before the first
file is touched.
**Why.** Three levels is what the pane's tree shows without a scroll of its own (D-260 nests by
heading level), and a heading the library cannot read back is a note that would appear somewhere
the owner did not ask for. The dash is the only state a single shared `Tick` cannot draw.
**Overturned by.** The owner; or a tree that shows deeper nesting well.
**From.** The phase 4a brief; `docs/ai-library-plan.md` 5.7.

## D-134 – 2026-11-23 – A kind this build does not write is a later version's record

**Decision.** The five `mdshared-*` pins move from the note work's branch rev to the shared
`main` (`0984eed`), where `Kind::Markdown` and `runs::write_note` are merged beside `Kind::Rewrite`
(a file rewritten in place, phase W, D-074) and `library::put`. mdedit reads three kinds and writes
two: a record whose kind it does not write — `Kind::Rewrite`, today — is not put in the runs it
could undo. `open_runs` moves it into the newer-version list, the same list and the same words an
unreadable record gets ("Written by a newer version of mdedit", "Not shown or undone here"), and
`run_undo_ask`, `run_undo`, `run_show` and `run_finish` refuse it by name ("It is left as it is").
`Record::originals` is written empty by the two records this build makes, as the shared crate says.
**Why.** Phase W is being built elsewhere; a record of it must not be read as a split (its undo
would look for line ranges and a source that a rewrite's record does not have) and nothing here
may guess what undoing a rewrite means. The fallback is the one the app already has for records it
cannot read, so there is one way to say "a later version wrote this", not two.
**Overturned by.** Phase W landing here, which gives Rewrite its own show, undo and finish; or the
shared crate giving the kinds one table of what a build can do with each.
**From.** The owner's brief of 2026-11-23 (repin to shared `0984eed`, honest handling of Rewrite);
D-120.
## D-140 — 2026-11-23 — A rewrite of a file is R2's review, applied in place, the original in the record

**Decision.** "Rewrite them" runs one file at a time: the model's answer becomes R2's list of
changes over the whole file (not a new kind of review), and Apply writes `Hunks::compose` of the
ticked changes in place, in the file's own line endings. Before each file is written, the run's
record in `.runs` holds the file's original text, its digest and the digest written (D-076); undo,
from "AI runs…", puts an original back only while the file still holds what the run wrote, and
keeps a file changed since. A file changed between the request and Apply is refused, and the
review stays open with its list.
**Why.** D-074 and D-076 chose this; R2 gave the list, and the plan (5.6) says "edits in place
after the review, the original kept in the record". Reusing R2's review keeps one set of keys and
one meaning for Apply.
**What would change it.** The owner after using it; a review per file that must show side by side
in a document tab instead of the panel.
**Overturned by.** The owner.
**From.** DeepSeek Flash, building phase W, 2026-11-23.

## D-141 — 2026-11-23 — The task window sees names and numbers, never paths; pictures are never read

**Decision.** The engine is told each file's name, size, date, first heading and first 1,200
bytes (the digest of 5.6), numbered 0..n in the request; the answer's numbers are the window's
places, and no path is sent or written from the answer. A picture's bytes are never read; its name
is sent, and it is linked. A PDF, Word file or archive is never read either: File them links it by
name, and Split them and Rewrite them leave it out with its reason on the row. The JSON contract
is mdedit's (`task_ui`), not the engine's.
**Why.** The engine must stay free of what a task is (7.2), and a model that returns a path could
write anywhere; numbers cannot. Pictures in the protocol are phase V (D-078), not yet priced.
**What would change it.** Phase V: the engine reading pictures itself, `proto::VERSION` 2.
**Overturned by.** The owner.
**From.** DeepSeek Flash, building phase W, 2026-11-23.

## D-142 — 2026-11-23 — One request at a time, in the order the results are read

**Decision.** File them sends batches of 40 files, one request after the other; Split them one
plan per file, in the window's order, the next opening when a plan is written; Rewrite them one
file per request, at most 20 files a run, the next sent only when the previous file's review is
applied, discarded or copied. The engine could run several (5.6), but nothing here does.
**Why.** Rate limits, and the results are read in order; a rewrite's review is the user's
decision before the next file is touched.
**What would change it.** The owner asking for parallel requests (D-080 priced it); then the
window's batches would run together and results would still be merged in file order.
**Overturned by.** The owner.
**From.** DeepSeek Flash, building phase W, 2026-11-23.

## D-143 — 2026-11-23 — With the AI off the window still works, and Rewrite is disabled

**Decision.** File them composes the plan locally (every file under the section typed beside the
destination) and writes the links; Split them cuts at headings; Rewrite them is disabled with
"The AI connection is off" and "Open AI settings". Nothing is started, and "Show what is sent"
says so.
**Why.** 5.6, and the app's rule that the AI is off by default and nothing leaves the machine
unless a request is made.
**What would change it.** The owner.
**Overturned by.** The owner.
**From.** DeepSeek Flash, building phase W, 2026-11-23.

## D-144 — 2026-11-23 — "Show what is sent" parks the window in a read-only tab

**Decision.** The link opens the literal text of every request the window would send in a
read-only copy tab, headed by a line saying nothing has been sent by it, and parks the window;
"AI task on your files…" brings the window back with its files, instruction and ticks. It is
offered only while no request is out.
**Why.** 3.10 and 5.6: the owner can check exactly what would leave the machine; the request is
not made to show it.
**What would change it.** The owner preferring a panel over a tab.
**Overturned by.** The owner.
**From.** DeepSeek Flash, building phase W, 2026-11-23.

## D-145 — 2026-11-23 — A rewrite's record is this build's: listed, shown and undone, not a later version's

**Decision.** Phase W writes `rewrite` records, so the guard of D-134 stays only for a kind this
build still does not write (`markdown`, and any word from a later version): `open_runs`, the runs
list, Show the notes and Undo treat `Kind::Rewrite` as their own, with its originals and its undo.
The test that proved a later version's record is listed and refused now uses a record whose kind
word this build does not know, so the same behaviour is still proven; `Markdown` — main's kind,
which this build writes — is not used for it.
**Why.** D-134's fallback was right while Rewrite existed only on phase W's branch; with W on main
the record is read by the build that wrote it, and its undo (each original back while the file
holds what the run wrote) is what the owner asked for.
**What would change it.** A later version's record kind that this build must read before it knows
it; then the guard needs the same table for that kind.
**Overturned by.** The owner.
**From.** The owner's brief of 2026-11-23 (merge phase W onto main, remove Rewrite's guard).

## D-150 · 2026-11-23 · The AI step is a second stage of the import card, and it only names headings

**Decision.** "Sort a folder by AI." (5.7, phase 4) is not a second command and not a second
writer: it is two stages of the "Add a folder" card. **Ask the AI** asks for a scheme of at most 8
sections with at most 4 subsections each, which the owner can add to, rename and delete before
anything else is asked; **Sort N files** asks the AI to put each file under one numbered section
of that scheme. The answers become the same plan rows the folder plan has, with the AI's
sections, confidence and one-line reason; nothing is written until the owner ticks and presses
Add, which is 4a's own single write. The AI never names a path, a folder or a file: it names the
headings, and the import's writer puts the links under them exactly as it does for a plan by
folders. **Keep the folders** leaves the AI and returns to the folder plan, ticks and edits intact.
**Why.** The section headings are the one thing a model can judge from a digest, and the paths,
the library file and the write already have one tested owner. One card also means one place to
correct the scheme or the ticks, and an AI answer that is wrong can be changed rather than
undone after the fact.
**Overturned by.** The owner; or a sort that must write without being read first.
**From.** `docs/ai-library-plan.md` 5.7; D-078; the phase 4 brief.

## D-151 · 2026-11-23 · A folder sort sends a short digest per file, in batches, and nothing of the library

**Decision.** The scheme request carries one line per file: its path inside the chosen folder (at
most 200 bytes), its title (120) and the first 120 bytes of its text. The filing requests repeat
the path and title and send the first 1,200 bytes of the file (a start of about 2 KB with its
line), 40 files to a request. At most 500 files are sent in a run; the rest stay in the folder plan
and the card says how many. A request is capped at 200 KB, as a rewrite's; a scheme whose paths
and titles alone pass that is refused with the number, rather than cut silently. The library,
the settings and every other file are never sent. The engine makes the request (D-079); the editor
does not.
**Why.** 5.7's budgets: enough text to tell a recipe from a receipt, not enough to be the file;
one line per file so a batch can be numbered and answered per file; a cap on the run so a folder
of thousands does not become an unbounded bill. The 500 is the rate cap the owner set for the
library work (D-080's sibling rule), and 40 keeps a batch well inside a model's answer.
**Overturned by.** The owner; or a provider that prices a batch so differently that another size
is clearly right.
**From.** `docs/ai-library-plan.md` 5.7; D-079, D-080; the phase 4 brief.

## D-152 · 2026-11-23 · Unsure, out-of-range and left-out files land in Unsorted, unticked

**Decision.** A file's answer is a section number, a confidence from 0 to 1 and a one-line
`why`. A number the scheme does not have, an id that was not sent, a file the answer leaves out,
and a `why` or title the card cannot read all leave that file under **Unsorted**, unticked, with
the AI's reason shown dim; a section number out of range says the scheme does not have it. An
answer that cannot be read at all is a refusal for the whole batch: its files stay in Unsorted,
and the card says which request failed and why. **Tick only the sure ones** ticks what the AI
placed at 0.6 or more, never a file the group already lists; **Tick all** ticks every file the
group does not already list, including Unsorted. Confidence and `why` are shown and never written.
**Why.** A wrong guess must cost a tick, not a wrong link: the owner sees what the AI was unsure
about, and the files it could not place are still one click from being added under their
folders. 0.6 is the plan's line between "sure enough to keep" and "look at it".
**Overturned by.** The owner; or evidence that 0.6 ticks too much or too little.
**From.** `docs/ai-library-plan.md` 5.7; the phase 4 brief.

## D-153 · 2026-11-23 · A failed sort keeps what came, and Escape stops every request

**Decision.** The filing requests go out up to the connection's "Requests at once" (D-080), three
by default and one for a local model, and the card shows how many answered and how many are in
flight. A request that fails stops new ones; those in flight may finish, and the files they
answered keep their sections, so the card offers what it has and says what was not reached. The
scheme request failing, or an answer that cannot be read, ends that stage with nothing changed and
a **Try again** beside **Keep the folders**. Escape while a request is out cancels every request
and gives back the edited scheme, untouched. A request is sent by the engine as every AI request
is (D-079), and no request is made while the connection is off.
**Why.** A model that fails halfway must not throw away the half that worked, and an owner who
sees it going wrong must be able to stop it without losing the scheme they edited.
**Overturned by.** The owner; or a provider that cannot cancel a request (then the wait is said).
**From.** `docs/ai-library-plan.md` 5.7, D-080; the split-by-AI behaviour (DESIGN 76).

## D-154 · 2026-11-23 · "Requests at once" is a setting per connection, 1 to 8

**Decision.** **Requests at once** is a row on the AI rewrite page, a number from 1 to 8 with
steppers, stored per connection as `ai_parallel_<key>` in `settings.ini`; an empty value means
the default (3, or 1 for a local model). It is read when a sort starts, so a change applies to the
next run.
**Why.** D-080 asks for a number the owner can lower for a rate-limited key and the local model's
one request; per connection because a local model and a hosted one are not the same. The defaults
live in core next to `ai_parallel_default`, so the card and the tests agree.
**Overturned by.** The owner; or a provider whose limits are better read from its own answer.
**From.** D-080; the phase 4 brief.

## D-155 · 2026-11-24 · A plan is made only from a tab that holds what its file holds

**Decision.** "Split into notes…" refuses a tab that has not read its file again since another
program saved it: still loading, marked changed on disk, a file stamp that is not the one the tab
read, or, for a UTF-8 file, bytes that are not the tab's text. The toast says "{name} changed on
disk since this tab read it. Reload it first: the notes are made from the file on disk."
**Why.** Audit 4, A F1 (a release blocker): the notes were cut from the tab's text and the digest
taken from the disk, so a stale tab wrote notes of the old text and step 8 could put the newer
file in the Recycle Bin. Refusing costs a reload; guessing which text the owner meant could cost
the newer text.
**Overturned by.** The owner; or a plan that cuts its notes from the disk's bytes, not the tab's.
**From.** `docs/review-prompts/audit4/a-mdedit-core.md` F1; D-071.

## D-156 · 2026-11-24 · An undo that could not take everything out is "undo pending"

**Decision.** A record gets the state `undo pending` when an undo began and a link could not be
taken out of its library file, or the Recycle Bin would not take a note. Undo can be pressed
again and takes the rest out. Finish is not offered, and `runs::finish` refuses it. The run's row
in "AI runs…" says "partly undone".
**Why.** Audit 4, A F2 (D W4): a note the bin refused was left on disk, its line already gone,
the record said "undone", and Undo was refused from then on. The note was orphaned. A link that
could not come out marked the run `links pending`, the state of a run whose links never went in,
so Finish (offered for it by D-157) would have put links back into a run half undone.
**Overturned by.** The owner; or an undo that can always finish in one press.
**From.** Audit 4 A F2, A F3, D W4; D-064, D-070.

## D-157 · 2026-11-24 · A run whose links are pending is finished or undone from "AI runs…"

**Decision.** "AI runs…" offers Finish for a split or Markdown run that stopped, is still
writing, or has its links pending (`overlays::offers_finish`). When step 6 fails, the toast says
"The notes are written, but {file} could not be updated: {why}. AI runs can finish it or undo
it." This replaces the "Try again / Undo" that the plan's step table (4.3, row 6) named. The toast
has no buttons, and Finish is the "try again".
**Why.** Audit 3 C4, reopened as audit 4 A F3: the toast promised a Try again that nothing
offered, and Finish was offered only for a run that stopped, so a links-pending run could only be
undone.
**Overturned by.** The owner; or a toast with buttons (none of the app's toasts has one).
**From.** `docs/ai-library-plan.md` 4.3; audit 3 C4; audit 4 A F3.

## D-158 · 2026-11-24 · Undo keeps a note a tab holds with unsaved changes

**Decision.** Undo does not remove, or put in the Recycle Bin, a note that is open in a tab with
changes not saved (`recycle::SparingUnsaved`). The links are still taken out first, as always.
The note stays, the panel says "it is open in a tab with changes not saved: save or close it,
then Undo again", and the record is `undo pending` (D-156). Undo again takes the note out once
the tab is saved or closed.
**Why.** Audit 3 C6, reopened as audit 4 A F5b: the file went to the bin while its tab still held
the owner's edits, and the tab's next save would have made a stray file at the old path.
**Overturned by.** The owner.
**From.** Audit 3 C6; audit 4 A F5b; D-070.

## D-159 · 2026-11-24 · A file "Move the notes…" moves takes its place in the session with it

**Decision.** When "Move the notes…" moves a file, its entry in the recent files (in its order),
its caret place and its bookmarks follow it, as a rename's do, along with any tab open on it.
**Why.** Audit 4, A F11d: the tab followed but the session did not. The recent files then listed
a path that was gone, and the place and bookmarks were lost.
**Overturned by.** The owner.
**From.** Audit 4 A F11d; DESIGN 75 (`Docs::follow_move`).

## D-160 · 2026-11-24 · A question held for the page is dropped when the tab changes

**Decision.** In visual editing, a question the page asked while the pane held it back (the line
to place, a click's place) is given only while the document it was asked of is still in front
(`Visual::take_pending`). After a tab switch it is dropped, never asked of the new document.
**Why.** Audit 4, A F13: on the frame of a tab switch, the question held for the old document
was asked of the new one, before `follow_tab` cleared it. The new tab starts from its own caret
(DESIGN 72), so nothing is lost by dropping it.
**Overturned by.** The owner.
**From.** Audit 4 A F13; DESIGN 72.

## D-161 · 2026-11-24 · The library's writers keep a byte order mark, refuse what is not UTF-8, and never take the owner's `.tmp`

**Decision.** Every write of a library file by mdedit goes through
`mdshared_app::files::replace`. This covers the pane's writers through `library_attempt`, the
run's gate, "Add this file" and 4a's write to an existing group. `files::replace` uses a
temporary name of its own (`name.part`, then `name.<pid>.<n>.part`), keeps the file's
permissions, refuses a read-only file and leaves nothing behind when it fails.

The text is read by `library_text`:
- A UTF-8 byte order mark is set aside and written back.
- A file that is not UTF-8 (another code page, UTF-16) is refused with "is not UTF-8 text" and
  left as it is. The pane still reads and lists it.

Other rules:
- "Add this file" is shared `library::put::put_note`, mdview's writer (rule 8).
- The plan's preflight refuses a read-only group file, and one that is not UTF-8, before the
  first byte.
- "Edit a library file" makes `notes.md` with `files::create_new`, so it never writes the
  template over a file that is there.

The app's own files (the session, the macros, the saved prompts) stay on `write_atomic`.
**Why.** Audit 4, RB3 (D1, A F7, W3; audit 3 C9), F17 and F18:
- `write_atomic` wrote to `<name>.tmp`, a name the owner may use, and renamed it over the file,
  dropping its permissions.
- A BOM hid the group's heading from the writers: "Rename" did nothing without a word, and a new
  note dropped its `home:` line.
- A file in another code page would have been rewritten with every letter outside ASCII
  changed.
- A read-only group was found out only after the notes were on disk.
**Overturned by.** The owner; or library files that may be written in another encoding.
**From.** Audit 4 RB3, RB5 (D3), A F7, F17, F18; audit 3 C9; D-081 (shared first).

## D-162 · 2026-11-24 · A rewrite's record names files outside the library, and its undo names every file

**Decision.** A rewrite run's record holds the originals of files anywhere: a rewrite is run on
the owner's files, which are not in the library. D-072's confinement ("a record reaches no
further than the library folder does") therefore holds for the notes, the pictures, the group's
file and the folders a split or a Markdown run made. It does not hold for a rewrite's originals.
What guards a rewrite's undo instead:
- it puts a file back only while that file still holds what the run wrote (its SHA-256);
- it writes no path the record does not name;
- the question before it names each file it puts back: "Puts back the original of N files this
  run rewrote: …".

This narrows D-072 for rewrites; it does not replace it. **Waiting** for the owner.
**Why.** Audit 4, A F8: no entry said that D-072 does not hold for a rewrite. A rewrite cannot
be confined, because its files are wherever the owner keeps them. Its record therefore gives more
than write access to the library folder: whoever can write into a synced `.runs`, and knows a
file's exact bytes, can have "Undo this run" replace that file. The question did not name the
file, and now it does, so the owner sees what will be put back before it is done.
**Overturned by.** The owner; or rewrite records kept beside the files they rewrote.
**From.** Audit 4 A F8; D-072, D-074, D-076.

## D-163 · 2026-11-24 · No crash folder beside a settings file that names no folder

**Decision.** Crash reports go beside the settings file `MDEDIT_SETTINGS` names, in `crashes`,
only when the value has a folder in it. An empty value or a bare file name keeps no reports, as
a `--no-session` window or a shot keeps none.
**Why.** Audit 4, A F20: a bare `settings.ini` put `crashes` in whatever folder the program
started in, and an empty value put it in the current folder. Neither is a place anyone chose.
**Overturned by.** The owner.
**From.** Audit 4 A F20; CLAUDE.md rule 9; DESIGN 61.

## D-164 · 2026-11-24 · The question before a section goes grows with its words

**Decision.** "Take this section out of the library…" grows its card from the lines its question
wraps to, as the delete question does, from the same 196 floor. A card capped at the window's
height cuts its body at the divider rather than draw it into the buttons. Shot
`library-remove-long`.
**Why.** Audit 4, A F19: the question names the section, whose name can be as long as a file's,
and the fixed card ran its last lines through the buttons.
**Overturned by.** The owner, on looking at it (rule 6).
**From.** Audit 4 A F19; DESIGN 71's delete question.

## D-165 · 2026-11-24 · The library-folder check folds a `..` the disk cannot answer by its spelling

**Decision.** Whether a group's folder is the library folder (audit 3 A7) is decided by
`library_write::same_folder`:
1. Both paths go through `mdshared_app::paths::comparable`, which asks the disk where every part
   that exists leads.
2. Any `.` and `..` that remain are folded by their spelling. They can only follow a part that is
   not there, so no link can be crossed by folding them.

`home: Trip/..` is then the library folder on every system, whether or not `Trip` exists. This
does not touch D-072, which refuses any `..` in a record outright.
**Why.** Audit 4, A F14: the check compared text, so `home: Trip/..` passed. And on Linux and
macOS, `std::path::absolute` keeps a `..` that follows a missing folder, so `comparable` alone
still let it through there.
**Overturned by.** Shared `comparable` folding it (then the local fold goes).
**From.** Audit 4 A F14; audit 3 A7; D-072.

## D-166 · 2026-11-24 · "Find the file…" carries the note it was asked for

**Decision.** The answer of the file picker that "Find the file…" opens carries the note it was
opened for (`Picked::LibraryFile(Moving, …)`). The window no longer remembers which note a picker
is for.
**Why.** Audit 4, A F12: off Windows the picker answers later. Another "Find the file…" or a
re-read in between then wrote the found path into another note's link.
**Overturned by.** The owner.
**From.** Audit 4 A F12.

## D-167 · 2026-11-24 · A rewrite run larger than one record goes on in a second record

**Decision.** A record holds at most what undo reads back (`runs::RECORD_MAX`, 4 MiB). When the
next file's original would not fit (`runs::rewrite_fits`), the task window's record is closed as
done and the file begins a new record. A run then has one row in "AI runs…" per record, and each
row undoes its own files.
**Why.** Audit 4, RB2 (B2, D2): a run past 4 MiB wrote a record that could not be read back, so
none of the run could be undone. Two rows that each undo are better than one that cannot.
**Overturned by.** The owner; or a record format that holds more.
**From.** Audit 4 RB2, B2, D2; D-076.

## D-168 · 2026-11-24 · While a request is out, the task window keeps what it asked

**Decision.** While a request is out, the task window takes no key and no click that would
change what was asked: the task, the files, the prompt and the group stay as they were sent.
Stop, Cancel and the scrollbars still work. The answer is read as the task that asked
(`Asking.task`), not as the task the window shows. A window parked by "Show what is sent" comes
back as it was, with its task.
**Why.** Audit 4, B5 and B7: a click on the task switch while a filing request was out turned its
answer into a rewrite review, and a parked rewrite window came back as File them.
**Overturned by.** The owner.
**From.** Audit 4 B5, B7; DESIGN 83.

## D-169 · 2026-11-24 · A file review belongs to the window and the file it was asked for

**Decision.**
- **Discard and Copy result** leave the file as it was and untick it, and the run goes on with
  the next file (B3).
- **A review kept from a window since closed** writes its own file as a run of its own, done at
  once, and sends nothing; no other window's record takes it (B4).
- **Run**, while a kept review is waiting, shows that review again. **Run again** without its
  window keeps it (B10).
- **The file is read again when it is sent**, so a file edited after it was added goes as it is
  now (B12).
- **A rewrite that wrote nothing** leaves no record when the run ends (B6).
**Why.** Audit 4:
- Discard and Copy result asked for the same file again, so the window never moved on.
- A kept review applied later marked another window's file done and sent an empty instruction.
- From the waiting window the kept review could not be reached.
- An edited file went as its old text.
- An empty record listed a run that did nothing.
**Overturned by.** The owner.
**From.** Audit 4 B3, B4, B6, B10, B12; DESIGN 83.

## D-170 · 2026-11-24 · A rewrite counts what is ticked, needs an instruction, and cuts a digest on a character

**Decision.**
- The 20 files a rewrite takes are the ticked ones that can be rewritten. Unticking one frees its
  place, and a file the limit unticked is ticked again when the task changes (B13).
- Rewrite them with an empty box sends nothing, and the footer says "Rewrite them needs an
  instruction: type one, or pick a saved prompt." (B14).
- A file's digest for filing is cut at the last character boundary under `DIGEST_MAX` (RB1, B1).
**Why.** Audit 4:
- Unticked files counted against the 20.
- An empty instruction was sent, as the editor's own rewrite refuses it.
- A cut inside a character panicked, and the release build aborts on a panic (rule 11).
**Overturned by.** The owner.
**From.** Audit 4 RB1, B1, B13, B14.

## D-171 · 2026-11-24 · Every AI sort starts from the folders' plan

**Decision.** The import card keeps the folders' own plan (each file's sections, title, tick and
order) from before the first sort. Ask, Sort, a failure and **Keep the folders** all start from
it again, so "Keep the folders" gives back the folders' headings and ticks, not an earlier sort's.
Ticks changed on a sorted plan are dropped when the AI is asked again.
**Why.** Audit 4, B8: "Keep the folders" after a second Ask kept the first sort's sections, and
Add would have written headings the owner had just declined. The Failed stage's line, "the files
would still be added under their folders", was untrue in the same way.
**Overturned by.** The owner; or a sort that refines the last one rather than the folders.
**From.** Audit 4 B8; D-152, D-153. Made by the AI agent on `fix/audit4-ai`.

## D-172 · 2026-11-24 · A split answer belongs to the plan screen it was asked for

**Decision.** The answer of "Ask the AI…" on a plan is read only on a plan screen with the same
name and outline as the one that asked. An answer that arrives for another plan is dropped.
**Why.** Audit 4, B11: in "Split them", a plan written before its answer came opened the next
file's card, and the answer then cut that file with the first file's line numbers.
**Overturned by.** The owner.
**From.** Audit 4 B11; DESIGN 76. Made by the AI agent on `fix/audit4-ai`. The core's half is on
`fix/audit4`: a write cancels its plan's split that is still out, as `plan_close` does.

## D-173 · 2026-11-24 · Two section names are one heading when they differ in ASCII case only

**Decision.** In the AI sort's scheme, two section names are the same heading when they differ
only in ASCII case (`eq_ignore_ascii_case`), both where a twice-named section is dropped and
where the scheme is checked before "Sort N files". `Çay` and `çay` are two headings.
**Why.** Audit 4, B n1: the two places used two rules, so a scheme with `Çay` and `çay` was read,
then refused, until the owner renamed one. One rule, the one the library's reader uses, removes
the contradiction.
**Overturned by.** The owner; or a library reader that folds case beyond ASCII.
**From.** Audit 4 B n1. Made by the AI agent on `fix/audit4-ai`.

## D-174 · 2026-11-24 · "Requests at once" stays per connection, and a change of kind writes its default

**Decision.** "Requests at once" stays a setting per connection (D-154). When a connection's
preset is changed between a local model and a hosted provider, the new kind's default (1 or 3)
is written in its place. Adds to D-154; it does not replace it.
**Why.** Audit 4, B n4: a 5 set for a hosted service still applied after the preset was changed
to a local model, where the default would have been 1.
**Overturned by.** The owner; or the setting kept per preset.
**From.** Audit 4 B n4; D-080, D-154. Made by the AI agent on `fix/audit4-ai`.

## D-175 · 2026-11-24 · A Markdown note whose write makes nothing keeps its card and leaves nothing

**Decision.** In "Markdown note from a file" (phase 5), a write that started and made no note
counts as a failure. Examples: the name was taken by a file with other bytes, the folder could
not be made, the disk is full. The card stays with the AI's answer and says why. The run's
record, its log and the empty folders it made are taken away, so "AI runs…" does not list a run
that did nothing. A refusal that can pass on a second try (a tab saved, the AI turned on) keeps
the button. One that cannot (the source changed since its text was taken) disables it.
**Why.** Audit 4, C3: such a write lost the card and an answer that may have taken minutes, and
left a "done" record that made nothing. Shared `runs::write_of` still writes that record. mdedit
removes it until shared stops (D-180).
**Overturned by.** The owner; or shared not writing a record for a run that made nothing.
**From.** Audit 4 C3, C12; D-120. Made by the LIB agent on `fix/audit4-lib`.

## D-176 · 2026-11-24 · A note name taken while the AI answered gets the next free name

**Decision.** When the name a Markdown note was given on its card is taken by a file before the
write, the note gets the next free name (`name 2.md` …), as it would have had when the card
opened. It is never written over the file.
**Why.** Audit 4, C3: the answer can take minutes, and a refusal there threw it away.
**Overturned by.** The owner.
**From.** Audit 4 C3; D-075, D-120. Made by the LIB agent on `fix/audit4-lib`.

## D-177 · 2026-11-24 · A held note is held by what it is, and a moving drag takes no key but Esc

**Decision.** A press on a note row in the notes pane holds that note by what it is (its
`RowId`), not by its row's index, from the press to the release. Rows made again in between (a
finished read, a save) find it again. When it is gone, the release moves and opens nothing.
While a note is being moved, every key but Esc is swallowed. A drop beside the pane, at a row's
height, is on no row.
**Why.** Audit 4, C1, C2 and A F15: a key or a finished library read made the rows again during
a drag, and the release moved, or opened, whatever note then had the row's index. A drop over
the editor still moved the note.
**Overturned by.** The owner.
**From.** Audit 4 C1, C2, A F15; D-130. Made by the LIB agent on `fix/audit4-lib`.

## D-178 · 2026-11-24 · An import does not join a group of the same name with another home

**Decision.** "Add a folder to the library…" (4a) refuses a group that has the folder's name and
a `home:` other than the folder. The card names the group and its folder, and the write checks
again. This narrows D-132's "an existing group is joined" to a group whose home is the folder.
**Waiting:** the owner decides between refusing it and joining it with the card saying so. This
is the default until they answer. Decided, not built: the LIB agent's plan is to carry the
group's home in `Where`, refuse through `plan.blocked`, and check again at write.
**Why.** Audit 4, C4: such a group was joined silently, its notes went to the other folder's
group, and the card named the wrong folder. DESIGN 81 already said it is refused.
**Overturned by.** The owner.
**From.** Audit 4 C4; D-132; DESIGN 81.

## D-179 · 2026-11-24 · A file larger than the Recycle Bin holds is refused, not deleted for good

**Decision.** On Windows, before a file is handed to the Recycle Bin, its size is compared with
the bin's capacity on that drive (`MaxCapacity`). A larger file is refused with a reason, as a
drive with no bin is: undo keeps it and says why (D-156), and step 8 leaves the original where
it is.
**Why.** Audit 4, D4: Windows deletes for good a file larger than the bin can hold, and reports
it as recycled.
**Overturned by.** The owner; or a Windows call that refuses instead of deleting.
**From.** Audit 4 D4 (shared `platform/recycle.rs`); DESIGN 77. The refusal is tested; no real
bin quota was tried.

## D-180 · 2026-11-24 · What audit 4 leaves on purpose

**Decision.**
- **W11 is left.** It needs `jiff`, a new dependency, and rule 7 asks for the owner first.
- **W12 is left.** It cannot be checked on this PC.
- **W13 is kept.** The in-place button change stays, because both released apps draw it.
- **Two shared causes are left for the owner**, because changing them changes both apps:
  - a literal `%20` in a file name always reads as a space (C6);
  - `runs::write_of` marks done a run that wrote nothing (C3; mdedit works around it, D-175).
**Why.** Each one either needs the owner's word (a dependency, a change to both apps) or cannot
be verified here.
**Overturned by.** The owner.
**From.** `docs/review-prompts/audit4/PAUSED.md`; audit 4 report D.

## D-181 · 2026-11-24 · A row's menu closes when the rows under it are made again and its row is another

**Decision.** A row's menu in the notes pane can be left open while the rows are made again from
a finished read. It closes when the row at its place is no longer the row it was opened on (by
`RowId`), and stays open when the read left that row where it was (`App::library_build`, through
which every re-read makes the rows).
**Why.** Audit 4, A F16: the menu's items act on the row's index, so "Remove from the library"
took out the line of whatever note then had the index, with no question. Closing only when the
row changed keeps a menu the owner is reading from vanishing on every re-read.
**Overturned by.** The owner; or menus that carry what their row is, not its index.
**From.** Audit 4 A F16; DESIGN 71.

## D-182 · 2026-11-24 · A move between library files looks at the note's line first

**Decision.** "Move to…" and a drag between two library files read the note's line in its own
file before the new place is written. When that line no longer holds the note (the file changed
and the pane had not read it again), neither file is written, and the toast says "the note's line
in {group} has changed since the pane read it". The removal still checks again after the new
place is written. The line is still rewritten as `- [title](path)` in the new file (C12b, open:
keeping its own text needs a shared writer that inserts a given line, rule 8).
**Why.** Audit 4, C12a: the destination was written first and the source line checked second, so
a stale line number listed the note in both files, with the "half" toast. One read prevents it.
**Overturned by.** The owner.
**From.** Audit 4 C12a, C12b; D-130.

## D-183 · 2026-11-24 · An import refuses a same-named group kept in another folder, and joins one with no folder

**Decision.** "Add a folder to the library…" (4a) looks at a group that has the folder's name:
- Its `home:` is the folder picked, or a folder that holds it: the group is joined.
- It names no `home:`: it is joined, and the card's group line says "with no folder of its own".
- Its `home:` is another folder: the plan is refused, naming the group and that folder, and the
  card's group line names the group's own folder, not the one picked. The `home:` is read again
  at Add, so a group whose `home:` changed while the card was open is refused there too.

This replaces D-178's interim, which was decided and not built, with what is built. D-178's
question is still the owner's: they may have such a group joined with the card saying so.
**Why.** Audit 4, C4: such a group was joined silently, its notes went into the other folder's
group, and the card named the wrong folder.
**Overturned by.** The owner (D-178's question).
**From.** Audit 4 C4; D-178, D-132; DESIGN 81. Made by the LIB agent on `fix/audit4-lib`.

## D-184 · 2026-11-24 · An import leaves the library's own folder out, and says so

**Decision.** When the folder picked for "Add a folder to the library…" holds the library
folder, the walk leaves the library folder out (`library_left_out`), and the card's header says it
was left out.
**Why.** Audit 4, C5: picking the folder that holds the library offered the library's own
files, ticked, as notes. Added, a library file became a note of another group, and "Delete this
library file" then refused it.
**Overturned by.** The owner.
**From.** Audit 4 C5; DESIGN 81. Made by the LIB agent on `fix/audit4-lib`.

## D-185 · 2026-11-24 · An import leaves out a file whose link would not read back, and names it

**Decision.** A file whose line in a library file would not read back as that file (a `%20` in
its name, which the shared reader takes for a space) is left out of the import and named on the
card. The rest of the folder goes in.
**Why.** Audit 4, C6: one such name refused the whole folder at Add, without naming the file.
The reader's rule is shared, and changing it changes both apps (D-180), so the import works
around it.
**Overturned by.** The owner; or the shared reader keeping a literal `%20` (D-180).
**From.** Audit 4 C6; D-180; DESIGN 81. Made by the LIB agent on `fix/audit4-lib`.

## D-186 · 2026-11-24 · An import's walk is bounded, can be stopped, and waits its turn

**Decision.**
- The walk stops after 50,000 entries read (`MAX_READ`) or at a folder 64 deep (`MAX_DEPTH`).
  The plan of such a walk is refused with that reason, never written in part.
- "Add a folder to the library…" while a folder is being read stops that read, and the toast
  says so.
- A read that ends while Settings, another card, the AI panel, the task window or a dialog is
  open waits until it closes.
**Why.** Audit 4, C8 and C12h: the walk stopped only at 2,000 files taken, so a large tree with
few text files (a drive's root, a photo archive) was walked to the end. Nothing could stop it,
and its card opened over whatever was on screen when it ended. Its depth had no bound, and a
stack overflow on its thread would abort the app (rule 11).
**Overturned by.** The owner; the bounds if a real library outgrows them.
**From.** Audit 4 C8, C12h; DESIGN 81. Made by the LIB agent on `fix/audit4-lib`.

## D-187 · 2026-11-24 · An import's new group file is written once, whole

**Decision.** A new group's library file is made in one exclusive create (`files::create_new`)
holding the dry run's text: its heading, its `home:` and its links. A file that took the name
meanwhile is left alone, and the import is refused. There is no second write and no
`remove_own_file`.
**Why.** Audit 4, C9: the file was made with its heading and `home:`, and the links went in by a
second write, so a crash between the two left a group with no notes. `remove_own_file` covered
only a refusal.
**Overturned by.** The owner.
**From.** Audit 4 C9; D-161; DESIGN 81. Made by the LIB agent on `fix/audit4-lib`.

## D-188 · 2026-11-24 · An import refused at Add says why in its footer and keeps its button

**Decision.** When Add is refused at the write (a group file open with unsaved changes, a group
whose folder is elsewhere, a line that would not read back), the card stays open, the reason is
in its footer, and the button stays on. A second press can pass once the cause is gone.
**Why.** Audit 4, C4 to C9: the card kept what Add was refused with, and its comment said the
footer drew it, but nothing drew it, so Add seemed to do nothing. The refusals C4 and C6 added at
the write needed a place to be said.
**Overturned by.** The owner.
**From.** Audit 4 C4 to C9; DESIGN 81. Made by the LIB agent on `fix/audit4-lib`.

## D-189 · 2026-11-24 · A Markdown note refused at the write can be written on a second press

**Decision.** This extends D-175 and D-176 from the start of the write to its own refusals. In
"Markdown note from a file", every refusal at the write except "the source has changed" comes
back as a failure that can pass (`note_write_failed`). Examples: the links would not read back,
a tab holds the group with unsaved changes. The card stays with the AI's answer, the reason is in
its footer, and the button stays on. Only a changed source disables the button for good
(`note_write_refused`). A run that made no note still leaves no record (D-175), and a name taken
while the answer was out still gets the next free name (D-176).
**Why.** Audit 4, C3 and C12: `note_write_refused` disabled the button for good, also for a
refusal that could pass on a second try, so the answer could not be written.
**Overturned by.** The owner.
**From.** Audit 4 C3, C12e; D-175, D-176; DESIGN 82. Made by the LIB agent on `fix/audit4-lib`.

## D-190 · 2026-11-24 · The scheme's preview says what can be done at each stage

**Decision.** In "Sort a folder by AI" (phase 4), the last line of the preview beside the scheme
depends on the stage. While the scheme is shown (`Scheme`) it says "Rename, add or delete before
the files are sent". While the files are being sent (`Filing`) it says they are being sent and
that Stop brings the scheme back to change.
**Why.** Audit 4, the review of `import-folder-ai-sorting`: the line told the owner to change the
scheme at a stage where nothing could be changed.
**Overturned by.** The owner.
**From.** Audit 4; DESIGN 84. Made by the LIB agent on `fix/audit4-lib`.

## D-191 · 2026-11-24 · A rewrite record is measured in its longest state

**Decision.** This extends D-167. Whether a file's original still fits in a rewrite's record
(`runs::rewrite_fits`) is measured with the record in the longest state word it can be closed
with (`State::longest`), not the state it has when the file is added. A record that fits when
it is written then still reads back after any later state: done, stopped, undone, or an undo
that was refused (`undo pending`).
**Why.** The review of the audit 4 fixes, S1: the record was measured as `writing`. One that just
fit grew 5 bytes past `RECORD_MAX` when an undo was refused and closed it as `undo pending`, and
it could no longer be read, so Undo was gone for that run: the failure RB2 was about.
**Overturned by.** A record format that holds more, or a state word longer than the margin.
**From.** The review of the audit 4 fixes, S1; D-167; audit 4 RB2.

## D-192 · 2026-11-24 · Undo leaves what a later run names

**Decision.** Undo of a run leaves a note or a picture the run marked as its own when a run
started later names it in a record that is not undone. Its line stays too. The undo question
says how many ("2 files are a later run's and stay."), and the report lists them under "A
later run's, kept", each with why (`UndoPlan.later`, `UndoReport.later`). Only the records named
as started later are read, and not a rewrite's or a move's, which make no notes.
**Why.** The review of the audit 4 fixes, S2. A F9 marks the notes whose names are free as the
run's in its first record. A run that died before writing them left that mark; a later run of
the same plan then found the names free, wrote the notes and ended done. The dead run's undo
took them, and the later run's record said done with nothing on disk.
**Overturned by.** The owner; or records that know which run wrote a file by its digest.
**From.** The review of the audit 4 fixes, S2; audit 4 A F9 (D W5); D-069, D-070.

## D-193 · 2026-11-24 · A run's record keeps what its links did, and undo gives that back byte for byte

**Decision.** A run's record keeps, for each library file its links went into, the text the file
held before the run's first add and the digest of the text its last add left (`Record.added`).
It is kept only when the file holds exactly those links added to that text, and only while the
record still reads back with it. While the file still holds what the run left, undo puts the
text from before back, byte for byte: its blank lines, its line ends, a missing last newline
and a byte order mark as they were (`runs::with_removals_added`). A file changed since is undone
line by line, as before. A writer's gate opts in (`Gate::remove_links_added`, whose default is
the old `remove_links`); mdedit's does.
**Why.** The review of the audit 4 fixes, S3: C11's fix took out one blank line after the link,
right for one shape of file only. An owner's blank line at the end was eaten, a link put above a
heading left its blank, and a file with no final newline gained one. Recording what the add did
is exact for every shape.
**Overturned by.** The owner.
**From.** The review of the audit 4 fixes, S3; audit 4 C11; rule 8 (the old function stays for a
caller that does not opt in).

## D-194 · 2026-11-24 · The split's guard stays, with a test of its own

**Decision.** This keeps D-172. `split_asked_here`, which reads a split's answer only on the plan
screen with the name and outline it was asked for, stays, although every way a plan leaves the
screen now stops its split first. It has its own test, which answers a split asked for one
document while another's plan is open.
**Why.** The review of the audit 4 fixes, T2: after the merge the core's cancel stopped the
split before the answer came, so the guard could be made always true with every test passing.
Taking it out would leave one path between an answer and the wrong document's lines; the guard
costs one comparison.
**Overturned by.** The owner.
**From.** The review of the audit 4 fixes, T2; D-172.

## D-195 · 2026-11-24 · "File them" writes the group's file as every library writer does

**Decision.** This extends D-161 to the AI task window. "File them" asks `library_file_free` first,
so it is refused while the group's library file is open in a tab with changes not saved: the
file is not written, the window stays for the owner to save and press Add again, and the toast
says why. It reads the file through `library_text`, so a byte order mark is kept and a file that
is not UTF-8 is refused and left as it is.
**Why.** The review of the audit 4 fixes, W1: it read with `read_to_string` and wrote with
`files::replace`, and never asked whether a tab held the file. The tab's next save then met a
file changed on disk.
**Overturned by.** The owner.
**From.** The review of the audit 4 fixes, W1; D-161; audit 4 RB3.

## D-196 · 2026-11-24 · A long name gets a short temporary, and UTF-16 is found behind a UTF-8 mark

**Decision.** This extends D-161.
- `files::replace` gives a file whose name is too long to carry `.part` a short temporary of its
  own beside it, so such a file can still be replaced.
- `files::utf16_marked` finds a UTF-16 mark after a UTF-8 one too. Shared `put_note` and
  mdedit's `library_text` both use it, so a file that begins `EF BB BF FF FE` is refused as
  UTF-16.
**Why.** The review of the audit 4 fixes, its RB3 and RB5 nits: a name of 251 characters or more
could not be replaced on Windows (the old `.tmp` had the same limit one character later), and
the UTF-16 test looked at the raw bytes, so a UTF-8 mark hid the UTF-16 one.
**Overturned by.** The owner.
**From.** The review of the audit 4 fixes, nits RB3 and RB5; D-161.

## D-197 · 2026-11-24 · One way to make the rows again, and one library-folder check

**Decision.** This extends D-181 and D-165.
- Every rebuild of the notes pane's rows goes through `App::library_build`. A rebuild that takes
  a name box out first takes the row of an open menu before the box goes.
- `note.rs`'s two checks of a note's group compare by `library_write::same_folder`, as every
  other check of the library folder does.
**Why.** The review of the audit 4 fixes, its A F16 and A F14 nits: seven direct
`ui.library.build` calls were left, so D-181's "every re-read" was not true, and taking a name
box out moves every row under it. `note.rs` still compared with `same_path`, which `home:
Recipes/..` passes.
**Overturned by.** The owner.
**From.** The review of the audit 4 fixes, nits A F16 and A F14; D-181, D-165.

## D-198 · 2026-11-24 · What the review of the audit 4 fixes leaves

**Decision.**
- **B5 is left.** A drop or the picker's answer still adds files while a request is out. The
  files are appended, so every index the request holds stays right, and what was asked does not
  change; D-168 keeps the task, the prompt and the group, which would change it.
- **RB5's stamp is left.** A write of the same size inside the clock's step is not seen. It is
  the stamp every caller uses.
- **RB2's cost is left.** `rewrite_fits` serialises the record once per file, on a click of
  Apply, not on launch, a key or a scroll.
- **D4 without `MaxCapacity` is left.** Windows' default share is not a fixed number to check
  against, and the files that go to the bin are text files far below any drive's share.
- **mdview's E5, E7, E8 and E12 nits are left**, with their reasons in mdview's `HANDOFF.md`:
  the first Ctrl+P after a read resolves the notes on the UI thread (1.6 ms for 400); a refused
  tick keeps the page still once (a protocol change); the zoom view's scale rule and the Notes
  switch a frame late are as decided.
- **Two test-only switches** stand in for what no test can stage on disk: shared's
  `runs::DIES_AT` (A F9), a constant false in a build, and mdedit's `plan_write::REFUSE_LINKS`
  (the review's T4), compiled into the tests only, which refuses the write of a run's links as
  a race would.
**Why.** Each is either a cost off the paths the budgets measure, a guess, or a change to both
apps' protocol.
**Overturned by.** The owner.
**From.** `docs/review-prompts/audit4/FIXES.md`, "The review of the fixes".

## D-199 · 2026-11-24 · A place put back with word wrap never opens past the end

**Decision.** When a file's place is put back (opened again, or at launch from the session) with
word wrap on, the top is lowered on the first layout until the last row is no higher than the
view's bottom, as without wrap. It is done once: scrolling by hand with wrap still goes on until
the last row is at the view's top (section 24), and nothing else about the place changes.
**Why.** The owner: a 119-line file reopened with its last line halfway up the window and empty
space below. A place saved in a shorter view (a split, a smaller window) was put back as it was,
because only the layout without wrap clamps the top. Clamping every wrapped frame instead would
take away the scroll past the end that wrapped scrolling has had since phase 4.
**Overturned by.** The owner; or the owner asking that wrapped scrolling stop at the end too, which
would make the one-shot fit the clamp of every frame.
**From.** The owner's report and screenshot, 2026-11-24.

## D-200 · 2026-11-25 · A fourth task, Write notes, writes notes from pictures and text

**Decision.** Write notes writes one or many Markdown notes, in as many groups and sections as needed, with the pictures copied into `pictures\` in each note's folder and embedded and described in it. Pictures are sent only by this task, only after a button that says so. Completes D-078.
**Why.** The owner: give it images and have it make markdowns, multiple or single, in as many trees as needed, with the pictures linked inside.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-201 · 2026-11-25 · Look, then arrange

**Decision.** The pictures are looked at four to a request (the model says what each shows); the notes are arranged in a separate text-only request that places pictures by number. The app writes every picture path; the model never does. Arrange again sends no picture.
**Why.** The owner chose the recommendation: any number of pictures, a failure costs four, a retry sends none.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-202 · 2026-11-25 · A picture's file name is sent, never its folder

**Decision.** With each picture its file name goes, never a folder or path.
**Why.** The owner chose the recommendation: names tell the model the order.
**Overturned by.** The owner, if names prove private.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-203 · 2026-11-25 · The picture limits are settings, with the plan's defaults

**Decision.** 100 pictures a run, 4 per request, 1568 px on the long side (2576 for small print), 3.5 MB each as sent are defaults the owner can change in Settings > AI, as are the other per-run limits of the AI tasks (files per request, the rewrite's 20 files and 200 KB). Each is clamped to a safe range.
**Why.** The owner: configurable, and other similar things configurable too, with the recommended defaults.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-204 · 2026-11-25 · File them never files a picture

**Decision.** A picture is never a library entry of its own: File them leaves pictures out, with a row that points to Write notes. Replaces D-050 and the listed-beside-it half of D-079.
**Why.** The owner called a picture listed as itself not useful, and chose the recommendation.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-205 · 2026-11-25 · New notes are reviewed as whole notes

**Decision.** Write notes' review shows the proposed tree and each note as it will read (and as Markdown), with ticks and renames, not change by change.
**Why.** There is no before to compare; the owner chose the recommendation.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-206 · 2026-11-25 · What is sent is a card

**Decision.** For the task window, Show what is sent is a card in the window, each picture as it goes then each request's text, with Open the text in a tab.
**Why.** The owner chose the recommendation.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-207 · 2026-11-25 · One command, AI task...

**Decision.** AI task on this file... and AI task on your files... become one command, AI task...; the document in front is the first line of the window's Add menu, never added by itself. AI task on this file... stays only where the pointer names a file (a tab's menu, a pane row). A note, a section or a group of the Notes pane has the AI task in its own menu; a section or group means every note under it.
**Why.** The owner reported the two commands as doing the same thing and chose the recommendation.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-208 · 2026-11-25 · Selecting several rows waits

**Decision.** Ctrl and Shift selection in the Notes pane and the Files pane is not built until the owner asks after using one row.
**Why.** The owner chose later.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-209 · 2026-11-25 · File them copies files in by default

**Decision.** File them has Link them, Copy them in and Move them in; Copy them in is the default. A file's pictures inside its own folder travel with it, links kept; others are linked where they are.
**Why.** The owner asked that Markdown files be copied or moved like pictures, and chose copy as the default.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-210 · 2026-11-25 · Undoing a move in may write outside the library

**Decision.** Undo of Move them in puts each file back at its recorded path only, only while that path is free, only the bytes the run moved. A named exception to D-072.
**Why.** Without it a move cannot be undone; the owner said yes.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-211 · 2026-11-25 · After a run, the first written note opens with its preview

**Decision.** After a task writes notes the first opens in a tab with the preview; a rewritten file that is open is re-read at once; a setting turns the opening off.
**Why.** The owner reported no preview after an AI task and chose the recommendation.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-212 · 2026-11-25 · The engine speaks protocol 2 and shrinks pictures

**Decision.** proto::VERSION becomes 2 with images in a run; the engine reads, resizes and strips metadata from each picture itself with the jpeg-encoder crate (approved by the owner, D-078's condition); the copies in the library are untouched. mdedit reads the engine's version and sends no picture to an engine of version 1. HEIC, AVIF, TIFF and SVG are not sent.
**Why.** The owner said yes to phase V and the crate.
**Overturned by.** The owner.
**From.** The owner's answers to `docs/ai-pictures-plan.md` section 18, 2026-11-25.

## D-213 · 2026-11-25 · A Claude Code run loads no MCP server

**Decision.** Every Claude Code run of the engine, text or pictures, adds `--strict-mcp-config` (with no `--mcp-config`) to `--tools ""`, so the model has no tool at all, not even the MCP servers of the user's own Claude Code configuration.
**Why.** Phase V0 found that `--tools ""` turns off the built-in tools only: the run's `init` line listed about fifty MCP tools from the user's configuration. The engine's design has always been that a run can only answer with text. With the flag the same picture was read the same way and no tool was listed.
**Overturned by.** A Claude Code release where `--tools ""` covers MCP tools, or where the flag goes away.
**From.** `docs/measurements/2026-11-25-v0-pictures.md`.

## D-214 · 2026-11-25 · The task window's settings are a page of their own, "AI tasks"

**Decision.** D-211's choice ("After a run, open": first note and its preview, first note only,
nothing) and D-203's three limits (files in one filing request 1 to 200, default 40; files in
one rewrite run 1 to 100, default 20; the largest rewrite 10 to 2,000 KB, default 200) are a
third AI page, "AI tasks", between AI rewrite and Updates. The picture limits join it with V3.
A split opens its first note and leaves the preview as it was; a Markdown note from a file opens
with its preview; File them opens nothing and puts the Notes pane on its first line.
**Why.** On AI rewrite the four rows made the page 33 px taller than the default window, where
every page must fit (the settings layout test); a page of their own also says they belong to the
task window, not to the rewrite of the open document. The split rule is the plan's (4.4).
**Overturned by.** The owner, by looking at the shots `settings-ai-tasks`.
**From.** Phase O of `docs/ai-pictures-plan.md`.

## D-215 · 2026-11-25 · Copy them in: the copies, their record and their undo

**Decision.** THE FILES on "Copy them in" (D-209) copies each file into the folder its group and
section give it, before the library's lines are written; the lines lead to the copies. A file
inside the library's folder or the group's home is linked where it is. A name taken by other
bytes gets `-2` and on; the same bytes are linked and not written again. The run's record is a
new kind, `copies`: undo removes a copy of any extension only while it is what was written and
no library file lists it, and a picture copy only while its original is unchanged where it was.
If the lines cannot be written, the copies are taken away at once. The plan face says "copied in"
or "linked where it is" on each row; the picture count is in the report, not on the row.
**Why.** The plan's rules (4.3, section 10), with nothing of the owner's changed. Counting the
pictures on the row would read every Markdown file at each draw.
**Overturned by.** The owner, by looking at the shots `task-files-copy` and `task-files-link`.

## D-216 · 2026-11-25 · A script in the library opens as text

**Decision.** A note of the library whose file is of a kind that runs (`.sh`, `.js`, `.bat`,
`.ps1` and the rest of `PROGRAMS`) opens in a tab when it is text, like any text file; only a
program's bytes, or one that cannot be read, are shown in its folder. Nothing is ever started.
This replaces phase F's rule (DESIGN 85, 2026-11-25) that showed every such file in its folder.
**Why.** Fable's review of the milestone: before phase F a click opened a script in a tab, and
the plan (section 14) says everything but pictures and documents "opens in a tab as now". A
tab runs nothing.
**Overturned by.** The owner.

## D-217 · 2026-11-25 · A copy stays while its original is gone

**Decision.** Undo of "Copy them in" removes a copy of a file only while the record names the
file it was copied from and that file is where it was with the digest it had; otherwise the
copy and its line stay, as a picture's copy already did. The record says so in `"copied"`.
Folders made above the group's home are not the record's. This adds to D-215, whose undo
removed a copy whenever it was the bytes written and nothing listed it.
**Why.** Fable's review of C1: the owner may have deleted or changed the original after the
copy was made, and then the copy is the only one; undo is the one thing that deletes.
**Overturned by.** The owner.

## D-218 · 2026-11-25 · What Move them in moves, and what it leaves

**Decision.** Move them in is a copy, then the original to the Recycle Bin, never a rename. An
original stays (copied only) when it is on another drive, changed since it was copied, listed by
a group, or linked to by a library note outside the library's folder and the group's home; a
picture stays when another file beside it or listed by the library shows it. Library notes inside
that reach are rewritten to lead to the copy; nothing else on the disk is looked for, and the
footer says so. Undo follows D-210.
**Why.** Every step can be undone or leaves both files: a copy that failed half way never loses
the original, the bin holds what was taken, and a note undo could not give back is never changed.
**Overturned by.** The owner.

## D-219 · 2026-11-25 · Move them in: a picture follows its files, a tab's unsaved work is never moved

**Decision.** Replaces nothing; tightens D-218 after Fable's review of C2. A picture goes only
when every file of the run that shows it goes. A file or a library note open in a tab with changes
not saved is never moved or rewritten, by the move or by its undo; what such a note links to stays.
A move whose record could not be read back is a copy. Undo names each file it puts back, puts back
only a file of its copy's kind whose copy the record made, and keeps a copy that a note it kept
still leads to.
**Why.** Each of these left a file showing or leading to nothing, wrote under an open tab, or let
a record name any file anywhere for undo to create.
**Overturned by.** The owner.

## D-220 · 2026-11-25 · Move them in leaves an original the library already lists

**Decision.** Tightens D-218 and D-219. A file of a move whose very bytes the library already holds and lists is not moved: its original stays where it is, and so does the picture it shows. The toast says "the library lists the same file already: left where it is". The rest of the run moves.
**Why.** Fable's re-review of C2: such a file gets no copy from the run, yet its original went, and the record named a copy the record never made. `Reach::of` then refused the whole record, so no file of the run could be put back.
**Overturned by.** The owner.

## D-221 · 2026-11-25 · Pictures go to Claude Code; Codex waits for one try by hand

**Decision.** Written as D-214 on the engine's branch and numbered again when it was merged, since D-214 was taken. Claude Code is sent pictures as stream-json image blocks on stdin, the text last. The engine can send Codex pictures (`--image` per file, then `-- -`), but the window offers Codex no pictures until one made-up picture has been read through it by hand. The two APIs are sent the documented shapes; each is proven the first time a key is used.
**Why.** V0: Claude Code read "V0 TEST 42" exactly. Codex's account was over its usage limit, so no model saw the picture; and Codex does not refuse a missing image file before the turn, so the engine checks its own files first. No API key was available to the session.
**Overturned by.** A by-hand try that fails (that connection is offered no pictures, plan section 6), or one that works (Codex is offered them).
**From.** `docs/measurements/2026-11-25-v0-pictures.md`.

## D-222 · 2026-11-25 · The engine keeps its own picture readers, and its Codex folders are swept by a run

**Decision.** For phase V1, the engine keeps its own BMP reader instead of moving the editor's DIB reader into `mdedit-core`, as plan section 7 said. It also has its own base64 writer of a dozen lines. A Codex run with pictures first removes picture folders that earlier runs left in the temp folder (`mdedit-ai-pictures-*`, older than six hours); the plan said this would happen when the engine starts. D-220's toast reads "the same file is in the library already: left where it is", because the run's own line may be the one that lists it (Fable's third look at C2).
**Why.** The engine must not depend on `mdedit-core`, which brings ropey, regex and the rest into a program that only talks to providers. Neither the BMP reader nor the base64 writer needs a new package. Sweeping when the engine starts would put file work on every start, for a folder that only Codex runs make. Six hours is far longer than any run.
**Overturned by.** A shared crate that both programs can depend on lightly.

## D-223 · 2026-11-25 · A note's body keeps live only what the app wrote again

**Decision.** Replaces the line-by-line cleaner of DESIGN 91 (phase V2). On a line of a note's body, only three things written by the model stay live:
- a fence at the line's first column, whose lines go as they are;
- a code span that closes on its own line;
- the pictures and links the app writes again from checked parts.

Every other `[`, `]`, unclosed backtick, `!` before a link, and `<` is escaped, so a link reference definition is never one. Indented lines are read like any other. A link's address is one plain token, with no title. A link's words are escaped whole. The work is linear in the line. A picture's name escapes `#`, `?`, `%` and `\` in its address. "One per picture" numbers a title on a 40-character base, at most 101 times. The arrange request is given a picture's whole text on one line.
**Why.** Fable's review of V1 and V2. A parser reads blocks and paragraphs, not lines, and six inputs it found put a remote picture or a tag live in a note. Among them: an indented line under a list, `!!`, nested words, a definition, an address with a space, and a backtick or fence that spans lines. Two long titles alike looped for ever. A line of brackets took time in the square of its length.
**Overturned by.** A CommonMark parser in the build to check against. Its test-only use was refused under rule 7 for now, so each case is tested by its exact output.

## D-224 · 2026-11-25 · What is kept of a picture is listed, not what goes

**Decision.** Replaces the details lists of DESIGN 90 (phase V1). When "Remove location and camera details" is on:
- **JPEG:** keeps APP0, APP2's colour profile and APP14; every other APPn, every comment, and anything after the end of the image goes.
- **PNG:** keeps its image, colour, transparency, density and animation chunks; any other chunk goes.
- **GIF:** loses its comments and application blocks (but the loop count) and what follows its end.
- **An animation whose blocks cannot be walked:** refused, with the reason and the option to untick.
- **An animated PNG:** goes whole like a GIF, never flattened to its first frame.

**Why.** Fable's review of V1. A phone's motion-photo video and a second picture's EXIF followed a JPEG's end. Content credentials and unknown chunks were kept. GIFs were never stripped. An unreadable animated WebP was sent as it came.
**Overturned by.** The owner.

## D-225 · 2026-11-25 · `sent` is said as the request leaves; "no pictures" is the server's own words

**Decision.** Replaces the order of DESIGN 90.
- **`sent`:** the engine says it when the sender lets the request go: the HTTP request is made, or the CLI process has started. It is never said for a run that failed first.
- **"This model does not read pictures":** said only for a 400 or 422 whose server says, in its own words, that the model takes no images. The model's and the host's names are not read. A refusal of one picture (too large, a format) keeps its message. The server's words stay in the sentence.
- **Codex picture folders:** swept also when the engine starts, on a thread of its own. Their names carry a counter.

**Why.** Fable's review of V1:
- a run with no model chosen had a `sent` record of pictures that never went;
- a host named `images…`, a model named `…-vision`, or "image too large" read as no vision;
- a killed engine's folder waited for a next Codex run.

D-222's reason against sweeping at start (file work on every start) is met by the thread: the greeting does not wait for it.
**Overturned by.** The owner.

## D-226 · 2026-11-25 · Write notes: what a run keeps, and the text files it does not read

**Decision.**
- **Looks kept.** A picture's look is kept for the window's next Send while the instruction is
  the same and, when "Write out the text" is on now, it was asked with it. Otherwise it is asked
  again. "Try it again" after either changed starts the run anew.
- **A look's answer that leaves a picture out** fails that picture (its own batch, to try again
  or go on without); one that names none fails its batch whole. No batch without a picture is
  sent.
- **Text files** take no part when no request reads them: with the AI off (the pictures alone
  make the notes) and with One per picture. Their rows say so; the button and the summary do not
  count them; text files alone with the AI off are refused with the reason.
- **"One note per picture instead"** after a failed arrange applies to that window, not the
  setting.

**Why.** Fable's review of V3: a second Send arranged with no picture, the batches a failure kept
from leaving were lost, looks asked without the text were reused for a run that wanted it, and
the button named files no request read.
**Overturned by.** The owner.

## D-227 · 2026-11-25 · A pasted picture is a temp file that goes with its window, and is always copied

**Decision.** Ctrl+V adds the clipboard's picture only on Write notes and only when the clipboard
has no text (a spreadsheet's cells offer both; the text goes to the field). It is saved under
`%TEMP%\mdedit-pasted\` because the engine reads pictures from a path. It is removed when the
window closes, and any left older than a day (a crash) when a picture is pasted. A note never
links one where it is: it is copied beside the note even with Copy off.
**Why.** The plan held it in memory until the write, but the engine needs a file; Fable's review
of V3 found the files were never removed.
**Overturned by.** The owner.

## D-228 · 2026-11-25 · The plan of Write notes lives in its window, for now

**Decision.** Replaces, for this release, plan section 15's "Plan ready" waiting in the Notes
pane. Esc and Back on the plan card go back to the task window, whose button reads "Review the
plan". Closing the window throws the plan away: Cancel or Esc on a window that keeps one says so
once, and closes the second time. "Arrange again…" keeps the plan before it until the new one is
read, and gives it back if the request fails or is stopped. Its instruction is its own; the
window's, which the looks were asked with, is not changed. The throw-away button is "Discard",
with Back beside it.
**Why.** A plan outside its window would outlive the window's pasted pictures and its looks;
keeping it in the window is the smaller change. Fable's review of V4 found the plan lost on a
failed "Arrange again…" and the looks thrown away by its instruction.
**Overturned by.** The owner asking for "Plan ready" in the Notes pane.

## D-229 · 2026-11-25 · "What is sent" for Write notes is a card, and a picture's wholeness is said once

**Decision.** On Write notes, "Show what is sent" is a card over the window with a tab per
request: each look's pictures as sent and its text word for word, the arrange's text with the
sections. "Open the text in a tab" is kept. The other tasks keep the text tab. "A picture is
sent whole: everything visible in it, a password or a face included." is said by the first window
of Write notes with a picture and the AI on, once ever (`ai_pictures_told`), in a band above the
button.
**Why.** Plan section 13: pictures cannot be shown in a text tab, and what a picture gives away
is not in its file's name.
**Overturned by.** The owner.

## D-230 · 2026-11-25 · A run of Write notes is a record of its own kind, and undo keeps what a kept note shows

**Decision.** Shared `runs.rs` gains `Kind::Notes` (written "notes", an addition: no other kind
changes). Its undo checks no source file; a copy goes only while it is the bytes written, its
original (when it names one) is there unchanged, and no note undo kept shows it, decided from the
kept notes as they read at undo. A pasted picture's copy names no original. Every planned copy is
in the record from its first save, and each folder before it is made. Further:
- with Copy off, a picture on a network share is copied, as a pasted one is, never linked by a
  `file:///` address that would not lead to it;
- a note path already taken refuses the run (split's "already there" is not used);
- a failed copy leaves a line in its note saying so;
- when no note was written, or a pasted picture's copy failed, the window stays open.

**Why.** V5 first recorded a notes run as a Markdown run with a faked source, so undo refused
when the owner changed the first copy and could break a kept note's picture; Fable's review of V4
and V5 found a crash during the copies left files no record owned, a pasted picture lost when its
copy failed, and a share's pictures linked wrongly.
**Overturned by.** The owner.

## D-231 · 2026-11-25 · One animator for the window; menus fade in as they open, closing is instant

**Decision.** mdedit takes up shared's motion (shared `main` `48eb3ce`). One `Animator` per window
(`bins/mdedit/src/motion.rs`): `compose` begins and ends its frame, and the deadline it gives is a
timer in `timers` that is cleared when it fires; nothing moving, no deadline, no frame. Whether
anything moves is decided in `Motion::policy` alone: never in a shot or a test, never with the
Animations setting off (D-232), and otherwise as the system asks (`motion_reduced`, read and
watched with `watch_motion_reduced` (called on a short helper thread, since it can wait
up to 5 s for its hidden window) from the first menu that could fade, not at launch, and read
again when the window gains the focus).
A menu, context menu or submenu fades in and drops 6 px into place by `anim::document::MENU`
(167 ms, decelerating), the one place its timing is set; closing stays instant, and the word
completion list is never animated. While a menu enters, a frame that only moves it puts back the
pixels that were under the menus and draws the menus on them, so only the menus' presence bounds
are drawn and presented; a frame where anything else changes draws everything under them, as
before. The fade blends with the frost key (`Painter::set_frost_key`), so a menu over frosted
chrome fades as the compositor shows it. The menus' damage is their `presence_bounds()` in place
of `rect().inset(-20)`; the completion list keeps the old bounds.
**Why.** STATE's "In hand": the menu open fade first, with the budgets measured before and after
(`docs/measurements/2026-11-25-menu-fade.md`). The watch is a thread and a hidden window, so it
waits for a menu.
**Overturned by.** The owner, on looking at it; or a second thing that moves, which joins the same
animator with a group of its own.

## D-232 · 2026-11-25 · An Animations setting, on by default

**Decision.** Settings › Appearance gains one switch, "Animations" (`animations` in
`[preferences]`, on by default): off, every motion is instant, by the same path as a shot
(`Policy::Never`). The system's wish for less motion still makes it instant while the switch is
on. One switch, no setting per kind of motion. Its help says "Menus fade in as they open. Off, or
when the system asks for less motion, nothing moves." and wraps at 520 px.
**Why.** The owner (2026-11-25, through the coordinator): "ADD one setting to mdedit:
'Animations', a single on/off toggle in Settings, ON by default"; "One toggle only, no per-kind
settings." D-231 had left the switch to the owner.
**Overturned by.** The owner.

## D-233 · 2026-11-25 · The dependency changes of the pin at shared main 48eb3ce are accepted

**Decision.** Moving the shared crates from `ac9fde1` to `48eb3ce` changes `Cargo.lock` by two
things and no more: a new package `libloading 0.9.1` (registry, depends on `cfg-if` and
`windows-link`), which `mdshared-fetch` now pins exactly (`=0.9.1`, shared `31d9602`), with
`libloading 0.8.9` kept for the crates that ask for it; and a dependency edge from `mdshared-text`
to `zune-jpeg`, a package already in the lock (`mdview-render` and `mdedit-ai` use it). Every other
change in the lock is the shared crates' `rev`. The Windows release exe went from 10,045,440 to
10,066,944 bytes (+21,504) with the pin alone, every shared change included; the menu fade and the
setting add 32,256 more (10,099,200).
**Why.** The owner (2026-11-25, through the coordinator), on these two changes: "accept". Rule 7
asks for a new dependency to be priced; this one comes with shared, not from mdedit.
**Overturned by.** The owner.
