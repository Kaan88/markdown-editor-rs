# Pictures into notes, and the AI workflow over files: the plan

Written 2026-11-24 on the branch `mock/ai-pictures`, from `main` at `03bf4ea` (mdedit 1.4.0).
**Nothing in it is built.** It is a plan and eighteen drawings (`shots/mock/ai-pictures-*.png`,
made by the throwaway `--mock` flag of this branch, section 5). It builds on
`docs/ai-library-plan.md` (5.6, 5.9, 5.10, 4.1 to 4.5, 6) and DESIGN 83 and 84, and replaces the
"Pictures the model can see (phase V)" part of 5.6. The decisions it would take are listed in
section 19 and are recorded in `docs/DECISIONS.md` only when the owner says yes.

What was checked for it, and what was not, is in section 20.

## 1. What the owner asked

The owner added screenshots to "AI task on your files…" and typed "categorize and describe the
screenshots in one md". Nothing of the kind happened (section 2). Then, in answers:

1. Pictures **copied**, beside the notes.
2. "make it powerful, maybe options I don't know. I want to be able to give it images have it
   make markdowns, multiple or single in as many trees needed with pictures and linked inside. not
   as itself. that doesn't seem useful." So: from pictures, and text files, the AI **writes
   Markdown notes**, one or many, in as many groups, sections and folders as it takes, with the
   pictures **inside** the notes and described. A picture as a library entry of its own is not
   wanted.
3. The connections that matter: Claude and GPT. And what happens with Claude Code and Codex.
4. Size per run: unknown; propose limits.
5. Yes to phase V and to the `jpeg-encoder` crate, after the design is talked through. And: "have
   it complete my idea of an AI workflow."

**Four points added by the owner the same day**, each answered in section 4:

6. "AI task on this file and on your files does the same thing." (4.1)
7. "I should also be able to select multiple files from the tree for AI workflow." Then
   corrected: "just an addition, probably not necessary. clicking and using on the library should
   be possible too." So the main way is **one row of the library**, and selecting several is a
   later, optional piece. (4.2)
8. "The same for images should apply to md's. I want to copy or move them along with images
   maybe." (4.3)
9. "No preview shows up after AI." (4.4)

## 2. What exists today, and what is missing

**Why the run did what it did.**

- The engine sends text only. `Request::Run` is `{ id, conn, model, effort, system, prompt }`
  (`bins/mdedit-ai/src/proto.rs:31`), `proto::VERSION` is 1 (`proto.rs:15`), the Claude body's
  content is one string (`bins/mdedit-ai/src/api.rs:285`), the OpenAI body's too (`api.rs:584`),
  and both command lines are given text on stdin (`bins/mdedit-ai/src/cli.rs:309`, `:419`).
- A picture is told by its extension and never read (`bins/mdedit/src/app/task.rs:194`). What
  the model gets of it is one line, its name and "a picture" (`bins/mdedit/src/task_ui.rs:811`).
  The window says so: "PICTURES · NAMES ARE SENT, THE PICTURES ARE NOT: LINKED"
  (`task_ui.rs:144`).
- "File them" then writes **every placed file as a link line**, a picture like a note:
  `add_note_in_group(text, sections, title, link_path(...))` in `task_file_write`
  (`app/task.rs:951`). That is the `- [title](picture.png)` the owner found. The instruction
  asked for one Markdown file with descriptions; no task in the window writes a note from a
  model's words about several files, so "File them" was the nearest and did its own thing.
- Clicking such a line opens it in a tab: `library_row` calls `self.open(&path)` for every
  note row whatever the file is (`bins/mdedit/src/app/library.rs:240`). A PNG in the text editor
  is garbage.

**Promised in `ai-library-plan.md` 5.6 and not built.**

| Promise | State |
|---|---|
| A picture line opens in the system's viewer, not a tab | Not built. `mdshared_ui::platform::open_file` exists (shared `platform.rs:376`); nothing in mdedit calls it |
| "Copy the pictures beside the notes" | Not built in the window. The shared writer has the step: `runs::Request.pictures`, `NewPicture { source, folder }`, copied exclusively, recorded and undone (shared `runs.rs:1519`, `:1744`). All five callers in mdedit pass `pictures: Vec::new()` |
| A picture written into a note as `![name](pictures/name.jpg)` | Not built |
| D-079's setting (a run may add pictures to notes it did not write) | Not built: no setting, no code |
| Phase V, pictures the model can see | Not built. The resizer was measured only (`spike/v-resizer`, `a0d1338`) |
| mdedit reads the engine's version | Not built: `parse_event` drops `hello` (`bins/mdedit/src/ai.rs:712`); STATE lists it as known wrong |

**For the four added points.**

- The two commands differ by one line. "AI task on your files…" is `task_open(File, Vec::new())`
  and "AI task on this file…" is `task_open(File, vec![p])` with the document in front
  (`bins/mdedit/src/app/commands.rs:2160` to `:2174`). Both are in the palette
  (`commands.rs:424`, `:425`), the AI menu (`overlays.rs:183`, `:188`) and the Notes library menu
  (`overlays.rs:919`, `:925`). Nothing in the window says where its one row came from.
- The Notes pane has one cursor and no selection (`library_pane.rs:116`, `cur`). A click on a
  note opens it; a click on a group or a section folds it (`app/library.rs:223`).
- "File them" writes links and nothing else. A text file stays wherever it was, and so do the
  pictures it shows.
- After a run, three of the four tasks open nothing, and none touches the preview (4.4 has the
  lines).

**Found while reading, not known before.**

- **"File them" leaves no run record.** `task_file_write` never calls `runs::` (the only calls
  in `app/task.rs` are the rewrite's, from line 1332). So the owner's run is not in "AI runs…"
  and cannot be undone there; the lines come out by hand with the Notes pane's Remove. The files
  were not touched: only links were written.
- The resizer spike does not read a JPEG's orientation and does not remove a picture's metadata
  (location, date, camera). Both matter here (section 7).

## 3. The workflow, end to end

One new task, **Write notes**, in the same window, and a clear rule for the old ones.

1. **Give it files.** One command, "AI task…", and the window's **Add** menu (4.1); a note, a
   section or a whole group of the Notes pane, from its own menu (4.2); a drop from Explorer;
   or (new) Ctrl+V of a screenshot while the window is open. Pictures and text files together.
2. **Say what the notes should be**, or pick a saved prompt. Choose where they go and, under
   **Options**, how many notes, whether the pictures are copied and renamed, whether they are
   described, whether their text is written out, and how large they are sent.
3. **See what is sent** before it is: every picture as it will go (format, pixels, bytes, what
   was done to it), and the text of every request, word for word.
4. **Send.** Two kinds of request, and the window says which is going:
   - **Look**: the pictures, four to a request, three requests at a time (D-080). For each
     picture the model answers a title, what it shows, its subjects and, when asked, the text in
     it. Each row shows its answer as it arrives.
   - **Arrange**: one request with **no picture in it**: the answers of the look, the text
     files, the instruction, and the sections the group already has. The model answers the
     notes: titles, sections, and each note's Markdown with a mark where each picture goes.
5. **Review the plan.** The notes as a tree, each with its pictures; the selected note as it
   will read, and as Markdown. Tick, untick, rename, move a picture to another note. "Arrange
   again…" asks the second request again with a changed instruction: **the pictures are not sent
   a second time**.
6. **Write.** The record first, then folders, the copies of the pictures, the notes, the
   read-back, the links (4.3 of the library plan, unchanged). The Notes pane opens on the first
   new note.
7. **Afterwards.** The first note opens with its preview (4.4). "Undo this run…" on the pane's
   foot and in "AI runs…"; the new group's own menu and "AI task on these notes…" to go on
   (translate them, tidy them); "Again…" on a run's row to reopen the window on the same files.

Why two kinds of request and not one: a request can hold only so many pictures, so past a
handful the model cannot see them all at once anyway; a failed request loses four pictures and
not forty; the owner sees per picture what the model made of it before any note exists; and a
second try at the notes costs one small text request. The price is one more request on a small
run, and that the notes are written from the model's own descriptions and not while it looks.
Question 1 of section 18.

**With the AI off** the task still works (3.2 of the library plan): "One per picture" or "One
note", composed here with no request: a title from the file's name and the picture inside.
"Make the plan" instead of "Send".

## 4. The tasks, as the whole should be

The window's switch becomes `Write notes | File them | Split them | Rewrite them`. What each is
for, said once:

| Task | From | Makes | The files given | State |
|---|---|---|---|---|
| **Write notes** | Pictures, text files, notes | New notes of the model's words, pictures inside | Not changed; pictures copied | **New: this plan** |
| File them | Text files, notes, other files | Links in a group | Linked where they are, or **copied or moved into the group's folders with the pictures they show** (4.3) | Built. Changes: the copy and the move (4.3), pictures are no longer filed (section 14), and it gets a record |
| Split them | A long text file | Notes cut from its own lines | Not changed (Recycle Bin on a tick) | Built |
| Rewrite them | Text files | The same files, changed after a review | Rewritten, original in the record | Built |

Things the owner might expect as tasks and are **uses of Write notes**, each a saved prompt
shipped with the program (`kind: notes` in `ai-prompts.md`, beside `file` and `split`):

- *Describe and sort*: the default. "Describe each picture and sort them into notes by what
  they are about."
- *Read the text in them*: turns on "Write out the text in the pictures". Screenshots, scans,
  whiteboards, receipts.
- *A step-by-step guide*: one note, the screenshots in order, what to do at each.
- *One note for all of them*, *One note per picture*: the "How many notes" switch.
- *Merge these notes into one*: text files in, one note out, the originals kept.
- *A table from these*: receipts, price tags, readings: a Markdown table and the pictures under.

Later, each its own piece, each needing its own yes (section 17): pictures **into notes that
already exist** (D-079, behind its setting, through the change list); **"Describe this
picture"** on an image link of the open document; **"Sort this group again…"** (phase 4's
scheme and filing, on a group that is already in the library); a drop onto a group in the Notes
pane.

### 4.1 The ways in: one command

The owner is right: two commands that open the same window, one of them with a row already in
it, are one command twice.

- **One command, "AI task…"**, in the palette, the AI menu and the Notes library menu. "AI task
  on this file…" leaves all three.
- **It opens on the row of the Notes pane the keyboard is on**, when the pane has the keyboard,
  and says so under the title: "The 4 notes of Recipes › Pasta, from the Notes pane." (shot
  `from-section`). Otherwise it opens empty.
- **The document in front is not added by itself.** It is the first line of the window's **Add**
  menu, by name, one click: "This document · sample.md" (shot `add`). The menu holds every way
  in: this document; the open tabs; the row of the Notes pane ("Pasta · 4 notes"; the drawing
  shows the later, optional wording "the notes selected"); Files…; A folder…; the picture on the
  clipboard. "Add files…" becomes "Add".
- **Where the place names the file, the place keeps its own entry**: a tab's right-click ("AI
  task on this file…", for the tab under the pointer, which today's code cannot do without
  activating it first, DESIGN 83), a row of the Notes pane (4.2), and a row of the
  Files pane when it has a menu. Those are not duplicates: what they take is under the pointer.
- **The task it starts on follows what is in it**: pictures, Write notes; notes of the library,
  Rewrite them; other text files, File them; a mix, the task used last.

Not chosen: adding the document in front silently and saying so in the window. The owner would
have to remember which tab was in front, and for the pictures task it is the wrong file more
often than the right one. Half a day.

### 4.2 From the library: one row, and the AI task on it

The main way, and all that the first build has: **a single row of the Notes pane**. A note, a
section, or a whole group. Shots `tree-row` and `from-section`.

- **The row's own menu gets one line at its head.** On a note: "AI task on this note…". On a
  section: "AI task on these 4 notes…". On a group: "AI task on these 23 notes…". The number is
  in the line, so what will be taken is known before the click. The rest of each menu is as it
  is today.
- **A section or a group means all its notes**: every note under it, the ones in its
  subsections too, folded or not, as the library reads at that moment. A section with no note
  under it has the line disabled.
- **With the keyboard**: the menu key and Shift+F10 already open the row's menu
  (`app/library.rs:320`), and the line has an access key. And the one command, "AI task…" (4.1),
  **takes the row the keyboard is on** when the pane has the keyboard.
- **"Clicking and using"** is therefore: click a section or a group (the pane keeps the
  keyboard after that click, `app/library.rs:231`), then "AI task…" from the AI menu, the
  palette or its key. A click on a note opens it and gives the keyboard to the editor
  (`library.rs:240`), as today; "AI task…" then opens empty with that note as the first line of
  Add, "This document".
- **The window opens on those notes** and says where they came from: "The 4 notes of Recipes ›
  Pasta, from the Notes pane." It starts on Rewrite them. Rows can be unticked there, which is
  how "all of Pasta but one" is done without any selecting in the pane.
- A group larger than a task takes (Rewrite them: twenty notes) still opens; the rows past the
  limit say they wait, as the window does today.

One day: the line in three menus, the notes under a row, the command taking the keyboard's row,
the header's sentence, its tests and shots. No new state in the pane, and nothing in the shared
tree changes.

**Later and optional: several rows selected** (shots `tree-select`, `from-notes`; phase MS,
section 17, on its own yes). The owner called it "probably not necessary", and unticking in
the window covers most of what it would add. If it is wanted: Ctrl+click adds or removes a row
and opens nothing; Shift+click takes a range; Ctrl+click on a section or a group takes all of
it ("all 4"); Shift+Up and Down, Ctrl+Space and Ctrl+A with the keyboard; Esc clears the
selection before it does anything else. Selected rows are tinted, the pane's foot says "6 notes
selected", and the selection has a menu of its own: "AI task on these 6 notes…", "Move the
notes…" (one group at a time), "Copy their paths", "Remove 6 from the library…" (one write per
library file), "Clear the selection". It is kept by path across a re-read and never saved with
the session. Three days. The same in the Files pane, which has no keyboard and no row menu to
build on, is a further day and a half (phase MF).

**Which tasks make sense for notes that are already in the library**, and what the window does
with each (shot `from-section`):

| Task | For library notes |
|---|---|
| **Rewrite them** | The one it opens on. Each note in place after its change list, the original in the record. Twenty notes and 200 KB each, as built; past that the rows say which wait |
| **Write notes** | *Merge into one*: "One note" under Options, the notes as its material, a new note written, the originals kept. The report then offers "Remove the 6 originals from the library…", a question of its own. Also a summary, an index, a table drawn from them |
| **Split them** | Each long note into notes of its own lines, in the same group |
| **File them** | They are filed already: the rows read "already in the library", as today. To put them under other headings there is "Move the notes…", and, later, "Sort this group again…" (phase S), which a group's or a section's own menu is the natural way into |

### 4.3 Text files copied or moved in, with their pictures

"File them" gains a switch beside the task, **THE FILES: `Link them | Copy them in | Move them
in`** (shots `file-copy`, `file-move`). It is beside the task because it is as much a part of
what is done as the task is.

- **Link them**: today. A line in the group; the file stays where it is.
- **Copy them in**: each file is copied into the folder the plan gives it (the group's home, its
  section's folder), under its own name, and the line points at the copy. The original stays.
- **Move them in**: the same, and the original is gone from where it was.

**Which pictures go with a file.** The ones its text shows by a relative address that exists,
of the picture kinds, **inside the file's own folder or below it**: the rule "Move the notes…"
already uses for what a note carries (D-091, D-093). They keep their place relative to the
file, so `img/harbour.jpg` beside the original is `img/harbour.jpg` beside the copy and the
text does not change. A picture that lives elsewhere (`../shared/logo.png`, an absolute path)
is not taken; the file's link to it is rewritten so it still finds it. A `.txt` file has no
links and is copied byte for byte.

**A picture two files show.** Copied: each folder that needs it gets one (in one folder, the
same bytes are used once). Moved: it goes with the first file, and the other's link is
rewritten to it, as the mover does today for a target two notes use.

**Links.**

- *From* a file that goes in, to another file of the same run: rewritten to that file's new
  place. To a file that is not in the run: rewritten to reach it where it is, relative on the
  same drive, absolute with forward slashes on another (D-093).
- *To* a copied file: nothing changes; the original is still there.
- *To* a moved file: the library's notes that link to it are rewritten (the mover's scan, at
  most 10,000 files), and so are the files of the same run. **Any other file on the disk that
  links to it is not looked for and will not find it.** The window says so before the run
  (shot `file-move`) and the plan says it again. This is the reason Copy is the safer default.
- A file that is not UTF-8 and needs a link rewritten is copied or moved with its links as they
  were, and its row says so.

**Collisions and edges.** A name that is taken in the folder: the same bytes, it is used and
reads "already there"; other bytes, `-2`, `-3`. A file already inside the library folder or the
group's home is linked where it is whatever the switch says ("already inside the library's
folder"); rearranging those is "Move the notes…". **A move to another drive is not a move**:
the file is copied and the original stays, and its row says "on another drive: copied". Nothing
is copied or moved before "Add N to the library" on the plan face, which lists each file with
where it lands and how many pictures go with it.

**What is reused.** The carry rule, the link rewriting, and the move's record and undo are
`mdshared_app::moves` (`carry`, shared `moves.rs:799`; `undo`, `:3112`). What is new: the mover
moves notes the library lists, inside the library or a group's home; a move *in* starts outside
both. So the shared crate gets a variant beside what exists (rule 8), and the copy is the
shared writer's picture step widened to any file (`runs.rs:1744`).

**Undo.** A copy: the copies the run created come out while they are byte for byte what was
written and no library file lists them, and a picture's copy only while its original is still
there (section 10). A move: every file goes back to where it was, with its pictures, and the
rewritten links get their old text, as the mover's undo does. **That writes outside the library
folder, which no undo does today** (D-072 confines a record to the library and the group's
home). The rule proposed: a file is put back only at exactly the path the record holds, only
when nothing is there, only while it is still the bytes that were moved; otherwise it stays in
the library and is listed. This needs the owner's yes; question 10.

**The default.** Copy them in, for files outside the library, remembered once changed. It is
what the owner asked for pictures, it makes a library that can travel, and it touches nothing
of the owner's. Its price is two copies of a text that can then differ; question 9.

**Not here.** Write notes does not copy or move its text files: they are material, a new note is
written, and they stay. Split and Rewrite work where the file is. "Add a folder to the library"
makes the folder the group's home, so nothing needs to travel.

Two days for the copy, two and a half for the move.

### 4.4 After a run: what opens, and the preview

**"No preview shows up after AI" was not reproduced.** What the code does after each task:

| After | What the code does today | Where |
|---|---|---|
| Split | Shows the Notes pane with the cursor on the first note. **Opens no tab** | `open_notes_of`, `app/plan_write.rs:595`, called at `:253` |
| Markdown note from a file | Opens the note in a tab. Does nothing about the preview | `app/note.rs:658` |
| File them | Closes the window and shows a toast. No tab, and the pane is not moved to the new lines | `app/task.rs:984` |
| Rewrite them | Writes the file and shows a toast. No tab. A tab that already holds the file is not re-read there (the split calls `check_changed`, `plan_write.rs:239`; the rewrite does not) | `task_review_apply`, `app/task.rs:1311` |
| AI edit of the open document | The text changes in its tab; the preview pane follows, because it follows the tab in front every frame | `app.rs:3049`, `preview_sync`, `app/preview.rs:109` |

The preview pane shows only the document in the tab in front (`preview.rs:109`), and it is
opened only by the Preview button (`toggle_preview_pane`, `preview.rs:28`). So the likeliest
meaning of the report is the plain one: **after a run nothing asks for a preview, and after
three of the four tasks there is not even a tab for one to follow.** With the preview in
mdview's own window instead of the pane, a note opened by a run is a new document that was
never linked to the viewer; that path was not traced. Whether a rewritten file that is open and
previewed shows stale text until the window is next focused was not tried either.

**What should happen**, one rule per task:

| After | Opens | The preview |
|---|---|---|
| **Write notes** | The first note written, in a tab; the Notes pane on it | **Shown**, in the mode chosen in Settings › Editor. A note of pictures read as text shows none of them |
| Markdown note from a file | The note, as today | Shown |
| Split | The first note, in a tab | Follows if it was showing before the run; not turned on |
| File them (linked, copied or moved) | Nothing: the files are the owner's and were not changed. The pane goes to the first new line | Untouched |
| Rewrite them | Nothing new. A tab that holds a rewritten file is re-read at once | Follows the tab, so it shows the new text |
| AI edit of the open document | Nothing: it is open | Follows, as today; the mdview-window mode to be checked |

- Only the **first** note opens, never forty tabs. The report lists the rest.
- One setting, Settings › AI, "After a run that writes notes": `Open the first with its preview`
  (default) `| Open the first | Open nothing`.
- No viewer installed and no built-in pane: the note opens without a preview and nothing is
  said, as the Preview button behaves today.
- Shot `result` is this rule after Write notes.

One day, with the cause found first by trying it in a real window.

## 5. Each screen

All drawn by `--mock <name>`, 1280 x 800, light and dark. The sample is five pictures and one
text file (six recipe notes and four travel files for the later screens); every word and number
in the drawings is made up.

| Shot (`shots/mock/ai-pictures-…-light.png`, `…-dark.png`) | What it shows |
|---|---|
| `window` | The task window on Write notes: the fourth segment, the Options button beside WHERE IT GOES, a thumbnail on each picture row, what each picture is sent as, and the footer's sentence and button "Send 5 pictures and 1 file" |
| `options` | The options open: How many notes, and seven ticks with a line under each (section 12) |
| `looking` | While the pictures are looked at: the field says how far it is, each row shows what the model said of it, Stop |
| `plan` | The review when the model made a note per subject: group, sections, notes, the pictures under each note, the unused picture, and the selected note as it will read |
| `plan-one` | The same for "one md": one note, its headings, the pictures under them. This is the owner's own request |
| `sent` | "What is sent": the pictures as they go, then the text of a request word for word, one tab per request |
| `result` | The library afterwards: the group in the Notes pane, the note as text with its `![…](pictures/…)` lines, the note as it reads, and the pane's foot with "Undo this run…" |
| `undo` | The question before a run with pictures is undone |
| `limit` | 104 pictures ticked where a run takes 100, and three pictures left out, each with its reason |
| `failed` | One look request failed (a rate limit): what was looked at is kept; "Try it again", "Go on with 4" |
| `no-vision` | The model does not read pictures: the sentence, and "Open AI settings" |
| `add` | 4.1: the window opened empty by the one command, its **Add** menu open: this document, the open tabs, the notes selected in the Notes pane, Files…, A folder…, the picture on the clipboard |
| `tree-row` | 4.2, the main way: the menu of one row of the Notes pane, the section Pasta, with "AI task on these 4 notes…" at its head and the rest as today |
| `from-section` | 4.2: the window opened from that row on Rewrite them, with the section's four notes and the sentence saying where they came from |
| `tree-select` | 4.2, **later and optional**: six notes selected (a whole section by one Ctrl+click, "all 4", and two more), the selection's menu, and the foot "6 notes selected" |
| `from-notes` | 4.2, later and optional: the window opened from that selection |
| `file-copy` | 4.3: File them with THE FILES on "Copy them in": each file with the pictures that go with it, one already inside the library, and a picture that is not filed |
| `file-move` | 4.3: the same on "Move them in", with the footer's warning about other files that link to them |

Notes on them:

- **The header button is "Add"**, a menu, on every face of the window (4.1). The six window
  shots of the first round were shot again for it.
- **The window** keeps `task_ui::geo`. One change is proposed to it: a saved prompt's row 40 px
  high, not 44, so two rows show whole in the list's 80 px (today the second is cut through).
- **A picture's row** reads its pixels and size, and at the right what goes: "sent as it is" or
  "sent smaller: 1568 × 882 · 204 KB". Those figures are the engine's, measured when the file is
  added (section 6, `prepare`); until they arrive the row says "measuring…".
- **The button names what leaves**: "Send 5 pictures and 1 file". Not "Run".
- **The plan** is a new card (1120 x 720), not the split's: its right side is a note of the
  model's words, not the owner's lines. It reuses the shared tick, switch, rows and footer. Under
  each picture row is what the model said it shows, so a wrong description is seen where the tick
  is. F2 renames a note, a section or a picture's copy; the row menu has "Put in…" (another
  note), "Leave out", and for a note "Join with…".
- **The result** shot draws the pane, the text and the preview by hand; the tab bar above still
  says `sample.md` because the mock draws over a real window.

## 6. The protocol: `proto::VERSION` 2

Two changes, both free of task words (D-055).

**`run` gains `images`.** A list of files the engine reads itself, so a request line stays small:

```json
{"op":"run","id":9,"conn":{"kind":"claude-api"},"model":"…","system":"…","prompt":"…",
 "images":[{"path":"C:\\…\\IMG_4032.jpg","sha256":"…"}],"long_side":1568,"strip":true}
```

Before the first `delta` the engine says what went, for the record:
`{"ev":"sent","id":9,"images":[{"bytes":372004,"sha256":"…"}],"text_bytes":912}`. `sha256` in
the request is the digest of the bytes the owner was shown; a picture that no longer makes those
bytes is refused (`{"ev":"error","kind":"image",…}`: "IMG_4032.jpg changed since it was
measured"), so what is shown is what is sent, to the byte.

**A new request, `prepare`.** The same list, nothing sent anywhere: for each picture one event
with the format, the pixels and bytes as they would be sent, the digest, what was done
("as it is", "smaller", "details removed"), the original's pixels, and a thumbnail (a PNG of at
most 160 px, base64, one event per picture so no line nears the 1 MB limit); or why not. The
editor never decodes an original, so its memory does not move with the size of a photo.

**`hello`.** The engine says `"version":2`. mdedit reads it (it does not today) and, with an
older engine beside it, disables sending pictures with "The AI engine beside mdedit is older
than mdedit and cannot be sent pictures. Reinstall or update." Text tasks go on as before.

**New failure kinds**: `no_vision` ("This model does not read pictures"), `image` (one picture
could not be prepared; the message names it).

**Per connection.**

| Connection | How the picture goes | Known | Not known |
|---|---|---|---|
| **Claude API** | `content` becomes a list: one `{"type":"image","source":{"type":"base64","media_type":"image/jpeg","data":"…"}}` per picture, then the text block | The shape is the documented one. `base64` is in the lock already | Nothing was sent for this plan |
| **OpenAI-compatible** (GPT, and local servers) | The user message's `content` becomes a list: the text, then `{"type":"image_url","image_url":{"url":"data:image/jpeg;base64,…"}}` per picture | The shape is the usual one | Which local models accept it. A text model answers 400; the engine maps a 400 that speaks of images to `no_vision` |
| **Claude Code** | `-p --input-format stream-json`, and one line on stdin: a user message whose content is the image blocks and the text | The flag is in `claude --help` of 2.1.330 on this PC (read 2026-11-24) | **Never run with a picture.** Whether it takes image blocks with `--tools ""`. If not, there is no other door: the model has no tools and cannot read a file, by design |
| **Codex** | `exec --image <FILE>…`: the engine writes the prepared pictures into a new private folder under the temp folder, names them on the command line, and deletes the folder when the process ends | The flag is in `codex exec --help` of 0.170.0 on this PC (read 2026-11-24) | **Never run with a picture.** `--image` takes several files, so the `-` that means "prompt on stdin" may need `--` before it |

The first half day of the build (phase V0) sends one made-up picture through each of the four
and writes down what happened. A connection that fails it is offered no pictures: Write notes
says "Claude Code cannot be sent pictures: choose the Claude API or an OpenAI-compatible
connection in Settings › AI" where the button is, and the text tasks are untouched.

## 7. What is done to a picture before it is sent

In the engine, from the spike's `resize.rs` (1,046 lines with its tests), with four additions.

- **Formats sent**: JPEG, PNG, WebP, GIF, by the file's first bytes, never its name. **BMP** is
  turned into a PNG (the DIB reader exists, `image_paste.rs:337`; it moves to `mdedit-core` so
  both programs have it). **SVG, AVIF, TIFF, HEIC** are not sent.
- **HEIC**: no. There is no decoder in the tree, a good one is a C library with its own licence
  and patents, and mdview could not show the picture in the note anyway. The row says "HEIC: not
  a picture this program can read. Save it as JPEG first." (shot `limit`).
- **Size.** A picture inside both limits is sent as it came, byte for byte. Over them it is
  scaled down by area average and written again (JPEG at quality 85, PNG as PNG, WebP as PNG).
  The limits: the long side, **1568 px**, or **2576 px** with "Send larger pictures, for small
  print"; and 3.5 MB as sent. The spike's 2000 px becomes these two because the provider's
  reference gives 1568 as the older models' size and 2576 as the current ones'.
- **Added: orientation.** A phone photo is often stored sideways with a note saying how to turn
  it. The engine reads that note and turns the picture when it writes it again; a sideways
  picture that would have gone as it came is written again too.
- **Added: metadata out** ("Remove location and camera details before sending", on by default).
  A JPEG's EXIF and XMP, a PNG's text and EXIF chunks, a WebP's EXIF and XMP: taken out of what
  is sent, without decoding when the picture is otherwise untouched. A phone photo carries where
  and when it was taken; a description does not need it. **The copies in the library keep
  theirs**: they are the owner's files in the owner's folder.
- **Refused, with the reason on the row**: more than 64 million pixels; an animation over
  3.5 MB (it cannot be scaled); a file that does not decode; a picture still over 3.5 MB after
  four tries.
- Everything read is a file nobody has checked: sizes come from the header before anything is
  allocated, and no path can panic (rule 11). The spike's tests cover this and stay.

## 8. The answer contracts

Both live in `mdedit-core` (a new `notes_ai.rs`), read by the depth-bounded reader `import_ai.rs`
already has. Every field optional, every number checked, nothing that can panic.

**Look**, per request of at most four pictures. The model is given the instruction, and per
picture a number and its file name (never a folder):

```json
{"version":1,"pictures":[
  {"id":0,"title":"Login error","says":"The sign-in window with a red error.",
   "subjects":["Sign-in","Errors"],"kind":"screenshot","text":"Your session has expired."}]}
```

**Arrange**, one request, text only. Given: the instruction, "How many notes", the look's
answers numbered as before (of `text`, the first 300 characters), each text file whole, and the
headings the group has. Answered:

```json
{"version":1,"notes":[
  {"title":"Sign-in fails after the update","sections":["Sign-in"],
   "body":"Since the update…\n\n## The error\n\n![The sign-in window](picture:0)\n\nPressing…"}],
 "unused":[4]}
```

A picture is placed by writing an ordinary Markdown picture whose address is `picture:` and its
number. **The app writes the real line.** The model never writes a path, and cannot.

**The checks**, in order. Each has a test.

1. The envelope rules of 5.2: one JSON object, `version` 1, at most 2 MB (the engine's cap).
2. An `id` or a `picture:` number that was not sent is ignored. The first answer for an id wins.
3. `title`: `clean_name` (`task_ui.rs:298`), one line, 120 characters. `sections`:
   `clean_sections` (`task_ui.rs:317`), three levels, 64 characters each; a section is a folder
   name and passes the folder rule. The file's name is the library's own rule (D-075); a name
   that is taken gets `-2`, `-3`.
4. `says` 600 characters, `subjects` three of 40, `text` 6,000 characters: longer is cut at a
   character and marked "(cut)". `kind` is one of five words or "other".
5. **A note's body**: at most 200 KB; line ends made LF; control characters other than tab and
   line feed dropped; a first heading that repeats the title dropped, because the app writes
   `# Title` itself; a front-matter block at its head dropped.
6. **Pictures in a body.** `![alt](picture:N)` with N sent and ticked becomes
   `![alt](pictures/<name>)` on a line of its own. `alt` is one line of 200 characters with its
   brackets escaped; an empty one takes the look's `says`. A picture placed twice keeps its
   first place. **Any other picture in the body** (a web address, a path) loses its `!`, so no
   note this program writes can make a viewer fetch something.
7. **Links in a body** are kept only to `http:`, `https:` and `mailto:`; any other target is
   unlinked and its words kept. Raw HTML has its `<` written as `&lt;` outside code. (What
   mdview does with raw HTML was not checked for this plan; the escape is the safe side.)
8. "Write out the text in the pictures": the app adds the look's `text` under the picture as a
   quote, whole. It does not travel through the second request, so it is exact and costs nothing
   twice.
9. More than 100 notes, an empty body with no picture, two notes of one path: the extra ones
   are dropped and counted in the footer. "One note" with several answered: they are joined
   under their titles as headings. "One per picture": there is no arrange request at all.
10. A picture in no note is listed under NOT USED, unticked, and **is not copied**.
11. Where a note goes is worked out by the library's own function from the group and the cleaned
    sections, and checked to be inside the library folder or the group's home, as every writer
    checks it. No text of a model is ever part of a path except through the name rule.

A hostile picture (one whose text says "ignore your instructions and…") can at worst make the
model write odd text into a note the owner then reads in the review before it is written. It
cannot name a file, reach a file, add a picture address or run anything.

## 9. Where the files go

**Decided: `pictures\` inside the folder the note is in**, so one per section folder. A note
`library\Screenshots\Sign-in\sign-in-fails-after-the-update.md` shows
`library\Screenshots\Sign-in\pictures\login-error.png` as `pictures/login-error.png`.

Why there and not one folder per group, or one per note:

- The link is the same short thing at every depth, with no `..` in it, and it is what image
  paste already writes for a picture beside its document.
- **"Move the notes…" already carries it.** A moved note takes the pictures only it links to,
  when they are inside its own folder (D-091, D-093). One folder for the whole group would put
  every picture outside that reach, and every move would rewrite links instead.
- A section folder copied to a stick or zipped takes its pictures with it.
- A folder per note doubles the folders for nothing the mover needs.

**Names.** "Name the copies after what they show" (on): the look's `title` through the note
name rule, with the original's extension: `login-error.png`. Off: the original's own name,
cleaned. A name that is taken: if the file there has the same bytes it is used and not written
again (the writer does this today, shared `runs.rs:1775`); otherwise `-2`, `-3` … `-99`.

**Copies are the originals, byte for byte**, full size, metadata and all. What is made smaller
is only what is sent. The originals are never moved, renamed or changed.

**"Copy the pictures beside the notes" off**: linked where they are, by a relative path when
they are on the note's drive, as image paste does (`image_paste::link_to_file`). The plan card
then says "the notes break if these pictures are moved". Not the default.

**A pasted screenshot** has no file: it is held in memory as the PNG the clipboard gave, is
sent like any picture, and is written straight into `pictures\` at the write.

Changes to the shared writer (rule 8, each a variant beside what exists): `NewPicture` gains an
optional name; the request gains the list of sources (it has one `source` today); `write_notes`
beside `write_note` for notes of a model's words. mdview's shots must not move.

## 10. Records and undo

A record of a new kind, `notes`, in `.runs` like the others. An older build lists it as
"Written by a newer version of mdedit" and never touches it (D-072). It holds, besides what a
split's holds: the sources' paths with their lengths and digests; per picture the copy's path,
`created`, its digest, **and the original's path and digest**; in `sent`, the number of pictures
and their bytes as sent. It does not hold the notes' text, the model's answers or any picture.

Undo follows 4.5 with one rule added, in the spirit of D-071:

- **A copy is removed only while its original is still where it was and is still the same
  file.** If the owner deleted the screenshots after the run, the copies in the library are the
  only ones, and undo keeps them and says so.
- The rest is as built: links out first; a note or copy only if this run created it, no library
  file lists it, and it is byte for byte what was written; a note changed since is kept, and so
  are the pictures it shows; folders the run made, if empty; the group's file, if the run made
  it and it is empty again. The question says the numbers (shot `undo`).

"File them" gets a record too (kind `filed`), so the next run of it can be undone: its links,
the copies it made, and a move in the mover's own form (4.3, which also has the one new thing
an undo would do: put a moved file back outside the library).

## 11. Limits and costs

| Limit | Value | Why |
|---|---|---|
| Pictures in one run | **100** | Twenty-five look requests, three at a time: a few minutes (an estimate, not measured). A list a person can still review. Past it the button is disabled with both numbers (shot `limit`) |
| Pictures in one look request | **4**, and 8 MB as sent | A failure loses four. Progress shows every few seconds. Far inside every provider's request size |
| A picture as sent | 3.5 MB; long side 1568 px, or 2576 px | 3.5 MB is about 4.7 MB as base64, under the 5 MB a provider takes per picture (the library plan's figure) |
| A picture on disk | 64 MiB read, 64 million pixels | The spike's caps |
| Text files in one run | 200 KB together | They go whole in the one arrange request; `REWRITE_MAX` (`task_ui.rs:253`) |
| Notes from one run | 100 | Half the split's 200: these are written by a model |
| A note | 200 KB | As one request |
| Copies | 100 files, 200 MB | 200 MB is the library plan's figure for a run |
| Requests at once | The connection's setting, 3; 1 for a local model | D-080 |

A limit that is hit is said in words with both numbers, and never cuts a run short silently.

**What it costs**, so the owner can judge the sizes. A picture is charged by its pixels, about
`width × height / 750` tokens on the Claude API (from memory of the provider's page; checked in
V0). So a 1568 × 882 screenshot is about 1,850 tokens, a 1568 × 1176 photo about 2,450, and the
same photo at 2576 px about 6,600. A hundred screenshots are about 200,000 tokens to look at,
plus the answers (about 150 tokens each), one arrange request of perhaps 30,000 and the notes it
writes. At the list prices in the API reference bundled with Claude Code (cached 2026-11-03:
Sonnet 5.5 $2 in and $10 out per million tokens, Opus 5.5 $4 and $20, Fable 5.2 $8 and $40),
that is roughly **$0.70 to $3 for a hundred pictures**, by my arithmetic and not by a run, and
a few cents for the owner's five. Through Claude Code or Codex it comes out of
the plan's allowance instead. Prices move and the program has no way to know them, so **the
window shows bytes and requests, which are exact, and never money**.

## 12. The options

| Option | Default | What it does |
|---|---|---|
| How many notes | As many as it takes | Or "One note", or "One per picture" (no second request) |
| Copy the pictures beside the notes | **On** | Off links them where they are |
| Name the copies after what they show | **On** | Off keeps the files' own names |
| Describe each picture | **On** | A sentence or two under it and the text in its brackets. Off: the pictures and the headings only |
| Write out the text in the pictures | Off | The text of each, word for word, as a quote under it. On in the prompt "Read the text in them" |
| Remove location and camera details before sending | **On** | Section 7 |
| Send larger pictures, for small print | Off | 2576 px, about 2.7 times the cost |
| Let it add to notes I already have | Off, and disabled until Settings › AI allows it | D-079. Phase A, not in the first build |

"File them" has one choice of its own, beside the task and not under Options: **THE FILES,
`Link them | Copy them in | Move them in`**, on "Copy them in" (4.3). And Settings › AI has
"After a run that writes notes" (4.4).

They are kept per task in the settings file, like the window's other choices. A saved prompt can
carry them (`notes: one`, `text: yes`), so a prompt is a whole recipe.

## 13. What leaves the machine, and how the window says so

Added to the table of 6.1 of the library plan:

| Command | Sent to the provider you chose | Not sent |
|---|---|---|
| Write notes, the look | **The pictures themselves**, made smaller where the row says, without their location and camera details unless you untick it; each one's file name; your instruction | The folders they are in. Any picture you unticked or that was left out |
| Write notes, the arrange | What the model said of each picture; the text files, whole; the group's headings; your instruction | Any picture |
| Write notes with the AI off | Nothing | |

Where it is said:

- The heading over the pictures: "PICTURES · SENT TO THE AI, AS EACH ROW SAYS · LOCATION AND
  CAMERA DETAILS REMOVED". The old heading, "names are sent, the pictures are not", stays for
  File them.
- The footer: "5 pictures, 1.5 MB as sent, and 1 text file · 3 requests", and under it the
  connection, the model and "the pictures themselves are sent".
- The button: "Send 5 pictures and 1 file".
- **"Show what is sent" becomes a card** (shot `sent`), because a picture cannot be shown in a
  text tab: the pictures with their thumbnails, as sent, and the literal text of each request.
  "Open the text in a tab" keeps what exists today.
- **The first time ever**, one more sentence above the button, once: "A picture is sent whole:
  everything visible in it, a password or a face included." The AI stays off by default; this
  task does nothing until a connection is chosen.

The Codex connection writes the prepared pictures to the temp folder for the length of the
request. That is said in Settings beside the connection, and the folder is deleted when the
process ends and looked for again when the engine next starts.

## 14. The two faults of today

**A picture line opens garbage.** `library_row` (and the palette's "Notes library" list, which
opens the same paths) asks one question first: is this a picture (`image_paste::is_image_file`,
SVG excepted, because that is text and the editor colours it) or one of the kinds the task
window already calls "other" (`task_ui::other_reason`: a PDF, a Word file, an archive)? Then it
is handed to the system with `platform::open_file`, and the toast says "Opened in the system's
viewer." Everything else opens in a tab as now. A file that cannot be handed over says why.
Half a day with its test, and worth doing first and alone: it fixes the lines the owner already
has.

**A picture as an entry of its own.** "File them" stops filing pictures. Under their heading
the rows are unticked and read "a picture is not a note: Write notes puts it inside one", and
when pictures are added while File them is chosen the window offers one line, "Write notes
from these instead", that switches the task. Other files (a PDF) are still linked by name, as
5.6 decided. The `with` of the filing answer stays in the reader for a build or two and is no
longer asked for. D-050's "listed in the library beside it" ends here; section 19.

The owner's existing picture lines: nothing removes them by itself. They come out with the
pane's Remove; the files are where they always were.

## 15. Failure cases

| What happens | What the owner sees | What is left |
|---|---|---|
| The model does not read pictures | "This model does not read pictures: {model} at {host}." and Open AI settings (shot `no-vision`) | Nothing written |
| A CLI connection that failed V0's try | The sentence of section 6 where the button is | Nothing sent |
| An older engine beside a newer mdedit | The sentence of section 6 | Text tasks work |
| One look request fails (rate limit, network, a refusal) | Which request, why, how long to wait; "Try it again", "Go on with N" (shot `failed`) | The other answers are kept |
| A look answer that cannot be read | The same, with "The answer could not be read" | The same |
| A picture changed between measuring and sending | "{name} changed since it was measured"; it is measured again | Nothing of it sent |
| The arrange answer cannot be read, or places nothing | "Try again" and "One note per picture instead" (which needs no request) | The look's answers |
| Stop | What was looked at is kept; the button comes back | Nothing written |
| Esc on the plan | The plan is kept and waits in the Notes pane ("Plan ready", D-053) | Nothing written |
| The window closed during the look | The requests are cancelled; a run never resumes from disk (D-080) | Nothing written |
| A copy cannot be written (disk full, a name too long) | In the report: "{picture} could not be copied: {why}"; its note is written with the line left out and says so | The record tells the truth |
| A note's name is taken, the group's file is open with changes, the group is full | Checked before step 1, the button disabled with the reason, as today (4.3) | Nothing written |
| The program dies during the write | "AI runs…": Finish or Undo, as today | A state one can live with |
| A picture is deleted before the write | That picture fails to copy, as above | |

## 16. Tests, shots, budgets

**Tests**, each written to fail with its fix reverted (rule 2). The fake engine gains a
`prepare` answer and two canned answers, `LOOK` and `ARRANGE`.

- *Engine*: a `run` with `images` puts image blocks before the text for Claude and `image_url`
  parts for OpenAI; the Claude Code line and the Codex arguments carry them; `sent` reports the
  bytes that went; a digest that does not match is refused and nothing is sent; `hello` says 2;
  the temp folder of a Codex run is gone afterwards; a 400 about images is `no_vision`. The
  resizer: the spike's tests, plus a sideways JPEG comes out upright, EXIF with a location is
  gone from what is sent and the pixels are unchanged, a BMP goes as a PNG, HEIC and SVG are
  refused with their words.
- *Core* (`notes_ai`): each of the eleven checks of section 8, one test each; no input panics
  (a fuzz-style table of broken JSON, as `import_ai` has); a body's foreign picture loses its
  `!`; a `javascript:` link is unlinked; "One per picture" builds its notes with no answer.
- *Window*: a picture's bytes are still never read by the editor itself; the requests are
  exactly what "What is sent" shows; unticked pictures are not sent; 101 pictures disable the
  button; an engine of version 1 disables pictures and leaves text tasks; a failed look request
  keeps the others; "Arrange again" sends no picture; Stop cancels every request; with the AI
  off nothing starts and the plan is composed.
- *Write and undo*: the copies are byte for byte the originals and in `pictures\` beside their
  note; a taken name becomes `-2`, the same bytes are reused; a picture in no note is not
  copied; the record holds each original's path and digest; undo removes copies and notes, and
  keeps a copy whose original is gone, and a note changed since with its pictures; an older
  record still reads.
- *The ways in* (4.1): one command; it opens on the pane's row when the pane has the keyboard,
  and empty otherwise, and says which; each line of Add adds what it names; the task it starts
  on follows what is in it.
- *One row* (4.2): a note's line takes that note; a section's takes every note under it, in a
  subsection and in a folded one too, and none of its neighbours'; a group's takes the group; a
  section with no note has the line disabled; the line's number is the number of rows the window
  opens with; nothing is opened in a tab.
- *Copy and move* (4.3): a copied file is byte for byte the original when no link needed
  rewriting; its pictures keep their relative place; a picture outside its folder is not taken
  and its link still reaches it; a link between two files of the run points at the new place; a
  taken name becomes `-2`; a file already in the library is only linked; another drive copies;
  undo of a move puts each file back only at its recorded path, only when that is free, and
  restores the rewritten links; undo of a copy keeps a picture whose original is gone.
- *After a run* (4.4): Write notes opens the first note and shows the preview; File them opens
  nothing and moves the pane; a rewritten file open in a tab holds the new text at once; "Open
  nothing" opens nothing.
- *The two faults*: a picture line calls the system's open and opens no tab; File them writes no
  line for a picture; File them leaves a record that undo reverses.

**Shots** added to `tests/shots.sh`, both themes, from `fixtures/notes-demo/` (five small
pictures made for it, and a text file), by `--task-demo`: `notes`, `notes-options`,
`notes-looking`, `notes-plan`, `notes-plan-one`, `notes-plan-markdown`, `notes-plan-rename`,
`notes-sent`, `notes-limit`, `notes-failed`, `notes-no-vision`, `notes-off` (the AI off),
`notes-first-time`; `--dialog run-undo-pictures`; `library-pane-notes-written`; and
`task-files-pictures` (File them, the pictures not filed); `task-add` (the Add menu),
`task-files-copy`, `task-files-move`, `task-files-copy-plan`;
`library-row-ai-note`, `library-row-ai-section`, `library-row-ai-group`, `task-from-section`;
`--dialog run-undo-moved`. Each looked at in both themes; the
real thumbnails, not the drawn ones of the mock.

**Budgets** (rule 1). Nothing here is on the launch path: the new modules are reached from the
command, the drop and the paste alone, and the engine is a separate program started when a
request is made. To be measured before and after with the method of `docs/CODE.md`, seven
interleaved runs: first frame, private memory, a keystroke, a scroll frame, idle CPU. Expected
and to be shown: no figure outside the runs' ranges.

- `mdedit.exe`: no new crate. Larger by the card, the core and the wiring; W added 102 KB, this
  is about twice the screens. A guess until built.
- `mdedit-ai.exe`: +593 KB in the spike (2,125,824 to 2,719,232 bytes), one new `Cargo.lock`
  package, `jpeg-encoder` 0.7.1 with no dependencies of its own; `zune-jpeg`, `png` and
  `image-webp` are in the lock already and become the engine's direct dependencies at the
  versions pinned there. Orientation and metadata are our own code.
- Memory while a plan is open: the thumbnails, 160 px, decoded for the rows in view and at most
  48 kept, about 4 MB at the worst, freed when the card closes. The originals are decoded only
  in the engine, one at a time (a 12-megapixel photo is 48 MB as pixels, for 143 ms).
- **Rule 7, said plainly**: one new dependency, `jpeg-encoder`, engine only. The owner has said
  yes to it; it is added in phase V1 and not before.

## 17. Phases

**What ships first**: four small pieces that need no change to the engine and no new crate, each
answering something the owner already ran into. Three and a half days together, in this order.

| Order | Phase | What ships | Days |
|---|---|---|---|
| 1 | F | The two faults (section 14): a picture line opens in the system's viewer; File them stops filing pictures and leaves a record | 1 |
| 2 | E | 4.1: one command, the Add menu, the tab's own entry, the starting task | 0.5 |
| 3 | L | 4.2: "AI task on this note / these N notes…" on a note, a section and a group of the Notes pane, and the command taking the keyboard's row | 1 |
| 4 | O | 4.4: the cause of "no preview after AI" found in a real window; what opens after each task; the setting | 1 |

**Then the pictures task**, in order, eighteen days less the day of F already counted above:

| Order | Phase | What ships | Days |
|---|---|---|---|
| 5 | V0 | One picture through each of the four connections, by hand, written down. Decides which connections get pictures | 0.5 |
| 6 | V1 | The engine: `VERSION` 2, `images`, `prepare`, `sent`, the resizer with orientation, metadata and BMP, the four senders, the two failure kinds; mdedit reads `hello` | 3.5 |
| 7 | V2 | The core: the two contracts, their readers, the composer, the eleven checks, names and collisions | 3 |
| 8 | V3 | The window: the fourth task, the options, picture rows with thumbnails and figures, the limits, the look with its progress and Stop, the paste, "What is sent" as a card | 3.5 |
| 9 | V4 | The plan card: tree, the note as it will read and as Markdown, ticks, rename, "Put in…", "Arrange again…", the kept plan | 3 |
| 10 | V5 | The write: the shared writer's variants, the record of kind `notes`, undo's rule for copies, the report, "AI runs…", "Again…" | 2.5 |
| 11 | V6 | The shots, the budgets measured, DESIGN, STATE, DECISIONS, the release notes | 1 |

**Then the files that travel** (4.3), which can also come before the pictures task: it needs
nothing of it.

| Order | Phase | What ships | Days |
|---|---|---|---|
| 12 | C1 | "Copy them in", with the pictures and the links | 2 |
| 13 | C2 | "Move them in", the shared variant, its record and undo | 2.5 |
| | | **Total** | **25** |

Later, each on its own yes, none counted above:

| Phase | What | Days |
|---|---|---|
| MS | 4.2: several rows selected in the Notes pane, with their menu and foot. Optional: "probably not necessary" | 3 |
| MF | 4.2: the same in the Files pane, mouse only, and "AI task on these files…" | 1.5 |
| A | Pictures and text into notes that already exist, behind D-079's setting, through the change list, the originals in the record | 3 |
| I | "Describe this picture" on an image link of the open document, through the review | 1.5 |
| S | "Sort this group again…" by AI, links only, with "Move the notes…" offered after | 3 |
| P | A drop of pictures onto a group in the Notes pane (with phase DD) | 1 |

The library plan priced phase V at 6.5 days. That was "pictures placed by what they show" in
"File them". This is a different thing: a task that writes notes, with its own review, and 18
days is its honest size. The owner's added points are seven more (E, L, O, C1, C2).

## 18. Questions for the owner

1. **Look, then arrange (two kinds of request), or everything in one request?** Recommended: two.
   Any number of pictures, a failure costs four, and trying the notes again sends no picture.
   One request is simpler to explain and writes the notes while seeing the pictures, but it
   stops working at a dozen or two pictures and a second try sends them all again.
2. **Are the pictures' file names sent?** Recommended: yes, the name only, never the folder.
   `Screenshot 2026-11-20 143022.png` tells the model the order. Say no if names can hold
   something private.
3. **A hundred pictures a run, four to a request, 1568 px.** Recommended as written. Say if you
   expect folders of several hundred: then the look should be kept on disk between runs, which
   D-080 forbids today and is two more days.
4. **Should "File them" refuse pictures altogether** (section 14)? Recommended: yes. It ends
   D-050's "listed beside it", which is the thing you called not useful.
5. **New notes of the model's words are reviewed as whole notes**, not change by change, because
   there is no "before". Recommended: accept; it is what "Markdown note from a file" does on its
   Result page. The other way is to open each note as an unsaved tab instead of writing it.
6. **"What is sent" as a card and not a tab.** Recommended: the card, with "Open the text in a
   tab" on it.

7. **One command, "AI task…", and the document in front as the first line of Add** (4.1).
   Recommended. The other way is to keep adding it by itself and say so in the window.
8. **Selecting several rows: later, or not at all?** Recommended: not until one row and the
   window's ticks have been used for a while. It is three days (and a day and a half more for
   the Files pane) for something you called probably not necessary.
9. **"File them" on "Copy them in" by default?** Recommended: yes. Nothing of yours changes and
   the library carries its files; the price is two copies of a text. Say "Link them" to keep
   today's behaviour as the default, with the switch one click away.
10. **May undoing a move put files back outside the library folder?** Recommended: yes, under
    the rule of 4.3 (the recorded path, only if free, only the same bytes). Without it a move
    in cannot be undone by the program, and I would then not build the move at all.
11. **After a run that writes notes, open the first one with its preview?** Recommended: yes,
    with the setting of 4.4 to turn it down.

## 19. Decisions this plan would take

Recorded as new entries in `docs/DECISIONS.md` on a yes, each naming what it replaces.

- Write notes is a fourth task; pictures are sent only by it, and only when the owner presses a
  button that says so. Completes D-078.
- A picture is never a library entry of its own; "File them" does not file pictures. **Replaces
  D-050** and the "listed in the library beside it" half of D-079.
- Copies go to `pictures\` in the note's own folder, byte for byte, named after what they show.
  Sharpens D-051.
- The look and the arrange are separate requests; the arrange holds no picture.
- The model places a picture by number; the app writes the path. No other picture address
  survives in a note the program writes.
- What is sent is made smaller in the engine and has its metadata removed by default; the copies
  are untouched. `jpeg-encoder` is added to the engine (D-078's condition met).
- `proto::VERSION` 2; mdedit reads the engine's version.
- Undo removes a copy only while its original is still there and the same.
- "File them" leaves a record.
- The window shows bytes and requests, never money.
- HEIC, AVIF, TIFF and SVG are not sent.
- One command opens the window; "AI task on this file…" stays only where the pointer names the
  file. Changes DESIGN 83's "Not built of 5.6".
- A note, a section or a group of the Notes pane has "AI task…" in its own menu; a section or a
  group means every note under it. Selecting several rows is not built until asked for.
- "File them" may copy or move the files it files, with the pictures inside their own folder,
  and copies by default. Widens 3.3 of the library plan ("everything a run creates is inside
  your library") from notes to the files themselves.
- Undoing a move in may write outside the library, at the recorded path only. **An exception to
  D-072**, named.
- After a run that writes notes the first opens with its preview; a rewritten file that is open
  is re-read at once.

## 20. What stays out, and what was not checked

**Out.** Reading text out of pictures on this PC (an OCR engine is a large dependency; the model
does it). Making or editing pictures. Video. PDFs as pictures. Watching a folder, or anything on
a timer: every request follows a button. Uploading a picture to a provider's file store: it
goes inside the one request and nowhere else. Tools for the model, in any connection. Prices in
the window. Moving or deleting the originals. Keeping a model's answers on disk.

**Not checked for this plan.**

- **"No preview shows up after AI" was not reproduced**, and nothing was run to try. 4.4 is
  from reading the code; the mdview-window mode of the preview was not traced.
- The mover's code was read for its rules (D-091, D-093) and its two entry points, not for
  whether a move from outside the library fits its record as it stands.
- No request was made to any provider, and neither command line was run with a picture. Their
  flags were read from their help text on this PC.
- The Claude API's per-picture size, pictures per request and token formula are from memory and
  from the library plan; only "1568 px on older models, 2576 px on current ones" was read, in
  the reference bundled with Claude Code. V0 reads the provider's page.
- What mdview does with raw HTML in a note.
- The times in section 11 are estimates. The spike's figures (an 8.2 MB photo to 1.95 MB in
  143 ms) are its commit message's, not re-run here.
- The mock's thumbnails are drawn shapes, not decoded pictures; the real ones were not tried.
- No budget was measured: nothing was built. The mock code is on this branch only and ships
  nowhere.
- Linux and macOS: nothing.
