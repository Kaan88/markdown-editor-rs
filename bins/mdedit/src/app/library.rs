//! The notes library: the `library` folder beside the settings file, listed by the palette
//! ("Notes library"), written to by "Add this file to the notes library" and edited as the
//! Markdown files it is. Reading, the format and the writing are
//! [`mdshared_app::library`], shared with the viewer; what is here is when it is read, where
//! the folder is, and what the window does with it.

use super::*;
use crate::commands::strings;
use mdshared_app::library;

impl App {
    /// The library folder: the one the preference names (a path with no root is taken from the
    /// settings folder), else `library` beside the settings file (the application's data folder
    /// when the window runs without one). `None` while a shot runs on the user's own folder.
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
        self.docs.library = library::Library::default();
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
            self.docs.library = library::Library::default();
            return;
        };
        if self.docs.library.dir == dir && !self.docs.library.changed() {
            return;
        }
        self.docs.library = library::read(&dir);
    }

    /// The rows of the pane, with the library read again first. The folder is looked at at
    /// most every [`crate::library_pane::RECHECK`], so a pane left open costs nothing while
    /// nothing is clicked.
    pub(super) fn library_rows(&mut self) {
        self.library_load();
        self.ui.library.checked = Some(Instant::now());
        self.ui.library.build(&self.docs.library);
    }

    /// A click on a row: a group or a section folds, a note opens, a note that is not there
    /// says so.
    pub(super) fn library_row(&mut self, i: usize) {
        let Some(row) = self.ui.library.rows.get(i).cloned() else {
            return;
        };
        self.ui.library.cursor = self.ui.library.group_file(i);
        match row.kind {
            crate::library_pane::Kind::Group | crate::library_pane::Kind::Section => {
                self.ui.library.fold(&row.key);
                self.ui.library.build(&self.docs.library);
            }
            crate::library_pane::Kind::Note { missing: true, .. } => {
                self.docs.set_toast(strings::LIBRARY_MISSING_FILE);
            }
            crate::library_pane::Kind::Note { path, .. } => self.open(&path),
            crate::library_pane::Kind::Editing => {}
        }
    }

    /// While the pane shows, the library files are looked at again now and then, so a library
    /// edited in another window (or in a tab here) shows as it is now.
    pub(super) fn library_tick(&mut self, now: Instant) {
        if !(self.ui.panes.files && self.ui.library.on) || !self.ui.library.due(now) {
            return;
        }
        self.ui.library.checked = Some(now);
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
            self.ui.library.build(&self.docs.library);
            self.redraw();
        }
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
        let title = library::title_of(&path);
        // Open in a tab with changes not saved: writing the file under them would be lost to
        // that tab's next save, as the saved prompts file is protected (audit 1 round 5).
        if let Err(why) = self.library_file_free(file) {
            self.docs.set_toast(&why);
            return;
        }
        if let Some(parent) = file.parent()
            && let Err(e) = std::fs::create_dir_all(parent)
        {
            self.docs
                .set_toast(&strings::library_failed(&e.to_string()));
            return;
        }
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => library::TEMPLATE.to_string(),
            Err(e) => {
                self.docs
                    .set_toast(&strings::library_failed(&e.to_string()));
                return;
            }
        };
        let new = library::add_note(&text, section, &title, &path);
        // Written only when it reads back as the notes there were and this one at its place.
        let before = library::parse(&text, &self.docs.library.dir).entries;
        let after = library::parse(&new, &self.docs.library.dir).entries;
        let same = mdshared_app::paths::canonical_or_absolute(&path);
        let added = after.len() == before.len() + 1
            && after
                .iter()
                .any(|e| mdshared_app::paths::canonical_or_absolute(&e.path) == same);
        if !added {
            self.docs.set_toast(&strings::library_failed(
                "the file would not read back as its notes",
            ));
            return;
        }
        if let Err(e) = mdshared_app::settings::write_atomic(file, &new) {
            self.docs
                .set_toast(&strings::library_failed(&e.to_string()));
            return;
        }
        self.library_load();
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
                    .any(|e| mdshared_app::paths::same_path(&e.path, open))
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
            let file = dir.join(library::DEFAULT_FILE);
            if let Err(e) = std::fs::create_dir_all(&dir)
                .and_then(|()| std::fs::write(&file, library::TEMPLATE))
            {
                self.docs
                    .set_toast(&strings::library_failed(&e.to_string()));
                return;
            }
            self.open(&file);
            self.library_load();
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
