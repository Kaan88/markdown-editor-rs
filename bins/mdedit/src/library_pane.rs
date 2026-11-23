//! The notes library in the left pane, in the Files tree's place: a group (a library file) holds
//! its sections, a section holds the sections written under it and its notes. Clicking a note
//! opens it; clicking a group or a section folds it. The rows are made from
//! [`mdshared_app::library`], which is read again when a library file has been saved.

use crate::chrome::{Ctx, glyph};
use crate::commands::strings;
use crate::docs::Docs;
use crate::ui::{BarField, Drag, Hit, Scroller, Ui};
use mdshared_app::library::Library;
pub use mdshared_app::library::tree::{
    Empty, Kind, Row, empty_state, folder_of, key_of, link_path, listing, lists, omitted_label,
    rows, same_note, without_notes,
};
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::pane::{HEAD, ROW, ROW_H, draw_row_fill};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::textfield::{FieldColors, TextField};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long a library is used before the folder is looked at again, while the pane shows.
pub const RECHECK: Duration = Duration::from_millis(800);

/// What the box over the pane is typing a name for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    RenameGroup,
    RenameSection,
    RenameNote,
    RenameFile,
    NewGroup,
    NewSection,
    NewNote,
}

impl EditKind {
    /// A name for something that is not there yet, drawn in a row of its own.
    pub fn is_new(self) -> bool {
        matches!(
            self,
            EditKind::NewGroup | EditKind::NewSection | EditKind::NewNote
        )
    }
}

/// A name being typed: what for, where it is drawn (a row to cover, or the place a new row
/// goes) and what it applies to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    pub kind: EditKind,
    pub at: usize,
    pub depth: u16,
    /// The library file, the heading path (the section itself, or the one a new note or section
    /// goes under) and, for a note, its line and the file it names.
    pub file: PathBuf,
    pub section: Vec<String>,
    pub line: usize,
    pub note: PathBuf,
}

/// A note waiting for the group and section picker to say where it goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moving {
    pub file: PathBuf,
    pub line: usize,
    pub title: String,
    pub note: PathBuf,
}

/// A section waiting for "Remove" in its question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    pub file: PathBuf,
    pub section: Vec<String>,
}

/// What a row is, apart from where it is: how the keyboard's cursor finds its row again after
/// the library was read again and the rows were made anew.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowId {
    /// A group or a section, by its folding key.
    Fold(String),
    /// A note, by the folding key of the heading (or group) it is under, and its file.
    Note(String, PathBuf),
}

impl RowId {
    /// The heading or group this row is under.
    fn parent(&self) -> Option<RowId> {
        match self {
            RowId::Note(under, _) => Some(RowId::Fold(under.clone())),
            RowId::Fold(key) => key
                .rfind('\u{1}')
                .map(|at| RowId::Fold(key[..at].to_string())),
        }
    }
}

/// Where a key moves the cursor, other than to the row before or after.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Go {
    Home,
    End,
    /// A page up or down, in rows.
    Page(isize),
}

#[derive(Default)]
pub struct LibraryPane {
    /// The keyboard is in the pane: its keys move the cursor, and none reaches the text.
    pub focused: bool,
    /// The cursor's row, and what the row is (see [`RowId`]) so that it stays on the same note
    /// or heading when the rows are made again.
    pub cur: Option<usize>,
    cur_id: Option<RowId>,
    /// The cursor moved: the next frame scrolls it into view, when the pane's height is known.
    reveal_cur: bool,
    /// The left pane shows the library, not the Files tree or the function list.
    pub on: bool,
    /// The groups and sections folded shut; everything else is open, so a new library reads
    /// like the file it came from.
    folded: HashSet<String>,
    pub rows: Vec<Row>,
    pub scroll: f32,
    pub content: f32,
    pub view: f32,
    /// When the folder was last looked at, so a pane left open does not stat it every frame.
    pub checked: Option<Instant>,
    /// The library file of the row last clicked, while the pane shows: where "Edit a library
    /// file" goes first.
    pub cursor: Option<PathBuf>,
    /// The name being typed, and the box it is typed in.
    pub edit: Option<Edit>,
    pub field: TextField,
    /// The note waiting for the picker to say where it goes, the note waiting for the file
    /// picker to say where it went, and the section waiting for the answer to its question.
    pub moving: Option<Moving>,
    pub finding: Option<Moving>,
    pub removing: Option<Removal>,
    /// The library file waiting for "Delete" in its question.
    pub deleting: Option<PathBuf>,
    /// The group or section a note being dragged would go into, while one is dragged. It is
    /// never where the note already is, so a drop with no mark changes nothing.
    pub drop: Option<usize>,
}

/// Whether two places in a library are the same one: the same file, and the same heading path
/// under it (matched as the library matches headings, without regard to ASCII case).
fn same_place(a: &(PathBuf, Vec<String>), b: &(PathBuf, Vec<String>)) -> bool {
    mdshared_app::paths::same_path(&a.0, &b.0)
        && a.1.len() == b.1.len()
        && a.1.iter().zip(&b.1).all(|(x, y)| x.eq_ignore_ascii_case(y))
}

impl LibraryPane {
    /// Scrolls so that row `at` shows.
    pub fn reveal(&mut self, at: usize) {
        let top = at as f32 * ROW;
        let bottom = top + ROW + 8.0;
        if top < self.scroll {
            self.scroll = top;
        } else if bottom > self.scroll + self.view {
            self.scroll = (bottom - self.view).max(0.0);
        }
    }

    /// Gives up the name being typed. The rows are the ones the files say again.
    pub fn stop_edit(&mut self) {
        if self.edit.take().is_some() {
            self.field.clear();
            self.rows.retain(|r| r.kind != Kind::Editing);
        }
    }

    /// Starts typing a name over row `at` (a rename) or in a row of its own at `at` (a new one).
    pub fn start_edit(&mut self, edit: Edit, text: &str, library: &Library) {
        self.edit = Some(edit);
        self.field.set_text(text);
        self.field.select_all();
        self.build(library);
    }

    /// The library file the group of row `i` is: the row itself, or the one above it that
    /// starts the group.
    pub fn group_file(&self, i: usize) -> Option<PathBuf> {
        let rows = self.rows.get(..=i)?;
        let group = rows.iter().rev().find(|r| r.kind == Kind::Group)?;
        Some(PathBuf::from(&group.key))
    }

    pub fn is_folded(&self, key: &str) -> bool {
        self.folded.contains(key)
    }

    /// Folds a group or a section, or opens it again.
    pub fn fold(&mut self, key: &str) {
        if !self.folded.remove(key) {
            self.folded.insert(key.to_string());
        }
    }

    pub fn scroll_by(&mut self, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, (self.content - self.view).max(0.0));
    }

    /// Whether the folder should be looked at again before the rows are made.
    pub fn due(&self, now: Instant) -> bool {
        self.checked
            .is_none_or(|t| now.duration_since(t) >= RECHECK)
    }

    /// Whether row `i` is what `id` names.
    fn is(&self, i: usize, id: &RowId) -> bool {
        let Some(row) = self.rows.get(i) else {
            return false;
        };
        match (id, &row.kind) {
            (RowId::Fold(key), Kind::Group | Kind::Section) => row.key == *key,
            (RowId::Note(under, path), Kind::Note { path: p, .. }) => {
                p == path
                    && self
                        .parent_of(i)
                        .is_some_and(|at| self.rows[at].key == *under)
            }
            _ => false,
        }
    }

    /// What row `i` is, or `None` for the row a name is typed in.
    pub fn id_at(&self, i: usize) -> Option<RowId> {
        let row = self.rows.get(i)?;
        match &row.kind {
            Kind::Group | Kind::Section => Some(RowId::Fold(row.key.clone())),
            Kind::Note { path, .. } => Some(RowId::Note(
                self.rows[self.parent_of(i)?].key.clone(),
                path.clone(),
            )),
            Kind::Editing => None,
        }
    }

    /// The group or section row `i` stands under: the nearest row above it that is less deep.
    pub fn parent_of(&self, i: usize) -> Option<usize> {
        let depth = self.rows.get(i)?.depth;
        (0..i).rev().find(|&at| self.rows[at].depth < depth)
    }

    /// The row under `y` in the list `area`, when the pointer is on one.
    pub fn row_at(&self, y: f32, area: Rect) -> Option<usize> {
        if y < area.y || y >= area.bottom() {
            return None;
        }
        let at = ((y - area.y + self.scroll) / ROW).floor();
        if at < 0.0 {
            return None;
        }
        let at = at as usize;
        (at < self.rows.len()).then_some(at)
    }

    /// Where the note on `row` is listed: its library file, and the heading path it is written
    /// under (empty for the group's own notes). `None` for a row that is not a note.
    pub fn note_place(&self, row: usize) -> Option<(PathBuf, Vec<String>)> {
        if !matches!(self.rows.get(row)?.kind, Kind::Note { .. }) {
            return None;
        }
        let file = self.group_file(row)?;
        let parent = self.parent_of(row)?;
        Some((file, self.rows[parent].section.clone()))
    }

    /// Where a drop on `row` writes: a group's own notes, or a section's. `None` for a note and
    /// for the row a name is being typed in.
    pub fn row_place(&self, row: usize) -> Option<(PathBuf, Vec<String>)> {
        match &self.rows.get(row)?.kind {
            Kind::Group => Some((self.group_file(row)?, Vec::new())),
            Kind::Section => Some((self.group_file(row)?, self.rows[row].section.clone())),
            _ => None,
        }
    }

    /// The group or section a note dragged from `source` goes into when dropped at `y`: the row
    /// under the pointer, or the heading the note under it is written in. `None` when the
    /// pointer is not on a row of the list, or on the place the note is already in.
    pub fn drop_row(&self, y: f32, area: Rect, source: usize) -> Option<usize> {
        let at = self.drop_place(y, area)?;
        let from = self.note_place(source)?;
        let to = self.row_place(at)?;
        (!same_place(&from, &to)).then_some(at)
    }

    /// The group or section a drop at `y` goes into: the row under the pointer, or the heading
    /// the note under it is written in. `None` when the pointer is not on a row of the list.
    pub fn drop_place(&self, y: f32, area: Rect) -> Option<usize> {
        let mut at = self.row_at(y, area)?;
        while matches!(self.rows.get(at)?.kind, Kind::Note { .. }) {
            at = self.parent_of(at)?;
        }
        self.row_place(at).map(|_| at)
    }

    /// Puts the cursor on row `i` and has the next frame show it.
    pub fn set_cur(&mut self, i: usize) {
        if let Some(id) = self.id_at(i) {
            self.cur = Some(i);
            self.cur_id = Some(id);
            self.reveal_cur = true;
        }
    }

    /// Puts the cursor on what `id` names, if the rows have it.
    pub fn follow(&mut self, id: RowId) {
        if let Some(at) = (0..self.rows.len()).find(|&i| self.is(i, &id)) {
            self.set_cur(at);
        }
    }

    /// Scrolls the cursor into view when it moved since the last frame.
    pub fn show_cur(&mut self) {
        if std::mem::take(&mut self.reveal_cur)
            && let Some(at) = self.cur
        {
            self.reveal(at);
        }
    }

    /// The cursor's row after the rows were made anew: the same note or heading; when that is
    /// gone, the heading or group it was under; when that is gone too, the same place.
    fn restore_cur(&mut self) {
        let n = self.rows.len();
        let Some(mut id) = self.cur_id.clone() else {
            self.cur = None;
            return;
        };
        loop {
            if let Some(at) = (0..n).find(|&i| self.is(i, &id)) {
                (self.cur, self.cur_id) = (Some(at), Some(id));
                return;
            }
            match id.parent() {
                Some(up) => id = up,
                None => break,
            }
        }
        let at = self.cur.unwrap_or(0).min(n.saturating_sub(1));
        match self.id_at(at) {
            Some(id) => (self.cur, self.cur_id) = (Some(at), Some(id)),
            None => (self.cur, self.cur_id) = (None, None),
        }
    }

    /// The row a key moves the cursor to, from where it is (none: the ends).
    fn target(&self, go: Go) -> Option<usize> {
        let n = self.rows.len();
        if n == 0 {
            return None;
        }
        let ok = |i: &usize| self.rows[*i].kind != Kind::Editing;
        match (go, self.cur) {
            (Go::Home, _) => (0..n).find(ok),
            (Go::End, _) => (0..n).rev().find(ok),
            (Go::Page(by), Some(c)) => {
                let to = (c as isize + by).clamp(0, n as isize - 1) as usize;
                Some(to).filter(ok)
            }
            (Go::Page(by), None) if by > 0 => (0..n).find(ok),
            (Go::Page(_), None) => (0..n).rev().find(ok),
        }
    }

    /// Up or down one row; with no cursor yet, to the first or the last.
    pub fn step(&mut self, down: bool) {
        let n = self.rows.len();
        let ok = |i: &usize| self.rows[*i].kind != Kind::Editing;
        let to = match (self.cur, down) {
            (None, true) => (0..n).find(ok),
            (None, false) => (0..n).rev().find(ok),
            (Some(c), true) => (c + 1..n).find(ok),
            (Some(c), false) => (0..c).rev().find(ok),
        };
        if let Some(to) = to {
            self.set_cur(to);
        }
    }

    /// Home, End, a page.
    pub fn go(&mut self, go: Go) {
        if let Some(to) = self.target(go) {
            self.set_cur(to);
        }
    }

    /// Left: shuts an open group or section, else goes to the group or section it is under.
    pub fn left(&mut self, library: &Library) {
        let Some(c) = self.cur else { return };
        let Some(row) = self.rows.get(c) else { return };
        if matches!(row.kind, Kind::Group | Kind::Section) && !self.is_folded(&row.key) {
            let key = row.key.clone();
            self.fold(&key);
            self.build(library);
        } else if let Some(up) = self.parent_of(c) {
            self.set_cur(up);
        }
    }

    /// Right: opens a shut group or section, else goes to its first row.
    pub fn right(&mut self, library: &Library) {
        let Some(c) = self.cur else { return };
        let Some(row) = self.rows.get(c) else { return };
        if !matches!(row.kind, Kind::Group | Kind::Section) {
            return;
        }
        if self.is_folded(&row.key) {
            let key = row.key.clone();
            self.fold(&key);
            self.build(library);
        } else if self.rows.get(c + 1).is_some_and(|r| r.depth > row.depth) {
            self.set_cur(c + 1);
        }
    }

    /// A typed letter: the next row (going round) whose label starts with it.
    pub fn jump(&mut self, ch: char) {
        let n = self.rows.len();
        let from = self.cur.map_or(0, |c| c + 1);
        let want = ch.to_lowercase().next().unwrap_or(ch);
        let hit = (0..n).map(|k| (from + k) % n).find(|&i| {
            let row = &self.rows[i];
            row.kind != Kind::Editing
                && row
                    .label
                    .chars()
                    .next()
                    .is_some_and(|c| c.to_lowercase().next() == Some(want))
        });
        if let Some(i) = hit {
            self.set_cur(i);
        }
    }

    /// Opens what is folded above the note under `section` in `file`, so that its row is there.
    pub fn unfold_to(&mut self, file: &std::path::Path, section: &[String]) {
        for n in 0..=section.len() {
            self.folded.remove(&key_of(file, &section[..n]));
        }
    }

    /// The rows of `library`, in the order the files are written, leaving out what is folded.
    pub fn build(&mut self, library: &Library) {
        self.rows = rows(library, &self.folded);
        // What is being named and is not there yet has a row of its own, where it will be.
        if let Some(e) = self.edit.as_ref().filter(|e| e.kind.is_new()) {
            let at = e.at.min(self.rows.len());
            self.rows.insert(
                at,
                Row {
                    kind: Kind::Editing,
                    label: String::new(),
                    depth: e.depth,
                    key: String::new(),
                    section: Vec::new(),
                    home: None,
                },
            );
        }
        self.restore_cur();
    }
}

impl EditKind {
    /// What the empty box says it is for.
    pub fn placeholder(self) -> &'static str {
        match self {
            EditKind::NewGroup => strings::LIBRARY_NEW_GROUP_PLACEHOLDER,
            EditKind::NewSection => strings::LIBRARY_NEW_SECTION_PLACEHOLDER,
            EditKind::NewNote => strings::LIBRARY_NEW_NOTE_PLACEHOLDER,
            _ => strings::LIBRARY_NAME_PLACEHOLDER,
        }
    }
}

/// Where the text of the name box goes inside its frame.
pub fn field_area(frame: Rect) -> Rect {
    Rect::new(
        frame.x + 6.0,
        frame.y + 1.0,
        (frame.w - 12.0).max(0.0),
        frame.h - 2.0,
    )
}

/// A pane narrower than this leaves the notes' folders out: the titles need the room.
pub const FOLDER_MIN_PANE: f32 = 210.0;
const FOLDER_GAP: f32 = 10.0;

/// `text` as far as `room` allows, keeping its end and putting an ellipsis where the start was
/// (the end of a folder's name tells more than its start).
pub fn elide_left(p: &mut mdshared_ui::gfx::Painter, size: f32, text: &str, room: f32) -> String {
    if p.measure(Face::Ui, size, text) <= room {
        return text.to_string();
    }
    let starts: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    for &at in &starts {
        let tail = format!("\u{2026}{}", &text[at..]);
        if p.measure(Face::Ui, size, &tail) <= room {
            return tail;
        }
    }
    String::new()
}

/// The folder as it is shown on a row whose title is `title` wide and which has `room` for
/// both, and the room it takes with its gap (0 when there is none to spare). The title keeps
/// what it needs unless that leaves the folder under a third of the row.
pub fn folder_fit(
    p: &mut mdshared_ui::gfx::Painter,
    folder: &str,
    room: f32,
    title: f32,
) -> (String, f32) {
    let spare = (room - title - FOLDER_GAP).max(room * 0.3 - FOLDER_GAP);
    if spare < 24.0 {
        return (String::new(), 0.0);
    }
    let shown = elide_left(p, 11.5, folder, spare);
    if shown.is_empty() {
        return (shown, 0.0);
    }
    let w = p.measure(Face::Ui, 11.5, &shown);
    (shown, w)
}

/// The folder shown at the right of `row`, and the room it takes: a note's own folder, or the
/// folder a group's new notes go in. Nothing at all on a pane narrower than
/// [`FOLDER_MIN_PANE`], where the titles need the room.
fn row_folder(
    p: &mut mdshared_ui::gfx::Painter,
    row: &Row,
    face: Face,
    pane: f32,
    room: f32,
) -> (String, f32) {
    if pane < FOLDER_MIN_PANE {
        return (String::new(), 0.0);
    }
    let folder = match &row.kind {
        Kind::Note { path, .. } => folder_of(path),
        _ => row.home.clone(),
    };
    let Some(folder) = folder else {
        return (String::new(), 0.0);
    };
    let title = p.measure(face, 12.5, &row.label);
    folder_fit(p, &folder, room, title)
}

/// A line and the buttons under it, as the Files pane has when no folder is open.
fn draw_empty(c: &mut Ctx, r: Rect, area: Rect, state: Empty, has_file: bool) {
    let pal = *c.pal;
    let cy = area.y + 40.0;
    let text = match state {
        Empty::NoFolder => strings::LIBRARY_PANE_NO_FOLDER,
        Empty::NoNotes => strings::LIBRARY_EMPTY_PANE,
    };
    c.p.text_center(
        Face::Ui,
        13.0,
        Rect::new(r.x + 8.0, cy - 10.0, r.w - 16.0, 20.0),
        text,
        pal.muted,
    );
    let mut buttons = vec![(
        Hit::LibraryNewGroup,
        strings::LIBRARY_NEW_GROUP_BUTTON,
        true,
    )];
    if state == Empty::NoNotes {
        buttons.push((Hit::LibraryAddOpen, strings::LIBRARY_ADD_BUTTON, has_file));
    }
    let mut y = cy + 18.0;
    for (hit, label, enabled) in buttons {
        let w = c.p.measure(Face::Ui, 13.0, label) + 28.0;
        let b = Rect::new((r.cx() - w / 2.0).round(), y, w, 30.0);
        let fill = if enabled {
            c.fill_for(hit, false, false).unwrap_or(pal.bar)
        } else {
            pal.bar
        };
        c.p.frame(b, 6.0, fill, pal.line);
        let ink = if enabled { pal.text } else { pal.faint };
        c.p.text_center(Face::Ui, 13.0, b, label, ink);
        if enabled {
            c.hit(hit, b);
        }
        y += 38.0;
    }
}

/// The rows below the head.
pub fn list(r: Rect) -> Rect {
    let ry = r.y + HEAD;
    Rect::new(r.x, ry, r.w, (r.bottom() - ry).max(0.0))
}

pub fn draw(c: &mut Ctx, docs: &Docs, ui: &mut Ui, r: Rect, plan_ready: Option<usize>) {
    let pal = *c.pal;
    c.p.fill(r, pal.pane);
    crate::panes::draw_pane_head(c, ui, r);

    let dragging = matches!(ui.drag, Some(Drag::Splitter { .. }));
    let mut area = list(r);
    // The pane's foot rows, each one row high and taken from the list so that no note shows
    // under them: "Plan ready" at the very bottom, and above it the count of a folder too large
    // for the reader ("512 of 583 groups shown").
    let mut bottom = r.bottom();
    if plan_ready.is_some() {
        bottom -= ROW_H;
    }
    let omitted = omitted_label(
        docs.library.groups.len(),
        docs.library_stamp.groups.len(),
        docs.library.omitted,
    );
    let omitted_foot = omitted.as_ref().map(|_| {
        bottom -= ROW_H;
        Rect::new(r.x, bottom, r.w, ROW_H)
    });
    area.h = (bottom - area.y).max(0.0);
    let active = docs.active_doc().and_then(|d| d.path.clone());
    let focused = ui.find_focused && ui.field == BarField::Library;
    let caret_on = ui.caret_on;
    // The row whose menu is up stays lit.
    let menu_row = match ui.popup_kind() {
        Some(crate::ui::PopupKind::LibraryRow(i)) => Some(i),
        _ => None,
    };
    // A note being dragged: the row it came from, and the heading it would go into.
    let dragged = match ui.drag {
        Some(Drag::LibraryNote {
            row, moving: true, ..
        }) => Some(row),
        _ => None,
    };
    let target = dragged.and(ui.library.drop);
    let lib = &mut ui.library;
    lib.content = lib.rows.len() as f32 * ROW + 8.0;
    lib.view = area.h;
    lib.scroll = lib.scroll.clamp(0.0, (lib.content - area.h).max(0.0));
    lib.show_cur();
    lib.scroll = lib.scroll.clamp(0.0, (lib.content - area.h).max(0.0));
    let scroll = lib.scroll;
    // Before the folder has been read (the pane came back at start) there is nothing to say yet.
    if lib.rows.is_empty() && lib.checked.is_some() {
        let has_file = docs.active_doc().is_some_and(|d| d.path.is_some());
        draw_empty(c, r, area, empty_state(&docs.library.dir), has_file);
    }
    let quiet = mix(pal.text, pal.muted, 0.35);
    let cursor_on = lib.focused && !focused;
    let old = c.p.clip(area);
    let first = (scroll / ROW).floor() as usize;
    for (i, row) in lib.rows.iter().enumerate().skip(first) {
        let y = area.y + i as f32 * ROW - scroll;
        if y > area.bottom() {
            break;
        }
        let rr = Rect::new(r.x + 6.0, y, r.w - 12.0, ROW_H);
        let hit = Hit::LibraryRow(i);
        let selected = match &row.kind {
            Kind::Note { path, .. } => active.as_deref().is_some_and(|a| same_note(a, path)),
            _ => false,
        };
        // The keyboard's row: an outline in the accent colour, as the pane's focus.
        if cursor_on && lib.cur == Some(i) {
            c.p.rbox(rr, 5.0, pal.accent);
            let inner = Rect::new(rr.x + 1.0, rr.y + 1.0, rr.w - 2.0, rr.h - 2.0);
            c.p.rbox(inner, 4.0, if selected { pal.on } else { pal.hover });
            if selected {
                c.p.rbox(
                    Rect::new(rr.x, rr.y + 7.0, 3.0, rr.h - 14.0),
                    1.5,
                    pal.accent,
                );
            }
        } else {
            draw_row_fill(
                c.p,
                &pal,
                rr,
                c.hover == Some(hit) || menu_row == Some(i),
                c.pressed == Some(hit),
                selected,
            );
        }
        if target == Some(i) {
            // Where a dragged note would go: the accent's ring over a tint, so the place is seen
            // before the button comes up.
            c.p.rbox(rr, 5.0, pal.accent);
            let inner = Rect::new(rr.x + 1.0, rr.y + 1.0, rr.w - 2.0, rr.h - 2.0);
            c.p.rbox(inner, 4.0, pal.on);
        } else if dragged == Some(i) {
            // The note being moved: a quiet ring, and its text dimmed below.
            c.p.rbox(rr, 5.0, pal.line);
            let inner = Rect::new(rr.x + 1.0, rr.y + 1.0, rr.w - 2.0, rr.h - 2.0);
            c.p.rbox(inner, 4.0, if selected { pal.on } else { pal.hover });
        }
        let ix = r.x + 14.0 + (row.depth as f32 + 1.0) * 14.0;
        let cy = rr.cy();
        let (face, color) = match &row.kind {
            Kind::Group => {
                let arrow = if lib.is_folded(&row.key) {
                    glyph::RIGHT
                } else {
                    glyph::DOWN
                };
                c.p.icon(arrow, 8.0, ix - 8.0, cy, pal.faint);
                c.p.icon(glyph::BOOKMARK, 13.0, ix + 9.0, cy, pal.folder);
                (Face::Semibold, pal.text)
            }
            Kind::Section => {
                let arrow = if lib.is_folded(&row.key) {
                    glyph::RIGHT
                } else {
                    glyph::DOWN
                };
                c.p.icon(arrow, 8.0, ix - 8.0, cy, pal.faint);
                (Face::Semibold, mix(pal.text, pal.muted, 0.15))
            }
            Kind::Note { missing, .. } => {
                let icon = if *missing { glyph::WARN } else { glyph::DOC };
                let color = if selected { pal.accent } else { pal.muted };
                c.p.icon(icon, 13.0, ix + 9.0, cy, color);
                let color = if selected {
                    pal.text
                } else if *missing {
                    pal.muted
                } else {
                    quiet
                };
                (Face::Ui, color)
            }
            Kind::Editing => {
                match lib.edit.as_ref().map(|e| e.kind) {
                    Some(EditKind::NewNote) => {
                        c.p.icon(glyph::DOC, 13.0, ix + 9.0, cy, pal.muted);
                    }
                    Some(EditKind::NewSection) => {
                        c.p.icon(glyph::DOWN, 8.0, ix - 8.0, cy, pal.faint);
                    }
                    _ => c.p.icon(glyph::BOOKMARK, 13.0, ix + 9.0, cy, pal.folder),
                }
                (Face::Ui, pal.text)
            }
        };
        // The note being moved reads as set aside until the button comes up.
        let color = if dragged == Some(i) { pal.faint } else { color };
        let x = if matches!(row.kind, Kind::Section) {
            ix + 4.0
        } else {
            ix + 23.0
        };
        c.hit(hit, rr);
        if lib.edit.as_ref().is_some_and(|e| e.at == i) {
            let frame = Rect::new(
                x - 6.0,
                rr.y + 1.0,
                (rr.right() - x + 2.0).max(40.0),
                rr.h - 2.0,
            );
            let hint = lib.edit.as_ref().map_or("", |e| e.kind.placeholder());
            c.p.frame(frame, 4.0, pal.field, pal.accent);
            let colors = FieldColors {
                text: pal.text,
                placeholder: pal.faint,
                selection: pal.selection,
                caret: pal.text,
            };
            let area = field_area(frame);
            lib.field
                .draw(c.p, area, Face::Ui, 13.0, &colors, hint, focused, caret_on);
            c.hit(Hit::LibraryField, frame);
        } else {
            let right = rr.right() - 12.0;
            let mut max = (right - x).max(0.0);
            // A note's folder, or where a group's new notes go, dimmed at the right: which
            // taxes.md is this one, and where the next note of this group lands.
            let (shown, w) = row_folder(c.p, row, face, r.w, max);
            if w > 0.0 {
                c.p.text_right(Face::Ui, 11.5, right, cy, &shown, pal.faint);
                max = (max - w - FOLDER_GAP).max(0.0);
            }
            c.p.text(face, 12.5, x, cy, &row.label, color, max);
        }
    }
    c.p.restore(old);
    let thumb_hit = Hit::ScrollBar(Scroller::Files);
    if let Some((track, thumb)) = scrollbar(area, lib.content, area.h, scroll) {
        let active = c.hover == Some(thumb_hit)
            || matches!(
                ui.drag,
                Some(Drag::Thumb {
                    target: Scroller::Files,
                    ..
                })
            );
        draw_scrollbar(c.p, &pal, track, thumb, active);
        c.hit(thumb_hit, track);
    }
    crate::panes::edge(c, r, dragging);
    if let (Some(foot), Some(label)) = (omitted_foot, &omitted) {
        c.p.fill(Rect::new(foot.x, foot.y, foot.w, c.p.px()), pal.popup_line);
        c.p.text(
            Face::Ui,
            12.0,
            r.x + 14.0,
            foot.y + 8.0,
            label,
            pal.muted,
            (foot.w - 28.0).max(0.0),
        );
    }
    if let Some(n) = plan_ready {
        let foot = Rect::new(r.x, r.bottom() - ROW_H, r.w, ROW_H);
        c.p.fill(Rect::new(foot.x, foot.y, foot.w, c.p.px()), pal.popup_line);
        let label = format!(
            "Plan ready: {}",
            if n == 1 {
                "1 note".to_string()
            } else {
                format!("{n} notes")
            }
        );
        let mut x = r.x + 14.0;
        let max = (foot.w - 28.0).max(0.0);
        let ty = foot.y + 8.0;
        let w = c.p.text(Face::Ui, 12.0, x, ty, &label, pal.text, max);
        x += w + 6.0;
        for (label, hit, gap) in [
            ("\u{00B7}", None, 6.0),
            ("Show", Some(Hit::LibraryPlanShow), 12.0),
            ("\u{00B7}", None, 6.0),
            ("Drop", Some(Hit::LibraryPlanDrop), 0.0),
        ] {
            if x > r.right() - 8.0 {
                break;
            }
            match hit {
                Some(hit) => {
                    let tw = c.p.measure(Face::Ui, 12.0, label) + 8.0;
                    let b = Rect::new(x, foot.y + 3.0, tw, 20.0);
                    if c.hover == Some(hit) {
                        c.p.rbox(b, 4.0, pal.hover);
                    }
                    c.p.text(Face::Ui, 12.0, x + 4.0, ty, label, pal.accent, tw);
                    c.hit(hit, b);
                    x += tw + gap;
                }
                None => {
                    x += c.p.text(Face::Ui, 12.0, x, ty, label, pal.faint, 20.0) + gap;
                }
            }
        }
    }
    // The note being dragged, at the pointer: what is moving, and that it is still held.
    if let Some(src) = dragged
        && let Some(row) = ui.library.rows.get(src)
    {
        let label = &row.label;
        let w = (c.p.measure(Face::Ui, 12.5, label) + 38.0).min((r.w - 8.0).max(60.0));
        let x = (ui.mouse.0 + 14.0).min(r.right() - w - 4.0).max(r.x + 4.0);
        let y = (ui.mouse.1 + 12.0)
            .min(r.bottom() - ROW_H - 4.0)
            .max(r.y + HEAD + 2.0);
        let chip = Rect::new(x, y, w, ROW_H);
        c.p.frame(chip, 6.0, pal.popup, pal.accent);
        c.p.icon(glyph::DOC, 13.0, chip.x + 14.0, chip.cy(), pal.muted);
        c.p.text(
            Face::Ui,
            12.5,
            chip.x + 26.0,
            chip.cy(),
            label,
            pal.text,
            (w - 34.0).max(0.0),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdshared_app::library::{Entry, Group};

    fn entry(title: &str, sections: &[&str]) -> Entry {
        Entry {
            title: title.to_string(),
            path: PathBuf::from(format!("D:/{title}.md")),
            sections: sections.iter().map(|s| (*s).to_string()).collect(),
            line: 1,
            missing: false,
        }
    }

    fn library() -> Library {
        let group = Group {
            name: "Personal".to_string(),
            home: None,
            file: PathBuf::from("D:/lib/personal.md"),
            stamp: mdshared_app::files::FileStamp::Missing,
            sections: vec![
                vec!["Money".to_string()],
                vec!["Money".to_string(), "2026".to_string()],
                vec!["Trips".to_string()],
            ],
            entries: vec![
                entry("Inbox", &[]),
                entry("Taxes", &["Money"]),
                entry("This year", &["Money", "2026"]),
                entry("Japan", &["Trips"]),
            ],
        };
        Library {
            dir: PathBuf::from("D:/lib"),
            groups: vec![group],
            omitted: 0,
        }
    }

    fn shown(pane: &LibraryPane) -> Vec<(u16, &str)> {
        pane.rows
            .iter()
            .map(|r| (r.depth, r.label.as_str()))
            .collect()
    }

    #[test]
    fn the_pane_is_a_tree_of_groups_sections_and_notes() {
        let mut pane = LibraryPane::default();
        pane.build(&library());
        assert_eq!(
            shown(&pane),
            vec![
                (0, "Personal"),
                (1, "Inbox"),
                (1, "Money"),
                (2, "Taxes"),
                (2, "2026"),
                (3, "This year"),
                (1, "Trips"),
                (2, "Japan"),
            ]
        );
    }

    #[test]
    fn folding_a_section_hides_what_is_under_it_and_opens_again() {
        let lib = library();
        let mut pane = LibraryPane::default();
        pane.build(&lib);
        let money = pane.rows[2].key.clone();
        pane.fold(&money);
        pane.build(&lib);
        assert_eq!(
            shown(&pane),
            vec![
                (0, "Personal"),
                (1, "Inbox"),
                (1, "Money"),
                (1, "Trips"),
                (2, "Japan"),
            ],
            "the section's notes and the section inside it are gone"
        );
        // The group folds to one row; opening it again brings back what was open before.
        let group = pane.rows[0].key.clone();
        pane.fold(&group);
        pane.build(&lib);
        assert_eq!(shown(&pane), vec![(0, "Personal")]);
        pane.fold(&group);
        pane.fold(&money);
        pane.build(&lib);
        assert_eq!(shown(&pane).len(), 8);
    }
    fn painter() -> (mdshared_text::Bitmap, mdshared_ui::gfx::Gfx) {
        (
            mdshared_text::Bitmap::new(8, 8),
            mdshared_ui::gfx::Gfx::new(),
        )
    }

    #[test]
    fn a_folder_name_is_cut_from_its_left_to_fit_and_keeps_its_end() {
        let (mut bm, mut gfx) = painter();
        let mut p = mdshared_ui::gfx::Painter::new(&mut bm, &mut gfx, 1.0);
        let whole = "Documents-2026-taxes";
        let full = p.measure(Face::Ui, 11.5, whole);
        assert_eq!(elide_left(&mut p, 11.5, whole, full + 1.0), whole);
        let cut = elide_left(&mut p, 11.5, whole, full * 0.5);
        assert!(
            cut.starts_with('\u{2026}') && whole.ends_with(&cut[3..]),
            "{cut}"
        );
        assert!(p.measure(Face::Ui, 11.5, &cut) <= full * 0.5, "{cut}");
        assert!(cut.chars().count() < whole.chars().count());
        assert_eq!(
            elide_left(&mut p, 11.5, whole, 2.0),
            "",
            "no room, no folder"
        );
    }

    #[test]
    fn the_title_keeps_its_room_and_the_folder_gets_what_is_left_or_a_third() {
        let (mut bm, mut gfx) = painter();
        let mut p = mdshared_ui::gfx::Painter::new(&mut bm, &mut gfx, 1.0);
        // Plenty of room: the whole folder.
        let (shown, w) = folder_fit(&mut p, "taxes", 300.0, 60.0);
        assert_eq!(shown, "taxes");
        assert!(w > 0.0 && w < 60.0);
        // A long title leaves little: the folder still gets about a third, cut from the left.
        let (shown, w) = folder_fit(&mut p, "Documents-2026-taxes", 200.0, 190.0);
        assert!(shown.starts_with('\u{2026}'), "{shown}");
        assert!(w <= 200.0 * 0.3 + 1.0, "{w}");
        // A row too narrow for even that leaves the folder out.
        assert_eq!(
            folder_fit(&mut p, "taxes", 60.0, 55.0),
            (String::new(), 0.0)
        );
        // ...even for a short name, which would fit in the 5 px the title leaves.
        assert_eq!(folder_fit(&mut p, "a", 60.0, 55.0), (String::new(), 0.0));
    }

    #[test]
    fn a_group_row_shows_the_folder_its_home_line_names_and_only_on_a_wide_enough_pane() {
        let (mut bm, mut gfx) = painter();
        let mut p = mdshared_ui::gfx::Painter::new(&mut bm, &mut gfx, 1.0);
        let mut row = Row {
            kind: Kind::Group,
            label: "Personal".to_string(),
            depth: 0,
            key: String::new(),
            section: Vec::new(),
            home: Some("work".to_string()),
        };
        let (shown, w) = row_folder(&mut p, &row, Face::Semibold, 300.0, 200.0);
        assert_eq!(shown, "work");
        assert!(w > 0.0 && w < 200.0);
        // A pane narrower than FOLDER_MIN_PANE leaves it out, as it does a note's folder.
        assert_eq!(
            row_folder(&mut p, &row, Face::Semibold, 200.0, 200.0),
            (String::new(), 0.0)
        );
        assert_eq!(
            row_folder(&mut p, &row, Face::Semibold, FOLDER_MIN_PANE, 200.0).0,
            "work"
        );
        // A group with no `home:` line shows nothing.
        row.home = None;
        assert_eq!(
            row_folder(&mut p, &row, Face::Semibold, 300.0, 200.0),
            (String::new(), 0.0)
        );
        // A long folder is cut from its left, as a note's is: its end tells more.
        row.home = Some("Documents-2026-taxes-and-notes".to_string());
        let (shown, _) = row_folder(&mut p, &row, Face::Semibold, 300.0, 190.0);
        assert!(shown.starts_with('\u{2026}'), "{shown}");
        assert!(shown.ends_with("notes"), "{shown}");
    }

    #[test]
    fn the_cursor_follows_its_note_through_new_rows_and_falls_back_to_its_heading() {
        let mut lib = library();
        let mut pane = LibraryPane::default();
        pane.build(&lib);
        let taxes = pane.rows.iter().position(|r| r.label == "Taxes").unwrap();
        pane.set_cur(taxes);
        // A note added above it: its row moves, the cursor with it.
        lib.groups[0]
            .entries
            .insert(1, entry("Aardvark", &["Money"]));
        pane.build(&lib);
        assert_eq!(pane.cur.map(|c| pane.rows[c].label.as_str()), Some("Taxes"));
        assert_eq!(pane.cur, Some(taxes + 1));
        // The note gone: the heading it was under.
        lib.groups[0].entries.retain(|e| e.title != "Taxes");
        pane.build(&lib);
        assert_eq!(pane.cur.map(|c| pane.rows[c].label.as_str()), Some("Money"));
        // The heading folded away inside its group: the group.
        let money = pane.rows[pane.cur.unwrap()].key.clone();
        pane.set_cur(pane.rows.iter().position(|r| r.label == "2026").unwrap());
        pane.fold(&money);
        pane.build(&lib);
        assert_eq!(pane.cur.map(|c| pane.rows[c].label.as_str()), Some("Money"));
        // Every row gone: no cursor.
        lib.groups.clear();
        pane.build(&lib);
        assert_eq!(pane.cur, None);
    }

    #[test]
    fn the_keys_move_the_cursor_by_the_tree() {
        let lib = library();
        let mut pane = LibraryPane::default();
        pane.build(&lib);
        let at = |p: &LibraryPane| p.cur.map(|c| p.rows[c].label.clone()).unwrap_or_default();
        pane.step(false);
        assert_eq!(at(&pane), "Japan", "no cursor yet: Up goes to the last row");
        pane.go(Go::Home);
        assert_eq!(at(&pane), "Personal");
        pane.right(&lib);
        assert_eq!(at(&pane), "Inbox");
        pane.jump('t');
        assert_eq!(at(&pane), "Taxes");
        pane.jump('t');
        assert_eq!(at(&pane), "This year");
        pane.jump('t');
        assert_eq!(at(&pane), "Trips");
        pane.left(&lib);
        assert_eq!(pane.rows.len(), 7, "Trips shut");
        assert_eq!(at(&pane), "Trips");
        pane.left(&lib);
        assert_eq!(at(&pane), "Personal");
    }

    #[test]
    fn the_pane_with_no_rows_has_no_cursor_to_move() {
        let mut pane = LibraryPane::default();
        pane.step(true);
        pane.step(false);
        pane.go(Go::End);
        pane.go(Go::Page(5));
        pane.jump('a');
        pane.set_cur(0);
        assert_eq!(pane.cur, None);
        // A cursor left over from rows that are gone moves nowhere, and does not panic.
        pane.cur = Some(3);
        pane.go(Go::Page(5));
        pane.go(Go::Page(-5));
        assert_eq!(pane.cur, Some(3));
    }
}
