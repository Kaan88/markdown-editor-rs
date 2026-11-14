//! What clicks, menu items and shortcuts do, and the wakes from worker threads and the loop's
//! timers. Adapted from the viewer's `bins/mdv-app/src/app/commands.rs`.

use super::*;
use crate::commands::CommandId as C;
use crate::docs::{EditOp, SaveFailed};
use crate::ui::{Act, After, BarField, BarItem, EditAct, Pending, Popup};
use crate::updater;
use mdedit_core::find::Query;
use mdedit_core::limits::COPY_LIMIT;
use mdedit_core::lines;
use mdedit_core::save::SaveError;
use mdedit_core::table::TableOp;

/// How long after the last edit the find marks are searched for again.
const REFIND_DELAY: Duration = Duration::from_millis(300);
/// What `--type-steps` types, over and over.
const TYPE_TEXT: &str = "the quick brown fox jumps over the lazy dog ";

/// How a Save as went: written, not done, or (off Windows) asked of a dialog still open.
enum SaveAs {
    #[cfg_attr(not(windows), allow(dead_code))]
    Saved,
    Cancelled,
    #[cfg_attr(windows, allow(dead_code))]
    Waiting,
}

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
            Hit::WinClose => self.request(After::Exit),
            Hit::NewTab => self.command(C::NewFile),
            Hit::TabClose(i) => self.close_tab(i),
            Hit::UpdateGo => self.install_update(),
            Hit::UpdateClose => self.docs.updates.dismiss(),
            Hit::StatusEol => self.open_popup(PopupKind::Eol),
            Hit::StatusEncoding => self.open_popup(PopupKind::Encoding),
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
                self.ui.focus_field(BarField::Find);
                self.caret_reset();
            }
            Hit::ReplaceOne => self.command(C::ReplaceNext),
            Hit::ReplaceAll => self.command(C::ReplaceAll),
            Hit::ReplaceClose => self.close_replace(),
            Hit::InSelection => {
                if let Some(r) = self.docs.active_ready_mut() {
                    r.view.scope_on = !r.view.scope_on;
                    r.fit_current_to_scope();
                }
            }
            Hit::FindOpt(opt) => {
                self.ui.toggle_option(opt);
                self.find_changed();
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
            Hit::OutlineRow(i) => {
                if let Some(line) = self.ui.outline.symbols.get(i).map(|s| s.line) {
                    self.docs.go_to(line + 1, 1);
                    self.ui.find_focused = false;
                    self.caret_reset();
                }
            }
            Hit::OutlineFiles => self.command(C::ToggleFiles),
            Hit::ResultsRow(i) => self.go_to_result(i),
            Hit::ResultsClose => self.ui.results.close(),
            Hit::ResultsReplaceAll => self.replace_all_in_files(),
            Hit::ResultsUndo => self.replace_undo(),
            Hit::ResultsCancel => self.ui.results.replace.cancel(),
            Hit::EmptyOpen => self.command(C::OpenFiles),
            Hit::EmptyRecent(i) => {
                if let Some(p) = self.docs.recents.paths().get(i).cloned() {
                    self.open(&p);
                }
            }
            Hit::DlgButton(n) => match self.ui.dialog.as_ref().map(|d| d.kind) {
                Some(kind) if kind.is_confirm() => self.answer(n),
                Some(DialogKind::ReplaceFiles) => self.replace_dialog_button(n),
                Some(DialogKind::Recent) if n == 1 => self.docs.recents.clear(),
                Some(DialogKind::Macros) if n == 1 => self.macros.delete_all_saved(),
                Some(DialogKind::About) if n == 1 => {
                    // The answer shows in the window (the bar or a toast), so the dialog goes.
                    self.close_dialog();
                    if matches!(self.docs.updates.stage(), updater::Stage::Available(_)) {
                        self.install_update();
                    } else {
                        self.check_for_updates();
                    }
                }
                Some(DialogKind::About) if n == 2 => {
                    self.docs.check_updates = !self.docs.check_updates;
                }
                _ => self.close_dialog(),
            },
            Hit::DlgOpen(i)
                if self.ui.dialog.as_ref().map(|d| d.kind) == Some(DialogKind::ReplaceFiles) =>
            {
                if let Some(c) = self.ui.results.replace.confirm.as_mut() {
                    c.toggle(i);
                    c.cursor = i;
                }
            }
            Hit::DlgOpen(i)
                if self.ui.dialog.as_ref().map(|d| d.kind) == Some(DialogKind::Macros) =>
            {
                if self.macros.use_saved(i) {
                    self.close_dialog();
                    self.play_macro(false);
                }
            }
            Hit::DlgRemove(i)
                if self.ui.dialog.as_ref().map(|d| d.kind) == Some(DialogKind::Macros) =>
            {
                self.macros.delete_saved(i);
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

    /// The find box's text goes to the front of the remembered queries.
    pub(super) fn remember_find(&mut self) {
        let text = self.ui.find.text().to_string();
        self.ui.find_recall.push(&text);
    }

    /// The replace strip closes, and with it the range Replace was limited to.
    pub(super) fn close_replace(&mut self) {
        self.ui.close_replace();
        self.docs.clear_replace_scope();
    }

    /// The find box's text as a query for a search of files or tabs, or a toast saying why
    /// there is none.
    fn search_query(&mut self) -> Option<Query> {
        let text = self.ui.find.text().to_string();
        if text.is_empty() {
            self.command(C::Find);
            self.docs.set_toast(strings::FIND_WHAT);
            return None;
        }
        self.remember_find();
        let query = Query::new(&text, self.ui.options());
        if query.is_none() {
            self.docs.set_toast(strings::BAD_PATTERN);
        }
        query
    }

    /// Looks for the find box's text in the files of the Files pane's folder, or of the active
    /// file's folder when no folder is open, on a worker thread.
    pub(super) fn find_in_files(&mut self) {
        if self.ui.results.replace.busy() {
            self.docs.set_toast(strings::REPLACE_BUSY);
            return;
        }
        let Some(query) = self.search_query() else {
            return;
        };
        let root = self.ui.tree.root().map(Path::to_path_buf).or_else(|| {
            let path = self.docs.active_doc()?.path.as_ref()?;
            path.parent().map(Path::to_path_buf)
        });
        let Some(root) = root else {
            self.docs.set_toast(strings::NO_SEARCH_FOLDER);
            return;
        };
        let (load, waker) = (self.docs.load_options(), self.docs.waker());
        self.ui.results.start_tree(&root, query, load, waker);
    }

    /// Looks for the find box's text in every open tab, in the text as it is now.
    pub(super) fn find_in_tabs(&mut self) {
        if self.ui.results.replace.busy() {
            self.docs.set_toast(strings::REPLACE_BUSY);
            return;
        }
        let Some(query) = self.search_query() else {
            return;
        };
        let cancel = std::sync::atomic::AtomicBool::new(false);
        let mut left_out = false;
        let mut tabs = Vec::new();
        for d in &self.docs.tabs {
            // A tab not read yet, or with more text than is searched while the window waits.
            let Some(r) = d
                .ready()
                .filter(|r| r.buffer.len_bytes() <= crate::results::TAB_BYTES_MAX)
            else {
                left_out = true;
                continue;
            };
            let max = crate::results::TAB_HITS_MAX;
            let (hits, more) = mdedit_core::search::in_buffer(&r.buffer, &query, max, &cancel);
            tabs.push((d.id, d.name(), d.path.clone(), hits, more));
        }
        self.ui.results.set_tabs(query.text(), tabs, left_out);
    }

    /// Opens the file or tab of a row of the results and selects the match there.
    fn go_to_result(&mut self, row: usize) {
        let Some((path, doc, range)) = self.ui.results.target(row) else {
            return;
        };
        let open = doc
            .and_then(|id| self.docs.index_of_id(id))
            .or_else(|| path.as_deref().and_then(|p| self.docs.index_of(p)));
        let i = match (open, path) {
            (Some(i), _) => {
                self.docs.activate(i);
                i
            }
            (None, Some(p)) if p.is_file() => self.docs.open(&p),
            (None, Some(p)) => {
                self.open(&p);
                return;
            }
            (None, None) => {
                self.docs.set_toast(strings::RESULT_TAB_CLOSED);
                return;
            }
        };
        self.ui.results.select(row);
        self.docs.select_in(i, range);
        self.ui.find_focused = false;
        self.caret_reset();
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
            Act::ViewMenu => self.open_popup(PopupKind::View),
            Act::MacroMenu => self.open_popup(PopupKind::Macro),
            Act::LinesMenu | Act::ConvertMenu | Act::MarkdownMenu => {
                if let (Some(kind), Some(sub)) = (self.ui.popup_kind(), act.submenu()) {
                    self.open_sub(kind, sub, false);
                }
            }
            Act::PlaySaved(i) => {
                if self.macros.use_saved(i) {
                    self.play_macro(false);
                }
            }
            Act::Eol(to) => {
                self.docs.seal();
                self.edit(EditOp::LineEndings(to));
            }
            Act::SaveIn(encoding) => {
                self.docs.save_in(encoding);
            }
            Act::ReadAs(encoding) => {
                // Unsaved changes would be lost: `request` asks first.
                if let Some(doc) = self.docs.active_doc().filter(|d| d.path.is_some()) {
                    self.request(After::ReadAs(doc.id, encoding));
                }
            }
            Act::Theme(choice) => self.set_theme(choice),
            Act::Opt(opt) => {
                self.ui.toggle_option(opt);
                self.find_changed();
            }
            Act::Edit(edit) => {
                let f = self.ui.field_mut();
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
                if changed && self.ui.field == BarField::Find {
                    self.find_changed();
                }
                self.caret_reset();
            }
        }
        self.redraw();
    }

    /// Copies the selection as it is in the buffer. True when it is on the clipboard.
    pub(super) fn copy(&mut self) -> bool {
        let Some(r) = self.docs.active_ready() else {
            return false;
        };
        // Several selections go one to a line, in the order they are in the text.
        let (list, _) = r.all_sels();
        let ranges: Vec<_> = list
            .iter()
            .map(|s| s.range())
            .filter(|r| !r.is_empty())
            .collect();
        let eol = r.info.eol.dominant();
        let total: usize = ranges.iter().map(|r| r.len() + eol.len()).sum();
        if ranges.is_empty() {
            return false;
        }
        if total > COPY_LIMIT + eol.len() {
            self.docs.set_toast(strings::COPY_TOO_LARGE);
            return false;
        }
        let mut text = String::with_capacity(total);
        for (k, range) in ranges.into_iter().enumerate() {
            if k > 0 {
                text.push_str(eol);
            }
            for piece in r.buffer.chunks(range) {
                text.push_str(piece);
            }
        }
        if !platform::set_clipboard_text(&text) {
            self.docs.set_toast("Could not copy to the clipboard");
            return false;
        }
        true
    }

    // ---- editing -----------------------------------------------------------------------------

    /// Sets the text size. The first visible line stays where it is: the scroll is in lines.
    pub(super) fn set_zoom(&mut self, zoom: u32) {
        let zoom = zoom.clamp(metrics::ZOOM_MIN, metrics::ZOOM_MAX);
        if zoom != self.docs.zoom {
            self.docs.zoom = zoom;
            self.view.zoom = zoom;
            self.redraw();
        }
    }

    /// Applies an editing command to the active document. True when the text changed.
    pub(super) fn edit(&mut self, op: EditOp) -> bool {
        let changed = self.docs.edit(op, self.service.as_ref());
        if changed && self.docs.find_stale() {
            self.find_due = Some(Instant::now() + REFIND_DELAY);
        }
        self.caret_reset();
        self.redraw();
        changed
    }

    fn paste(&mut self) {
        if let Some(text) = platform::clipboard_text().filter(|t| !t.is_empty()) {
            self.edit(EditOp::Paste(&text));
        }
    }

    // ---- saving and unsaved changes ----------------------------------------------------------

    /// Saves a document. On a file that changed on disk it asks first, and `after` waits for
    /// the answer. True when the file was written.
    pub(super) fn save_doc(&mut self, id: DocId, overwrite: bool, after: After) -> bool {
        let Some(i) = self.docs.index_of_id(id) else {
            return false;
        };
        let name = self.docs.tabs[i].name();
        self.docs.apply_save_options(i, self.service.as_ref());
        match self.docs.save(i, overwrite) {
            Ok(()) => true,
            Err(SaveFailed::Error(SaveError::ChangedOnDisk)) => {
                self.ask(DialogKind::Overwrite, id, after);
                false
            }
            // A new document: ask where to put it. Cancelling the file dialog cancels `after`.
            Err(SaveFailed::Untitled) => match self.save_as(i, after) {
                SaveAs::Saved => true,
                SaveAs::Cancelled => {
                    self.cancel_pending();
                    false
                }
                // The dialog is open; the answer carries `after` on (see `finish_save_as`).
                SaveAs::Waiting => false,
            },
            Err(SaveFailed::Error(e)) => {
                self.docs.set_toast(&format!("{name}: {e}"));
                self.cancel_pending();
                false
            }
            Err(SaveFailed::NotSavable) => {
                self.docs.set_toast(&format!("{name} cannot be saved"));
                self.cancel_pending();
                false
            }
        }
    }

    /// Asks for a file name and writes tab `i` there. Where the dialog blocks (Windows) the answer
    /// is `Saved` or `Cancelled`; elsewhere the dialog runs on a helper thread, the answer is
    /// `Waiting`, and `finish_save_as` carries on with `after` when the result comes back.
    fn save_as(&mut self, i: usize, after: After) -> SaveAs {
        #[cfg(windows)]
        let _ = after;
        let Some(doc) = self.docs.tabs.get(i).filter(|d| d.ready().is_some()) else {
            return SaveAs::Cancelled;
        };
        // A new document is offered its name as a text file, in the folder in use.
        let current = doc.path.clone().unwrap_or_else(|| {
            self.start_dir()
                .unwrap_or_default()
                .join(format!("{}.txt", doc.name()))
        });
        #[cfg(windows)]
        {
            let Some(to) = platform::pick_save(&current) else {
                return SaveAs::Cancelled;
            };
            if self.save_to(i, &to) {
                SaveAs::Saved
            } else {
                SaveAs::Cancelled
            }
        }
        #[cfg(not(windows))]
        {
            if self.dialog_busy {
                return SaveAs::Cancelled;
            }
            let id = doc.id;
            let wake = self.docs.waker();
            self.dialog_busy = true;
            platform::pick_save_async(&current, move |to| {
                wake(Wake::Dialog(Picked::Save { doc: id, to, after }));
            });
            SaveAs::Waiting
        }
    }

    /// Writes tab `i` to `to`; the toast says why when it fails. True when the file was written.
    fn save_to(&mut self, i: usize, to: &Path) -> bool {
        let dark = self.dark();
        self.docs.apply_save_options(i, self.service.as_ref());
        match self.docs.save_as(i, to, self.service.as_ref(), dark) {
            Ok(()) => {
                self.last_active = None;
                self.ui.tree.refresh();
                true
            }
            Err(SaveFailed::Error(e)) => {
                let name = to.file_name().map_or_else(
                    || to.display().to_string(),
                    |n| n.to_string_lossy().into_owned(),
                );
                self.docs.set_toast(&format!("{name}: {e}"));
                false
            }
            Err(SaveFailed::NotSavable | SaveFailed::Untitled) => {
                self.docs.set_toast("This file cannot be saved");
                false
            }
        }
    }

    /// A file dialog was answered (off Windows): does what the blocking call's caller did next.
    #[cfg(not(windows))]
    fn dialog_done(&mut self, picked: Picked) {
        self.dialog_busy = false;
        match picked {
            Picked::Files(paths) => {
                for path in paths {
                    self.open(&path);
                }
            }
            Picked::Folder(dir) => {
                if let Some(dir) = dir {
                    self.set_folder(dir);
                }
            }
            Picked::CompareFile(paths) => {
                if let Some(path) = paths.first() {
                    self.compare_with(path);
                }
            }
            Picked::Save { doc, to, after } => {
                let saved = match (to, self.docs.index_of_id(doc)) {
                    (Some(to), Some(i)) => self.save_to(i, &to),
                    _ => false,
                };
                // Carrying on is `request(after)`, as after the Save button; it does nothing for
                // `After::Nothing`. A cancel, a failure or a closed tab cancels the whole flow.
                if saved {
                    self.request(after);
                } else {
                    self.cancel_pending();
                }
            }
        }
    }

    pub(super) fn close_tab(&mut self, index: usize) {
        if let Some(id) = self.docs.tabs.get(index).map(|d| d.id) {
            self.request(After::CloseTab(id));
        }
    }

    /// Does `after`, once no unsaved changes are in its way: while a document it would lose has
    /// some, that document is shown and the user is asked about it.
    pub(super) fn request(&mut self, after: After) {
        // Closing the window keeps unsaved text for the next start instead of asking.
        if after == After::Exit && self.keep_unsaved() {
            self.perform(after);
            return;
        }
        let in_scope = |id: DocId| match after {
            After::CloseTab(t) | After::Reload(t) | After::ReadAs(t, _) => id == t,
            After::CloseOthers(keep) => id != keep,
            After::CloseAll | After::Exit => true,
            After::Nothing => false,
        };
        let next = self
            .docs
            .modified_ids()
            .into_iter()
            .find(|&id| in_scope(id));
        let Some(id) = next else {
            self.perform(after);
            return;
        };
        if let Some(i) = self.docs.index_of_id(id) {
            self.docs.activate(i);
        }
        let kind = if matches!(after, After::Reload(_) | After::ReadAs(..)) {
            DialogKind::Discard
        } else {
            DialogKind::Unsaved
        };
        self.ask(kind, id, after);
    }

    /// Writes the unsaved text of every tab beside the session, for the next start. False when
    /// there is no session to come back to or a text could not be written: then the user is asked.
    fn keep_unsaved(&mut self) -> bool {
        let Some(dir) = self.backup_dir() else {
            return false;
        };
        match self.docs.write_backups(&dir) {
            Ok(list) => {
                self.session.unsaved = list;
                true
            }
            Err(e) => {
                eprintln!("mdedit: could not keep the unsaved text: {e}");
                false
            }
        }
    }

    /// Opens a question about document `id`; `after` goes on when it is answered.
    pub(super) fn ask(&mut self, kind: DialogKind, id: DocId, after: After) {
        let subject = self
            .docs
            .index_of_id(id)
            .map(|i| self.docs.tabs[i].name())
            .unwrap_or_default();
        self.open_dialog(kind);
        if let Some(d) = self.ui.dialog.as_mut() {
            d.subject = subject;
        }
        self.ui.pending = Some(Pending { doc: id, after });
        self.redraw();
    }

    fn perform(&mut self, after: After) {
        let service = self.service.as_ref();
        let index = |docs: &Docs, id: DocId| docs.index_of_id(id);
        match after {
            After::Nothing => {}
            After::CloseTab(id) => {
                if let Some(i) = index(&self.docs, id) {
                    self.docs.close(i, service);
                }
            }
            After::CloseOthers(id) => {
                if let Some(i) = index(&self.docs, id) {
                    self.docs.close_others(i, service);
                }
            }
            After::CloseAll => self.docs.close_all(service),
            After::Exit => self.exit = true,
            After::Reload(id) | After::ReadAs(id, _) => {
                if let Some(i) = index(&self.docs, id)
                    && !matches!(self.docs.tabs[i].state, DocState::Pending)
                {
                    if let After::ReadAs(_, encoding) = after {
                        self.docs.tabs[i].forced = encoding;
                    }
                    self.docs.start_load(i);
                }
                self.ui.tree.refresh();
            }
        }
        self.redraw();
    }

    /// The answer to the question dialog: button `n`, the first being its default.
    pub(super) fn answer(&mut self, n: usize) {
        let kind = self.ui.dialog.as_ref().map(|d| d.kind);
        let Some(p) = self.ui.pending.take() else {
            self.close_dialog();
            return;
        };
        self.close_dialog();
        match (kind, n) {
            (Some(DialogKind::Unsaved), 0) => {
                if self.save_doc(p.doc, false, p.after) {
                    self.request(p.after);
                }
            }
            (Some(DialogKind::Unsaved), 1) => {
                if let Some(i) = self.docs.index_of_id(p.doc) {
                    self.docs.tabs[i].discard = true;
                }
                self.request(p.after);
            }
            (Some(DialogKind::Overwrite), 0) => {
                if self.save_doc(p.doc, true, p.after) {
                    self.request(p.after);
                }
            }
            (Some(DialogKind::Discard), 0) => self.perform(p.after),
            _ => self.cancel_pending(),
        }
    }

    /// The question was cancelled: nothing closes, and changes given up on the way count again.
    fn cancel_pending(&mut self) {
        self.ui.pending = None;
        for d in &mut self.docs.tabs {
            d.discard = false;
        }
    }

    pub(super) fn tab_command(&mut self, id: C, index: usize) {
        let Some((doc, path)) = self.docs.tabs.get(index).map(|d| (d.id, d.path.clone())) else {
            return;
        };
        match id {
            C::CompareOther
            | C::CompareFile
            | C::CompareSaved
            | C::ShowInOtherPane
            | C::PreviewInViewer
            | C::PreviewSideBySide => {
                self.docs.activate(index);
                self.command(id);
            }
            C::CloseTab => self.request(After::CloseTab(doc)),
            C::CloseOtherTabs => self.request(After::CloseOthers(doc)),
            C::CloseAllTabs => self.request(After::CloseAll),
            C::CopyFilePath => {
                if let Some(path) = path
                    && platform::set_clipboard_text(&path.display().to_string())
                {
                    self.docs.set_toast("File path copied");
                }
            }
            C::ShowInFileManager => {
                if let Some(path) = path {
                    platform::reveal_in_file_manager(&path);
                }
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
            // Above their field: the menu is moved up to fit in the window.
            PopupKind::Eol | PopupKind::Encoding => {
                let hit = if kind == PopupKind::Eol {
                    Hit::StatusEol
                } else {
                    Hit::StatusEncoding
                };
                if let Some(r) = self.hit_rect(hit) {
                    return (r.right(), r.y - 4.0, true);
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
            (Some(p), PopupKind::Recent | PopupKind::View | PopupKind::Macro)
                if p.kind == PopupKind::Main =>
            {
                let r = p.menu.rect();
                (r.right(), r.y, true)
            }
            _ => self.popup_anchor(kind),
        };
        let Some(menu) = self.build_menu(kind, x, y, right) else {
            return;
        };
        self.ui.popup = Some(Popup::new(kind, menu));
        self.ui.tip = None;
    }

    fn build_menu(
        &mut self,
        kind: PopupKind,
        x: f32,
        y: f32,
        right: bool,
    ) -> Option<mdshared_ui::menu::Menu<Act>> {
        Some(match kind {
            PopupKind::Main => overlays::main_menu(&self.docs, x, y),
            PopupKind::Recent => overlays::recent_menu(&self.docs, x, y, right),
            PopupKind::View => overlays::view_menu(&self.docs, &self.ui, x, y, right),
            PopupKind::Macro => {
                let recording = self.macros.recording.is_some();
                let any = !self.macros.last().is_empty();
                let names: Vec<String> =
                    self.macros.saved().iter().map(|n| n.name.clone()).collect();
                overlays::macro_menu(recording, any, &names, x, y, right)
            }
            PopupKind::Lines => overlays::lines_menu(&self.docs, x, y, right),
            PopupKind::Convert => overlays::convert_menu(&self.docs, x, y, right),
            PopupKind::Markdown => overlays::markdown_menu(&self.docs, x, y, right),
            PopupKind::Eol => status::eol_menu(&self.docs, x, y),
            PopupKind::Encoding => status::encoding_menu(&self.docs, x, y),
            _ => return None,
        })
    }

    /// Opens `sub` as a cascade beside the open menu, which stays. With `keys` its first item
    /// is lit, since the keyboard opened it; the pointer lights its own.
    pub(super) fn open_sub(&mut self, _parent: PopupKind, sub: PopupKind, keys: bool) {
        let mouse_y = self.ui.mouse.1;
        let Some(p) = self.ui.popup.as_ref() else {
            return;
        };
        if p.sub.as_ref().is_some_and(|s| s.kind == sub) {
            return;
        }
        let r = p.menu.rect();
        // Level with the item the pointer is on, else with the menu's top.
        let y = if keys || !r.contains(self.ui.mouse.0, mouse_y) {
            r.y
        } else {
            mouse_y - 14.0
        };
        // Beside the parent: on its right when there is room, otherwise on its left.
        let (x, right) = overlays::sub_side(r, self.logical().0);
        let Some(mut menu) = self.build_menu(sub, x, y, right) else {
            return;
        };
        if keys {
            menu.step(true);
        }
        if let Some(p) = self.ui.popup.as_mut() {
            p.sub = Some(Box::new(Popup::new(sub, menu)));
            p.dwell = None;
        }
        self.ui.tip = None;
        self.redraw();
    }

    /// An item of a menu was chosen, by click or key: a menu item opens its submenu and leaves
    /// the cascade up, anything else closes the menus and runs.
    pub(super) fn choose(&mut self, act: Act, keys: bool) {
        if let Some(sub) = act.submenu() {
            if let Some(kind) = self.ui.popup_kind() {
                self.open_sub(kind, sub, keys);
            }
            return;
        }
        self.ui.popup = None;
        self.act(act);
    }

    pub(super) fn open_dialog(&mut self, kind: DialogKind) {
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.find_focused = false;
        self.ui.dialog = Some(Dialog::new(kind));
        self.ui.dlg_hover_row = None;
    }

    pub(super) fn close_dialog(&mut self) {
        // Closed without an answer (Escape, a click beside it): the question is cancelled.
        if self.ui.pending.is_some() {
            self.cancel_pending();
        }
        // A confirmation closed without "Replace": nothing was written, the plan is dropped.
        if self
            .ui
            .dialog
            .as_ref()
            .is_some_and(|d| d.kind == DialogKind::ReplaceFiles)
        {
            self.ui.results.replace.confirm = None;
        }
        self.ui.dialog = None;
        self.ui.pressed = None;
        self.ui.dlg_hover_row = None;
        self.ui.hover = None;
        self.wants_frame = true;
    }

    pub(super) fn start_dir(&self) -> Option<PathBuf> {
        self.docs
            .active_doc()
            .and_then(|d| d.path.as_deref()?.parent().map(Path::to_path_buf))
            .or_else(|| self.docs.folder.clone())
    }

    pub(super) fn set_folder(&mut self, dir: PathBuf) {
        let dir = canonical_or_absolute(&dir);
        self.docs.folder = Some(dir.clone());
        // What was searched, and any replacement's report, belonged to the old folder.
        if !self.ui.results.replace.busy() {
            self.ui.results.replace.clear();
        }
        self.ui.tree.set_root(Some(dir));
        self.ui.panes.files_scroll = 0.0;
        self.last_active = None;
        self.ui.panes.files = true;
        self.ui.outline.on = false;
    }

    pub(super) fn set_theme(&mut self, choice: ThemeChoice) {
        self.docs.theme = choice;
        self.theme_from_args = false;
    }

    pub(super) fn command(&mut self, id: C) {
        // The find box has the keyboard: these are not for the document.
        let edits = matches!(
            id,
            C::Undo
                | C::Redo
                | C::Cut
                | C::Paste
                | C::DuplicateLine
                | C::MoveLineUp
                | C::MoveLineDown
                | C::DeleteLine
                | C::ToggleComment
                | C::UpperCase
                | C::LowerCase
                | C::TrimTrailing
                | C::TabsToSpaces
                | C::SortLines
                | C::SortLinesDescending
                | C::SortLinesIgnoreCase
                | C::ReverseLines
                | C::UniqueLines
                | C::JoinLines
                | C::MdBold
                | C::MdItalic
                | C::MdCode
                | C::MdStrike
                | C::MdHeading
                | C::MdLink
                | C::MdTable
                | C::MdRowBelow
                | C::MdRowAbove
                | C::MdDeleteRow
                | C::MdColumnRight
                | C::MdColumnLeft
                | C::MdDeleteColumn
                | C::MdMoveColumnLeft
                | C::MdMoveColumnRight
        );
        if self.ui.find_focused && edits {
            return;
        }
        // A command ends the run of typing that one Undo takes back.
        if !matches!(id, C::Undo | C::Redo) {
            self.docs.seal();
        }
        match id {
            C::NewFile => {
                self.docs.new_untitled();
                self.ui.find_focused = false;
                self.caret_reset();
            }
            #[cfg(windows)]
            C::OpenFiles => {
                for path in platform::pick_files(self.start_dir().as_deref()) {
                    self.open(&path);
                }
            }
            #[cfg(not(windows))]
            C::OpenFiles => {
                if !self.dialog_busy {
                    self.dialog_busy = true;
                    let wake = self.docs.waker();
                    platform::pick_files_async(self.start_dir().as_deref(), move |files| {
                        wake(Wake::Dialog(Picked::Files(files)));
                    });
                }
            }
            #[cfg(windows)]
            C::OpenFolder => {
                let start = self.docs.folder.clone().or_else(|| self.start_dir());
                if let Some(dir) = platform::pick_folder(start.as_deref()) {
                    self.set_folder(dir);
                }
            }
            #[cfg(not(windows))]
            C::OpenFolder => {
                if !self.dialog_busy {
                    self.dialog_busy = true;
                    let start = self.docs.folder.clone().or_else(|| self.start_dir());
                    let wake = self.docs.waker();
                    platform::pick_folder_async(start.as_deref(), move |dir| {
                        wake(Wake::Dialog(Picked::Folder(dir)));
                    });
                }
            }
            C::Save => {
                if let Some(doc) = self.docs.active_doc().filter(|d| d.ready().is_some()) {
                    let id = doc.id;
                    self.save_doc(id, false, After::Nothing);
                }
            }
            C::SaveAs => {
                if let Some(i) = self.docs.active() {
                    self.save_as(i, After::Nothing);
                }
            }
            C::Reload => match self.docs.active_doc().map(|d| (d.id, d.modified())) {
                // A new document has no file to read again.
                Some(_) if self.docs.active_doc().is_some_and(|d| d.path.is_none()) => {}
                Some((id, true)) => self.request(After::Reload(id)),
                _ => {
                    self.docs.reload_active();
                    self.ui.tree.refresh();
                }
            },
            C::OpenRecent => self.open_popup(PopupKind::Recent),
            C::ManageRecent => self.open_dialog(DialogKind::Recent),
            C::ClearRecent => self.docs.recents.clear(),
            C::CloseTab => {
                if let Some(i) = self.docs.active() {
                    self.close_tab(i);
                }
            }
            C::CloseAllTabs => self.request(After::CloseAll),
            C::Exit => self.request(After::Exit),
            C::Undo => {
                self.edit(EditOp::Undo);
            }
            C::Redo => {
                self.edit(EditOp::Redo);
            }
            C::Cut => {
                if self.copy() {
                    self.edit(EditOp::Insert(""));
                }
            }
            C::Copy => {
                self.copy();
            }
            C::Paste => self.paste(),
            C::DuplicateLine => {
                self.edit(EditOp::DuplicateLines);
            }
            C::MoveLineUp => {
                self.edit(EditOp::MoveLines(false));
            }
            C::MoveLineDown => {
                self.edit(EditOp::MoveLines(true));
            }
            C::DeleteLine => {
                self.edit(EditOp::DeleteLines);
            }
            C::ToggleComment => {
                self.edit(EditOp::ToggleComment);
            }
            C::UpperCase => {
                self.edit(EditOp::Case(true));
            }
            C::LowerCase => {
                self.edit(EditOp::Case(false));
            }
            C::SortLines => {
                self.edit(EditOp::SortLines(lines::SortOrder::Ascending));
            }
            C::SortLinesDescending => {
                self.edit(EditOp::SortLines(lines::SortOrder::Descending));
            }
            C::SortLinesIgnoreCase => {
                self.edit(EditOp::SortLines(lines::SortOrder::IgnoreCase));
            }
            C::ReverseLines => {
                self.edit(EditOp::ReverseLines);
            }
            C::UniqueLines => {
                self.edit(EditOp::UniqueLines);
            }
            C::JoinLines => {
                self.edit(EditOp::JoinLines);
            }
            C::MdBold => {
                self.edit(EditOp::MdWrap("**"));
            }
            C::MdItalic => {
                self.edit(EditOp::MdWrap("*"));
            }
            C::MdCode => {
                self.edit(EditOp::MdWrap("`"));
            }
            C::MdStrike => {
                self.edit(EditOp::MdWrap("~~"));
            }
            C::MdHeading => {
                self.edit(EditOp::MdHeading);
            }
            C::MdLink => {
                self.edit(EditOp::MdLink);
            }
            C::MdTable => {
                self.edit(EditOp::MdTable);
            }
            C::MdRowBelow => {
                self.edit(EditOp::MdTableOp(TableOp::RowBelow));
            }
            C::MdRowAbove => {
                self.edit(EditOp::MdTableOp(TableOp::RowAbove));
            }
            C::MdDeleteRow => {
                self.edit(EditOp::MdTableOp(TableOp::DeleteRow));
            }
            C::MdColumnRight => {
                self.edit(EditOp::MdTableOp(TableOp::ColumnRight));
            }
            C::MdColumnLeft => {
                self.edit(EditOp::MdTableOp(TableOp::ColumnLeft));
            }
            C::MdDeleteColumn => {
                self.edit(EditOp::MdTableOp(TableOp::DeleteColumn));
            }
            C::MdMoveColumnLeft => {
                self.edit(EditOp::MdTableOp(TableOp::MoveColumnLeft));
            }
            C::MdMoveColumnRight => {
                self.edit(EditOp::MdTableOp(TableOp::MoveColumnRight));
            }
            C::TrimTrailing => {
                self.edit(EditOp::TrimTrailing);
            }
            C::TabsToSpaces => {
                self.edit(EditOp::TabsToSpaces);
            }
            C::SelectAll => {
                if let Some(r) = self.docs.active_ready_mut() {
                    r.view.extra.clear();
                    r.view.sel = mdedit_core::selection::select_all(&r.buffer);
                }
            }
            C::Find => {
                self.ui.focus_field(BarField::Find);
                self.ui.find.select_all();
                self.caret_reset();
            }
            C::Replace => {
                // A selection over several lines is what Replace is limited to, until the
                // strip closes.
                self.docs.capture_replace_scope(self.ui.replace_open);
                // With a query already there, the next thing to type is what replaces it.
                self.ui.replace_open = true;
                self.ui.focus_field(if self.ui.find.is_empty() {
                    BarField::Find
                } else {
                    BarField::Replace
                });
                self.ui.field_mut().select_all();
                self.caret_reset();
            }
            C::ReplaceNext | C::ReplaceAll => {
                let with = self.ui.replace.text().to_string();
                self.remember_find();
                self.ui.replace_recall.push(&with);
                if id == C::ReplaceAll {
                    self.edit(EditOp::ReplaceAll(&with));
                } else {
                    // The selected match is replaced, then the next one is selected; with no
                    // match selected this only goes to the next.
                    self.edit(EditOp::ReplaceOne(&with));
                    self.docs.find_step_scoped(true);
                }
            }
            C::FindInFiles => self.find_in_files(),
            C::FindInTabs => self.find_in_tabs(),
            C::NextResult | C::PreviousResult => match self.ui.results.step(id == C::NextResult) {
                Some(row) => self.go_to_result(row),
                None => self.docs.set_toast(strings::NO_RESULTS),
            },
            C::CloseResults => self.ui.results.close(),
            C::FindNext | C::FindPrevious => {
                let empty = self
                    .docs
                    .active_ready()
                    .is_none_or(|r| r.find.text.is_empty());
                if empty {
                    self.command(C::Find);
                } else {
                    self.remember_find();
                    self.docs.find_step_scoped(id == C::FindNext);
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
            // The left pane shows the Files tree or the function list: each command shows its
            // own, and hides the pane when that is what shows.
            C::ToggleFiles | C::ToggleOutline => {
                let outline = id == C::ToggleOutline;
                let shown = self.ui.panes.files && self.ui.outline.on == outline;
                self.ui.panes.files = !shown;
                self.ui.outline.on = outline && !shown;
            }
            C::CommandPalette => {
                if self.palette_open() {
                    self.close_palette();
                } else {
                    self.open_palette("");
                }
            }
            C::CompareOther => self.compare_other(),
            C::CompareFile => self.compare_file(),
            C::CompareSaved => self.compare_saved(),
            C::StopCompare => self.compare_stop(),
            C::NextDifference => self.compare_jump(true),
            C::PreviousDifference => self.compare_jump(false),
            C::GoToLine => {
                if self.docs.active_ready().is_some() {
                    self.ui.naming = false;
                    self.ui.focus_field(BarField::Goto);
                    self.ui.goto.select_all();
                    self.caret_reset();
                }
            }
            C::ZoomIn => self.set_zoom(metrics::zoom_stepped(self.docs.zoom, 1)),
            C::ZoomOut => self.set_zoom(metrics::zoom_stepped(self.docs.zoom, -1)),
            C::ZoomReset => self.set_zoom(100),
            C::ToggleWhitespace => self.docs.whitespace = !self.docs.whitespace,
            C::ToggleWrap => {
                self.docs.wrap = !self.docs.wrap;
                self.docs.sync_wrap();
            }
            C::ToggleMinimap => self.docs.minimap = !self.docs.minimap,
            C::ToggleBookmark => {
                if let Some(r) = self.docs.active_ready_mut() {
                    r.toggle_bookmark();
                }
            }
            C::NextBookmark | C::PreviousBookmark => {
                if !self.docs.go_to_bookmark(id == C::NextBookmark) {
                    self.docs.set_toast(strings::NO_BOOKMARKS);
                }
                self.caret_reset();
            }
            C::ClearBookmarks => {
                if let Some(r) = self.docs.active_ready_mut() {
                    r.bookmarks.clear();
                }
            }
            C::ToggleFold => {
                if let Some(r) = self.docs.active_ready_mut()
                    && !r.toggle_fold()
                {
                    self.docs.set_toast(strings::NO_BLOCK);
                }
                self.caret_reset();
            }
            C::FoldAll => {
                if let Some(r) = self.docs.active_ready_mut() {
                    match r.fold_all() {
                        (0, _) if r.view.folds.is_empty() => {
                            self.docs.set_toast(strings::NO_BLOCKS)
                        }
                        (_, true) => self.docs.set_toast(strings::FOLD_ALL_PART),
                        _ => {}
                    }
                }
                self.caret_reset();
            }
            C::UnfoldAll => {
                if let Some(r) = self.docs.active_ready_mut() {
                    r.unfold_all();
                }
            }
            C::ToggleSplit => {
                // The split is what a comparison stands on.
                self.compare_stop();
                if !self.docs.toggle_split() {
                    self.docs.set_toast(strings::SPLIT_NEEDS_TWO);
                }
            }
            C::ShowInOtherPane => {
                self.compare_stop();
                if self.docs.show_in_other_pane() {
                    self.ui.find_focused = false;
                } else {
                    self.docs.set_toast(strings::SPLIT_NEEDS_TWO);
                }
            }
            C::PreviewInViewer => self.preview(false),
            C::PreviewSideBySide => self.preview(true),
            C::FocusOther => {
                if self.docs.focus_other() {
                    self.ui.find_focused = false;
                    self.caret_reset();
                } else {
                    self.docs.set_toast(strings::NO_OTHER_PANE);
                }
            }
            C::RecordMacro if self.macros.playing => {}
            C::RecordMacro => {
                if self.macros.recording.is_some() {
                    match self.macros.finish() {
                        0 => self.docs.set_toast(strings::MACRO_EMPTY),
                        n => self.docs.set_toast(&strings::macro_recorded(n)),
                    }
                } else {
                    self.macros.recording = Some(Vec::new());
                    self.docs.set_toast(strings::MACRO_RECORDING);
                }
                self.docs.recording = self.macros.recording.is_some();
            }
            C::SaveMacroAs => {
                if self.macros.recording.is_some() {
                    self.docs.set_toast(strings::MACRO_BUSY);
                } else if self.macros.last().is_empty() {
                    self.docs.set_toast(strings::NO_MACRO);
                } else {
                    // The Go to box asks for the name, in the find box's place.
                    self.ui.naming = true;
                    self.ui.focus_field(BarField::Goto);
                    self.ui.goto.clear();
                }
            }
            C::DeleteMacros => self.open_dialog(DialogKind::Macros),
            C::PlayMacro => self.play_macro(false),
            C::PlayMacroToEnd => self.play_macro(true),
            C::ShowShortcuts => self.open_dialog(DialogKind::Shortcuts),
            C::About => self.open_dialog(DialogKind::About),
            C::Settings => self.open_settings(),
            C::NextTab => self.docs.step_tab(true),
            C::PreviousTab => self.docs.step_tab(false),
            C::Escape => self.escape(),
            C::MakeDefault => self.make_default(),
            C::CloseOtherTabs | C::CopyFilePath | C::ShowInFileManager => {
                if let Some(i) = self.docs.active() {
                    self.tab_command(id, i);
                }
            }
        }
        self.caret_reset();
        self.redraw();
    }

    /// Makes mdedit the default application for text files and says how it went (off Windows).
    fn make_default(&mut self) {
        #[cfg(not(windows))]
        {
            use mdshared_ui::desktop_entry::{EDITOR_MIME_TYPES, make_default};
            let text = match make_default("mdedit", EDITOR_MIME_TYPES) {
                Ok(()) => "mdedit is now the default for text files".to_string(),
                Err(e) => format!("Could not set the default: {e}"),
            };
            self.docs.set_toast(&text);
        }
    }

    /// Escape peels one layer: a menu, a dialog, the find box's focus, the find, the selection.
    /// Plays the macro once, or again and again for as long as each run leaves the caret on
    /// a later line than the one before (a macro that ends with Down goes through the file).
    /// A question or a menu that a key opens ends it.
    pub(super) fn play_macro(&mut self, to_end: bool) {
        if self.macros.playing {
            return;
        }
        if self.macros.recording.is_some() {
            self.docs.set_toast(strings::MACRO_BUSY);
            return;
        }
        if self.macros.last().is_empty() {
            self.docs.set_toast(strings::NO_MACRO);
            return;
        }
        let steps = self.macros.steps.clone();
        let mods = self.mods;
        self.macros.playing = true;
        let line = |app: &Self| {
            app.docs
                .active_ready()
                .map(|r| r.buffer.line_of(r.view.sel.caret))
        };
        let mut runs = 0;
        'runs: loop {
            let before = line(self);
            for step in &steps {
                self.mods = step.mods;
                self.key_pressed(step.key.clone(), step.typed.as_deref());
                if self.ui.dialog.is_some() || self.ui.popup.is_some() {
                    break 'runs;
                }
            }
            runs += 1;
            if !to_end || runs >= crate::macros::RUNS_MAX {
                break;
            }
            if line(self).zip(before).is_none_or(|(now, was)| now <= was) {
                break;
            }
            // The empty line after the file's last line break is not one more line to do.
            if self.docs.active_ready().is_some_and(|r| {
                let last = r.buffer.len_lines().saturating_sub(1);
                r.buffer.line_of(r.view.sel.caret) == last && r.buffer.line_range(last).is_empty()
            }) {
                break;
            }
        }
        self.mods = mods;
        self.macros.playing = false;
        if to_end {
            self.docs.set_toast(&strings::macro_ran(runs));
        }
        self.caret_reset();
    }

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
            if self.ui.field == BarField::Goto {
                self.ui.field = BarField::Find;
            }
            return;
        }
        if self.ui.replace_open {
            self.close_replace();
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
        if self.compare.is_some() {
            self.compare_stop();
            return;
        }
        if let Some(r) = self.docs.active_ready_mut() {
            r.view.sel = mdedit_core::Selection::at(r.view.sel.caret);
        }
    }

    // ---- wakes and timers --------------------------------------------------------------------

    pub(super) fn woken(&mut self, wake: Wake) {
        match wake {
            Wake::Instance => {
                let requests: Vec<Request> = self
                    .incoming
                    .as_ref()
                    .map(|r| r.try_iter().collect())
                    .unwrap_or_default();
                self.handed_over(requests);
            }
            Wake::Link => {
                // The links are over; the next frame no longer shows the mark.
                self.docs.links.take_gone();
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
            Wake::Results => {
                self.ui.results.poll();
                self.replace_poll();
            }
            Wake::Outline => {
                self.ui.outline.poll();
            }
            Wake::Compare => {
                self.compare_timer(Instant::now());
            }
            Wake::Palette => {
                if self.palette.poll() {
                    self.palette
                        .sync(&self.docs, &self.ui, &mut self.macros, true);
                    self.palette.tick += 1;
                }
            }
            Wake::Update(updater::Event::Checked { manual, result }) => {
                self.session.update_checked = updater::unix_now();
                if let Some(text) = self.docs.updates.checked(&updater::APP, manual, result) {
                    self.docs.set_toast(&text);
                }
            }
            Wake::Update(updater::Event::Installed(result)) => {
                if let Some(text) = self.docs.updates.finished(&updater::APP, result) {
                    self.docs.set_toast(&text);
                }
            }
            #[cfg(not(windows))]
            Wake::Dialog(picked) => self.dialog_done(picked),
        }
        self.redraw();
    }

    // ---- updates -----------------------------------------------------------------------------

    /// At launch: clears what an earlier update left behind and, when a look for a newer
    /// version is due, takes it. Nothing here runs for a launch without a session or for a copy
    /// that cannot update itself, and all of it is on its own thread.
    pub(super) fn updates_at_launch(&mut self) {
        if self.session_path.is_none() || !self.save {
            return;
        }
        let Some(files) = updater::installed_files() else {
            return;
        };
        let due = updater::check_due(
            self.docs.check_updates,
            true,
            self.session.update_checked,
            updater::unix_now(),
        );
        let wake = self.docs.waker();
        updater::spawn_at_launch(updater::APP, files, due, updater::fetch, move |e| {
            wake(Wake::Update(e))
        });
    }

    /// Looks for a newer version now, because the user asked: the answer always shows.
    pub(super) fn check_for_updates(&mut self) {
        let wake = self.docs.waker();
        updater::spawn_check(updater::APP, true, updater::fetch, move |e| {
            wake(Wake::Update(e))
        });
    }

    /// Downloads and swaps in the version that was offered, on its own thread.
    pub(super) fn install_update(&mut self) {
        let Some(files) = updater::installed_files() else {
            self.docs.set_toast(updater::NOT_INSTALLED);
            return;
        };
        let Some(offer) = self.docs.updates.begin() else {
            return;
        };
        let wake = self.docs.waker();
        updater::spawn_install(updater::APP, offer, files, updater::fetch, move |e| {
            wake(Wake::Update(e))
        });
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
        let blinking =
            self.focused && (self.ui.find_focused || self.text_focused() || self.palette_open());
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
        // The pointer resting on a menu item with a submenu; no menu, no timer.
        if let Some((_, due)) = self.ui.popup.as_ref().and_then(|p| p.dwell) {
            if now >= due {
                let kind = self.ui.popup_kind();
                let sub = self.ui.popup.as_mut().and_then(Popup::rested);
                match sub.zip(kind) {
                    Some((sub, kind)) => self.open_sub(kind, sub, false),
                    // A rest on a plain item closed the submenu.
                    None => self.redraw(),
                }
            } else {
                at(due);
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
                self.ui.autoscroll_due = matches!(
                    self.ui.drag,
                    Some(Drag::Select { .. } | Drag::Column { .. })
                )
                .then(|| now + AUTOSCROLL_TICK);
            }
            if let Some(d) = self.ui.autoscroll_due {
                at(d);
            }
        }
        if self.compare.is_some()
            && let Some(due) = self.compare_timer(now)
        {
            at(due);
        }
        if let Some(due) = self.find_due {
            if now >= due {
                self.find_due = None;
                self.docs.refind();
                self.wants_frame = true;
            } else {
                at(due);
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
        if let Some(due) = self.link_due {
            at(due);
        }
        if let Some(due) = self.run_due {
            if now >= due {
                self.run_due = None;
                if let Some(id) = self.args.run {
                    self.command(id);
                }
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
        // The CONTENT size (the window reports the surface, margin included, with the client-drawn
        // shadow), so the saved size does not grow by the margin on every start.
        let surface = w.inner_size();
        let content = self.content_size(surface.width, surface.height);
        let (lw, lh) = logical_size(content, w.scale_factor());
        self.session.set_window_size(lw, lh, w.is_maximized());
    }

    /// `--type-steps`: one keystroke per frame at the caret, then a summary of the time from
    /// keystroke to pixels (the edit plus the frame's own work) and exit without saving.
    pub(super) fn type_step(&mut self) {
        let Some(n) = self.type_steps else { return };
        if n == 0 {
            let (p50, p95, max) = summary(&self.type_times);
            let count = self.type_times.len().max(1) as f64;
            eprintln!(
                "mdedit: {} keystrokes at {}x{} px, keystroke to pixels: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms; mean edit {:.3} ms",
                self.type_times.len(),
                self.size.0,
                self.size.1,
                p50,
                p95,
                max,
                self.type_edit_ms / count,
            );
            self.exit = true;
            return;
        }
        self.type_steps = Some(n - 1);
        let k = self.type_count;
        self.type_count += 1;
        let letter = &TYPE_TEXT[k % TYPE_TEXT.len()..][..1];
        let op = match k {
            _ if k % 61 == 60 => EditOp::Newline,
            _ if k % 23 == 22 => EditOp::Backspace,
            _ => EditOp::Insert(letter),
        };
        let frames = self.step_times.len();
        let t = Instant::now();
        self.docs.edit(op, self.service.as_ref());
        let edit = crate::trace::ms(t.elapsed());
        self.draw();
        if self.step_times.len() > frames {
            let own = self.step_times.last().map_or(0.0, FrameTimes::own_ms);
            self.type_times.push(edit + own);
            self.type_edit_ms += edit;
        }
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
            let d = if self.step_down { 1.0 } else { -1.0 };
            if r.wrapped() {
                // Turn where a step moves nothing: an end of the document.
                let before = (r.view.top, r.view.top_row);
                view::scroll_rows(r, d * metrics::WHEEL_LINES);
                if (r.view.top, r.view.top_row) == before {
                    self.step_down = !self.step_down;
                }
            } else {
                let max = metrics::max_top(r.buffer.len_lines(), r.view.view_lines);
                if self.step_down && r.view.top >= max {
                    self.step_down = false;
                } else if !self.step_down && r.view.top <= 0.0 {
                    self.step_down = true;
                }
                let d = if self.step_down { 1.0 } else { -1.0 };
                r.view.top = metrics::clamp_top(
                    r.view.top + d * metrics::WHEEL_LINES,
                    r.buffer.len_lines(),
                    r.view.view_lines,
                );
            }
        }
        self.draw();
    }
}
