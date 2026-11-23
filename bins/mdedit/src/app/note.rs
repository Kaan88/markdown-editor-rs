//! "A Markdown note from a file…" as the window hosts it (`docs/ai-library-plan.md` 5.8, phase 5;
//! `note_ui.rs` is the card and `note_draw.rs` draws it). Offered on a group's row: the file is
//! picked, read and sent, and its answer becomes a new note in that group's folder, written
//! through `mdedit_core::runs` as a [`Kind::Markdown`](mdedit_core::runs::Kind::Markdown) run. The
//! original is never renamed and never changed; the record it leaves is undone from "AI runs…"
//! like any other.
//!
//! Everything that can be refused is refused before the request goes out and again before a byte
//! is written: no library folder, a group's folder that is the library folder, a group's library
//! file open with unsaved changes, the group's 2,000 notes, a source that changed while the answer
//! was on its way, and a link that would not read back.

use super::*;
use crate::commands::strings;
use crate::note_ui::{Act, NoteCard, Phase, Source, Target};
use crate::replace_ui::Outcome;
use mdedit_core::ai::Connection;
use mdedit_core::load::{LoadOptions, load_path};
use mdedit_core::runs::{self, Group, NewNote, Request, Sent};
use mdshared_app::library;
use mdshared_app::paths::same_path;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

/// A source larger than this is refused whole: it is the one-request limit of the AI (5.6).
const NOTE_MAX: u64 = 200_000;

/// Why a picked file cannot be a note's source, or the text it holds.
fn read_source(path: &Path) -> Result<Source, String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let meta = std::fs::metadata(path).map_err(|e| format!("{name} could not be read: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{name} is not a file."));
    }
    if meta.len() > NOTE_MAX {
        return Err(format!(
            "{name} is too long for one request: {} KB of {} KB.",
            meta.len().div_ceil(1000),
            NOTE_MAX / 1000
        ));
    }
    if meta.len() == 0 {
        return Err(format!("{name} is empty."));
    }
    let opts = LoadOptions {
        max_bytes: NOTE_MAX,
        ..LoadOptions::default()
    };
    let cancel = AtomicBool::new(false);
    let loaded = load_path(path, &opts, &|_, _| {}, &cancel)
        .map_err(|e| format!("{name} could not be read: {e}"))?;
    if loaded.info.lossy || loaded.info.has_nul {
        return Err(format!(
            "{name} is not a text file: some of its bytes are not text this editor can read."
        ));
    }
    let mut text = String::new();
    loaded.buffer.text(0..loaded.buffer.len_bytes(), &mut text);
    if text.trim().is_empty() {
        return Err(format!("{name} has no text."));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{name} could not be read: {e}"))?;
    let encoding = match loaded.info.encoding {
        mdedit_core::encoding::Encoding::Utf8 | mdedit_core::encoding::Encoding::Utf8Bom => None,
        e => Some(format!("{}, written as UTF-8", e.label())),
    };
    Ok(Source {
        path: path.to_path_buf(),
        name,
        lines: text.lines().count(),
        len: bytes.len() as u64,
        sha256: mdshared_app::update::sha256_hex(&bytes),
        eol: loaded.info.eol.dominant().to_string(),
        encoding,
        text,
    })
}

impl App {
    pub(super) fn note_is_open(&self) -> bool {
        self.note.is_some()
    }

    /// Opens `card` over the window.
    pub(super) fn note_open_card(&mut self, mut card: NoteCard) {
        if self.palette_open() {
            self.close_palette();
        }
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.hover = None;
        self.ui.find_focused = false;
        let (w, h) = self.logical();
        card.resize(w, h);
        self.note = Some(card);
        self.caret_reset();
        self.redraw();
    }

    /// "Markdown note from a file…" on a group's row: the picker, on the folder last opened from.
    pub(super) fn note_open_group(&mut self, file: &Path) {
        if self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
            .is_none()
        {
            self.docs.set_toast(strings::LIBRARY_NO_FOLDER);
            return;
        }
        self.library_load();
        let file = file.to_path_buf();
        let start = self.start_dir();
        #[cfg(windows)]
        {
            let picked = platform::pick_files(start.as_deref()).into_iter().next();
            self.note_file_picked(file, picked);
        }
        #[cfg(not(windows))]
        if !self.dialog_busy {
            self.dialog_busy = true;
            let wake = self.docs.waker();
            platform::pick_files_async(start.as_deref(), move |files| {
                wake(Wake::Dialog(Picked::NoteFile(file, files)));
            });
        }
    }

    /// The picker answered: the file is read, the group's folder and the note's name worked out,
    /// and the card opens with everything that would refuse the note already said on it.
    pub(super) fn note_file_picked(&mut self, group: PathBuf, picked: Option<PathBuf>) {
        let Some(path) = picked else {
            return;
        };
        let source = match read_source(&path) {
            Ok(source) => source,
            Err(why) => {
                self.docs.set_toast(&why);
                self.redraw();
                return;
            }
        };
        let (target, blocked) = match self.note_target(&group, &source) {
            Ok(pair) => pair,
            Err(why) => {
                self.docs.set_toast(&why);
                self.redraw();
                return;
            }
        };
        let prompt = mdedit_core::ai::prompt(mdedit_core::ai::FORMAT, &source.text);
        let via = self.ai_via();
        let card = NoteCard::new(source, target, prompt, via, blocked);
        self.note_open_card(card);
    }

    /// Where the note goes, and why it cannot be written there (or `None`). The name is a free
    /// one now: `stem.md`, or `stem 2.md` when that name is taken (3.9, D-075).
    fn note_target(
        &self,
        group_file: &Path,
        source: &Source,
    ) -> Result<(Target, Option<String>), String> {
        let Some(root) = self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
        else {
            return Err(strings::LIBRARY_NO_FOLDER.to_string());
        };
        let Some(group) = self
            .docs
            .library
            .groups
            .iter()
            .find(|g| same_path(&g.file, group_file))
            .cloned()
        else {
            return Err("That group is not in the library any more.".to_string());
        };
        let home = group
            .home
            .clone()
            .unwrap_or_else(|| root.join(library::folder_name(&group.name)));
        let stem_ish = Path::new(&source.name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| source.name.clone());
        let title = stem_ish.trim().to_string();
        let Some(stem) = super::library_write::note_stem_for(&title) else {
            return Err(format!("{} has no name to give a note.", source.name));
        };
        let path = super::library_write::free_name(&home, &stem);
        let target = Target {
            group: group.name.clone(),
            file: group.file.clone(),
            home,
            path,
            title,
        };
        let blocked = self.note_blocked(&root, &target);
        Ok((target, blocked))
    }

    /// Everything that would refuse the note, in the footer's words; `None` when it can be made.
    fn note_blocked(&self, root: &Path, target: &Target) -> Option<String> {
        if self.ai_conn() == Connection::Off {
            return Some(
                "AI is off. Choose a connection in Settings \u{203A} AI first.".to_string(),
            );
        }
        if same_path(&target.home, root) {
            return Some(format!(
                "{} is the library folder itself, so the note would be a library file. Choose another folder for the group.",
                target.home.display()
            ));
        }
        if target.path.parent().is_some_and(|p| same_path(p, root)) {
            return Some(format!(
                "{} would be a library file, not a note.",
                target.path.display()
            ));
        }
        if target.home.exists() && !target.home.is_dir() {
            return Some(format!(
                "{} is a file, not a folder.",
                target.home.display()
            ));
        }
        if let Err(why) = self.library_file_free(&target.file) {
            return Some(why);
        }
        let text = match std::fs::read_to_string(&target.file) {
            Ok(text) => text,
            Err(e) => {
                return Some(format!("{} could not be read: {e}", target.file.display()));
            }
        };
        let parsed = library::parse(&text, root);
        if parsed.entries.len().saturating_add(1) > library::MAX_ENTRIES {
            return Some(format!(
                "{} would have more than {} notes.",
                target.group,
                library::MAX_ENTRIES
            ));
        }
        // The dry run: the line exactly as the write would put it, against the file as it reads.
        let note = self.note_row(root, target, String::new());
        let link = runs::Link::new(&note);
        let new = runs::with_links_added(&text, std::slice::from_ref(&link));
        if !runs::check_links_added(&parsed, &library::parse(&new, root), &[link], root) {
            return Some(format!(
                "{} would not read back as its old notes plus this one.",
                target.group
            ));
        }
        None
    }

    /// A note of the request's shape: its path, title and the line that lists it.
    fn note_row(&self, root: &Path, target: &Target, text: String) -> NewNote {
        NewNote {
            path: target.path.clone(),
            from_source: text.clone(),
            text,
            title: target.title.clone(),
            link_path: crate::library_pane::link_path(root, &target.path),
            sections: Vec::new(),
            ranges: Vec::new(),
            heading_shift: 0,
            title_added: false,
            already: false,
        }
    }

    pub(super) fn note_close(&mut self) {
        if let Some(id) = self
            .note
            .as_ref()
            .and_then(|c| c.asking.as_ref().map(|a| a.id))
        {
            self.ai.cancel_id(id);
            self.docs.ai_status = None;
        }
        self.note = None;
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    fn note_act(&mut self, act: Act) {
        match act {
            Act::Close => self.note_close(),
            Act::Send => self.note_send(),
            Act::Write => self.note_write(),
            Act::Stop => self.note_stop(),
            Act::Nothing => {}
        }
        self.redraw();
    }

    pub(super) fn note_key(&mut self, key: crate::keys::Key) {
        let mods = self.mods;
        let Some(card) = self.note.as_mut() else {
            return;
        };
        let act = card.key(&key, mods);
        self.note_act(act);
    }

    pub(super) fn note_pointer(&mut self, x: f32, y: f32) {
        if let Some(card) = self.note.as_mut() {
            card.pointer(x, y);
        }
        self.redraw();
    }

    pub(super) fn note_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(card) = self.note.as_mut() {
            card.press(x, y);
        }
        self.redraw();
    }

    pub(super) fn note_release(&mut self) {
        let (x, y) = self.ui.mouse;
        let Some(card) = self.note.as_mut() else {
            return;
        };
        let act = card.release(x, y);
        self.note_act(act);
    }

    pub(super) fn note_wheel(&mut self, dy: f32) {
        if let Some(card) = self.note.as_mut() {
            card.wheel(dy);
        }
        self.redraw();
    }

    /// "Make the note": the whole file goes, with the built-in Format as Markdown instruction.
    fn note_send(&mut self) {
        let Some(card) = self.note.as_ref() else {
            return;
        };
        if card.demo {
            if let Some(card) = self.note.as_mut() {
                card.notice = Some("A demonstration of the card: it sends nothing.".to_string());
                card.tick += 1;
            }
            return;
        }
        if card.blocked.is_some() || card.phase != Phase::Ready {
            return;
        }
        if self.ai.running() {
            if let Some(card) = self.note.as_mut() {
                card.notice = Some(
                    "An AI rewrite is running. Stop it first (Esc in the editor).".to_string(),
                );
                card.tick += 1;
            }
            return;
        }
        if self.ai_conn() == Connection::Off {
            if let Some(card) = self.note.as_mut() {
                card.blocked = Some(
                    "AI is off. Choose a connection in Settings \u{203A} AI first.".to_string(),
                );
                card.tick += 1;
            }
            return;
        }
        let (prompt, via) = (card.prompt.clone(), card.via.clone());
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let req = json!({
            "op": "run",
            "conn": crate::ai::conn_json(c, p),
            "model": p.ai_model(c),
            "effort": p.ai_effort(c),
            "system": mdedit_core::ai::SYSTEM,
            "prompt": prompt,
        });
        let wake = self.docs.waker();
        match self.ai.ask(&wake, crate::ai::Ask::Note, req) {
            Ok(id) => {
                if let Some(card) = self.note.as_mut() {
                    card.set_asking(id);
                }
                self.docs.ai_status = Some(format!(
                    "AI: writing a Markdown note\u{2026} ({via})   Esc stops"
                ));
            }
            Err(why) => {
                if let Some(card) = self.note.as_mut() {
                    card.notice = Some(why);
                    card.tick += 1;
                }
            }
        }
        self.redraw();
    }

    /// Stop: the request is dropped and the card goes back to what it showed before it.
    pub(super) fn note_stop(&mut self) {
        if let Some(id) = self
            .note
            .as_ref()
            .and_then(|c| c.asking.as_ref().map(|a| a.id))
        {
            self.ai.cancel_id(id);
        }
        self.docs.ai_status = None;
        if let Some(card) = self.note.as_mut() {
            card.set_stopped(None);
        }
        self.redraw();
    }

    /// The answer came: the note's text is made from it, with the file's own line endings. An
    /// empty answer, or one that is the file's text word for word, writes nothing and says so.
    pub(super) fn note_done(&mut self, text: &str) {
        let Some(card) = self.note.as_mut() else {
            return;
        };
        if card.asking.is_none() {
            return;
        }
        self.docs.ai_status = None;
        let eol = card.source.eol.clone();
        let body = mdedit_core::ai::note_body(text, &eol);
        if body.is_empty() {
            card.set_stopped(Some(
                "The AI sent back no text. Nothing was written.".to_string(),
            ));
        } else if body == mdedit_core::ai::note_body(&card.source.text, &eol) {
            card.set_stopped(Some(
                "The AI changed nothing, so there is no note to write.".to_string(),
            ));
        } else {
            card.set_answer(text.to_string(), &eol);
        }
        self.redraw();
    }

    /// The request failed, or the engine ended. A cancellation is not news.
    pub(super) fn note_failed(&mut self, kind: &str, message: &str) {
        self.docs.ai_status = None;
        let Some(card) = self.note.as_mut() else {
            return;
        };
        if card.asking.is_none() {
            return;
        }
        card.set_stopped((kind != "cancelled").then(|| message.to_string()));
        self.redraw();
    }

    /// "Write the note": the checks again, then the write of 4.3 with `Kind::Markdown`, then the
    /// report, the note open in a tab, and the Notes pane on it.
    fn note_write(&mut self) {
        let Some(mut card) = self.note.take() else {
            return;
        };
        if card.demo {
            card.notice = Some("A demonstration of the card: it writes nothing.".to_string());
            card.tick += 1;
            self.note = Some(card);
            return;
        }
        let root = match self
            .library_dir()
            .filter(|d| !d.as_os_str().is_empty() && d.is_absolute())
        {
            Some(root) => root,
            None => return self.note_write_refused(card, strings::LIBRARY_NO_FOLDER),
        };
        // The library, the file and the group as they read now: the card's answer may be minutes
        // old (5.10).
        if let Some(why) = self.note_blocked(&root, &card.target) {
            return self.note_write_refused(card, &why);
        }
        match std::fs::read(&card.source.path) {
            Ok(bytes)
                if bytes.len() as u64 == card.source.len
                    && mdshared_app::update::sha256_hex(&bytes)
                        .eq_ignore_ascii_case(&card.source.sha256) => {}
            Ok(_) => {
                let why = format!(
                    "{} has changed since the note was made. Make the note again.",
                    card.source.name
                );
                return self.note_write_refused(card, &why);
            }
            Err(e) => {
                let why = format!("{} could not be read: {e}", card.source.name);
                return self.note_write_refused(card, &why);
            }
        }
        let note = self.note_row(&root, &card.target, card.result.clone());
        let folders = if card.target.home.is_dir() {
            Vec::new()
        } else {
            vec![card.target.home.clone()]
        };
        let request = Request {
            library: root.clone(),
            source: card.source.path.clone(),
            instruction: mdedit_core::ai::FORMAT.to_string(),
            sent: Some(Sent {
                connection: self.docs.prefs.ai_connection.key().to_string(),
                model: self
                    .docs
                    .prefs
                    .ai_model(self.docs.prefs.ai_connection)
                    .to_string(),
                bytes: card.prompt.len() as u64,
                requests: card.asking.as_ref().map_or(1, |a| a.requests.max(1)),
            }),
            group: Group {
                name: card.target.group.clone(),
                file: card.target.file.clone(),
                created: false,
                home: card.target.home.clone(),
            },
            folders,
            notes: vec![note],
            pictures: Vec::new(),
        };
        let report = {
            let mut gate = super::plan_write::PlanGate {
                app: self,
                library: root.clone(),
            };
            runs::write_note(&request, &mut gate)
        };
        if let Some(why) = report.start_error.clone() {
            return self.note_write_refused(card, &format!("The note could not be written: {why}"));
        }
        self.library_rows();
        if let Some(path) = report
            .notes_written
            .first()
            .or_else(|| report.notes_present.first())
            .cloned()
        {
            if report.link_failures.is_empty() {
                self.open(&path);
            }
            self.docs.set_toast(&format!(
                "The note is written: {}. The original is where it was.",
                path.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.display().to_string())
            ));
        }
        self.open_notes_of(&report, &request);
        self.show_outcome(Outcome::run(&report, &root));
        self.redraw();
    }

    fn note_write_refused(&mut self, mut card: NoteCard, why: &str) {
        self.docs.set_toast(why);
        card.blocked = Some(why.to_string());
        card.tick += 1;
        self.note = Some(card);
        self.redraw();
    }

    /// `--note-demo NAME`: the card over a file and an answer made in the program. It sends
    /// nothing and writes nothing.
    pub(super) fn note_demo(&mut self, name: &str) -> bool {
        match crate::note_ui::demo(name) {
            Some(card) => {
                self.note_open_card(card);
                true
            }
            None => false,
        }
    }
}
