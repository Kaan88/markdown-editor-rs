//! "Add a folder to the library…" (`docs/ai-library-plan.md` 5.7, phase 4a) as the window hosts it:
//! `mdedit_core::import` is the walk and the plan, `import_ui.rs` is the card, `import_draw.rs`
//! draws it. The card is drawn over everything, as the split's plan card and the move card are,
//! and takes every key, pointer and wheel event while it is open; the plan card's routes in
//! `app/plan.rs` hand them here.
//!
//! What "Add the files" writes is one library file: the group's, made with its `home:` line when
//! the library has no group of that name. The links go in in one atomic write through the
//! window's own gate ([`App::library_attempt_at`]), which refuses a library file that is open with
//! unsaved changes, reads it again, checks the result and writes it at once. Nothing of the
//! owner's is copied, moved, renamed or deleted, and no record is written: an import creates no
//! file of its own (D-111).

use super::library_write::{Wrote, file_stem_for};
use super::*;
use crate::commands::strings;
use crate::import_ui::{Act as IAct, ImportCard};
use crate::library_pane::{RowId, key_of};
use crate::ui::PaneFace;
use mdedit_core::import;
use mdedit_core::runs;
use mdshared_app::library;
use mdshared_app::paths::same_path;

/// The folder a walk is reading, while its thread runs.
impl App {
    pub(super) fn import_is_open(&self) -> bool {
        self.importer.is_some()
    }

    /// Opens `card` over the window.
    pub(super) fn import_open(&mut self, mut card: ImportCard) {
        if self.palette_open() {
            self.close_palette();
        }
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.hover = None;
        self.ui.find_focused = false;
        let (w, h) = self.logical();
        card.resize(w, h);
        self.importer = Some(card);
        if self.importer.as_ref().is_some_and(|c| !c.demo) {
            self.import_ai_refresh();
        }
        self.caret_reset();
        self.redraw();
    }

    /// "Add a folder to the library…": a folder picker, then the folder is read on its own thread
    /// and the plan opens when it is.
    pub(super) fn import_choose_folder(&mut self) {
        if self.import_job.is_some() {
            self.docs
                .set_toast("A folder is being read for the library already.");
            return;
        }
        let start = self.start_dir();
        #[cfg(windows)]
        if let Some(dir) = platform::pick_folder(start.as_deref()) {
            self.import_folder_picked(dir);
        }
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            platform::pick_folder_async(start.as_deref(), move |dir| {
                wake(Wake::Dialog(Picked::ImportFolder(dir)));
            });
        }
    }

    /// The folder picker answered: the folder is walked on a thread of its own, because a folder
    /// of thousands of files is seconds of reading.
    pub(super) fn import_folder_picked(&mut self, dir: PathBuf) {
        if let Err(why) = self.import_ready(&dir) {
            self.docs.set_toast(&why);
            return;
        }
        self.docs
            .set_toast(&format!("Reading the files in {}…", import::shown(&dir)));
        let wake = self.docs.waker();
        let reading = dir.clone();
        let job = mdshared_app::job::Job::start(
            move || wake(Wake::Import),
            move |cancel, _progress| import::walk(&reading, &|| cancel.cancelled()),
        );
        match job {
            Ok(job) => {
                self.import_job = Some(job);
                self.import_dir = Some(dir);
            }
            Err(_) => {
                // A thread could not be made: read here rather than open nothing.
                let walked = import::walk(&dir, &|| false);
                self.import_begin(dir, walked);
            }
        }
    }

    /// The walk on its thread has finished: its plan opens, or the reason it cannot is said.
    pub(super) fn import_take(&mut self) {
        let Some(job) = self.import_job.as_ref() else {
            return;
        };
        let Some(walked) = job.take() else {
            return;
        };
        self.import_job = None;
        let Some(dir) = self.import_dir.take() else {
            return;
        };
        self.import_begin(dir, walked);
    }

    fn import_begin(&mut self, dir: PathBuf, walked: import::Walk) {
        match self.import_plan(&dir, walked) {
            Ok(plan) => self.import_open(ImportCard::new(plan)),
            Err(why) => self.docs.set_toast(&why),
        }
    }

    /// What a folder has to be before it is read: a library folder to write into, and a folder
    /// that is one. The rest is said on the card, which can say the 2,000 and the names.
    fn import_ready(&mut self, dir: &Path) -> Result<(), String> {
        let library = self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
            .ok_or_else(|| strings::LIBRARY_NO_FOLDER.to_string())?;
        if !dir.is_dir() {
            return Err(format!("{} is not a folder.", dir.display()));
        }
        // A7: the library folder itself would make every file in it a library file.
        if same_path(dir, &library) {
            return Err(strings::LIBRARY_HOME_IS_LIBRARY.to_string());
        }
        Ok(())
    }

    /// The plan of `walked`, over `dir`: the group the links go in (the one the library has for
    /// the folder's name, or the one this run would make), and the notes the library does not list
    /// already. The library is read here, so a group another window made a moment ago is known.
    fn import_plan(&mut self, dir: &Path, walked: import::Walk) -> Result<import::Plan, String> {
        self.import_ready(dir)?;
        let Some(library) = self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
        else {
            return Err(strings::LIBRARY_NO_FOLDER.to_string());
        };
        self.library_load();
        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.trim().is_empty() {
            return Err("That folder has no name to give the group.".to_string());
        }
        let group = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(&name))
            .cloned();
        let (group_file, entries, listed) = match &group {
            Some(g) => (g.file.clone(), g.entries.len(), import::listed(&g.entries)),
            None => {
                let Some(stem) = file_stem_for(&name) else {
                    return Err(format!(
                        "\u{201c}{name}\u{201d} cannot be a library file's name."
                    ));
                };
                (library.join(format!("{stem}.md")), 0, Vec::new())
            }
        };
        if group.is_none() && group_file.exists() {
            return Err(format!(
                "{} is in the library folder and is not one of its files. Rename it first.",
                import::shown(&group_file)
            ));
        }
        let at = import::Where {
            root: &library,
            group: group.as_ref().map(|g| (g.name.as_str(), g.file.as_path())),
            new_file: group_file,
            entries,
            listed: &listed,
        };
        Ok(import::plan(walked, dir, at))
    }

    pub(super) fn import_close(&mut self) {
        self.ai.cancel_sort();
        self.docs.ai_status = None;
        self.importer = None;
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    fn import_act(&mut self, act: IAct) {
        match act {
            IAct::Close => self.import_close(),
            IAct::Write => self.import_write(),
            IAct::SortAsk => self.import_sort_ask(),
            IAct::SortStart => self.import_sort_start(),
            IAct::SortStop => self.import_sort_stop(),
            IAct::SortBack => self.import_sort_back(),
            IAct::Settings => self.ai_open_settings(),
            IAct::Nothing => {}
        }
        self.redraw();
    }

    pub(super) fn import_key(&mut self, key: crate::keys::Key, typed: Option<&str>) {
        let mods = self.mods;
        let Some(card) = self.importer.as_mut() else {
            return;
        };
        let act = card.key(&key, mods, typed);
        self.caret_reset();
        self.import_act(act);
    }

    pub(super) fn import_pointer(&mut self, x: f32, y: f32) {
        if let Some(card) = self.importer.as_mut() {
            card.pointer(x, y);
        }
        self.redraw();
    }

    pub(super) fn import_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(card) = self.importer.as_mut() {
            card.press(x, y);
        }
        self.redraw();
    }

    pub(super) fn import_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(card) = self.importer.as_mut() else {
            return;
        };
        let act = card.release(x, y);
        self.import_act(act);
    }

    pub(super) fn import_wheel(&mut self, dy: f32) {
        if let Some(card) = self.importer.as_mut() {
            card.wheel(dy);
        }
        self.redraw();
    }

    /// "Add the files": everything that can be refused is refused first, and the card stays open
    /// with the reason; then the links are written in one go and the pane opens on the group.
    pub(super) fn import_write(&mut self) {
        let Some(mut card) = self.importer.take() else {
            return;
        };
        if card.demo {
            card.refusal =
                Some("A demonstration of the import card: it writes nothing.".to_string());
            card.tick += 1;
            self.importer = Some(card);
            return;
        }
        match self.import_links(&card.plan) {
            Ok(added) => {
                let name = card.plan.group_name.clone();
                let file = card.plan.group_file.clone();
                let first = card
                    .plan
                    .notes
                    .iter()
                    .find(|n| n.ticked)
                    .map(|n| (n.path.clone(), n.sections.clone()));
                self.docs.set_toast(&format!(
                    "{} in the library under \u{201c}{name}\u{201d}.",
                    if added == 1 {
                        "1 file is".to_string()
                    } else {
                        format!("{added} files are")
                    }
                ));
                self.ui.tree.refresh();
                self.library_rows();
                if let Some((path, sections)) = first {
                    self.open_imported(&file, &sections, &path);
                }
            }
            Err(why) => {
                self.docs.set_toast(&why);
                card.refusal = Some(why);
                card.tick += 1;
                self.importer = Some(card);
            }
        }
        self.redraw();
    }

    /// The links of a plan written into the group's library file, in one atomic write. `Err` is
    /// what the card says, with nothing written but the group's own file taken away again if this
    /// run made it.
    fn import_links(&mut self, plan: &import::Plan) -> Result<usize, String> {
        if let Some(why) = plan.problem() {
            return Err(why);
        }
        let library = plan.library.clone();
        if library.as_os_str().is_empty() || !library.is_absolute() {
            return Err(strings::LIBRARY_NO_FOLDER.to_string());
        }
        self.library_file_free(&plan.group_file)?;
        let links = plan.links();
        let group_text = if plan.group_new {
            if plan.group_file.exists() {
                return Err(format!(
                    "{} is in the library folder already.",
                    import::shown(&plan.group_file)
                ));
            }
            if super::plan_write::library_files(&library) >= library::MAX_FILES {
                return Err(format!(
                    "The library already holds its {} files.",
                    library::MAX_FILES
                ));
            }
            let text = plan
                .group_text()
                .ok_or_else(|| "The group's name cannot be a heading.".to_string())?;
            let read = library::parse(&text, &library);
            if read.name.as_deref().map(str::trim) != Some(plan.group_name.trim())
                || read.home.is_none()
            {
                return Err(format!(
                    "{} would not read back as its notes.",
                    import::shown(&plan.group_file)
                ));
            }
            Some(text)
        } else {
            match std::fs::read_to_string(&plan.group_file) {
                Ok(text) => Some(text),
                Err(e) => {
                    return Err(format!(
                        "{} could not be read: {e}",
                        import::shown(&plan.group_file)
                    ));
                }
            }
        };
        // The dry run, before the library file is made: the links exactly as the write would put
        // them, against the file as it reads now.
        let before = group_text.clone().unwrap_or_default();
        let new = runs::with_links_added(&before, &links);
        let old = library::parse(&before, &library);
        if !runs::check_links_added(&old, &library::parse(&new, &library), &links, &library) {
            return Err(format!(
                "{} would not read back as its old notes plus the new ones.",
                import::shown(&plan.group_file)
            ));
        }
        let made = if plan.group_new {
            let text = group_text.clone().unwrap_or_default();
            if let Err(e) = std::fs::create_dir_all(&library)
                .and_then(|()| mdshared_app::files::create_new(&plan.group_file, text.as_bytes()))
            {
                return Err(format!(
                    "{} could not be made: {e}",
                    import::shown(&plan.group_file)
                ));
            }
            Some(text)
        } else {
            None
        };
        let wrote = self.library_attempt_at(
            &plan.group_file,
            &library,
            |text| Some(runs::with_links_added(text, &links)),
            |old, new| runs::check_links_added(old, new, &links, &library),
        );
        match wrote {
            Wrote::Done => Ok(links.len()),
            Wrote::Nothing => {
                remove_own_file(&plan.group_file, made.as_deref());
                Err("The library file was not changed.".to_string())
            }
            Wrote::Refused(why) => {
                remove_own_file(&plan.group_file, made.as_deref());
                Err(why)
            }
        }
    }

    /// The pane opens on the group the import wrote into, its cursor on the first note added.
    fn open_imported(&mut self, file: &Path, sections: &[String], note: &Path) {
        self.ui.panes.files = true;
        self.ui.set_pane_face(PaneFace::Notes);
        self.ui.library.unfold_to(file, sections);
        self.ui.library.build(&self.docs.library);
        self.ui
            .library
            .follow(RowId::Note(key_of(file, sections), note.to_path_buf()));
    }

    /// A scheme name being typed takes the input method's composing text, like the plan's.
    pub(super) fn import_preedit(&mut self, pre: Option<mdshared_ui::textfield::Preedit>) -> bool {
        let Some(card) = self.importer.as_mut() else {
            return false;
        };
        card.tick += 1;
        card.edit.as_mut().is_some_and(|f| f.set_preedit(pre))
    }

    /// `--plan-demo folder…`: the card over a plan made in the program. Nothing is read or
    /// written, and the card refuses to add.
    pub(super) fn import_demo(&mut self, name: &str) -> bool {
        match crate::import_ui::demo(name) {
            Some(card) => {
                self.import_open(card);
                true
            }
            None => false,
        }
    }
}

/// Takes away a file this write made, when it still holds exactly what was put in it.
fn remove_own_file(path: &Path, text: Option<&str>) {
    let Some(text) = text else {
        return;
    };
    if std::fs::read(path).is_ok_and(|bytes| bytes == text.as_bytes()) {
        let _ = std::fs::remove_file(path);
    }
}
