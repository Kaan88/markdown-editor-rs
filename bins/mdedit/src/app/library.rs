//! The notes library: the `library` folder beside the settings file, listed by the palette
//! ("Notes library"), written to by "Add this file to the notes library" and edited as the
//! Markdown files it is. Reading, the format and the writing are
//! [`mdshared_app::library`], shared with the viewer; what is here is when it is read, where
//! the folder is, and what the window does with it.

use super::*;
use crate::commands::strings;
use crate::keys::{self, KeyName};
use crate::ui::PaneFace;
use mdshared_app::library;

impl App {
    /// The library folder: the one the preference names (a path with no root is taken from the
    /// settings folder), else `library` beside the settings file. `None` while the window has no
    /// session -- a shot, `--no-session` without `MDEDIT_SETTINGS`, or a settings file that could
    /// not be had -- so a build never reads or writes the owner's library (A11).
    pub(super) fn library_dir(&self) -> Option<PathBuf> {
        let beside = self.snippets_path()?.parent()?.to_path_buf();
        let set = self.docs.prefs.library_dir.trim();
        if set.is_empty() {
            return Some(beside.join(library::DIR));
        }
        let set = Path::new(set);
        Some(if set.is_absolute() {
            set.to_path_buf()
        } else {
            beside.join(set)
        })
    }

    /// The preference changed: what was read from the old folder and what the pane held of it
    /// go, and the new folder is read when it is next listed (at once, if the pane shows).
    pub(super) fn library_dir_changed(&mut self) {
        self.library_read_abandon();
        self.docs.forget_library();
        self.ui.library.cursor = None;
        self.ui.library.stop_edit();
        self.ui.drop_library_field_focus();
        self.ui.library.rows.clear();
        self.ui.library.checked = None;
        if self.ui.panes.files && self.ui.library.on {
            self.library_rows();
        }
    }

    /// Settings › Notes library › Choose: a folder picker, then the preference is that folder.
    pub(super) fn library_choose_folder(&mut self) {
        let start = self
            .library_dir()
            .filter(|d| d.is_dir())
            .or_else(|| self.start_dir());
        #[cfg(windows)]
        if let Some(dir) = platform::pick_folder(start.as_deref()) {
            self.library_dir_picked(dir);
        }
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            platform::pick_folder_async(start.as_deref(), move |dir| {
                wake(Wake::Dialog(Picked::LibraryFolder(dir)));
            });
        }
    }

    /// A folder was chosen for the library.
    pub(super) fn library_dir_picked(&mut self, dir: PathBuf) {
        self.set_library_dir(&dir.to_string_lossy());
        self.ai_settings_refresh();
    }

    /// Sets the library folder preference (empty: the default) and starts reading from it.
    pub(super) fn set_library_dir(&mut self, dir: &str) {
        let mut p = self.docs.prefs.clone();
        p.library_dir = dir.to_string();
        p.clamp();
        self.set_prefs(p);
        self.library_dir_changed();
    }

    /// Reads the library when it is listed for the first time, and again whenever a library
    /// file has been saved, added or removed since. Nothing is read at the start: the first
    /// list pays for it.
    pub(super) fn library_load(&mut self) {
        let Some(dir) = self.library_dir() else {
            self.docs.forget_library();
            return;
        };
        if self.docs.library_stamp.dir == dir && !self.docs.library_stamp.changed() {
            return;
        }
        self.docs.hold_library(library::read(&dir));
    }

    /// Reads the library on its own thread; the rows on screen stay as they are until a result
    /// is taken. One read at a time: a caller that finds one running waits for its result.
    fn library_start_read(&mut self) {
        let Some(dir) = self.library_dir() else {
            return;
        };
        let wake = self.docs.waker();
        let started = mdshared_app::job::Job::start(
            move || wake(Wake::Library),
            move |cancel, _progress| {
                // The read itself cannot be cut short, so the flag is looked at before it and
                // after: a read the pane no longer wants is dropped rather than handed back.
                if cancel.cancelled() {
                    return None;
                }
                let read = library::read(&dir);
                (!cancel.cancelled()).then_some(read)
            },
        );
        match started {
            Ok(job) => self.library_job = Some(job),
            Err(_) => {
                // A thread could not be made: read here rather than leave the pane stale.
                self.library_load();
                self.ui.library.stop_edit();
                self.ui.drop_library_field_focus();
                self.library_build();
                self.redraw();
            }
        }
    }

    /// Asks a read still running to stop and forgets it: its result will not be looked at, and
    /// nothing of it reaches the rows.
    pub(super) fn library_read_abandon(&mut self) {
        if let Some(job) = self.library_job.take() {
            job.cancel();
        }
    }

    /// Takes a read that has finished, on every pass of the loop: the wake that says it is done
    /// can arrive after the pane was hidden or pointed at another folder.
    pub(super) fn library_read_take(&mut self) {
        let Some(job) = self.library_job.as_ref() else {
            return;
        };
        let read = job.take();
        if read.is_none() && job.running() {
            return;
        }
        self.library_job = None;
        if let Some(Some(read)) = read {
            self.library_accept(read);
        }
    }

    /// Shows a library a thread read, unless the folder moved under it, or the pane no longer
    /// shows the notes or no longer reads that folder: then nothing is touched and it is false.
    /// The keyboard's cursor follows its note or heading by identity, in `build`.
    pub(super) fn library_accept(&mut self, read: mdshared_app::library::Library) -> bool {
        if read.changed() {
            // A stamp moved while it was read, so the result mixes two states. It is dropped,
            // and the next pass reads again at once rather than wait out the interval.
            self.ui.library.checked = None;
            return false;
        }
        if !(self.ui.panes.files && self.ui.library.on)
            || self.library_dir().as_deref() != Some(read.dir.as_path())
        {
            return false;
        }
        self.docs.hold_library(read);
        // The rows moved under a name being typed: it is dropped rather than put on the wrong
        // row.
        self.ui.library.stop_edit();
        self.ui.drop_library_field_focus();
        self.library_build();
        self.redraw();
        true
    }

    /// The pane's rows made again from the library as it is held now. A row's menu left open
    /// over them is closed when its row is no longer the row it was opened on: the menu's items
    /// act on the row's index, and "Remove from the library" would take another note's line out
    /// with no question (audit 4, A F16). A re-read that leaves the row where it was keeps the
    /// menu.
    pub(super) fn library_build(&mut self) {
        let menu = self.library_menu_row();
        self.ui.library.build(&self.docs.library);
        self.library_menu_check(menu);
    }

    /// The row a row's menu is open on, and what that row is, taken before the rows change:
    /// before a name box goes too, since taking it out moves every row below it (review of the
    /// audit 4 fixes, A F16).
    pub(super) fn library_menu_row(&self) -> Option<(usize, Option<crate::library_pane::RowId>)> {
        match self.ui.popup_kind() {
            Some(crate::ui::PopupKind::LibraryRow(i)) => Some((i, self.ui.library.id_at(i))),
            _ => None,
        }
    }

    /// The rows were made again: a menu whose row is another now closes.
    pub(super) fn library_menu_check(
        &mut self,
        menu: Option<(usize, Option<crate::library_pane::RowId>)>,
    ) {
        if let Some((i, was)) = menu
            && (was.is_none() || self.ui.library.id_at(i) != was)
        {
            self.ui.popup = None;
            self.redraw();
        }
    }

    /// The rows of the pane, with the library read again first. The folder is looked at at
    /// most every [`crate::library_pane::RECHECK`], so a pane left open costs nothing while
    /// nothing is clicked.
    pub(super) fn library_rows(&mut self) {
        self.library_load();
        self.ui.library.checked = Some(Instant::now());
        self.library_build();
    }

    /// A click or Enter on a row: a group or a section folds, a note opens, a note that is not
    /// there says so. The keyboard stays in the pane after a fold, and goes to the text when a
    /// note opened.
    pub(super) fn library_row(&mut self, i: usize) {
        use crate::library_pane::Kind;
        let Some(row) = self.ui.library.rows.get(i).cloned() else {
            return;
        };
        self.ui.library.cursor = self.ui.library.group_file(i);
        self.ui.library.set_cur(i);
        match row.kind {
            Kind::Group | Kind::Section => {
                self.ui.library.focused = true;
                self.ui.library.fold(&row.key);
                self.library_build();
            }
            Kind::Note { missing: true, .. } => {
                self.ui.library.focused = true;
                self.docs.set_toast(strings::LIBRARY_MISSING_FILE);
            }
            Kind::Note { path, .. } => {
                if self.open_note(&path) {
                    self.ui.library.focused = false;
                    self.caret_reset();
                } else {
                    self.ui.library.focused = true;
                }
            }
            Kind::Editing => {}
        }
    }

    /// Opens a note of the library where it belongs: text in a tab; a picture, a PDF or a Word
    /// file with the system's own app, never as text in a tab (the pictures plan, section 14:
    /// a picture opened in the editor is garbage); a program, or bytes of no known kind, only
    /// shown in their folder, never started. True when a tab has it.
    pub(super) fn open_note(&mut self, path: &Path) -> bool {
        match note_opener(path) {
            NoteOpen::Tab => {
                self.open(path);
                true
            }
            NoteOpen::System(ext) => {
                if system::open(path, ext) {
                    self.docs.set_toast(strings::LIBRARY_OPENED_OUTSIDE);
                } else {
                    let name = path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    self.docs
                        .set_toast(&strings::library_not_opened_outside(&name));
                }
                false
            }
            NoteOpen::Folder(why) => {
                system::reveal(path);
                self.docs.set_toast(why);
                false
            }
        }
    }

    /// Gives the keyboard to the pane when it shows the notes and does not have it: the cursor
    /// is where it was, else on the open note, else on the first row. False when the pane is
    /// not showing the notes or already has the keyboard, which leaves the command to close it.
    pub(super) fn library_focus(&mut self) -> bool {
        if !(self.ui.panes.files && self.ui.library.on) || self.ui.library.focused {
            return false;
        }
        if self.ui.library.rows.is_empty() {
            return false;
        }
        self.ui.find_focused = false;
        self.ui.results.set_focus(false);
        if self.ui.library.cur.is_none() {
            let open = self.docs.active_doc().and_then(|d| d.path.clone());
            let at = open
                .and_then(|p| {
                    self.ui.library.rows.iter().position(|r| match &r.kind {
                        crate::library_pane::Kind::Note { path, .. } => {
                            crate::library_pane::same_note(path, &p)
                        }
                        _ => false,
                    })
                })
                .unwrap_or(0);
            self.ui.library.set_cur(at);
        } else if let Some(c) = self.ui.library.cur {
            self.ui.library.set_cur(c);
        }
        self.ui.library.focused = true;
        self.redraw();
        true
    }

    /// Keys for the pane while it has the keyboard. True when the key was used; the shortcuts
    /// (Ctrl or Alt with a key, F3 and the rest) are left to the commands.
    pub(super) fn library_key(&mut self, key: &keys::Key, mods: Mods, typed: Option<&str>) -> bool {
        use crate::library_pane::{Go, Kind};
        use crate::ui::LibAct;
        if mods.ctrl || mods.alt {
            return false;
        }
        let cur = self.ui.library.cur;
        let page = (self.ui.library.view / crate::panes::ROW).floor().max(1.0) as isize;
        let name = match key {
            keys::Key::Name(n) => n.clone(),
            // The menu key asks for the row's menu, as Shift+F10 does.
            keys::Key::Menu => {
                self.library_key_menu();
                return true;
            }
            keys::Key::Insert => return false,
        };
        match name {
            KeyName::Up => self.ui.library.step(false),
            KeyName::Down => self.ui.library.step(true),
            KeyName::Home => self.ui.library.go(Go::Home),
            KeyName::End => self.ui.library.go(Go::End),
            KeyName::PageUp => self.ui.library.go(Go::Page(-page)),
            KeyName::PageDown => self.ui.library.go(Go::Page(page)),
            KeyName::Left => self.ui.library.left(&self.docs.library),
            KeyName::Right => self.ui.library.right(&self.docs.library),
            KeyName::Enter => {
                if let Some(c) = cur {
                    self.library_row(c);
                }
            }
            KeyName::F(2) => {
                if let Some(c) = cur {
                    self.library_act(LibAct::Rename, c);
                }
            }
            KeyName::F(10) if mods.shift => self.library_key_menu(),
            KeyName::Delete if !mods.shift => {
                // Notes and sections come out of the library file; a group is a file of its own,
                // which its menu deletes after asking.
                let removable = cur
                    .and_then(|c| self.ui.library.rows.get(c))
                    .is_some_and(|r| matches!(r.kind, Kind::Note { .. } | Kind::Section));
                if let (true, Some(c)) = (removable, cur) {
                    self.library_act(LibAct::Remove, c);
                }
            }
            KeyName::Escape => {
                self.ui.library.focused = false;
                self.caret_reset();
            }
            KeyName::Char(_) => {
                if let Some(ch) = typed
                    .and_then(|t| t.chars().next())
                    .filter(|c| !c.is_control())
                {
                    self.ui.library.jump(ch);
                }
            }
            // Keys of the text that the pane keeps from it.
            KeyName::Tab | KeyName::Backspace | KeyName::Space | KeyName::Delete => {}
            KeyName::F(_) => return false,
        }
        self.redraw();
        true
    }

    /// The menu of the cursor's row, opened under the row by the keyboard.
    fn library_key_menu(&mut self) {
        let Some(c) = self.ui.library.cur else { return };
        self.library_with_rows_drawn();
        let at = self
            .hit_rect(Hit::LibraryRow(c))
            .or(self.areas.files)
            .map_or((0.0, 0.0), |r| {
                (r.x + 40.0, r.bottom().min(r.y + mdshared_ui::pane::ROW_H))
            });
        self.library_row_menu(c, at.0 - 2.0, at.1 - 2.0, true);
    }

    /// The hit rectangles are those of the last frame; a key pressed before any was drawn
    /// (a test, or the first frame) has them made.
    fn library_with_rows_drawn(&mut self) {
        if self
            .ui
            .library
            .cur
            .is_some_and(|c| self.hit_rect(Hit::LibraryRow(c)).is_none())
        {
            self.compose_layout_only();
        }
    }

    /// "Show this file in the notes library": the pane shows the notes, opened down to the open
    /// file's note, with the cursor on it and scrolled to it.
    pub(super) fn library_show_file(&mut self) {
        let Some(path) = self.docs.active_doc().and_then(|d| d.path.clone()) else {
            self.docs.set_toast(strings::LIBRARY_NEEDS_FILE);
            return;
        };
        self.library_load();
        let found = self
            .docs
            .library
            .groups
            .iter()
            .flat_map(|g| g.entries.iter().map(move |e| (g, e)))
            .find(|(_, e)| crate::library_pane::same_note(&e.path, &path))
            .map(|(g, e)| (g.file.clone(), e.sections.clone(), e.path.clone()));
        let Some((file, sections, note)) = found else {
            self.docs.set_toast(strings::LIBRARY_NOT_LISTED);
            return;
        };
        self.ui.set_pane_face(PaneFace::Notes);
        self.ui.library.unfold_to(&file, &sections);
        self.library_rows();
        let under = crate::library_pane::key_of(&file, &sections);
        self.ui
            .library
            .follow(crate::library_pane::RowId::Note(under, note));
        self.redraw();
    }

    /// While the pane shows, the library files are looked at again now and then, so a library
    /// edited in another window (or in a tab here) shows as it is now. The look is the cheap
    /// list of names and stamps; a read it finds worth starting runs on a thread, so a save in
    /// another window no longer stalls the event loop for the whole library's read.
    pub(super) fn library_tick(&mut self, now: Instant) {
        // Rows must not move under a held note: the drag's row and its target are indices into
        // them, so a re-read waits until the button is up.
        if matches!(self.ui.drag, Some(Drag::LibraryNote { moving: true, .. })) {
            return;
        }
        // A finished read is taken first, whatever else this pass does.
        self.library_read_take();
        if !(self.ui.panes.files && self.ui.library.on) || !self.ui.library.due(now) {
            return;
        }
        self.ui.library.checked = Some(now);
        // The first list is read here, once, as it always was; every later re-read is a job.
        if self.docs.library_stamp.dir.as_os_str().is_empty() {
            let before = self.docs.library.groups.len();
            let stamps: Vec<_> = self.docs.library.groups.iter().map(|g| g.stamp).collect();
            self.library_load();
            let changed = self.docs.library.groups.len() != before
                || self
                    .docs
                    .library
                    .groups
                    .iter()
                    .zip(&stamps)
                    .any(|(g, s)| g.stamp != *s);
            if changed {
                // The rows moved under a name being typed: it is dropped rather than put on the
                // wrong row.
                self.ui.library.stop_edit();
                self.ui.drop_library_field_focus();
                self.library_build();
                self.redraw();
            }
            return;
        }
        if self.library_job.is_some() || !self.docs.library_stamp.changed() {
            return;
        }
        self.library_start_read();
    }

    /// The "Notes library" command: the list of notes, read again first.
    pub(super) fn library_open(&mut self) {
        self.library_load();
        self.palette
            .open_library(&self.docs, &self.ui, &mut self.macros);
    }

    /// "Add this file to the notes library": asks where to keep it, once there is a file to
    /// keep. The answer is a [`crate::palette::Target::NotePlace`].
    pub(super) fn library_add(&mut self) {
        let Some(path) = self.docs.active_doc().and_then(|d| d.path.clone()) else {
            self.docs.set_toast(strings::LIBRARY_NEEDS_FILE);
            return;
        };
        if self.library_dir().is_none() {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        }
        self.library_load();
        let same = mdshared_app::paths::canonical_or_absolute(&path);
        let listed = self
            .docs
            .library
            .entries()
            .find(|(_, e)| mdshared_app::paths::canonical_or_absolute(&e.path) == same)
            .map(|(_, e)| e.title.clone());
        if let Some(title) = listed {
            self.docs.set_toast(&strings::library_has(&title));
            return;
        }
        self.palette
            .open_library_add(&self.docs, &self.ui, &mut self.macros);
    }

    /// Writes the active file into library file `file` under the heading path `section` (its
    /// end when the path is empty), making the file from its template when it is not there yet.
    pub(super) fn library_put(&mut self, file: &Path, section: &[String]) {
        let Some(doc) = self.docs.active_doc() else {
            return;
        };
        let Some(path) = doc.path.clone() else {
            self.docs.set_toast(strings::LIBRARY_NEEDS_FILE);
            return;
        };
        // Open in a tab with changes not saved: writing the file under them would be lost to
        // that tab's next save, as the saved prompts file is protected (audit 1 round 5).
        if let Err(why) = self.library_file_free(file) {
            self.docs.set_toast(&why);
            return;
        }
        // The one writer of a note into a library file, shared with mdview's "Add to the notes
        // library" (rule 8): the folder and, from its template, the file made when they are not
        // there; written only when it reads back as the notes there were and this one at its
        // place, through `files::replace`, a byte order mark kept. A file that is not UTF-8 text,
        // or that changed while it was read, is left as it is (audit 4, RB3, D3, F17).
        let dir = self.docs.library.dir.clone();
        let title = match library::put::put_note(file, section, &path, &dir) {
            Ok(title) => title,
            Err(e) => {
                self.docs
                    .set_toast(&strings::library_failed(&e.to_string()));
                return;
            }
        };
        self.library_load();
        // The pane's rows are made from the library, so they are made again now, with what is
        // folded above the new note opened: it shows at once, not after a fold and an unfold.
        // A name being typed there goes with them, and the keyboard back to the text rather
        // than to a box the new rows no longer hold.
        let menu = self.library_menu_row();
        self.ui.library.unfold_to(file, section);
        self.ui.library.stop_edit();
        self.ui.drop_library_field_focus();
        self.ui.library.build(&self.docs.library);
        self.library_menu_check(menu);
        self.caret_reset();
        self.redraw();
        let group = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| mdshared_app::paths::same_path(&g.file, file))
            .map(|g| g.name.clone())
            .unwrap_or_default();
        self.docs.set_toast(&strings::library_added(&title, &group));
    }

    /// The library file "Edit a library file" opens without asking: the group of the row last
    /// clicked in the pane while it shows, else of the note in the tab in front when the library
    /// lists it, else the only group. `None` when there is more than one to choose from.
    fn library_file_to_edit(&self) -> Option<PathBuf> {
        let groups = &self.docs.library.groups;
        let named = |file: &Path| {
            groups
                .iter()
                .find(|g| mdshared_app::paths::same_path(&g.file, file))
                .map(|g| g.file.clone())
        };
        if self.ui.panes.files
            && self.ui.library.on
            && let Some(file) = self.ui.library.cursor.as_deref().and_then(named)
        {
            return Some(file);
        }
        if let Some(open) = self.docs.active_doc().and_then(|d| d.path.as_deref())
            && let Some(group) = groups.iter().find(|g| {
                g.entries
                    .iter()
                    .any(|e| crate::library_pane::same_note(&e.path, open))
            })
        {
            return Some(group.file.clone());
        }
        match groups.as_slice() {
            [only] => Some(only.file.clone()),
            _ => None,
        }
    }

    /// "Edit a library file": opens one library file in a tab, the one `library_file_to_edit`
    /// finds, else the one chosen from a list; makes the first one from its template when the
    /// folder is empty.
    pub(super) fn library_edit(&mut self) {
        let Some(dir) = self.library_dir() else {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        };
        self.library_load();
        if self.docs.library.groups.is_empty() {
            match first_library_file(&dir) {
                Ok(file) => {
                    self.open(&file);
                    self.library_load();
                }
                Err(e) => self
                    .docs
                    .set_toast(&strings::library_failed(&e.to_string())),
            }
            return;
        }
        match self.library_file_to_edit() {
            Some(file) => self.open(&file),
            None => self
                .palette
                .open_library_files(&self.docs, &self.ui, &mut self.macros),
        }
    }
}

/// The library file "Edit a library file" opens in a library that read as empty: `notes.md`,
/// made from the template when it is not there. Made without replacing, so a `notes.md` the read
/// did not list (made since, or one it could not read) is opened as it is and never written over
/// (audit 4, F18). Anything there by that name that is not a file is refused.
pub(super) fn first_library_file(dir: &Path) -> std::io::Result<PathBuf> {
    let file = dir.join(library::DEFAULT_FILE);
    std::fs::create_dir_all(dir)?;
    match mdshared_app::files::create_new(&file, library::TEMPLATE.as_bytes()) {
        Ok(()) => Ok(file),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && file.is_file() => Ok(file),
        Err(e) => Err(e),
    }
}

/// Where a note of the library opens, by its file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NoteOpen {
    /// In a tab, as text.
    Tab,
    /// With the system's app for its kind; the extension, which the system call is limited to.
    System(&'static str),
    /// Shown in its folder and nothing started, for the reason said.
    Folder(&'static str),
}

/// The kinds handed to the system: the pictures (SVG excepted: it is text, and the editor
/// colours it) and the documents the task window calls "other". Nothing that runs.
const OUTSIDE: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "avif", "pdf", "doc", "docx", "odt", "rtf", "xls",
    "xlsx", "ods", "ppt", "pptx", "odp", "zip", "7z", "rar", "mp3", "wav", "flac", "ogg", "m4a",
    "mp4", "mkv", "mov", "avi", "webm", "wmv",
];

/// Kinds that run, or start something that does: never started from the library. One that is
/// text (a script) opens in a tab like any text, which runs nothing; one that is not is shown
/// in its folder as a program.
const PROGRAMS: &[&str] = &[
    "exe", "com", "bat", "cmd", "ps1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "msi", "msp",
    "scr", "pif", "lnk", "url", "reg", "hta", "cpl", "jar", "app", "dll", "so", "dylib", "sh",
    "command",
];

/// What a click on a note whose file is `path` does. A picture or a document goes to the
/// system; Markdown and plain text open in a tab; any other file opens in a tab when its first
/// 8 KB hold no NUL byte (what the editor's loader calls text), a script too, and is only shown
/// otherwise, as a program for the kinds that run. A file that cannot be read is left to the
/// tab, which says why, unless it is of a kind that runs: that is only shown.
pub(super) fn note_opener(path: &Path) -> NoteOpen {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if let Some(k) = OUTSIDE.iter().find(|k| **k == ext) {
        return NoteOpen::System(k);
    }
    let program = PROGRAMS.contains(&ext.as_str());
    if !program && matches!(ext.as_str(), "md" | "markdown" | "txt" | "svg") {
        return NoteOpen::Tab;
    }
    let mut head = [0u8; 8192];
    let read = std::fs::File::open(path).and_then(|mut f| {
        use std::io::Read;
        f.read(&mut head)
    });
    match read {
        Ok(n) if head[..n].contains(&0) && program => NoteOpen::Folder(strings::LIBRARY_PROGRAM),
        Ok(n) if head[..n].contains(&0) => NoteOpen::Folder(strings::LIBRARY_NOT_TEXT),
        Ok(_) => NoteOpen::Tab,
        Err(_) if program => NoteOpen::Folder(strings::LIBRARY_PROGRAM),
        Err(_) => NoteOpen::Tab,
    }
}

/// The system's own app, and the file manager. A test build starts neither: it notes what it
/// would have done, for the test to read.
mod system {
    use std::path::Path;
    #[cfg(test)]
    use std::path::PathBuf;

    #[cfg(test)]
    thread_local! {
        static DONE: std::cell::RefCell<Vec<(&'static str, PathBuf)>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }

    /// Opens `path` with the system's app for `ext`, the only extension it is allowed.
    pub fn open(path: &Path, ext: &str) -> bool {
        #[cfg(test)]
        {
            let _ = ext;
            DONE.with(|d| d.borrow_mut().push(("open", path.to_path_buf())));
            path.is_file()
        }
        #[cfg(not(test))]
        {
            mdshared_ui::platform::open_file(path, &[ext])
        }
    }

    pub fn reveal(path: &Path) {
        #[cfg(test)]
        DONE.with(|d| d.borrow_mut().push(("reveal", path.to_path_buf())));
        #[cfg(not(test))]
        mdshared_ui::platform::reveal_in_file_manager(path);
    }

    /// What was handed over since the last call, on this thread.
    #[cfg(test)]
    pub fn taken() -> Vec<(&'static str, PathBuf)> {
        DONE.with(|d| std::mem::take(&mut *d.borrow_mut()))
    }
}

#[cfg(test)]
pub(super) use system::taken as handed_to_system;
