//! Write notes' plan card (`docs/ai-pictures-plan.md` step 5, phase V4): the notes the arrange
//! made, as a tree at the left (the group, its sections, the notes, each note's pictures with what
//! the model said of them, and the pictures no note shows), and the selected note at the right as
//! it will read, or as its Markdown.
//!
//! The card is a view of `notes_task::Run::arranged` and its edits are applied to it, for that is
//! what the write reads: a note left out is out of `arranged.notes`, a picture no ticked note
//! shows is in `arranged.unused`, a rename is the draft's own title or sections, and "Put in…"
//! moves the picture's part from one body to another. The card keeps what `arranged` cannot: the
//! notes left out (so they can be ticked again) and where each picture left out came from. Its
//! drawing is `notes_plan_draw.rs`, the window's part `app/notes_plan.rs`. Nothing here exists
//! until a plan is ready, so nothing is on the launch path.

use crate::notes_task::Run;
use crate::task_ui::{DestChoice, TaskScreen};
use mdedit_core::notes_ai::{self, Arranged, Compose, Draft, Part};
use mdshared_ui::gfx::Rect;
use mdshared_ui::menu::Menu;
use mdshared_ui::textfield::TextField;
use std::collections::HashMap;

/// The card's size (plan section 5); a smaller window gets a smaller card.
pub const CARD_W: f32 = 1120.0;
pub const CARD_H: f32 = 720.0;
/// The tree's width at full size.
pub const LEFT_W: f32 = 530.0;
/// The header's height, under which the tree and the note start.
pub const HEAD_H: f32 = 64.0;
/// The strip over the note: its file and the switch.
pub const STRIP_H: f32 = 44.0;
/// How far the tree moves in for each level.
pub const INDENT: f32 = 20.0;

/// What a row of the tree stands for: kept by what it is, not by its place, so an edit that
/// moves rows keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Sel {
    Group,
    /// A section, by its path of names.
    Section(Vec<String>),
    /// A note, by the card's key for it.
    Note(u64),
    /// A picture, by its place in the window.
    Picture(usize),
}

/// A row of the tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The group the notes go to: the window's destination.
    Group,
    /// A section: a heading in the group's file and a folder on disk.
    Section { path: Vec<String> },
    /// A note; `depth` is how many sections it is under.
    Note { key: u64, on: bool, depth: usize },
    /// A heading inside a note, over the pictures that follow it.
    Heading { text: String, depth: usize },
    /// A picture: in the note `key` (ticked while that note is), or in none (`key` `None`).
    Picture {
        id: usize,
        key: Option<u64>,
        on: bool,
        depth: usize,
    },
    /// The line over the pictures no note shows.
    Unused,
}

impl Row {
    pub fn height(&self) -> f32 {
        match self {
            Row::Group => 32.0,
            Row::Section { .. } => 28.0,
            Row::Note { .. } => 32.0,
            Row::Heading { .. } => 24.0,
            Row::Picture { .. } => 40.0,
            Row::Unused => 40.0,
        }
    }

    /// What selecting it selects; a heading and the line over the unused are not selected.
    pub fn sel(&self) -> Option<Sel> {
        match self {
            Row::Group => Some(Sel::Group),
            Row::Section { path } => Some(Sel::Section(path.clone())),
            Row::Note { key, .. } => Some(Sel::Note(*key)),
            Row::Picture { id, .. } => Some(Sel::Picture(*id)),
            Row::Heading { .. } | Row::Unused => None,
        }
    }
}

/// A note of the plan, ticked (in `arranged.notes`) or left out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: u64,
    pub on: bool,
    pub draft: Draft,
}

/// Where a picture left out was, so ticking it puts it back there.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Origin {
    key: u64,
    at: usize,
    alt: String,
}

/// A line of the row menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Rename,
    LeaveOut,
    /// Tick it again.
    Keep,
    /// "Put in…": the list of notes follows.
    PutIn,
    /// "Join with…": the list of notes follows.
    JoinWith,
    /// A note of that list.
    Into(u64),
}

/// What the pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Hit {
    Card,
    /// A row of the tree, by its place in [`NotesPlan::laid`].
    Row(usize),
    /// A row's tick.
    Tick(usize),
    /// A side of the switch: 0 as it will read, 1 Markdown.
    View(usize),
    Discard,
    Back,
    Again,
    Write,
    TreeTrack,
    PageTrack,
    AgainField,
    AgainGo,
    AgainClose,
}

/// Which list a scrollbar drag moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bar {
    Tree,
    Page,
}

/// The row menu: for which row, and whether it is the list of notes "Put in…" or "Join with…"
/// opened.
pub struct RowMenu {
    pub menu: Menu<Act>,
    pub sel: Sel,
    pub then: Option<Act>,
}

pub struct NotesPlan {
    /// The notes, ticked or left out, in the answer's order.
    pub entries: Vec<Entry>,
    /// Every picture the plan was made of, in the window's order.
    pub pictures: Vec<usize>,
    origins: HashMap<usize, Origin>,
    /// Notes the reader dropped (check 9), carried as the answer had them.
    pub dropped: usize,
    /// What `arranged` was made from this card last: a plan changed since is another plan.
    pub synced: Arranged,
    /// On screen; `false` while the window is back in front with the plan kept (D-053).
    pub shown: bool,
    pub sel: Option<Sel>,
    /// The note shown at the right.
    pub note: Option<u64>,
    /// The right side shows the Markdown.
    pub markdown: bool,
    pub tree_scroll: f32,
    pub page_scroll: f32,
    /// What the last drawing laid out: the rows, and the heights to scroll.
    pub laid: Vec<Row>,
    pub tree_content: f32,
    pub page_content: f32,
    pub tree_area: Rect,
    pub page_area: Rect,
    /// F2: the row being renamed and its name as typed.
    pub edit: Option<(Sel, TextField)>,
    pub menu: Option<RowMenu>,
    /// "Arrange again…" open: its instruction.
    pub again: Option<TextField>,
    /// A scrollbar held: which, and where on its thumb.
    pub grab: Option<(Bar, f32)>,
    pub hover: Option<Hit>,
    pub pressed: Option<Hit>,
    pub hits: Vec<(Hit, Rect)>,
    pub tick: u64,
}

/// Where a note goes, as a key two notes of one path share (`notes_ai`'s rule).
fn path_key(d: &Draft) -> String {
    let mut k = d.folders().join("/");
    k.push('/');
    k.push_str(&d.file_name());
    k.to_lowercase()
}

/// The text of a heading line (`## Text`), if the line is one.
fn heading_of(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start_matches(' ');
    if line.len() - t.len() > 3 {
        return None;
    }
    let level = t.chars().take_while(|&c| c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &t[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    Some((level, plain(rest.trim().trim_end_matches('#').trim())))
}

/// Markdown's escapes read for the eye: `\[` is `[`, `&lt;` is `<`.
pub fn plain(text: &str) -> String {
    let text = text.replace("&lt;", "<").replace("&amp;", "&");
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\'
            && let Some(&n) = it.peek()
            && n.is_ascii_punctuation()
        {
            out.push(n);
            it.next();
            continue;
        }
        out.push(c);
    }
    out
}

impl NotesPlan {
    pub fn new(a: &Arranged) -> NotesPlan {
        let entries: Vec<Entry> = a
            .notes
            .iter()
            .enumerate()
            .map(|(k, d)| Entry {
                key: k as u64,
                on: true,
                draft: d.clone(),
            })
            .collect();
        let mut pictures: Vec<usize> = entries
            .iter()
            .flat_map(|e| e.draft.pictures())
            .chain(a.unused.iter().copied())
            .collect();
        pictures.sort_unstable();
        pictures.dedup();
        let first = entries.first().map(|e| e.key);
        NotesPlan {
            entries,
            pictures,
            origins: HashMap::new(),
            dropped: a.dropped,
            synced: a.clone(),
            shown: true,
            sel: first.map(Sel::Note),
            note: first,
            markdown: false,
            tree_scroll: 0.0,
            page_scroll: 0.0,
            laid: Vec::new(),
            tree_content: 0.0,
            page_content: 0.0,
            tree_area: Rect::default(),
            page_area: Rect::default(),
            edit: None,
            menu: None,
            again: None,
            grab: None,
            hover: None,
            pressed: None,
            hits: Vec::new(),
            tick: 0,
        }
    }

    /// The plan as the write reads it: the ticked notes, and every picture none of them shows.
    pub fn arranged(&self) -> Arranged {
        let notes: Vec<Draft> = self
            .entries
            .iter()
            .filter(|e| e.on)
            .map(|e| e.draft.clone())
            .collect();
        let unused = self
            .pictures
            .iter()
            .copied()
            .filter(|id| !notes.iter().any(|d| d.pictures().contains(id)))
            .collect();
        Arranged {
            notes,
            unused,
            dropped: self.dropped,
        }
    }

    pub fn entry(&self, key: u64) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    fn entry_mut(&mut self, key: u64) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.key == key)
    }

    /// The note whose body shows the picture, ticked or not.
    pub fn holder(&self, id: usize) -> Option<u64> {
        self.entries
            .iter()
            .find(|e| e.draft.pictures().contains(&id))
            .map(|e| e.key)
    }

    /// How many notes are ticked: the button writes them.
    pub fn on_notes(&self) -> usize {
        self.entries.iter().filter(|e| e.on).count()
    }

    /// The pictures the ticked notes show.
    pub fn on_pictures(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.on)
            .map(|e| e.draft.pictures().len())
            .sum()
    }

    /// The note shown at the right: the one selected or holding the picture selected, else the
    /// last one shown while it is still in the plan, else the first.
    pub fn shown_note(&self) -> Option<u64> {
        let from_sel = match &self.sel {
            Some(Sel::Note(k)) => Some(*k),
            Some(Sel::Picture(id)) => self.holder(*id),
            _ => None,
        };
        from_sel
            .or(self.note)
            .filter(|k| self.entry(*k).is_some())
            .or_else(|| self.entries.iter().find(|e| e.on).map(|e| e.key))
            .or_else(|| self.entries.first().map(|e| e.key))
    }

    /// The tick of a row changes: a note is left out or kept; a picture in a ticked note is left
    /// out, and one in no note is put back where it was.
    pub fn toggle(&mut self, sel: &Sel) -> bool {
        match sel {
            Sel::Note(k) => match self.entry_mut(*k) {
                Some(e) => {
                    e.on = !e.on;
                    true
                }
                None => false,
            },
            Sel::Picture(id) => match self.holder(*id) {
                Some(k) if self.entry(k).is_some_and(|e| e.on) => self.leave_out_picture(*id),
                // A picture in a note left out goes with it: "Put in…" takes it elsewhere.
                Some(_) => false,
                None => self.put_back(*id),
            },
            _ => false,
        }
    }

    /// "Leave out": a ticked note, or a picture of one.
    pub fn leave_out(&mut self, sel: &Sel) -> bool {
        match sel {
            Sel::Note(k) => match self.entry_mut(*k).filter(|e| e.on) {
                Some(e) => {
                    e.on = false;
                    true
                }
                None => false,
            },
            Sel::Picture(id) => {
                self.holder(*id)
                    .is_some_and(|k| self.entry(k).is_some_and(|e| e.on))
                    && self.leave_out_picture(*id)
            }
            _ => false,
        }
    }

    /// Takes the picture's part out of its note's body; returns its words and where it was.
    fn take_picture(&mut self, id: usize) -> Option<(u64, usize, String)> {
        let key = self.holder(id)?;
        let e = self.entry_mut(key)?;
        let at = e
            .draft
            .body
            .iter()
            .position(|p| matches!(p, Part::Picture { id: i, .. } if *i == id))?;
        let alt = match e.draft.body.remove(at) {
            Part::Picture { alt, .. } => alt,
            Part::Text(_) => String::new(),
        };
        Some((key, at, alt))
    }

    fn leave_out_picture(&mut self, id: usize) -> bool {
        match self.take_picture(id) {
            Some((key, at, alt)) => {
                self.origins.insert(id, Origin { key, at, alt });
                true
            }
            None => false,
        }
    }

    /// A picture in no note ticked again: back where it was when that note is ticked, else at
    /// the end of the note shown.
    fn put_back(&mut self, id: usize) -> bool {
        let origin = self.origins.get(&id).cloned();
        let back = origin
            .as_ref()
            .filter(|o| self.entry(o.key).is_some_and(|e| e.on));
        let (key, at, alt) = match back {
            Some(o) => (o.key, Some(o.at), o.alt.clone()),
            None => {
                let Some(key) = self
                    .shown_note()
                    .filter(|k| self.entry(*k).is_some_and(|e| e.on))
                    .or_else(|| self.entries.iter().find(|e| e.on).map(|e| e.key))
                else {
                    return false;
                };
                (key, None, origin.map(|o| o.alt).unwrap_or_default())
            }
        };
        let Some(e) = self.entry_mut(key) else {
            return false;
        };
        let at = at.map_or(e.draft.body.len(), |a| a.min(e.draft.body.len()));
        e.draft.body.insert(at, Part::Picture { id, alt });
        self.origins.remove(&id);
        true
    }

    /// "Put in…": the picture moves to the end of note `key`'s body, with its words.
    pub fn put_in(&mut self, id: usize, key: u64) -> bool {
        if !self.pictures.contains(&id)
            || self.holder(id) == Some(key)
            || !self.entry(key).is_some_and(|e| e.on)
        {
            return false;
        }
        let alt = match self.take_picture(id) {
            Some((_, _, alt)) => alt,
            None => self.origins.remove(&id).map(|o| o.alt).unwrap_or_default(),
        };
        self.origins.remove(&id);
        let Some(e) = self.entry_mut(key) else {
            return false;
        };
        e.draft.body.push(Part::Picture { id, alt });
        true
    }

    /// "Join with…": note `from` goes under its title as a heading at the end of note `into`,
    /// as "One note" joins an answer of several (`notes_ai::joined`).
    pub fn join(&mut self, into: u64, from: u64) -> bool {
        if into == from
            || !self.entry(into).is_some_and(|e| e.on)
            || !self.entry(from).is_some_and(|e| e.on)
        {
            return false;
        }
        let Some(at) = self.entries.iter().position(|e| e.key == from) else {
            return false;
        };
        let gone = self.entries.remove(at);
        let Some(e) = self.entry_mut(into) else {
            return false;
        };
        let joined = notes_ai::joined(Arranged {
            notes: vec![e.draft.clone(), gone.draft],
            ..Arranged::default()
        });
        if let Some(d) = joined.notes.into_iter().next() {
            e.draft = d;
        }
        for o in self.origins.values_mut() {
            if o.key == from {
                o.key = into;
                o.at = usize::MAX;
            }
        }
        if self.note == Some(from) {
            self.note = Some(into);
        }
        if self.sel == Some(Sel::Note(from)) {
            self.sel = Some(Sel::Note(into));
        }
        true
    }

    /// Whether another ticked note than `key` would go where `d` goes.
    fn clashes(&self, key: u64, d: &Draft) -> bool {
        let k = path_key(d);
        self.entries
            .iter()
            .any(|e| e.key != key && e.on && path_key(&e.draft) == k)
    }

    /// F2 on a note: its title, by the title rule (check 3).
    pub fn rename_note(&mut self, key: u64, text: &str) -> Result<(), String> {
        let title = notes_ai::clean_title(text);
        if title.is_empty() {
            return Err("A note needs a title.".to_string());
        }
        let Some(e) = self.entry(key) else {
            return Err("That note is no longer in the plan.".to_string());
        };
        let mut d = e.draft.clone();
        d.title = title;
        if e.on && self.clashes(key, &d) {
            return Err(format!(
                "Another note would be {}: give it another title.",
                d.file_name()
            ));
        }
        if let Some(e) = self.entry_mut(key) {
            e.draft = d;
        }
        Ok(())
    }

    /// F2 on a section: its name in every note under it, by the sections' rule (check 3).
    pub fn rename_section(&mut self, path: &[String], text: &str) -> Result<(), String> {
        let Some(name) = notes_ai::clean_sections(&[text.to_string()])
            .into_iter()
            .next()
        else {
            return Err("A section needs a name.".to_string());
        };
        let Some(last) = path.len().checked_sub(1) else {
            return Err("That is not a section.".to_string());
        };
        let before = self.entries.clone();
        for e in self.entries.iter_mut() {
            if e.draft.sections.starts_with(path) {
                e.draft.sections[last] = name.clone();
            }
        }
        let on: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.on)
            .map(|e| path_key(&e.draft))
            .collect();
        let mut seen = on.clone();
        seen.sort();
        seen.dedup();
        if seen.len() != on.len() {
            self.entries = before;
            return Err(format!(
                "Two notes would be one file under {name}: rename one of them first."
            ));
        }
        if let Some(Sel::Section(p)) = self.sel.as_mut()
            && p.as_slice() == path
        {
            p[last] = name;
        }
        Ok(())
    }

    /// The tree: the group, then at each level the notes there and the sections under it; each
    /// note's pictures under it (with the heading over them, when it has headings); then the
    /// pictures no note shows.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = vec![Row::Group];
        self.level(&[], &mut rows);
        let unused: Vec<usize> = self
            .pictures
            .iter()
            .copied()
            .filter(|&id| self.holder(id).is_none())
            .collect();
        if !unused.is_empty() {
            rows.push(Row::Unused);
            for id in unused {
                rows.push(Row::Picture {
                    id,
                    key: None,
                    on: false,
                    depth: 0,
                });
            }
        }
        rows
    }

    fn level(&self, prefix: &[String], rows: &mut Vec<Row>) {
        let depth = prefix.len();
        for e in self.entries.iter().filter(|e| e.draft.sections == prefix) {
            rows.push(Row::Note {
                key: e.key,
                on: e.on,
                depth,
            });
            let mut heading: Option<String> = None;
            let mut said: Option<String> = None;
            for part in &e.draft.body {
                match part {
                    Part::Text(t) => {
                        for line in t.lines() {
                            if let Some((_, h)) = heading_of(line).filter(|(_, h)| !h.is_empty()) {
                                heading = Some(h);
                            }
                        }
                    }
                    Part::Picture { id, .. } => {
                        if let Some(h) = heading.as_ref().filter(|h| said.as_ref() != Some(*h)) {
                            rows.push(Row::Heading {
                                text: h.clone(),
                                depth: depth + 1,
                            });
                            said = Some(h.clone());
                        }
                        rows.push(Row::Picture {
                            id: *id,
                            key: Some(e.key),
                            on: e.on,
                            depth: depth + 1,
                        });
                    }
                }
            }
        }
        let mut names: Vec<&String> = Vec::new();
        for e in &self.entries {
            if e.draft.sections.len() > depth
                && e.draft.sections.starts_with(prefix)
                && !names.contains(&&e.draft.sections[depth])
            {
                names.push(&e.draft.sections[depth]);
            }
        }
        for name in names {
            let mut path = prefix.to_vec();
            path.push(name.clone());
            rows.push(Row::Section { path: path.clone() });
            self.level(&path, rows);
        }
    }

    /// The selectable rows' choices, in the tree's order, for Up and Down.
    pub fn selectable(&self) -> Vec<Sel> {
        self.rows().iter().filter_map(Row::sel).collect()
    }

    pub fn sig(&self, caret_on: bool) -> (u64, Option<Hit>, Option<Hit>, bool) {
        let typing = self.edit.is_some() || self.again.is_some();
        (self.tick, self.hover, self.pressed, typing && caret_on)
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(h, _)| *h)
    }

    pub fn max_scroll(&self, bar: Bar) -> f32 {
        match bar {
            Bar::Tree => (self.tree_content - self.tree_area.h).max(0.0),
            Bar::Page => (self.page_content - self.page_area.h).max(0.0),
        }
    }

    /// Scrolls one list by `dy`, kept inside it.
    pub fn scroll_by(&mut self, bar: Bar, dy: f32) {
        let max = self.max_scroll(bar);
        match bar {
            Bar::Tree => self.tree_scroll = (self.tree_scroll + dy).clamp(0.0, max),
            Bar::Page => self.page_scroll = (self.page_scroll + dy).clamp(0.0, max),
        }
        self.tick += 1;
    }
}

/// The file name a picture is copied as (plan section 9): the owner's own, when renamed here;
/// else the look's title by the name rule when "Name the copies after what they show" is on, or
/// the file's own name.
pub fn copy_name(s: &TaskScreen, run: &Run, id: usize) -> String {
    if let Some(name) = run.copy_names.get(&id) {
        return name.clone();
    }
    let Some(f) = s.files.get(id) else {
        return format!("picture-{id}");
    };
    let title = run
        .look(id)
        .map(|l| l.title.as_str())
        .filter(|_| s.notes.copy && s.notes.rename);
    notes_ai::picture_name(title, &f.name, &extension(&f.path))
}

fn extension(path: &std::path::Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The name the owner typed for a picture's copy, with the original's extension: its own
/// spelling kept but for what a file name cannot hold, the extension added when it was not typed.
/// `None` when nothing was typed: the copy then takes its name as before.
pub fn owner_copy_name(typed: &str, path: &std::path::Path) -> Option<String> {
    let ext = extension(path);
    let mut stem = typed.trim();
    if !ext.is_empty()
        && let Some(cut) = stem.len().checked_sub(ext.len() + 1)
        && stem.is_char_boundary(cut)
        && stem[cut..].eq_ignore_ascii_case(&format!(".{ext}"))
    {
        stem = stem[..cut].trim_end();
    }
    if stem.is_empty() {
        return None;
    }
    Some(notes_ai::picture_name(None, &format!("{stem}.{ext}"), &ext))
}

/// The note as it is written (`notes_ai::compose`), each picture by the name it is copied as.
pub fn markdown(s: &TaskScreen, run: &Run, d: &Draft) -> String {
    let name = |id: usize| Some(copy_name(s, run, id));
    notes_ai::compose(
        d,
        &Compose {
            looks: &run.looks,
            name: &name,
            text_out: s.notes.text_out,
        },
    )
}

/// A block of the note as it will read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// A heading and its level; the title is level 1.
    Heading(usize, String),
    Para(String),
    Quote(String),
    Code(String),
    /// A picture, and the words in its brackets.
    Picture(usize, String),
}

/// The note read simply, for the right side: its title, its paragraphs, headings, quotes and
/// code, and each picture where it stands, its text under it when that is written out.
pub fn blocks(s: &TaskScreen, run: &Run, d: &Draft) -> Vec<Block> {
    let mut out = vec![Block::Heading(1, d.title.clone())];
    let mut para: Vec<String> = Vec::new();
    let mut code: Option<(char, Vec<String>)> = None;
    let flush = |para: &mut Vec<String>, out: &mut Vec<Block>| {
        if !para.is_empty() {
            out.push(Block::Para(para.join(" ")));
            para.clear();
        }
    };
    for part in &d.body {
        match part {
            Part::Text(t) => {
                for line in t.split('\n') {
                    if let Some((ch, lines)) = code.as_mut() {
                        let rest = line.trim_start();
                        if rest.starts_with(*ch) && rest.trim_start_matches(*ch).trim().is_empty() {
                            out.push(Block::Code(lines.join("\n")));
                            code = None;
                        } else {
                            lines.push(line.to_string());
                        }
                        continue;
                    }
                    if line.starts_with("```") || line.starts_with("~~~") {
                        flush(&mut para, &mut out);
                        let ch = if line.starts_with('~') { '~' } else { '`' };
                        code = Some((ch, Vec::new()));
                        continue;
                    }
                    if line.trim().is_empty() {
                        flush(&mut para, &mut out);
                    } else if let Some((level, h)) = heading_of(line) {
                        flush(&mut para, &mut out);
                        out.push(Block::Heading(level.max(2), h));
                    } else if let Some(q) = line.trim_start().strip_prefix('>') {
                        flush(&mut para, &mut out);
                        let q = plain(q.trim());
                        match out.last_mut() {
                            Some(Block::Quote(prev)) => {
                                prev.push(' ');
                                prev.push_str(&q);
                            }
                            _ => out.push(Block::Quote(q)),
                        }
                    } else if let Some(item) = line
                        .trim_start()
                        .strip_prefix("- ")
                        .or_else(|| line.trim_start().strip_prefix("* "))
                    {
                        flush(&mut para, &mut out);
                        out.push(Block::Para(format!("\u{2022} {}", plain(item.trim()))));
                    } else {
                        para.push(plain(line.trim()));
                    }
                }
            }
            Part::Picture { id, alt } => {
                flush(&mut para, &mut out);
                let look = run.look(*id);
                let words = match alt.trim() {
                    "" => look.map(|l| l.says.clone()).unwrap_or_default(),
                    a => plain(a),
                };
                out.push(Block::Picture(*id, words));
                if s.notes.text_out
                    && let Some(l) = look.filter(|l| !l.text.trim().is_empty())
                {
                    out.push(Block::Quote(l.text.trim().replace('\n', " ")));
                }
            }
        }
    }
    if let Some((_, lines)) = code {
        out.push(Block::Code(lines.join("\n")));
    }
    flush(&mut para, &mut out);
    out
}

/// The group's name and the line beside it: whether it is new, and its folder.
pub fn group_line(s: &TaskScreen) -> (String, String) {
    let shown = |p: &std::path::Path| -> String {
        let sep = std::path::MAIN_SEPARATOR;
        if !s.library.as_os_str().is_empty()
            && let Ok(rel) = p.strip_prefix(&s.library)
        {
            return format!("library{sep}{}{sep}", rel.display());
        }
        format!("{}{sep}", p.display())
    };
    match &s.dest {
        DestChoice::Group(i) => match s.groups.get(*i) {
            Some(g) => (g.name.clone(), shown(&g.home)),
            None => (
                "No group".to_string(),
                "choose one in the window".to_string(),
            ),
        },
        DestChoice::New(name) if !name.trim().is_empty() => {
            let home = s
                .library
                .join(mdshared_app::library::folder_name(name.trim()));
            (
                name.trim().to_string(),
                format!("a new group \u{00B7} {}", shown(&home)),
            )
        }
        _ => (
            "No group chosen".to_string(),
            "choose where the notes go in the window".to_string(),
        ),
    }
}

/// Why nothing can be written now, said on the footer.
pub fn write_blocked(s: &TaskScreen, plan: &NotesPlan) -> Option<String> {
    if plan.on_notes() == 0 {
        return Some("No note is ticked.".to_string());
    }
    // The write needs the group itself: a new one's name typed but not yet made is not one.
    match s.group() {
        Some(_) => None,
        None => Some("Choose where the notes go in the window first: Back.".to_string()),
    }
}

/// The file the note shown is written as, under the group's folder: `Sign-in\sign-in-fails.md`.
pub fn note_path(d: &Draft) -> String {
    let mut parts = d.folders();
    parts.push(d.file_name());
    parts.join(std::path::MAIN_SEPARATOR_STR)
}

/// "2 notes", "1 picture".
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}
