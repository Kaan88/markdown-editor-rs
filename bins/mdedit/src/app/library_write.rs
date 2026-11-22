//! What the notes library pane's menus do: each is one write to a library file, made through the
//! writers of [`mdshared_app::library`] and never by editing text here. A write is refused while
//! the file is open in a tab with changes not saved, is made only when the new text reads back as
//! the old notes plus the change, and goes through `write_atomic`. A writer that finds nothing to
//! do (it gives `None`) leaves the file alone without a word. The notes themselves are never
//! written, moved or deleted.

use super::*;
use crate::commands::strings;
use crate::library_pane::{Edit, EditKind, Kind, Moving, Removal, Row};
use crate::ui::{BarField, DialogKind, LibAct, Popup, PopupKind};
use mdshared_app::library::{self, Parsed};
use mdshared_app::paths::{canonical_or_absolute, same_path};

/// How an attempt to write a library file ended.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Wrote {
    Done,
    /// The writer had nothing to do, or the line was not what it expects: nothing was written.
    Nothing,
    /// It was refused, for this reason (said plainly).
    Refused(String),
}

/// Whether line `line` of a library file's text is the note naming `path`.
fn note_at(text: &str, dir: &Path, line: usize, path: &Path) -> bool {
    library::parse(text, dir)
        .entries
        .iter()
        .any(|e| e.line == line && same_path(&e.path, path))
}

/// A name as a file name, by the one rule of the shared crate
/// ([`library::folder_name`]): nothing the file system refuses, no `%` for the reader to take for
/// `%20`, no dots at the end, not a name Windows keeps for devices. `None` when nothing is left
/// to name it by: no text, or nothing but characters that were replaced.
pub(super) fn file_stem_for(name: &str) -> Option<String> {
    let stem = library::folder_name(name);
    // `folder_name` always gives something: `Untitled` for no name, `_` for what was replaced.
    let no_name = name.trim().trim_end_matches(['.', ' ']).is_empty();
    let all_replaced = stem.chars().all(|c| c == '_') && !name.contains('_');
    (!no_name && !all_replaced).then_some(stem)
}

/// A new note's title as a file name: lower case with hyphens (D-075), then [`file_stem_for`].
pub(super) fn note_stem_for(title: &str) -> Option<String> {
    file_stem_for(&mdedit_core::split::note_slug(title))
}

/// `stem.md` in `dir`, or `stem 2.md`, `stem 3.md` … when that is taken.
fn free_name(dir: &Path, stem: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.md"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} {n}.md")))
        .find(|p| !p.exists())
        .unwrap_or(first)
}

/// Removes the folders made for a note that could not be linked, deepest first, while they are
/// empty. A folder holding anything is left as it is.
fn remove_empty_folders(made: &[PathBuf]) {
    for folder in made {
        let _ = std::fs::remove_dir(folder);
    }
}

impl App {
    /// The name of the group whose library file is `file`.
    fn library_group_name(&self, file: &Path) -> String {
        self.docs
            .library
            .groups
            .iter()
            .find(|g| same_path(&g.file, file))
            .map(|g| g.name.clone())
            .unwrap_or_default()
    }

    /// Whether the open file can be written under a group or section: it has a file and the
    /// library does not list it yet.
    fn library_can_add(&self) -> bool {
        let Some(path) = self.docs.active_doc().and_then(|d| d.path.as_deref()) else {
            return false;
        };
        let same = canonical_or_absolute(path);
        !self
            .docs
            .library
            .entries()
            .any(|(_, e)| canonical_or_absolute(&e.path) == same)
    }

    /// Whether (x, y) is in the notes library's list: the pane shows it, below its head.
    pub(super) fn library_pane_at(&self, x: f32, y: f32) -> bool {
        self.ui.panes.files
            && self.ui.library.on
            && self
                .areas
                .files
                .is_some_and(|r| crate::library_pane::list(r).contains(x, y))
    }

    /// The library read again, and the pane's rows made from it.
    fn library_changed(&mut self) {
        self.library_load();
        self.library_rows();
        self.redraw();
    }

    /// A library file the window may write: not while a tab holds changes to it that are not
    /// saved, which the tab's next save would undo.
    pub(super) fn library_file_free(&self, file: &Path) -> Result<(), String> {
        match self
            .docs
            .tabs
            .iter()
            .find(|d| d.modified() && d.path.as_deref().is_some_and(|p| same_path(p, file)))
        {
            Some(open) => Err(strings::library_unsaved(&open.name())),
            None => Ok(()),
        }
    }

    /// Rewrites a library file. `change` makes the new text from the old, or `None` when there
    /// is nothing to do; `ok` is given the notes of the old text and of the new and must agree
    /// that the new one is the old plus the change. Nothing is said here: the caller decides.
    /// The paths inside the file resolve against the library folder the pane shows.
    pub(super) fn library_attempt(
        &mut self,
        file: &Path,
        change: impl FnOnce(&str) -> Option<String>,
        ok: impl Fn(&Parsed, &Parsed) -> bool,
    ) -> Wrote {
        let dir = self.docs.library.dir.clone();
        self.library_attempt_at(file, &dir, change, ok)
    }

    /// [`library_attempt`](Self::library_attempt) with the library folder its lines resolve
    /// against given, so a plan writes its links against the folder it was made from even when
    /// the setting changed while it was open.
    pub(super) fn library_attempt_at(
        &mut self,
        file: &Path,
        dir: &Path,
        change: impl FnOnce(&str) -> Option<String>,
        ok: impl Fn(&Parsed, &Parsed) -> bool,
    ) -> Wrote {
        if let Err(why) = self.library_file_free(file) {
            return Wrote::Refused(why);
        }
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) => return Wrote::Refused(strings::library_failed(&e.to_string())),
        };
        let Some(new) = change(&text) else {
            return Wrote::Nothing;
        };
        if !ok(&library::parse(&text, dir), &library::parse(&new, dir)) {
            return Wrote::Refused(strings::library_failed(
                "the file would not read back as its notes",
            ));
        }
        if let Err(e) = mdshared_app::settings::write_atomic(file, &new) {
            return Wrote::Refused(strings::library_failed(&e.to_string()));
        }
        self.library_changed();
        Wrote::Done
    }

    /// [`library_attempt`](Self::library_attempt) with a refusal said in a toast.
    pub(super) fn library_write(
        &mut self,
        file: &Path,
        change: impl FnOnce(&str) -> Option<String>,
        ok: impl Fn(&Parsed, &Parsed) -> bool,
    ) -> bool {
        match self.library_attempt(file, change, ok) {
            Wrote::Done => true,
            Wrote::Nothing => false,
            Wrote::Refused(why) => {
                self.docs.set_toast(&why);
                false
            }
        }
    }

    // ---- the menus ---------------------------------------------------------------------------

    /// A right-click on row `i`: the pane's row moves to it and its menu opens.
    pub(super) fn library_row_menu(&mut self, i: usize, x: f32, y: f32, cues: bool) {
        let Some(row) = self.ui.library.rows.get(i).cloned() else {
            return;
        };
        if row.kind == Kind::Editing {
            return;
        }
        self.ui.library.cursor = self.ui.library.group_file(i);
        let menu = overlays::library_row_menu(
            &row.kind,
            row.home.as_deref(),
            i,
            self.library_can_add(),
            x + 2.0,
            y + 2.0,
            false,
        );
        self.ui.popup = Some(Popup::with_cues(PopupKind::LibraryRow(i), menu, cues));
    }

    /// The menu of the pane's empty space, at the pointer, or below the Notes head's plus button.
    pub(super) fn library_empty_menu(&mut self, x: f32, y: f32, align_right: bool, cues: bool) {
        let has_file = self.docs.active_doc().is_some_and(|d| d.path.is_some());
        let menu = overlays::library_empty_menu(has_file, x, y, align_right);
        self.ui.popup = Some(Popup::with_cues(PopupKind::LibraryEmpty, menu, cues));
    }

    /// What a menu item of the pane does, to row `i`.
    pub(super) fn library_act(&mut self, op: LibAct, i: usize) {
        match op {
            LibAct::NewGroup => {
                let at = self.ui.library.rows.len();
                let dir = self.library_dir().unwrap_or_default();
                self.library_start(EditKind::NewGroup, at, 0, dir, "");
            }
            LibAct::RevealFolder => {
                let Some(dir) = self.library_dir() else {
                    self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
                    return;
                };
                self.library_load();
                match self.docs.library.groups.first() {
                    Some(g) => {
                        platform::reveal_in_file_manager(&g.file);
                    }
                    None if dir.is_dir() => {
                        platform::reveal_in_file_manager(&dir);
                    }
                    None => self.docs.set_toast(strings::LIBRARY_NO_FOLDER_YET),
                }
            }
            LibAct::Refresh => {
                self.docs.forget_library();
                self.library_changed();
            }
            _ => {
                let Some(row) = self.ui.library.rows.get(i).cloned() else {
                    return;
                };
                let Some(file) = self.ui.library.group_file(i) else {
                    return;
                };
                self.library_row_act(op, i, &row, &file);
            }
        }
    }

    fn library_row_act(&mut self, op: LibAct, i: usize, row: &Row, file: &Path) {
        let section = row.section.clone();
        match (op, &row.kind) {
            (LibAct::Open, Kind::Note { .. }) => self.library_row(i),
            (LibAct::OpenOther, Kind::Note { path, .. }) => {
                self.open(path);
                self.command(C::ShowInOtherPane);
            }
            (LibAct::Rename, Kind::Group) => {
                self.library_start(
                    EditKind::RenameGroup,
                    i,
                    row.depth,
                    file.to_path_buf(),
                    &row.label,
                );
            }
            (LibAct::Rename, Kind::Section) => {
                self.library_start_in(EditKind::RenameSection, i, row, file, &row.label);
            }
            (LibAct::Rename, Kind::Note { .. }) => {
                self.library_start_in(EditKind::RenameNote, i, row, file, &row.label);
            }
            (LibAct::RenameFile, Kind::Group) => {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.library_start(
                    EditKind::RenameFile,
                    i,
                    row.depth,
                    file.to_path_buf(),
                    &stem,
                );
            }
            (LibAct::SetHome, Kind::Group) => self.library_choose_group_home(file),
            (LibAct::ClearHome, Kind::Group) => self.library_clear_group_home(file),
            (LibAct::NewNote, Kind::Group | Kind::Section) => {
                self.library_start_in(EditKind::NewNote, i + 1, row, file, "");
            }
            (LibAct::NewSection, Kind::Group | Kind::Section) => {
                self.library_start_in(EditKind::NewSection, i + 1, row, file, "");
            }
            (LibAct::AddOpen, Kind::Group | Kind::Section) => self.library_put(file, &section),
            (LibAct::MoveTo, Kind::Note { path, line, .. }) => {
                // The places are the library as it is now, as the other lists read it.
                self.library_load();
                self.ui.library.moving = Some(Moving {
                    file: file.to_path_buf(),
                    line: *line,
                    title: row.label.clone(),
                    note: path.clone(),
                });
                self.palette
                    .open_library_move(&self.docs, &self.ui, &mut self.macros);
            }
            (LibAct::FindFile, Kind::Note { path, line, .. }) => {
                self.library_find_file(file, *line, &row.label, path);
            }
            (LibAct::CopyPath, Kind::Group) => self.library_copy_path(file),
            (LibAct::CopyPath, Kind::Note { path, .. }) => self.library_copy_path(path),
            (LibAct::Reveal, Kind::Group) => {
                platform::reveal_in_file_manager(file);
            }
            (LibAct::Reveal, Kind::Note { path, .. }) => {
                platform::reveal_in_file_manager(path);
            }
            (LibAct::GoToLine, Kind::Note { line, .. }) => {
                self.library_go_to(file, Some(*line), &[])
            }
            (LibAct::GoToLine, Kind::Section) => self.library_go_to(file, None, &section),
            (LibAct::EditFile, Kind::Group) => self.open(file),
            (LibAct::Remove, Kind::Note { path, line, .. }) => {
                self.library_remove_note(file, *line, path, &row.label);
            }
            (LibAct::Remove, Kind::Section) => self.library_ask_remove(file, section),
            (LibAct::DeleteFile, Kind::Group) => self.library_ask_delete(file),
            _ => {}
        }
    }

    fn library_copy_path(&mut self, path: &Path) {
        if platform::set_clipboard_text(&path.display().to_string()) {
            self.docs.set_toast("Path copied");
        }
    }

    /// Opens a library file with the caret on a note's line, or on a section's heading.
    fn library_go_to(&mut self, file: &Path, line: Option<usize>, section: &[String]) {
        let line = line.or_else(|| {
            let text = std::fs::read_to_string(file).ok()?;
            library::section_line(&text, section)
        });
        self.open(file);
        if let Some(line) = line {
            self.docs.go_to_or_wait(line, 1);
        }
    }

    // ---- the name box ------------------------------------------------------------------------

    /// Starts typing a name for a group, or a file, or the new group.
    fn library_start(&mut self, kind: EditKind, at: usize, depth: u16, file: PathBuf, text: &str) {
        let edit = Edit {
            kind,
            at,
            depth,
            file,
            section: Vec::new(),
            line: 0,
            note: PathBuf::new(),
        };
        self.library_begin(edit, text);
    }

    /// Starts typing a name for what `row` is, or for what goes under it.
    fn library_start_in(&mut self, kind: EditKind, at: usize, row: &Row, file: &Path, text: &str) {
        let (line, note) = match &row.kind {
            Kind::Note { path, line, .. } => (*line, path.clone()),
            _ => (0, PathBuf::new()),
        };
        let depth = match kind {
            EditKind::NewNote | EditKind::NewSection => row.depth + 1,
            _ => row.depth,
        };
        let edit = Edit {
            kind,
            at,
            depth,
            file: file.to_path_buf(),
            section: row.section.clone(),
            line,
            note,
        };
        self.library_begin(edit, text);
    }

    fn library_begin(&mut self, edit: Edit, text: &str) {
        let at = edit.at;
        self.ui.library.start_edit(edit, text, &self.docs.library);
        self.ui.library.reveal(at);
        self.ui.focus_field(BarField::Library);
        self.caret_reset();
        self.redraw();
    }

    /// Enter, or a click elsewhere: the name is used.
    pub(super) fn library_edit_commit(&mut self) {
        let Some(edit) = self.ui.library.edit.clone() else {
            return;
        };
        let name = self.ui.library.field.text().trim().to_string();
        self.library_edit_cancel();
        if name.is_empty() {
            return;
        }
        match edit.kind {
            EditKind::RenameGroup => self.library_rename_group(&edit.file, &name),
            EditKind::RenameSection => {
                self.library_rename_section(&edit.file, &edit.section, &name);
                // The keyboard's row is the section still, under the name it has now.
                let mut renamed = edit.section.clone();
                if let Some(last) = renamed.last_mut() {
                    *last = name.clone();
                }
                let key = crate::library_pane::key_of(&edit.file, &renamed);
                if self.ui.library.cur.is_some() {
                    self.ui
                        .library
                        .follow(crate::library_pane::RowId::Fold(key));
                }
            }
            EditKind::RenameNote => {
                self.library_rename_note(&edit.file, edit.line, &edit.note, &name)
            }
            EditKind::RenameFile => self.library_rename_file(&edit.file, &name),
            EditKind::NewGroup => self.library_new_group(&name),
            EditKind::NewSection => self.library_new_section(&edit.file, &edit.section, &name),
            EditKind::NewNote => self.library_new_note(&edit.file, &edit.section, &name),
        }
        self.redraw();
    }

    /// Escape: the name is dropped and the rows are the files' again.
    pub(super) fn library_edit_cancel(&mut self) {
        self.ui.library.stop_edit();
        self.ui.drop_library_field_focus();
        self.ui.library.build(&self.docs.library);
        self.caret_reset();
        self.redraw();
    }

    // ---- the writes --------------------------------------------------------------------------

    pub(super) fn library_rename_group(&mut self, file: &Path, name: &str) {
        let want = name.trim().to_string();
        self.library_write(
            file,
            |t| library::set_group_name(t, name),
            |b, a| a.name.as_deref() == Some(want.as_str()) && a.entries.len() == b.entries.len(),
        );
    }

    pub(super) fn library_rename_section(&mut self, file: &Path, section: &[String], name: &str) {
        self.library_write(
            file,
            |t| {
                let line = library::section_line(t, section)?;
                library::rename_section(t, line, name)
            },
            |b, a| a.sections.len() == b.sections.len() && a.entries.len() == b.entries.len(),
        );
    }

    pub(super) fn library_rename_note(
        &mut self,
        file: &Path,
        line: usize,
        note: &Path,
        name: &str,
    ) {
        let dir = self.docs.library.dir.clone();
        let want = name.replace(['[', ']'], "").trim().to_string();
        self.library_write(
            file,
            |t| {
                if !note_at(t, &dir, line, note) {
                    return None;
                }
                library::rename_note(t, line, name)
            },
            |b, a| {
                a.entries.len() == b.entries.len()
                    && a.entries
                        .iter()
                        .find(|e| e.line == line)
                        .is_some_and(|e| e.title == want)
            },
        );
    }

    /// Renames the library file itself (its group keeps its name): the order the groups are
    /// listed in follows the file names.
    pub(super) fn library_rename_file(&mut self, file: &Path, name: &str) {
        if let Err(why) = self.library_file_free(file) {
            self.docs.set_toast(&why);
            return;
        }
        if let Some(open) = self
            .docs
            .tabs
            .iter()
            .find(|d| d.path.as_deref().is_some_and(|p| same_path(p, file)))
        {
            let name = open.name();
            self.docs.set_toast(&strings::library_busy_file(&name));
            return;
        }
        let Some(stem) = file_stem_for(name) else {
            self.docs.set_toast(&strings::library_not_a_name(name));
            return;
        };
        let to = file.with_file_name(format!("{stem}.md"));
        if same_path(&to, file) && to.file_name() == file.file_name() {
            return;
        }
        if to.exists() && !same_path(&to, file) {
            self.docs
                .set_toast(&strings::library_exists(&format!("{stem}.md")));
            return;
        }
        if let Err(e) = std::fs::rename(file, &to) {
            self.docs
                .set_toast(&strings::library_failed(&e.to_string()));
            return;
        }
        self.ui.library.cursor = Some(to);
        self.library_changed();
    }

    /// "Set the group's folder…": a folder picker, starting on the folder the group has or the
    /// library folder, then the group's `home:` line names the folder chosen.
    fn library_choose_group_home(&mut self, file: &Path) {
        let start = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| same_path(&g.file, file))
            .and_then(|g| g.home.clone())
            .filter(|d| d.is_dir())
            .or_else(|| self.library_dir().filter(|d| d.is_dir()))
            .or_else(|| self.start_dir());
        #[cfg(windows)]
        if let Some(dir) = platform::pick_folder(start.as_deref()) {
            self.library_group_home_picked(file, dir);
        }
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            let file = file.to_path_buf();
            platform::pick_folder_async(start.as_deref(), move |dir| {
                wake(Wake::Dialog(Picked::GroupHome(file, dir)));
            });
        }
    }

    /// The folder picker answered: the group's `home:` line names `dir`, relative to the library
    /// folder when it is inside it and by its whole path otherwise. Nothing on disk moves.
    pub(super) fn library_group_home_picked(&mut self, file: &Path, dir: PathBuf) {
        // A7: a group whose folder is the library folder writes `.md` files there, and the reader
        // takes each for a library file. The picker cannot make one.
        if self
            .library_dir()
            .is_some_and(|root| same_path(&dir, &root))
        {
            self.docs.set_toast(strings::LIBRARY_HOME_IS_LIBRARY);
            return;
        }
        let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let home = crate::library_pane::link_path(&base, &dir);
        let want = canonical_or_absolute(&dir);
        let done = self.library_write(
            file,
            |t| library::set_group_home(t, &home),
            |b, a| {
                a.entries.len() == b.entries.len()
                    && a.home
                        .as_deref()
                        .is_some_and(|h| same_path(&canonical_or_absolute(h), &want))
            },
        );
        if done {
            let group = self.library_group_name(file);
            self.docs
                .set_toast(&strings::library_home_now(&group, &dir.to_string_lossy()));
        }
    }

    /// "Clear the group's folder": the `home:` line goes and the notes are left where they are.
    fn library_clear_group_home(&mut self, file: &Path) {
        let done = self.library_write(file, library::clear_group_home, |b, a| {
            b.home.is_some() && a.home.is_none() && a.entries.len() == b.entries.len()
        });
        if done {
            let group = self.library_group_name(file);
            self.docs.set_toast(&strings::library_home_cleared(&group));
        }
    }

    /// Makes a library file for a new group, with the group's name on it and the folder its
    /// notes are written in (`home:`), named for the group.
    pub(super) fn library_new_group(&mut self, name: &str) {
        let Some(dir) = self.library_dir() else {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        };
        let Some(stem) = file_stem_for(name) else {
            self.docs.set_toast(&strings::library_not_a_name(name));
            return;
        };
        let Some(text) = library::new_group_with_home(name, Some(Path::new(&stem))) else {
            return;
        };
        let file = dir.join(format!("{stem}.md"));
        if file.exists() {
            self.docs
                .set_toast(&strings::library_exists(&format!("{stem}.md")));
            return;
        }
        let read = library::parse(&text, &dir);
        if read.name.as_deref() != Some(name.trim()) || read.home.is_none() {
            self.docs.set_toast(&strings::library_failed(
                "the file would not read back as its notes",
            ));
            return;
        }
        if let Err(e) = std::fs::create_dir_all(&dir)
            .and_then(|()| mdshared_app::settings::write_atomic(&file, &text))
        {
            self.docs
                .set_toast(&strings::library_failed(&e.to_string()));
            return;
        }
        self.ui.library.cursor = Some(file);
        self.library_changed();
    }

    pub(super) fn library_new_section(&mut self, file: &Path, parent: &[String], name: &str) {
        self.library_write(
            file,
            |t| library::add_section(t, parent, name),
            |b, a| a.sections.len() == b.sections.len() + 1 && a.entries.len() == b.entries.len(),
        );
    }

    /// Makes `<name>.md` in the folder its group's `home:` line names, one folder deeper for
    /// each heading of `section`, lists it under that heading by a link relative to the library
    /// folder when it is inside it, and opens it. A group with no `home:` line gets one in the
    /// same write, naming a folder derived from its file name; nothing already on disk moves.
    /// The note is made only once the library file may be written, and taken away again with
    /// the folders made for it if the link could not be.
    pub(super) fn library_new_note(&mut self, file: &Path, section: &[String], name: &str) {
        if let Err(why) = self.library_file_free(file) {
            self.docs.set_toast(&why);
            return;
        }
        let Some(stem) = note_stem_for(name) else {
            self.docs.set_toast(&strings::library_not_a_name(name));
            return;
        };
        let base = file.parent().map(Path::to_path_buf).unwrap_or_default();
        let own = file
            .file_stem()
            .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let text = match std::fs::read_to_string(file) {
            Ok(text) => text,
            Err(e) => {
                self.docs
                    .set_toast(&strings::library_failed(&e.to_string()));
                return;
            }
        };
        let parsed = library::parse(&text, &base);
        // A group with no `home:` line gets one the first time a note is made in it (decision
        // 10), named for the group as its heading writes it, so the folder reads in Explorer as
        // the group reads in the pane. The line is relative to the library folder, so a library
        // that moves keeps working.
        let (home, new_home) = match parsed.home {
            Some(home) => {
                // A7: a `home:` that is the library folder itself (written by hand, or by an
                // older version) would put this note in the library as a library file.
                if self
                    .library_dir()
                    .is_some_and(|root| same_path(&home, &root))
                {
                    self.docs.set_toast(strings::LIBRARY_HOME_IS_LIBRARY);
                    return;
                }
                (home, None)
            }
            None => {
                let named = parsed.name.as_deref().unwrap_or(&own);
                let folder = PathBuf::from(library::folder_name(named));
                (base.join(&folder), Some(folder))
            }
        };
        let dir = library::note_folder(&home, section);
        // The folders this note has to make, deepest first, so a refused link can take them
        // away again while they are empty.
        let mut made = Vec::new();
        let mut at = dir.as_path();
        while !at.exists() {
            made.push(at.to_path_buf());
            match at.parent() {
                Some(parent) => at = parent,
                None => break,
            }
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            remove_empty_folders(&made);
            self.docs
                .set_toast(&strings::library_home_failed(&e.to_string()));
            return;
        }
        let note = free_name(&dir, &stem);
        let heading = name.replace(['[', ']'], "");
        if let Err(e) = std::fs::write(&note, format!("# {}\n", heading.trim())) {
            remove_empty_folders(&made);
            self.docs
                .set_toast(&strings::library_failed(&e.to_string()));
            return;
        }
        let link = crate::library_pane::link_path(&base, &note);
        let done = self.library_write(
            file,
            |t| {
                let with_home = match &new_home {
                    Some(folder) => {
                        library::set_group_home(t, folder).unwrap_or_else(|| t.to_string())
                    }
                    None => t.to_string(),
                };
                Some(library::add_note_in_group(&with_home, section, name, &link))
            },
            |b, a| a.entries.len() == b.entries.len() + 1,
        );
        if done {
            if new_home.is_some() {
                let group = self.library_group_name(file);
                self.docs
                    .set_toast(&strings::library_home_now(&group, &home.to_string_lossy()));
            }
            self.open(&note);
        } else {
            let _ = std::fs::remove_file(&note);
            remove_empty_folders(&made);
        }
    }

    /// Takes a note out of the library file. One line goes; the note itself is not touched.
    pub(super) fn library_remove_note(
        &mut self,
        file: &Path,
        line: usize,
        note: &Path,
        title: &str,
    ) {
        let dir = self.docs.library.dir.clone();
        let done = self.library_write(
            file,
            |t| {
                if !note_at(t, &dir, line, note) {
                    return None;
                }
                library::remove_note(t, line)
            },
            |b, a| a.entries.len() + 1 == b.entries.len(),
        );
        if done {
            let group = self.library_group_name(file);
            self.docs
                .set_toast(&strings::library_took_out(title, &group));
        }
    }

    /// Asks before a section goes, saying how many notes go with it.
    fn library_ask_remove(&mut self, file: &Path, section: Vec<String>) {
        let notes = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| same_path(&g.file, file))
            .map_or(0, |g| {
                g.entries
                    .iter()
                    .filter(|e| e.sections.starts_with(&section))
                    .count()
            });
        let name = section.last().cloned().unwrap_or_default();
        self.ui.library.removing = Some(Removal {
            file: file.to_path_buf(),
            section,
        });
        self.open_dialog(DialogKind::LibraryRemove);
        if let Some(d) = self.ui.dialog.as_mut() {
            d.subject = strings::library_remove_body(&name, notes);
        }
    }

    /// The answer to the question about removing a section: button 0 is "Remove".
    pub(super) fn library_remove_answer(&mut self, n: usize) {
        let removal = self.ui.library.removing.take();
        self.close_dialog();
        let (Some(r), 0) = (removal, n) else {
            return;
        };
        self.library_write(
            &r.file,
            |t| {
                let line = library::section_line(t, &r.section)?;
                library::remove_section(t, line)
            },
            |b, a| a.sections.len() < b.sections.len() && a.entries.len() <= b.entries.len(),
        );
    }

    /// Whether `file` is one of the library folder's own Markdown files: a regular file that is
    /// directly in the folder (as the folder is really spelled, so a link out of it does not
    /// count) and ends in `.md`. The only files the window ever deletes.
    pub(super) fn library_own_file(&self, file: &Path) -> bool {
        let Some(dir) = self.library_dir().and_then(|d| d.canonicalize().ok()) else {
            return false;
        };
        let Ok(real) = file.canonicalize() else {
            return false;
        };
        real.is_file()
            && real
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"))
            && real.parent() == Some(dir.as_path())
    }

    /// Whether a library file lists `file` as a note, in any group: then it is a note and not a
    /// library file, and it is not deleted (the toast says where it is listed). The folder is read
    /// as it is on disk now, so a list drawn before another window changed it is no excuse.
    fn library_lists_as_note(&mut self, file: &Path) -> bool {
        let Some(dir) = self.library_dir() else {
            return false;
        };
        let read = library::read(&dir);
        let Some((group, entry)) = crate::library_pane::listing(&read, file) else {
            return false;
        };
        let shown = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let says = strings::library_is_a_note(&shown, &group.name, &entry.sections);
        self.docs.set_toast(&says);
        true
    }

    /// Asks before a library file is deleted, saying its group and how many notes it lists. It
    /// is refused at once while a tab holds changes to it that are not saved.
    fn library_ask_delete(&mut self, file: &Path) {
        if let Err(why) = self.library_file_free(file) {
            self.docs.set_toast(&why);
            return;
        }
        if !self.library_own_file(file) {
            self.docs.set_toast(strings::LIBRARY_NOT_IN_FOLDER);
            return;
        }
        if self.library_lists_as_note(file) {
            return;
        }
        let (name, notes) = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| same_path(&g.file, file))
            .map_or((String::new(), 0), |g| (g.name.clone(), g.entries.len()));
        let shown = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        self.ui.library.deleting = Some(file.to_path_buf());
        self.open_dialog(DialogKind::LibraryDelete);
        if let Some(d) = self.ui.dialog.as_mut() {
            d.subject = strings::library_delete_body(&name, &shown, notes);
        }
    }

    /// The answer to the question about deleting a library file: button 0 is "Delete".
    pub(super) fn library_delete_answer(&mut self, n: usize) {
        let file = self.ui.library.deleting.take();
        self.close_dialog();
        if let (Some(file), 0) = (file, n) {
            self.library_delete_file(&file);
        }
    }

    /// Deletes a library file with `std::fs::remove_file`, and reads the library again. Refused,
    /// with a toast, while a tab holds unsaved changes to it and for a file that is not one of
    /// the folder's own. The notes it listed are not touched.
    pub(super) fn library_delete_file(&mut self, file: &Path) -> bool {
        if let Err(why) = self.library_file_free(file) {
            self.docs.set_toast(&why);
            return false;
        }
        if !self.library_own_file(file) {
            self.docs.set_toast(strings::LIBRARY_NOT_IN_FOLDER);
            return false;
        }
        if self.library_lists_as_note(file) {
            return false;
        }
        let group = self.library_group_name(file);
        if let Err(e) = std::fs::remove_file(file) {
            self.docs
                .set_toast(&strings::library_failed(&e.to_string()));
            return false;
        }
        self.library_changed();
        self.docs.set_toast(&strings::library_deleted(&group));
        true
    }

    /// The picker answered: the note waiting for a place goes there.
    pub(super) fn library_move_to(&mut self, to: &Path, section: &[String]) {
        let Some(m) = self.ui.library.moving.take() else {
            return;
        };
        self.library_move(&m.file, m.line, &m.title, &m.note, to, section);
    }

    /// Moves the note written on `line` of `from` under the heading `section` of `to`. Within
    /// one file that is one write. Between two it is two: the new place first and the old one
    /// second, so that whatever goes wrong the note is in at least one of them; if the second
    /// cannot be made the toast says so, and the note is listed in both.
    pub(super) fn library_move(
        &mut self,
        from: &Path,
        line: usize,
        title: &str,
        note: &Path,
        to: &Path,
        section: &[String],
    ) -> bool {
        let dir = self.docs.library.dir.clone();
        if same_path(from, to) {
            let done = self.library_write(
                from,
                |t| {
                    if !note_at(t, &dir, line, note) {
                        return None;
                    }
                    // The group's own row has no section heading: the line goes in the group's
                    // own notes rather than at the end of the file, under its last section.
                    if section.is_empty() {
                        library::move_note_in_group(t, line)
                    } else {
                        library::move_note(t, line, section)
                    }
                },
                |b, a| {
                    a.entries.len() == b.entries.len()
                        && a.entries.iter().any(|e| same_path(&e.path, note))
                },
            );
            if done {
                let group = self.library_group_name(to);
                self.docs.set_toast(&strings::library_moved(title, &group));
            }
            return done;
        }
        // Both files are checked before either is written, so a refusal leaves both alone.
        for file in [from, to] {
            if let Err(why) = self.library_file_free(file) {
                self.docs.set_toast(&why);
                return false;
            }
        }
        let link = crate::library_pane::link_path(to.parent().unwrap_or(Path::new("")), note);
        let added = self.library_write(
            to,
            |t| Some(library::add_note_in_group(t, section, title, &link)),
            |b, a| a.entries.len() == b.entries.len() + 1,
        );
        if !added {
            return false;
        }
        let removed = self.library_attempt(
            from,
            |t| {
                if !note_at(t, &dir, line, note) {
                    return None;
                }
                library::remove_note(t, line)
            },
            |b, a| a.entries.len() + 1 == b.entries.len(),
        );
        let there = self.library_group_name(to);
        let here = self.library_group_name(from);
        match removed {
            Wrote::Done => {
                self.docs.set_toast(&strings::library_moved(title, &there));
                true
            }
            Wrote::Nothing => {
                self.docs.set_toast(&strings::library_move_half(
                    title,
                    &there,
                    &here,
                    "its line there had changed",
                ));
                false
            }
            Wrote::Refused(why) => {
                self.docs
                    .set_toast(&strings::library_move_half(title, &there, &here, &why));
                false
            }
        }
    }

    /// "Find the file…": a picker, then the link's target is the file chosen. A note moved in
    /// the file manager is put right this way.
    fn library_find_file(&mut self, file: &Path, line: usize, title: &str, note: &Path) {
        self.ui.library.finding = Some(Moving {
            file: file.to_path_buf(),
            line,
            title: title.to_string(),
            note: note.to_path_buf(),
        });
        #[cfg(windows)]
        {
            let start = note
                .parent()
                .filter(|p| p.is_dir())
                .map(Path::to_path_buf)
                .or_else(|| self.start_dir());
            let picked = platform::pick_files(start.as_deref()).into_iter().next();
            self.library_found(picked);
        }
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            let start = note
                .parent()
                .filter(|p| p.is_dir())
                .map(Path::to_path_buf)
                .or_else(|| self.start_dir());
            platform::pick_files_async(start.as_deref(), move |files| {
                wake(Wake::Dialog(Picked::LibraryFile(files)));
            });
        }
    }

    /// The picker for "Find the file…" answered.
    pub(super) fn library_found(&mut self, picked: Option<PathBuf>) {
        let Some(m) = self.ui.library.finding.take() else {
            return;
        };
        if let Some(path) = picked {
            self.library_set_note_path(&m.file, m.line, &m.note, &path);
        }
    }

    /// Writes `path` as the target of the link on `line`, keeping its title.
    pub(super) fn library_set_note_path(
        &mut self,
        file: &Path,
        line: usize,
        old: &Path,
        path: &Path,
    ) {
        let dir = self.docs.library.dir.clone();
        let link = crate::library_pane::link_path(file.parent().unwrap_or(Path::new("")), path);
        self.library_write(
            file,
            |t| {
                if !note_at(t, &dir, line, old) {
                    return None;
                }
                library::set_note_path(t, line, &link)
            },
            |b, a| {
                a.entries.len() == b.entries.len()
                    && a.entries
                        .iter()
                        .find(|e| e.line == line)
                        .is_some_and(|e| crate::library_pane::same_note(&e.path, path))
            },
        );
    }
}
