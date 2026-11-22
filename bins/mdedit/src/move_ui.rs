//! "Move the notes…" (`docs/ai-library-plan.md` 5.12): the card that shows what a move would do,
//! one row per file, and lets the rows be ticked. The plan is `mdedit_core::moves`; this is its
//! state, its events and the numbers of its layout (`move_draw.rs` draws it, `app/moving.rs` is
//! the window's part). Nothing here reads or writes the disk.
//!
//! It is the split's plan card made small: the same backdrop, card, tick boxes, list arithmetic
//! and footer buttons, with no preview, no strip and no editing.

use crate::keys::Key;
use mdedit_core::moves::{FileKind, Plan, Row, Start, Verdict};
use mdshared_app::paths::same_path;
use mdshared_ui::gfx::Rect;
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::overlay::{DIALOG_FOOTER, dialog_card};
use mdshared_ui::rows::{Nav, Rows};
use mdshared_ui::scroll::offset_for_thumb;
use std::path::{Component, Path, PathBuf};

// ---- the numbers of the layout ----------------------------------------------------------------------

pub const CARD_W: f32 = 880.0;
pub const CARD_H: f32 = 620.0;
pub const HEAD_H: f32 = 92.0;
pub const LIST_HEAD: f32 = 40.0;
/// Three lines: the name, where it is and where it goes, and why.
pub const ROW_H: f32 = 62.0;

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
}

pub fn geo(card: Rect) -> Geo {
    let header = Rect::new(card.x, card.y, card.w, HEAD_H);
    let footer = Rect::new(card.x, card.bottom() - DIALOG_FOOTER, card.w, DIALOG_FOOTER);
    let list_head = Rect::new(card.x, header.bottom(), card.w, LIST_HEAD);
    let list = Rect::new(
        card.x,
        list_head.bottom(),
        card.w,
        (footer.y - list_head.bottom() - 4.0).max(0.0),
    );
    Geo {
        card,
        header,
        list_head,
        list,
    }
}

// ---- what a row says ---------------------------------------------------------------------------------

/// `path` with `base`'s components taken off, compared part by part in the platform's case;
/// `None` when it is not below `base`.
pub fn below(path: &Path, base: &Path) -> Option<PathBuf> {
    if base.as_os_str().is_empty() {
        return None;
    }
    let mut parts = path.components();
    for b in base.components() {
        let p = parts.next()?;
        if !same_path(Path::new(p.as_os_str()), Path::new(b.as_os_str())) {
            return None;
        }
    }
    let rest: PathBuf = parts.collect();
    (!rest.as_os_str().is_empty()).then_some(rest)
}

/// A path as a row shows it: below the first of `bases` that holds it, else whole. A path below
/// a base reads as the base's last name and the rest, so "Work\Inbox\plan.md" says which folder
/// it is in without the drive and the folders above.
pub fn short(path: &Path, bases: &[&Path]) -> String {
    for base in bases {
        if let Some(rest) = below(path, base) {
            let name = base
                .components()
                .rev()
                .find_map(|c| match c {
                    Component::Normal(n) => Some(n.to_string_lossy().into_owned()),
                    _ => None,
                })
                .unwrap_or_default();
            let rest = rest.display().to_string();
            return if name.is_empty() {
                rest
            } else {
                format!("{name}{}{rest}", std::path::MAIN_SEPARATOR)
            };
        }
    }
    path.display().to_string()
}

/// What the card is about, in its title.
pub fn title(plan: &Plan) -> String {
    let name = plan.group.as_ref().map_or("", |g| g.name.as_str());
    match plan.start {
        Start::Group => format!("Move {}'s notes into its folder", name),
        Start::Headings => "Move the notes to match the headings".to_string(),
        Start::Library => "Move the library".to_string(),
    }
}

/// The line under the title: where the files go.
pub fn subtitle(plan: &Plan) -> String {
    match (plan.start, &plan.group, &plan.from_library) {
        (Start::Library, _, Some(from)) => {
            format!("From {} to {}", from.display(), plan.library.display())
        }
        (Start::Headings, Some(g), _) => format!(
            "{} \u{00B7} each note into the folder of its section, under {}",
            g.name,
            g.home.display()
        ),
        (_, Some(g), _) => format!("{}'s folder is {}", g.name, g.home.display()),
        _ => plan.library.display().to_string(),
    }
}

/// The folders a row's paths are shown below, nearest first.
pub fn bases(plan: &Plan) -> Vec<&Path> {
    let mut out: Vec<&Path> = Vec::new();
    if let Some(g) = &plan.group {
        out.push(&g.home);
    }
    if let Some(from) = &plan.from_library {
        out.push(from);
    }
    out.push(&plan.library);
    out
}

/// A row's two paths, as the card shows them.
pub fn from_to(plan: &Plan, row: &Row) -> (String, String) {
    let b = bases(plan);
    (short(&row.from, &b), short(&row.to, &b))
}

/// A row's third line: the verdict when it has one, else why it moves.
pub fn why(row: &Row) -> String {
    match &row.verdict {
        Verdict::Ready => row.why.clone(),
        v => v.text(),
    }
}

/// What a row is called: its title, or its file's name.
pub fn name(row: &Row) -> String {
    if row.title.trim().is_empty() {
        row.from
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        row.title.clone()
    }
}

/// The kind of file a row is, when it is not a note.
pub fn kind_text(row: &Row) -> Option<&'static str> {
    match row.kind {
        FileKind::Note => None,
        FileKind::Library => Some("library file"),
        FileKind::Record => Some("record"),
    }
}

// ---- state ----------------------------------------------------------------------------------------------

/// What the pointer can be on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MHit {
    Card,
    Tick(usize),
    Row(usize),
    TickAll,
    UntickAll,
    Cancel,
    Move,
    Thumb,
    Track,
}

/// What an event asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Nothing,
    Close,
    Move,
}

pub struct MoveCard {
    pub plan: Plan,
    pub sel: usize,
    pub scroll: f32,
    pub hover: Option<MHit>,
    pub pressed: Option<MHit>,
    /// Raised by every change of what the card shows: it is in the overlay's signature.
    pub tick: u64,
    pub win: (f32, f32),
    /// The thumb is held, this far below its top.
    pub drag: Option<f32>,
    /// What the last Move was refused with: drawn in the footer until a tick changes.
    pub refusal: Option<String>,
    /// A card a shot draws: Move is refused, and nothing is moved.
    pub demo: bool,
    /// Rectangles recorded as the card is drawn (buttons and links whose size is their text's).
    pub spots: Vec<(MHit, Rect)>,
}

impl MoveCard {
    pub fn new(plan: Plan) -> MoveCard {
        MoveCard {
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

    /// How many rows can be ticked at all.
    pub fn movable(&self) -> usize {
        self.plan
            .rows
            .iter()
            .filter(|r| !r.verdict.blocked())
            .count()
    }

    /// Why Move cannot be pressed, or `None` when it can.
    pub fn problem(&self) -> Option<String> {
        if let Some(why) = &self.plan.blocked {
            return Some(why.clone());
        }
        if self.plan.ticked() == 0 {
            return Some("Nothing is ticked.".to_string());
        }
        None
    }

    /// Ticks or unticks row `i`. A blocked row stays unticked: `Plan::set_ticked` says so.
    pub fn toggle(&mut self, i: usize) {
        let on = self.plan.rows.get(i).is_some_and(|r| !r.ticked);
        self.plan.set_ticked(i, on);
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
        self.scroll = self.rows().reveal(i);
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

    pub fn hit_at(&self, x: f32, y: f32) -> Option<MHit> {
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
                        MHit::Tick(i)
                    } else {
                        MHit::Row(i)
                    }
                }
                None => MHit::Card,
            });
        }
        Some(MHit::Card)
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
        let hit = self.hit_at(x, y).filter(|h| *h != MHit::Card);
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
            Some(MHit::Thumb) => {
                if let Some((_, thumb)) = rows.thumb(g.list) {
                    self.drag = Some(y - thumb.y);
                }
            }
            Some(MHit::Track) => {
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
            MHit::Tick(i) | MHit::Row(i) => {
                self.select(i);
                self.toggle(i);
            }
            MHit::TickAll => self.tick_all(),
            MHit::UntickAll => self.untick_all(),
            MHit::Cancel => return Act::Close,
            MHit::Move => return self.move_pressed(),
            _ => {}
        }
        Act::Nothing
    }

    fn move_pressed(&mut self) -> Act {
        if self.problem().is_some() {
            return Act::Nothing;
        }
        self.tick += 1;
        Act::Move
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
            KeyName::Enter => return self.move_pressed(),
            KeyName::Space => {
                let i = self.sel;
                self.toggle(i);
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
    pub fn sig(&self) -> (u64, Option<MHit>, Option<MHit>) {
        (self.tick, self.hover, self.pressed)
    }
}

// ---- the shots ------------------------------------------------------------------------------------------

/// Every name `--move-demo` takes.
#[cfg(test)]
pub const NAMES: [&str; 2] = ["move", "results"];

/// A folder of notes for the shots: made up, never read or written, and the same on every run.
pub fn demo_root() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"D:\Notes")
    } else {
        PathBuf::from("/home/me/Notes")
    }
}

/// The plan the shots draw: the group Work, whose notes are in the wrong folders, one taken name,
/// one listed twice, one that is a link, and one that lives outside the library.
pub fn demo_plan() -> Plan {
    let root = demo_root();
    let library = root.join("library");
    let home = library.join("Work");
    let downloads = if cfg!(windows) {
        PathBuf::from(r"D:\Downloads")
    } else {
        PathBuf::from("/home/me/Downloads")
    };
    let row = |title: &str,
               from: PathBuf,
               to: PathBuf,
               sections: &[&str],
               why: &str,
               verdict: Verdict| {
        let blocked = verdict.blocked();
        let outside = matches!(verdict, Verdict::Outside(_));
        Row {
            kind: FileKind::Note,
            from,
            to,
            title: title.to_string(),
            sections: sections.iter().map(|s| s.to_string()).collect(),
            why: why.to_string(),
            ticked: !blocked && !outside,
            verdict,
            len: 2_048,
            sha256: String::new(),
            outside,
        }
    };
    let rows = vec![
        row(
            "Quarterly plan",
            home.join("Inbox").join("quarterly-plan.md"),
            home.join("Projects").join("quarterly-plan.md"),
            &["Projects"],
            "its section is Projects",
            Verdict::Ready,
        ),
        row(
            "Standup notes",
            home.join("standup.md"),
            home.join("Meetings").join("Weekly").join("standup.md"),
            &["Meetings", "Weekly"],
            "its section is Meetings \u{203A} Weekly",
            Verdict::Ready,
        ),
        row(
            "Budget",
            home.join("Old").join("budget.md"),
            home.join("Projects").join("budget.md"),
            &["Projects"],
            "its section is Projects",
            Verdict::Taken,
        ),
        row(
            "Interview questions",
            downloads.join("interview-questions.md"),
            home.join("Hiring").join("interview-questions.md"),
            &["Hiring"],
            "not in the group's folder",
            Verdict::Outside(downloads.display().to_string()),
        ),
        row(
            "Offsite agenda",
            home.join("Misc").join("offsite-agenda.md"),
            home.join("offsite-agenda.md"),
            &[],
            "it has no section, so it belongs in the group's own folder",
            Verdict::Ready,
        ),
        row(
            "Roadmap",
            home.join("roadmap.md"),
            home.join("Projects").join("roadmap.md"),
            &["Projects"],
            "its section is Projects",
            Verdict::Listed(2),
        ),
        row(
            "Shared drive",
            home.join("shared.md"),
            home.join("Admin").join("shared.md"),
            &["Admin"],
            "its section is Admin",
            Verdict::NotAFile,
        ),
    ];
    Plan {
        start: Start::Group,
        library: library.clone(),
        from_library: None,
        group: Some(mdedit_core::moves::GroupRef {
            name: "Work".to_string(),
            file: library.join("Work.md"),
            home,
        }),
        label: "the notes of Work into their folder".to_string(),
        rows,
        blocked: None,
        missing: 1,
        others: 0,
        others_capped: false,
        omitted: 0,
    }
}

/// `--move-demo move`: the card over the demonstration plan.
pub fn demo(name: &str) -> Option<MoveCard> {
    match name {
        "move" => {
            let mut card = MoveCard::new(demo_plan());
            card.demo = true;
            Some(card)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_is_shown_below_the_nearest_folder_that_holds_it() {
        let root = demo_root();
        let home = root.join("Work");
        let library = root.join("library");
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(
            short(&home.join("Inbox").join("a.md"), &[&home, &library]),
            format!("Work{sep}Inbox{sep}a.md")
        );
        assert_eq!(
            short(&library.join("Work.md"), &[&home, &library]),
            format!("library{sep}Work.md")
        );
        let elsewhere = PathBuf::from(if cfg!(windows) {
            r"E:\x\a.md"
        } else {
            "/x/a.md"
        });
        assert_eq!(
            short(&elsewhere, &[&home, &library]),
            elsewhere.display().to_string()
        );
        // The folder itself is not below itself.
        assert_eq!(below(&home, &home), None);
    }

    #[test]
    fn a_blocked_row_cannot_be_ticked_by_a_click_space_or_tick_all() {
        let mut card = demo("move").expect("the demo");
        card.resize(1280.0, 800.0);
        let taken = card
            .plan
            .rows
            .iter()
            .position(|r| r.verdict == Verdict::Taken)
            .expect("a taken row");
        assert!(!card.plan.rows[taken].ticked);
        // A click on its tick box.
        let g = card.geo();
        let r = card.rows().rect(g.list, taken);
        card.press(r.x + 20.0, r.cy());
        assert_eq!(card.release(r.x + 20.0, r.cy()), Act::Nothing);
        assert!(!card.plan.rows[taken].ticked, "a click leaves it unticked");
        assert_eq!(card.sel, taken, "but it is selected");
        // Space on it.
        card.key(&Key::Name(KeyName::Space), Mods::default());
        assert!(!card.plan.rows[taken].ticked, "Space leaves it unticked");
        // Tick all.
        card.tick_all();
        assert!(!card.plan.rows[taken].ticked, "Tick all leaves it unticked");
        // And a ready row does tick and untick.
        card.select(0);
        card.key(&Key::Name(KeyName::Space), Mods::default());
        assert!(!card.plan.rows[0].ticked);
        card.key(&Key::Name(KeyName::Space), Mods::default());
        assert!(card.plan.rows[0].ticked);
    }

    #[test]
    fn move_is_refused_with_nothing_ticked_and_escape_closes() {
        let mut card = demo("move").expect("the demo");
        card.resize(1280.0, 800.0);
        assert_eq!(card.problem(), None);
        card.untick_all();
        assert_eq!(card.problem().as_deref(), Some("Nothing is ticked."));
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default()),
            Act::Nothing,
            "Enter does not move with nothing ticked"
        );
        card.tick_all();
        assert_eq!(
            card.key(&Key::Name(KeyName::Enter), Mods::default()),
            Act::Move
        );
        assert_eq!(
            card.key(&Key::Name(KeyName::Escape), Mods::default()),
            Act::Close
        );
    }

    #[test]
    fn an_outside_row_comes_unticked_and_tick_all_leaves_it_so_but_a_click_ticks_it() {
        let mut card = demo("move").expect("the demo");
        card.resize(1280.0, 800.0);
        let out = card
            .plan
            .rows
            .iter()
            .position(|r| r.outside)
            .expect("an outside row");
        assert!(!card.plan.rows[out].ticked);
        card.tick_all();
        assert!(!card.plan.rows[out].ticked);
        card.toggle(out);
        assert!(card.plan.rows[out].ticked, "named, it can be ticked");
    }
}
