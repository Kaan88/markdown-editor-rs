//! Compare two files, the window's part: the commands that start and stop it, the keys that go
//! from difference to difference, and the checks each frame makes while it is on. What is
//! compared and how it is drawn is in `compare.rs` and the view.
//!
//! A comparison stands on the split view: the file in the left pane against the file in the right
//! one. It ends with Stop comparing, Escape, a closed tab, or the split ending, and every end
//! gives back what it changed (word wrap and folds come back, each pane's top is a line again).
//! There are no merge arrows: both panes are ordinary editors.

use super::*;
use crate::compare::{Align, Compare, Refusal};
use mdedit_highlight::DocId;

impl App {
    /// Both documents of `a` and `b` are read.
    fn compare_ready(&self, a: DocId, b: DocId) -> bool {
        let ready = |id| {
            self.docs
                .index_of_id(id)
                .is_some_and(|i| self.docs.tabs[i].ready().is_some())
        };
        ready(a) && ready(b)
    }

    /// "Compare with the other pane": the file in the left pane against the one in the right.
    pub(super) fn compare_other(&mut self) {
        if self.compare.is_some() {
            return;
        }
        let (Some(i), Some(o)) = (self.docs.active(), self.docs.other()) else {
            self.docs.set_toast(strings::COMPARE_NEEDS_SPLIT);
            return;
        };
        let (mine, theirs) = (self.docs.tabs[i].id, self.docs.tabs[o].id);
        let (a, b) = if self.docs.other_left {
            (theirs, mine)
        } else {
            (mine, theirs)
        };
        if !self.compare_ready(a, b) {
            self.docs.set_toast(strings::COMPARE_NOT_READ);
            return;
        }
        self.compare_begin(a, b, None);
    }

    /// "Compare with file…": asks for a file and shows it in the right pane.
    pub(super) fn compare_file(&mut self) {
        if self.compare.is_some() || self.docs.active_ready().is_none() {
            return;
        }
        #[cfg(windows)]
        {
            if let Some(path) = platform::pick_files(self.start_dir().as_deref())
                .into_iter()
                .next()
            {
                self.compare_with(&path);
            }
        }
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            platform::pick_files_async(self.start_dir().as_deref(), move |files| {
                wake(Wake::Dialog(Picked::CompareFile(files)));
            });
        }
    }

    /// Opens `path` in the right pane against the active file.
    pub(super) fn compare_with(&mut self, path: &Path) {
        if self.compare.is_some() {
            return;
        }
        let Some(a_index) = self.docs.active() else {
            return;
        };
        let a = self.docs.tabs[a_index].id;
        let k = self.docs.add_pending(path);
        if self.docs.tabs[k].id == a {
            self.docs.set_toast(strings::COMPARE_SAME_FILE);
            return;
        }
        let kid = self.docs.tabs[k].id;
        self.docs.set_other(kid, false);
        if matches!(self.docs.tabs[k].state, crate::docs::DocState::Pending) {
            self.docs.start_load(k);
        }
        self.docs
            .activate(self.docs.index_of_id(a).unwrap_or(a_index));
        // Started once the file is read.
        self.compare_wait = Some((a, self.docs.tabs[k].id));
        self.redraw();
    }

    /// "Compare with saved": the file as it is on disk, in a read-only tab at the left, against
    /// the text as edited.
    pub(super) fn compare_saved(&mut self) {
        if self.compare.is_some() {
            return;
        }
        let Some(doc) = self.docs.active_doc() else {
            return;
        };
        let (Some(path), true) = (doc.path.clone(), doc.modified()) else {
            self.docs.set_toast(strings::COMPARE_NOTHING_SAVED);
            return;
        };
        let (name, language, mine) = (doc.name(), doc.ready().and_then(|r| r.language), doc.id);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.docs
                    .set_toast(&strings::compare_unreadable(&name, &e.to_string()));
                return;
            }
        };
        let k = self
            .docs
            .open_copy(strings::saved_copy_name(&name), &bytes, language);
        let id = self.docs.tabs[k].id;
        self.docs.set_other(id, true);
        self.compare_begin(id, mine, Some(id));
    }

    /// Puts both documents in compare mode.
    pub(super) fn compare_begin(&mut self, a: DocId, b: DocId, saved: Option<DocId>) {
        for (id, right) in [(a, false), (b, true)] {
            if let Some(i) = self.docs.index_of_id(id)
                && let Some(r) = self.docs.tabs[i].ready_mut()
            {
                r.view.align = Some(Align::pending(right));
                r.view.top_row = 0;
                r.view.left = 0.0;
            }
        }
        let mut c = Compare::new(a, b, saved);
        // Where the panes are now is where they stay until one moves.
        let top = |id| {
            self.docs
                .index_of_id(id)
                .and_then(|i| self.docs.tabs[i].ready())
                .map_or(0.0, |r| r.view.top)
        };
        c.synced = (top(a), top(b));
        self.compare = Some(c);
        self.ui.find_focused = false;
        self.redraw();
    }

    /// Ends the comparison and gives both documents back as they were.
    pub(super) fn compare_stop(&mut self) {
        self.compare_wait = None;
        let Some(mut c) = self.compare.take() else {
            return;
        };
        c.release(&mut self.docs);
        if let Some(id) = c.saved
            && let Some(i) = self.docs.index_of_id(id)
        {
            // The read-only copy goes with the comparison, and the split with it.
            if self.docs.other == Some(id) {
                self.docs.other = None;
            }
            self.docs.close(i, self.service.as_ref());
        }
        self.docs.sync_wrap();
        self.redraw();
    }

    /// F7 and Shift+F7: the caret goes to the start of the next or the previous difference.
    pub(super) fn compare_jump(&mut self, forward: bool) {
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let Some(al) = r.view.align.clone() else {
            return;
        };
        let lines = r.buffer.len_lines();
        let row = al.row_of(r.buffer.line_of(r.view.sel.caret));
        let to = if forward {
            al.next_change(row)
        } else {
            al.prev_change(row)
        };
        let Some(to) = to else {
            let text = if forward {
                strings::NO_NEXT_DIFFERENCE
            } else {
                strings::NO_PREVIOUS_DIFFERENCE
            };
            self.docs.set_toast(text);
            return;
        };
        let at = r.buffer.line_range(al.line_near(to, lines)).start;
        r.view.sel = mdedit_core::Selection::at(at);
        r.view.extra.clear();
        view::reveal(r, at, true);
        self.caret_reset();
        self.redraw();
    }

    /// "Copy this change to the other side" (`across`) and "Take this change from the other
    /// side": the lines of the difference the caret is in replace the other side's, in the
    /// document that changes, as one undo step. The comparison follows the edit by itself.
    pub(super) fn compare_copy(&mut self, across: bool) {
        let pick = match crate::compare::pick(&self.docs, across) {
            Ok(p) => p,
            Err(why) => {
                let text = match why {
                    Refusal::NotCompared => return,
                    Refusal::Stale => strings::COMPARE_STALE,
                    Refusal::NoChange => strings::NOT_IN_A_DIFFERENCE,
                    Refusal::ReadOnly if across => strings::COMPARE_TARGET_READ_ONLY,
                    Refusal::ReadOnly => strings::TAKE_TARGET_READ_ONLY,
                };
                self.docs.set_toast(text);
                return;
            }
        };
        let Some(eol) = self.docs.tabs[pick.target]
            .ready()
            .map(|r| r.info.eol.dominant())
        else {
            return;
        };
        let Some(from) = self.docs.tabs[pick.source]
            .ready()
            .map(|r| mdedit_core::merge::take_lines(&r.buffer, pick.from.clone(), eol))
        else {
            return;
        };
        let splice = mdedit_core::merge::Splice {
            target: pick.to,
            from,
        };
        self.docs.seal();
        self.docs.edit_in(
            pick.target,
            EditOp::CopyLines(&splice),
            self.service.as_ref(),
        );
        self.caret_reset();
        self.redraw();
    }

    /// Before each frame: starts a comparison that waited for its files, ends one whose files or
    /// split are gone, and keeps the two panes at the same row.
    pub(super) fn compare_sync(&mut self) {
        if let Some((a, b)) = self.compare_wait {
            let gone = |id| self.docs.index_of_id(id).is_none();
            if gone(a) || gone(b) {
                self.compare_wait = None;
            } else if self.compare_ready(a, b) {
                self.compare_wait = None;
                self.compare_begin(a, b, None);
            }
        }
        let Some(c) = &self.compare else {
            return;
        };
        let shown = (
            self.docs.active_doc().map(|d| d.id),
            self.docs.other().map(|i| self.docs.tabs[i].id),
        );
        let alive = self.compare_ready(c.a, c.b)
            && matches!(shown, (Some(x), Some(y)) if (x == c.a && y == c.b) || (x == c.b && y == c.a));
        if !alive {
            self.compare_stop();
            return;
        }
        if let Some(c) = self.compare.as_mut() {
            c.sync_scroll(&mut self.docs);
        }
    }

    /// Runs the comparison's debounce and takes in its answer; when it next needs to run.
    pub(super) fn compare_timer(&mut self, now: Instant) -> Option<Instant> {
        let waker = self.docs.waker();
        let c = self.compare.as_mut()?;
        let (next, changed) = c.step(&mut self.docs, now, &waker);
        if changed {
            self.wants_frame = true;
        }
        next
    }

    /// The comparison of `--compare`, `--compare-saved` and `--next-difference` for a shot: made
    /// and waited for, so the picture shows the result.
    pub(super) fn shot_compare(&mut self) {
        let a = self.args.clone();
        if a.compare.is_some() {
            // Both files are read: the comparison starts.
            self.compare_sync();
        } else if a.compare_saved {
            self.compare_saved();
        } else if self.docs.other().is_some() && a.next_difference > 0 {
            self.compare_other();
        }
        if self.compare.is_none() {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            self.compare_timer(Instant::now());
            if self.compare.as_ref().is_some_and(Compare::settled) {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        // The panes need a layout before the caret can be brought into view.
        self.compose_layout_only();
        for _ in 0..a.next_difference {
            self.compare_jump(true);
            self.compose_layout_only();
        }
        // The change the caret is in goes across, and the diff is made again.
        if let Some(across) = a.copy_change {
            self.compare_copy(across);
            let deadline = Instant::now() + Duration::from_secs(10);
            while Instant::now() < deadline {
                self.compare_timer(Instant::now());
                if self.compare.as_ref().is_some_and(Compare::settled) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            self.compose_layout_only();
        }
        // The other pane follows the active one.
        self.compare_sync();
        self.compose_layout_only();
    }
}
