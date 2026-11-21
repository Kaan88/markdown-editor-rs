//! The plan screen as the window hosts it (`plan_ui.rs` is the screen, `plan_draw.rs` draws it):
//! it is drawn over everything, as the palette and the AI panels are, and while it is open it
//! takes every key, pointer and wheel event. It writes nothing: "Write the notes" records that it
//! was pressed.
//!
//! Escape on a plan the owner has changed does not lose it: the screen is parked, and
//! [`App::plan_show`] brings the same plan back.

use super::*;
use crate::keys::Key as PlanKey;
use crate::plan_ui::{Act, Dest, Init, PlanScreen, fs_reader, headings_of};
use mdedit_core::encoding::Encoding;
use mdshared_app::library::folder_name;

impl App {
    pub(super) fn plan_is_open(&self) -> bool {
        self.plan.is_some()
    }

    /// Opens `screen` over the window.
    pub(super) fn plan_open(&mut self, mut screen: PlanScreen) {
        if self.palette_open() {
            self.close_palette();
        }
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.hover = None;
        self.ui.find_focused = false;
        let (w, h) = self.logical();
        screen.resize(w, h);
        screen.scale = self.scale;
        self.plan = Some(screen);
        self.plan_parked = None;
        self.caret_reset();
        self.redraw();
    }

    /// "Split into notes…" on the document in front: the plan, cut at its headings. The document
    /// must have a file and no unsaved changes, because the notes are made from the file on disk.
    #[allow(dead_code)] // the Split into notes command calls it; it is not in a menu yet
    pub(super) fn plan_open_active(&mut self) -> bool {
        let Some(doc) = self.docs.active_doc() else {
            return false;
        };
        let name = doc.name();
        if doc.path.is_none() || doc.modified() {
            self.docs.set_toast(&format!(
                "Save {name} first: the notes are made from the file on disk."
            ));
            return false;
        }
        let Some(ready) = doc.ready() else {
            return false;
        };
        let buffer = ready.buffer.clone();
        let encoding = match ready.info.encoding {
            Encoding::Utf8 | Encoding::Utf8Bom => None,
            e => Some(format!("{}, written as UTF-8", e.label())),
        };
        let headings = headings_of(&buffer);
        let root = self.docs.library.dir.clone();
        let stem = std::path::Path::new(&name)
            .file_stem()
            .map_or_else(|| name.clone(), |s| s.to_string_lossy().into_owned());
        let groups: Vec<Dest> = self
            .docs
            .library
            .groups
            .iter()
            .map(|g| Dest {
                name: g.name.clone(),
                home: g
                    .home
                    .clone()
                    .unwrap_or_else(|| root.join(folder_name(&g.name))),
                is_new: false,
            })
            .collect();
        let dest = groups
            .iter()
            .find(|g| g.name.eq_ignore_ascii_case(&stem))
            .cloned()
            .unwrap_or_else(|| Dest::new(&root, &stem, true));
        let mut screen = PlanScreen::new(Init {
            doc: &buffer,
            name: &name,
            headings: &headings,
            level: None,
            encoding,
            dest,
            groups,
            root,
            reader: fs_reader(),
        });
        screen.blocked = self.plan_blocked(&screen);
        self.plan_open(screen);
        true
    }

    /// Why nothing can be written to the group the plan names: its library file is open with
    /// changes not saved, or its folder is a file or cannot be written. Nothing is created to find
    /// out.
    #[allow(dead_code)] // used by `plan_open_active`
    fn plan_blocked(&self, screen: &PlanScreen) -> Option<String> {
        let home = &screen.dest.home;
        if home.exists() && !home.is_dir() {
            return Some(format!("{} is a file, not a folder.", home.display()));
        }
        if std::fs::metadata(home).is_ok_and(|m| m.permissions().readonly()) {
            return Some("The folder cannot be written.".to_string());
        }
        let file = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| g.name == screen.dest.name)
            .map(|g| g.file.clone())?;
        self.docs
            .tabs
            .iter()
            .any(|d| d.modified() && d.path.as_deref() == Some(file.as_path()))
            .then(|| "The group's library file has unsaved changes. Save it first.".to_string())
    }

    /// Closes the screen; a plan that was changed is kept to be shown again.
    pub(super) fn plan_close(&mut self) {
        if let Some(screen) = self.plan.take()
            && screen.edited
        {
            self.plan_parked = Some(screen);
        }
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    /// "Show" on a plan that Escape kept: the same plan, as it was left.
    #[allow(dead_code)] // the Notes pane's "Plan ready" row calls it
    pub(super) fn plan_show(&mut self) -> bool {
        match self.plan_parked.take() {
            Some(screen) => {
                self.plan_open(screen);
                true
            }
            None => false,
        }
    }

    /// "Drop" on a plan that Escape kept.
    #[allow(dead_code)] // the Notes pane's "Plan ready" row calls it
    pub(super) fn plan_drop(&mut self) {
        self.plan_parked = None;
        self.redraw();
    }

    fn plan_act(&mut self, act: Act) {
        match act {
            Act::Close => self.plan_close(),
            Act::Write | Act::Nothing => {}
        }
        self.redraw();
    }

    pub(super) fn plan_key(&mut self, key: PlanKey, typed: Option<&str>) {
        let mods = self.mods;
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        let act = screen.key(&key, mods, typed);
        self.caret_reset();
        self.plan_act(act);
    }

    pub(super) fn plan_pointer(&mut self, x: f32, y: f32) {
        if let Some(screen) = self.plan.as_mut() {
            screen.pointer(x, y);
        }
        self.redraw();
    }

    pub(super) fn plan_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(screen) = self.plan.as_mut() {
            screen.press(x, y);
        }
        self.caret_reset();
        self.redraw();
    }

    pub(super) fn plan_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        let act = screen.release(x, y);
        self.plan_act(act);
    }

    pub(super) fn plan_right_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(screen) = self.plan.as_mut() {
            screen.right_press(x, y);
        }
        self.redraw();
    }

    /// `dy` in logical px, positive towards the end.
    pub(super) fn plan_wheel(&mut self, dy: f32) {
        let (x, y) = self.ui.mouse;
        if let Some(screen) = self.plan.as_mut() {
            screen.wheel(x, y, dy);
        }
        self.redraw();
    }

    /// What an input method composes goes to the name being typed.
    pub(super) fn plan_preedit(&mut self, pre: Option<mdshared_ui::textfield::Preedit>) -> bool {
        let Some(screen) = self.plan.as_mut() else {
            return false;
        };
        screen.tick += 1;
        screen
            .edit
            .as_mut()
            .is_some_and(|e| e.field.set_preedit(pre))
    }

    /// `--plan-demo NAME`: a plan for a shot, made from a document built in the program.
    pub(super) fn plan_demo(&mut self, name: &str) -> bool {
        match crate::plan_demo::screen(name) {
            Some(screen) => {
                self.plan_open(screen);
                true
            }
            None => false,
        }
    }
}
