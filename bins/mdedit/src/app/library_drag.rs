//! Dragging a note in the notes library (D-130): a note row dragged onto a group or a section
//! is moved there, by the same write the pane's "Move to…" makes (`library_move`), so a library
//! file open with unsaved changes refuses it and no note file is copied, moved or deleted. The
//! drag itself is [`Drag::LibraryNote`]: it starts on a press on a note and becomes a move after
//! a few px, a plain click still opening the note, `Esc` abandons it, and a hold at the pane's
//! edge scrolls the list.
//!
//! Only notes are dragged. The redesign's design of record (`docs/archive/library-redesign.md`,
//! 3.4) says "notes only, never sections", and a group row is a library file whose order is its
//! file name; what dragging those should do is the owner's to say.

use super::*;
use crate::keys::KeyName;
use crate::library_pane::{self, Kind, RowId};
use crate::ui::{Drag, PaneFace};
use std::time::Instant;

impl App {
    /// Whether row `i` of the notes pane is a note, the only row a drag starts on.
    pub(super) fn library_note_row(&self, i: usize) -> bool {
        self.ui
            .library
            .rows
            .get(i)
            .is_some_and(|r| matches!(r.kind, Kind::Note { .. }))
    }

    /// A press on note row `i` at `(x, y)`: the drag waits to move, and the pane holds the note
    /// by what it is, so the release acts on this note whatever the rows become meanwhile.
    pub(super) fn library_drag_press(&mut self, i: usize, x: f32, y: f32) {
        self.ui.library.hold(i);
        self.ui.drag = Some(Drag::LibraryNote {
            row: i,
            x,
            y,
            moving: false,
        });
    }

    /// The pointer moved while a note is held: the heading it would go into is marked, and a
    /// pointer at the pane's edge starts the scroll.
    pub(super) fn library_drag(&mut self, x: f32, y: f32) {
        let area = self.areas.files.map(library_pane::list);
        self.ui.library.drop = area.and_then(|a| self.ui.library.drop_row(x, y, a));
        self.library_drag_arm(x, y);
        self.set_cursor(CursorIcon::Grabbing);
        self.redraw();
    }

    /// While a note is held at the top or the bottom of the pane, a scroll step a tick, so a
    /// note can go to a heading that was out of sight when the drag began. Only over the pane:
    /// a pointer carried over the editor at that height scrolls nothing.
    fn library_drag_arm(&mut self, x: f32, y: f32) {
        let Some(area) = self.areas.files.map(library_pane::list) else {
            return;
        };
        let over = x >= area.x && x < area.right();
        let near = y < area.y + 12.0 || y > area.bottom() - 12.0;
        self.ui.autoscroll_due = (over && near).then(|| Instant::now() + AUTOSCROLL_TICK);
    }

    /// One step of that scroll, and the pointer's place looked at again.
    pub(super) fn library_drag_scroll(&mut self) {
        let Some(area) = self.areas.files.map(library_pane::list) else {
            return;
        };
        let (x, y) = self.ui.mouse;
        let over = x >= area.x && x < area.right();
        let step = if !over {
            0.0
        } else if y < area.y + 12.0 {
            -mdshared_ui::pane::ROW
        } else if y > area.bottom() - 12.0 {
            mdshared_ui::pane::ROW
        } else {
            0.0
        };
        if step != 0.0 {
            self.ui.library.scroll_by(step);
            self.ui.library.drop = self.ui.library.drop_row(x, y, area);
            self.redraw();
        }
        self.library_drag_arm(x, y);
    }

    /// A key while a note is held and moving: `Esc` puts it back where it was, and every other
    /// key is kept from the pane and the text, so nothing rebuilds the rows or acts on a row
    /// mid-drag (audit 4, C2). True when the key was taken here.
    pub(super) fn library_drag_key(&mut self, name: Option<&KeyName>) -> bool {
        if !matches!(self.ui.drag, Some(Drag::LibraryNote { moving: true, .. })) {
            return false;
        }
        if name == Some(&KeyName::Escape) {
            self.ui.drag = None;
            self.ui.library.let_go();
            self.ui.autoscroll_due = None;
            self.set_cursor(CursorIcon::Default);
            self.redraw();
        }
        true
    }

    /// The button came up on a press that held a note: without a move it is the click a note row
    /// always was, with one it is a drop. Either acts on the note pressed, found by what it is,
    /// and on nothing when that note is no longer in the rows.
    pub(super) fn library_note_release(&mut self, moving: bool) {
        let held = self.ui.library.held_at;
        if moving {
            self.library_drag_release();
        } else {
            self.set_cursor(CursorIcon::Default);
            if let Some(row) = held {
                self.activate(Hit::LibraryRow(row));
            }
        }
        self.ui.library.let_go();
    }

    /// The button came up: the note goes under the heading it was dropped on. A pointer that is
    /// not on a row, or on the note's own heading, moves nothing and says nothing.
    fn library_drag_release(&mut self) {
        self.ui.autoscroll_due = None;
        self.set_cursor(CursorIcon::Default);
        let Some(area) = self.areas.files.map(library_pane::list) else {
            return;
        };
        let (x, y) = self.ui.mouse;
        let target = self.ui.library.drop_row(x, y, area);
        self.ui.library.drop = None;
        let (Some(target), Some(source)) = (target, self.ui.library.held_at) else {
            return;
        };
        let Some((file, section)) = self.ui.library.row_place(target) else {
            return;
        };
        let Some(row) = self.ui.library.rows.get(source).cloned() else {
            return;
        };
        let Kind::Note { path, line, .. } = row.kind else {
            return;
        };
        let Some(from) = self.ui.library.group_file(source) else {
            return;
        };
        if self.library_move(&from, line, &row.label, &path, &file, &section) {
            // Where it went, in sight, its cursor row following it when the pane has the
            // keyboard: the note may have left the part of the tree that was on screen.
            self.ui.library.unfold_to(&file, &section);
            self.library_rows();
            self.ui
                .library
                .follow(RowId::Note(library_pane::key_of(&file, &section), path));
            self.redraw();
        }
    }

    /// `--library-demo drag`: the notes pane with a note held over another heading, for a shot;
    /// `ai-menu`: a section's menu open, the AI task at its head (pictures plan 4.2). The library
    /// is made up; nothing is read or written.
    pub(super) fn library_demo(&mut self, name: &str) -> bool {
        if name != "drag" && name != "ai-menu" {
            return false;
        }
        use mdshared_app::files::FileStamp;
        use mdshared_app::library::{Entry, Group, Library};
        let dir = PathBuf::from("D:/notes/library");
        let entry = |title: &str, sections: &[&str], path: &str| Entry {
            title: title.to_string(),
            path: dir.join(path),
            sections: sections.iter().map(|s| (*s).to_string()).collect(),
            line: 1,
            missing: false,
        };
        let groups = vec![
            Group {
                name: "Japan 2025".to_string(),
                home: Some("Japan 2025".into()),
                file: dir.join("japan.md"),
                stamp: FileStamp::Missing,
                sections: vec![
                    vec!["Tokyo".to_string()],
                    vec!["Tokyo".to_string(), "Food".to_string()],
                ],
                entries: vec![
                    entry(
                        "Asakusa and the river",
                        &["Tokyo"],
                        "Japan 2025/Tokyo/asakusa.md",
                    ),
                    entry(
                        "Ramen",
                        &["Tokyo", "Food"],
                        "Japan 2025/Tokyo/Food/ramen.md",
                    ),
                    entry("Packing list", &[], "japan-packing.md"),
                ],
            },
            Group {
                name: "Personal".to_string(),
                home: Some("Personal".into()),
                file: dir.join("personal.md"),
                stamp: FileStamp::Missing,
                sections: vec![
                    vec!["Money".to_string()],
                    vec!["Money".to_string(), "2025".to_string()],
                ],
                entries: vec![
                    entry("Inbox", &[], "inbox.md"),
                    entry("Taxes", &["Money"], "Personal/Money/taxes.md"),
                ],
            },
        ];
        self.docs.hold_library(Library {
            dir,
            groups,
            omitted: 0,
        });
        self.ui.panes.files = true;
        self.ui.set_pane_face(PaneFace::Notes);
        self.ui.library.checked = Some(Instant::now());
        self.library_build();
        self.ui.library.focused = true;
        self.compose();
        if name == "ai-menu" {
            let Some(tokyo) = self.ui.library.rows.iter().position(|r| r.label == "Tokyo") else {
                return true;
            };
            self.ui.library.set_cur(tokyo);
            let (x, y) = self
                .hit_rect(Hit::LibraryRow(tokyo))
                .map_or((60.0, 140.0), |r| (r.x + 70.0, r.cy()));
            self.ui.mouse = (x, y);
            self.library_row_menu(tokyo, x, y, false);
            return true;
        }
        let at = |label: &str| self.ui.library.rows.iter().position(|r| r.label == label);
        let (Some(from), Some(to)) = (at("Asakusa and the river"), at("Food")) else {
            return true;
        };
        self.ui.library.set_cur(from);
        self.ui.library.hold(from);
        self.ui.library.drop = Some(to);
        if let Some(row) = self.hit_rect(Hit::LibraryRow(to)) {
            self.ui.mouse = (row.x + 96.0, row.cy());
        }
        self.ui.drag = Some(Drag::LibraryNote {
            row: from,
            x: self.ui.mouse.0,
            y: self.ui.mouse.1 - 40.0,
            moving: true,
        });
        true
    }
}
