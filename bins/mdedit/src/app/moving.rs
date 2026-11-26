//! "Move the notes…" as the window hosts it (`move_ui.rs` is the card, `move_draw.rs` draws it,
//! `mdedit_core::moves` is the machine). The card is drawn over everything, as the split's plan
//! card is, and takes every key, pointer and wheel event while it is open; the plan card's routes
//! in `app/plan.rs` hand them here.
//!
//! The move itself is the core's: `preflight` refuses before the first file moves, `write` moves
//! them by a rename that never replaces, and every library file it rewrites goes through the
//! window's own gate ([`App::library_attempt_at`]): a file open with unsaved changes is refused,
//! the file is read again, the new text is checked and written at once. The record it writes is
//! listed in "AI runs…", whose undo is the core's `undo` through the same gate.

use super::library_write::Wrote;
use super::*;
use crate::commands::strings;
use crate::move_ui::{Act as MAct, MoveCard};
use crate::replace_ui::Outcome;
use mdedit_core::moves::{self, Disk, Edit, Expected, MoveGate};
use mdedit_core::runs::{self, Record};
use mdshared_app::paths::same_path;

/// The window's half of [`MoveGate`]: the library writer every other command uses.
pub(super) struct MovesGate<'a> {
    pub(super) app: &'a mut App,
}

impl MoveGate for MovesGate<'_> {
    fn free(&mut self, file: &Path) -> Result<(), String> {
        self.app.library_file_free(file)
    }

    fn edit(
        &mut self,
        file: &Path,
        base: &Path,
        edits: &[Edit],
        expect: &Expected,
    ) -> Result<(), String> {
        let mut why: Option<String> = None;
        let wrote = self.app.library_attempt_at(
            file,
            base,
            |text| match moves::apply_edits(text, edits, expect, base) {
                Ok(new) => Some(new),
                Err(e) => {
                    why = Some(e);
                    None
                }
            },
            // `apply_edits` has read the new text back against what it must say.
            |_, _| true,
        );
        match wrote {
            Wrote::Done => Ok(()),
            Wrote::Nothing => Err(why.unwrap_or_else(|| "the file was not changed".to_string())),
            Wrote::Refused(why) => Err(why),
        }
    }

    /// A moved note whose links change (D-091): refused while a tab holds unsaved changes to
    /// it, then written whole, only while it says what the move read.
    fn rewrite(&mut self, file: &Path, before: &str, after: &str) -> Result<(), String> {
        self.app.library_file_free(file)?;
        moves::rewrite_note(file, before, after)
    }
}

/// A rename that never replaces a file at `to`: the one "Rename file" uses (`MoveFileExW`
/// without `MOVEFILE_REPLACE_EXISTING` on Windows, a hard link and an unlink elsewhere), with a
/// move across volumes said as `CrossesDevices`, which the core answers with a copy.
fn rename_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    crate::docs::move_no_replace(from, to).map_err(|e| {
        // ERROR_NOT_SAME_DEVICE: MoveFileExW without MOVEFILE_COPY_ALLOWED does not cross.
        if cfg!(windows) && e.raw_os_error() == Some(17) {
            return std::io::Error::new(std::io::ErrorKind::CrossesDevices, e);
        }
        e
    })
}

/// The real disk, with the rename that asks the system not to replace.
pub(super) fn disk() -> Disk {
    Disk {
        rename: rename_no_replace,
    }
}

impl App {
    pub(super) fn move_is_open(&self) -> bool {
        self.mover.is_some()
    }

    /// Opens `card` over the window.
    pub(super) fn move_open(&mut self, mut card: MoveCard) {
        if self.palette_open() {
            self.close_palette();
        }
        self.ui.popup = None;
        self.ui.hide_tip();
        self.ui.hover = None;
        self.ui.find_focused = false;
        let (w, h) = self.logical();
        card.resize(w, h);
        self.mover = Some(card);
        self.redraw();
    }

    /// "Move the notes…" on a group's row: the plan for the group whose library file is `file`.
    /// A plan with nothing to move says why in a toast and opens nothing.
    pub(super) fn move_open_group(&mut self, file: &Path) {
        let Some(library) = self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
        else {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        };
        self.library_load();
        let Some(group) = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| same_path(&g.file, file))
            .cloned()
        else {
            return;
        };
        let mut plan = moves::plan_group(&library, &group);
        // A note takes the pictures only it links to, and its other links are rewritten (D-091).
        moves::carry(&mut plan);
        if plan.rows.is_empty() {
            let why = plan.blocked.clone().unwrap_or_else(|| {
                format!(
                    "Every note of {} is already where it belongs: nothing to move.",
                    group.name
                )
            });
            self.docs.set_toast(&why);
            self.redraw();
            return;
        }
        self.move_open(MoveCard::new(plan));
    }

    pub(super) fn move_close(&mut self) {
        self.mover = None;
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    fn move_act(&mut self, act: MAct) {
        match act {
            MAct::Close => self.move_close(),
            MAct::Move => self.move_write(),
            MAct::Nothing => {}
        }
        self.redraw();
    }

    pub(super) fn move_key(&mut self, key: crate::keys::Key) {
        let mods = self.mods;
        let Some(card) = self.mover.as_mut() else {
            return;
        };
        let act = card.key(&key, mods);
        self.move_act(act);
    }

    pub(super) fn move_pointer(&mut self, x: f32, y: f32) {
        if let Some(card) = self.mover.as_mut() {
            card.pointer(x, y);
        }
        self.redraw();
    }

    pub(super) fn move_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(card) = self.mover.as_mut() {
            card.press(x, y);
        }
        self.redraw();
    }

    pub(super) fn move_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(card) = self.mover.as_mut() else {
            return;
        };
        let act = card.release(x, y);
        self.move_act(act);
    }

    pub(super) fn move_wheel(&mut self, dy: f32) {
        if let Some(card) = self.mover.as_mut() {
            card.wheel(dy);
        }
        self.redraw();
    }

    /// Move: everything that can be refused is refused first, and the card stays open with the
    /// reason; then the files move, the card closes, the library is read again and the report
    /// takes the results panel.
    pub(super) fn move_write(&mut self) {
        let Some(mut card) = self.mover.take() else {
            return;
        };
        if card.demo {
            card.refusal = Some("A demonstration of the move card: it moves nothing.".to_string());
            card.tick += 1;
            self.mover = Some(card);
            return;
        }
        let req = card.plan.request();
        let pre = {
            let mut gate = MovesGate { app: self };
            moves::preflight(&req, &mut gate)
        };
        let report = match pre {
            Err(why) => Err(format!("The move could not start: {why}")),
            Ok(()) => {
                let mut gate = MovesGate { app: self };
                let report = moves::write(&req, &mut gate, &mut disk());
                match report.start_error.clone() {
                    Some(why) => Err(why),
                    None => Ok(report),
                }
            }
        };
        let report = match report {
            Ok(report) => report,
            Err(why) => {
                // Nothing moved: the card stays, with every tick, and the reason.
                self.docs.set_toast(&why);
                card.refusal = Some(why);
                card.tick += 1;
                self.mover = Some(card);
                return;
            }
        };
        let kept: Vec<(PathBuf, String)> = card
            .plan
            .rows
            .iter()
            .filter(|r| !r.ticked)
            .map(|r| {
                (
                    r.from.clone(),
                    if r.verdict.blocked() {
                        r.verdict.text()
                    } else {
                        "not ticked".to_string()
                    },
                )
            })
            .collect();
        self.follow_moves(&report.moved);
        self.follow_moves(&report.carried);
        self.library_rows();
        let bases = crate::move_ui::bases(&card.plan);
        self.show_outcome(Outcome::moved(&report, &bases, &kept));
        self.redraw();
    }

    /// Tabs open on a file that moved follow it, so they do not show it as gone.
    fn follow_moves(&mut self, moved: &[(PathBuf, PathBuf)]) {
        for (from, to) in moved {
            self.docs.follow_move(from, to);
        }
    }

    // ---- undo, from "AI runs…" ----

    /// The question before undoing a move: what goes back and what stays.
    pub(super) fn move_undo_ask(&mut self, path: PathBuf, record: &Record, library: &Path) {
        let plan = moves::prepare(record, library);
        if let Some(why) = &plan.refusal {
            self.docs.set_toast(why);
        }
        self.ui.runs.undo = Some((path, plan.sentence(), plan.acts()));
        self.open_dialog(DialogKind::RunUndo);
    }

    /// The answer: the move run the other way, through the same gate.
    pub(super) fn move_undo(&mut self, path: PathBuf, record: &Record, library: &Path) {
        let report = {
            let mut gate = MovesGate { app: self };
            moves::undo(&path, record, library, &mut gate, &mut disk())
        };
        self.ui.dialog = None;
        self.ui.runs.undo = None;
        if let Some(why) = &report.start_error {
            self.docs.set_toast(why);
        }
        self.follow_moves(&report.moved);
        self.follow_moves(&report.carried);
        self.library_rows();
        let from = record.moved.as_ref().and_then(|m| m.library_from.clone());
        let mut bases: Vec<&Path> = vec![&record.group.home];
        if let Some(from) = &from {
            bases.push(from);
        }
        bases.push(library);
        self.show_outcome(Outcome::move_undone(&report, &bases));
        self.redraw();
    }

    /// "Show the notes" on a move: the first note it moved, where it is now.
    pub(super) fn move_show(&mut self, record: &Record) {
        let Some(m) = record.moved.as_ref() else {
            return;
        };
        let undone = record.state == runs::State::Undone;
        let found = m
            .files
            .iter()
            .filter(|f| f.done && f.kind == moves::FileKind::Note)
            .map(|f| if undone { &f.from } else { &f.to })
            .find(|p| p.is_file())
            .cloned();
        let Some(path) = found else {
            self.docs
                .set_toast("None of the files that move carried is there.");
            return;
        };
        self.ui.dialog = None;
        self.open(&path);
        self.redraw();
    }

    /// `--move-demo NAME`: the card over a plan made in the program, or the results panel after
    /// a move. Nothing is read or written: the folders are made up, and the card refuses to move.
    pub(super) fn move_demo(&mut self, name: &str) -> bool {
        if name == "results" {
            let plan = crate::move_ui::demo_plan();
            let library = plan.library.clone();
            let ticked: Vec<&moves::Row> = plan.rows.iter().filter(|r| r.ticked).collect();
            let report = moves::Report {
                record_path: Some(
                    library
                        .join(runs::RUNS_DIR)
                        .join("2026-11-22T09-30-00Z-move.json"),
                ),
                moved: ticked
                    .iter()
                    .map(|r| (r.from.clone(), r.to.clone()))
                    .collect(),
                edited: vec![library.join("Work.md")],
                lines: ticked.len(),
                carried: ticked
                    .iter()
                    .flat_map(|r| r.carry.iter().flat_map(|c| &c.files))
                    .map(|c| (c.from.clone(), c.to.clone()))
                    .collect(),
                rewritten: ticked
                    .iter()
                    .filter(|r| r.carry.as_ref().is_some_and(|c| c.rewrites > 0))
                    .map(|r| r.to.clone())
                    .collect(),
                links: ticked
                    .iter()
                    .filter_map(|r| r.carry.as_ref())
                    .map(|c| c.rewrites)
                    .sum(),
                emptied: plan
                    .group
                    .as_ref()
                    .map(|g| vec![g.home.join("Inbox"), g.home.join("Misc")])
                    .unwrap_or_default(),
                closed: true,
                ..Default::default()
            };
            let kept: Vec<(PathBuf, String)> = plan
                .rows
                .iter()
                .filter(|r| !r.ticked)
                .map(|r| {
                    (
                        r.from.clone(),
                        if r.verdict.blocked() {
                            r.verdict.text()
                        } else {
                            "not ticked".to_string()
                        },
                    )
                })
                .collect();
            let bases = crate::move_ui::bases(&plan);
            self.show_outcome(Outcome::moved(&report, &bases, &kept));
            self.redraw();
            return true;
        }
        match crate::move_ui::demo(name) {
            Some(card) => {
                self.move_open(card);
                true
            }
            None => false,
        }
    }
}
