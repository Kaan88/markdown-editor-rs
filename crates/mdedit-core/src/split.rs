//! Splitting a document into notes at its headings: the plan, the text each note will hold, the
//! rest of the document, and the counts a screen shows (`docs/ai-library-plan.md` 5.2, 3.6 and
//! 5.3).
//!
//! A [`Plan`] is made by [`plan`] from three things the caller has already found: the document's
//! text (a [`Buffer`], borrowed and read-only), the document's headings, and the level to cut at.
//! The headings are passed in on purpose: this crate does not depend on the function list, so the
//! shell scans them (`mdedit_outline::symbols`, whose `depth` is a heading's level minus one) and
//! this module only cuts. What a heading says is read from its line, so a [`Heading`] is a line
//! and a level, nothing more.
//!
//! A note is a range of lines, a title, and the sections it sits under (the headings above the cut
//! level, outermost first). Notes follow each other and never overlap. The rest — every non-blank
//! line of the document that is in no *ticked* note, as labelled runs — is computed from what is
//! ticked, so it follows every change by construction. That is the invariant of the module, and
//! [`Plan::check`] states it and verifies it against the document.
//!
//! A note's text is the source's, byte for byte, line endings included, except for the promotion
//! of its headings: **promote, never add, never strip.** A note that starts with a heading has
//! that heading made level 1 and every heading in it raised by the same number of levels; only the
//! `#` signs change, never a word; a line inside a code fence is not looked at, and an underlined
//! (setext) heading is left alone. A note that does not start with a heading gets `# <title>` and
//! a blank line above it, and the note records that (`Note::title_added`), so a screen can mark
//! them and a writer can take them away again. A missing final line ending is added.
//!
//! **Bounds.** [`plan`] scans the document's bytes once, for the blank lines, and reads only the
//! headings' own lines for their titles, so it costs O(bytes + headings); a note's text costs
//! O(its lines) and its byte count costs its headings and one rope lookup. A plan holds at most
//! [`SPLIT_MAX_NOTES`] notes; past that the lines are left in the rest and [`Plan::capped`] is
//! set, so the plan stays bounded in memory and the number of rows on a screen stays sane. The
//! rest is found from one bit per line saying whether the line is blank (`lines / 8` bytes held
//! for the plan's life), so it costs O(notes + the blank edges of the gaps) and a tick never reads
//! the document. A tick, a rename or a move to another section costs the headings inside the note;
//! a merge or a new note adds the notes it divides. Nothing here is quadratic in the number of
//! lines. [`Plan::check`] is O(lines) in time and memory and is meant for tests and a debug build.

use crate::buffer::Buffer;
use crate::diff::line_count;
use crate::limits::{SPLIT_MAX_NOTES, SPLIT_MAX_SECTIONS, SPLIT_SECTION_MAX, SPLIT_TITLE_MAX};
use std::ops::Range;

/// The levels the switch offers: `#`, `##` and `###`. A deeper heading is inside a note, never a
/// cut of its own.
pub const SPLIT_LEVELS: [u8; 3] = [1, 2, 3];

/// The deepest ATX heading Markdown has, and the deepest this module reads.
const ATX_MAX_LEVEL: u8 = 6;

/// A note is "holding most of the document" above this share of a document of at least
/// [`MOST_MIN_LINES`] lines: `lines * 2 > total`.
const MOST_MIN_LINES: usize = 10;
/// A note of at most this many lines is a "short" note; at least [`SHORT_MIN_NOTES`] of them, and
/// a third or more of the notes (`short * SHORT_SHARE >= notes`), is "many notes of one or two
/// lines".
const SHORT_NOTE_LINES: usize = 2;
const SHORT_MIN_NOTES: usize = 5;
const SHORT_SHARE: usize = 3;
/// This many notes with the same title is worth saying: the second file would be "Title 2.md".
const SAME_TITLE_MIN: usize = 5;

/// A heading of the document: where it starts and its level. What it says is read from the line.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Heading {
    /// 0-based line the heading starts on.
    pub line: usize,
    /// 1 for `#`, 2 for `##`, up to 6. A setext heading is 1 for `===` and 2 for `---`.
    pub level: u8,
}

/// How many headings a document has at each level the switch offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Levels {
    counts: [usize; SPLIT_LEVELS.len()],
}

impl Levels {
    /// Headings at `level`; 0 for a level the switch does not offer.
    pub fn count(&self, level: u8) -> usize {
        SPLIT_LEVELS
            .iter()
            .position(|&l| l == level)
            .map_or(0, |i| self.counts[i])
    }

    /// The levels that occur at least once, shallowest first.
    pub fn available(&self) -> impl Iterator<Item = u8> + '_ {
        SPLIT_LEVELS
            .iter()
            .copied()
            .filter(move |&l| self.count(l) > 0)
    }

    /// The level a plan opens with: the shallowest that occurs at least twice, or, when none
    /// does, the shallowest that occurs at all. `None` when the document has no heading at a
    /// level the switch offers, so there is nothing to cut at.
    pub fn default_level(&self) -> Option<u8> {
        self.available()
            .find(|&l| self.count(l) >= 2)
            .or_else(|| self.available().next())
    }
}

/// The headings of `headings` counted at each level the switch offers.
pub fn levels(headings: &[Heading]) -> Levels {
    let mut counts = [0usize; SPLIT_LEVELS.len()];
    for h in headings {
        if let Some(i) = SPLIT_LEVELS.iter().position(|&l| l == h.level) {
            counts[i] = counts[i].saturating_add(1);
        }
    }
    Levels { counts }
}

/// One line of a note as it will be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteLine {
    /// The 1-based line of the document it came from; `None` for a line the app added.
    pub source: Option<usize>,
    /// The line as it will be written, its line break included, so joining the lines of a note
    /// gives its text to the byte.
    pub text: String,
    /// The `#` signs a level change took away: a preview strikes that many through before the
    /// ones that are left. 0 when the line was not changed.
    pub removed: u8,
    /// The app wrote this line (a title, or a label of the rest), not the document.
    pub added: bool,
}

/// One note of a plan: a range of the document's lines and what a screen needs to show it.
#[derive(Clone, Debug)]
pub struct Note {
    range: Range<usize>,
    title: String,
    sections: Vec<String>,
    ticked: bool,
    title_added: bool,
    heading_shift: u8,
    boundary_moved: bool,
    bytes: usize,
}

impl Note {
    /// The note's lines, 0-based and half-open.
    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    /// The first line of the note, 1-based, as the screen and the record show it.
    pub fn first_line(&self) -> usize {
        self.range.start.saturating_add(1)
    }

    /// The last line of the note, 1-based and inclusive.
    pub fn last_line(&self) -> usize {
        self.range.end
    }

    /// Lines in the note (the length of its range, blank lines included).
    pub fn lines(&self) -> usize {
        self.range.len()
    }

    /// The note's title: the heading's words, or the name the user gave it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The sections it sits under, outermost first: under `# Tokyo`, `## Food`, a `###` note has
    /// `["Tokyo", "Food"]`.
    pub fn sections(&self) -> &[String] {
        &self.sections
    }

    /// Whether the note is written when the plan is written.
    pub fn ticked(&self) -> bool {
        self.ticked
    }

    /// Whether the app wrote `# <title>` and the blank line after it above the note's first line.
    pub fn title_added(&self) -> bool {
        self.title_added
    }

    /// How many levels the note's headings were raised by: 0 when nothing changed.
    pub fn heading_shift(&self) -> u8 {
        self.heading_shift
    }

    /// Whether the note was asked to start inside a code block and was moved to the line after
    /// it: the row says so.
    pub fn boundary_moved(&self) -> bool {
        self.boundary_moved
    }

    /// The bytes the note's file will hold.
    pub fn bytes(&self) -> usize {
        self.bytes
    }
}

/// The lines that are in no ticked note: one note's worth, in runs, each labelled with where it
/// came from (3.6).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rest {
    runs: Vec<Range<usize>>,
    lines: usize,
    ticked: bool,
    title: String,
    title_added: bool,
}

impl Rest {
    /// The runs of lines in no ticked note, in source order, 0-based and half-open; blank lines
    /// at the edges of a run are not in it, blank lines inside one are.
    pub fn runs(&self) -> &[Range<usize>] {
        &self.runs
    }

    /// The non-blank lines in those runs: the number the row and the footer show.
    pub fn lines(&self) -> usize {
        self.lines
    }

    /// The lines the runs span, blank lines inside them included (what the note writes from the
    /// document).
    pub fn span_lines(&self) -> usize {
        self.runs.iter().map(|r| r.len()).sum()
    }

    /// Whether this note too is written.
    pub fn ticked(&self) -> bool {
        self.ticked
    }

    /// The name of the note: the document's name with " (the rest)" after it.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Always true: the rest is the app's note, so its title is the app's.
    pub fn title_added(&self) -> bool {
        self.title_added
    }
}

/// The counts a screen shows: the list's footer, and the rule of the Recycle Bin box.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// Notes that will be written: the ticked ones, the rest included when it is ticked.
    pub notes: usize,
    /// Notes in the plan, ticked or not.
    pub all_notes: usize,
    /// Lines of the document.
    pub document_lines: usize,
    /// Lines inside ticked notes (the rest's when it is ticked), the notes' ranges as they are.
    pub lines_in_notes: usize,
    /// Non-blank lines in no ticked note: what stays only in the original.
    pub lines_in_no_note: usize,
    /// Places those lines come in.
    pub places: usize,
    /// Bytes of the document.
    pub document_bytes: usize,
}

impl Totals {
    /// Whether every non-blank line of the document is inside a ticked note: the one rule that
    /// offers the Recycle Bin (5.4).
    pub fn covered(&self) -> bool {
        self.lines_in_no_note == 0
    }
}

/// Something a plan says about itself, shown above the list and never blocking anything (3.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Warning {
    /// No split at all: the plan holds fewer than two notes (`notes` is how many).
    NoSplit { notes: usize },
    /// One note holds more than half of a document of at least ten lines.
    OneNoteHoldsMost {
        note: usize,
        lines: usize,
        total: usize,
    },
    /// Many notes of one or two lines: at least five of them, and a third of the notes or more.
    ManyShortNotes { notes: Vec<usize> },
    /// These notes do not start at a heading while the rest do.
    NotAtAHeading { notes: Vec<usize> },
    /// This many notes share a title: the second file is "Title 2.md", and so on.
    SameTitle { title: String, notes: Vec<usize> },
}

/// The plan for one document: its notes, the rest, and the counts and warnings of both. It
/// borrows the document's text and writes nothing, reads nothing and asks nothing.
#[derive(Clone, Debug)]
pub struct Plan<'a> {
    doc: &'a Buffer,
    name: String,
    level: u8,
    total: usize,
    bytes: usize,
    /// The headings the plan was cut from, in line order, one per line.
    headings: Vec<Heading>,
    notes: Vec<Note>,
    rest_ticked: bool,
    /// Bit `line % 64` of word `line / 64` is set when line `line` is blank.
    blank_words: Vec<u64>,
    /// The cut stopped at [`SPLIT_MAX_NOTES`]; the lines past it are in the rest.
    capped: bool,
}

/// Cuts `doc` at its headings of `level`: one note per heading at that level, from the heading to
/// the line before the next heading at that level or above. Headings above the level become the
/// notes' sections, and the lines before the first note, between a section heading and its first
/// note, and after the last one, are in no note (5.2).
///
/// `name` is the document's file name: the rest's title and labels use it. `level` is brought
/// into [`SPLIT_LEVELS`], because that is all the switch offers. Every note starts ticked, the
/// rest unticked.
pub fn plan<'a>(doc: &'a Buffer, name: &str, headings: &[Heading], level: u8) -> Plan<'a> {
    let total = line_count(doc);
    let level = level.clamp(SPLIT_LEVELS[0], SPLIT_LEVELS[2]);
    let mut heads: Vec<Heading> = headings
        .iter()
        .copied()
        .filter(|h| h.line < total && (1..=ATX_MAX_LEVEL).contains(&h.level))
        .collect();
    heads.sort_by(|a, b| a.line.cmp(&b.line).then(a.level.cmp(&b.level)));
    heads.dedup_by_key(|h| h.line);
    let mut p = Plan {
        doc,
        name: name.to_string(),
        level,
        total,
        bytes: doc.len_bytes(),
        headings: heads,
        notes: Vec::new(),
        rest_ticked: false,
        blank_words: blank_words(doc, total),
        capped: false,
    };
    p.cut();
    p
}

impl<'a> Plan<'a> {
    /// The document's file name, as it was given.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The level the plan was cut at.
    pub fn level(&self) -> u8 {
        self.level
    }

    /// The document the plan was made from.
    pub fn document(&self) -> &'a Buffer {
        self.doc
    }

    /// Lines of the document, as the comparison counts them: a final line break does not make a
    /// line.
    pub fn lines(&self) -> usize {
        self.total
    }

    /// Bytes of the document.
    pub fn document_bytes(&self) -> usize {
        self.bytes
    }

    /// Whether the cut stopped at [`SPLIT_MAX_NOTES`]: the lines of the notes not made are in the
    /// rest.
    pub fn capped(&self) -> bool {
        self.capped
    }

    /// The notes, in line order.
    pub fn notes(&self) -> &[Note] {
        &self.notes
    }

    /// The note at `i`, or `None` past the last one.
    pub fn note(&self, i: usize) -> Option<&Note> {
        self.notes.get(i)
    }

    /// The index of the note that holds `line`, if one does.
    pub fn note_at(&self, line: usize) -> Option<usize> {
        let i = self.notes.partition_point(|n| n.range.end <= line);
        self.notes
            .get(i)
            .filter(|n| n.range.start <= line)
            .map(|_| i)
    }

    /// The note at `i` as it will be written, line by line; empty past the last note.
    pub fn note_lines(&self, i: usize) -> Vec<NoteLine> {
        self.notes
            .get(i)
            .map(|n| self.compose(n))
            .unwrap_or_default()
    }

    /// The text of the note at `i` as it will be written; empty past the last note.
    pub fn note_text(&self, i: usize) -> String {
        join(&self.note_lines(i))
    }

    /// The rest, computed from what is ticked now.
    pub fn rest(&self) -> Rest {
        let mut runs: Vec<Range<usize>> = Vec::new();
        let mut at = 0usize;
        for n in self.notes.iter().filter(|n| n.ticked) {
            if n.range.start > at
                && let Some(r) = self.run_in(at..n.range.start)
            {
                runs.push(r);
            }
            at = at.max(n.range.end);
        }
        if at < self.total
            && let Some(r) = self.run_in(at..self.total)
        {
            runs.push(r);
        }
        let lines = runs.iter().fold(0usize, |sum, r| {
            sum.saturating_add(self.non_blank(r.clone()))
        });
        Rest {
            runs,
            lines,
            ticked: self.rest_ticked,
            title: self.rest_title(),
            title_added: true,
        }
    }

    /// Whether the rest too is written.
    pub fn rest_ticked(&self) -> bool {
        self.rest_ticked
    }

    /// The rest as it will be written: the title, then for each run its label and its lines.
    pub fn rest_lines(&self) -> Vec<NoteLine> {
        let rest = self.rest();
        let eol = rest.runs.first().map_or("\n", |r| self.eol_of(r.start));
        let mut out: Vec<NoteLine> = Vec::new();
        if rest.title_added {
            out.push(NoteLine {
                source: None,
                text: format!("# {}{eol}", rest.title),
                removed: 0,
                added: true,
            });
            out.push(NoteLine {
                source: None,
                text: eol.to_string(),
                removed: 0,
                added: true,
            });
        }
        for run in &rest.runs {
            out.push(NoteLine {
                source: None,
                text: format!(
                    "<!-- lines {}-{} of {} -->{}",
                    run.start.saturating_add(1),
                    run.end,
                    self.name,
                    self.eol_of(run.start)
                ),
                removed: 0,
                added: true,
            });
            for line in run.clone() {
                if line >= self.total {
                    break;
                }
                let mut text = String::new();
                self.doc
                    .text(self.doc.line_range_with_break(line), &mut text);
                if !(text.ends_with('\n') || text.ends_with('\r')) {
                    text.push_str(self.eol_of(run.start));
                }
                out.push(NoteLine {
                    source: Some(line.saturating_add(1)),
                    text,
                    removed: 0,
                    added: false,
                });
            }
        }
        out
    }

    /// The text of the rest as it will be written.
    pub fn rest_text(&self) -> String {
        join(&self.rest_lines())
    }

    /// The counts of the plan as it stands.
    pub fn totals(&self) -> Totals {
        let rest = self.rest();
        let mut notes = 0usize;
        let mut lines_in_notes = 0usize;
        for n in &self.notes {
            if n.ticked {
                notes = notes.saturating_add(1);
                lines_in_notes = lines_in_notes.saturating_add(n.range.len());
            }
        }
        if rest.ticked {
            notes = notes.saturating_add(1);
            lines_in_notes = lines_in_notes.saturating_add(rest.span_lines());
        }
        Totals {
            notes,
            all_notes: self.notes.len(),
            document_lines: self.total,
            lines_in_notes,
            lines_in_no_note: if rest.ticked { 0 } else { rest.lines },
            places: rest.runs.len(),
            document_bytes: self.bytes,
        }
    }

    /// Whether every non-blank line is inside a ticked note: the Recycle Bin rule of 5.4, which
    /// the rest's own tick makes true by construction.
    pub fn covered(&self) -> bool {
        self.totals().covered()
    }

    /// The distinct section paths of the plan, in the order they first appear, for the "Move to a
    /// section…" menu.
    pub fn sections(&self) -> Vec<Vec<String>> {
        let mut out: Vec<Vec<String>> = Vec::new();
        for n in &self.notes {
            if !out.iter().any(|s| s == &n.sections) {
                out.push(n.sections.clone());
            }
        }
        out
    }

    /// Ticks or unticks the note at `i`; nothing past the last note.
    pub fn set_ticked(&mut self, i: usize, ticked: bool) {
        if let Some(n) = self.notes.get_mut(i) {
            n.ticked = ticked;
        }
    }

    /// Ticks every note; the rest is a row of its own and is left as it is.
    pub fn tick_all(&mut self) {
        for n in &mut self.notes {
            n.ticked = true;
        }
    }

    /// Unticks every note; the rest is a row of its own and is left as it is.
    pub fn untick_all(&mut self) {
        for n in &mut self.notes {
            n.ticked = false;
        }
    }

    /// Ticks or unticks the rest.
    pub fn set_rest_ticked(&mut self, ticked: bool) {
        self.rest_ticked = ticked;
    }

    /// Renames the note at `i`: one line, trimmed, cut to [`SPLIT_TITLE_MAX`] bytes; an empty name
    /// becomes "Lines 123-156". A title the app added is rewritten; a heading that came from the
    /// document keeps its words and only the file's name follows the new title (5.2).
    pub fn rename(&mut self, i: usize, title: &str) {
        let Some(note) = self.notes.get(i) else {
            return;
        };
        let mut title = clean(title, SPLIT_TITLE_MAX);
        if title.is_empty() {
            title = fallback_title(&note.range);
        }
        if let Some(note) = self.notes.get_mut(i) {
            note.title = title;
        }
        self.refresh_bytes(i);
    }

    /// Moves the note at `i` to the section path `sections`: names trimmed, cut to
    /// [`SPLIT_SECTION_MAX`] bytes, empty ones dropped, at most [`SPLIT_MAX_SECTIONS`] kept. Its
    /// lines, title and tick are not changed, and no file is moved or made.
    pub fn move_to_section(&mut self, i: usize, sections: &[&str]) {
        let Some(note) = self.notes.get_mut(i) else {
            return;
        };
        note.sections = sections
            .iter()
            .filter_map(|s| {
                let name = clean(s, SPLIT_SECTION_MAX);
                (!name.is_empty()).then_some(name)
            })
            .take(SPLIT_MAX_SECTIONS)
            .collect();
    }

    /// Merges the note at `i` into the note above it, and says which note is left (`None` when
    /// there is no note above). The surviving note keeps the title, the sections and the tick of
    /// the note above and grows to the end of this one, so the lines between them (a section
    /// heading, blank lines) join it too: a note's lines are one run.
    pub fn merge_with_above(&mut self, i: usize) -> Option<usize> {
        if i == 0 {
            return None;
        }
        let end = self.notes.get(i)?.range.end;
        let above = self.notes.get_mut(i - 1)?;
        above.range.end = end;
        self.notes.remove(i);
        self.refresh_bytes(i - 1);
        Some(i - 1)
    }

    /// Starts a new note at `line`, and says where it is (`None` when nothing was made). A line
    /// inside the note at `k` splits it in two there, both parts keeping its sections and tick; a
    /// line in no note makes a note from it to the line before the next note (or the end), with
    /// the sections above it and ticked. Lines are trimmed of blank ones at the ends; a line
    /// inside a fenced code block is moved to the line after the block (and the note says so);
    /// the note is never empty. A note that starts at a heading takes its words as the title,
    /// otherwise the app adds `# <title>` and calls it "Lines 123-156".
    pub fn new_note_at(&mut self, line: usize) -> Option<usize> {
        if self.total == 0 {
            return None;
        }
        let asked = line.min(self.total - 1);
        let at = self.after_code_block(asked)?;
        if at >= self.total {
            return None;
        }
        if let Some(k) = self.note_at(at) {
            let start = self.notes.get(k)?.range.start;
            if at == start {
                return None;
            }
            let end = self.notes.get(k)?.range.end;
            let (sections, ticked) = {
                let n = self.notes.get(k)?;
                (n.sections.clone(), n.ticked)
            };
            let range = self.trim(at..end);
            if range.is_empty() {
                return None;
            }
            let upper = self.trim(start..range.start);
            if upper.is_empty() {
                return None;
            }
            let mut note = self.note_for(range, sections, ticked);
            note.boundary_moved = at != asked;
            self.notes.get_mut(k)?.range = upper;
            self.refresh_bytes(k);
            self.notes.insert(k.saturating_add(1), note);
            self.refresh_bytes(k.saturating_add(1));
            Some(k.saturating_add(1))
        } else {
            let end = self
                .notes
                .iter()
                .find(|n| n.range.start > at)
                .map_or(self.total, |n| n.range.start);
            let range = self.trim(at..end);
            if range.is_empty() {
                return None;
            }
            let sections = self.sections_at(range.start);
            let mut note = self.note_for(range.clone(), sections, true);
            note.boundary_moved = at != asked;
            let i = self.notes.partition_point(|n| n.range.start < range.start);
            self.notes.insert(i, note);
            self.refresh_bytes(i);
            Some(i)
        }
    }

    /// What the plan says about itself (3.5), in a stable order. Warnings never block anything.
    pub fn warnings(&self) -> Vec<Warning> {
        let mut out: Vec<Warning> = Vec::new();
        let total = self.total;
        if self.notes.len() < 2 {
            out.push(Warning::NoSplit {
                notes: self.notes.len(),
            });
        }
        if total >= MOST_MIN_LINES
            && let Some((i, n)) = self
                .notes
                .iter()
                .enumerate()
                .max_by_key(|(_, n)| n.range.len())
                .filter(|(_, n)| n.range.len().saturating_mul(2) > total)
        {
            out.push(Warning::OneNoteHoldsMost {
                note: i,
                lines: n.range.len(),
                total,
            });
        }
        let short: Vec<usize> = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.range.len() <= SHORT_NOTE_LINES)
            .map(|(i, _)| i)
            .collect();
        if short.len() >= SHORT_MIN_NOTES
            && short.len().saturating_mul(SHORT_SHARE) >= self.notes.len()
        {
            out.push(Warning::ManyShortNotes { notes: short });
        }
        let untitled: Vec<usize> = self
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.title_added)
            .map(|(i, _)| i)
            .collect();
        if !untitled.is_empty() && untitled.len() < self.notes.len() {
            out.push(Warning::NotAtAHeading { notes: untitled });
        }
        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
        for (i, n) in self.notes.iter().enumerate() {
            match groups.iter_mut().find(|(t, _)| *t == n.title) {
                Some((_, notes)) => notes.push(i),
                None => groups.push((n.title.clone(), vec![i])),
            }
        }
        for (title, notes) in groups {
            if notes.len() >= SAME_TITLE_MIN {
                out.push(Warning::SameTitle { title, notes });
            }
        }
        out
    }

    /// Verifies the plan against the document, and says what is wrong first: the notes follow
    /// each other and do not overlap; no line is in two notes, and every line is in one note, in
    /// the rest, or blank; the rest is exactly the non-blank lines in no ticked note; the stored
    /// byte counts are what the notes will write; titles and sections are clean. O(lines) in time
    /// and memory, so it is for tests and a debug build, not for every draw.
    pub fn check(&self) -> Result<(), String> {
        if self.total != line_count(self.doc) {
            return Err(format!(
                "the plan has {} lines, the document {}",
                self.total,
                line_count(self.doc)
            ));
        }
        if self.bytes != self.doc.len_bytes() {
            return Err("the document's byte count is stale".to_string());
        }
        let mut prev_end = 0usize;
        for (i, n) in self.notes.iter().enumerate() {
            if n.range.start >= n.range.end {
                return Err(format!("note {i} is empty or reversed: {:?}", n.range));
            }
            if n.range.start < prev_end {
                return Err(format!(
                    "note {i} starts at line {} but the note before ends at line {}",
                    n.range.start, prev_end
                ));
            }
            if n.range.end > self.total {
                return Err(format!(
                    "note {i} ends at line {} past the document's {}",
                    n.range.end, self.total
                ));
            }
            prev_end = n.range.end;
            if n.title.trim().is_empty() || n.title.len() > SPLIT_TITLE_MAX {
                return Err(format!("note {i} has a title of {} bytes", n.title.len()));
            }
            if n.sections.len() > SPLIT_MAX_SECTIONS {
                return Err(format!("note {i} has {} sections", n.sections.len()));
            }
            for s in &n.sections {
                if s.trim().is_empty() || s.len() > SPLIT_SECTION_MAX {
                    return Err(format!(
                        "note {i} has a section of {} bytes: {s:?}",
                        s.len()
                    ));
                }
            }
            if n.heading_shift >= ATX_MAX_LEVEL {
                return Err(format!(
                    "note {i} raises its headings by {}",
                    n.heading_shift
                ));
            }
            let bytes = self
                .note_lines(i)
                .iter()
                .fold(0usize, |sum, l| sum.saturating_add(l.text.len()));
            if bytes != n.bytes {
                return Err(format!(
                    "note {i} counts {} bytes and writes {bytes}",
                    n.bytes
                ));
            }
            if self.blank(n.range.start) || self.blank(n.range.end.saturating_sub(1)) {
                return Err(format!("note {i} starts or ends on a blank line"));
            }
        }
        let mut owner: Vec<usize> = vec![usize::MAX; self.total];
        for (i, n) in self.notes.iter().enumerate() {
            for line in n.range.clone() {
                match owner.get_mut(line) {
                    Some(o) if *o == usize::MAX => *o = i,
                    _ => return Err(format!("line {} is in two notes", line.saturating_add(1))),
                }
            }
        }
        // The rest, found a line at a time, where `rest()` walks the gaps between the notes.
        let mut runs: Vec<Range<usize>> = Vec::new();
        let mut open: Option<usize> = None;
        let mut uncovered = 0usize;
        for line in 0..self.total {
            let ticked = owner
                .get(line)
                .and_then(|&i| self.notes.get(i))
                .is_some_and(|n| n.ticked);
            let blank = self.blank(line);
            if ticked {
                if let Some(a) = open.take() {
                    runs.push(a..line);
                }
            } else {
                if !blank && open.is_none() {
                    open = Some(line);
                }
                if !blank {
                    uncovered = uncovered.saturating_add(1);
                }
            }
        }
        if let Some(a) = open.take() {
            runs.push(a..self.total);
        }
        let runs: Vec<Range<usize>> = runs
            .into_iter()
            .filter_map(|r| {
                let r = self.trim(r);
                (!r.is_empty()).then_some(r)
            })
            .collect();
        let rest = self.rest();
        if runs != rest.runs {
            return Err(format!(
                "the rest is {:?} but the lines in no ticked note are {runs:?}",
                rest.runs
            ));
        }
        if uncovered != rest.lines {
            return Err(format!(
                "the rest counts {} lines but {uncovered} are in no ticked note",
                rest.lines
            ));
        }
        let totals = self.totals();
        let ticked = self.notes.iter().filter(|n| n.ticked).count() + usize::from(rest.ticked);
        if totals.all_notes != self.notes.len()
            || totals.notes != ticked
            || totals.document_lines != self.total
            || totals.document_bytes != self.bytes
            || totals.lines_in_no_note != if rest.ticked { 0 } else { uncovered }
            || totals.places != runs.len()
        {
            return Err(format!("the totals do not agree with the plan: {totals:?}"));
        }
        Ok(())
    }

    // ---- building

    /// Cuts the notes from `headings`; the one place a plan's notes are made.
    fn cut(&mut self) {
        let heads = self.headings.clone();
        let (level, total) = (self.level, self.total);
        let mut notes: Vec<Note> = Vec::new();
        let mut stack: Vec<(u8, String)> = Vec::new();
        let mut open: Option<Note> = None;
        for h in &heads {
            if h.level > level {
                continue;
            }
            if let Some(mut n) = open.take() {
                n.range.end = h.line;
                n.range = self.trim(n.range);
                if !n.range.is_empty() {
                    notes.push(n);
                }
            }
            if h.level < level {
                while stack.last().is_some_and(|(l, _)| *l >= h.level) {
                    stack.pop();
                }
                let name = clean(&self.heading_text(h.line), SPLIT_SECTION_MAX);
                if !name.is_empty() {
                    stack.push((h.level, name));
                }
            } else if notes.len() >= SPLIT_MAX_NOTES {
                self.capped = true;
            } else {
                let range = h.line..h.line.saturating_add(1);
                let (title, title_added, heading_shift) = self.heading_of(h.line);
                open = Some(Note {
                    range,
                    title,
                    sections: stack.iter().map(|(_, n)| n.clone()).collect(),
                    ticked: true,
                    title_added,
                    heading_shift,
                    boundary_moved: false,
                    bytes: 0,
                });
            }
        }
        if let Some(mut n) = open.take() {
            n.range.end = total;
            n.range = self.trim(n.range);
            if !n.range.is_empty() {
                notes.push(n);
            }
        }
        for mut n in notes {
            n.bytes = self.bytes_of(&n);
            self.notes.push(n);
        }
    }

    /// A note for `range`: its title, whether the app adds one, and the shift, all read from the
    /// line the range starts on.
    fn note_for(&self, range: Range<usize>, sections: Vec<String>, ticked: bool) -> Note {
        let (mut title, title_added, heading_shift) = self.heading_of(range.start);
        if title.is_empty() {
            title = fallback_title(&range);
        }
        Note {
            range,
            title,
            sections,
            ticked,
            title_added,
            heading_shift,
            boundary_moved: false,
            bytes: 0,
        }
    }

    /// The title of a note that starts at `line`, whether the app added it, and the number of
    /// levels its headings are raised by.
    fn heading_of(&self, line: usize) -> (String, bool, u8) {
        let body = self.line_body(line);
        match atx(&body) {
            Some(a) => {
                let title = clean(a.text(), SPLIT_TITLE_MAX);
                (title, false, a.level.saturating_sub(1))
            }
            None if self.is_heading(line) => (clean(&body, SPLIT_TITLE_MAX), false, 0),
            None => (String::new(), true, 0),
        }
    }

    /// What a heading's line says: its words without the `#` signs, or the line itself when it is
    /// an underlined (setext) heading.
    fn heading_text(&self, line: usize) -> String {
        let body = self.line_body(line);
        match atx(&body) {
            Some(a) => a.text().to_string(),
            None => body.trim().to_string(),
        }
    }

    /// Whether the heading scan called `line` a heading.
    fn is_heading(&self, line: usize) -> bool {
        self.headings
            .binary_search_by_key(&line, |h| h.line)
            .is_ok()
    }

    /// The sections a note that starts at `from` sits under: the headings above it that are
    /// shallower than the note's own heading, or than the cut when the line is no heading.
    fn sections_at(&self, from: usize) -> Vec<String> {
        let own = self
            .headings
            .binary_search_by_key(&from, |h| h.line)
            .ok()
            .and_then(|i| self.headings.get(i));
        let cut = own.map_or(self.level, |h| h.level);
        let mut stack: Vec<(u8, String)> = Vec::new();
        for h in &self.headings {
            if h.line > from {
                break;
            }
            if h.level >= cut {
                continue;
            }
            while stack.last().is_some_and(|(l, _)| *l >= h.level) {
                stack.pop();
            }
            if h.line < from {
                let name = clean(&self.heading_text(h.line), SPLIT_SECTION_MAX);
                if !name.is_empty() {
                    stack.push((h.level, name));
                }
            }
        }
        stack
            .into_iter()
            .map(|(_, n)| n)
            .take(SPLIT_MAX_SECTIONS)
            .collect()
    }

    // ---- text

    /// The note as it will be written: the app's title lines first when they are the app's, then
    /// the source's lines, their headings raised.
    fn compose(&self, note: &Note) -> Vec<NoteLine> {
        let eol = self.eol_of(note.range.start);
        let mut out: Vec<NoteLine> = Vec::with_capacity(note.range.len().saturating_add(2));
        if note.title_added {
            out.push(NoteLine {
                source: None,
                text: format!("# {}{eol}", note.title),
                removed: 0,
                added: true,
            });
            out.push(NoteLine {
                source: None,
                text: eol.to_string(),
                removed: 0,
                added: true,
            });
        }
        let mut fence = Fence::default();
        let mut raw = String::new();
        for line in note.range.clone() {
            if line >= self.total {
                break;
            }
            let body = self.doc.line_range(line);
            let full = self.doc.line_range_with_break(line);
            let break_len = full.len().saturating_sub(body.len());
            self.doc.text(full, &mut raw);
            let cut = raw.len().saturating_sub(break_len);
            let body_text = raw.get(..cut).unwrap_or("");
            let brk = raw.get(cut..).unwrap_or("");
            let code = fence.line(body_text);
            let mut text = String::with_capacity(raw.len().saturating_add(1));
            let mut removed = 0u8;
            match atx(body_text) {
                Some(a) if !code && note.heading_shift > 0 => {
                    let new = a.level.saturating_sub(note.heading_shift).max(1);
                    removed = a.level.saturating_sub(new);
                    text.push_str(a.indent);
                    for _ in 0..new {
                        text.push('#');
                    }
                    text.push_str(a.rest);
                }
                _ => text.push_str(body_text),
            }
            text.push_str(brk);
            if break_len == 0 {
                text.push_str(eol);
            }
            out.push(NoteLine {
                source: Some(line.saturating_add(1)),
                text,
                removed,
                added: false,
            });
        }
        out
    }

    /// The bytes the note's file will hold: the source's lines as they stand, less the `#` signs
    /// the level change takes away, plus the lines the app adds and a line break where the last
    /// line has none. Counted without composing the text, so it costs the note's headings, not
    /// its lines.
    fn bytes_of(&self, note: &Note) -> usize {
        let eol = self.eol_of(note.range.start);
        let mut bytes = 0usize;
        if note.title_added {
            bytes = bytes.saturating_add(2 + note.title.len() + 2 * eol.len());
        }
        if note.range.start < note.range.end && note.range.end <= self.total {
            let start = self.doc.line_start(note.range.start);
            let end = self.doc.line_start(note.range.end);
            bytes = bytes.saturating_add(end.saturating_sub(start));
            if self.break_len(note.range.end.saturating_sub(1)) == 0 {
                bytes = bytes.saturating_add(eol.len());
            }
            if note.heading_shift > 0 {
                let first = self.headings.partition_point(|h| h.line < note.range.start);
                let inside = self.headings.get(first..).unwrap_or(&[]);
                for h in inside.iter().take_while(|h| h.line < note.range.end) {
                    let body = self.line_body(h.line);
                    if let Some(a) = atx(&body) {
                        let new = a.level.saturating_sub(note.heading_shift).max(1);
                        bytes = bytes.saturating_sub(usize::from(a.level.saturating_sub(new)));
                    }
                }
            }
        }
        bytes
    }

    /// Recomputes the stored byte count of the note at `i`.
    fn refresh_bytes(&mut self, i: usize) {
        let Some(note) = self.notes.get(i).cloned() else {
            return;
        };
        let bytes = self.bytes_of(&note);
        if let Some(note) = self.notes.get_mut(i) {
            note.bytes = bytes;
        }
    }

    // ---- lines

    /// The line's text without its line break.
    fn line_body(&self, line: usize) -> String {
        let mut s = String::new();
        self.doc.line_text(line, &mut s);
        s
    }

    /// The line break that ends `line`, or "\n" when it has none (the app writes LF then).
    fn eol_of(&self, line: usize) -> &'static str {
        let last = self
            .doc
            .line_range_with_break(line)
            .end
            .checked_sub(1)
            .and_then(|i| self.doc.char_at(i));
        match (self.break_len(line), last) {
            (2, _) => "\r\n",
            (1, Some('\n')) => "\n",
            (1, Some('\r')) => "\r",
            _ => "\n",
        }
    }

    /// Bytes of the line break that ends `line`: 0, 1 or 2.
    fn break_len(&self, line: usize) -> usize {
        let full = self.doc.line_range_with_break(line);
        let body = self.doc.line_range(line);
        full.len().saturating_sub(body.len())
    }

    /// Whether line `line` is blank: spaces, tabs and its line break, nothing else.
    fn blank(&self, line: usize) -> bool {
        self.blank_words
            .get(line / 64)
            .is_some_and(|w| (w >> (line % 64)) & 1 == 1)
    }

    /// Non-blank lines in `r`.
    fn non_blank(&self, r: Range<usize>) -> usize {
        r.len().saturating_sub(self.blanks(r))
    }

    /// Blank lines in `r`.
    fn blanks(&self, r: Range<usize>) -> usize {
        let a = r.start.min(self.total);
        let b = r.end.min(self.total);
        if a >= b {
            return 0;
        }
        let mut n = 0usize;
        for w in (a / 64)..=((b - 1) / 64) {
            let word = self.blank_words.get(w).copied().unwrap_or(0);
            let lo = if w == a / 64 { a % 64 } else { 0 };
            let hi = if w == (b - 1) / 64 {
                (b - 1) % 64 + 1
            } else {
                64
            };
            let width = hi - lo;
            let mask = if width >= 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            n = n.saturating_add((word >> lo & mask).count_ones() as usize);
        }
        n
    }

    /// `r` without the blank lines at its ends, inside the document.
    fn trim(&self, r: Range<usize>) -> Range<usize> {
        let mut a = r.start.min(self.total);
        let mut b = r.end.min(self.total);
        while a < b && self.blank(a) {
            a = a.saturating_add(1);
        }
        while b > a && self.blank(b.saturating_sub(1)) {
            b = b.saturating_sub(1);
        }
        a..b
    }

    /// The gap `r` as a run of the rest, or nothing when it holds no non-blank line.
    fn run_in(&self, r: Range<usize>) -> Option<Range<usize>> {
        let r = self.trim(r);
        (!r.is_empty()).then_some(r)
    }

    /// The first line at or after `line` that is outside a fenced code block; `None` when the
    /// document ends inside one.
    fn after_code_block(&self, line: usize) -> Option<usize> {
        let mut fence = Fence::default();
        let mut body = String::new();
        for l in 0..line.min(self.total) {
            self.doc.line_text(l, &mut body);
            fence.line(&body);
        }
        if fence.open.is_none() {
            return Some(line);
        }
        let mut l = line;
        while l < self.total {
            self.doc.line_text(l, &mut body);
            if !fence.line(&body) {
                return Some(l);
            }
            l = l.saturating_add(1);
        }
        None
    }

    /// The name of the rest's note: the document's name without ".md", then " (the rest)".
    fn rest_title(&self) -> String {
        let stem = std::path::Path::new(&self.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(self.name.as_str());
        clean(&format!("{stem} (the rest)"), SPLIT_TITLE_MAX)
    }
}

// ---- reading Markdown

/// An ATX heading line, read as the function list reads one: at most three columns of
/// indentation, one to six `#` signs, then a blank or the end of the line, then text.
struct Atx<'a> {
    /// The indentation before the `#` signs.
    indent: &'a str,
    /// How many `#` signs there are.
    level: u8,
    /// Everything after them, as it stands, trailing blanks included.
    rest: &'a str,
}

impl Atx<'_> {
    /// The heading's words: trimmed, with a closing run of `#` signs and the blanks before it
    /// taken off, as the function list shows them.
    fn text(&self) -> &str {
        let t = self.rest.trim();
        let open = t.trim_end_matches('#');
        if open.is_empty() || open.ends_with([' ', '\t']) {
            open.trim_end()
        } else {
            t
        }
    }
}

/// The ATX heading a line is, if it is one.
fn atx(line: &str) -> Option<Atx<'_>> {
    let (cols, rest) = indent_of(line);
    if cols >= 4 {
        return None;
    }
    let body = rest.trim_end();
    let hashes = body.bytes().take_while(|&b| b == b'#').count();
    if !(1..=usize::from(ATX_MAX_LEVEL)).contains(&hashes) {
        return None;
    }
    let after = body.get(hashes..)?;
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    let a = Atx {
        indent: line
            .get(..line.len().saturating_sub(rest.len()))
            .unwrap_or(""),
        level: hashes as u8,
        rest: rest.get(hashes..)?,
    };
    (!a.text().is_empty()).then_some(a)
}

/// The indentation in columns (a tab is 4) and the rest of the line. The function list's rule.
fn indent_of(s: &str) -> (usize, &str) {
    let mut cols = 0usize;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b' ' => cols = cols.saturating_add(1),
            b'\t' => cols = cols.saturating_add(4),
            _ => return (cols, &s[i..]),
        }
    }
    (cols, "")
}

/// The fenced code block of a Markdown document, read as the function list reads one.
#[derive(Clone, Copy, Debug, Default)]
struct Fence {
    /// The open fence: its character and the length of the opening run.
    open: Option<(u8, usize)>,
}

/// A run of at least three backticks or tildes: the character and the run's length.
fn fence_run(s: &str) -> Option<(u8, usize)> {
    let c = *s.as_bytes().first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = s.bytes().take_while(|&b| b == c).count();
    (n >= 3).then_some((c, n))
}

impl Fence {
    /// One line: true when it is part of a fenced code block (its opening or closing line
    /// included), so nothing in it is a heading.
    fn line(&mut self, line: &str) -> bool {
        let (cols, rest) = indent_of(line);
        let t = rest.trim_end();
        if let Some((ch, n)) = self.open {
            if cols < 4
                && let Some((c, k)) = fence_run(t)
                && c == ch
                && k >= n
                && t.get(k..).is_some_and(|r| r.trim().is_empty())
            {
                self.open = None;
            }
            return true;
        }
        if cols < 4
            && let Some(run) = fence_run(t)
        {
            self.open = Some(run);
            return true;
        }
        false
    }
}

// ---- small helpers

/// One line, trimmed, with control characters turned into spaces, cut to `max` bytes at a char
/// boundary.
fn clean(s: &str, max: usize) -> String {
    let mut out = String::with_capacity(s.len().min(max));
    for c in s.chars() {
        let c = if c.is_control() { ' ' } else { c };
        if out.len().saturating_add(c.len_utf8()) > max {
            break;
        }
        out.push(c);
    }
    out.trim().to_string()
}

/// "Lines 123-156": the title of a note that has none of its own.
fn fallback_title(range: &Range<usize>) -> String {
    format!("Lines {}-{}", range.start.saturating_add(1), range.end)
}

/// The lines' text, one after the other.
fn join(lines: &[NoteLine]) -> String {
    let mut out = String::with_capacity(lines.iter().map(|l| l.text.len()).sum());
    for l in lines {
        out.push_str(&l.text);
    }
    out
}

/// The bit for each line of the document that is blank. One pass over the text's chunks, without
/// looking a line up in the rope, so it costs the document's bytes and not a tree walk per line.
fn blank_words(doc: &Buffer, total: usize) -> Vec<u64> {
    let mut words = vec![0u64; total.div_ceil(64)];
    let mut line = 0usize;
    let mut blank = true;
    let mut after_cr = false;
    for chunk in doc.rope().chunks() {
        for &b in chunk.as_bytes() {
            if after_cr {
                after_cr = false;
                if b == b'\n' {
                    continue;
                }
            }
            match b {
                b'\n' => {
                    mark_blank(&mut words, line, blank);
                    line = line.saturating_add(1);
                    blank = true;
                }
                b'\r' => {
                    mark_blank(&mut words, line, blank);
                    line = line.saturating_add(1);
                    blank = true;
                    after_cr = true;
                }
                b' ' | b'\t' => {}
                _ => blank = false,
            }
        }
    }
    if line < total {
        mark_blank(&mut words, line, blank);
    }
    words
}

/// Sets the bit of a blank line.
fn mark_blank(words: &mut [u64], line: usize, blank: bool) {
    if blank && let Some(w) = words.get_mut(line / 64) {
        *w |= 1u64 << (line % 64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{Rng, lines_of};
    use std::time::Instant;

    /// `# Japan trip journal`, a preamble, two `##` chapters (one holding a fenced code block
    /// with a heading in it), a `#` section and a chapter under it.
    const FIXTURE: &str = r#"# Japan trip journal

Notes from two weeks in October, written on trains.

## Planning the route

We left on the third and changed trains twice.

```
## not a heading
```

## Asakusa and the river

### What we did

We walked along the Sumida and ate at a stall.

# Appendix

## Packing list

- passport
- socks
"#;

    fn buf(text: &str) -> Buffer {
        Buffer::from_text(text)
    }

    fn assert_ok(p: &Plan) {
        if let Err(e) = p.check() {
            panic!("{e}");
        }
    }

    /// The document's headings, found with a plain scan of its own (the module is given its
    /// headings; the function list is not a dependency of this crate).
    fn headings_of(text: &str) -> Vec<Heading> {
        let run = |s: &str| -> Option<(u8, usize)> {
            let c = *s.as_bytes().first()?;
            if c != b'`' && c != b'~' {
                return None;
            }
            let n = s.bytes().take_while(|&b| b == c).count();
            (n >= 3).then_some((c, n))
        };
        let mut out = Vec::new();
        let mut open: Option<(u8, usize)> = None;
        for (i, line) in lines_of(text).iter().enumerate() {
            let body = line.trim_end_matches('\n').trim_end_matches('\r');
            let indent = body
                .len()
                .saturating_sub(body.trim_start_matches([' ', '\t']).len());
            let t = body.trim_start_matches([' ', '\t']).trim_end();
            if let Some((ch, n)) = open {
                if indent < 4
                    && let Some((c, k)) = run(t)
                    && c == ch
                    && k >= n
                    && t.get(k..).is_some_and(|r| r.trim().is_empty())
                {
                    open = None;
                }
                continue;
            }
            if indent < 4
                && let Some(r) = run(t)
            {
                open = Some(r);
                continue;
            }
            if indent >= 4 {
                continue;
            }
            let hashes = t.bytes().take_while(|&b| b == b'#').count();
            if (1..=6).contains(&hashes)
                && (t.len() == hashes
                    || t.get(hashes..).is_some_and(|r| r.starts_with([' ', '\t'])))
                && !t.get(hashes..).unwrap_or("").trim().is_empty()
            {
                out.push(Heading {
                    line: i,
                    level: hashes as u8,
                });
            }
        }
        out
    }

    /// A line of the document as it stands, its break included.
    fn source_line(doc: &Buffer, line: usize) -> String {
        let mut s = String::new();
        doc.text(doc.line_range_with_break(line), &mut s);
        s
    }

    /// A line of the document without its break.
    fn body_of(doc: &Buffer, line: usize) -> String {
        let mut s = String::new();
        doc.line_text(line, &mut s);
        s
    }

    /// Puts back the `#` signs a level change took away.
    fn undemote(text: &str, removed: u8) -> String {
        if removed == 0 {
            return text.to_string();
        }
        let body = text.trim_end_matches(['\n', '\r']);
        let (_, rest) = indent_of(body);
        let hashes = rest.bytes().take_while(|&b| b == b'#').count();
        let at = body.len().saturating_sub(rest.len()).saturating_add(hashes);
        let mut out = String::from(text.get(..at).unwrap_or(""));
        for _ in 0..removed {
            out.push('#');
        }
        out.push_str(text.get(at..).unwrap_or(""));
        out
    }

    /// The whole source rebuilt from the plan: every line not in a note as the document has it,
    /// and every note's lines with its level change undone, in order. Line by line it must be the
    /// source to the byte.
    fn rebuild(p: &Plan, doc: &Buffer) -> String {
        let mut out = String::new();
        let mut at = 0usize;
        for i in 0..p.notes().len() {
            let n = p.note(i).expect("a note");
            assert!(n.range().start >= at, "the notes follow each other");
            for line in at..n.range().start {
                out.push_str(&source_line(doc, line));
            }
            for nl in p.note_lines(i) {
                if nl.added {
                    continue;
                }
                let line = nl.source.expect("a line of the source") - 1;
                let whole = source_line(doc, line);
                let seen = undemote(&nl.text, nl.removed);
                if whole.ends_with('\n') || whole.ends_with('\r') {
                    assert_eq!(seen, whole, "line {} of note {i}", line + 1);
                } else {
                    assert_eq!(
                        seen.trim_end_matches(['\n', '\r']),
                        body_of(doc, line),
                        "line {} of note {i}",
                        line + 1
                    );
                    assert!(
                        seen.ends_with('\n') || seen.ends_with('\r'),
                        "a missing final line ending is added"
                    );
                }
                out.push_str(&seen);
            }
            at = n.range().end;
        }
        for line in at..p.lines() {
            out.push_str(&source_line(doc, line));
        }
        out
    }

    /// The non-blank lines in no ticked note, counted a line at a time.
    fn expected_rest_lines(p: &Plan) -> usize {
        (0..p.lines())
            .filter(|&line| {
                !p.note_at(line)
                    .is_some_and(|i| p.note(i).is_some_and(|n| n.ticked()))
            })
            .filter(|&line| !p.blank(line))
            .count()
    }

    fn in_rest(p: &Plan, line: usize) -> bool {
        p.rest().runs().iter().any(|r| r.contains(&line))
    }

    #[test]
    fn the_blank_map_says_what_the_lines_say() {
        let cases = [
            "",
            "\n",
            " \t\n",
            "a",
            "a\n",
            "a\n\nb\n",
            "a\r\n\r\nb",
            "a\rb\r",
            "\r\n\r\n",
            "a\r\n\n\rb\r\n",
            "  \t \r\n \r\n",
            "\u{feff}x\n",
        ];
        for text in cases {
            let doc = buf(text);
            let total = line_count(&doc);
            let p = plan(&doc, "a.md", &[], 1);
            assert_eq!(p.lines(), total, "{text:?}");
            for line in 0..total {
                let mut s = String::new();
                doc.line_text(line, &mut s);
                assert_eq!(p.blank(line), s.trim().is_empty(), "{text:?} line {line}");
            }
            assert!(!p.blank(total), "{text:?}: no bit past the last line");
        }
    }

    #[test]
    fn a_split_at_two_hashes_gives_byte_exact_notes() {
        let doc = buf(FIXTURE);
        let p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        assert_ok(&p);
        assert_eq!(p.notes().len(), 3);
        let titles: Vec<&str> = p.notes().iter().map(|n| n.title()).collect();
        assert_eq!(
            titles,
            [
                "Planning the route",
                "Asakusa and the river",
                "Packing list"
            ]
        );
        assert_eq!(p.note(0).expect("a note").range(), 4..11);
        assert_eq!(p.note(1).expect("a note").range(), 12..17);
        assert_eq!(p.note(2).expect("a note").range(), 20..24);
        assert_eq!(
            p.note_text(0),
            "# Planning the route\n\nWe left on the third and changed trains twice.\n\n```\n## not a heading\n```\n"
        );
        assert_eq!(
            p.note_text(1),
            "# Asakusa and the river\n\n## What we did\n\nWe walked along the Sumida and ate at a stall.\n"
        );
        assert_eq!(p.note_text(2), "# Packing list\n\n- passport\n- socks\n");
        // Rebuilt from the notes, line by line, the source comes back to the byte.
        assert_eq!(rebuild(&p, &doc), FIXTURE);
        for i in 0..p.notes().len() {
            assert_eq!(p.note(i).expect("a note").bytes(), p.note_text(i).len());
        }
    }

    #[test]
    fn the_default_level_is_the_shallowest_that_occurs_twice() {
        let l = levels(&headings_of(FIXTURE));
        assert_eq!((l.count(1), l.count(2), l.count(3)), (2, 3, 1));
        assert_eq!(l.count(4), 0);
        assert_eq!(l.available().collect::<Vec<u8>>(), vec![1, 2, 3]);
        assert_eq!(l.default_level(), Some(1));
        // One `#` and two `##`: the `#` occurs once, so the cut is at `##`.
        assert_eq!(
            levels(&headings_of("# A\n\n## B\n\n## C\n")).default_level(),
            Some(2)
        );
        // Nothing occurs twice: the shallowest that occurs at all.
        assert_eq!(
            levels(&headings_of("# A\n\n## B\n")).default_level(),
            Some(1)
        );
        // No heading at a level the switch offers.
        assert_eq!(levels(&headings_of("one\ntwo\n")).default_level(), None);
        assert_eq!(levels(&[]).available().count(), 0);
    }

    #[test]
    fn a_heading_inside_a_code_fence_is_not_a_heading() {
        let doc = buf(FIXTURE);
        let heads = headings_of(FIXTURE);
        assert!(!heads.iter().any(|h| h.line == 9), "the scan skips it");
        let p = plan(&doc, "Japan trip journal.md", &heads, 2);
        assert_ok(&p);
        assert!(
            !p.notes().iter().any(|n| n.range().start == 9),
            "no note starts at the code line"
        );
        // Promotion leaves the line alone: `## not a heading` is still `## not a heading`.
        assert!(
            p.note_text(0).contains("\n```\n## not a heading\n```\n"),
            "{:?}",
            p.note_text(0)
        );
    }

    #[test]
    fn promotion_moves_every_level_and_no_word() {
        let text = "# A\n\n## B\n\n### C\n\n#### D\n\nA line with  two  spaces and a # in it.\n";
        let doc = buf(text);
        let p = plan(&doc, "a.md", &headings_of(text), 2);
        assert_ok(&p);
        assert_eq!(p.notes().len(), 1);
        assert_eq!(p.note(0).expect("a note").heading_shift(), 1);
        assert_eq!(
            p.note_text(0),
            "# B\n\n## C\n\n### D\n\nA line with  two  spaces and a # in it.\n"
        );
        let lines = p.note_lines(0);
        assert_eq!(
            lines.iter().map(|l| l.removed).collect::<Vec<u8>>(),
            vec![1, 0, 1, 0, 1, 0, 0]
        );
        assert_eq!(
            lines.iter().map(|l| l.source).collect::<Vec<_>>(),
            vec![
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(8),
                Some(9)
            ]
        );
        assert!(lines.iter().all(|l| !l.added));
        assert_eq!(rebuild(&p, &doc), text);
    }

    #[test]
    fn a_note_without_a_heading_gets_a_title_line_that_is_marked_added() {
        let doc = buf("one\ntwo\n");
        let mut p = plan(&doc, "a.md", &[], 1);
        assert_ok(&p);
        assert!(p.notes().is_empty());
        let i = p.new_note_at(0).expect("a note");
        assert_ok(&p);
        let n = p.note(i).expect("a note");
        assert!(n.title_added());
        assert_eq!(n.heading_shift(), 0);
        assert_eq!(n.title(), "Lines 1-2");
        let lines = p.note_lines(i);
        assert_eq!(lines.len(), 4);
        assert!(lines[0].added && lines[0].source.is_none());
        assert_eq!(lines[0].text, "# Lines 1-2\n");
        assert!(lines[1].added && lines[1].source.is_none());
        assert_eq!(lines[1].text, "\n");
        assert_eq!(lines[2].text, "one\n");
        assert_eq!(lines[3].text, "two\n");
        assert_eq!(p.note_text(i), "# Lines 1-2\n\none\ntwo\n");
        assert_eq!(n.bytes(), p.note_text(i).len());
    }

    #[test]
    fn the_rest_is_the_non_blank_lines_outside_ticked_notes() {
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        let check_rest = |p: &Plan| {
            assert_ok(p);
            let rest = p.rest();
            assert_eq!(rest.lines(), expected_rest_lines(p));
            assert_eq!(p.totals().lines_in_no_note, rest.lines());
            for r in rest.runs() {
                assert!(r.start < r.end);
                assert!(!p.blank(r.start) && !p.blank(r.end - 1));
            }
        };
        // As cut: the journal's title and prose, and the appendix's heading.
        assert_eq!(p.rest().runs().to_vec(), vec![0..3, 18..19]);
        assert_eq!(p.rest().lines(), 3);
        assert_eq!(p.rest().span_lines(), 4);
        check_rest(&p);

        // After a tick: the unticked chapter joins the rest with the lines round it.
        p.set_ticked(1, false);
        check_rest(&p);
        assert_eq!(p.rest().runs().to_vec(), vec![0..3, 12..19]);

        // After a merge: the note above takes the lines between them.
        assert_eq!(p.merge_with_above(2), Some(1));
        check_rest(&p);
        assert_eq!(p.note(1).expect("a note").range(), 12..24);

        // After a new note at a line of the source.
        assert!(p.new_note_at(1).is_some());
        check_rest(&p);
        assert_eq!(p.rest().runs().to_vec(), vec![0..1, 12..24]);
    }

    #[test]
    fn merge_and_a_new_note_keep_every_line_in_exactly_one_note() {
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        assert_eq!(p.merge_with_above(1), Some(0));
        assert_ok(&p);
        assert!(p.new_note_at(6).is_some());
        assert_ok(&p);
        // A blank line with no room after it makes nothing, and says so.
        assert_eq!(p.new_note_at(19), None);
        assert!(p.new_note_at(18).is_some());
        assert_ok(&p);
        let mut count = vec![0usize; p.lines()];
        for n in p.notes() {
            for line in n.range() {
                count[line] = count[line].saturating_add(1);
            }
        }
        assert!(count.iter().all(|&c| c <= 1), "a line is in two notes");
        for (line, &c) in count.iter().enumerate() {
            if c == 0 {
                assert!(
                    p.blank(line) || in_rest(&p, line),
                    "line {} is in no note and not in the rest",
                    line + 1
                );
            }
        }
    }

    #[test]
    fn a_document_with_no_headings_has_no_notes_and_all_of_it_in_the_rest() {
        let doc = buf("one\n\ntwo\n");
        let mut p = plan(&doc, "a.md", &[], 1);
        assert_ok(&p);
        assert!(p.notes().is_empty());
        assert_eq!(p.lines(), 3);
        let rest = p.rest();
        assert_eq!(rest.runs().to_vec(), vec![0..3]);
        assert_eq!(rest.lines(), 2);
        assert_eq!(p.totals().notes, 0);
        assert_eq!(p.totals().lines_in_notes, 0);
        assert!(!p.covered());
        assert!(p.warnings().contains(&Warning::NoSplit { notes: 0 }));
        // A note can still be started by hand.
        assert_eq!(p.new_note_at(0), Some(0));
        assert_ok(&p);
    }

    #[test]
    fn a_document_that_is_one_heading_is_one_note() {
        let text = "# Only\n\nSome text.\n";
        let doc = buf(text);
        let p = plan(&doc, "a.md", &headings_of(text), 1);
        assert_ok(&p);
        assert_eq!(p.notes().len(), 1);
        let n = p.note(0).expect("a note");
        assert_eq!(n.range(), 0..3);
        assert_eq!(n.title(), "Only");
        assert!(!n.title_added());
        assert_eq!(n.heading_shift(), 0);
        assert_eq!(p.note_text(0), text);
        assert!(p.covered());
        assert!(p.warnings().contains(&Warning::NoSplit { notes: 1 }));
    }

    #[test]
    fn an_empty_document_has_nothing_to_split() {
        let doc = buf("");
        let mut p = plan(&doc, "a.md", &[], 1);
        assert_ok(&p);
        assert_eq!(p.lines(), 0);
        assert!(p.notes().is_empty());
        assert!(p.rest().runs().is_empty());
        assert_eq!(p.totals().document_lines, 0);
        assert_eq!(p.note_text(0), "");
        assert_eq!(p.rest_text(), "# a (the rest)\n\n");
        assert_eq!(p.new_note_at(0), None);
        p.set_ticked(0, true);
        p.rename(0, "x");
        assert_eq!(p.merge_with_above(0), None);
        assert_ok(&p);
    }

    #[test]
    fn each_note_carries_the_sections_above_it() {
        let doc = buf(FIXTURE);
        let p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        assert_eq!(
            p.note(0).expect("a note").sections().to_vec(),
            vec!["Japan trip journal".to_string()]
        );
        assert_eq!(
            p.note(1).expect("a note").sections().to_vec(),
            vec!["Japan trip journal".to_string()]
        );
        assert_eq!(
            p.note(2).expect("a note").sections().to_vec(),
            vec!["Appendix".to_string()]
        );
        assert_eq!(
            p.sections(),
            vec![
                vec!["Japan trip journal".to_string()],
                vec!["Appendix".to_string()]
            ]
        );
        // Deeper: `### What we did` is under `## Asakusa and the river`.
        let p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 3);
        assert_eq!(p.notes().len(), 1);
        assert_eq!(p.note(0).expect("a note").title(), "What we did");
        assert_eq!(
            p.note(0).expect("a note").sections().to_vec(),
            vec![
                "Japan trip journal".to_string(),
                "Asakusa and the river".to_string()
            ]
        );
        // A note started by hand at a `#` heading of a `##` plan is at the top level: the note is
        // that section, not one of its chapters.
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        assert!(p.new_note_at(18).is_some());
        let i = p.note_at(18).expect("a note at the appendix");
        let n = p.note(i).expect("a note");
        assert_eq!(n.title(), "Appendix");
        assert!(!n.title_added());
        assert!(n.sections().is_empty(), "{:?}", n.sections());
        assert_ok(&p);
    }

    #[test]
    fn ticking_the_rest_makes_the_coverage_complete() {
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        let t = p.totals();
        assert_eq!(t.notes, 3);
        assert_eq!(t.all_notes, 3);
        assert_eq!(t.lines_in_notes, 16);
        assert_eq!(t.lines_in_no_note, 3);
        assert_eq!((t.places, t.document_lines), (2, 24));
        assert_eq!(t.document_bytes, FIXTURE.len());
        assert!(!t.covered());
        p.set_rest_ticked(true);
        let t = p.totals();
        assert_eq!(t.notes, 4);
        assert_eq!(t.lines_in_notes, 20);
        assert_eq!(t.lines_in_no_note, 0);
        assert!(t.covered());
        assert_ok(&p);
    }

    #[test]
    fn renaming_rewrites_only_a_title_the_app_added() {
        let doc = buf("one\ntwo\n");
        let mut p = plan(&doc, "a.md", &[], 1);
        let i = p.new_note_at(0).expect("a note");
        p.rename(i, "  The   beginning ");
        assert_eq!(p.note(i).expect("a note").title(), "The   beginning");
        assert_eq!(p.note_text(i), "# The   beginning\n\none\ntwo\n");
        assert_eq!(p.note(i).expect("a note").bytes(), p.note_text(i).len());
        assert_ok(&p);
        // An empty name is "Lines 1-2".
        p.rename(i, "   ");
        assert_eq!(p.note(i).expect("a note").title(), "Lines 1-2");
    }

    #[test]
    fn renaming_a_note_that_starts_with_a_heading_keeps_its_words() {
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        let before = p.note_text(0);
        p.rename(0, "Trains and timetables");
        assert_eq!(p.note(0).expect("a note").title(), "Trains and timetables");
        assert_eq!(p.note_text(0), before, "the document's words stay");
        assert_ok(&p);
    }

    #[test]
    fn a_note_started_inside_a_code_fence_starts_after_it() {
        let text = "## A\ntext\n```\ncode\n## inside\n```\nafter\n## B\nend\n";
        let doc = buf(text);
        let heads = headings_of(text);
        assert_eq!(heads.iter().map(|h| h.line).collect::<Vec<_>>(), vec![0, 7]);
        let mut p = plan(&doc, "a.md", &heads, 2);
        assert_eq!(p.notes().len(), 2);
        let i = p.new_note_at(4).expect("a note after the block");
        assert_ok(&p);
        let n = p.note(i).expect("a note");
        assert_eq!(n.first_line(), 7);
        assert!(n.boundary_moved());
        assert_eq!(n.title(), "Lines 7-7");
        assert_eq!(p.note_text(i), "# Lines 7-7\n\nafter\n");
    }

    #[test]
    fn the_counts_of_the_plan_agree_with_its_notes() {
        let doc = buf(FIXTURE);
        let p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        let t = p.totals();
        assert_eq!(t.all_notes, 3);
        assert_eq!(t.notes, 3);
        assert_eq!(t.document_lines, 24);
        assert_eq!(t.document_bytes, FIXTURE.len());
        for (i, n) in p.notes().iter().enumerate() {
            assert_eq!(n.lines(), n.range().len());
            assert_eq!(n.lines(), n.last_line() - n.first_line() + 1);
            assert_eq!(n.bytes(), p.note_text(i).len());
        }
        let sum: usize = p
            .notes()
            .iter()
            .filter(|n| n.ticked())
            .map(|n| n.lines())
            .sum();
        assert_eq!(sum, t.lines_in_notes);
    }

    #[test]
    fn the_warnings_name_the_notes_they_mean() {
        // Six one-line notes with the same title.
        let mut text = String::from("# Doc\n\n");
        for _ in 0..6 {
            text.push_str("## Same\n\n");
        }
        let doc = buf(&text);
        let p = plan(&doc, "a.md", &headings_of(&text), 2);
        assert_eq!(p.notes().len(), 6);
        let w = p.warnings();
        let same = w
            .iter()
            .find(|w| matches!(w, Warning::SameTitle { .. }))
            .expect("a same-title warning");
        assert_eq!(
            same,
            &Warning::SameTitle {
                title: "Same".to_string(),
                notes: vec![0, 1, 2, 3, 4, 5]
            }
        );
        assert!(
            w.iter()
                .any(|w| matches!(w, Warning::ManyShortNotes { .. }))
        );
        assert!(!w.iter().any(|w| matches!(w, Warning::NotAtAHeading { .. })));
        assert!(!w.iter().any(|w| matches!(w, Warning::NoSplit { .. })));

        // One note holds most of a document of more than ten lines.
        let mut text = String::from("# Doc\n\n## Big\n");
        for i in 0..18 {
            text.push_str(&format!("line {i}\n"));
        }
        text.push_str("\n## Small\none line\n");
        let doc = buf(&text);
        let p = plan(&doc, "a.md", &headings_of(&text), 2);
        let w = p.warnings();
        let most = w
            .iter()
            .find_map(|w| match w {
                Warning::OneNoteHoldsMost { note, lines, .. } => Some((*note, *lines)),
                _ => None,
            })
            .expect("a most-lines warning");
        assert_eq!(most, (0, 19));

        // A note that does not start at a heading, while the others do.
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        assert!(p.new_note_at(1).is_some());
        let w = p.warnings();
        let untitled = w
            .iter()
            .find_map(|w| match w {
                Warning::NotAtAHeading { notes } => Some(notes.clone()),
                _ => None,
            })
            .expect("a not-at-a-heading warning");
        assert_eq!(untitled.len(), 1);
        assert!(p.note(untitled[0]).expect("a note").title_added());
    }

    #[test]
    fn a_hundred_operations_leave_the_invariant_intact() {
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        let mut rng = Rng::new(17);
        for step in 0..200 {
            let n = p.notes().len();
            match rng.below(6) {
                0 if n > 0 => p.set_ticked(rng.below(n), rng.chance(50)),
                1 if n > 0 => p.rename(rng.below(n), &format!("Note {step}")),
                2 if n > 1 => {
                    p.merge_with_above(rng.below(n));
                }
                3 => {
                    p.new_note_at(rng.below(p.lines()));
                }
                4 if n > 0 => p.move_to_section(rng.below(n), &["A", "B"]),
                _ => p.set_rest_ticked(rng.chance(50)),
            }
            if let Err(e) = p.check() {
                panic!("after step {step}: {e}");
            }
        }
    }

    #[test]
    fn check_finds_two_notes_over_the_same_line() {
        let doc = buf("a\nb\nc\n");
        let mut p = plan(&doc, "a.md", &[], 1);
        p.new_note_at(0).expect("a note");
        let copy = p.notes[0].clone();
        p.notes.push(copy);
        assert!(p.check().is_err());
    }

    #[test]
    fn a_large_document_is_cut_in_one_pass() {
        let mut text = String::new();
        for i in 0..1000 {
            text.push_str(&format!("## Chapter {i}\n"));
            for j in 0..200 {
                text.push_str(&format!("line {j}\n"));
            }
            text.push('\n');
        }
        let doc = buf(&text);
        let started = Instant::now();
        let p = plan(&doc, "big.md", &headings_of(&text), 2);
        let took = started.elapsed();
        assert_eq!(p.notes().len(), 1000);
        assert!(!p.capped());
        assert_eq!(p.lines(), 1000 * 202);
        assert_eq!(p.note(999).expect("a note").range().start, 999 * 202);
        assert_ok(&p);
        eprintln!("split of 201,000 lines into 1000 notes: {took:?}");
    }

    #[test]
    fn crlf_line_endings_are_kept_and_a_missing_last_one_is_added() {
        let text = "# A\r\n\r\n## B\r\n\r\ntext";
        let doc = buf(text);
        let p = plan(&doc, "a.md", &headings_of(text), 2);
        assert_ok(&p);
        assert_eq!(p.lines(), 5);
        assert_eq!(p.notes().len(), 1);
        let n = p.note(0).expect("a note");
        assert_eq!(n.range(), 2..5);
        assert_eq!(n.sections().to_vec(), vec!["A".to_string()]);
        assert_eq!(p.note_text(0), "# B\r\n\r\ntext\r\n");
        assert_eq!(n.bytes(), 13);
    }

    #[test]
    fn section_names_are_one_line_and_cut_to_the_folder_length() {
        let doc = buf(FIXTURE);
        let mut p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        let long = "x".repeat(SPLIT_SECTION_MAX + 10);
        p.move_to_section(0, &["  Tokyo  ", "", &long, "a", "b"]);
        assert_ok(&p);
        let sections = p.note(0).expect("a note").sections().to_vec();
        assert_eq!(sections.len(), SPLIT_MAX_SECTIONS);
        assert_eq!(sections[0], "Tokyo");
        assert_eq!(sections[1].len(), SPLIT_SECTION_MAX);
        // An empty path is the top level, and `sections()` lists it.
        p.move_to_section(0, &["", "   "]);
        assert!(p.note(0).expect("a note").sections().is_empty());
        assert_ok(&p);
        assert!(p.sections().contains(&Vec::<String>::new()));
    }

    #[test]
    fn a_plan_stops_at_the_most_notes_it_holds() {
        let mut text = String::from("# Doc\n");
        for i in 0..(SPLIT_MAX_NOTES + 5) {
            text.push_str(&format!("## N{i}\nx\n"));
        }
        let doc = buf(&text);
        let p = plan(&doc, "a.md", &headings_of(&text), 2);
        assert_eq!(p.notes().len(), SPLIT_MAX_NOTES);
        assert!(p.capped());
        assert_ok(&p);
        // The lines of the notes not made are in the rest, not lost.
        assert_eq!(p.rest().lines(), 11);
        assert_eq!(p.rest().runs().len(), 2);
    }

    #[test]
    fn the_rest_is_written_with_a_label_above_each_run() {
        let doc = buf(FIXTURE);
        let p = plan(&doc, "Japan trip journal.md", &headings_of(FIXTURE), 2);
        assert_eq!(
            p.rest_text(),
            "# Japan trip journal (the rest)\n\n\
             <!-- lines 1-3 of Japan trip journal.md -->\n\
             # Japan trip journal\n\n\
             Notes from two weeks in October, written on trains.\n\
             <!-- lines 19-19 of Japan trip journal.md -->\n\
             # Appendix\n"
        );
        assert_eq!(p.rest().title(), "Japan trip journal (the rest)");
        assert!(p.rest().title_added());
    }
}
