//! The notes library in the left pane, in the Files tree's place: a group (a library file) holds
//! its sections, a section holds the sections written under it and its notes. Clicking a note
//! opens it; clicking a group or a section folds it. The rows are made from
//! [`mdshared_app::library`], which is read again when a library file has been saved.

use crate::chrome::{Ctx, glyph};
use crate::commands::strings;
use crate::docs::Docs;
use crate::ui::{BarField, Drag, Hit, Scroller, Ui};
use mdshared_app::library::Library;
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::pane::{HEAD, ROW, ROW_H, draw_row_fill};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::textfield::{FieldColors, TextField};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long a library is used before the folder is looked at again, while the pane shows.
pub const RECHECK: Duration = Duration::from_millis(800);

/// What a row stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A library file. Its key is the file.
    Group,
    /// A heading inside one.
    Section,
    /// A note: the file it opens, whether that file is gone, and its line in the library file.
    Note {
        path: PathBuf,
        missing: bool,
        line: usize,
    },
    /// The row a name is being typed in for something that is not there yet.
    Editing,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: Kind,
    pub label: String,
    pub depth: u16,
    /// What folding remembers this row by; empty for a note.
    pub key: String,
    /// The headings a section is under, itself last; empty for the others.
    pub section: Vec<String>,
}

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

#[derive(Default)]
pub struct LibraryPane {
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
                },
            );
        }
    }
}

/// The key a group or a section is remembered by: the library file, then the headings.
fn key_of(file: &std::path::Path, section: &[String]) -> String {
    let mut key = file.to_string_lossy().into_owned();
    for s in section {
        key.push('\u{1}');
        key.push_str(s);
    }
    key
}

fn rows(library: &Library, folded: &HashSet<String>) -> Vec<Row> {
    let mut rows = Vec::new();
    for group in &library.groups {
        let key = key_of(&group.file, &[]);
        let shut = folded.contains(&key);
        rows.push(Row {
            kind: Kind::Group,
            label: group.name.clone(),
            depth: 0,
            key,
            section: Vec::new(),
        });
        if shut {
            continue;
        }
        let note = |e: &mdshared_app::library::Entry, depth: u16| Row {
            kind: Kind::Note {
                path: e.path.clone(),
                missing: e.missing,
                line: e.line,
            },
            label: e.title.clone(),
            depth,
            key: String::new(),
            section: Vec::new(),
        };
        // The notes written before the first heading belong to the group itself.
        rows.extend(
            group
                .entries
                .iter()
                .filter(|e| e.sections.is_empty())
                .map(|e| note(e, 1)),
        );
        for section in &group.sections {
            // A section inside a folded one is not listed at all.
            if (1..section.len()).any(|n| folded.contains(&key_of(&group.file, &section[..n]))) {
                continue;
            }
            let key = key_of(&group.file, section);
            let depth = section.len() as u16;
            rows.push(Row {
                kind: Kind::Section,
                label: section.last().cloned().unwrap_or_default(),
                depth,
                key: key.clone(),
                section: section.clone(),
            });
            if folded.contains(&key) {
                continue;
            }
            rows.extend(
                group
                    .entries
                    .iter()
                    .filter(|e| e.sections == *section)
                    .map(|e| note(e, depth + 1)),
            );
        }
    }
    rows
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

/// The rows below the head.
pub fn list(r: Rect) -> Rect {
    let ry = r.y + HEAD;
    Rect::new(r.x, ry, r.w, (r.bottom() - ry).max(0.0))
}

pub fn draw(c: &mut Ctx, docs: &Docs, ui: &mut Ui, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.pane);
    crate::panes::draw_pane_head(c, ui, r);

    let dragging = matches!(ui.drag, Some(Drag::Splitter { .. }));
    let area = list(r);
    let active = docs.active_doc().and_then(|d| d.path.clone());
    let focused = ui.find_focused && ui.field == BarField::Library;
    let caret_on = ui.caret_on;
    // The row whose menu is up stays lit.
    let menu_row = match ui.popup_kind() {
        Some(crate::ui::PopupKind::LibraryRow(i)) => Some(i),
        _ => None,
    };
    let lib = &mut ui.library;
    lib.content = lib.rows.len() as f32 * ROW + 8.0;
    lib.view = area.h;
    lib.scroll = lib.scroll.clamp(0.0, (lib.content - area.h).max(0.0));
    let scroll = lib.scroll;
    // Before the folder has been read (the pane came back at start) there is nothing to say yet.
    if lib.rows.is_empty() && lib.checked.is_some() {
        let text = if docs.library.dir.as_os_str().is_empty() {
            strings::LIBRARY_NO_FOLDER
        } else {
            strings::LIBRARY_EMPTY_PANE
        };
        let at = Rect::new(r.x + 8.0, area.y + 24.0, r.w - 16.0, 20.0);
        c.p.text_center(Face::Ui, 12.5, at, text, pal.muted);
    }
    let quiet = mix(pal.text, pal.muted, 0.35);
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
            Kind::Note { path, .. } => active
                .as_deref()
                .is_some_and(|a| mdshared_app::paths::same_path(a, path)),
            _ => false,
        };
        draw_row_fill(
            c.p,
            &pal,
            rr,
            c.hover == Some(hit) || menu_row == Some(i),
            c.pressed == Some(hit),
            selected,
        );
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
            let max = (rr.right() - x - 12.0).max(0.0);
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
}
