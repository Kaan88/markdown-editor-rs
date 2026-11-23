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
use mdshared_ui::gfx::Rect;
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::overlay::{DIALOG_FOOTER, dialog_card};
use mdshared_ui::rows::{Nav, Rows};
use mdshared_ui::scroll::offset_for_thumb;
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
        (
            Rect::new(card.x, list_head.bottom(), card.w, body),
            None,
        )
    };
    Geo {
        card,
        header,
        list_head,
        list,
        preview,
    }
}

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

/// The group the links go in, and where its notes are.
pub fn group_line(plan: &Plan) -> String {
    let folder = if plan.group_new {
        format!("which this run makes, with its folder {}", folder_shown(plan))
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
            format!(
                "{} \u{00B7} {} ticked",
                count(all, "file", "files"),
                ticked
            )
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
}

/// What an event asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    Close,
    Write,
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
    /// A card a shot draws: Add is refused, and nothing is written.
    pub demo: bool,
    /// Rectangles recorded as the card is drawn (buttons and links whose size is their text's).
    pub spots: Vec<(IHit, Rect)>,
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
            demo: false,
            spots: Vec::new(),
        }
    }

    pub fn geo(&self) -> Geo {
        geo(card(self.win.0, self.win.1))
    }

    pub fn rows(&self) -> Rows {
        Rows {
            row_h: ROW_H,
            view: self.geo().list.h,
            count: self.plan.rows.len(),
            scroll: self.scroll,
        }
    }

    /// Why "Add the files" cannot be pressed, or `None` when it can.
    pub fn problem(&self) -> Option<String> {
        self.plan.problem()
    }

    /// Ticks or unticks row `i`: a file's own box, or a whole section.
    pub fn toggle(&mut self, i: usize) {
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

    pub fn select(&mut self, i: usize) {
        let i = i.min(self.plan.rows.len().saturating_sub(1));
        self.sel = i;
        let rows = self.rows();
        // A card not laid out yet has no viewport: selecting before the first draw must not
        // scroll it, or the list would open part-way down.
        if rows.view > 0.0 {
            self.scroll = rows.reveal(i);
        }
        self.tick += 1;
    }

    // ---- events ----

    pub fn resize(&mut self, w: f32, h: f32) {
        if self.win != (w, h) {
            self.win = (w, h);
            self.scroll = self.rows().clamp_scroll(self.scroll);
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
                    let row = rows.rect(g.list, i);
                    if x < row.x + 40.0 {
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
            // A click anywhere on a row ticks it: there is nothing else on a row to choose.
            IHit::Tick(i) | IHit::Row(i) => {
                self.select(i);
                self.toggle(i);
            }
            IHit::TickAll => self.tick_all(),
            IHit::UntickAll => self.untick_all(),
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

    pub fn wheel(&mut self, dy: f32) {
        self.tick += 1;
        let rows = self.rows();
        self.scroll = rows.clamp_scroll(self.scroll + dy);
    }

    pub fn key(&mut self, key: &Key, _mods: Mods) -> Act {
        self.tick += 1;
        let Key::Name(name) = key else {
            return Act::Nothing;
        };
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

    /// What the frame of the card depends on besides its tick.
    pub fn sig(&self) -> (u64, Option<IHit>, Option<IHit>) {
        (self.tick, self.hover, self.pressed)
    }
}

// ---- the shots ------------------------------------------------------------------------------------------

/// Every name `--plan-demo folder…` takes.
pub const NAMES: [&str; 5] = [
    "folder",
    "folder-sections",
    "folder-taken",
    "folder-small",
    "folder-over",
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
    let note = |rel: &str, sections: &[&str], title: &str, head: &[&str], already: bool| {
        import::Note {
            path: folder.join(rel),
            sections: sections.iter().map(|s| s.to_string()).collect(),
            title: title.to_string(),
            head: head.iter().map(|s| s.to_string()).collect(),
            ticked: !already,
            already,
        }
    };
    let notes = vec![
        note(
            "weeknight-pasta.md",
            &[],
            "Weeknight pasta",
            &["# Weeknight pasta", "", "Forty minutes, one pot, and the sauce"],
            false,
        ),
        note(
            "Bread/sourdough.md",
            &["Bread"],
            "Sourdough",
            &["# Sourdough", "", "A starter kept on the counter, fed twice a"],
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
            &["# Carbonara", "", "Eggs, pecorino, guanciale. No cream, ever."],
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
            &["# Gnocchi", "", "Potato, flour, and as little handling as possible"],
            false,
        ),
        note(
            "Pasta/Italian/risotto.md",
            &["Pasta", "Italian"],
            "Risotto",
            &["# Risotto", "", "Stirred, one ladle at a time, for eighteen minutes"],
            false,
        ),
        note(
            "Sauces/pesto.md",
            &["Sauces"],
            "Pesto",
            &["# Pesto", "", "Basil, pine nuts, garlic, and a lot of olive oil"],
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
            &["# Beurre blanc", "", "Shallots and wine reduced to almost nothing"],
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
    match name {
        "folder-sections" | "folder-taken" => {
            // Two file rows unticked, and the sections above them say so.
            for i in 0..card.plan.rows.len() {
                if card.plan.note_of(i).is_some_and(|n| n == 2 || n == 6) {
                    card.toggle(i);
                }
            }
            card.select(4);
        }
        "folder-over" => card.select(0),
        _ => card.select(1),
    }
    Some(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_shows_rows_under_their_sections_with_a_third_level() {
        let card = demo("folder").expect("the demo");
        assert!(card.plan.rows.len() > card.plan.notes.len(), "sections and files");
        let sections: Vec<String> = card
            .plan
            .rows
            .iter()
            .filter_map(|r| match &r.kind {
                import::RowKind::Section { path } => Some(path.join("/")),
                import::RowKind::File(_) => None,
            })
            .collect();
        assert_eq!(sections, ["Bread", "Pasta", "Pasta/Italian", "Sauces", "Sauces/French"]);
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
        card.key(&Key::Name(KeyName::Space), Mods::default());
        // Pasta holds ragu, and Italian holds gnocchi and risotto: three.
        assert_eq!(card.plan.ticked(), before - 3);
        card.key(&Key::Name(KeyName::Space), Mods::default());
        assert_eq!(card.plan.ticked(), before);
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
            card.key(&Key::Name(KeyName::Enter), Mods::default()),
            Act::Nothing,
            "Enter does not add with nothing ticked"
        );
        card.tick_all();
        assert_eq!(card.key(&Key::Name(KeyName::Enter), Mods::default()), Act::Write);
        assert_eq!(
            card.key(&Key::Name(KeyName::Escape), Mods::default()),
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
}
