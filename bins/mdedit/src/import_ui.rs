//! "Add a folder to the library…" (`docs/ai-library-plan.md` 5.7, phase 4a): the card that shows
//! what an import would list, one row per file under its subfolders' headings, and lets the rows be
//! ticked. The plan is `mdedit_core::import`; this is its state, its events and the numbers of its
//! layout (`import_draw.rs` draws it, `app/importing.rs` is the window's part). Nothing here reads
//! or writes the disk.
//!
//! It is the split's plan card made for a folder: the same backdrop, card, tick boxes, list
//! arithmetic, footer buttons and a preview of the selected file, with section headings rather
//! than the coverage strip.

use crate::keys::Key;
use mdedit_core::import::{self, Plan};
use mdedit_core::import_ai::{self, Scheme};
use mdshared_ui::gfx::Rect;
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::overlay::{DIALOG_FOOTER, dialog_card};
use mdshared_ui::rows::{Nav, Rows};
use mdshared_ui::scroll::offset_for_thumb;
use mdshared_ui::textfield::TextField;
use std::path::PathBuf;

// ---- the numbers of the layout ----------------------------------------------------------------------

pub const CARD_W: f32 = 1040.0;
pub const CARD_H: f32 = 700.0;
pub const HEAD_H: f32 = 112.0;
pub const LIST_HEAD: f32 = 40.0;
/// A section's heading, and a file: two lines, the title and where it is.
pub const ROW_H: f32 = 46.0;
/// The card is this wide before the preview is dropped and the list takes the width (5.3's rule
/// for the split's card, kept here).
pub const PREVIEW_MIN: f32 = 860.0;
pub const PREVIEW_W: f32 = 400.0;
/// The card a preview is shown in has a list this wide at least.
pub const LIST_MIN: f32 = 380.0;

/// The card for a `w` x `h` window.
pub fn card(w: f32, h: f32) -> Rect {
    dialog_card(CARD_W, CARD_H, w, h)
}

/// Where the parts of the card go.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub card: Rect,
    pub header: Rect,
    pub list_head: Rect,
    pub list: Rect,
    /// The preview of the selected row, `None` when the card is too narrow for one.
    pub preview: Option<Rect>,
}

pub fn geo(card: Rect) -> Geo {
    let header = Rect::new(card.x, card.y, card.w, HEAD_H);
    let footer = Rect::new(card.x, card.bottom() - DIALOG_FOOTER, card.w, DIALOG_FOOTER);
    let list_head = Rect::new(card.x, header.bottom(), card.w, LIST_HEAD);
    let body = (footer.y - list_head.bottom() - 4.0).max(0.0);
    let (list, preview) = if card.w >= PREVIEW_MIN {
        let split = (card.w - PREVIEW_W).max(LIST_MIN);
        (
            Rect::new(card.x, list_head.bottom(), split, body),
            Some(Rect::new(
                card.x + split,
                list_head.bottom(),
                card.w - split,
                body,
            )),
        )
    } else {
        (Rect::new(card.x, list_head.bottom(), card.w, body), None)
    };
    Geo {
        card,
        header,
        list_head,
        list,
        preview,
    }
}

/// A row of the scheme list: one line, so more sections fit than files do.
pub const SCHEME_ROW_H: f32 = 34.0;

// ---- what a row says ---------------------------------------------------------------------------------

/// What the card is about, in its title.
pub fn title() -> &'static str {
    "Add a folder to the library"
}

/// The line under the title: which folder, and what is in it.
pub fn subtitle(plan: &Plan) -> String {
    let files = plan.notes.len();
    let folders = plan.folders.max(1);
    format!(
        "{} \u{00B7} {} in {}",
        plan.folder.display(),
        count(files, "file", "files"),
        count(folders, "folder", "folders")
    )
}

/// The group the links go in, and where its notes are: the folder a group the library has names
/// itself, which need not be the folder picked (audit 4, C4).
pub fn group_line(plan: &Plan) -> String {
    let folder = if plan.group_new {
        format!(
            "which this run makes, with its folder {}",
            folder_shown(plan)
        )
    } else if plan.home.as_os_str().is_empty() {
        "which the library already has, with no folder of its own".to_string()
    } else {
        format!(
            "which the library already has, with its folder {}",
            folder_shown(plan)
        )
    };
    format!(
        "The group is \u{201c}{}\u{201d}, {folder}.",
        plan.group_name
    )
}

/// A group's folder as the card says it: below the library when it is inside it, else whole.
fn folder_shown(plan: &Plan) -> String {
    match plan.home.strip_prefix(&plan.library) {
        Ok(inside) if !inside.as_os_str().is_empty() => {
            let library = plan
                .library
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "library".to_string());
            format!("{library}{}{}", std::path::MAIN_SEPARATOR, inside.display())
        }
        _ => plan.home.display().to_string(),
    }
}

/// The header's last line: what the card leaves out and why, or `None` when it leaves nothing
/// out. The files the group lists already; the files whose names a library file cannot hold,
/// named (audit 4, C6); the library's own folder (C5).
pub fn left_out_line(card: &ImportCard) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let n = card.plan.already();
    if n > 0 {
        parts.push(format!(
            "The group lists {} already, so {} not written again.",
            count(n, "file", "files"),
            if n == 1 { "it is" } else { "they are" }
        ));
    }
    if !card.unlinkable.is_empty() {
        const NAMED: usize = 3;
        let mut names: Vec<String> = card
            .unlinkable
            .iter()
            .take(NAMED)
            .map(|p| import::shown(p))
            .collect();
        if card.unlinkable.len() > NAMED {
            names.push(format!("{} more", card.unlinkable.len() - NAMED));
        }
        parts.push(format!(
            "Left out, as a library file cannot hold {} name: {}.",
            if card.unlinkable.len() == 1 {
                "its"
            } else {
                "their"
            },
            names.join(", ")
        ));
    }
    if card.library_left_out {
        parts.push("The library\u{2019}s own folder is left out.".to_string());
    }
    (!parts.is_empty()).then(|| parts.join("  "))
}

/// A count with its word: "1 file", "2 files".
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// A row's title: a section's name, or a note's.
pub fn row_title(plan: &Plan, row: usize) -> String {
    match plan.rows.get(row).map(|r| &r.kind) {
        Some(import::RowKind::Section { path }) => path
            .last()
            .cloned()
            .unwrap_or_else(|| "the folder".to_string()),
        Some(import::RowKind::File(i)) => plan
            .notes
            .get(*i)
            .map(|n| n.title.clone())
            .unwrap_or_default(),
        None => String::new(),
    }
}

/// How many notes a section row holds, and how many of them are ticked.
pub fn section_count(plan: &Plan, path: &[String]) -> (usize, usize) {
    let under = |note: &import::Note| {
        note.sections.len() >= path.len()
            && note
                .sections
                .iter()
                .zip(path)
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    let all: Vec<&import::Note> = plan.notes.iter().filter(|n| under(n)).collect();
    (all.len(), all.iter().filter(|n| n.ticked).count())
}

/// A row's second line: a file's folders, and its name; or a section's count.
pub fn row_under(plan: &Plan, row: usize) -> String {
    match plan.rows.get(row).map(|r| &r.kind) {
        Some(import::RowKind::Section { path }) => {
            let (all, ticked) = section_count(plan, path);
            format!("{} \u{00B7} {} ticked", count(all, "file", "files"), ticked)
        }
        Some(import::RowKind::File(i)) => {
            let Some(note) = plan.notes.get(*i) else {
                return String::new();
            };
            let mut parts: Vec<String> = Vec::new();
            if !note.sections.is_empty() {
                parts.push(note.sections.join(" \u{203A} "));
            }
            parts.push(import::shown(&note.path));
            if note.already {
                parts.push("already in the library".to_string());
            }
            // A file the AI sorted says how sure it was and why, dim on its row; a file it
            // could not place says so.
            if note.unsorted {
                parts.push("the AI did not place this one".to_string());
            } else if let Some(c) = note.confidence {
                let mut ai = format!("{c:.1}");
                if !note.why.is_empty() {
                    ai.push_str(" \u{00B7} ");
                    ai.push_str(&note.why);
                }
                parts.push(ai);
            }
            parts.join(" \u{00B7} ")
        }
        None => String::new(),
    }
}

/// The line under the preview's title: where the link points, and what happens to the file.
pub fn preview_note(plan: &Plan, note: &import::Note) -> String {
    let link = mdshared_app::library::tree::link_path(&plan.library, &note.path);
    format!(
        "\u{2192} {} \u{00B7} the file is not moved or changed",
        link.display()
    )
}

// ---- state ----------------------------------------------------------------------------------------------

/// Where the AI step is (5.7). `Off` is the plan by the folder's own subfolders.
#[derive(Clone, Debug, PartialEq)]
pub enum SortStage {
    /// No AI step: the folder's subfolders are the sections.
    Off,
    /// The scheme request is out.
    Asking,
    /// The AI's sections, which the owner can rename, delete or add to.
    Scheme(Scheme),
    /// The filing requests are out: the scheme stays on the card, and how many answers came of
    /// how many requests, and how many are in flight right now.
    Filing {
        scheme: Scheme,
        done: usize,
        total: usize,
        in_flight: usize,
    },
    /// The AI's sort is on the plan, wholly or as far as it got.
    Filed {
        requests: usize,
        placed: usize,
        unsorted: usize,
        /// The sentence a request that failed leaves: the plan is still usable, and the notes it
        /// never reached keep their folders.
        stopped: Option<String>,
    },
    /// The scheme could not be had; the plan is as the folders made it.
    Failed(String),
}

/// What the pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IHit {
    Card,
    Tick(usize),
    Row(usize),
    TickAll,
    UntickAll,
    Cancel,
    Add,
    Thumb,
    Track,
    /// The list head's AI offer, or the retry after a failed sort.
    AskAi,
    /// "Settings" beside the reason the AI is off: the settings' AI service page.
    Settings,
    /// "Tick only the sure ones" (5.7).
    SortSure,
    /// The scheme list's own links.
    AddSection,
    RenameSection,
    DeleteSection,
    /// The scheme's primary and the wait's stop.
    SortStart,
    SortStop,
    /// Back to the folder's own sections.
    SortBack,
}

/// What an event asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    Close,
    Write,
    /// "Ask the AI to sort them…": the scheme request.
    SortAsk,
    /// "Sort the files": the filing requests.
    SortStart,
    /// Stop the request that is out, keeping the scheme the owner edited.
    SortStop,
    /// Back to the folders, and cancel anything that is out.
    SortBack,
    /// The settings' AI service page, where a connection is chosen.
    Settings,
}

pub struct ImportCard {
    pub plan: Plan,
    pub sel: usize,
    pub scroll: f32,
    pub hover: Option<IHit>,
    pub pressed: Option<IHit>,
    /// Raised by every change of what the card shows: it is in the overlay's signature.
    pub tick: u64,
    pub win: (f32, f32),
    /// The thumb is held, this far below its top.
    pub drag: Option<f32>,
    /// What the last Add was refused with: drawn in the footer until a tick changes.
    pub refusal: Option<String>,
    /// Files of the folder left out because a library file cannot hold their names, which the
    /// header names (audit 4, C6).
    pub unlinkable: Vec<PathBuf>,
    /// The library folder was in the folder picked, and its files are not offered (C5).
    pub library_left_out: bool,
    /// A card a shot draws: Add is refused, and nothing is written.
    pub demo: bool,
    /// Rectangles recorded as the card is drawn (buttons and links whose size is their text's).
    pub spots: Vec<(IHit, Rect)>,
    /// Where the AI step is.
    pub sort: SortStage,
    /// Why "Ask the AI to sort them…" cannot be pressed, when it cannot: the connection is off,
    /// or an AI run is going. `None` when it can.
    pub ai_reason: Option<String>,
    /// "Claude API · Claude Opus 5.5 · High", for the wait and the card's line.
    pub ai_via: String,
    /// What the AI step would send, for the line before it starts.
    pub ai_estimate: import_ai::Estimate,
    /// The name box of a scheme row being renamed, and the row it belongs to.
    pub edit: Option<TextField>,
    pub edit_row: usize,
}

impl ImportCard {
    pub fn new(plan: Plan) -> ImportCard {
        ImportCard {
            plan,
            sel: 0,
            scroll: 0.0,
            hover: None,
            pressed: None,
            tick: 0,
            win: (0.0, 0.0),
            drag: None,
            refusal: None,
            unlinkable: Vec::new(),
            library_left_out: false,
            demo: false,
            spots: Vec::new(),
            sort: SortStage::Off,
            ai_reason: None,
            ai_via: String::new(),
            ai_estimate: import_ai::Estimate::default(),
            edit: None,
            edit_row: 0,
        }
    }

    pub fn geo(&self) -> Geo {
        geo(card(self.win.0, self.win.1))
    }

    /// The rows the list shows: the scheme's while it is being made or edited, the plan's
    /// otherwise.
    pub fn row_count(&self) -> usize {
        match &self.sort {
            SortStage::Scheme(_) | SortStage::Filing { .. } => {
                self.scheme().map_or(0, |s| s.rows().len())
            }
            _ => self.plan.rows.len(),
        }
    }

    fn row_h(&self) -> f32 {
        match self.sort {
            SortStage::Scheme(_) | SortStage::Filing { .. } => SCHEME_ROW_H,
            _ => ROW_H,
        }
    }

    pub fn rows(&self) -> Rows {
        Rows {
            row_h: self.row_h(),
            view: self.geo().list.h,
            count: self.row_count(),
            scroll: self.scroll,
        }
    }

    /// The scheme, while there is one on the card: being edited, or being filed.
    pub fn scheme(&self) -> Option<&Scheme> {
        match &self.sort {
            SortStage::Scheme(s) => Some(s),
            SortStage::Filing { scheme, .. } => Some(scheme),
            _ => None,
        }
    }

    /// Whether a scheme name is being typed: the caret blinks in the box.
    pub fn editing(&self) -> bool {
        self.edit.is_some()
    }

    /// Why "Add the files" cannot be pressed, or `None` when it can.
    pub fn problem(&self) -> Option<String> {
        self.plan.problem()
    }

    /// Why "Sort the files" cannot be pressed, or `None` when it can.
    pub fn sort_problem(&self) -> Option<String> {
        match &self.sort {
            SortStage::Scheme(s) => s.problem(),
            SortStage::Failed(_) => Some("Ask the AI again, or keep the folders.".to_string()),
            _ => None,
        }
    }

    /// The reason "Ask the AI to sort them…" is offered and can be pressed, if it can.
    pub fn can_ask(&self) -> bool {
        self.ai_reason.is_none()
            && !matches!(self.sort, SortStage::Asking | SortStage::Filing { .. })
    }

    /// Ticks or unticks row `i`: a file's own box, or a whole section.
    pub fn toggle(&mut self, i: usize) {
        if !matches!(
            self.sort,
            SortStage::Off | SortStage::Filed { .. } | SortStage::Failed(_)
        ) {
            return;
        }
        self.plan.toggle_row(i);
        self.refusal = None;
        self.tick += 1;
    }

    pub fn tick_all(&mut self) {
        self.plan.tick_all();
        self.refusal = None;
        self.tick += 1;
    }

    pub fn untick_all(&mut self) {
        self.plan.untick_all();
        self.refusal = None;
        self.tick += 1;
    }

    /// "Tick only the sure ones" (5.7): the AI's confidence of 0.6 and over.
    pub fn tick_sure(&mut self) {
        import_ai::tick_sure(&mut self.plan);
        self.refusal = None;
        self.tick += 1;
    }

    pub fn select(&mut self, i: usize) {
        let i = i.min(self.row_count().saturating_sub(1));
        self.sel = i;
        let rows = self.rows();
        // A card not laid out yet has no viewport: selecting before the first draw must not
        // scroll it, or the list would open part-way down.
        if rows.view > 0.0 {
            self.scroll = rows.reveal(i);
        }
        self.tick += 1;
    }

    // ---- the AI step ----

    /// "Ask the AI to sort them…": the scheme request is out.
    pub fn sort_asking(&mut self) {
        self.edit = None;
        self.sort = SortStage::Asking;
        self.tick += 1;
    }

    /// The scheme came: the card shows it before a single file is filed.
    pub fn sort_scheme(&mut self, scheme: Scheme) {
        self.sort = SortStage::Scheme(scheme);
        self.sel = 0;
        self.scroll = 0.0;
        self.tick += 1;
    }

    /// The filing requests' progress.
    pub fn sort_progress(&mut self, scheme: Scheme, done: usize, total: usize, in_flight: usize) {
        self.sort = SortStage::Filing {
            scheme,
            done,
            total,
            in_flight,
        };
        self.tick += 1;
    }

    /// The answers are on the plan.
    pub fn sort_filed(
        &mut self,
        requests: usize,
        placed: usize,
        unsorted: usize,
        stopped: Option<String>,
    ) {
        self.sort = SortStage::Filed {
            requests,
            placed,
            unsorted,
            stopped,
        };
        self.sel = 0;
        self.scroll = 0.0;
        self.tick += 1;
    }

    /// The scheme could not be had, or a request failed before anything was placed.
    pub fn sort_failed(&mut self, why: String) {
        self.sort = SortStage::Failed(why);
        self.tick += 1;
    }

    /// Back to the folder's own sections: the AI's scheme is dropped and nothing is written.
    pub fn sort_back(&mut self) {
        self.edit = None;
        self.sort = SortStage::Off;
        self.tick += 1;
    }

    /// Starts renaming the selected scheme row.
    pub fn start_rename(&mut self) {
        let Some(scheme) = self.scheme() else {
            return;
        };
        let Some(name) = scheme.row_name(self.sel) else {
            return;
        };
        let mut field = TextField::new();
        field.set_text(name);
        self.edit = Some(field);
        self.edit_row = self.sel;
        self.tick += 1;
    }

    /// Adds a section and puts the name box on it.
    pub fn add_section(&mut self) {
        let Some(scheme) = self.scheme().cloned() else {
            return;
        };
        let mut scheme = scheme;
        let row = scheme.add("New section");
        self.sort = SortStage::Scheme(scheme);
        self.sel = row;
        self.start_rename();
        let rows = self.rows();
        if rows.view > 0.0 {
            self.scroll = rows.reveal(row);
        }
        self.tick += 1;
    }

    /// Deletes the selected scheme row. A top-level row takes its subsections with it, and the
    /// last row is not deleted.
    pub fn delete_section(&mut self) {
        let Some(scheme) = self.scheme().cloned() else {
            return;
        };
        let mut scheme = scheme;
        let before = scheme.rows().len();
        scheme.delete(self.sel);
        self.sort = SortStage::Scheme(scheme);
        self.sel = self.sel.min(self.row_count().saturating_sub(1));
        if before == self.row_count() {
            self.refusal = Some("A scheme needs at least one section.".to_string());
        }
        self.tick += 1;
    }

    /// Enter in the name box: the row takes the name.
    pub fn commit_edit(&mut self) {
        let Some(field) = self.edit.take() else {
            return;
        };
        let name = field.text().to_string();
        if let SortStage::Scheme(scheme) = &mut self.sort {
            scheme.rename(self.edit_row, &name);
        }
        self.tick += 1;
    }

    /// Escape in the name box: the row keeps the name it had.
    pub fn cancel_edit(&mut self) {
        self.edit = None;
        self.tick += 1;
    }

    // ---- events ----

    pub fn resize(&mut self, w: f32, h: f32) {
        if self.win != (w, h) {
            self.win = (w, h);
            let rows = self.rows();
            self.scroll = rows.clamp_scroll(self.scroll);
            // A card laid out after its rows were chosen (a shot's, or one opened on a row
            // below the fold) keeps the selected row in view.
            self.scroll = rows.reveal(self.sel);
            self.tick += 1;
        }
    }

    pub fn hit_at(&self, x: f32, y: f32) -> Option<IHit> {
        let g = self.geo();
        if !g.card.contains(x, y) {
            return None;
        }
        if let Some((hit, _)) = self.spots.iter().rev().find(|(_, r)| r.contains(x, y)) {
            return Some(*hit);
        }
        if g.list.contains(x, y) {
            let rows = self.rows();
            return Some(match rows.row_at(g.list, x, y) {
                Some(i) => {
                    let scheme =
                        matches!(self.sort, SortStage::Scheme(_) | SortStage::Filing { .. });
                    let row = rows.rect(g.list, i);
                    if !scheme && x < row.x + 40.0 {
                        IHit::Tick(i)
                    } else {
                        IHit::Row(i)
                    }
                }
                None => IHit::Card,
            });
        }
        Some(IHit::Card)
    }

    pub fn pointer(&mut self, x: f32, y: f32) {
        if let Some(grab) = self.drag {
            let g = self.geo();
            let rows = self.rows();
            if let Some((track, thumb)) = rows.thumb(g.list) {
                let off = offset_for_thumb(track, thumb.h, y - grab, rows.content(), rows.view);
                self.scroll = rows.clamp_scroll(off);
                self.tick += 1;
            }
            return;
        }
        let hit = self.hit_at(x, y).filter(|h| *h != IHit::Card);
        if hit != self.hover {
            self.hover = hit;
            self.tick += 1;
        }
    }

    pub fn press(&mut self, x: f32, y: f32) {
        self.tick += 1;
        let hit = self.hit_at(x, y);
        self.pressed = hit;
        let g = self.geo();
        let rows = self.rows();
        match hit {
            Some(IHit::Thumb) => {
                if let Some((_, thumb)) = rows.thumb(g.list) {
                    self.drag = Some(y - thumb.y);
                }
            }
            Some(IHit::Track) => {
                if let Some((_, thumb)) = rows.thumb(g.list) {
                    let page = if y < thumb.y { -rows.view } else { rows.view };
                    self.scroll = rows.clamp_scroll(rows.scroll + page);
                }
            }
            _ => {}
        }
    }

    pub fn release(&mut self, x: f32, y: f32) -> Act {
        self.tick += 1;
        self.drag = None;
        let Some(pressed) = self.pressed.take() else {
            return Act::Nothing;
        };
        if self.hit_at(x, y) != Some(pressed) {
            return Act::Nothing;
        }
        match pressed {
            // A click anywhere on a row ticks it: there is nothing else on a row to choose. On
            // the scheme's list it only selects.
            IHit::Tick(i) | IHit::Row(i) => {
                self.select(i);
                self.toggle(i);
            }
            IHit::TickAll => self.tick_all(),
            IHit::UntickAll => self.untick_all(),
            IHit::SortSure => self.tick_sure(),
            IHit::AddSection => self.add_section(),
            IHit::RenameSection => self.start_rename(),
            IHit::DeleteSection => self.delete_section(),
            IHit::AskAi => {
                if self.can_ask() {
                    return Act::SortAsk;
                }
            }
            IHit::SortStart => return self.sort_pressed(),
            IHit::SortStop => return Act::SortStop,
            IHit::SortBack => return Act::SortBack,
            IHit::Settings => return Act::Settings,
            IHit::Cancel => return Act::Close,
            IHit::Add => return self.add_pressed(),
            _ => {}
        }
        Act::Nothing
    }

    fn add_pressed(&mut self) -> Act {
        if self.problem().is_some() {
            return Act::Nothing;
        }
        self.tick += 1;
        Act::Write
    }

    fn sort_pressed(&mut self) -> Act {
        if self.sort_problem().is_some() || self.edit.is_some() {
            return Act::Nothing;
        }
        self.tick += 1;
        Act::SortStart
    }

    pub fn wheel(&mut self, dy: f32) {
        self.tick += 1;
        let rows = self.rows();
        self.scroll = rows.clamp_scroll(self.scroll + dy);
    }

    pub fn key(&mut self, key: &Key, mods: Mods, typed: Option<&str>) -> Act {
        self.tick += 1;
        // Insert adds a section on the scheme's list; everywhere else it means nothing here.
        if matches!(key, Key::Insert) {
            if matches!(self.sort, SortStage::Scheme(_)) && self.edit.is_none() {
                self.add_section();
            }
            return Act::Nothing;
        }
        let Key::Name(name) = key else {
            return Act::Nothing;
        };
        // A scheme name being typed takes every key but Enter and Escape.
        if self.edit.is_some() {
            match name {
                KeyName::Enter => self.commit_edit(),
                KeyName::Escape => self.cancel_edit(),
                n => {
                    if let Some(field) = self.edit.as_mut() {
                        field.handle_key(n, mods, typed, true);
                    }
                }
            }
            return Act::Nothing;
        }
        if matches!(self.sort, SortStage::Scheme(_)) {
            return match name {
                KeyName::Escape => Act::SortBack,
                KeyName::Enter => self.sort_pressed(),
                KeyName::F(2) => {
                    self.start_rename();
                    Act::Nothing
                }
                KeyName::Delete => {
                    self.delete_section();
                    Act::Nothing
                }
                KeyName::Up => self.key_nav(Nav::Up),
                KeyName::Down => self.key_nav(Nav::Down),
                KeyName::Home => self.key_nav(Nav::Home),
                KeyName::End => self.key_nav(Nav::End),
                KeyName::PageUp => self.key_nav(Nav::PageUp),
                KeyName::PageDown => self.key_nav(Nav::PageDown),
                _ => Act::Nothing,
            };
        }
        if matches!(self.sort, SortStage::Asking | SortStage::Filing { .. }) {
            return match name {
                KeyName::Escape => Act::SortStop,
                _ => Act::Nothing,
            };
        }
        if matches!(self.sort, SortStage::Failed(_)) {
            return match name {
                KeyName::Escape => Act::SortBack,
                KeyName::Enter if self.can_ask() => Act::SortAsk,
                _ => Act::Nothing,
            };
        }
        let nav = match name {
            KeyName::Escape => return Act::Close,
            KeyName::Enter => return self.add_pressed(),
            KeyName::Space => {
                self.toggle(self.sel);
                return Act::Nothing;
            }
            KeyName::Up => Nav::Up,
            KeyName::Down => Nav::Down,
            KeyName::Home => Nav::Home,
            KeyName::End => Nav::End,
            KeyName::PageUp => Nav::PageUp,
            KeyName::PageDown => Nav::PageDown,
            _ => return Act::Nothing,
        };
        if let Some(to) = self.rows().nav(Some(self.sel), nav) {
            self.select(to);
        }
        Act::Nothing
    }

    fn key_nav(&mut self, nav: Nav) -> Act {
        if let Some(to) = self.rows().nav(Some(self.sel), nav) {
            self.select(to);
        }
        Act::Nothing
    }

    /// What the frame of the card depends on besides its tick; the caret's blink matters while a
    /// scheme name is being typed.
    pub fn sig(&self, caret_on: bool) -> (u64, Option<IHit>, Option<IHit>, bool) {
        (
            self.tick,
            self.hover,
            self.pressed,
            self.editing() && caret_on,
        )
    }
}

// ---- the shots ------------------------------------------------------------------------------------------

/// Every name `--plan-demo folder…` takes.
pub const NAMES: [&str; 17] = [
    "folder",
    "folder-sections",
    "folder-taken",
    "folder-small",
    "folder-over",
    "folder-skipped",
    "folder-elsewhere",
    "folder-stopped",
    "folder-refused",
    "folder-scheme",
    "folder-scheme-rename",
    "folder-ai",
    "folder-ai-sorting",
    "folder-ai-asking",
    "folder-ai-off",
    "folder-ai-failed",
    "folder-unsorted",
];

/// The folder the shots show: made up, never read or written, and the same on every run.
pub fn demo_folder() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"D:\Notes\Recipes")
    } else {
        PathBuf::from("/home/me/Notes/Recipes")
    }
}

/// The plan the shots draw: a folder of recipes with subfolders two deep, one file in a third
/// level, two the library already lists, and a plain text file with no heading.
pub fn demo_plan(name: &str) -> Plan {
    let folder = demo_folder();
    let library = if cfg!(windows) {
        PathBuf::from(r"D:\Notes\library")
    } else {
        PathBuf::from("/home/me/Notes/library")
    };
    let note =
        |rel: &str, sections: &[&str], title: &str, head: &[&str], already: bool| import::Note {
            path: folder.join(rel),
            sections: sections.iter().map(|s| s.to_string()).collect(),
            title: title.to_string(),
            head: head.iter().map(|s| s.to_string()).collect(),
            start: head.join("\n"),
            bytes: head.join("\n").len() as u64,
            modified: "2026-11-23".to_string(),
            ticked: !already,
            already,
            confidence: None,
            why: String::new(),
            unsorted: false,
        };
    let notes = vec![
        note(
            "weeknight-pasta.md",
            &[],
            "Weeknight pasta",
            &[
                "# Weeknight pasta",
                "",
                "Forty minutes, one pot, and the sauce",
            ],
            false,
        ),
        note(
            "Bread/sourdough.md",
            &["Bread"],
            "Sourdough",
            &[
                "# Sourdough",
                "",
                "A starter kept on the counter, fed twice a",
            ],
            false,
        ),
        note(
            "Bread/focaccia.md",
            &["Bread"],
            "Focaccia",
            &["# Focaccia", "", "The dough is wet and the tin is oiled"],
            false,
        ),
        note(
            "Pasta/carbonara.md",
            &["Pasta"],
            "Carbonara",
            &[
                "# Carbonara",
                "",
                "Eggs, pecorino, guanciale. No cream, ever.",
            ],
            true,
        ),
        note(
            "Pasta/ragu.md",
            &["Pasta"],
            "Ragù",
            &["# Ragù", "", "Four hours at the lowest heat the oven has"],
            false,
        ),
        note(
            "Pasta/Italian/gnocchi.md",
            &["Pasta", "Italian"],
            "Gnocchi",
            &[
                "# Gnocchi",
                "",
                "Potato, flour, and as little handling as possible",
            ],
            false,
        ),
        note(
            "Pasta/Italian/risotto.md",
            &["Pasta", "Italian"],
            "Risotto",
            &[
                "# Risotto",
                "",
                "Stirred, one ladle at a time, for eighteen minutes",
            ],
            false,
        ),
        note(
            "Sauces/pesto.md",
            &["Sauces"],
            "Pesto",
            &[
                "# Pesto",
                "",
                "Basil, pine nuts, garlic, and a lot of olive oil",
            ],
            false,
        ),
        note(
            "Sauces/tomato sauce.md",
            &["Sauces"],
            "Tomato sauce",
            &["# Tomato sauce", "", "Tinned tomatoes, an onion, and time"],
            false,
        ),
        note(
            "Sauces/French/beurre blanc.md",
            &["Sauces", "French"],
            "Beurre blanc",
            &[
                "# Beurre blanc",
                "",
                "Shallots and wine reduced to almost nothing",
            ],
            false,
        ),
        note(
            "notes.txt",
            &[],
            "notes",
            &["Things to try: a clay pot for the bread, the fish"],
            false,
        ),
    ];
    let mut plan = import::Plan {
        folder: folder.clone(),
        group_name: "Recipes".to_string(),
        library: library.clone(),
        group_file: library.join("Recipes.md"),
        group_new: true,
        home: folder.clone(),
        rows: Vec::new(),
        notes: Vec::new(),
        entries: 0,
        folders: 5,
        more: false,
        blocked: None,
    };
    if name == "folder-over" {
        // Over the library's 2,000: refused with the count.
        plan.more = true;
    }
    if name == "folder-taken" {
        // The library already has the group, and lists three of the files.
        plan.group_new = false;
        plan.entries = 3;
        plan.group_name = "Recipes".to_string();
    }
    plan.rows = rows_for(&notes);
    plan.notes = notes;
    plan.blocked = demo_blocked(&plan);
    if name == "folder-elsewhere" {
        // The library's "Recipes" keeps its notes in another folder: the card says that folder
        // and refuses (audit 4, C4).
        plan.group_new = false;
        plan.entries = 12;
        plan.home = if cfg!(windows) {
            PathBuf::from(r"D:\Archive\Recipes")
        } else {
            PathBuf::from("/home/me/Archive/Recipes")
        };
        plan.blocked = Some(import::home_apart_reason("Recipes", &plan.home));
    }
    if name == "folder-stopped" {
        plan.blocked = Some(import::stopped_reason(import::Stop::Entries));
    }
    plan
}

/// The rows of a list of notes: a heading where a section starts, then its notes, nested.
fn rows_for(notes: &[import::Note]) -> Vec<import::Row> {
    let mut rows: Vec<import::Row> = Vec::new();
    let mut open: Vec<String> = Vec::new();
    for (i, note) in notes.iter().enumerate() {
        let shared = open
            .iter()
            .zip(&note.sections)
            .take_while(|(a, b)| a == b)
            .count();
        for n in shared..note.sections.len() {
            let path = note.sections[..=n].to_vec();
            let ticked = notes
                .iter()
                .filter(|x| x.sections.len() > n && x.sections[..=n] == path[..])
                .all(|x| x.ticked);
            rows.push(import::Row {
                kind: import::RowKind::Section { path },
                ticked,
            });
        }
        open = note.sections.clone();
        rows.push(import::Row {
            kind: import::RowKind::File(i),
            ticked: note.ticked,
        });
    }
    rows
}

/// What the demo's card says when nothing can be written: nothing, or the 2,000's reason.
fn demo_blocked(plan: &Plan) -> Option<String> {
    if plan.more {
        return Some(format!(
            "The folder holds more than {} files, which is the most the library keeps for a group. Add them in parts.",
            mdshared_app::library::MAX_ENTRIES
        ));
    }
    None
}

/// `--plan-demo folder…`: the card over the demonstration plan.
pub fn demo(name: &str) -> Option<ImportCard> {
    if !NAMES.contains(&name) {
        return None;
    }
    let plan = demo_plan(name);
    let mut card = ImportCard::new(plan);
    card.demo = true;
    card.ai_via = "Claude API \u{00B7} Claude Opus 5.5 \u{00B7} effort high".to_string();
    card.ai_estimate = import_ai::estimate(&card.plan);
    match name {
        "folder-sections" => {
            // Two file rows unticked, and the sections above them say so.
            for i in 0..card.plan.rows.len() {
                if card.plan.note_of(i).is_some_and(|n| n == 2 || n == 6) {
                    card.toggle(i);
                }
            }
            card.select(6);
        }
        "folder-taken" => {
            // The library already lists Carbonara: its row is not written again.
            card.select(5);
        }
        "folder-over" => card.select(0),
        "folder-scheme" => {
            card.sort_scheme(demo_scheme());
            card.select(2);
        }
        "folder-scheme-rename" => {
            card.sort_scheme(demo_scheme());
            card.select(0);
            card.start_rename();
            if let Some(field) = card.edit.as_mut() {
                field.set_text("Noodles");
            }
        }
        "folder-ai-sorting" => {
            card.sort_scheme(demo_scheme());
            card.sort_progress(demo_scheme(), 4, 14, 3);
            card.select(3);
        }
        "folder-ai" | "folder-unsorted" => {
            let scheme = demo_scheme();
            let plan = card.plan.clone();
            let batches = import_ai::batches(&plan, &scheme);
            let answer = demo_answer(&plan, &batches, name == "folder-unsorted");
            let answers: Vec<Option<import_ai::Answer>> = vec![Some(answer); batches.len()];
            let sorted = import_ai::settle(&mut card.plan, &scheme, &batches, &answers, true);
            card.sort_filed(1 + batches.len(), sorted.placed, sorted.unsorted, None);
            let last = card.row_count().saturating_sub(1);
            card.select(if name == "folder-unsorted" { last } else { 2 });
        }
        "folder-ai-asking" => card.sort_asking(),
        "folder-ai-off" => {
            card.ai_reason =
                Some("The AI is off: choose a connection in Settings \u{203A} AI.".to_string());
        }
        "folder-ai-failed" => card.sort_failed(
            "The AI\u{2019}s answer could not be read: it held no JSON object.".to_string(),
        ),
        "folder-skipped" => {
            // Two files whose names a library file cannot hold, and the library's own folder,
            // left out and said in the header; the rest can be added.
            card.unlinkable = vec![
                demo_folder().join("my%20draft.md"),
                demo_folder().join("Bread").join("rye%20loaf.md"),
            ];
            card.library_left_out = true;
            card.select(2);
        }
        "folder-refused" => {
            // An Add refused at the write, its reason in the footer and the button still there.
            card.refusal = Some(crate::commands::strings::library_unsaved("Recipes.md"));
            card.select(2);
        }
        _ => card.select(2),
    }
    Some(card)
}

/// The scheme the AI shots show: five sections, three with a subsection under them.
pub fn demo_scheme() -> Scheme {
    let section = |name: &str, subs: &[&str]| import_ai::Section {
        name: name.to_string(),
        subs: subs.iter().map(|s| s.to_string()).collect(),
    };
    Scheme {
        sections: vec![
            section("Pasta", &["Italian", "Baked"]),
            section("Bread", &["Sweet"]),
            section("Sauces", &[]),
            section("Fish", &[]),
            section("Desserts", &[]),
        ],
        dropped: 0,
    }
}

/// The answer the AI shots show, read and checked by the same settler a real one is: one file
/// the AI did not place, and with `unsorted`, a section it does not have and a second file left
/// out.
pub fn demo_answer(plan: &Plan, batches: &[import_ai::Batch], unsorted: bool) -> import_ai::Answer {
    let Some(batch) = batches.first() else {
        return import_ai::Answer::default();
    };
    let mut files = Vec::new();
    for (id, &note_i) in batch.ids.iter().enumerate() {
        let Some(note) = plan.notes.get(note_i) else {
            continue;
        };
        let (mut section, confidence, why) = match note.title.as_str() {
            "Weeknight pasta" => (Some(0), 0.92, "a pasta dish, one pot"),
            "Sourdough" => (Some(3), 0.96, "the bread the folder is built around"),
            "Focaccia" => (Some(3), 0.78, "baked, and eaten with the bread"),
            "Rag\u{00F9}" => (Some(0), 0.88, "a sauce, but the pasta it goes on"),
            "Gnocchi" => (Some(1), 0.81, "the Italian shelf"),
            "Risotto" => (Some(1), 0.74, "the Italian shelf"),
            "Pesto" => (Some(5), 0.95, "a sauce kept in a jar"),
            "Tomato sauce" => (Some(5), 0.55, "a sauce, less sure which"),
            "Beurre blanc" => (Some(5), 0.83, "a sauce finished with butter"),
            _ => (None, 0.0, "no clear section"),
        };
        if unsorted {
            if note.title == "Tomato sauce" {
                section = Some(9);
            }
            if note.title == "Beurre blanc" {
                continue;
            }
        }
        let Some(section) = section else {
            continue;
        };
        files.push(import_ai::Filed {
            id,
            section: Some(section),
            title: None,
            confidence,
            why: why.to_string(),
        });
    }
    import_ai::Answer { files }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_shows_rows_under_their_sections_with_a_third_level() {
        let card = demo("folder").expect("the demo");
        assert!(
            card.plan.rows.len() > card.plan.notes.len(),
            "sections and files"
        );
        let sections: Vec<String> = card
            .plan
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                import::RowKind::Section { path } => Some(path.join("/")),
                import::RowKind::File(_) => None,
            })
            .collect();
        assert_eq!(
            sections,
            ["Bread", "Pasta", "Pasta/Italian", "Sauces", "Sauces/French"]
        );
        // The library already lists Carbonara, so its row is not ticked.
        let carbonara = card
            .plan
            .notes
            .iter()
            .position(|n| n.title == "Carbonara")
            .expect("Carbonara");
        assert!(card.plan.notes[carbonara].already);
        assert!(!card.plan.notes[carbonara].ticked);
    }

    #[test]
    fn a_section_row_ticks_and_unticks_everything_under_it() {
        let mut card = demo("folder").expect("the demo");
        card.resize(1280.0, 800.0);
        let at = card
            .plan
            .rows
            .iter()
            .position(|r| matches!(&r.kind, import::RowKind::Section { path } if path.join("/") == "Pasta"))
            .expect("Pasta");
        let before = card.plan.ticked();
        card.select(at);
        card.key(&Key::Name(KeyName::Space), Mods::default(), None);
        // Pasta holds ragu, and Italian holds gnocchi and risotto: three.
        assert_eq!(card.plan.ticked(), before - 3);
        card.key(&Key::Name(KeyName::Space), Mods::default(), None);
        assert_eq!(card.plan.ticked(), before);
    }

    /// The reason the AI is off sits beside a link to the settings, and the link asks for them.
    #[test]
    fn the_off_cards_settings_link_asks_for_the_settings() {
        let mut card = demo("folder-ai-off").expect("the demo");
        card.resize(1280.0, 800.0);
        let g = card.geo();
        let r = Rect {
            x: g.card.x + 10.0,
            y: g.card.y + 10.0,
            w: 60.0,
            h: 20.0,
        };
        card.spots = vec![(IHit::Settings, r)];
        let (x, y) = (r.x + 2.0, r.y + 2.0);
        card.pressed = card.hit_at(x, y);
        assert_eq!(card.release(x, y), Act::Settings);
    }

    #[test]
    fn a_click_on_a_section_rows_box_ticks_or_unticks_its_notes() {
        let mut card = demo("folder").expect("the demo");
        card.resize(1280.0, 800.0);
        let at = card
            .plan
            .rows
            .iter()
            .position(|r| matches!(&r.kind, import::RowKind::Section { path } if path.len() == 1))
            .expect("a top section");
        let note = card.plan.note_of(at + 1).expect("a note under it");
        assert!(card.plan.notes[note].ticked);
        let g = card.geo();
        let r = card.rows().rect(g.list, at);
        let (x, y) = (r.x + 20.0, r.cy());
        assert_eq!(card.hit_at(x, y), Some(IHit::Tick(at)));
        card.press(x, y);
        assert_eq!(card.release(x, y), Act::Nothing);
        assert!(!card.plan.notes[note].ticked, "{:?}", card.plan.rows);
    }

    #[test]
    fn add_is_refused_with_nothing_ticked_and_a_demo_writes_nothing() {
        let mut card = demo("folder").expect("the demo");
        card.resize(1280.0, 800.0);
        card.untick_all();
        assert_eq!(card.problem().as_deref(), Some("Nothing is ticked."));
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default(), None),
            Act::Nothing,
            "Enter does not add with nothing ticked"
        );
        card.tick_all();
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default(), None),
            Act::Write
        );
        assert_eq!(
            card.key(&Key::Name(KeyName::Escape), Mods::default(), None),
            Act::Close
        );
    }

    #[test]
    fn the_folder_over_the_most_says_so_in_the_card() {
        let card = demo("folder-over").expect("the demo");
        let why = card.problem().unwrap_or_default();
        assert!(why.contains("2000") && why.contains("parts"), "{why}");
    }

    #[test]
    fn a_narrow_card_drops_the_preview_and_keeps_the_list() {
        let mut card = demo("folder").expect("the demo");
        card.resize(1280.0, 800.0);
        assert!(card.geo().preview.is_some());
        card.resize(760.0, 560.0);
        let g = card.geo();
        assert!(g.preview.is_none());
        assert!((g.list.w - g.card.w).abs() < 0.01);
        // Every row is still reachable by the keys.
        let last = card.plan.rows.len() - 1;
        card.select(last);
        assert_eq!(card.sel, last);
    }

    // ---- the AI step (5.7) ----

    #[test]
    fn the_scheme_stage_sorts_on_enter_and_goes_back_on_escape_without_ticking() {
        let mut card = demo("folder-scheme").expect("the demo");
        card.resize(1280.0, 800.0);
        assert!(matches!(card.sort, SortStage::Scheme(_)));
        assert_eq!(card.row_count(), 8, "five sections and three subsections");
        assert_eq!(card.row_h(), SCHEME_ROW_H);
        let before = card.plan.ticked();
        // Space toggles nothing on the scheme's list.
        card.key(&Key::Name(KeyName::Space), Mods::default(), None);
        assert_eq!(card.plan.ticked(), before);
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default(), None),
            Act::SortStart
        );
        assert_eq!(
            card.key(&Key::Name(KeyName::Escape), Mods::default(), None),
            Act::SortBack
        );
    }

    #[test]
    fn a_scheme_row_is_renamed_added_and_deleted_in_the_card() {
        let mut card = demo("folder-scheme").expect("the demo");
        card.resize(1280.0, 800.0);
        card.select(0);
        card.key(&Key::Name(KeyName::F(2)), Mods::default(), None);
        assert!(card.editing());
        card.edit.as_mut().expect("the box").set_text("Noodles");
        card.key(&Key::Name(KeyName::Enter), Mods::default(), None);
        assert!(!card.editing());
        assert_eq!(card.scheme().expect("the scheme").rows()[0].name, "Noodles");
        // A subsection moves with its section's name.
        assert_eq!(
            card.scheme().expect("the scheme").rows()[1].path,
            ["Noodles".to_string(), "Italian".to_string()]
        );
        // Insert adds a section with its name box on it; Escape leaves the name as it was.
        let before = card.row_count();
        card.key(&Key::Insert, Mods::default(), None);
        assert_eq!(card.row_count(), before + 1);
        assert!(card.editing());
        assert_eq!(card.sel, before);
        card.key(&Key::Name(KeyName::Escape), Mods::default(), None);
        assert!(!card.editing());
        assert_eq!(
            card.scheme().expect("the scheme").rows()[before].name,
            "New section"
        );
        // Delete takes it away again; Delete on the last row leaves it.
        card.key(&Key::Name(KeyName::Delete), Mods::default(), None);
        assert_eq!(card.row_count(), before);
        while card.row_count() > 1 {
            card.select(card.row_count() - 1);
            card.key(&Key::Name(KeyName::Delete), Mods::default(), None);
        }
        card.key(&Key::Name(KeyName::Delete), Mods::default(), None);
        assert_eq!(card.row_count(), 1, "a scheme keeps one section");
    }

    #[test]
    fn a_repeated_scheme_name_is_said_before_the_files_are_sorted() {
        let mut card = demo("folder-scheme").expect("the demo");
        card.resize(1280.0, 800.0);
        card.select(0);
        card.start_rename();
        card.edit.as_mut().expect("the box").set_text("Bread");
        card.commit_edit();
        let why = card.sort_problem().unwrap_or_default();
        assert!(why.contains("Two sections"), "{why}");
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default(), None),
            Act::Nothing,
            "Enter does not sort a scheme it cannot send"
        );
    }

    #[test]
    fn the_caret_blinks_in_the_signature_only_while_a_name_is_typed() {
        let mut card = demo("folder-scheme").expect("the demo");
        assert_eq!(card.sig(false), card.sig(true), "no box, no caret");
        card.start_rename();
        assert_ne!(card.sig(false), card.sig(true), "the box's caret blinks");
    }

    #[test]
    fn the_ai_sorted_plan_marks_unsorted_and_tick_sure_uses_six_tenths() {
        let card = demo("folder-unsorted").expect("the demo");
        let SortStage::Filed {
            placed, unsorted, ..
        } = card.sort
        else {
            panic!("a sorted plan")
        };
        assert_eq!(placed + unsorted, 10, "every note the AI was asked about");
        assert!(card.plan.notes.iter().any(|n| n.unsorted));
        let mut card = demo("folder-ai").expect("the demo");
        card.tick_sure();
        for note in &card.plan.notes {
            if note.already || note.unsorted {
                assert!(!note.ticked, "{} is not written unasked", note.title);
            } else if note.confidence.is_some_and(|c| c >= 0.6) {
                assert!(note.ticked, "{} is sure", note.title);
            } else {
                assert!(!note.ticked, "{} is not sure", note.title);
            }
        }
        assert_eq!(card.plan.ticked(), 8, "the 0.55 one stays out");
    }

    #[test]
    fn the_ai_off_card_says_why_and_the_failed_card_offers_its_two_ways_out() {
        let card = demo("folder-ai-off").expect("the demo");
        assert!(!card.can_ask());
        assert!(card.ai_reason.as_deref().unwrap_or("").contains("off"));
        let mut card = demo("folder-ai-failed").expect("the demo");
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default(), None),
            Act::SortAsk
        );
        assert_eq!(
            card.key(&Key::Name(KeyName::Escape), Mods::default(), None),
            Act::SortBack
        );
        let mut card = demo("folder-ai-asking").expect("the demo");
        assert_eq!(
            card.key(&Key::Name(KeyName::Escape), Mods::default(), None),
            Act::SortStop
        );
        assert!(!card.can_ask(), "the offer waits while a request is out");
    }
}
