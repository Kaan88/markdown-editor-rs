//! The AI task window over files as the window hosts it (`docs/ai-library-plan.md` 5.6, phase
//! W). The window's state and layout are `task_ui.rs`, its drawing `task_draw.rs`. Here are the
//! three tasks: File them (links into a group), Split them (each file's plan card, cut at its
//! headings or by the AI) and Rewrite them (each file's changes reviewed with 5.5's list, then
//! written in place with its original kept in the run's record, D-074 and D-076).
//!
//! Nothing the window is given is opened in a tab. Requests go to the engine only while the
//! window is; closing it stops one in flight. The model never sees a path: it sees names and
//! numbers, and returns numbers.

use super::*;
use crate::ai::{Ask, FileReview, Panel, Review, Spec};
use crate::keys::KeyName;
use crate::task_ui::{
    self, Asking, DestChoice, FileEntry, FileKind, Focus, Group, Hit, TaskKind, TaskScreen,
};
use mdedit_core::ai::{self as core_ai, Connection};
use mdedit_core::runs::{self, Sent};
use serde_json::json;

impl App {
    /// Whether the task window is open.
    pub(super) fn task_is_open(&self) -> bool {
        self.task.is_some()
    }

    /// ""AI task…" and a tab's "AI task on this file…": the window, with `add` in it.
    /// A window parked by "Show what is sent" comes back as it was, its task with it: `kind` is
    /// for a new window only (audit 4, B7).
    pub(super) fn task_open(&mut self, kind: TaskKind, add: Vec<PathBuf>) {
        self.library_load();
        if let Some(mut t) = self.task_parked.take() {
            let before = t.limits.rewrite_bytes;
            t.limits = task_ui::Limits::of(&self.docs.prefs);
            t.recount_limit();
            t.tick += 1;
            self.task = Some(t);
            self.task_read_again(before);
            self.task_add_paths(add);
            self.task_finish_open();
            return;
        }
        let Some(root) = self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
        else {
            self.docs
                .set_toast(crate::commands::strings::LIBRARY_NO_FOLDER);
            return;
        };
        let groups: Vec<Group> = self
            .docs
            .library
            .groups
            .iter()
            .map(|g| Group {
                name: g.name.clone(),
                file: g.file.clone(),
                home: g
                    .home
                    .clone()
                    .unwrap_or_else(|| root.join(mdshared_app::library::folder_name(&g.name))),
            })
            .collect();
        let dest = if let Some(first) = (groups.len() == 1).then_some(0) {
            DestChoice::Group(first)
        } else if groups.is_empty() {
            DestChoice::New(String::new())
        } else {
            DestChoice::Decide
        };
        let prompts = match kind {
            TaskKind::File => self.docs.ai_file_prompts.clone(),
            TaskKind::Split => self.docs.ai_split_prompts.clone(),
            TaskKind::Rewrite => self.docs.ai_prompts.clone(),
        };
        let mut s = TaskScreen::new(kind, groups, dest, prompts, self.ai_via());
        self.task_seq += 1;
        s.id = self.task_seq;
        s.ai_on = self.ai_conn() != Connection::Off;
        s.win = self.logical();
        s.scale = self.scale;
        if matches!(s.dest, DestChoice::New(ref name) if name.is_empty()) {
            s.new_group = Some(mdshared_ui::textfield::TextField::new());
            s.focus = Focus::NewGroup;
        }
        s.limits = task_ui::Limits::of(&self.docs.prefs);
        self.task = Some(s);
        self.task_add_paths(add);
        self.task_finish_open();
    }

    /// A window brought back after the largest rewrite changed from `before`: a text file it
    /// kept without its text (too long then) is read again, so its row says what it is now
    /// (review of phase O, 8).
    fn task_read_again(&mut self, before: usize) {
        let again: Vec<(usize, PathBuf)> = self
            .task
            .as_ref()
            .filter(|s| s.limits.rewrite_bytes != before)
            .map(|s| {
                s.files
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.kind == FileKind::Text && f.text.is_none() && !f.done)
                    .map(|(i, f)| (i, f.path.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let probed: Vec<(usize, FileEntry)> = again
            .into_iter()
            .map(|(i, p)| (i, self.task_probe(&p)))
            .collect();
        if let Some(s) = self.task.as_mut() {
            for (i, new) in probed {
                if let Some(f) = s.files.get_mut(i) {
                    f.text = new.text;
                    f.why = new.why;
                    f.size = new.size;
                    f.eol = new.eol;
                    f.bom = new.bom;
                    f.sample = new.sample;
                    f.heading = new.heading;
                }
            }
        }
    }

    fn task_finish_open(&mut self) {
        self.close_for_ai_panel();
        // The limits as the settings say now (D-203), for a window opened or brought back.
        let limits = task_ui::Limits::of(&self.docs.prefs);
        let files_go = self.docs.prefs.ai_files;
        let library = self.docs.library.dir.clone();
        if let Some(s) = self.task.as_mut() {
            s.limits = limits;
            s.files_go = files_go;
            s.library = library;
            s.recount_limit();
            s.tick += 1;
        }
        self.caret_reset();
        self.redraw();
    }

    /// Adds paths to the open window, each probed once. A path already in it is not added twice,
    /// and a folder dropped on it is walked one level.
    pub(super) fn task_add_paths(&mut self, paths: Vec<PathBuf>) {
        let Some(s) = self.task.as_mut() else {
            return;
        };
        if !paths.is_empty() {
            // The header's "from the Notes pane" is no longer the whole of it.
            s.from = None;
        }
        let mut fresh: Vec<FileEntry> = Vec::new();
        for path in paths {
            let mut list = Vec::new();
            if path.is_dir() {
                if let Ok(read) = std::fs::read_dir(&path) {
                    for e in read.flatten() {
                        if e.path().is_file() {
                            list.push(e.path());
                        }
                    }
                }
            } else {
                list.push(path);
            }
            for p in list {
                let known = self.task.as_ref().is_some_and(|s| {
                    s.files
                        .iter()
                        .chain(fresh.iter())
                        .any(|f| mdshared_app::paths::same_path(&f.path, &p))
                });
                if known {
                    continue;
                }
                fresh.push(self.task_probe(&p));
            }
        }
        if let Some(s) = self.task.as_mut() {
            s.files.extend(fresh);
            s.recount_limit();
            s.tick += 1;
        }
        self.redraw();
    }

    /// What a file is to a task: text, a picture or another kind, and the reason it is left out
    /// when it is. A picture's bytes are never read; nor a PDF's or a Word file's.
    fn task_probe(&self, path: &Path) -> FileEntry {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let mut out = FileEntry {
            path: path.to_path_buf(),
            name,
            size: 0,
            kind: FileKind::Other,
            why: None,
            ticked: true,
            text: None,
            bom: false,
            sample: String::new(),
            heading: None,
            title: None,
            sections: Vec::new(),
            with: None,
            over_limit: false,
            limited: false,
            eol: "\n".to_string(),
            modified: None,
            already: false,
            done: false,
            limits: task_ui::Limits::of(&self.docs.prefs),
        };
        let meta = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) => {
                out.why = Some(format!("could not be read: {e}"));
                return out;
            }
        };
        if !meta.is_file() {
            out.why = Some("not a file".to_string());
            return out;
        }
        out.size = meta.len();
        out.modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| task_ui::date_of(d.as_secs()));
        out.already = self
            .docs
            .library
            .entries()
            .any(|(_, e)| crate::library_pane::same_note(&e.path, path));
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if task_ui::PICTURES.contains(&ext.as_str()) {
            out.kind = FileKind::Picture;
            return out;
        }
        if task_ui::other_reason(&ext).is_some() {
            out.why = task_ui::other_reason(&ext).map(str::to_string);
            return out;
        }
        if out.size == 0 {
            out.why = Some("empty".to_string());
            return out;
        }
        if out.size > task_ui::ADD_MAX {
            out.why = Some(format!(
                "too large for this window: {} of {} MB",
                task_ui::size_text(out.size),
                task_ui::ADD_MAX / (1024 * 1024)
            ));
            return out;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                out.why = Some(format!("could not be read: {e}"));
                return out;
            }
        };
        let loaded = mdedit_core::load::load_bytes(&bytes, &self.docs.load_options());
        if loaded.info.has_nul || loaded.info.lossy {
            out.why = Some("not a text file".to_string());
            return out;
        }
        let all = loaded.buffer.rope().to_string();
        out.kind = FileKind::Text;
        out.sample = all.chars().take(task_ui::DIGEST_BYTES).collect();
        out.heading = task_ui::first_heading(&all);
        out.eol = loaded.info.eol.dominant().to_string();
        let utf8 = matches!(
            loaded.info.encoding,
            mdedit_core::encoding::Encoding::Utf8 | mdedit_core::encoding::Encoding::Utf8Bom
        );
        out.bom = loaded.info.encoding == mdedit_core::encoding::Encoding::Utf8Bom;
        if utf8 && bytes.len() <= out.limits.rewrite_bytes {
            out.text = Some(all);
        }
        out
    }

    /// "AI task on this note / these N notes…" (pictures plan 4.2): the window on the notes
    /// under pane row `i`, folded or not, on Rewrite them, saying where they came from.
    pub(super) fn task_from_library(&mut self, i: usize) {
        let Some(row) = self.ui.library.rows.get(i).cloned() else {
            return;
        };
        let notes = crate::library_pane::notes_under(&self.docs.library, &row);
        if notes.is_empty() {
            self.docs
                .set_toast("There is no note under it with a file to work on.");
            self.redraw();
            return;
        }
        // A window parked by "Show what is sent" comes back as it was, its task, files and
        // plan with it (audit 4, B7): the notes are not put into it.
        if self.task_parked.is_some() {
            self.task_open(TaskKind::Rewrite, Vec::new());
            self.docs.set_toast(
                "The AI task window you left is back as it was. Close it to start one on these \
                 notes.",
            );
            self.redraw();
            return;
        }
        let from = self.ui.library.came_from(i, notes.len());
        self.task_open(TaskKind::Rewrite, notes);
        if self.task.as_ref().is_none_or(|s| s.asking.is_some()) {
            return;
        }
        self.task_act(Hit::Kind(TaskKind::Rewrite));
        if let Some(s) = self.task.as_mut() {
            s.from = Some(from);
            s.tick += 1;
        }
        self.redraw();
    }

    /// "AI task on this file…": the window with the tab's file, on File them, or on Rewrite
    /// them when the library lists the file already (pictures plan 4.1: a note of the library is
    /// filed; what is left is to work on it). A parked window comes back on its own task.
    pub(super) fn task_open_on_file(&mut self, path: PathBuf) {
        let listed = self
            .docs
            .library
            .entries()
            .any(|(_, e)| crate::library_pane::same_note(&e.path, &path));
        let parked = self.task_parked.is_some();
        let kind = if listed {
            TaskKind::Rewrite
        } else {
            TaskKind::File
        };
        self.task_open(kind, vec![path]);
        if listed && !parked && self.task.as_ref().is_some_and(|s| s.asking.is_none()) {
            self.task_act(Hit::Kind(TaskKind::Rewrite));
        }
    }

    /// The row of the Notes pane the keyboard is on, when the pane has the keyboard: "AI
    /// task…" then works on it (pictures plan 4.2).
    pub(super) fn library_row_has_notes(&self, i: usize) -> bool {
        self.ui.library.rows.get(i).is_some_and(|row| {
            !crate::library_pane::notes_under(&self.docs.library, row).is_empty()
        })
    }

    pub(super) fn library_keyboard_row(&self) -> Option<usize> {
        let pane = &self.ui.library;
        if !(self.ui.panes.files && pane.on && pane.focused) {
            return None;
        }
        pane.cur.filter(|&i| {
            pane.rows
                .get(i)
                .is_some_and(|r| r.kind != crate::library_pane::Kind::Editing)
        })
    }

    /// "Add…": the menu of what can be added (D-207): each open tab's file by name, the
    /// document in front first and said so, those already in the window left out; then
    /// "Files…". The document in front is never added by itself.
    pub(super) fn task_add_menu(&mut self) {
        let active = self.docs.active();
        let mut tabs: Vec<(bool, PathBuf, String)> = self
            .docs
            .tabs
            .iter()
            .enumerate()
            .filter_map(|(i, d)| d.path.clone().map(|p| (Some(i) == active, p, d.name())))
            .collect();
        tabs.sort_by_key(|(front, _, _)| !*front);
        let Some(s) = self.task.as_mut() else { return };
        // As many tabs as the card has room for under the button, the one in front first, and
        // "Files…" always last and on screen (review of phase E, 5).
        let lines = task_ui::add_menu_lines(s.win.0, s.win.1);
        let mut items: Vec<task_ui::AddItem> = tabs
            .into_iter()
            .filter(|(_, p, _)| {
                !s.files
                    .iter()
                    .any(|f| crate::library_pane::same_note(&f.path, p))
            })
            .map(|(front, path, name)| task_ui::AddItem::Tab { path, name, front })
            .take(lines.saturating_sub(1))
            .collect();
        items.push(task_ui::AddItem::Files);
        s.add_items = items;
        s.add_open = !s.add_open;
        s.dest_open = false;
        s.tick += 1;
        self.redraw();
    }

    /// "Files…": the file dialog, then the same path as a drop.
    pub(super) fn task_pick_files(&mut self) {
        let start = self.docs.active_doc().and_then(|d| d.path.clone());
        let start = start.as_deref().and_then(Path::parent);
        // Windows opens the dialog here; the other systems open it on a thread, so its answer
        // comes back as `Picked::TaskFiles` (`App::dialog_done`).
        #[cfg(windows)]
        self.task_add_paths(platform::pick_files(start));
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            let start = start.map(Path::to_path_buf);
            platform::pick_files_async(start.as_deref(), move |files| {
                wake(Wake::Dialog(Picked::TaskFiles(files)));
            });
        }
    }

    /// The window's keys. It takes every key while it is open, as the plan screen does.
    pub(super) fn task_key(&mut self, name: Option<KeyName>, typed: Option<&str>) {
        let Some(name) = name else { return };
        let mods = self.mods;
        // Escape stops a request first, then closes a list, then the window.
        if name == KeyName::Escape {
            if let Some(s) = self.task.as_mut()
                && s.asking.is_some()
            {
                self.task_cancel_request();
                return;
            }
            if self
                .task
                .as_ref()
                .is_some_and(|s| s.dest_open || s.add_open)
            {
                if let Some(s) = self.task.as_mut() {
                    s.dest_open = false;
                    s.add_open = false;
                    s.tick += 1;
                }
                self.redraw();
                return;
            }
            self.task_close();
            return;
        }
        if let Some(s) = self.task.as_mut()
            && s.add_open
        {
            s.add_open = false;
            s.tick += 1;
        }
        if self.task.as_ref().is_some_and(|s| s.asking.is_some()) {
            return;
        }
        if self.task.as_ref().is_some_and(|s| s.dest_open) {
            self.task_dest_key(&name, mods);
            return;
        }
        if name == KeyName::PageDown || name == KeyName::PageUp {
            if let Some(s) = self.task.as_mut() {
                let dy = if name == KeyName::PageDown {
                    160.0
                } else {
                    -160.0
                };
                s.list.scroll_by(dy);
                s.tick += 1;
            }
            self.redraw();
            return;
        }
        if name == KeyName::Tab {
            if let Some(s) = self.task.as_mut() {
                s.focus = match s.focus {
                    Focus::Instruction if s.new_group.is_some() => Focus::NewGroup,
                    Focus::Instruction if s.task == TaskKind::File => Focus::Section,
                    Focus::Section | Focus::NewGroup => Focus::Instruction,
                    _ => Focus::Instruction,
                };
                s.tick += 1;
            }
            self.redraw();
            return;
        }
        if name == KeyName::Enter {
            let focus = self.task.as_ref().map(|s| s.focus);
            if focus == Some(Focus::NewGroup) {
                self.task_new_group_commit();
            } else {
                self.task_run();
            }
            return;
        }
        let Some(s) = self.task.as_mut() else { return };
        s.tick += 1;
        match s.focus {
            Focus::Instruction => {
                if s.instruction.handle_key(&name, mods, typed, true) == Some(true) {
                    s.prompt_sel = None;
                }
            }
            Focus::Section => {
                s.section.handle_key(&name, mods, typed, true);
            }
            Focus::NewGroup => {
                if let Some(f) = s.new_group.as_mut() {
                    f.handle_key(&name, mods, typed, true);
                }
            }
        }
        self.caret_reset();
        self.redraw();
    }

    fn task_dest_key(&mut self, name: &KeyName, mods: crate::keys::Mods) {
        let _ = mods;
        let Some(s) = self.task.as_mut() else { return };
        let items = s.groups.len() + 2;
        let cur = s.dest_hot.unwrap_or(0);
        match name {
            KeyName::Up => s.dest_hot = Some(cur.saturating_sub(1)),
            KeyName::Down => s.dest_hot = Some((cur + 1).min(items.saturating_sub(1))),
            KeyName::Enter | KeyName::Space => {
                let k = s.dest_hot.unwrap_or(0);
                s.dest_open = false;
                self.task_choose_dest(k);
            }
            KeyName::Escape => s.dest_open = false,
            _ => {}
        }
        if let Some(s) = self.task.as_mut() {
            s.tick += 1;
        }
        self.redraw();
    }

    fn task_choose_dest(&mut self, k: usize) {
        let Some(s) = self.task.as_mut() else { return };
        if k < s.groups.len() {
            s.dest = DestChoice::Group(k);
            s.new_group = None;
            s.focus = Focus::Instruction;
        } else if k == s.groups.len() {
            s.dest = DestChoice::New(String::new());
            s.new_group = Some(mdshared_ui::textfield::TextField::new());
            s.focus = Focus::NewGroup;
        } else {
            s.dest = DestChoice::Decide;
            s.new_group = None;
            s.focus = Focus::Instruction;
        }
        s.tick += 1;
        self.caret_reset();
        self.redraw();
    }

    /// Enter on the new group's name: the group is made in the library folder, and chosen.
    pub(super) fn task_new_group_commit(&mut self) {
        let Some(name) = self
            .task
            .as_ref()
            .and_then(|s| s.new_group.as_ref())
            .map(|f| f.text().trim().to_string())
            .filter(|n| !n.is_empty())
        else {
            return;
        };
        let Some(root) = self.library_dir() else {
            return;
        };
        let Some(stem) = super::library_write::file_stem_for(&name) else {
            self.docs
                .set_toast(&crate::commands::strings::library_not_a_name(&name));
            return;
        };
        let Some(text) = mdshared_app::library::new_group_with_home(&name, Some(Path::new(&stem)))
        else {
            return;
        };
        let file = root.join(format!("{stem}.md"));
        if file.exists() {
            self.docs
                .set_toast(&crate::commands::strings::library_exists(&format!(
                    "{stem}.md"
                )));
            return;
        }
        if let Err(e) = std::fs::create_dir_all(&root)
            .and_then(|()| mdshared_app::files::create_new(&file, text.as_bytes()))
        {
            self.docs
                .set_toast(&crate::commands::strings::library_failed(&e.to_string()));
            return;
        }
        self.library_load();
        let Some(s) = self.task.as_mut() else { return };
        s.groups.push(Group {
            name: name.trim().to_string(),
            file,
            home: root.join(mdshared_app::library::folder_name(&name)),
        });
        s.dest = DestChoice::Group(s.groups.len() - 1);
        s.new_group = None;
        s.focus = Focus::Instruction;
        s.tick += 1;
        self.redraw();
    }

    pub(super) fn task_pointer(&mut self, x: f32, y: f32) {
        if self.task_drag_thumb(y) {
            return;
        }
        let hit = self
            .task
            .as_ref()
            .and_then(|s| s.hit_at(x, y))
            .filter(|h| !matches!(h, Hit::Card | Hit::Body));
        let text = self
            .task
            .as_ref()
            .and_then(|s| s.hit_at(x, y))
            .is_some_and(|h| matches!(h, Hit::Field | Hit::Section | Hit::NewGroupField));
        self.set_cursor(if text {
            CursorIcon::Text
        } else {
            CursorIcon::Default
        });
        if let Some(s) = self.task.as_mut()
            && s.hover != hit
        {
            s.hover = hit;
            s.tick += 1;
            self.redraw();
        }
    }

    pub(super) fn task_press(&mut self) {
        let (x, y) = self.ui.mouse;
        let hit = self.task.as_ref().and_then(|s| s.hit_at(x, y));
        let mut dragged = false;
        if let Some(s) = self.task.as_mut() {
            s.pressed = hit.filter(|h| !matches!(h, Hit::Card | Hit::Body | Hit::Thumb));
            // The list's scrollbar: a press on the thumb takes it, one on the track brings it
            // there and takes it.
            if hit == Some(Hit::Track)
                && let Some((_, thumb)) = s.list.bar()
            {
                s.list.grab = Some(if thumb.contains(x, y) {
                    y - thumb.y
                } else {
                    thumb.h / 2.0
                });
                s.pressed = None;
                dragged = true;
            }
            s.tick += 1;
        }
        if dragged {
            self.task_drag_thumb(y);
        }
        self.redraw();
    }

    /// A drag of the list's scrollbar thumb: the list goes where the thumb is.
    fn task_drag_thumb(&mut self, y: f32) -> bool {
        let Some(s) = self.task.as_mut() else {
            return false;
        };
        let Some(grab) = s.list.grab else {
            return false;
        };
        if let Some((track, thumb)) = s.list.bar() {
            let to = mdshared_ui::scroll::offset_for_thumb(
                track,
                thumb.h,
                y - grab,
                s.list.content,
                s.list.view,
            );
            s.list.scroll = to.clamp(0.0, s.list.max_scroll());
        }
        s.tick += 1;
        self.redraw();
        true
    }

    pub(super) fn task_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(s) = self.task.as_mut() else { return };
        s.list.grab = None;
        let Some(pressed) = s.pressed.take() else {
            return;
        };
        if s.hit_at(x, y) != Some(pressed) {
            return;
        }
        self.task_act(pressed);
    }

    pub(super) fn task_act(&mut self, hit: Hit) {
        // While a request is out the window takes no key that changes what it asked about, and
        // no click either: the task, the files, the prompt and the group stay as they were sent
        // (audit 4, B5). Stop, Cancel and the scrollbars still work.
        let asking = self.task.as_ref().is_some_and(|s| s.asking.is_some());
        if asking
            && matches!(
                hit,
                Hit::Kind(_)
                    | Hit::File(_)
                    | Hit::Prompt(_)
                    | Hit::Dest
                    | Hit::DestItem(_)
                    | Hit::AddFiles
                    | Hit::AddItem(_)
                    | Hit::Section
                    | Hit::Field
                    | Hit::NewGroupField
                    | Hit::FilesGo(_)
            )
        {
            return;
        }
        // A click anywhere but the Add menu closes it.
        if !matches!(hit, Hit::AddItem(_) | Hit::AddFiles)
            && let Some(s) = self.task.as_mut()
        {
            s.add_open = false;
        }
        match hit {
            Hit::AddFiles => self.task_add_menu(),
            Hit::AddItem(k) => {
                let item = self.task.as_mut().and_then(|s| {
                    s.add_open = false;
                    s.tick += 1;
                    s.add_items.get(k).cloned()
                });
                match item {
                    Some(task_ui::AddItem::Tab { path, .. }) => self.task_add_paths(vec![path]),
                    Some(task_ui::AddItem::Files) => self.task_pick_files(),
                    None => {}
                }
                self.redraw();
            }
            Hit::Kind(k) => {
                let task = k;
                if let Some(s) = self.task.as_mut() {
                    s.set_task(task);
                    s.prompts = task.prompts_of(
                        &self.docs.ai_file_prompts,
                        &self.docs.ai_split_prompts,
                        &self.docs.ai_prompts,
                    );
                    s.focus = Focus::Instruction;
                }
                self.caret_reset();
                self.redraw();
            }
            Hit::File(i) => {
                if let Some(s) = self.task.as_mut() {
                    s.toggle(i);
                }
                self.redraw();
            }
            Hit::Prompt(k) => {
                let prompt = self.task.as_ref().and_then(|s| s.prompts.get(k)).cloned();
                if let (Some(s), Some(p)) = (self.task.as_mut(), prompt) {
                    s.instruction.set_text(p.instruction.trim());
                    s.prompt_sel = Some(k);
                    s.focus = Focus::Instruction;
                    s.tick += 1;
                }
                self.caret_reset();
                self.redraw();
            }
            Hit::FilesGo(go) => {
                // THE FILES (D-209): the window's, and remembered once changed.
                if let Some(s) = self.task.as_mut() {
                    s.files_go = go;
                    s.tick += 1;
                }
                if self.docs.prefs.ai_files != go {
                    let mut p = self.docs.prefs.clone();
                    p.ai_files = go;
                    self.set_prefs(p);
                }
                self.redraw();
            }
            Hit::Dest => {
                if let Some(s) = self.task.as_mut() {
                    s.dest_open = !s.dest_open;
                    s.dest_hot = None;
                    s.tick += 1;
                }
                self.redraw();
            }
            Hit::DestItem(k) => {
                if let Some(s) = self.task.as_mut() {
                    s.dest_open = false;
                }
                self.task_choose_dest(k);
            }
            Hit::Section => {
                if let Some(s) = self.task.as_mut() {
                    s.focus = Focus::Section;
                    s.tick += 1;
                }
                self.caret_reset();
                self.redraw();
            }
            Hit::Field => {
                if let Some(s) = self.task.as_mut() {
                    s.focus = Focus::Instruction;
                    s.tick += 1;
                }
                self.caret_reset();
                self.redraw();
            }
            Hit::NewGroupField => {
                if let Some(s) = self.task.as_mut() {
                    s.focus = Focus::NewGroup;
                    s.tick += 1;
                }
                self.caret_reset();
                self.redraw();
            }
            Hit::ShowSent => self.task_show_sent(),
            Hit::Primary => self.task_primary(),
            Hit::Cancel => self.task_close(),
            Hit::OpenSettings => self.ai_open_settings(),
            Hit::Card | Hit::Body | Hit::Track | Hit::Thumb => {}
        }
    }

    /// `dy` in logical px, positive towards the end.
    pub(super) fn task_wheel(&mut self, dy: f32) {
        let (x, y) = self.ui.mouse;
        let Some(s) = self.task.as_mut() else { return };
        let g = task_ui::geo(task_ui::card(s.win.0, s.win.1));
        if g.prompts.contains(x, y) {
            s.prompt_scroll = (s.prompt_scroll + dy).max(0.0);
        } else {
            s.list.scroll_by(dy);
        }
        s.tick += 1;
        self.redraw();
    }

    /// What an input method composes goes to the field the keyboard is in.
    pub(super) fn task_preedit(&mut self, pre: Option<mdshared_ui::textfield::Preedit>) -> bool {
        let Some(s) = self.task.as_mut() else {
            return false;
        };
        s.tick += 1;
        match s.focus {
            Focus::Instruction => s.instruction.set_preedit(pre),
            Focus::Section => s.section.set_preedit(pre),
            Focus::NewGroup => s.new_group.as_mut().is_some_and(|f| f.set_preedit(pre)),
        }
    }

    pub(super) fn task_right_press(&mut self) {
        if let Some(s) = self.task.as_mut() {
            s.dest_open = false;
            s.tick += 1;
        }
        self.redraw();
    }

    // ---- the three tasks ------------------------------------------------------------------------

    pub(super) fn task_run(&mut self) {
        let on = self.ai_conn() != Connection::Off;
        let Some(s) = self.task.as_mut() else { return };
        if s.asking.is_some() {
            return;
        }
        s.ai_on = on;
        s.blocked = None;
        let task = s.task;
        let count = s.count();
        let empty = s.instruction.text().trim().is_empty();
        let id = s.id;
        // A kept review is shown again: while the window is open, AI › Show the last result
        // cannot be reached (audit 4, B10). The run goes on once it is answered.
        if let Some(kept) = self.ai.kept.as_ref() {
            if kept.file.as_ref().is_none_or(|fr| fr.window != id) {
                self.docs.set_toast(
                    "A review was kept from before: Apply, Copy result or Discard it, then Run.",
                );
            }
            self.ai_show_kept();
            return;
        }
        if count == 0 {
            return;
        }
        // The editor's own rewrite refuses an empty box; so does this (audit 4, B14).
        if task == TaskKind::Rewrite && on && empty {
            if let Some(s) = self.task.as_mut() {
                s.blocked = Some(task_ui::REWRITE_NEEDS_INSTRUCTION.to_string());
                s.tick += 1;
            }
            self.redraw();
            return;
        }
        match task {
            TaskKind::File if on => self.task_file_start(),
            TaskKind::File => self.task_file_plan_now(),
            TaskKind::Split => self.task_split_start(),
            TaskKind::Rewrite if !on => self.ai_open_settings(),
            TaskKind::Rewrite => self.task_rewrite_next(),
        }
    }

    /// "File them" with the AI off: every file under the section, composed now.
    fn task_file_plan_now(&mut self) {
        let Some(s) = self.task.as_mut() else { return };
        let section = task_ui::clean_name(s.section.text(), 64);
        let section = if section.is_empty() {
            Vec::new()
        } else {
            vec![mdshared_app::library::folder_name(&section)]
        };
        for &i in &s.taking_part() {
            let f = &mut s.files[i];
            f.title = Some(task_ui::default_title(f));
            f.sections = section.clone();
        }
        s.planned = true;
        s.tick += 1;
        self.redraw();
    }

    /// "Run on N files": "File them"'s batches, one request at a time, in order.
    fn task_file_start(&mut self) {
        let Some(s) = self.task.as_mut() else { return };
        let ids = s.taking_part();
        if ids.is_empty() {
            return;
        }
        s.batch_queue = ids
            .chunks(s.limits.batch.max(1))
            .map(<[usize]>::to_vec)
            .collect();
        s.answers.clear();
        s.planned = false;
        s.tick += 1;
        self.task_send_batch();
    }

    fn task_send_batch(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        let Some(batch) = s.batch_queue.first().cloned() else {
            if let Some(s) = self.task.as_mut() {
                s.asking = None;
            }
            self.task_merge_answers();
            return;
        };
        let instruction = s.instruction.text().trim().to_string();
        let prompt = task_ui::filing_prompt(&instruction, &batch, &s.files);
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": task_ui::FILING_SYSTEM,
            "prompt": prompt,
        });
        let wake = self.docs.waker();
        let what = format!(
            "Asking {} \u{00B7} {} file{}\u{2026}",
            self.ai_via(),
            batch.len(),
            if batch.len() == 1 { "" } else { "s" }
        );
        match self.ai.ask(&wake, Ask::Task, req) {
            Ok(id) => {
                self.task_sent.bytes += prompt.len() as u64;
                self.task_sent.requests += 1;
                if let Some(s) = self.task.as_mut() {
                    s.asking = Some(Asking {
                        task: s.task,
                        request: id,
                        what,
                        chars: 0,
                        index: 0,
                        prompt,
                        batch,
                    });
                    s.blocked = None;
                    s.tick += 1;
                }
            }
            Err(why) => {
                if let Some(s) = self.task.as_mut() {
                    s.blocked = Some(why);
                    s.tick += 1;
                }
            }
        }
        self.redraw();
    }

    /// The answers are in: each file takes its title and section, and the window shows the plan.
    fn task_merge_answers(&mut self) {
        let Some(s) = self.task.as_mut() else { return };
        for f in s.files.iter_mut() {
            f.title = None;
            f.sections.clear();
            f.with = None;
        }
        let answers = std::mem::take(&mut s.answers);
        for (ids, filing) in answers {
            for placed in filing.placed {
                let Some(&at) = ids.get(placed.id) else {
                    continue;
                };
                let Some(f) = s.files.get_mut(at) else {
                    continue;
                };
                f.title = placed.title;
                f.sections = placed.sections;
                f.with = placed.with.and_then(|w| ids.get(w).copied());
            }
        }
        // A picture or another file placed with a text one takes that file's section.
        for i in 0..s.files.len() {
            let Some(with) = s.files[i].with else {
                continue;
            };
            let sections = s
                .files
                .get(with)
                .map(|t| t.sections.clone())
                .unwrap_or_default();
            if s.files[i].sections.is_empty() {
                s.files[i].sections = sections;
            }
        }
        s.leave_out_unplaced();
        s.planned = true;
        s.asking = None;
        s.tick += 1;
        self.redraw();
    }

    /// The model's answer to one "File them" request.
    fn task_file_answer(&mut self, text: &str) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(asking) = s.asking.take() else {
            return;
        };
        match task_ui::read_filing(text, &asking.batch, &s.files) {
            Ok(filing) => {
                s.answers.push((asking.batch.clone(), filing));
                if !s.batch_queue.is_empty() {
                    s.batch_queue.remove(0);
                }
                s.tick += 1;
                if s.batch_queue.is_empty() {
                    self.task_merge_answers();
                } else {
                    self.task_send_batch();
                }
            }
            Err(why) => {
                s.blocked = Some(format!("The answer could not be read: {why}."));
                s.tick += 1;
                self.redraw();
            }
        }
    }

    /// "Add N to the library": links into the chosen group, each written and read back.
    pub(super) fn task_file_write(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        let DestChoice::Group(group_i) = s.dest else {
            self.docs
                .set_toast("Choose the group the files go to first.");
            return;
        };
        let Some(group) = s.groups.get(group_i).cloned() else {
            return;
        };
        let base = group
            .file
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let targets: Vec<(PathBuf, String, Vec<String>)> = s
            .to_write()
            .into_iter()
            .filter_map(|i| {
                let f = &s.files[i];
                if !s.planned {
                    return None;
                }
                let title = f.title.clone().unwrap_or_else(|| task_ui::default_title(f));
                let sections = if f.with.is_some() {
                    f.with
                        .and_then(|w| s.files.get(w))
                        .map(|t| t.sections.clone())
                        .unwrap_or_default()
                } else {
                    f.sections.clone()
                };
                Some((f.path.clone(), title, sections))
            })
            .collect();
        if targets.is_empty() {
            self.docs
                .set_toast("Nothing is placed to add to the library.");
            return;
        }
        let library = self.docs.library.dir.clone();
        if library.as_os_str().is_empty() || !library.is_absolute() {
            self.docs
                .set_toast(crate::commands::strings::LIBRARY_NO_FOLDER);
            return;
        }
        let instruction = s.instruction.text().trim().to_string();
        // THE FILES on "Copy them in" (pictures plan 4.3, D-209): the copies are made first,
        // each in the folder its group and section give it, and the lines lead to them. A file
        // inside the library's folder or the group's home is linked where it is.
        let copying = s.files_go == mdedit_core::prefs::FilesGo::Copy;
        let mut copies: Option<(
            Vec<mdedit_core::moves::copy_in::Planned>,
            mdedit_core::moves::copy_in::Written,
        )> = None;
        let mut targets = targets;
        if copying {
            use mdedit_core::moves::copy_in;
            let inside: Vec<PathBuf> = [library.clone(), group.home.clone()]
                .into_iter()
                .filter(|d| !d.as_os_str().is_empty())
                .collect();
            let wanted: Vec<copy_in::Wanted> = targets
                .iter()
                .map(|(path, _, sections)| copy_in::Wanted {
                    from: path.clone(),
                    dir: mdshared_app::library::note_folder(&group.home, sections),
                })
                .collect();
            let plan = copy_in::plan(&wanted, &inside);
            // A7, as every writer of notes keeps it (Fable's review of C1): a copy never goes into
            // the library folder itself, where the reader would take it for a library file.
            if plan.iter().any(|p| {
                !matches!(p.copy, copy_in::Copy::Stays { .. })
                    && p.target()
                        .parent()
                        .is_some_and(|d| super::library_write::same_folder(d, &library))
            }) {
                self.docs
                    .set_toast(crate::commands::strings::LIBRARY_HOME_IS_LIBRARY);
                self.redraw();
                return;
            }
            match copy_in::write(&plan, &library, &group.home) {
                Ok(written) => {
                    for (t, p) in targets.iter_mut().zip(&plan) {
                        t.0 = p.target().to_path_buf();
                    }
                    // A file whose very bytes are in the group's folder already, and listed
                    // there, is not listed a second time (Fable's review of C1).
                    let listed = |path: &Path| {
                        self.docs
                            .library
                            .entries()
                            .any(|(_, e)| crate::library_pane::same_note(&e.path, path))
                    };
                    let keep: Vec<bool> = plan
                        .iter()
                        .map(|p| {
                            !(matches!(p.copy, copy_in::Copy::There { .. }) && listed(p.target()))
                        })
                        .collect();
                    let mut k = keep.iter();
                    targets.retain(|_| k.next().copied().unwrap_or(true));
                    if targets.is_empty() {
                        copy_in::take_back(&written);
                        self.docs.set_toast(
                            "Nothing was added: the library lists every one of these files already.",
                        );
                        self.redraw();
                        return;
                    }
                    copies = Some((plan, written));
                }
                Err(why) => {
                    // Nothing was added, and what was copied is taken away again.
                    self.docs
                        .set_toast(&format!("Nothing was added to the library: {why}."));
                    self.redraw();
                    return;
                }
            }
        }
        // The links as the library file will hold them, each title cleaned as every writer
        // cleans it, so the file reads back as exactly its old notes plus these.
        let links: Vec<runs::Link> = targets
            .iter()
            .map(|(path, title, sections)| {
                let link = crate::library_pane::link_path(&base, path);
                let title = title.trim().replace(['[', ']'], "");
                runs::Link {
                    line: mdshared_app::library::link_line(&title, &link),
                    path: link,
                    title,
                    sections: sections.clone(),
                }
            })
            .collect();
        match self.task_file_links(&group, &base, &links) {
            Ok((before, created)) => {
                // The run's record, so "AI runs…" can undo it (the pictures plan, section 14):
                // made after the add, since a run of links writes nothing it could leave half
                // made; a record that cannot be written is said, and the links stay.
                let home = mdshared_app::library::parse(&before, &base)
                    .home
                    .unwrap_or_else(|| group.home.clone());
                let rec_group = runs::Group {
                    name: group.name.clone(),
                    file: group.file.clone(),
                    created,
                    home,
                };
                let made = match &copies {
                    Some((_, written)) => runs::copies_record(
                        &library,
                        &instruction,
                        rec_group,
                        &links,
                        &before,
                        written,
                    ),
                    None => runs::links_record(&library, &instruction, rec_group, &links, &before),
                };
                self.library_load();
                self.library_rows();
                self.task_close();
                if let Some((path, _, sections)) = targets.first() {
                    self.library_show_link(&group.file, sections, path);
                }
                let n = links.len();
                let what = if n == 1 {
                    "1 file".to_string()
                } else {
                    format!("{n} files")
                };
                let copied = copies
                    .as_ref()
                    .map(|(plan, written)| task_ui::copied_text(plan, written));
                match made {
                    Ok(_) => self.docs.set_toast(&match copied {
                        Some(c) => format!(
                            "Added {what} to the library in {}: {c}. AI runs\u{2026} can undo it.",
                            group.name
                        ),
                        None => format!(
                            "Added {what} to the library in {}. AI runs\u{2026} can undo it.",
                            group.name
                        ),
                    }),
                    Err(e) => self.docs.set_toast(&format!(
                        "Added {what} to the library in {}, but its record could not be written \
                         ({e}), so AI runs\u{2026} cannot undo it.",
                        group.name
                    )),
                }
            }
            Err(why) => {
                // Nothing was written: the window stays, for the owner to put it right, and the
                // copies made for the lines are taken away again.
                if let Some((_, written)) = &copies {
                    mdedit_core::moves::copy_in::take_back(written);
                }
                self.docs.set_toast(&why);
            }
        }
        self.redraw();
    }

    /// The Notes pane on the first line a run of File them added (pictures plan 4.4): shown,
    /// unfolded to it, the cursor on it. The files are the owner's and were not changed, so
    /// nothing opens in a tab.
    fn library_show_link(&mut self, group: &Path, section: &[String], path: &Path) {
        use crate::library_pane::{Kind, key_of};
        self.ui.panes.files = true;
        self.ui.set_pane_face(crate::ui::PaneFace::Notes);
        self.ui.library.unfold_to(group, section);
        self.library_build();
        let rows = &self.ui.library.rows;
        let group_key = key_of(group, &[]);
        let Some(start) = rows
            .iter()
            .position(|r| r.kind == Kind::Group && r.key == group_key)
        else {
            return;
        };
        let at = rows[start + 1..]
            .iter()
            .take_while(|r| r.kind != Kind::Group)
            .position(|r| {
                matches!(&r.kind, Kind::Note { path: p, .. }
                    if mdshared_app::paths::same_path(p, path))
            });
        if let Some(at) = at {
            self.ui.library.set_cur(start + 1 + at);
        }
    }

    /// Writes File them's links into the group's library file in one write: a new group's file
    /// made whole by one exclusive create, from the library's template; a group the library has
    /// through the window's gate, refused while a tab holds it unsaved, a byte order mark kept and
    /// a file that is not UTF-8 left alone (W1, RB3). Gives the file's text before the links and
    /// whether the run made the file; `Err` is what the toast says, with nothing written.
    fn task_file_links(
        &mut self,
        group: &task_ui::Group,
        base: &Path,
        links: &[runs::Link],
    ) -> Result<(String, bool), String> {
        use super::library_write::Wrote;
        use crate::commands::strings;
        self.library_file_free(&group.file)?;
        if !group.file.exists() {
            let stem = group
                .file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| group.name.clone());
            let template =
                mdshared_app::library::new_group_with_home(&group.name, Some(Path::new(&stem)))
                    .ok_or_else(|| "The group's name cannot be a heading.".to_string())?;
            let new = runs::with_links_added(&template, links);
            if !runs::check_links_added(
                &mdshared_app::library::parse(&template, base),
                &mdshared_app::library::parse(&new, base),
                links,
                base,
            ) {
                return Err(strings::library_failed(
                    "the new group would not read back as these notes",
                ));
            }
            std::fs::create_dir_all(base)
                .and_then(|()| mdshared_app::files::create_new(&group.file, new.as_bytes()))
                .map_err(|e| strings::library_failed(&e.to_string()))?;
            return Ok((template, true));
        }
        let (before, _) = super::library_write::library_text(&group.file)
            .map_err(|why| strings::library_failed(&why))?;
        match self.library_attempt_at(
            &group.file,
            base,
            |text| Some(runs::with_links_added(text, links)),
            |old, new| runs::check_links_added(old, new, links, base),
        ) {
            Wrote::Done => Ok((before, false)),
            Wrote::Nothing => Err("The library file was not changed.".to_string()),
            Wrote::Refused(why) => Err(why),
        }
    }

    /// "Split them": each file's plan card, in turn. The window waits behind the plan; its
    /// Writing closes the file's place and brings the window back.
    fn task_split_start(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        let Some(index) = s.taking_part().into_iter().next() else {
            return;
        };
        let path = s.files[index].path.clone();
        let Some(root) = self.library_dir() else {
            return;
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.docs
                    .set_toast(&format!("{} could not be read: {e}", path.display()));
                return;
            }
        };
        let loaded = mdedit_core::load::load_bytes(&bytes, &self.docs.load_options());
        if loaded.info.lossy || loaded.info.has_nul {
            self.docs
                .set_toast(&crate::commands::strings::split_lossy(&s.files[index].name));
            return;
        }
        let buffer = loaded.buffer;
        let headings = crate::plan_ui::headings_of(&buffer);
        let name = s.files[index].name.clone();
        let stem = Path::new(&name)
            .file_stem()
            .map(|x| x.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        let groups: Vec<crate::plan_ui::Dest> = s
            .groups
            .iter()
            .map(|g| crate::plan_ui::Dest {
                name: g.name.clone(),
                file: g.file.clone(),
                home: g.home.clone(),
                is_new: false,
            })
            .collect();
        let dest = match s.dest {
            DestChoice::Group(i) => groups.get(i).cloned(),
            _ => groups
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(&stem))
                .cloned(),
        };
        let dest = dest.unwrap_or_else(|| crate::plan_ui::Dest::new(&root, &stem, true));
        let encoding = match loaded.info.encoding {
            mdedit_core::encoding::Encoding::Utf8 | mdedit_core::encoding::Encoding::Utf8Bom => {
                None
            }
            e => Some(format!("{}, written as UTF-8", e.label())),
        };
        let mut screen = crate::plan_ui::PlanScreen::new(crate::plan_ui::Init {
            doc: mdedit_core::split::document(&buffer),
            name: &name,
            path: Some(&path),
            headings: &headings,
            level: None,
            encoding,
            dest,
            groups,
            root: root.clone(),
            reader: crate::plan_ui::fs_reader(),
        });
        screen.source.len = bytes.len() as u64;
        screen.source.sha256 = mdshared_app::update::sha256_hex(&bytes);
        screen.ai_offer = self.ai_conn() != Connection::Off;
        screen.recycle = Some(crate::plan_ui::RecycleBox {
            wanted: false,
            refused: Some(crate::plan_ui::RECYCLE_ONE_DOCUMENT.to_string()),
        });
        screen.blocked = super::plan_write::plan_blocked(self, &screen);
        screen.blocked_for = Some(screen.dest.clone());
        let instruction = s.instruction.text().trim().to_string();
        self.task_parked = self.task.take();
        self.task_plan = Some(index);
        self.plan_open(screen);
        if !instruction.is_empty() && self.ai_conn() != Connection::Off {
            self.split_start(Spec {
                title: "Split".into(),
                instruction,
            });
        }
        self.redraw();
    }

    /// A plan opened from the task window closed: the window comes back and the file's place
    /// says whether the plan was written.
    pub(super) fn task_plan_done(&mut self, written: bool) {
        let Some(index) = self.task_plan.take() else {
            return;
        };
        // A split still out was asked for this file's plan, which is written or closed now: it
        // stops here, so its answer never reaches the next file's (audit 4, B11).
        if self.ai.split.is_some() {
            self.ai.cancel_split();
            self.docs.ai_status = None;
        }
        let Some(mut t) = self.task_parked.take() else {
            return;
        };
        if let Some(f) = t.files.get_mut(index) {
            if written {
                f.done = true;
            }
            f.ticked = false;
        }
        t.tick += 1;
        let more = written && t.task == TaskKind::Split && !t.taking_part().is_empty();
        self.task = Some(t);
        if more {
            // Split them is one plan per file, in order: the next opens when this one is written.
            self.task_split_start();
        }
        self.redraw();
    }

    /// "Rewrite them": the next file's request.
    fn task_rewrite_next(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        let next = s
            .taking_part()
            .into_iter()
            .find(|&i| !s.files[i].done && s.files[i].text.is_some());
        let Some(index) = next else {
            self.task_rewrite_finish();
            return;
        };
        self.task_rewrite_begin(index);
    }

    fn task_rewrite_begin(&mut self, index: usize) {
        // The file is read again: the request is made from what is on disk now, not from what
        // was read when it was added, so a file edited since can still be rewritten (audit 4,
        // B12).
        let Some(path) = self
            .task
            .as_ref()
            .and_then(|s| s.files.get(index))
            .map(|f| f.path.clone())
        else {
            return;
        };
        let fresh = self.task_probe(&path);
        let Some(s) = self.task.as_mut() else { return };
        let Some(f) = s.files.get_mut(index) else {
            return;
        };
        f.size = fresh.size;
        f.kind = fresh.kind;
        f.why = fresh.why;
        f.text = fresh.text;
        f.bom = fresh.bom;
        f.sample = fresh.sample;
        f.heading = fresh.heading;
        f.eol = fresh.eol;
        f.modified = fresh.modified;
        if let Some(why) = f.reason_for(TaskKind::Rewrite) {
            s.blocked = Some(format!("{}: {why}. Nothing was sent.", f.name));
            f.ticked = false;
            s.recount_limit();
            s.tick += 1;
            self.redraw();
            return;
        }
        let Some(s) = self.task.as_ref() else { return };
        let Some(f) = s.files.get(index) else { return };
        let Some(text) = f.text.clone() else { return };
        let instruction = s.instruction.text().trim().to_string();
        let prompt = core_ai::prompt(&instruction, &text);
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": core_ai::SYSTEM,
            "prompt": prompt,
        });
        let wake = self.docs.waker();
        let name = f.name.clone();
        let what = format!("Asking {} \u{00B7} {name}\u{2026}", self.ai_via());
        match self.ai.ask(&wake, Ask::Task, req) {
            Ok(id) => {
                self.task_sent.bytes += prompt.len() as u64;
                self.task_sent.requests += 1;
                if let Some(s) = self.task.as_mut() {
                    s.asking = Some(Asking {
                        task: TaskKind::Rewrite,
                        request: id,
                        what,
                        chars: 0,
                        index,
                        prompt,
                        batch: vec![index],
                    });
                    s.blocked = None;
                    s.tick += 1;
                }
            }
            Err(why) => {
                if let Some(s) = self.task.as_mut() {
                    s.blocked = Some(why);
                    s.tick += 1;
                }
            }
        }
        self.redraw();
    }

    /// A rewrite's answer: 5.5's review over the file, its Apply writing in place.
    fn task_rewrite_answer(&mut self, text: &str) {
        let Some(s) = self.task.as_mut() else { return };
        let Some(asking) = s.asking.take() else {
            return;
        };
        let index = asking.index;
        let Some(f) = s.files.get(index) else { return };
        let Some(original) = f.text.clone() else {
            return;
        };
        let name = f.name.clone();
        let path = f.path.clone();
        let eol = f.eol.clone();
        let bom = f.bom;
        let window = s.id;
        let instruction = s.instruction.text().trim().to_string();
        let fitted = core_ai::fit_end(&original, text);
        let result = core_ai::with_eol(&fitted, &eol);
        if result.trim().is_empty() {
            s.blocked = Some(format!(
                "{name}: the AI sent back no text. Nothing was changed."
            ));
            s.tick += 1;
            self.redraw();
            return;
        }
        if result == original {
            self.docs
                .set_toast(&format!("{name}: the AI changed nothing."));
            self.task_after_answer_skipped(index);
            return;
        }
        let hunks = mdedit_core::hunks::Hunks::new(&original, &result);
        let blocked = self.task_file_changed(&path, &original, bom);
        let preview = self.docs.prefs.ai_review_preview;
        let via = self.ai_via();
        let sent = self.task_sent_now();
        self.close_for_ai_panel();
        self.ai.panel = Some(Panel::Review(Review {
            doc: DocId::default(),
            start: 0,
            end: original.len(),
            original: original.clone(),
            result,
            subtitle: format!("{name} \u{00B7} whole file \u{00B7} {via}"),
            spec: Spec {
                title: format!("Rewrite {name}"),
                instruction: instruction.clone(),
            },
            hunks,
            list: Default::default(),
            blocked,
            preview,
            panes: None,
            sent: 0,
            due: None,
            grab: None,
            base_dir: path.parent().map(Path::to_path_buf),
            file: Some(FileReview {
                path,
                window,
                index,
                bom,
                eol,
                instruction,
                sent,
            }),
        }));
        if preview {
            self.ai_open_panes();
        }
        self.ai.tick += 1;
        self.redraw();
    }

    /// Why the file the review is about cannot be written now: it changed, or it is gone.
    fn task_file_changed(&self, path: &Path, original: &str, bom: bool) -> Option<String> {
        let bytes = std::fs::read(path).ok()?;
        let (has, body) = match bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
            Some(rest) => (true, rest),
            None => (false, &bytes[..]),
        };
        let text = std::str::from_utf8(body).ok()?;
        if has != bom || text != original {
            return Some(core_ai::BLOCKED_CHANGED.to_string());
        }
        None
    }

    /// The review's Apply on a file: the ticked changes are written in place, and the file's
    /// original goes into the run's record first (D-074, D-076).
    pub(super) fn task_review_apply(&mut self) {
        let Some(Panel::Review(rv)) = self.ai.panel.as_ref() else {
            return;
        };
        let Some(fr) = rv.file.clone() else { return };
        if rv.blocked.is_some() || rv.hunks.ticked() == 0 {
            return;
        }
        let text = rv.hunks.compose();
        let original = rv.original.clone();
        if let Some(why) = self.task_file_changed(&fr.path, &original, fr.bom) {
            if let Some(rv) = self.ai.review() {
                rv.blocked = Some(why);
            }
            self.ai.tick += 1;
            self.redraw();
            return;
        }
        let Some(library) = self.library_dir() else {
            return;
        };
        let file = runs::RewriteFile {
            path: fr.path.clone(),
            before: original,
            after: text,
            bom: fr.bom,
        };
        let own = self.task_window_of(&fr).is_some();
        let written = if own {
            self.task_rewrite_into(&library, &fr, &file)
        } else {
            Self::task_rewrite_alone(&library, &fr, &file)
        };
        match written {
            Ok(closed) => {
                if let Err(e) = closed {
                    self.docs
                        .set_toast(&format!("The record could not be updated: {e}"));
                }
                let name = fr
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // A tab that holds the file shows the new text at once, and a preview following
                // it with it (pictures plan 4.4); one with unsaved changes is told instead.
                self.docs.check_changed();
                self.docs.set_toast(&format!(
                    "{name} rewritten. Its original is in AI runs, where Undo brings it back."
                ));
                if let Some(s) = self.task_window_of(&fr)
                    && let Some(f) = s.files.get_mut(fr.index)
                {
                    f.done = true;
                    f.ticked = false;
                    s.tick += 1;
                }
                self.ai.panel = None;
                self.ai.tick += 1;
                self.task_review_goes_on(&fr);
            }
            Err(why) => {
                if let Some(rv) = self.ai.review() {
                    rv.blocked = Some(why);
                }
                self.ai.tick += 1;
                self.redraw();
            }
        }
    }

    /// The window a file review came from, open or parked behind the request's tab, when its
    /// file is still at the review's place; `None` for a review kept from a window since closed
    /// (audit 4, B4).
    fn task_window_of(&mut self, fr: &FileReview) -> Option<&mut TaskScreen> {
        let own = |s: &TaskScreen| {
            s.id == fr.window && s.files.get(fr.index).is_some_and(|f| f.path == fr.path)
        };
        if self.task.as_ref().is_some_and(own) {
            return self.task.as_mut();
        }
        if self.task_parked.as_ref().is_some_and(own) {
            return self.task_parked.as_mut();
        }
        None
    }

    /// The review's window is the one open now: only then does the run go on to its next file.
    fn task_owns(&self, fr: &FileReview) -> bool {
        self.task.as_ref().is_some_and(|s| {
            s.id == fr.window && s.files.get(fr.index).is_some_and(|f| f.path == fr.path)
        })
    }

    /// The run goes on with its next file when the review was the open window's; a review kept
    /// from a window since closed, or another one, touches no window and sends nothing.
    fn task_review_goes_on(&mut self, fr: &FileReview) {
        if self.task_owns(fr) {
            self.task_rewrite_next();
        } else {
            self.redraw();
        }
    }

    /// The window's record takes the file's original. A record holds what can be read back
    /// (`runs::RECORD_MAX`): when the original would not fit, that record is closed as done and
    /// the file begins the next one (audit 4, B2). The `Ok` holds how the last close went.
    fn task_rewrite_into(
        &mut self,
        library: &Path,
        fr: &FileReview,
        file: &runs::RewriteFile,
    ) -> Result<std::io::Result<()>, String> {
        let full = self.task_record.as_ref().is_some_and(|(_, record)| {
            !record.originals.is_empty() && !runs::rewrite_fits(record, library, file)
        });
        if full && let Some((path, mut record)) = self.task_record.take() {
            self.task_rewritten += record.originals.len();
            record.state = runs::State::Done;
            if let Err(e) = record.close(&path, library) {
                self.docs
                    .set_toast(&format!("The record could not be updated: {e}"));
            }
        }
        if self.task_record.is_none() {
            let sent = self.task_sent_now();
            match runs::start_rewrite(library, &fr.instruction, Some(sent), &fr.path) {
                Ok(record) => self.task_record = Some(record),
                Err(e) => return Err(format!("The run's record could not be made: {e}")),
            }
        }
        let sent = self.task_sent_now();
        let Some((record_path, record)) = self.task_record.as_mut() else {
            return Err("The run's record could not be made.".to_string());
        };
        record.sent = Some(sent);
        runs::rewrite_into(record_path, record, library, file)?;
        Ok(record.close(record_path.as_path(), library))
    }

    /// A review kept from a window since closed: its file is a run of its own, done at once, so
    /// no other window's record takes it (audit 4, B4).
    fn task_rewrite_alone(
        library: &Path,
        fr: &FileReview,
        file: &runs::RewriteFile,
    ) -> Result<std::io::Result<()>, String> {
        let (path, mut record) =
            runs::start_rewrite(library, &fr.instruction, Some(fr.sent.clone()), &fr.path)
                .map_err(|e| format!("The run's record could not be made: {e}"))?;
        if let Err(why) = runs::rewrite_into(&path, &mut record, library, file) {
            let _ = std::fs::remove_file(&path);
            return Err(why);
        }
        record.state = runs::State::Done;
        Ok(record.close(&path, library))
    }

    /// Discard on a rewrite's review: this file stays as it was, and the run goes on.
    pub(super) fn task_review_discard(&mut self) {
        let Some(Panel::Review(rv)) = self.ai.panel.as_ref() else {
            return;
        };
        let Some(fr) = rv.file.clone() else { return };
        let name = fr
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.docs.set_toast(&format!("{name} was left as it was."));
        self.ai.panel = None;
        self.ai.tick += 1;
        self.task_review_left(&fr);
    }

    /// Discard or Copy result on a file's review: the file is left as it was and unticked, so
    /// the run does not ask for it again, and the run goes on with the next (DESIGN 83; audit
    /// 4, B3).
    pub(super) fn task_review_left(&mut self, fr: &FileReview) {
        if let Some(s) = self.task_window_of(fr)
            && let Some(f) = s.files.get_mut(fr.index)
        {
            f.ticked = false;
            s.recount_limit();
            s.tick += 1;
        }
        self.task_review_goes_on(fr);
    }

    /// Run again on a rewrite's review: the same instruction, the same file. With the window
    /// that asked closed there is nothing to ask from, and the review stays (audit 4, B10).
    pub(super) fn task_review_again(&mut self) {
        let Some(Panel::Review(rv)) = self.ai.panel.as_mut() else {
            return;
        };
        let Some(fr) = rv.file.clone() else { return };
        if !self.task_owns(&fr) {
            self.docs.set_toast(
                "The task window this file was in is closed, so it cannot be asked again. \
                 Apply, Copy result or Discard this result.",
            );
            self.redraw();
            return;
        }
        self.ai.panel = None;
        self.ai.tick += 1;
        self.task_rewrite_begin(fr.index);
    }

    /// The file's answer changed nothing or was not applied: the run goes on with the next one.
    fn task_after_answer_skipped(&mut self, index: usize) {
        if let Some(s) = self.task.as_mut()
            && let Some(f) = s.files.get_mut(index)
        {
            f.done = true;
            f.ticked = false;
        }
        self.task_rewrite_next();
    }

    /// The run is over: its record says done, and the window says what happened.
    fn task_rewrite_finish(&mut self) {
        let written = self.task_rewritten
            + self
                .task_record
                .as_ref()
                .map(|(_, r)| r.originals.len())
                .unwrap_or(0);
        let library = self.library_dir();
        if let Some((path, mut record)) = self.task_record.take() {
            // A record that took no file's original is removed, as Cancel does (audit 4, B6).
            if record.originals.is_empty() {
                let _ = std::fs::remove_file(&path);
            } else {
                record.state = runs::State::Done;
                if let Some(library) = library {
                    let _ = record.close(&path, &library);
                }
            }
        }
        self.task_rewritten = 0;
        self.task_sent = Sent::default();
        self.task = None;
        self.task_parked = None;
        self.task_plan = None;
        if written > 0 {
            self.docs.set_toast(&format!(
                "{} rewritten. Undo is in AI runs.",
                if written == 1 {
                    "1 file".to_string()
                } else {
                    format!("{written} files")
                }
            ));
        }
        self.redraw();
    }

    fn task_sent_now(&self) -> Sent {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        Sent {
            connection: c.key().to_string(),
            model: p.ai_model(c).to_string(),
            bytes: self.task_sent.bytes,
            requests: self.task_sent.requests,
        }
    }

    /// The engine's answer to the task window.
    pub(super) fn task_done(&mut self, text: &str) {
        let Some(s) = self.task.as_ref() else { return };
        let Some(asking) = s.asking.as_ref() else {
            return;
        };
        // The task that asked, not the one the window shows now (audit 4, B5).
        match asking.task {
            TaskKind::File => self.task_file_answer(text),
            TaskKind::Rewrite => self.task_rewrite_answer(text),
            // Split them asks through each file's plan card, never through the window.
            TaskKind::Split => {
                if let Some(s) = self.task.as_mut() {
                    s.asking = None;
                    s.tick += 1;
                }
            }
        }
    }

    /// A task window's request failed, or the engine ended.
    pub(super) fn task_failed(&mut self, kind: &str, message: &str) {
        if kind == "cancelled" {
            return;
        }
        let Some(s) = self.task.as_mut() else { return };
        // The request is no longer out: an error leaves its place, and the window says why.
        if let Some(asking) = s.asking.take() {
            s.blocked = Some(match kind {
                "auth" | "not_found" | "bad_request" => {
                    format!("{message} Check Settings \u{203A} AI.")
                }
                _ => message.to_string(),
            });
            let _ = asking;
        }
        s.tick += 1;
        self.redraw();
    }

    /// The request in flight from the task window is stopped.
    pub(super) fn task_cancel_request(&mut self) {
        if let Some(s) = self.task.as_mut()
            && let Some(asking) = s.asking.take()
        {
            self.ai.cancel_task(asking.request);
            if let Some(s) = self.task.as_mut() {
                s.blocked = None;
                s.tick += 1;
            }
            self.docs.set_toast("Stopped.");
            self.redraw();
        }
    }

    /// "Show what is sent": the literal request text in a read-only tab. The window is parked
    /// behind it, and "AI task…" brings it back.
    pub(super) fn task_show_sent(&mut self) {
        if self.task.as_ref().is_some_and(|s| s.asking.is_some()) {
            self.docs
                .set_toast("The request is out. Its answer comes first.");
            return;
        }
        let Some(text) = self.task_request_text() else {
            return;
        };
        self.task_parked = self.task.take();
        let title = "What is sent";
        let i = self
            .docs
            .open_copy(title.to_string(), text.as_bytes(), None);
        self.docs.activate(i);
        self.docs
            .set_toast("The request text is in this tab. AI task… brings the window back.");
        self.relayout_areas();
        self.redraw();
    }

    /// The exact text of every request the window would send, or why none is.
    pub(super) fn task_request_text(&self) -> Option<String> {
        let s = self.task.as_ref()?;
        if !s.ai_on {
            return Some(
                "The AI connection is off. Nothing is sent.\n\nFile them and Split them write \
                 without a request; Rewrite them needs one.\n"
                    .to_string(),
            );
        }
        let instruction = s.instruction.text().trim().to_string();
        let ids = s.taking_part();
        let mut out = String::new();
        out.push_str(
            "What the AI task window would send. Nothing has been sent by this tab.\n\
             The window is behind it: AI task\u{2026} brings it back.\n\n",
        );
        match s.task {
            TaskKind::File => {
                let batches: Vec<&[usize]> = ids.chunks(s.limits.batch.max(1)).collect();
                for (k, batch) in batches.iter().enumerate() {
                    out.push_str(&format!(
                        "=== REQUEST {} of {} ===\nSYSTEM:\n{}\n\nPROMPT:\n",
                        k + 1,
                        batches.len(),
                        task_ui::FILING_SYSTEM
                    ));
                    out.push_str(&task_ui::filing_prompt(&instruction, batch, &s.files));
                    out.push('\n');
                }
            }
            TaskKind::Rewrite => {
                for (k, &i) in ids.iter().enumerate() {
                    let f = &s.files[i];
                    out.push_str(&format!(
                        "=== REQUEST {} of {}: {} ===\nSYSTEM:\n{}\n\nPROMPT:\n",
                        k + 1,
                        ids.len(),
                        f.name,
                        core_ai::SYSTEM
                    ));
                    out.push_str(&core_ai::prompt(
                        &instruction,
                        f.text.as_deref().unwrap_or(""),
                    ));
                    out.push('\n');
                }
            }
            TaskKind::Split => {
                if instruction.is_empty() {
                    return Some(
                        "No request: each file's plan is cut at its headings. Type an instruction \
                         to ask the AI to cut it."
                            .to_string(),
                    );
                }
                let mut requests = 0;
                for &i in &ids {
                    let f = &s.files[i];
                    let Ok(bytes) = std::fs::read(&f.path) else {
                        continue;
                    };
                    let loaded = mdedit_core::load::load_bytes(&bytes, &self.docs.load_options());
                    let buffer = loaded.buffer;
                    let headings = crate::plan_ui::headings_of(&buffer);
                    let doc = mdedit_core::split::document(&buffer);
                    let Ok(outline) = mdedit_core::split::answer::outline(
                        &*doc,
                        &headings,
                        mdedit_core::split::answer::OUTLINE_MAX,
                    ) else {
                        continue;
                    };
                    requests += 1;
                    out.push_str(&format!(
                        "=== REQUEST {requests}: {} ===\nSYSTEM:\n{}\n\nPROMPT:\n",
                        f.name,
                        mdedit_core::split::answer::SYSTEM
                    ));
                    out.push_str(&mdedit_core::split::answer::prompt(
                        &instruction,
                        &f.name,
                        &outline,
                    ));
                    out.push('\n');
                }
                if requests == 0 {
                    return Some("No file can be sent: each plan is cut at its headings.".into());
                }
            }
        }
        Some(out)
    }

    /// Escape, Cancel, or the end of a task: the window goes, a request with it. A rewrite's
    /// record stays, closed as done, so what was written can be undone.
    pub(super) fn task_close(&mut self) {
        let asking = self.task.as_mut().and_then(|s| s.asking.take());
        if let Some(asking) = asking {
            self.ai.cancel_task(asking.request);
        }
        if let Some((path, mut record)) = self.task_record.take() {
            if record.originals.is_empty() {
                let _ = std::fs::remove_file(&path);
            } else {
                record.state = runs::State::Done;
                if let Some(library) = self.library_dir() {
                    let _ = record.close(&path, &library);
                }
            }
        }
        self.task_sent = Sent::default();
        self.task_rewritten = 0;
        self.task = None;
        self.task_parked = None;
        self.task_plan = None;
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    /// The primary button's work on the plan face: the links.
    pub(super) fn task_primary(&mut self) {
        let Some(s) = self.task.as_ref() else { return };
        if s.planned {
            self.task_file_write();
        } else {
            self.task_run();
        }
    }

    /// `--ai-demo files|files-other|files-off|files-plan|sent`: the window for a shot, made
    /// from the fixtures under `fixtures/task-demo/`, with no engine and nothing sent.
    pub(super) fn task_demo(&mut self, name: &str) -> bool {
        let dir = Path::new("fixtures/task-demo");
        if !dir.is_dir() {
            return false;
        }
        let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|r| r.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        if names.is_empty() {
            return false;
        }
        names.sort();
        let kind = match name {
            "files-other" => TaskKind::Rewrite,
            "files-off" => TaskKind::File,
            _ => TaskKind::File,
        };
        let via = if name == "files-off" {
            self.ai_via()
        } else {
            "Claude".to_string()
        };
        self.task_seq += 1;
        let window = self.task_seq;
        let mut s = TaskScreen::new(
            kind,
            Vec::new(),
            DestChoice::New("Recipes".to_string()),
            self.docs.ai_file_prompts.clone(),
            via.clone(),
        );
        s.id = window;
        s.win = self.logical();
        s.scale = self.scale;
        s.ai_on = self.ai_conn() != Connection::Off;
        self.task = Some(s);
        self.close_for_ai_panel();
        let probed: Vec<_> = names.iter().map(|p| self.task_probe(p)).collect();
        let Some(s) = self.task.as_mut() else {
            return true;
        };
        s.files = probed;
        s.recount_limit();
        if name == "files-other" {
            s.task = TaskKind::Rewrite;
        }
        s.ai_on = name != "files-off";
        s.via = if s.ai_on {
            "Claude".to_string()
        } else {
            via.clone()
        };
        if name == "files-off" {
            s.ai_on = false;
        }
        if name == "files-long-dest" {
            // A group whose folder is deep and long, made up for the shot: the WHERE IT GOES
            // row must fit it beside the section field at every width.
            let home = if cfg!(windows) {
                r"C:\Users\someone\AppData\Local\mdedit\library\Recipes from my grandmother's kitchen"
            } else {
                "/home/someone/.local/share/mdedit/library/Recipes from my grandmother's kitchen"
            };
            s.groups = vec![crate::task_ui::Group {
                name: "Recipes from my grandmother's kitchen".to_string(),
                file: PathBuf::from(home).with_extension("md"),
                home: PathBuf::from(home),
            }];
            s.section.set_text("Italian main courses");
        }
        if !s.groups.is_empty() {
            s.dest = DestChoice::Group(0);
        } else {
            s.dest = DestChoice::New("Recipes".to_string());
        }
        s.instruction.set_text(match s.task {
            TaskKind::File => "Sort these into my recipes and bread notes.",
            TaskKind::Split => "One note per recipe.",
            TaskKind::Rewrite => "Make the method clearer, not longer.",
        });
        if name == "files-plan" {
            demo_plan(s);
        }
        if name == "files-copy" || name == "files-link" {
            // THE FILES (pictures plan 4.3, D-209) on the plan face: on "Copy them in" each row
            // says whether its file is copied in, and one already inside the library (made up
            // for the shot) is linked where it is; "files-link" is the switch on "Link them".
            demo_plan(s);
            let library = if cfg!(windows) {
                PathBuf::from(r"C:\Users\someone\Notes")
            } else {
                PathBuf::from("/home/someone/Notes")
            };
            if let Some(f) = s.files.iter_mut().find(|f| f.name == "notes.md") {
                f.path = library.join("Bread").join("notes.md");
            }
            s.library = library;
            if name == "files-link" {
                s.files_go = mdedit_core::prefs::FilesGo::Link;
            }
        }
        if name == "files-from" {
            // Opened from a section of the Notes pane (pictures plan 4.2): Rewrite them, and
            // the header says where the notes came from.
            s.task = TaskKind::Rewrite;
            s.instruction
                .set_text("Make the method clearer, not longer.");
            s.from =
                Some("The 3 notes of Recipes \u{203A} Pasta, from the Notes pane.".to_string());
        }
        if name == "files-add" {
            // The Add menu open, over made-up tabs: the one in front first and said so (D-207).
            let tab = |n: &str, front: bool| crate::task_ui::AddItem::Tab {
                path: PathBuf::from(n),
                name: n.to_string(),
                front,
            };
            s.add_items = vec![
                tab("shopping list.md", true),
                tab("week plan.txt", false),
                crate::task_ui::AddItem::Files,
            ];
            s.add_open = true;
        }
        let review = if name == "review" {
            // The rewrite's review over the first text file, with a made-up result and no
            // engine: 5.5's list over the whole file, Apply writing in place.
            let at = s
                .files
                .iter()
                .position(|f| f.text.as_deref().is_some_and(|t| t.contains("Layer it.")))
                .or_else(|| s.files.iter().position(|f| f.text.is_some()))
                .unwrap_or(0);
            let f = &s.files[at];
            let original = f.text.clone().unwrap_or_default();
            let result = original.replace("Layer it.", "Layer it, then rest it for ten minutes.");
            let file_name = f.name.clone();
            let path = f.path.clone();
            let eol = f.eol.clone();
            let bom = f.bom;
            let instruction = s.instruction.text().to_string();
            s.task = TaskKind::Rewrite;
            s.tick += 1;
            Some((at, original, result, file_name, path, eol, bom, instruction))
        } else {
            None
        };
        s.tick += 1;
        self.relayout_areas();
        if let Some((at, original, result, file_name, path, eol, bom, instruction)) = review {
            let hunks = mdedit_core::hunks::Hunks::new(&original, &result);
            self.close_for_ai_panel();
            self.ai.panel = Some(Panel::Review(Review {
                doc: DocId::default(),
                start: 0,
                end: original.len(),
                original,
                result,
                subtitle: format!("{file_name} \u{00B7} whole file \u{00B7} {}", via),
                spec: Spec {
                    title: format!("Rewrite {file_name}"),
                    instruction: instruction.clone(),
                },
                hunks,
                list: Default::default(),
                blocked: None,
                preview: false,
                panes: None,
                sent: 0,
                due: None,
                grab: None,
                base_dir: path.parent().map(Path::to_path_buf),
                file: Some(FileReview {
                    path,
                    window,
                    index: at,
                    bom,
                    eol,
                    instruction,
                    sent: Sent::default(),
                }),
            }));
            self.ai.tick += 1;
        }
        if name == "sent" {
            self.task_show_sent();
        }
        self.redraw();
        true
    }
}

/// The plan face of the shot `task-files-plan`: every file but the other kind is placed under
/// "Bread", and the one left over is left out as an answer leaves it: unticked.
pub(super) fn demo_plan(s: &mut TaskScreen) {
    s.planned = true;
    for (i, f) in s.files.iter_mut().enumerate() {
        if f.kind == FileKind::Other {
            continue;
        }
        f.title = Some(format!("A title {i}"));
        f.sections = vec!["Bread".to_string()];
    }
    s.leave_out_unplaced();
}
