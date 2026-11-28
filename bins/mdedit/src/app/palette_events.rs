//! The command palette as the window hosts it: it is drawn last, and while it is open every key,
//! pointer and wheel event goes to it and not to the page. Every outcome but "nothing" redraws.
//! Choosing closes it first, then acts, so what the choice starts (a dialog, the find box) begins
//! clean. Keys it does not use are dropped, except the shortcut that opened it, which closes it.

use super::*;
use crate::commands::CommandId as C;
use crate::keys::{self, KeyName};
use crate::palette::Target;
use mdshared_ui::palette::Outcome as Pick;

impl App {
    /// Opens the command palette with `query` in its box.
    pub(super) fn open_palette(&mut self, query: &str) {
        // A question or the settings on screen are answered first.
        if self.ui.dialog.is_some() || self.ui.settings.is_some() {
            return;
        }
        self.ui.popup = None;
        self.ui.hide_tip();
        self.ui.hover = None;
        self.ui.find_focused = false;
        self.ai_load_prompts();
        // "Show this file in the notes library" is listed by whether the library has it.
        self.library_load();
        self.palette
            .open(query, &self.docs, &self.ui, &mut self.macros);
        self.caret_reset();
        self.redraw();
    }

    /// Opens the language list, above the status bar's language field.
    pub(super) fn open_languages(&mut self) {
        if self.ui.dialog.is_some()
            || self.ui.settings.is_some()
            || self.docs.active_ready().is_none()
        {
            return;
        }
        self.ui.popup = None;
        self.ui.hide_tip();
        self.ui.hover = None;
        self.ui.find_focused = false;
        self.palette
            .open_languages(&self.docs, &self.ui, &mut self.macros);
        self.caret_reset();
        self.redraw();
    }

    /// Makes `lang` the active document's language (from the language list).
    fn choose_language(&mut self, lang: Option<mdedit_highlight::Language>) {
        let Some(i) = self.docs.active() else {
            return;
        };
        let dark = self.dark();
        if self.docs.set_language(i, lang, self.service.as_ref(), dark) {
            self.full = true;
        }
    }

    pub(super) fn close_palette(&mut self) {
        self.palette.close();
        self.ui.pressed = None;
        self.ui.hover = None;
        self.set_cursor(CursorIcon::Default);
        self.redraw();
    }

    /// The palette is open: it takes every key, pointer and wheel event.
    pub(super) fn palette_open(&self) -> bool {
        self.palette.open.is_some()
    }

    /// The entries after the query changed.
    fn palette_synced(&mut self) {
        self.palette
            .sync(&self.docs, &self.ui, &mut self.macros, false);
        self.palette_changed();
    }

    fn palette_changed(&mut self) {
        self.palette.tick += 1;
        self.redraw();
    }

    pub(super) fn palette_key(&mut self, name: Option<KeyName>, typed: Option<&str>) {
        let Some(name) = name else { return };
        let mods = self.mods;
        let opener = keys::command_for(&keys::Key::Name(name.clone()), mods);
        let Some(p) = self.palette.open.as_mut() else {
            return;
        };
        // On the pointer route the query box's edit menu takes the keys first, and Shift+F10
        // opens it at the caret.
        match p.palette.key_with_menu(&name, mods, typed, &mut self.gfx) {
            Pick::Unused => {
                if opener == Some(C::CommandPalette) {
                    self.close_palette();
                }
            }
            out => self.palette_outcome(out),
        }
        self.caret_reset();
    }

    /// The pointer moved: the query box's drag goes on (its text scrolls when the pointer is
    /// past an end), its edit menu lights a row, and the rest of the card follows the pointer.
    pub(super) fn palette_pointer(&mut self, x: f32, y: f32) {
        let now = Instant::now();
        let gfx = &mut self.gfx;
        let (moved, cursor) = match self.palette.open.as_mut() {
            Some(p) => (
                p.palette.pointer_motion((x, y), now, gfx),
                p.palette.cursor_at((x, y)).map(|c| c.icon()),
            ),
            None => (false, None),
        };
        let edge = self.edge_at(x, y).map(CursorIcon::from);
        self.set_cursor(cursor.or(edge).unwrap_or(CursorIcon::Default));
        if moved {
            self.palette_changed();
        }
    }

    /// A press: the query box counts its own clicks (a word, then all of it), takes a press
    /// in its padding or on the magnifier, and extends with Shift; `clicks` goes to the rows.
    pub(super) fn palette_press(&mut self, clicks: u32) {
        let (x, y) = self.ui.mouse;
        let (mods, clicks) = (self.mods, clicks.min(2) as u8);
        if let Some(p) = self.palette.open.as_mut() {
            let out = p
                .palette
                .pointer_press((x, y), mods, clicks, Instant::now(), &mut self.gfx);
            self.palette_outcome(out);
        }
        self.caret_reset();
    }

    pub(super) fn palette_release(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(p) = self.palette.open.as_mut() {
            let out = p
                .palette
                .pointer_release((x, y), Instant::now(), &mut self.gfx);
            self.palette_outcome(out);
        }
    }

    /// A right press: on the query box it opens the box's edit menu; outside the card it
    /// closes the palette, as a left press does.
    pub(super) fn palette_right_press(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(p) = self.palette.open.as_mut() {
            let out = p
                .palette
                .secondary_press((x, y), Instant::now(), &mut self.gfx);
            self.palette_outcome(out);
        }
        self.caret_reset();
    }

    /// The query box's drag held past one of its ends scrolls its text on, and its edit menu's
    /// submenu delay ends: when that is due.
    pub(super) fn palette_text_deadline(&self) -> Option<Instant> {
        self.palette.open.as_ref()?.palette.text_deadline()
    }

    pub(super) fn palette_text_tick(&mut self, now: Instant) {
        let moved = self
            .palette
            .open
            .as_mut()
            .is_some_and(|p| p.palette.tick_text(now, &mut self.gfx));
        if moved {
            self.caret_reset();
            self.palette_changed();
            self.wants_frame = true;
        }
    }

    /// `dy` in logical px, positive towards the end.
    pub(super) fn palette_wheel(&mut self, dy: f32) {
        if self
            .palette
            .open
            .as_mut()
            .is_some_and(|p| p.palette.wheel(dy))
        {
            self.palette_changed();
        }
    }

    fn palette_outcome(&mut self, out: Pick) {
        match out {
            Pick::None | Pick::Unused => {}
            Pick::Redraw => self.palette_changed(),
            Pick::QueryChanged => self.palette_synced(),
            Pick::Closed => self.close_palette(),
            Pick::Submitted => {
                let line = self.palette.open.as_ref().and_then(|p| p.line_query());
                if let Some((line, col)) = line {
                    self.close_palette();
                    self.go_to_line(line, col);
                }
            }
            Pick::Chosen(id) => {
                let target = self
                    .palette
                    .open
                    .as_ref()
                    .and_then(|p| p.target(id))
                    .cloned();
                match target {
                    Some(Target::Prefix(prefix)) => {
                        if let Some(p) = self.palette.open.as_mut() {
                            p.palette.set_query(prefix);
                        }
                        self.palette_synced();
                    }
                    Some(target) => {
                        self.close_palette();
                        self.run_target(target);
                    }
                    None => {}
                }
            }
        }
    }

    fn go_to_line(&mut self, line: usize, col: usize) {
        if self.docs.go_to(line, col) {
            self.ui.find_focused = false;
            self.caret_reset();
        }
    }

    fn run_target(&mut self, target: Target) {
        match target {
            Target::Cmd(id) => self.command(id),
            Target::Tab(doc) => {
                if let Some(i) = self.docs.index_of_id(doc) {
                    self.docs.activate(i);
                    self.ui.find_focused = false;
                    self.caret_reset();
                }
            }
            Target::Recent(path) | Target::File(path) => self.open(&path),
            Target::Symbol(line) => self.go_to_line(line + 1, 1),
            Target::AiPrompt(k) => self.ai_saved(k),
            Target::Language(lang) => self.choose_language(lang),
            Target::Note(path) => {
                self.open_note(&path);
            }
            Target::LibraryFile(path) => self.open(&path),
            Target::NotePlace(file, section) => self.library_put(&file, &section),
            Target::NoteMoveTo(file, section) => self.library_move_to(&file, &section),
            Target::Prefix(_) => {}
        }
        self.redraw();
    }
}

impl App {
    /// `--shot`: waits for the lists a worker is making for the open palette.
    pub(super) fn settle_palette(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.palette.pending() && Instant::now() < deadline {
            if self.palette.poll() {
                self.palette
                    .sync(&self.docs, &self.ui, &mut self.macros, true);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if self.palette.poll() {
            self.palette
                .sync(&self.docs, &self.ui, &mut self.macros, true);
        }
    }
}
