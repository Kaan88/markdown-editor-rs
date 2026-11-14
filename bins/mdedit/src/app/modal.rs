//! The settings dialog as the window hosts it: the form is `mdshared_ui::settings::Form`, built
//! from the preferences each time it opens; every change comes back as an outcome, is applied to
//! every open document at once and is written to the settings file (the `[preferences]` section).
//! Keys, the pointer and the wheel go to the form while it is open, and the page does not see them.

use super::*;
use crate::commands::CommandId as C;
use crate::{keys, updater};
use mdedit_core::prefs::{FONT_PT_MAX, FONT_PT_MIN, INDENT_SIZE_MAX, Prefs, TAB_WIDTH_MAX};
use mdshared_ui::corners::{self, Corners};
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
pub const LIVE_PREVIEW: u32 = 19;
pub const DETECT_INDENT: u32 = 20;
pub const WINDOW_CORNERS: u32 = 21;

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

/// The rows, from the preferences and what else the window knows. `corners` is the window's corner
/// setting, or `None` where the system gives the window no way to round them (the row is left out).
pub fn rows(
    prefs: &Prefs,
    theme: ThemeChoice,
    corners: Option<Corners>,
    check_updates: bool,
    look: &str,
) -> Vec<Row> {
    let mut rows = vec![
        Row::header("Editing"),
        Row::text(FONT_FAMILY, "Font", prefs.font_family.clone())
            .placeholder("Default")
            .help("Any installed monospace family."),
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
            DETECT_INDENT,
            "Detect indentation from the file",
            prefs.detect_indent,
        )
        .help("The two rows below apply when a file has no clear style."),
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
        Row::header("Preview"),
        Row::toggle(
            LIVE_PREVIEW,
            "Live preview while typing",
            prefs.live_preview,
        )
        .help("mdview shows the text as you type it. Off: it shows the file when you save."),
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
    if let Some(c) = corners {
        // After Theme, in the View group.
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::Choice { id, .. } if *id == THEME))
            .map_or(rows.len(), |i| i + 1);
        rows.insert(
            at,
            Row::choice(
                WINDOW_CORNERS,
                Corners::SETTING_LABEL,
                &Corners::LABELS,
                c.index(),
            )
            .help(corners::setting_help()),
        );
    }
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
        // One modal at a time: the palette gives way.
        if self.palette_open() {
            self.close_palette();
        }
        self.ui.popup = None;
        self.ui.tip = None;
        self.ui.hover = None;
        self.ui.find_focused = false;
        self.ui.dialog = None;
        let look = self.docs.updates.about_label();
        let rows = rows(
            &self.docs.prefs,
            self.docs.theme,
            self.corners_offered().then_some(self.session.corners),
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
            (DETECT_INDENT, Value::Bool(on)) => p.detect_indent = on,
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
            (LIVE_PREVIEW, Value::Bool(on)) => p.live_preview = on,
            (THEME, Value::Choice(i)) => {
                self.set_theme(theme_of(i));
                return;
            }
            (WINDOW_CORNERS, Value::Choice(i)) => {
                self.set_corners(Corners::from_index(i));
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

    /// Whether the window can be rounded here, so the setting is offered: Windows (11 rounds it,
    /// 10 ignores the call), or a Linux window with a shadow route (Wayland; X11 has none unless
    /// forced and its window is opaque). A shot has no window and shows the row.
    pub(super) fn corners_offered(&self) -> bool {
        cfg!(windows) || self.shadow.is_some() || self.args.shot.is_some()
    }

    /// Takes a new corner setting: Windows is told its preference; with the client-drawn margin
    /// the whole window is drawn again. It is kept for the next start (KDE chooses its route
    /// from it then).
    pub(super) fn set_corners(&mut self, c: Corners) {
        if self.session.corners == c {
            return;
        }
        self.session.corners = c;
        #[cfg(windows)]
        if let Some(w) = &self.window {
            style(w, self.dark(), c);
        }
        if self.shadow.as_mut().is_some_and(|s| s.set_corners(c)) {
            self.full = true;
            self.expose = true;
            self.wants_frame = true;
        }
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
                r.view.widest = 0.0;
                r.view.rows_stamp = (u64::MAX, 0, 0, 0);
            }
        }
        self.full = true;
        self.redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdshared_text::Bitmap;
    use mdshared_ui::gfx::{Face, Gfx, Painter, Rect};

    fn help_of(row: &Row) -> &str {
        match row {
            Row::Toggle { help, .. }
            | Row::Choice { help, .. }
            | Row::Number { help, .. }
            | Row::Text { help, .. } => help,
            _ => "",
        }
    }

    fn label_of(row: &Row) -> &str {
        match row {
            Row::Toggle { label, .. }
            | Row::Choice { label, .. }
            | Row::Number { label, .. }
            | Row::Text { label, .. }
            | Row::Action { label, .. } => label,
            _ => "",
        }
    }

    /// Every help line fits the space the form gives it (what is left of the row after the
    /// control), at the default window, a narrow one and scale 1.5: none is cut with an ellipsis.
    #[test]
    fn every_help_line_fits_in_every_layout() {
        let prefs = Prefs::default();
        for (w, h, scale) in [
            (1280.0, 800.0, 1.0),
            (560.0, 640.0, 1.0),
            (1280.0 / 1.5, 800.0 / 1.5, 1.5),
        ] {
            let mut form = Form::new(
                "Settings",
                rows(
                    &prefs,
                    ThemeChoice::System,
                    Some(Corners::Round),
                    true,
                    "Check",
                ),
            );
            let mut bmp = Bitmap::new(100, 100);
            let mut gfx = Gfx::new();
            let mut p = Painter::new(&mut bmp, &mut gfx, scale);
            form.layout(&mut p, Rect::new(0.0, 0.0, w, h));
            // The Preview section's row is among the rows checked below, with its help line.
            let live = form
                .rows()
                .iter()
                .position(|r| matches!(r, Row::Toggle { id, .. } if *id == LIVE_PREVIEW))
                .expect("the live preview row");
            assert!(!help_of(&form.rows()[live]).is_empty());
            for i in 0..form.rows().len() {
                let help = help_of(&form.rows()[i]);
                let (row, ctrl) = (form.rect_of_row(i), form.control_rect(i));
                if ctrl.w > 0.0 {
                    let label = label_of(&form.rows()[i]);
                    let room = (ctrl.x - row.x - 12.0 - 16.0).max(40.0);
                    let need = p.measure(Face::Ui, 13.5, label);
                    assert!(
                        need <= room,
                        "label {label:?} needs {need} of {room} px at {w}x{h}"
                    );
                }
                if help.is_empty() {
                    continue;
                }
                // The form's own rule: the text runs from the row's inset to 16 px before the
                // control.
                let room = (ctrl.x - row.x - 12.0 - 16.0).max(40.0);
                let need = p.measure(Face::Ui, 11.5, help);
                assert!(
                    need <= room,
                    "help {help:?} needs {need} of {room} px at {w}x{h} scale {scale}"
                );
            }
        }
    }
}
