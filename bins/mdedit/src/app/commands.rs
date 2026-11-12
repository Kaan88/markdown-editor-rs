//! What clicks, menu items and shortcuts do, and the wakes from worker threads and the loop's
//! timers. Adapted from the viewer's `bins/mdv-app/src/app/commands.rs`.

use super::*;
use crate::commands::CommandId as C;
use crate::ui::{Act, BarItem, EditAct, Popup};
use mdedit_core::limits::COPY_LIMIT;

impl App {
    // ---- activation and commands -------------------------------------------------------------

    pub(super) fn activate(&mut self, hit: Hit) {
        match hit {
            Hit::WinMin => {
                if let Some(w) = &self.window {
                    w.set_minimized(true);
                }
            }
            Hit::WinMax => self.toggle_maximized(),
            Hit::WinClose => self.exit = true,
            Hit::NewTab => self.command(C::OpenFiles),
            Hit::TabClose(i) => self.docs.close(i, self.service.as_ref()),
            Hit::Bar(item) => match item {
                BarItem::Open => self.command(C::OpenFiles),
                BarItem::Reload => self.command(C::Reload),
                BarItem::Recent => self.command(C::OpenRecent),
                BarItem::Files => self.command(C::ToggleFiles),
                BarItem::FindPrev => self.command(C::FindPrevious),
                BarItem::FindNext => self.command(C::FindNext),
                BarItem::Theme => self.command(C::Theme),
                BarItem::More => self.open_popup(PopupKind::Main),
            },
            Hit::FindClear => {
                self.ui.find.clear();
                self.docs.find_clear();
                self.ui.find_focused = true;
                self.caret_reset();
            }
            Hit::FilesRow(i) => {
                let Some(row) = self.ui.tree.rows().get(i).cloned() else {
                    return;
                };
                if row.is_dir {
                    self.ui.tree.toggle(i);
                } else {
                    self.open(&row.path);
                }
            }
            Hit::FilesFolder | Hit::FilesOpenFolder => self.command(C::OpenFolder),
            Hit::EmptyOpen => self.command(C::OpenFiles),
            Hit::EmptyRecent(i) => {
                if let Some(p) = self.docs.recents.paths().get(i).cloned() {
                    self.open(&p);
                }
            }
            Hit::DlgButton(n) => {
                if n == 0 {
                    self.close_dialog();
                } else {
                    self.docs.recents.clear();
                }
            }
            Hit::DlgOpen(i) => {
                if let Some(p) = self.docs.recents.paths().get(i).cloned() {
                    self.open(&p);
                    self.close_dialog();
                }
            }
            Hit::DlgRemove(i) => {
                if let Some(p) = self.docs.recents.paths().get(i).cloned() {
                    self.docs.recents.remove(&p);
                }
            }
            _ => {}
        }
        self.redraw();
    }

    /// Opens a file in a tab, or says why not.
    pub(super) fn open(&mut self, path: &Path) {
        if path.is_dir() {
            self.set_folder(path.to_path_buf());
            return;
        }
        if !path.exists() {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into(),
            );
            self.docs
                .set_toast(&format!("{name}: the file is not there"));
            self.docs.recents.remove(path);
            return;
        }
        self.docs.open(path);
        self.caret_reset();
    }

    pub(super) fn act(&mut self, act: Act) {
        match act {
            Act::Cmd(id) => self.command(id),
            Act::TabCmd(id, i) => self.tab_command(id, i),
            Act::Recent(path) => self.open(&path),
            Act::RecentMenu => self.open_popup(PopupKind::Recent),
            Act::Theme(choice) => self.set_theme(choice),
            Act::MatchCase => {
                self.ui.match_case = !self.ui.match_case;
                self.find_changed();
            }
            Act::Edit(edit) => {
                let f = &mut self.ui.find;
                let changed = match edit {
                    EditAct::Cut => {
                        f.has_selection()
                            && platform::set_clipboard_text(f.selected_text())
                            && f.delete_selection()
                    }
                    EditAct::Copy => {
                        if f.has_selection() {
                            platform::set_clipboard_text(f.selected_text());
                        }
                        false
                    }
                    // A search is one line here, and only that much is read.
                    EditAct::Paste => match platform::clipboard_line() {
                        Some(text) => f.insert(&text),
                        None => false,
                    },
                    EditAct::SelectAll => {
                        f.select_all();
                        false
                    }
                };
                self.ui.find_focused = true;
                if changed {
                    self.find_changed();
                }
                self.caret_reset();
            }
        }
        self.redraw();
    }

    /// Copies the selection as it is in the buffer.
    pub(super) fn copy(&mut self) {
        let Some(r) = self.docs.active_ready() else {
            return;
        };
        let range = r.sel.range();
        if range.is_empty() {
            return;
        }
        if range.len() > COPY_LIMIT {
            self.docs.set_toast(strings::COPY_TOO_LARGE);
            return;
        }
        let mut text = String::with_capacity(range.len());
        r.buffer.text(range, &mut text);
        if !platform::set_clipboard_text(&text) {
            self.docs.set_toast("Could not copy to the clipboard");
        }
    }

    pub(super) fn tab_command(&mut self, id: C, index: usize) {
        let Some(path) = self.docs.tabs.get(index).map(|d| d.path.clone()) else {
            return;
        };
        let service = self.service.as_ref();
        match id {
            C::CloseTab => self.docs.close(index, service),
            C::CloseOtherTabs => self.docs.close_others(index, service),
            C::CloseAllTabs => self.docs.close_all(service),
            C::CopyFilePath => {
                if platform::set_clipboard_text(&path.display().to_string()) {
                    self.docs.set_toast("File path copied");
                }
            }
            C::ShowInFileManager => {
                platform::reveal_in_file_manager(&path);
            }
            other => self.command(other),
        }
    }

    /// Where a popup hangs: below its bar button, or below "⋯" when the button is hidden.
    pub(super) fn popup_anchor(&self, kind: PopupKind) -> (f32, f32, bool) {
        let below =
            |r: Rect, right: bool| (if right { r.right() } else { r.x }, r.bottom() + 4.0, right);
        match kind {
            PopupKind::Recent => {
                if let Some(r) = self.hit_rect(Hit::Bar(BarItem::Recent)) {
                    return below(r, false);
                }
                if let Some(r) = self.hit_rect(Hit::Bar(BarItem::More)) {
                    return below(r, true);
                }
            }
            PopupKind::Main => {
                if let Some(r) = self.hit_rect(Hit::Bar(BarItem::More)) {
                    return below(r, true);
                }
            }
            _ => {}
        }
        let (w, _) = self.logical();
        (w - 8.0, self.areas.text.y + 4.0, true)
    }

    pub(super) fn open_popup(&mut self, kind: PopupKind) {
        if self.ui.popup_kind() == Some(kind) {
            self.ui.popup = None;
            return;
        }
        // From the main menu, Recent replaces it in place.
        let (x, y, right) = match (&self.ui.popup, kind) {
            (Some(p), PopupKind::Recent) if p.kind == PopupKind::Main => {
                let r = p.menu.rect();
                (r.right(), r.y, true)
            }
            _ => self.popup_anchor(kind),
        };
        let menu = match kind {
            PopupKind::Main => overlays::main_menu(&self.docs, &self.ui, x, y),
            PopupKind::Recent => overlays::recent_menu(&self.docs, x, y, right),
            _ => return,
        };
        self.ui.popup = Some(Popup { kind, menu });
        self.ui.tip = None;
    }

    pub(super) fn open_dialog(&mut self, kind: DialogKind) {
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.find_focused = false;
        self.ui.dialog = Some(Dialog::new(kind));
        self.ui.dlg_hover_row = None;
    }

    pub(super) fn close_dialog(&mut self) {
        self.ui.dialog = None;
        self.ui.pressed = None;
        self.ui.dlg_hover_row = None;
        self.ui.hover = None;
        self.wants_frame = true;
    }

    pub(super) fn start_dir(&self) -> Option<PathBuf> {
        self.docs
            .active_doc()
            .and_then(|d| d.path.parent().map(Path::to_path_buf))
            .or_else(|| self.docs.folder.clone())
    }

    pub(super) fn set_folder(&mut self, dir: PathBuf) {
        let dir = canonical_or_absolute(&dir);
        self.docs.folder = Some(dir.clone());
        self.ui.tree.set_root(Some(dir));
        self.ui.panes.files_scroll = 0.0;
        self.last_active = None;
        self.ui.panes.files = true;
    }

    fn set_theme(&mut self, choice: ThemeChoice) {
        self.docs.theme = choice;
        self.theme_from_args = false;
    }

    pub(super) fn command(&mut self, id: C) {
        match id {
            C::OpenFiles => {
                for path in platform::pick_files(self.start_dir().as_deref()) {
                    self.open(&path);
                }
            }
            C::OpenFolder => {
                let start = self.docs.folder.clone().or_else(|| self.start_dir());
                if let Some(dir) = platform::pick_folder(start.as_deref()) {
                    self.set_folder(dir);
                }
            }
            C::Reload => {
                self.docs.reload_active();
                self.ui.tree.refresh();
            }
            C::OpenRecent => self.open_popup(PopupKind::Recent),
            C::ManageRecent => self.open_dialog(DialogKind::Recent),
            C::ClearRecent => self.docs.recents.clear(),
            C::CloseTab => {
                if let Some(i) = self.docs.active() {
                    self.docs.close(i, self.service.as_ref());
                }
            }
            C::CloseAllTabs => self.docs.close_all(self.service.as_ref()),
            C::Exit => self.exit = true,
            C::Copy => self.copy(),
            C::SelectAll => {
                if let Some(r) = self.docs.active_ready_mut() {
                    r.sel = mdedit_core::selection::select_all(&r.buffer);
                }
            }
            C::Find => {
                self.ui.find_focused = true;
                self.ui.find.select_all();
                self.caret_reset();
            }
            C::FindNext | C::FindPrevious => {
                let empty = self
                    .docs
                    .active_ready()
                    .is_none_or(|r| r.find.text.is_empty());
                if empty {
                    self.command(C::Find);
                } else {
                    self.docs.find_step(id == C::FindNext);
                }
            }
            C::ClearSearch => {
                self.ui.find.clear();
                self.docs.find_clear();
            }
            C::Theme => {
                let next = match self.docs.theme {
                    ThemeChoice::System => ThemeChoice::Light,
                    ThemeChoice::Light => ThemeChoice::Dark,
                    ThemeChoice::Dark => ThemeChoice::System,
                };
                self.set_theme(next);
            }
            C::ThemeSystem => self.set_theme(ThemeChoice::System),
            C::ThemeLight => self.set_theme(ThemeChoice::Light),
            C::ThemeDark => self.set_theme(ThemeChoice::Dark),
            C::ToggleFiles => self.ui.panes.files = !self.ui.panes.files,
            C::ShowShortcuts => self.open_dialog(DialogKind::Shortcuts),
            C::About => self.open_dialog(DialogKind::About),
            C::NextTab => self.docs.step_tab(true),
            C::PreviousTab => self.docs.step_tab(false),
            C::Escape => self.escape(),
            C::CloseOtherTabs | C::CopyFilePath | C::ShowInFileManager => {
                if let Some(i) = self.docs.active() {
                    self.tab_command(id, i);
                }
            }
        }
        self.caret_reset();
        self.redraw();
    }

    /// Escape peels one layer: a menu, a dialog, the find box's focus, the find, the selection.
    pub(super) fn escape(&mut self) {
        if self.ui.popup.take().is_some() {
            return;
        }
        if self.ui.dialog.is_some() {
            self.close_dialog();
            return;
        }
        if self.ui.find_focused {
            self.ui.find_focused = false;
            return;
        }
        let has_query = self
            .docs
            .active_ready()
            .is_some_and(|r| !r.find.text.is_empty());
        if has_query {
            self.ui.find.clear();
            self.docs.find_clear();
            return;
        }
        if let Some(r) = self.docs.active_ready_mut() {
            r.sel = mdedit_core::Selection::at(r.sel.caret);
        }
    }

    // ---- wakes and timers --------------------------------------------------------------------

    pub(super) fn woken(&mut self, wake: Wake) {
        match wake {
            Wake::Instance => {
                let paths: Vec<PathBuf> = self
                    .incoming
                    .as_ref()
                    .map(|r| r.try_iter().collect())
                    .unwrap_or_default();
                for p in paths {
                    self.open(&p);
                }
                if let Some(w) = &self.window {
                    w.set_minimized(false);
                    w.focus_window();
                }
            }
            Wake::Files => {
                if let Some(w) = &self.watcher {
                    w.woken();
                }
                self.docs.check_changed();
                self.ui.tree.refresh();
            }
            Wake::Loaded | Wake::Found | Wake::Highlight => {
                self.drain();
            }
        }
        self.redraw();
    }

    pub(super) fn dropped(&mut self, path: PathBuf) {
        self.open(&path);
        self.redraw();
    }

    /// Runs whatever timers are due and returns the next deadline.
    pub(super) fn timers(&mut self, now: Instant) -> Option<Instant> {
        let mut next: Option<Instant> = None;
        let mut at = |t: Instant| next = Some(next.map_or(t, |n| n.min(t)));
        // Caret blink, only while the window has focus and the text or the find box has the keys.
        let blinking = self.focused && (self.ui.find_focused || self.text_focused());
        if blinking {
            if let Some(period) = platform::caret_blink_time() {
                let due = *self.ui.caret_due.get_or_insert(now + period);
                if now >= due {
                    self.ui.caret_on = !self.ui.caret_on;
                    self.ui.caret_due = Some(now + period);
                    self.wants_frame = true;
                }
                at(self.ui.caret_due.unwrap_or(now + period));
            }
        } else {
            self.ui.caret_due = None;
        }
        if let Some(tip) = self.ui.tip.as_mut().filter(|t| !t.shown) {
            if now >= tip.due {
                tip.shown = true;
                self.wants_frame = true;
            } else {
                at(tip.due);
            }
        }
        if let Some(until) = self.toast_until {
            if now >= until {
                self.toast_until = None;
                self.wants_frame = true;
            } else {
                at(until);
            }
        } else if let Some(until) = self.docs.toast_expires().filter(|&u| u > now) {
            // A message set since the last frame.
            self.wants_frame = true;
            at(until);
        }
        if let Some(due) = self.ui.autoscroll_due {
            if now >= due {
                self.autoscroll();
                self.ui.autoscroll_due = matches!(self.ui.drag, Some(Drag::Select { .. }))
                    .then(|| now + AUTOSCROLL_TICK);
            }
            if let Some(d) = self.ui.autoscroll_due {
                at(d);
            }
        }
        if let Some(due) = self.size_save_due {
            if now >= due {
                self.size_save_due = None;
                self.save_size();
                self.save_session();
            } else {
                at(due);
            }
        }
        if let Some(due) = self.exit_due {
            if now >= due {
                self.exit_due = None;
                self.exit = true;
            } else {
                at(due);
            }
        }
        next
    }

    pub(super) fn save_size(&mut self) {
        let Some(w) = &self.window else { return };
        if w.is_minimized() == Some(true) {
            return;
        }
        let size = w.inner_size().to_logical::<f64>(w.scale_factor());
        self.session.set_window_size(
            size.width.round() as u32,
            size.height.round() as u32,
            w.is_maximized(),
        );
    }

    /// `--scroll-steps`: one wheel step per frame, down and back at the ends, then a summary and
    /// exit.
    pub(super) fn step(&mut self) {
        let Some(n) = self.steps else { return };
        if n == 0 {
            let own: Vec<f64> = self.step_times.iter().map(FrameTimes::own_ms).collect();
            let (p50, p95, max) = summary(&own);
            let mean = |f: fn(&FrameTimes) -> Duration| {
                self.step_times
                    .iter()
                    .map(|t| crate::trace::ms(f(t)))
                    .sum::<f64>()
                    / self.step_times.len().max(1) as f64
            };
            eprintln!(
                "mdedit: {} scroll steps at {}x{} px, own work: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms; mean layout {:.2} paint {:.2} compose {:.2} copy {:.2}, OS present {:.2}",
                own.len(),
                self.size.0,
                self.size.1,
                p50,
                p95,
                max,
                mean(|t| t.layout),
                mean(|t| t.paint),
                mean(|t| t.compose),
                mean(|t| t.copy),
                mean(|t| t.present),
            );
            self.exit = true;
            return;
        }
        self.steps = Some(n - 1);
        if let Some(r) = self.docs.active_ready_mut() {
            let max = metrics::max_top(r.buffer.len_lines(), r.view_lines);
            if self.step_down && r.top >= max {
                self.step_down = false;
            } else if !self.step_down && r.top <= 0.0 {
                self.step_down = true;
            }
            let d = if self.step_down { 1.0 } else { -1.0 };
            r.top = metrics::clamp_top(
                r.top + d * metrics::WHEEL_LINES,
                r.buffer.len_lines(),
                r.view_lines,
            );
        }
        self.draw();
    }
}
