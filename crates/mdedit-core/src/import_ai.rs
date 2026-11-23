//! "Sort a folder by AI" (`docs/ai-library-plan.md` 5.7, phase 4): the two requests of the AI
//! step on the import card, the checks on what comes back, and the plan they make.
//!
//! The first request (the scheme) sends each file's path inside the folder, its title and the
//! start of its text, and asks for at most [`MAX_SECTIONS`] sections with at most [`MAX_SUBS`]
//! subsections each. The owner sees that scheme before anything else is asked and can rename,
//! delete or add sections. The second pass sends the files in batches of [`BATCH`] and asks for
//! one numbered section per file, with a confidence and a one-line reason.
//!
//! Nothing here reads or writes the disk: the walk (`crate::import`) has already read each file's
//! start, and the prompts are built from it. Nothing here can panic on what a model wrote: every
//! field is optional when read, every number is checked before it is used, and the JSON reader is
//! depth-bounded. A section number the scheme does not have, an id that was not sent, and a file
//! the answer leaves out all land in "Unsorted", unticked; the AI never writes, it only proposes
//! the headings the import's own writer puts the links under.

use crate::import::{self, Note, Plan};

/// What is sent in the first request: short, because Claude Code takes it on the command line.
pub const SYSTEM: &str = "You sort a folder of notes into sections for a notes library. You get \
one line per file: its path in the folder, its title, and the start of its text. Answer with one \
JSON object and nothing else: {\"version\":1,\"sections\":[{\"name\":\"\u{2026}\",\"subs\":[\"\u{2026}\"]}]}. \
At most 8 sections, each with at most 4 subsections. A section's name is one line of at most 120 \
characters. Do not include any text of the files.";

/// What is sent in the filing requests.
pub const FILE_SYSTEM: &str = "You put each file of a folder under one of the numbered sections \
the application gives. Answer with one JSON object and nothing else: \
{\"version\":1,\"files\":[{\"id\":N,\"section\":N,\"title\":\"\u{2026}\",\"confidence\":0.8,\"why\":\"\u{2026}\"}]}. \
`id` is a file's number and `section` one of the numbered sections. `title` is optional: use it \
only when the file's title should change. `confidence` is from 0 to 1. `why` is one short line. \
Do not include any text of the files.";

/// The most files one AI sort takes; the rest keep their folders (5.7).
pub const MAX_AI_FILES: usize = 500;
/// The files one filing request asks about.
pub const BATCH: usize = 40;
/// The most sections the scheme may hold.
pub const MAX_SECTIONS: usize = 8;
/// The most subsections one section may hold.
pub const MAX_SUBS: usize = 4;
/// The confidence at and above which "Tick only the sure ones" ticks a note (5.7).
pub const SURE: f32 = 0.6;
/// The most a section name or a title is kept at, in bytes.
pub const NAME_MAX: usize = 120;
/// The most of a file's path inside the folder that is sent.
pub const PATH_MAX: usize = 200;
/// The most of a `why` that is shown, in bytes.
pub const WHY_MAX: usize = 200;
/// The most of a file's start the scheme request sends (5.7).
pub const START_SENT: usize = 120;
/// The most of a file's text one digest carries: with its path and title, a 2 KB line.
pub const DIGEST_MAX: usize = 1200;
/// The most one request's text may be, as a rewrite's.
pub const PROMPT_MAX: usize = 200_000;

// ---- the scheme ---------------------------------------------------------------------------------

/// One top-level section of the scheme, with the subsections under it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub subs: Vec<String>,
}

/// The sections the model proposed, as the owner sees and edits them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scheme {
    pub sections: Vec<Section>,
    /// How many sections or subsections the answer held past the caps, or that were empty or
    /// twice: the card says so above the list.
    pub dropped: usize,
}

/// One row of the scheme card: a top-level section or a subsection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemeRow {
    /// The heading path the notes filed here are written under: one or two names.
    pub path: Vec<String>,
    pub name: String,
    /// A top-level section, which a deleted row takes its subsections with.
    pub top: bool,
}

impl Scheme {
    /// Every row in the order the card shows them: each section, then its subsections.
    pub fn rows(&self) -> Vec<SchemeRow> {
        let mut out = Vec::new();
        for section in &self.sections {
            out.push(SchemeRow {
                path: vec![section.name.clone()],
                name: section.name.clone(),
                top: true,
            });
            for sub in &section.subs {
                out.push(SchemeRow {
                    path: vec![section.name.clone(), sub.clone()],
                    name: sub.clone(),
                    top: false,
                });
            }
        }
        out
    }

    /// The heading path of every row, for numbering the filing request.
    pub fn paths(&self) -> Vec<Vec<String>> {
        self.rows().into_iter().map(|r| r.path).collect()
    }

    /// The index of the section a row belongs to.
    fn top_of(&self, row: usize) -> Option<(usize, Option<usize>)> {
        let mut at = 0usize;
        for (i, section) in self.sections.iter().enumerate() {
            if row == at {
                return Some((i, None));
            }
            at += 1;
            for j in 0..section.subs.len() {
                if row == at {
                    return Some((i, Some(j)));
                }
                at += 1;
            }
        }
        None
    }

    /// The row's own name, as the list shows it.
    pub fn row_name(&self, row: usize) -> Option<&str> {
        match self.top_of(row)? {
            (i, None) => Some(self.sections.get(i)?.name.as_str()),
            (i, Some(j)) => Some(self.sections.get(i)?.subs.get(j)?.as_str()),
        }
    }

    /// Renames the row; the name is cleaned as a heading (one line, a closing `#` off, at most
    /// [`NAME_MAX`] bytes). An empty name is kept, so the card can say a section needs one.
    pub fn rename(&mut self, row: usize, name: &str) -> bool {
        let Some((i, sub)) = self.top_of(row) else {
            return false;
        };
        let name = clean_name(name);
        match (sub, self.sections.get_mut(i)) {
            (None, Some(section)) => section.name = name,
            (Some(j), Some(section)) if j < section.subs.len() => section.subs[j] = name,
            _ => return false,
        }
        true
    }

    /// Deletes the row. A top-level section takes its subsections with it; the last row is never
    /// deleted, so the scheme always has one.
    pub fn delete(&mut self, row: usize) {
        if self.rows().len() <= 1 {
            return;
        }
        let Some((i, sub)) = self.top_of(row) else {
            return;
        };
        match sub {
            None => {
                self.sections.remove(i);
            }
            Some(j) => {
                if let Some(section) = self.sections.get_mut(i)
                    && j < section.subs.len()
                {
                    section.subs.remove(j);
                }
            }
        }
    }

    /// Adds a plain section at the end and returns its row.
    pub fn add(&mut self, name: &str) -> usize {
        let row = self.rows().len();
        self.sections.push(Section {
            name: clean_name(name),
            subs: Vec::new(),
        });
        row
    }

    /// Why the scheme cannot be sent, or `None` when it can: a row with no name, two rows with
    /// the same path, a name the library would read back differently, or more than
    /// [`MAX_SECTIONS`] top-level sections.
    pub fn problem(&self) -> Option<String> {
        if self.sections.is_empty() {
            return Some("Add a section first.".to_string());
        }
        if self.sections.len() > MAX_SECTIONS {
            return Some(format!(
                "At most {MAX_SECTIONS} sections: there are {}.",
                self.sections.len()
            ));
        }
        let rows = self.rows();
        if rows.iter().any(|r| r.name.trim().is_empty()) {
            return Some("A section needs a name.".to_string());
        }
        let mut seen: Vec<String> = Vec::new();
        for row in &rows {
            let key = row
                .path
                .iter()
                .map(|s| s.to_lowercase())
                .collect::<Vec<_>>()
                .join("\u{1F}");
            if seen.contains(&key) {
                return Some(format!(
                    "Two sections are called \u{201c}{}\u{201d}.",
                    row.name
                ));
            }
            seen.push(key);
            if !reads_back(&row.name) {
                return Some(format!(
                    "\u{201c}{}\u{201d} would not read back from the library.",
                    row.name
                ));
            }
        }
        None
    }

    /// The scheme in the request's words: one numbered row per section path.
    pub fn numbered(&self) -> String {
        let mut out = String::new();
        for (i, path) in self.paths().into_iter().enumerate() {
            out.push_str(&format!("{i} {}\n", path.join(" > ")));
        }
        out
    }
}

// ---- what is sent -------------------------------------------------------------------------------

/// The scheme request: every file's path inside the folder, its title and the start of its text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemeAsk {
    pub text: String,
    pub files: usize,
    /// The files past [`MAX_AI_FILES`], which keep their folders.
    pub more: usize,
}

impl SchemeAsk {
    pub fn bytes(&self) -> usize {
        SYSTEM.len().saturating_add(self.text.len())
    }
}

/// The files an AI sort takes: every note the group does not list already, in the plan's order,
/// up to [`MAX_AI_FILES`].
pub fn ai_notes(plan: &Plan) -> Vec<usize> {
    plan.notes
        .iter()
        .enumerate()
        .filter(|(_, n)| !n.already)
        .map(|(i, _)| i)
        .take(MAX_AI_FILES)
        .collect()
}

/// The scheme request over the plan, or the reason it cannot be built: a folder with no file to
/// sort, or names so long that even the short form is over [`PROMPT_MAX`].
pub fn scheme_ask(plan: &Plan) -> Result<SchemeAsk, String> {
    let ids = ai_notes(plan);
    if ids.is_empty() {
        return Err("There is nothing here for the AI to sort.".to_string());
    }
    let more = plan
        .notes
        .iter()
        .filter(|n| !n.already)
        .count()
        .saturating_sub(ids.len());
    // The start of each file is sent in full; if that makes the request too long, its first
    // forty bytes; if even that does, it is refused rather than sent over the limit.
    for start in [START_SENT, 40, 0] {
        let mut text =
            String::from("Sort these files into sections for a notes library.\n<files>\n");
        for (id, &i) in ids.iter().enumerate() {
            let Some(note) = plan.notes.get(i) else {
                continue;
            };
            text.push_str(&format!(
                "<file id=\"{id}\" path=\"{}\" title=\"{}\">{}</file>\n",
                attr(&relative(&note.path, &plan.folder), PATH_MAX),
                attr(&note.title, NAME_MAX),
                one_line(&note.start, start)
            ));
        }
        text.push_str("</files>");
        if text.len() <= PROMPT_MAX {
            return Ok(SchemeAsk {
                text,
                files: ids.len(),
                more,
            });
        }
    }
    Err(format!(
        "The paths and titles in this folder are too long to send in one request of {} KB.",
        PROMPT_MAX / 1000
    ))
}

/// One filing request: the scheme's numbered sections and the files of one batch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// The notes of the plan the file ids 0, 1, 2… stand for.
    pub ids: Vec<usize>,
    pub text: String,
}

impl Batch {
    pub fn bytes(&self) -> usize {
        FILE_SYSTEM.len().saturating_add(self.text.len())
    }
}

/// The filing requests, in order: batches of [`BATCH`] over the same files the scheme asked
/// about.
pub fn batches(plan: &Plan, scheme: &Scheme) -> Vec<Batch> {
    let ids = ai_notes(plan);
    let numbered = scheme.numbered();
    ids.chunks(BATCH)
        .map(|chunk| {
            let mut text =
                String::from("Put each of these files under one numbered section.\n<sections>\n");
            text.push_str(&numbered);
            text.push_str("</sections>\n<files>\n");
            for (id, &i) in chunk.iter().enumerate() {
                let Some(note) = plan.notes.get(i) else {
                    continue;
                };
                let modified = if note.modified.is_empty() {
                    String::new()
                } else {
                    format!(" modified=\"{}\"", attr(&note.modified, 32))
                };
                text.push_str(&format!(
                    "<file id=\"{id}\" path=\"{}\" title=\"{}\" size=\"{}\"{modified}>{}</file>\n",
                    attr(&relative(&note.path, &plan.folder), PATH_MAX),
                    attr(&note.title, NAME_MAX),
                    note.bytes,
                    one_line(&note.start, DIGEST_MAX)
                ));
            }
            text.push_str("</files>");
            Batch {
                ids: chunk.to_vec(),
                text,
            }
        })
        .collect()
}

/// What the AI step would send, for the card's line before it starts: bytes and requests.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Estimate {
    pub files: usize,
    pub requests: usize,
    pub bytes: usize,
}

/// The estimate of a run: the scheme request and one filing request per batch. It builds the
/// real texts, so the figure is the one the run will send, give or take the sections' names.
pub fn estimate(plan: &Plan) -> Estimate {
    let Ok(ask) = scheme_ask(plan) else {
        return Estimate::default();
    };
    let ids = ai_notes(plan);
    let mut bytes = ask.bytes();
    let mut requests = 1usize;
    for chunk in ids.chunks(BATCH) {
        let mut text =
            String::from("Put each of these files under one numbered section.\n<sections>\n");
        // About what eight sections of average names add.
        text.push_str("0 Example section\n1 Another section\n");
        text.push_str("</sections>\n<files>\n");
        for (id, &i) in chunk.iter().enumerate() {
            let Some(note) = plan.notes.get(i) else {
                continue;
            };
            text.push_str(&format!(
                "<file id=\"{id}\" path=\"{}\" title=\"{}\" size=\"{}\">{}</file>\n",
                attr(&relative(&note.path, &plan.folder), PATH_MAX),
                attr(&note.title, NAME_MAX),
                note.bytes,
                one_line(&note.start, DIGEST_MAX)
            ));
        }
        text.push_str("</files>");
        bytes = bytes
            .saturating_add(FILE_SYSTEM.len())
            .saturating_add(text.len());
        requests += 1;
    }
    Estimate {
        files: ids.len(),
        requests,
        bytes,
    }
}

/// A file's place in the folder it is in, as the request says it: with `/` between its parts on
/// Windows too, so a prompt reads the same on every machine.
pub fn relative(path: &std::path::Path, folder: &std::path::Path) -> String {
    let shown = path.strip_prefix(folder).unwrap_or(path);
    let text = shown.to_string_lossy();
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    }
}

/// `text` as one line for a request: control characters as spaces, the end trimmed, at most
/// `max` bytes cut at a character.
fn one_line(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max));
    for c in text.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if out.len().saturating_add(c.len_utf8()) > max {
            break;
        }
        out.push(c);
    }
    out.trim_end().to_string()
}

/// A value inside an attribute: the characters that would end it or the tag become `_`.
fn attr(text: &str, max: usize) -> String {
    let mut out = String::with_capacity(text.len().min(max));
    for c in text.chars() {
        let c = if c.is_control() || matches!(c, '"' | '<' | '>') {
            '_'
        } else {
            c
        };
        if out.len().saturating_add(c.len_utf8()) > max {
            break;
        }
        out.push(c);
    }
    out.trim_end().to_string()
}

// ---- what comes back ----------------------------------------------------------------------------

/// Why an answer could not be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    NoObject,
    NotJson,
    Version,
    NoSections,
    NoFiles,
}

impl Fault {
    pub fn text(self) -> &'static str {
        match self {
            Fault::NoObject => "it held no JSON object",
            Fault::NotJson => "its JSON object did not parse",
            Fault::Version => "its \"version\" was not 1",
            Fault::NoSections => "it had no usable sections",
            Fault::NoFiles => "it had no \"files\" array",
        }
    }
}

/// One file as the model filed it, after the checks.
#[derive(Clone, Debug, PartialEq)]
pub struct Filed {
    /// The file's place in the batch that was sent.
    pub id: usize,
    /// The section's number in the scheme, or `None` for one the scheme does not have (which
    /// lands in "Unsorted").
    pub section: Option<usize>,
    /// A new title, cleaned; `None` keeps the file's own.
    pub title: Option<String>,
    pub confidence: f32,
    pub why: String,
}

/// Reads the scheme answer: the first balanced object, which must parse, with a `version` of 1
/// or none and a `sections` array. At most [`MAX_SECTIONS`] sections are kept and at most
/// [`MAX_SUBS`] subsections under each; empty and twice-named ones are dropped and counted.
pub fn read_scheme(text: &str) -> Result<Scheme, Fault> {
    let object = first_object(text).ok_or(Fault::NoObject)?;
    let json = parse_json(object).ok_or(Fault::NotJson)?;
    if !matches!(json, Json::Obj(_)) {
        return Err(Fault::NotJson);
    }
    if let Some(v) = json.get("version")
        && v.num() != Some(1.0)
    {
        return Err(Fault::Version);
    }
    let list = json
        .get("sections")
        .and_then(Json::arr)
        .ok_or(Fault::NoSections)?;
    let mut scheme = Scheme::default();
    for entry in list {
        if scheme.sections.len() >= MAX_SECTIONS {
            scheme.dropped += 1;
            continue;
        }
        let Some(name) = entry
            .get("name")
            .and_then(Json::str)
            .map(clean_name)
            .filter(|n| !n.is_empty())
        else {
            scheme.dropped += 1;
            continue;
        };
        if scheme
            .sections
            .iter()
            .any(|s| s.name.eq_ignore_ascii_case(&name))
        {
            scheme.dropped += 1;
            continue;
        }
        let mut subs: Vec<String> = Vec::new();
        if let Some(list) = entry.get("subs").and_then(Json::arr) {
            for sub in list {
                if subs.len() >= MAX_SUBS {
                    scheme.dropped += 1;
                    continue;
                }
                let Some(sub) = sub
                    .str()
                    .map(clean_name)
                    .filter(|n| !n.is_empty() && !n.eq_ignore_ascii_case(&name))
                else {
                    scheme.dropped += 1;
                    continue;
                };
                if subs.iter().any(|s| s.eq_ignore_ascii_case(&sub)) {
                    scheme.dropped += 1;
                    continue;
                }
                subs.push(sub);
            }
        }
        scheme.sections.push(Section { name, subs });
    }
    if scheme.sections.is_empty() {
        return Err(Fault::NoSections);
    }
    Ok(scheme)
}

/// The answer as read, before it is settled on the plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Answer {
    pub files: Vec<Filed>,
}

/// Reads a filing answer. `ids` are the notes the batch's file numbers stand for, and
/// `sections` is how many the scheme has. An id that was not sent is ignored, and a second use of
/// one is ignored after the first; a section number outside the scheme is `None` (Unsorted); a
/// confidence that is not a number is 0, and one outside 0 to 1 is clamped.
pub fn read_files(text: &str, ids: &[usize], sections: usize) -> Result<Answer, Fault> {
    let object = first_object(text).ok_or(Fault::NoObject)?;
    let json = parse_json(object).ok_or(Fault::NotJson)?;
    if !matches!(json, Json::Obj(_)) {
        return Err(Fault::NotJson);
    }
    if let Some(v) = json.get("version")
        && v.num() != Some(1.0)
    {
        return Err(Fault::Version);
    }
    let list = json
        .get("files")
        .and_then(Json::arr)
        .ok_or(Fault::NoFiles)?;
    let mut answer = Answer::default();
    for entry in list {
        let Some(id) = entry.get("id").and_then(Json::whole) else {
            continue;
        };
        if id >= ids.len() || answer.files.iter().any(|f| f.id == id) {
            continue;
        }
        let section = entry
            .get("section")
            .and_then(Json::whole)
            .filter(|&s| s < sections);
        let title = entry
            .get("title")
            .and_then(Json::str)
            .map(clean_title)
            .filter(|t| !t.is_empty());
        let confidence = entry
            .get("confidence")
            .and_then(Json::num)
            .map(|c| c.clamp(0.0, 1.0) as f32)
            .unwrap_or(0.0);
        let why = entry
            .get("why")
            .and_then(Json::str)
            .map(|w| first_line(w, WHY_MAX))
            .unwrap_or_default();
        answer.files.push(Filed {
            id,
            section,
            title,
            confidence,
            why,
        });
    }
    Ok(answer)
}

// ---- settling the answers on the plan -----------------------------------------------------------

/// What the AI did, for the card's line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sorted {
    /// The notes put under one of the AI's sections.
    pub placed: usize,
    /// The notes that landed in "Unsorted": the answer left them out, or named a section the
    /// scheme does not have.
    pub unsorted: usize,
}

/// Puts the answers on the plan: each filed note's sections and title, its confidence and why.
/// When every batch answered (`complete`), a note no answer placed is moved to "Unsorted",
/// unticked; when a request failed, the notes it never reached keep their folders and their
/// ticks, so the rest of the plan is still usable. The rows are made again in the sections'
/// order and the note's own title.
pub fn settle(
    plan: &mut Plan,
    scheme: &Scheme,
    batches: &[Batch],
    answers: &[Option<Answer>],
    complete: bool,
) -> Sorted {
    let paths = scheme.paths();
    let ai: Vec<bool> = {
        let ids = ai_notes(plan);
        let mut at = vec![false; plan.notes.len()];
        for i in ids {
            if let Some(flag) = at.get_mut(i) {
                *flag = true;
            }
        }
        at
    };
    let mut answered: Vec<bool> = vec![false; plan.notes.len()];
    let mut out = Sorted::default();
    for (b, answer) in answers.iter().enumerate() {
        let Some(batch) = batches.get(b) else {
            continue;
        };
        let Some(answer) = answer else {
            continue;
        };
        for filed in &answer.files {
            let Some(&note_i) = batch.ids.get(filed.id) else {
                continue;
            };
            let Some(note) = plan.notes.get_mut(note_i) else {
                continue;
            };
            answered[note_i] = true;
            match filed.section.and_then(|s| paths.get(s).cloned()) {
                Some(path) => {
                    note.sections = path;
                    note.unsorted = false;
                    note.confidence = Some(filed.confidence);
                    note.why = filed.why.clone();
                    if let Some(title) = &filed.title {
                        note.title = title.clone();
                    }
                    out.placed += 1;
                }
                None => {
                    note.sections = vec![UNSORTED.to_string()];
                    note.unsorted = true;
                    note.ticked = false;
                    note.confidence = Some(0.0);
                    note.why = "the AI named a section the scheme does not have".to_string();
                    out.unsorted += 1;
                }
            }
        }
    }
    if complete {
        for (i, note) in plan.notes.iter_mut().enumerate() {
            if !note.already && ai.get(i).copied().unwrap_or(false) && !answered[i] {
                note.sections = vec![UNSORTED.to_string()];
                note.unsorted = true;
                note.ticked = false;
                note.confidence = Some(0.0);
                note.why = "the AI did not place this one".to_string();
                out.unsorted += 1;
            }
        }
    }
    reorder(plan, scheme);
    plan.rows = import::rows_of(&plan.notes);
    out
}

/// The heading the notes the AI could not place go under.
pub const UNSORTED: &str = "Unsorted";

/// Whether two heading paths are the same names, without regard to case.
fn same_path_names(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

/// Orders the notes as the card shows them after a sort: the scheme's sections in the order the
/// scheme has them, then the files the group already lists, then "Unsorted"; by title within a
/// section.
fn reorder(plan: &mut Plan, scheme: &Scheme) {
    let paths = scheme.paths();
    let rank = |note: &Note| -> (usize, usize) {
        if note.unsorted {
            return (paths.len() + 2, 0);
        }
        if let Some(i) = paths
            .iter()
            .position(|p| same_path_names(p, &note.sections))
        {
            return (i, 0);
        }
        if note.already {
            return (paths.len() + 1, 0);
        }
        (paths.len() + 1, 1)
    };
    plan.notes.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.title.cmp(&b.title))
            .then_with(|| a.path.cmp(&b.path))
    });
}

/// "Tick only the sure ones": ticks the notes the AI placed with a confidence of [`SURE`] or
/// more, and unticks the rest; a file the group lists already stays unticked.
pub fn tick_sure(plan: &mut Plan) {
    for note in &mut plan.notes {
        note.ticked = !note.already && !note.unsorted && note.confidence.is_some_and(|c| c >= SURE);
    }
    plan.rows = import::rows_of(&plan.notes);
}

// ---- names --------------------------------------------------------------------------------------

/// A section's name as the library must read it: one line, control characters as spaces, a
/// closing run of `#` taken off (the library's reader takes it off anyway), at most
/// [`NAME_MAX`] bytes at a character.
pub fn clean_name(text: &str) -> String {
    let line = one_line(text, NAME_MAX.saturating_mul(4));
    let trimmed = line.trim_end_matches('#').trim_end();
    let mut out = String::with_capacity(trimmed.len().min(NAME_MAX));
    for c in trimmed.chars() {
        if out.len().saturating_add(c.len_utf8()) > NAME_MAX {
            break;
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// A title as the note line takes it: as [`clean_name`], with the brackets a link cannot hold.
pub fn clean_title(text: &str) -> String {
    clean_name(text).replace(['[', ']'], "").trim().to_string()
}

/// `text`'s first line, controls as spaces, at most `max` bytes at a character.
fn first_line(text: &str, max: usize) -> String {
    let line = text.split(['\n', '\r']).next().unwrap_or("");
    one_line(line, max)
}

/// Whether a heading's text reads back as itself (the import's own rule for a section).
fn reads_back(text: &str) -> bool {
    let written = text.trim();
    !written.is_empty() && written.trim_end_matches('#').trim() == written
}

// ---- JSON --------------------------------------------------------------------------------------
//
// A small reader, as the run records' own: enough for the two answers and no more. Nothing here
// recurses without a depth bound, and every index is checked, because the release build of the
// app aborts on a panic and a model's answer is text from outside.

#[derive(Debug)]
enum Json {
    Null,
    Bool,
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(items) => items.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }

    fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) if n.is_finite() => Some(*n),
            _ => None,
        }
    }

    /// A whole number that is 0 or more, as a row or file number.
    fn whole(&self) -> Option<usize> {
        let n = self.num()?;
        (n >= 0.0 && n.fract() == 0.0 && n <= usize::MAX as f64).then_some(n as usize)
    }

    fn arr(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(items) => Some(items),
            _ => None,
        }
    }
}

const JSON_DEPTH: u8 = 32;

/// The first balanced `{…}` in `text`, braces inside strings not counted.
fn first_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let b = text.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    let mut i = start;
    while let Some(&c) = b.get(i) {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_str = false;
            }
        } else {
            match c {
                b'"' => in_str = true,
                b'{' => depth = depth.saturating_add(1),
                b'}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return text.get(start..=i);
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn parse_json(text: &str) -> Option<Json> {
    let mut r = Reader {
        b: text.as_bytes(),
        at: 0,
    };
    let value = r.value(JSON_DEPTH)?;
    r.skip();
    Some(value)
}

struct Reader<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn skip(&mut self) {
        while let Some(&c) = self.b.get(self.at) {
            if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, c: u8) -> bool {
        self.skip();
        if self.b.get(self.at) == Some(&c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self, word: &str) -> bool {
        self.skip();
        let end = self.at.saturating_add(word.len());
        if self.b.get(self.at..end) == Some(word.as_bytes()) {
            self.at = end;
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: u8) -> Option<Json> {
        if depth == 0 {
            return None;
        }
        self.skip();
        match *self.b.get(self.at)? {
            b'n' => self.word("null").then_some(Json::Null),
            b't' => self.word("true").then_some(Json::Bool),
            b'f' => self.word("false").then_some(Json::Bool),
            b'"' => self.string().map(Json::Str),
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                self.skip();
                if self.eat(b']') {
                    return Some(Json::Arr(items));
                }
                loop {
                    items.push(self.value(depth - 1)?);
                    if self.eat(b']') {
                        return Some(Json::Arr(items));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            b'{' => {
                self.at += 1;
                let mut items: Vec<(String, Json)> = Vec::new();
                self.skip();
                if self.eat(b'}') {
                    return Some(Json::Obj(items));
                }
                loop {
                    self.skip();
                    if self.b.get(self.at) != Some(&b'"') {
                        return None;
                    }
                    let key = self.string()?;
                    if !self.eat(b':') {
                        return None;
                    }
                    let value = self.value(depth - 1)?;
                    items.push((key, value));
                    if self.eat(b'}') {
                        return Some(Json::Obj(items));
                    }
                    if !self.eat(b',') {
                        return None;
                    }
                }
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<Json> {
        self.skip();
        let start = self.at;
        if self.b.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        let mut digits = false;
        while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
            digits = true;
        }
        if self.b.get(self.at) == Some(&b'.') {
            self.at += 1;
            while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        if matches!(self.b.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.b.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            while self.b.get(self.at).is_some_and(|c| c.is_ascii_digit()) {
                self.at += 1;
            }
        }
        if !digits {
            return None;
        }
        let text = std::str::from_utf8(self.b.get(start..self.at)?).ok()?;
        text.parse().ok().map(Json::Num)
    }

    fn string(&mut self) -> Option<String> {
        if self.b.get(self.at) != Some(&b'"') {
            return None;
        }
        self.at += 1;
        let mut out = String::new();
        loop {
            let c = *self.b.get(self.at)?;
            self.at += 1;
            match c {
                b'"' => return Some(out),
                b'\\' => {
                    let e = *self.b.get(self.at)?;
                    self.at += 1;
                    match e {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hi = self.hex4()?;
                            let ch = if (0xD800..0xDC00).contains(&hi) {
                                let save = self.at;
                                if self.b.get(self.at) == Some(&b'\\')
                                    && self.b.get(self.at + 1) == Some(&b'u')
                                {
                                    self.at += 2;
                                    match self.hex4() {
                                        Some(lo) if (0xDC00..0xE000).contains(&lo) => {
                                            char::from_u32(
                                                0x1_0000 + ((hi - 0xD800) << 10) + (lo - 0xDC00),
                                            )
                                            .unwrap_or('\u{FFFD}')
                                        }
                                        _ => {
                                            self.at = save;
                                            '\u{FFFD}'
                                        }
                                    }
                                } else {
                                    '\u{FFFD}'
                                }
                            } else {
                                char::from_u32(hi).unwrap_or('\u{FFFD}')
                            };
                            out.push(ch);
                        }
                        _ => return None,
                    }
                }
                c if c < 0x20 => return None,
                c if c < 0x80 => out.push(c as char),
                _ => {
                    let len = if c < 0xE0 {
                        2
                    } else if c < 0xF0 {
                        3
                    } else {
                        4
                    };
                    let from = self.at - 1;
                    let end = from + len;
                    let bytes = self.b.get(from..end)?;
                    out.push_str(std::str::from_utf8(bytes).ok()?);
                    self.at = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Option<u32> {
        let mut n = 0u32;
        for _ in 0..4 {
            let c = *self.b.get(self.at)?;
            self.at += 1;
            n = n * 16 + (c as char).to_digit(16)?;
        }
        Some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A plan in memory: `n` notes, all ticked, in one folder. Nothing is on disk.
    fn plan_with(n: usize) -> Plan {
        let folder = PathBuf::from("/notes/Recipes");
        let library = PathBuf::from("/notes/library");
        let notes: Vec<Note> = (0..n)
            .map(|i| Note {
                path: folder.join(format!("n{i}.md")),
                sections: Vec::new(),
                title: format!("Note {i}"),
                head: vec![format!("Note {i}")],
                start: format!("text of note {i}"),
                bytes: 10,
                modified: "2026-11-23".to_string(),
                ticked: true,
                already: false,
                confidence: None,
                why: String::new(),
                unsorted: false,
            })
            .collect();
        Plan {
            folder,
            group_name: "Recipes".to_string(),
            library,
            group_file: PathBuf::from("/notes/library/Recipes.md"),
            group_new: true,
            home: PathBuf::from("/notes/Recipes"),
            rows: import::rows_of(&notes),
            notes,
            entries: 0,
            folders: 1,
            more: false,
            blocked: None,
        }
    }

    fn scheme_of(names: &[&str]) -> Scheme {
        Scheme {
            sections: names
                .iter()
                .map(|n| Section {
                    name: n.to_string(),
                    subs: Vec::new(),
                })
                .collect(),
            dropped: 0,
        }
    }

    fn filed(id: usize, section: Option<usize>, confidence: f32) -> Filed {
        Filed {
            id,
            section,
            title: None,
            confidence,
            why: format!("reason {id}"),
        }
    }

    #[test]
    fn the_scheme_keeps_eight_sections_and_four_subsections_and_counts_the_rest() {
        let text = r#"Here it is: {"version":1,"sections":[
            {"name":"A","subs":["a1","a2","a3","a4","a5"]},
            {"name":"B","subs":["b1"]},
            {"name":"C"},{"name":"D"},{"name":"E"},{"name":"F"},{"name":"G"},
            {"name":"H"},{"name":"I"},{"name":"J"}]}"#;
        let s = read_scheme(text).expect("a scheme");
        assert_eq!(s.sections.len(), MAX_SECTIONS);
        assert_eq!(s.sections[0].subs.len(), MAX_SUBS);
        // One subsection past the cap, two sections past it.
        assert_eq!(s.dropped, 3);
        // A second name in another case is the section there is.
        let text = r#"{"sections":[{"name":"Pasta"},{"name":"pasta"},{"name":""},{"name":"  "}]}"#;
        let s = read_scheme(text).expect("a scheme");
        assert_eq!(s.sections.len(), 1);
        assert_eq!(s.dropped, 3);
        // Nothing usable is not a scheme.
        assert_eq!(read_scheme(r#"{"sections":[]}"#), Err(Fault::NoSections));
        assert_eq!(read_scheme("no object"), Err(Fault::NoObject));
        assert_eq!(
            read_scheme(r#"{"version":2,"sections":[]}"#),
            Err(Fault::Version)
        );
    }

    #[test]
    fn a_section_name_is_cleaned_as_the_library_reads_it() {
        assert_eq!(clean_name("  Pasta  "), "Pasta");
        assert_eq!(clean_name("C#"), "C");
        assert_eq!(clean_name("Two\nlines"), "Two lines");
        assert_eq!(clean_name(&"x".repeat(200)).len(), NAME_MAX);
        assert_eq!(clean_title("A [long] title"), "A long title");
        assert_eq!(
            clean_name("Pasta #"),
            "Pasta",
            "a closing run of # comes off"
        );
    }

    #[test]
    fn a_file_id_not_sent_is_ignored_and_the_first_use_wins() {
        let ids = [10, 11, 12];
        let text = r#"{"version":1,"files":[
            {"id":4,"section":0,"confidence":0.9},
            {"id":1,"section":0,"confidence":0.9},
            {"id":1,"section":1,"confidence":0.2},
            {"id":2,"section":7,"confidence":2.5},
            {"id":2,"section":0,"confidence":0.3}
        ]}"#;
        let a = read_files(text, &ids, 2).expect("an answer");
        assert_eq!(a.files.len(), 2, "{:?}", a.files);
        assert_eq!(a.files[0].id, 1);
        assert_eq!(a.files[0].section, Some(0));
        // The second use of id 1 is ignored; id 2's section 7 is outside a scheme of 2, and
        // its confidence is clamped.
        assert_eq!(a.files[1].id, 2);
        assert_eq!(a.files[1].section, None);
        assert!((a.files[1].confidence - 1.0).abs() < 1e-6);
        assert_eq!(read_files("{}", &ids, 2), Err(Fault::NoFiles));
        assert_eq!(read_files("nope", &ids, 2), Err(Fault::NoObject));
    }

    #[test]
    fn a_confusion_that_is_not_a_number_is_zero() {
        let text = r#"{"files":[{"id":0,"section":0,"confidence":"very"}]}"#;
        let a = read_files(text, &[0], 1).expect("an answer");
        assert_eq!(a.files[0].confidence, 0.0);
        let text = r#"{"files":[{"id":0,"section":0,"confidence":-3}]}"#;
        let a = read_files(text, &[0], 1).expect("an answer");
        assert_eq!(a.files[0].confidence, 0.0);
    }

    #[test]
    fn a_section_outside_the_scheme_lands_in_unsorted_and_unticked() {
        let mut plan = plan_with(3);
        let scheme = scheme_of(&["Pasta", "Bread"]);
        let batches = batches(&plan, &scheme);
        let answer = Answer {
            files: vec![filed(0, Some(1), 0.9), filed(1, None, 0.9)],
        };
        let out = settle(&mut plan, &scheme, &batches, &[Some(answer)], true);
        assert_eq!(out.placed, 1);
        assert_eq!(out.unsorted, 2, "the out-of-range one and the one left out");
        let unsorted = plan.notes.iter().filter(|n| n.unsorted).count();
        assert_eq!(unsorted, 2);
        for note in plan.notes.iter().filter(|n| n.unsorted) {
            assert_eq!(note.sections, [UNSORTED.to_string()]);
            assert!(!note.ticked, "an unplaced file is not written unasked");
        }
        let bread = plan
            .notes
            .iter()
            .find(|n| n.sections == ["Bread".to_string()])
            .expect("the placed one");
        assert_eq!(bread.title, "Note 0");
        assert_eq!(bread.confidence, Some(0.9));
        assert_eq!(bread.why, "reason 0");
    }

    #[test]
    fn a_failed_batch_leaves_the_others_usable() {
        let mut plan = plan_with(45);
        let scheme = scheme_of(&["Pasta"]);
        let batches = batches(&plan, &scheme);
        assert_eq!(batches.len(), 2);
        // One answer, and one batch that failed: the notes it reached keep their folders, and
        // their ticks.
        let answer = Answer {
            files: vec![filed(0, Some(0), 0.9)],
        };
        let out = settle(&mut plan, &scheme, &batches, &[Some(answer), None], false);
        assert_eq!(out.placed, 1);
        assert_eq!(out.unsorted, 0, "a failed request places nothing");
        // Every note keeps its tick: those the run reached are placed, those it did not keep
        // their folders.
        assert_eq!(plan.notes.iter().filter(|n| n.ticked).count(), 45);
        assert!(!plan.notes.iter().any(|n| n.unsorted));
        assert_eq!(
            plan.notes
                .iter()
                .find(|n| n.title == "Note 0")
                .unwrap()
                .sections,
            ["Pasta".to_string()]
        );
    }

    #[test]
    fn the_plan_takes_the_schemes_order_and_its_section_rows() {
        let mut plan = plan_with(4);
        plan.notes[0].title = "D".into();
        plan.notes[1].title = "C".into();
        plan.notes[2].title = "B".into();
        plan.notes[3].title = "A".into();
        let scheme = Scheme {
            sections: vec![
                Section {
                    name: "Pasta".into(),
                    subs: vec!["Italian".into()],
                },
                Section {
                    name: "Bread".into(),
                    subs: vec![],
                },
            ],
            dropped: 0,
        };
        let batches = batches(&plan, &scheme);
        let answer = Answer {
            files: vec![
                filed(0, Some(2), 0.9),
                filed(1, Some(1), 0.7),
                filed(2, Some(0), 0.8),
                filed(3, Some(2), 0.6),
            ],
        };
        settle(&mut plan, &scheme, &batches, &[Some(answer)], true);
        let titles: Vec<&str> = plan.notes.iter().map(|n| n.title.as_str()).collect();
        // Pasta (B), then Pasta > Italian (C), then Bread by title (A, D).
        assert_eq!(titles, ["B", "C", "A", "D"], "{titles:?}");
        let sections: Vec<Vec<String>> = plan
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                import::RowKind::Section { path } => Some(path.clone()),
                import::RowKind::File(_) => None,
            })
            .collect();
        assert_eq!(
            sections,
            vec![
                vec!["Pasta".to_string()],
                vec!["Pasta".to_string(), "Italian".to_string()],
                vec!["Bread".to_string()]
            ]
        );
    }

    #[test]
    fn tick_only_the_sure_ones_uses_six_tenths() {
        let mut plan = plan_with(4);
        let scheme = scheme_of(&["Pasta"]);
        let batches = batches(&plan, &scheme);
        let answer = Answer {
            files: vec![
                filed(0, Some(0), 0.6),
                filed(1, Some(0), 0.59),
                filed(2, None, 0.9),
            ],
        };
        settle(&mut plan, &scheme, &batches, &[Some(answer)], true);
        tick_sure(&mut plan);
        assert!(
            plan.notes
                .iter()
                .find(|n| n.title == "Note 0")
                .unwrap()
                .ticked
        );
        assert!(
            !plan
                .notes
                .iter()
                .find(|n| n.title == "Note 1")
                .unwrap()
                .ticked
        );
        assert!(
            !plan
                .notes
                .iter()
                .find(|n| n.title == "Note 2")
                .unwrap()
                .ticked
        );
        // The one the answer left out is unsorted and not ticked.
        assert!(
            !plan
                .notes
                .iter()
                .find(|n| n.title == "Note 3")
                .unwrap()
                .ticked
        );
    }

    #[test]
    fn the_scheme_request_names_each_file_and_sends_only_the_start_of_its_text() {
        let mut plan = plan_with(2);
        plan.notes[0].path = plan.folder.join("Pasta").join("one.md");
        plan.notes[0].title = "One".into();
        plan.notes[0].start = format!("# One\n{}", "abcdefghij".repeat(100));
        let ask = scheme_ask(&plan).expect("a request");
        assert!(ask.text.starts_with("Sort these files into sections"));
        assert!(ask.text.contains("path=\"Pasta/one.md\""), "{}", ask.text);
        assert!(ask.text.contains("title=\"One\""));
        // The element holds the first 120 bytes, on one line, and nothing past it.
        let sent = one_line(&plan.notes[0].start, START_SENT);
        assert!(ask.text.contains(&format!(">{sent}</file>")));
        assert!(!ask.text.contains(&format!("{sent}abcd")));
        assert!(!ask.text.contains(&"abcdefghij".repeat(13)));
        assert_eq!(ask.files, 2);
        assert_eq!(ask.more, 0);
    }

    #[test]
    fn the_filing_request_holds_forty_files_and_the_numbered_sections() {
        let plan = plan_with(85);
        let scheme = Scheme {
            sections: vec![
                Section {
                    name: "Pasta".into(),
                    subs: vec!["Italian".into()],
                },
                Section {
                    name: "Bread".into(),
                    subs: vec![],
                },
            ],
            dropped: 0,
        };
        let bs = batches(&plan, &scheme);
        assert_eq!(bs.len(), 3);
        assert_eq!(bs[0].ids.len(), BATCH);
        assert_eq!(bs[2].ids.len(), 5);
        assert!(bs[0].text.contains("0 Pasta\n1 Pasta > Italian\n2 Bread\n"));
        assert!(bs[0].text.contains("<file id=\"0\""));
        assert!(bs[0].text.contains("size=\"10\""));
        assert!(bs[0].text.contains("modified=\"2026-11-23\""));
        // The second batch starts its ids at 0 again.
        assert!(bs[1].text.contains("<file id=\"0\" path=\"n40.md\""));
        assert_eq!(bs[1].ids[0], 40);
    }

    #[test]
    fn the_estimate_counts_the_scheme_and_one_request_per_batch() {
        let plan = plan_with(85);
        let e = estimate(&plan);
        assert_eq!(e.files, 85);
        assert_eq!(e.requests, 1 + 3);
        assert!(e.bytes > 85 * 20, "{e:?}");
    }

    #[test]
    fn a_scheme_can_be_renamed_added_and_deleted() {
        let mut scheme = Scheme {
            sections: vec![
                Section {
                    name: "Pasta".into(),
                    subs: vec!["Italian".into(), "Baked".into()],
                },
                Section {
                    name: "Bread".into(),
                    subs: vec![],
                },
            ],
            dropped: 0,
        };
        assert_eq!(scheme.rows().len(), 4);
        assert!(scheme.rename(0, "Noodles"));
        assert_eq!(
            scheme.rows()[1].path,
            ["Noodles".to_string(), "Italian".to_string()]
        );
        assert!(scheme.rename(1, "Old"));
        assert_eq!(scheme.rows()[1].name, "Old");
        // Deleting a top section takes its subsections.
        scheme.delete(0);
        assert_eq!(scheme.sections.len(), 1);
        assert_eq!(scheme.sections[0].name, "Bread");
        // Adding a section appends it and returns its row.
        let row = scheme.add("Sauces");
        assert_eq!(row, 1);
        assert_eq!(scheme.row_name(row), Some("Sauces"));
        // A name that repeats one is refused, as is an empty one.
        assert_eq!(scheme.problem(), None);
        scheme.rename(1, "bread");
        assert!(scheme.problem().unwrap().contains("Two sections"));
        scheme.rename(1, "  ");
        assert_eq!(scheme.problem().unwrap(), "A section needs a name.");
        // The last row is never deleted.
        scheme.delete(1);
        scheme.delete(0);
        assert_eq!(scheme.sections.len(), 1);
    }

    #[test]
    fn random_answers_never_panic() {
        // A deterministic stream of text, as the split's fuzz test does: braces, quotes,
        // escapes, numbers and every byte class a model might write.
        let pieces = [
            "{",
            "}",
            "[",
            "]",
            "\"",
            "\\",
            ",",
            ":",
            "0",
            "1",
            "9",
            "-",
            "e",
            "n",
            "true",
            "false",
            "null",
            "version",
            "sections",
            "files",
            "id",
            "section",
            "confidence",
            "why",
            "name",
            "subs",
            "Ü",
            "😀",
            "\n",
            "\u{1}",
            " ",
            "\"id\":1",
            "\"section\":9",
        ];
        let mut state = 0x1234_5678u64;
        let mut text = String::new();
        for _ in 0..400 {
            text.clear();
            for _ in 0..64 {
                state = state
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let i = (state >> 33) as usize % pieces.len();
                text.push_str(pieces[i]);
            }
            let _ = read_scheme(&text);
            let _ = read_files(&text, &[0, 1, 2], 3);
        }
    }
}
