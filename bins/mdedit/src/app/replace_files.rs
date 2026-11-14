//! Replace in files, the window's part: Replace all… plans on a worker, the confirmation dialog
//! asks, Replace writes on a worker, the panel shows the report, and Undo replace puts the files
//! back. The steps and their state are in `replace_ui.rs`; the core is
//! `mdedit_core::replace_files`. Tabs whose files were changed are brought up to date by the
//! reload that follows a change on disk (`Docs::check_changed`): a tab without unsaved changes
//! reads the file again, one with unsaved changes was left out of the replacement and shows the
//! "changed on disk" banner if its file changed meanwhile.

use super::*;
use crate::replace_ui::{Confirm, Finished, Outcome};
use crate::results::State;
use crate::ui::DialogKind;

impl App {
    /// The files of the tabs with unsaved changes: what the file on disk holds is not what the
    /// user sees there, so no replacement touches them.
    fn unsaved_paths(&self) -> Vec<PathBuf> {
        self.docs
            .tabs
            .iter()
            .filter(|d| d.modified())
            .filter_map(|d| d.path.clone())
            .collect()
    }

    /// "Replace all…" in the results panel: plans the replacement on a worker; the confirmation
    /// opens when the plan is made.
    pub(super) fn replace_all_in_files(&mut self) {
        let res = &self.ui.results;
        if res.replace.busy() {
            self.docs.set_toast(strings::REPLACE_BUSY);
            return;
        }
        let done = matches!(res.state, State::Done { .. }) && res.files > 0;
        if !done || res.replace.running() || res.replace.outcome.is_some() {
            return;
        }
        let (skip, wake) = (self.unsaved_paths(), self.docs.waker());
        self.ui.results.replace.start_plan(wake, skip);
        self.ui.find_focused = false;
    }

    /// Takes in what a worker of the replacement finished.
    pub(super) fn replace_poll(&mut self) {
        let root = self
            .ui
            .results
            .replace
            .searched
            .as_ref()
            .map(|s| s.root.clone())
            .unwrap_or_default();
        let Some(done) = self.ui.results.replace.finished() else {
            return;
        };
        match done {
            Finished::Plan(plan) => {
                if plan.cancelled {
                    return;
                }
                if plan.files.is_empty() {
                    self.docs.set_toast(strings::REPLACE_NOTHING);
                } else if self.ui.dialog.is_some() || self.palette_open() {
                    self.docs.set_toast(strings::REPLACE_BLOCKED);
                } else {
                    self.ui.results.replace.confirm = Some(Confirm::new(plan, &root));
                    self.open_dialog(DialogKind::ReplaceFiles);
                }
            }
            Finished::Applied(report) => {
                // The matches are out of date: the report takes the panel's place.
                self.ui.results.rows.clear();
                self.show_outcome(Outcome::applied(report, &root));
            }
            Finished::Reverted(done) => self.show_outcome(Outcome::reverted(&done, &root)),
        }
    }

    /// Shows a report in the panel and brings the open tabs up to date with the files.
    fn show_outcome(&mut self, outcome: Outcome) {
        let res = &mut self.ui.results;
        res.replace.outcome = Some(outcome);
        res.replace.generation += 1;
        res.scroll = 0.0;
        res.selected = None;
        res.generation += 1;
        self.docs.check_changed();
        self.ui.tree.refresh();
    }

    /// Undo replace: puts back the files the last replacement changed.
    pub(super) fn replace_undo(&mut self) {
        if self.ui.results.replace.busy() {
            return;
        }
        let wake = self.docs.waker();
        self.ui.results.replace.start_revert(wake);
    }

    /// The confirmation's buttons: 0 Replace, 1 Cancel, 2 the backup option, 3 the skipped files.
    pub(super) fn replace_dialog_button(&mut self, n: usize) {
        match n {
            0 => self.replace_apply(),
            2 => {
                if let Some(c) = self.ui.results.replace.confirm.as_mut() {
                    c.backup = !c.backup;
                }
            }
            3 => {
                if let Some(c) = self.ui.results.replace.confirm.as_mut() {
                    c.show_skipped = !c.show_skipped;
                }
            }
            _ => self.close_dialog(),
        }
    }

    /// "Replace": writes the ticked files on a worker. Nothing is written before this.
    fn replace_apply(&mut self) {
        let any = self
            .ui
            .results
            .replace
            .confirm
            .as_ref()
            .is_some_and(|c| !c.chosen().is_empty());
        if !any {
            return;
        }
        // The tabs edited since the plan was made are skipped too.
        let (skip, wake) = (self.unsaved_paths(), self.docs.waker());
        self.ui.results.replace.start_apply(wake, skip);
        self.close_dialog();
    }

    /// Keys of the confirmation: Up and Down move through the files, Space ticks, Enter replaces,
    /// Escape cancels.
    pub(super) fn replace_dialog_key(&mut self, name: Option<crate::keys::KeyName>) {
        use crate::keys::KeyName;
        match name {
            Some(KeyName::Enter) => return self.replace_apply(),
            Some(KeyName::Escape) => return self.close_dialog(),
            _ => {}
        }
        let (Some(d), Some(c)) = (
            self.ui.dialog.as_mut(),
            self.ui.results.replace.confirm.as_mut(),
        ) else {
            return;
        };
        let last = c.names.len().saturating_sub(1);
        match name {
            Some(KeyName::Space) => c.toggle(c.cursor),
            Some(KeyName::Up) => c.cursor = c.cursor.saturating_sub(1),
            Some(KeyName::Down) => c.cursor = (c.cursor + 1).min(last),
            Some(KeyName::Home) => c.cursor = 0,
            Some(KeyName::End) => c.cursor = last,
            _ => return,
        }
        d.scroll = c.reveal(d.scroll, d.view);
        self.redraw();
    }
}

impl App {
    /// `--shot`: runs the folder search, and the replacement as far as the flags ask, waiting for
    /// each worker.
    pub(super) fn shot_replace(&mut self) {
        let a = self.args.clone();
        let wait = |app: &mut App, until: fn(&App) -> bool| {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !until(app) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(5));
                app.ui.results.poll();
                app.replace_poll();
            }
        };
        if a.find_in_files {
            self.find_in_files();
            wait(self, |app| {
                !matches!(app.ui.results.state, State::Running { .. })
            });
        }
        if let Some(text) = &a.results_replace {
            self.ui.results.replace.field.set_text(text);
        }
        if a.replace_dialog || a.replace_report {
            self.replace_all_in_files();
            wait(self, |app| !app.ui.results.replace.running());
        }
        if a.replace_report {
            self.replace_apply();
            wait(self, |app| !app.ui.results.replace.running());
            // The open tab reloads from the changed file; the picture waits for it.
            let dark = self.dark();
            let deadline = Instant::now() + Duration::from_secs(10);
            while self.docs.active_doc().is_some_and(|d| d.is_loading())
                && Instant::now() < deadline
            {
                let until = Instant::now() + Duration::from_millis(50);
                self.docs.wait_load(until, self.service.as_ref(), dark);
            }
        }
    }
}
