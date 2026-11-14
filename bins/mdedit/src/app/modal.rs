//! The settings dialog as the window hosts it: the form is `mdshared_ui::settings::Form`, built
//! from the preferences each time it opens; every change comes back as an outcome, is applied to
//! every open document at once and is written to the settings file (the `[preferences]` section).
//! Keys, the pointer and the wheel go to the form while it is open, and the page does not see them.

use super::*;
use crate::commands::CommandId as C;
use crate::{keys, updater};
use mdedit_core::prefs::{FONT_PT_MAX, FONT_PT_MIN, INDENT_SIZE_MAX, Prefs, TAB_WIDTH_MAX};
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::settings::{Form, Outcome as Out, Row, Value};

pub const FONT_FAMILY: u32 = 1;
pub const FONT_SIZE: u32 = 2;
pub const TAB_WIDTH: u32 = 3;
pub const INDENT_SPACES: u32 = 4;
pub const INDENT_SIZE: u32 = 5;
pub const AUTO_PAIR: u32 = 6;
pub const AUTO_INDENT: u32 = 7;
pub const TRIM_ON_SAVE: u32 = 8;
pub const FINAL_NEWLINE: u32 = 9;
pub const LINE_NUMBERS: u32 = 10;
pub const HIGHLIGHT_LINE: u32 = 11;
pub const MINIMAP: u32 = 12;
pub const WORD_WRAP: u32 = 13;
pub const THEME: u32 = 14;
pub const RESTORE_TABS: u32 = 15;
pub const CHECK_AT_LAUNCH: u32 = 16;
pub const CHECK_NOW: u32 = 17;
pub const DEFAULT_APP: u32 = 18;

const THEMES: [&str; 3] = ["System", "Light", "Dark"];

fn theme_index(c: ThemeChoice) -> usize {
    match c {
        ThemeChoice::System => 0,
        ThemeChoice::Light => 1,
        ThemeChoice::Dark => 2,
    }
}

fn theme_of(i: usize) -> ThemeChoice {
    match i {
        1 => ThemeChoice::Light,
        2 => ThemeChoice::Dark,
        _ => ThemeChoice::System,
    }
}

/// Whether the System group (make mdedit the default for text files) is offered: off Windows,
/// from an AppImage, until it is the default (as the main menu has it).
fn can_offer_default_app() -> bool {
    #[cfg(not(windows))]
    {
        mdshared_app::appimage::path().is_some()
            && !mdshared_ui::desktop_entry::is_default(
                strings::APP_NAME,
                mdshared_ui::desktop_entry::EDITOR_MIME_TYPES,
            )
    }
    #[cfg(windows)]
    {
        false
    }
}

/// The rows, from the preferences and what else the window knows.
pub fn rows(prefs: &Prefs, theme: ThemeChoice, check_updates: bool, look: &str) -> Vec<Row> {
    let mut rows = vec![
        Row::header("Editing"),
        Row::text(FONT_FAMILY, "Font", prefs.font_family.clone())
            .placeholder("Default")
            .help(
                "A monospace family, such as Cascadia Mono. One that is not installed is skipped.",
            ),
        Row::number(
            FONT_SIZE,
            "Font size",
            prefs.font_pt.round() as i32,
            FONT_PT_MIN as i32,
            FONT_PT_MAX as i32,
        )
        .unit("pt")
        .help("At 100 % zoom. Zoom (Ctrl and the wheel) scales it."),
        Row::number(
            TAB_WIDTH,
            "Tab width",
            prefs.tab_width as i32,
            1,
            TAB_WIDTH_MAX as i32,
        )
        .unit("columns"),
        Row::toggle(
            INDENT_SPACES,
            "Indent with spaces",
            prefs.indent_with_spaces,
        )
        .help("Tab and the automatic indent add spaces, not a tab character."),
        Row::number(
            INDENT_SIZE,
            "Indent size",
            prefs.indent_size as i32,
            1,
            INDENT_SIZE_MAX as i32,
        )
        .unit("spaces"),
        Row::toggle(AUTO_PAIR, "Close brackets and quotes", prefs.auto_pair)
            .help("Typing ( or \" adds the other half."),
        Row::toggle(AUTO_INDENT, "Automatic indent", prefs.auto_indent)
            .help("Enter keeps the indent and opens a block after a bracket."),
        Row::header("Saving"),
        Row::toggle(
            TRIM_ON_SAVE,
            "Trim trailing spaces on save",
            prefs.trim_on_save,
        ),
        Row::toggle(
            FINAL_NEWLINE,
            "End the file with a line break on save",
            prefs.final_newline_on_save,
        ),
        Row::header("View"),
        Row::toggle(LINE_NUMBERS, "Line numbers", prefs.line_numbers),
        Row::toggle(
            HIGHLIGHT_LINE,
            "Highlight the current line",
            prefs.highlight_line,
        ),
        Row::toggle(MINIMAP, "Minimap for new windows", prefs.minimap)
            .help("View > Minimap changes the window you are in."),
        Row::toggle(WORD_WRAP, "Word wrap for new documents", prefs.word_wrap)
            .help("Alt+Z changes the document you are in."),
        Row::choice(THEME, "Theme", &THEMES, theme_index(theme)),
        Row::header("Session"),
        Row::toggle(RESTORE_TABS, "Restore tabs at start", prefs.restore_session),
        Row::header("Updates"),
        Row::toggle(
            CHECK_AT_LAUNCH,
            "Check for updates at launch",
            check_updates,
        ),
        Row::action(CHECK_NOW, "Look for a newer version", look),
    ];
    if can_offer_default_app() {
        rows.push(Row::header("System"));
        rows.push(Row::action(
            DEFAULT_APP,
            "Make mdedit the default for text files",
            "Make default",
        ));
    }
    rows
}

impl App {
    pub(super) fn open_settings(&mut self) {
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.hover = None;
        self.ui.find_focused = false;
        self.ui.dialog = None;
        let look = self.docs.updates.about_label();
        let rows = rows(
            &self.docs.prefs,
            self.docs.theme,
            self.docs.check_updates,
            &look,
        );
        self.ui.settings = Some(Form::new("Settings", rows));
        self.modal_changed();
    }

    pub(super) fn close_settings(&mut self) {
        self.ui.settings = None;
        self.ui.pressed = None;
        self.ui.hover = None;
        self.set_cursor(CursorIcon::Default);
        self.modal_changed();
    }

    fn modal_changed(&mut self) {
        self.ui.overlay_tick += 1;
        self.redraw();
    }

    /// A key while the form is open. The key that opened it closes it.
    pub(super) fn settings_key(&mut self, name: &KeyName, mods: Mods, typed: Option<&str>) {
        let opener = keys::command_for(&keys::Key::Name(name.clone()), mods);
        let Some(f) = self.ui.settings.as_mut() else {
            return;
        };
        match f.key(name, mods, typed) {
            Out::Unused => {
                if opener == Some(C::Settings) {
                    self.close_settings();
                }
            }
            out => self.settings_outcome(out),
        }
        self.caret_reset();
    }

    pub(super) fn settings_pointer(&mut self, x: f32, y: f32) {
        let moved = self
            .ui
            .settings
            .as_mut()
            .is_some_and(|f| f.pointer_move(x, y));
        let edge = self.edge_at(x, y).map(CursorIcon::from);
        self.set_cursor(edge.unwrap_or(CursorIcon::Default));
        if moved {
            self.modal_changed();
        }
    }

    pub(super) fn settings_press(&mut self) {
        let (x, y) = self.ui.mouse;
        let mods = self.mods;
        if let Some(f) = self.ui.settings.as_mut() {
            let out = f.press(x, y, mods, 1);
            self.settings_outcome(out);
        }
        self.caret_reset();
    }

    pub(super) fn settings_release(&mut self) {
        let (x, y) = self.ui.mouse;
        if let Some(f) = self.ui.settings.as_mut() {
            let out = f.release(x, y);
            self.settings_outcome(out);
        }
    }

    /// `dy` in logical px, positive towards the end.
    pub(super) fn settings_wheel(&mut self, dy: f32) {
        let (x, y) = self.ui.mouse;
        if self.ui.settings.as_mut().is_some_and(|f| f.wheel(x, y, dy)) {
            self.modal_changed();
        }
    }

    fn settings_outcome(&mut self, out: Out) {
        match out {
            Out::None | Out::Unused => {}
            Out::Redraw => self.modal_changed(),
            Out::Closed => self.close_settings(),
            Out::Changed { id, value } => {
                self.apply_setting(id, value);
                self.modal_changed();
            }
            Out::Action(id) => {
                // The answer shows on the page (a bar or a toast): the dialog goes, as About's
                // buttons do.
                self.close_settings();
                match id {
                    CHECK_NOW => {
                        if matches!(self.docs.updates.stage(), updater::Stage::Available(_)) {
                            self.install_update();
                        } else {
                            self.check_for_updates();
                        }
                    }
                    DEFAULT_APP => self.command(C::MakeDefault),
                    _ => {}
                }
            }
        }
    }

    /// Applies one change to every open document and keeps it for the next start.
    pub(super) fn apply_setting(&mut self, id: u32, value: Value) {
        let mut p = self.docs.prefs.clone();
        match (id, value) {
            (FONT_FAMILY, Value::Text(t)) => p.font_family = t,
            (FONT_SIZE, Value::Number(n)) => p.font_pt = n as f32,
            (TAB_WIDTH, Value::Number(n)) => p.tab_width = n.max(1) as usize,
            (INDENT_SPACES, Value::Bool(on)) => p.indent_with_spaces = on,
            (INDENT_SIZE, Value::Number(n)) => p.indent_size = n.max(1) as usize,
            (AUTO_PAIR, Value::Bool(on)) => p.auto_pair = on,
            (AUTO_INDENT, Value::Bool(on)) => p.auto_indent = on,
            (TRIM_ON_SAVE, Value::Bool(on)) => p.trim_on_save = on,
            (FINAL_NEWLINE, Value::Bool(on)) => p.final_newline_on_save = on,
            (LINE_NUMBERS, Value::Bool(on)) => p.line_numbers = on,
            (HIGHLIGHT_LINE, Value::Bool(on)) => p.highlight_line = on,
            (MINIMAP, Value::Bool(on)) => p.minimap = on,
            (WORD_WRAP, Value::Bool(on)) => p.word_wrap = on,
            (RESTORE_TABS, Value::Bool(on)) => p.restore_session = on,
            (THEME, Value::Choice(i)) => {
                self.set_theme(theme_of(i));
                return;
            }
            (CHECK_AT_LAUNCH, Value::Bool(on)) => {
                self.docs.check_updates = on;
                return;
            }
            _ => return,
        }
        p.clamp();
        self.set_prefs(p);
    }

    /// Takes new preferences: typing help, the drawing, and the session file follow.
    pub(super) fn set_prefs(&mut self, prefs: Prefs) {
        self.docs.set_prefs(prefs.clone());
        self.session.prefs = prefs;
        if crate::live::apply(&self.session.prefs) {
            self.relayout_all();
        }
        self.wants_frame = true;
    }

    /// The text size, tab width, line numbers or family changed: every layout is stale.
    pub(super) fn relayout_all(&mut self) {
        self.view.clear();
        self.view.metrics = None;
        for doc in &mut self.docs.tabs {
            if let Some(r) = doc.ready_mut() {
                r.chunks = crate::view::cache::Lru::new(crate::view::CHUNK_INDEXES);
                r.widest = 0.0;
                r.rows_stamp = (u64::MAX, 0, 0, 0);
            }
        }
        self.full = true;
        self.redraw();
    }
}
