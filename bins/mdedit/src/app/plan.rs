//! The plan screen as the window hosts it (the screen and its drawing are `mdshared_ui::plan`):
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
    /// The plan screen, the move card, the import card or the Markdown note card is open: any of
    /// them is over everything and takes every event, and each route below hands the cards' to
    /// `app/moving.rs`, `app/importing.rs` and `app/note.rs`.
    pub(super) fn plan_is_open(&self) -> bool {
        self.plan.is_some() || self.move_is_open() || self.import_is_open() || self.note_is_open()
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
    /// must have a file, no unsaved changes and text, because the notes are made from the file on
    /// disk; and there must be a library folder to write them into. The library is read here, so
    /// a plan opened from any menu knows the groups it already has (A6).
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
        if ready.info.lossy {
            // Plan 3.11: a file whose bytes are not text is refused, as Save As refuses it.
            self.docs
                .set_toast(&crate::commands::strings::split_lossy(&name));
            return false;
        }
        let buffer = ready.buffer.clone();
        let encoding = match ready.info.encoding {
            Encoding::Utf8 | Encoding::Utf8Bom => None,
            e => Some(format!("{}, written as UTF-8", e.label())),
        };
        // The notes are cut from this file: its length and digest are kept with the plan, and a
        // plan shown again after the file changed refuses to write (A8).
        let path = doc.path.clone().unwrap_or_default();
        let (len, sha256) = match std::fs::read(&path) {
            Ok(bytes) => (bytes.len() as u64, mdshared_app::update::sha256_hex(&bytes)),
            Err(e) => {
                self.docs.set_toast(&format!(
                    "{name} could not be read: {e}. The notes are made from the file on disk."
                ));
                return false;
            }
        };
        // Where the notes go: no library folder, no plan. A window with no session has none
        // (A11), and the empty path would make the confinement check void (A6).
        let Some(root) = self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
        else {
            self.docs
                .set_toast(crate::commands::strings::LIBRARY_NO_FOLDER);
            return false;
        };
        self.library_load();
        let headings = headings_of(&buffer);
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
                file: g.file.clone(),
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
            doc: mdedit_core::split::document(&buffer),
            name: &name,
            path: Some(&path),
            headings: &headings,
            level: None,
            encoding,
            dest,
            groups,
            root,
            reader: fs_reader(),
        });
        screen.source.len = len;
        screen.source.sha256 = sha256;
        screen.ai_offer = self.docs.prefs.ai_connection != mdedit_core::ai::Connection::Off;
        // The Recycle Bin box (5.4), for this one document: disabled when its drive has no bin or
        // the system refuses, and otherwise while lines are in no note (the card works that out).
        screen.recycle = Some(crate::plan_ui::RecycleBox {
            wanted: false,
            refused: self.recycle.refusal(&path),
        });
        screen.blocked = self.plan_blocked(&screen);
        screen.blocked_for = Some(screen.dest.clone());
        self.plan_open(screen);
        true
    }

    /// Why nothing can be written to the group the plan names: the same checks the write makes
    /// before it writes anything (D2). Nothing is created to find out.
    fn plan_blocked(&self, screen: &PlanScreen) -> Option<String> {
        super::plan_write::plan_blocked(self, screen)
    }

    /// The destination or the plan changed: work the blocked reason out again, and repaint the
    /// card when it differs (C2: the check used to be made once, when the screen opened).
    fn plan_recheck(&mut self) {
        let Some(screen) = self.plan.as_ref() else {
            return;
        };
        // A demonstration writes nothing, and says so: it is never rechecked.
        if screen.source.path.is_none() {
            return;
        }
        if screen.blocked_for.as_ref() == Some(&screen.dest) {
            return;
        }
        let blocked = self.plan_blocked(screen);
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        screen.blocked = blocked;
        screen.blocked_for = Some(screen.dest.clone());
        screen.tick += 1;
    }

    /// Closes the screen; a plan that was changed is kept to be shown again.
    pub(super) fn plan_close(&mut self) {
        // A split still out is for this screen: it stops with it.
        if self.ai.split.is_some() {
            self.ai.cancel_split();
            self.docs.ai_status = None;
        }
        if let Some(screen) = self.plan.take()
            && screen.edited
        {
            self.plan_parked = Some(screen);
        }
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    /// "Show" on a plan that Escape kept: the same plan, as it was left. Everything it was
    /// blocked on is looked at again: the document may have changed since (A8), and so may the
    /// group's file or the library.
    pub(super) fn plan_show(&mut self) -> bool {
        match self.plan_parked.take() {
            Some(mut screen) => {
                screen.ai_offer = self.docs.prefs.ai_connection != mdedit_core::ai::Connection::Off;
                if screen.source.path.is_some() {
                    screen.blocked = self.plan_blocked(&screen);
                    screen.blocked_for = Some(screen.dest.clone());
                }
                self.plan_open(screen);
                true
            }
            None => false,
        }
    }

    /// "Drop" on a plan that Escape kept.
    pub(super) fn plan_drop(&mut self) {
        self.plan_parked = None;
        self.redraw();
    }

    fn plan_act(&mut self, act: Act) {
        match act {
            Act::Close => self.plan_close(),
            Act::Write => self.plan_write(),
            Act::Ai(a) => self.plan_ai(a),
            Act::Nothing => {}
        }
        self.plan_recheck();
        self.redraw();
    }

    pub(super) fn plan_key(&mut self, key: PlanKey, typed: Option<&str>) {
        if self.import_is_open() {
            return self.import_key(key);
        }
        if self.note_is_open() {
            return self.note_key(key);
        }
        if self.move_is_open() {
            return self.move_key(key);
        }
        let mods = self.mods;
        // Escape while the AI is asked stops the request and leaves the screen open.
        if self.ai.split.is_some()
            && matches!(&key, PlanKey::Name(crate::keys::KeyName::Escape))
            && self
                .plan
                .as_ref()
                .is_some_and(|s| s.edit.is_none() && s.menu.is_none())
        {
            self.split_stop();
            return;
        }
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        let (name, menu_key) = match &key {
            PlanKey::Name(n) => (Some(n), false),
            PlanKey::Insert => (None, false),
            PlanKey::Menu => (None, true),
        };
        let act = screen.key(name, menu_key, mods, typed);
        self.caret_reset();
        self.plan_act(act);
    }

    pub(super) fn plan_pointer(&mut self, x: f32, y: f32) {
        if self.import_is_open() {
            return self.import_pointer(x, y);
        }
        if self.note_is_open() {
            return self.note_pointer(x, y);
        }
        if self.move_is_open() {
            return self.move_pointer(x, y);
        }
        if let Some(screen) = self.plan.as_mut() {
            screen.pointer(x, y);
        }
        self.redraw();
    }

    pub(super) fn plan_press(&mut self) {
        if self.import_is_open() {
            return self.import_press();
        }
        if self.note_is_open() {
            return self.note_press();
        }
        if self.move_is_open() {
            return self.move_press();
        }
        let (x, y) = self.ui.mouse;
        if let Some(screen) = self.plan.as_mut() {
            screen.press(x, y);
        }
        // A press outside a name being typed commits it, and a new group is a new destination.
        self.plan_recheck();
        self.caret_reset();
        self.redraw();
    }

    pub(super) fn plan_release(&mut self) {
        if self.import_is_open() {
            return self.import_release();
        }
        if self.note_is_open() {
            return self.note_release();
        }
        if self.move_is_open() {
            return self.move_release();
        }
        let (x, y) = self.ui.mouse;
        let Some(screen) = self.plan.as_mut() else {
            return;
        };
        let act = screen.release(x, y);
        self.plan_act(act);
    }

    pub(super) fn plan_right_press(&mut self) {
        if self.import_is_open() {
            return;
        }
        let (x, y) = self.ui.mouse;
        if let Some(screen) = self.plan.as_mut() {
            screen.right_press(x, y);
        }
        self.redraw();
    }

    /// `dy` in logical px, positive towards the end.
    pub(super) fn plan_wheel(&mut self, dy: f32) {
        if self.import_is_open() {
            return self.import_wheel(dy);
        }
        if self.note_is_open() {
            return self.note_wheel(dy);
        }
        if self.move_is_open() {
            return self.move_wheel(dy);
        }
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

    /// `--plan-demo NAME`: a plan for a shot, made from a document built in the program. A
    /// demonstration writes nothing: its library folder is a name in the working folder, which is
    /// not where the program writes.
    pub(super) fn plan_demo(&mut self, name: &str) -> bool {
        // "Add a folder to the library…" (5.7): the import card's shots answer to `--plan-demo
        // folder…`, the name 5.7 gives them.
        if name.starts_with("folder") {
            return self.import_demo(name);
        }
        if matches!(name, "split-bin-report" | "split-bin-kept") {
            self.plan_bin_report_demo(name == "split-bin-report");
            return true;
        }
        match crate::plan_demo::screen(name) {
            Some(mut screen) => {
                screen.blocked =
                    Some("A demonstration of the plan screen: it writes nothing.".to_string());
                self.plan_open(screen);
                self.split_demo(name);
                true
            }
            None => false,
        }
    }

    /// `--plan-demo split-bin-report|split-bin-kept`: the report of a split whose original was
    /// put in the Recycle Bin, and is still listed in a group of the library; or whose original
    /// stayed, with the reason. Made in memory: nothing is read or written.
    fn plan_bin_report_demo(&mut self, recycled: bool) {
        use mdedit_core::runs::{Group, Kind, NoteRec, Record, Report, Source, State};
        let library = std::env::temp_dir()
            .join("mdedit-runs-demo")
            .join("library");
        let home = library.join("Japan 2025");
        let docs = if cfg!(windows) {
            r"D:\Docs"
        } else {
            "/home/me/Docs"
        };
        let source = PathBuf::from(docs).join("Japan trip journal.md");
        let titles = [
            ("Tokyo", "asakusa-and-the-river"),
            ("Kyoto", "the-temples-in-the-rain"),
            ("", "japan-trip-journal-the-rest"),
        ];
        let notes: Vec<PathBuf> = titles
            .iter()
            .map(|(section, name)| {
                let folder = if section.is_empty() {
                    home.clone()
                } else {
                    home.join(section)
                };
                folder.join(format!("{name}.md"))
            })
            .collect();
        let record = Record {
            version: 1,
            kind: Kind::Split,
            started: "2026-11-22T09:30:00Z".to_string(),
            started_secs: 0,
            state: State::Done,
            instruction: String::new(),
            sent: None,
            source: Source {
                path: source.clone(),
                len: 18_204,
                sha256: String::new(),
                recycled,
            },
            group: Group {
                name: "Japan 2025".to_string(),
                file: library.join("Japan 2025.md"),
                created: true,
                home: home.clone(),
            },
            folders: Vec::new(),
            notes: notes
                .iter()
                .map(|p| NoteRec {
                    path: p.clone(),
                    created: true,
                    sha256: String::new(),
                    ranges: Vec::new(),
                    heading_shift: 0,
                    title_added: false,
                    sections: Vec::new(),
                    line: String::new(),
                })
                .collect(),
            pictures: Vec::new(),
            sections_added: Vec::new(),
            moved: None,
            originals: Vec::new(),
        };
        let report = Report {
            record: Some(record),
            record_path: Some(
                library
                    .join(mdedit_core::runs::RUNS_DIR)
                    .join("2026-11-22T09-30-00Z-split.json"),
            ),
            notes_written: notes,
            linked: vec![library.join("Japan 2025.md")],
            closed: true,
            ..Default::default()
        };
        let mut outcome = crate::replace_ui::Outcome::run(&report, &library);
        if recycled {
            outcome.recycled(&source, &Ok(()), &["Trips".to_string()]);
        } else {
            outcome.recycled(
                &source,
                &Err("it has changed since the notes were cut from it".to_string()),
                &[],
            );
        }
        self.show_outcome(outcome);
        self.redraw();
    }
}
