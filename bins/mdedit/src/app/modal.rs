//! The settings dialog as the window hosts it: the form is `mdshared_ui::settings::Form`, built
//! from the preferences each time it opens; every change comes back as an outcome, is applied to
//! every open document at once and is written to the settings file (the `[preferences]` section).
//! Keys, the pointer and the wheel go to the form while it is open, and the page does not see them.

use super::*;
use crate::ai::Ask;
use crate::commands::CommandId as C;
use crate::{keys, updater};
use mdedit_core::ai::{self as core_ai, Connection};
use mdedit_core::prefs::{
    FONT_PT_MAX, FONT_PT_MIN, INDENT_SIZE_MAX, MINIMAP_WIDTH_MAX, MINIMAP_WIDTH_MIN, Prefs,
    STALE_DAYS_MAX, STALE_DAYS_MIN, TAB_WIDTH_MAX,
};
use mdshared_ui::corners::{self, Corners};
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::settings::{Form, Outcome as Out, Row, Value};
use serde_json::json;

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
pub const MINIMAP_WIDTH: u32 = 22;
pub const COMPLETE_AUTO: u32 = 70;
pub const BACKDROP: u32 = 80;
pub const START_ON_PINNED: u32 = 23;
pub const OFFER_STALE: u32 = 24;
pub const STALE_DAYS: u32 = 25;
pub const AI_CONNECTION: u32 = 90;
pub const AI_PROGRAM: u32 = 91;
pub const AI_CHECK: u32 = 92;
pub const AI_PROVIDER: u32 = 93;
pub const AI_BASE_URL: u32 = 94;
pub const AI_KEY: u32 = 95;
pub const AI_KEY_SAVE: u32 = 96;
pub const AI_MODEL: u32 = 97;
pub const AI_MODEL_NAME: u32 = 98;
pub const AI_FETCH: u32 = 99;
pub const AI_EFFORT: u32 = 100;
pub const AI_REVIEW: u32 = 101;
pub const AI_PROMPTS: u32 = 102;
pub const AI_CUSTOM_NAME: u32 = 103;
pub const AI_CUSTOM_REMOVE: u32 = 104;
/// The last of the AI rows' ids.
pub const AI_LAST: u32 = AI_CUSTOM_REMOVE;

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
        Row::toggle(
            COMPLETE_AUTO,
            "Suggest words while typing",
            prefs.complete_auto,
        )
        .help(format!(
            "A list of words opens after three letters. {} opens it anyway.",
            crate::commands::hint(crate::commands::CommandId::CompleteWord)
        )),
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
        Row::number(
            MINIMAP_WIDTH,
            "Minimap width",
            prefs.minimap_width as i32,
            MINIMAP_WIDTH_MIN as i32,
            MINIMAP_WIDTH_MAX as i32,
        )
        .unit("px"),
        Row::toggle(WORD_WRAP, "Word wrap for new documents", prefs.word_wrap).help(format!(
            "{} changes the document you are in.",
            crate::commands::hint(crate::commands::CommandId::ToggleWrap)
        )),
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
        Row::toggle(
            START_ON_PINNED,
            "Start on the last pinned tab",
            prefs.start_on_pinned,
        )
        .help("The pinned tab you used last shows at start, not the tab you left on."),
        Row::toggle(
            OFFER_STALE,
            "Offer to close tabs left unopened",
            prefs.offer_stale,
        )
        .help("At start. Pinned and changed tabs are never offered."),
        Row::number(
            STALE_DAYS,
            "Offer them after",
            prefs.stale_days as i32,
            STALE_DAYS_MIN as i32,
            STALE_DAYS_MAX as i32,
        )
        .unit("days")
        .help("Counted in days you use mdedit, not calendar days."),
        Row::header("Updates"),
        Row::toggle(
            CHECK_AT_LAUNCH,
            "Check for updates at launch",
            check_updates,
        ),
        Row::action(CHECK_NOW, "Look for a newer version", look),
    ];
    // The backdrop is Windows 11's: the row is offered, under the theme, where it can do something.
    #[cfg(windows)]
    if let Some(at) = rows.iter().position(|r| r.id() == Some(THEME)) {
        use mdshared_app::backdrop::Backdrop;
        rows.insert(
            at + 1,
            Row::choice(
                BACKDROP,
                Backdrop::TITLE,
                &Backdrop::LABELS,
                prefs.backdrop.index(),
            )
            .help(Backdrop::HELP),
        );
    }
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
        self.ai.key_draft.clear();
        self.ai.key_error = None;
        let rows = self.settings_rows();
        self.ui.settings = Some(Form::new("Settings", rows));
        self.ai_settings_needs();
        self.modal_changed();
    }

    /// The form's rows from the preferences, with the AI group before Updates.
    pub(super) fn settings_rows(&mut self) -> Vec<Row> {
        let look = self.docs.updates.about_label();
        let mut rows = rows(
            &self.docs.prefs,
            self.docs.theme,
            self.corners_offered().then_some(self.session.corners),
            self.docs.check_updates,
            &look,
        );
        self.ai_load_prompts();
        let at = rows
            .iter()
            .position(|r| matches!(r, Row::Header(h) if h == "Updates"))
            .unwrap_or(rows.len());
        rows.splice(at..at, self.ai_rows());
        rows
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
            Out::Action(id) if (AI_CONNECTION..=AI_LAST).contains(&id) => {
                self.ai_settings_action(id);
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
        if (AI_CONNECTION..=AI_LAST).contains(&id) {
            self.ai_setting(id, value);
            return;
        }
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
            (MINIMAP_WIDTH, Value::Number(n)) => p.minimap_width = n.max(0) as usize,
            (WORD_WRAP, Value::Bool(on)) => p.word_wrap = on,
            (RESTORE_TABS, Value::Bool(on)) => p.restore_session = on,
            (START_ON_PINNED, Value::Bool(on)) => p.start_on_pinned = on,
            (OFFER_STALE, Value::Bool(on)) => p.offer_stale = on,
            (STALE_DAYS, Value::Number(n)) => p.stale_days = n.max(0) as u32,
            (LIVE_PREVIEW, Value::Bool(on)) => p.live_preview = on,
            (COMPLETE_AUTO, Value::Bool(on)) => p.complete_auto = on,
            (BACKDROP, Value::Choice(i)) => {
                p.backdrop = mdshared_app::backdrop::Backdrop::from_index(i)
            }
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

// ---- Settings › AI -------------------------------------------------------------------------------

/// The presets the Provider list offers: all but Custom, whose place the custom providers take.
fn ai_presets() -> Vec<&'static core_ai::Preset> {
    core_ai::PRESETS
        .iter()
        .filter(|x| !x.base_url.is_empty())
        .collect()
}

/// The Provider list (the presets, the custom providers, then the item that adds one) and where
/// the current provider is in it.
fn ai_providers(p: &Prefs) -> (Vec<String>, usize) {
    let presets = ai_presets();
    let mut names: Vec<String> = presets.iter().map(|x| x.name.to_string()).collect();
    names.extend(p.ai_customs.iter().enumerate().map(|(i, c)| c.label(i)));
    let at = match p.ai_custom_chosen() {
        Some(_) => presets.len() + p.ai_custom,
        None => presets
            .iter()
            .position(|x| x.key == p.ai_preset)
            .unwrap_or(0),
    };
    if p.ai_customs.len() < core_ai::CUSTOMS_MAX {
        names.push("Add a custom provider…".into());
    }
    (names, at)
}

/// What the Model row offers: (the model's id, its label), "Default" first and "Other…" last.
struct Models {
    list: Vec<(String, String)>,
    /// Where the current model is in `list`.
    value: usize,
    /// The current model is typed, not one of the list.
    other: bool,
}

/// How long typing in the base URL pauses before its key is looked up.
pub(super) const AI_URL_DELAY: std::time::Duration = std::time::Duration::from_millis(500);

impl App {
    fn ai_key_slot(&self) -> Option<String> {
        let p = &self.docs.prefs;
        core_ai::key_slot(p.ai_connection, &p.ai_preset, p.ai_url())
    }

    fn ai_models_key(&self) -> String {
        let p = &self.docs.prefs;
        crate::ai::models_key(
            p.ai_connection,
            &core_ai::base_url(&p.ai_preset, p.ai_url()),
        )
    }

    /// The fetched list for the current connection, when there is one.
    fn ai_fetched(&self) -> Option<&Result<Vec<crate::ai::ModelInfo>, String>> {
        match self.docs.prefs.ai_connection {
            Connection::ClaudeApi | Connection::OpenAi => self.ai.models.get(&self.ai_models_key()),
            _ => None,
        }
    }

    fn ai_model_options(&self) -> Models {
        let c = self.docs.prefs.ai_connection;
        let current = self.docs.prefs.ai_model(c).to_string();
        let mut list = vec![(String::new(), "Default".to_string())];
        for (id, name) in c.aliases() {
            list.push((id.to_string(), name.to_string()));
        }
        if let Some(Ok(models)) = self.ai_fetched() {
            for m in models {
                let label = if m.name.is_empty() || m.name == m.id {
                    m.id.clone()
                } else {
                    format!("{} ({})", m.name, m.id)
                };
                list.push((m.id.clone(), label));
            }
        }
        let known = list.iter().position(|(id, _)| *id == current);
        list.push(("\u{0}other".into(), "Other…".into()));
        let other = known.is_none() || self.ai.model_other;
        let value = if other {
            list.len() - 1
        } else {
            known.unwrap_or(0)
        };
        Models { list, value, other }
    }

    /// The effort levels the current model takes: its own list, or the connection's.
    fn ai_effort_options(&self) -> Vec<String> {
        let c = self.docs.prefs.ai_connection;
        let model = self.docs.prefs.ai_model(c);
        if let Some(Ok(models)) = self.ai_fetched()
            && let Some(m) = models.iter().find(|m| m.id == model)
            && !m.efforts.is_empty()
        {
            return m.efforts.clone();
        }
        c.efforts().iter().map(|e| e.to_string()).collect()
    }

    fn ai_rows(&self) -> Vec<Row> {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let labels: Vec<&str> = Connection::ALL.iter().map(|c| c.label()).collect();
        let at = Connection::ALL.iter().position(|x| *x == c).unwrap_or(0);
        let mut rows = vec![
            Row::header("AI"),
            Row::choice(AI_CONNECTION, "Connection", &labels, at).help(core_ai::destination(
                c,
                &p.ai_preset,
                p.ai_url(),
            )),
        ];
        if c.is_cli() {
            let (value, name) = match c {
                Connection::ClaudeCode => (&p.ai_claude_path, "claude"),
                _ => (&p.ai_codex_path, "codex"),
            };
            let help = match self.ai.detected.get(c.key()) {
                Some(Ok((path, version))) if version.is_empty() => format!("Found at {path}."),
                Some(Ok((path, version))) => format!("Found {version} at {path}."),
                Some(Err(why)) => why.clone(),
                None => "Looking for it…".into(),
            };
            rows.push(
                Row::text(AI_PROGRAM, "Program", value.clone())
                    .placeholder(format!("Found by itself: {name}"))
                    .help(help),
            );
            rows.push(Row::action(AI_CHECK, "Check the program", "Check"));
        }
        if c == Connection::OpenAi {
            let (names, at) = ai_providers(p);
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            rows.push(
                Row::choice(AI_PROVIDER, "Provider", &names, at)
                    .help("Add a custom provider for any OpenAI-compatible server."),
            );
            if let Some(x) = p.ai_custom_chosen() {
                rows.push(
                    Row::text(AI_CUSTOM_NAME, "Name", x.name.clone())
                        .placeholder(x.label(p.ai_custom))
                        .help("As the provider list and the rewrite panel show it."),
                );
                rows.push(
                    Row::text(AI_BASE_URL, "Base URL", x.base_url.clone())
                        .placeholder("http://localhost:1234/v1")
                        .help("The address before /chat/completions."),
                );
            }
        }
        // A custom provider's key is optional: a local server (LM Studio, Ollama) takes none, and
        // is then sent no key at all.
        let optional = p.ai_custom_chosen().is_some();
        if let Some(slot) = self.ai_key_slot() {
            let stored = self.ai.keys.get(&slot).copied();
            let placeholder = match stored {
                Some(true) => "Stored. Type a new key to replace it.",
                _ if optional => "Optional: paste an API key",
                _ => "Paste your API key",
            };
            let help = self.ai.key_error.clone().unwrap_or_else(|| match stored {
                None => "Looking in the system's credential store…".into(),
                _ if optional => "Optional. Leave it empty for a server that takes none, such as LM Studio or Ollama. A key is kept in the system's credential store.".into(),
                _ => "Kept in the system's credential store, never in the settings file.".into(),
            });
            rows.push(
                Row::text(AI_KEY, "API key", self.ai.key_draft.clone())
                    .secret()
                    .placeholder(placeholder)
                    .help(help),
            );
            let button = if self.ai.key_draft.trim().is_empty() && stored == Some(true) {
                "Remove"
            } else {
                "Save"
            };
            rows.push(Row::action(AI_KEY_SAVE, "Store the key", button));
        }
        if optional {
            rows.push(Row::action(
                AI_CUSTOM_REMOVE,
                "Remove this provider",
                "Remove",
            ));
        }
        if c != Connection::Off {
            let models = self.ai_model_options();
            let labels: Vec<&str> = models.list.iter().map(|(_, l)| l.as_str()).collect();
            let help = match (c, self.ai_fetched()) {
                (Connection::ClaudeApi | Connection::OpenAi, _) if self.ai.fetching.is_some() => {
                    "Fetching the models…".to_string()
                }
                (_, Some(Err(why))) => why.clone(),
                (_, Some(Ok(list))) => format!("{} models offered.", list.len()),
                (Connection::ClaudeCode, _) => "Claude Code's names for its latest models.".into(),
                (Connection::Codex, _) => "Default is the model Codex is set to use.".into(),
                _ => String::new(),
            };
            rows.push(Row::choice(AI_MODEL, "Model", &labels, models.value).help(help));
            if models.other {
                rows.push(
                    Row::text(AI_MODEL_NAME, "Model name", p.ai_model(c).to_string())
                        .placeholder("The model's id"),
                );
            }
            if matches!(c, Connection::ClaudeApi | Connection::OpenAi) {
                rows.push(Row::action(AI_FETCH, "The list of models", "Fetch again"));
            }
            let efforts = self.ai_effort_options();
            let mut labels = vec!["Default".to_string()];
            labels.extend(efforts.iter().map(|e| core_ai::effort_label(e)));
            let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
            let at = efforts
                .iter()
                .position(|e| e == p.ai_effort(c))
                .map_or(0, |i| i + 1);
            rows.push(
                Row::choice(AI_EFFORT, "Effort", &labels, at)
                    .help("Higher thinks longer before answering. Default is the model's own."),
            );
            rows.push(
                Row::toggle(AI_REVIEW, "Show changes before applying", p.ai_review)
                    .help("Off: the result replaces the text at once, and Undo brings it back."),
            );
            let n = self.docs.ai_prompts.len();
            let s = if n == 1 { "" } else { "s" };
            rows.push(Row::action(
                AI_PROMPTS,
                format!("Saved prompts ({n} prompt{s})"),
                "Edit…",
            ));
        }
        rows
    }

    /// Asks the engine for what Settings › AI shows and does not know yet: whether the program is
    /// there, whether a key is stored, the models.
    pub(super) fn ai_settings_needs(&mut self) {
        let c = self.docs.prefs.ai_connection;
        // A shot never starts the engine.
        if c == Connection::Off || self.ui.settings.is_none() || self.args.shot.is_some() {
            return;
        }
        let wake = self.docs.waker();
        let mut failed = None;
        if c.is_cli() && !self.ai.detected.contains_key(c.key()) {
            let program = match c {
                Connection::ClaudeCode => &self.docs.prefs.ai_claude_path,
                _ => &self.docs.prefs.ai_codex_path,
            };
            let req = json!({"op": "detect", "cli": c.key(), "program": program});
            if let Err(e) = self.ai.ask(&wake, Ask::Detect(c), req) {
                failed = Some(e);
            }
        }
        // While a URL is being typed, its key and models wait for the pause (`url_due`): a reply
        // that comes in meanwhile, for the URL before, does not look up the one half typed
        // (audit 1 round 3, #8). The timer calls this again.
        let typing = self.ai.url_due.is_some();
        let slot = self.ai_key_slot().filter(|_| !typing);
        if let Some(slot) = &slot
            && !self.ai.keys.contains_key(slot)
            && !self.ai.pending_key(slot)
            && let Err(e) = self.ai.ask(
                &wake,
                Ask::Key(slot.clone()),
                json!({"op": "key_has", "slot": slot}),
            )
        {
            failed = Some(e);
        }
        // The models are asked for once a key is stored; a custom provider's key is optional, so
        // its list is asked for once the store has answered either way.
        let optional = self.docs.prefs.ai_custom_chosen().is_some();
        let key_ok = slot.as_ref().is_none_or(|s| match self.ai.keys.get(s) {
            Some(&stored) => stored || optional,
            None => false,
        });
        let base_ok = c != Connection::OpenAi
            || !core_ai::base_url(&self.docs.prefs.ai_preset, self.docs.prefs.ai_url()).is_empty();
        let key = self.ai_models_key();
        if matches!(c, Connection::ClaudeApi | Connection::OpenAi)
            && !typing
            && key_ok
            && base_ok
            && !self.ai.models.contains_key(&key)
            && self.ai.fetching.as_ref() != Some(&key)
        {
            let req = json!({"op": "models", "conn": crate::ai::conn_json(c, &self.docs.prefs)});
            match self.ai.ask(&wake, Ask::Models(key.clone()), req) {
                Ok(_) => self.ai.fetching = Some(key),
                Err(e) => failed = Some(e),
            }
        }
        if let Some(why) = failed {
            self.ai.key_error = Some(why);
        }
        self.ai_settings_refresh();
    }

    /// Settings › AI is made again from what is known now.
    pub(super) fn ai_settings_refresh(&mut self) {
        if self.ui.settings.is_none() {
            return;
        }
        let rows = self.settings_rows();
        if let Some(f) = self.ui.settings.as_mut() {
            f.set_rows(rows);
        }
        self.modal_changed();
    }

    pub(super) fn ai_setting(&mut self, id: u32, value: Value) {
        let mut p = self.docs.prefs.clone();
        let c = p.ai_connection;
        let mut ask = false;
        match (id, value) {
            (AI_CONNECTION, Value::Choice(i)) => {
                p.ai_connection = Connection::ALL.get(i).copied().unwrap_or_default();
                self.ai.key_draft.clear();
                self.ai.key_error = None;
                self.ai.model_other = false;
                ask = true;
            }
            (AI_PROGRAM, Value::Text(t)) => {
                match c {
                    Connection::ClaudeCode => p.ai_claude_path = t,
                    _ => p.ai_codex_path = t,
                }
                self.ai.detected.remove(c.key());
            }
            (AI_PROVIDER, Value::Choice(i)) => {
                let presets = ai_presets();
                let customs = p.ai_customs.len();
                if let Some(x) = presets.get(i) {
                    p.ai_preset = x.key.into();
                } else if i - presets.len() < customs {
                    p.ai_preset = "custom".into();
                    p.ai_custom = i - presets.len();
                } else if customs < core_ai::CUSTOMS_MAX {
                    // Add a custom provider: a new one, chosen, its URL to be typed.
                    p.ai_customs.push(core_ai::CustomProvider {
                        name: format!("Custom provider {}", customs + 1),
                        ..core_ai::CustomProvider::default()
                    });
                    p.ai_preset = "custom".into();
                    p.ai_custom = customs;
                }
                self.ai.key_draft.clear();
                self.ai.key_error = None;
                self.ai.model_other = false;
                ask = true;
            }
            (AI_CUSTOM_NAME, Value::Text(t)) => {
                if let Some(x) = p.ai_custom_chosen_mut() {
                    x.name = t;
                }
            }
            (AI_BASE_URL, Value::Text(t)) => {
                if let Some(x) = p.ai_custom_chosen_mut() {
                    x.base_url = t;
                }
                // The new URL's key is looked up once typing pauses.
                self.ai.url_due = Some(std::time::Instant::now() + AI_URL_DELAY);
            }
            (AI_KEY, Value::Text(t)) => {
                self.ai.key_draft = t;
                self.ai.key_draft_slot = self.ai_key_slot();
                self.ai_settings_refresh();
                return;
            }
            (AI_MODEL, Value::Choice(i)) => {
                let models = self.ai_model_options();
                if i + 1 >= models.list.len() {
                    self.ai.model_other = true;
                } else {
                    self.ai.model_other = false;
                    p.set_ai_model(c, &models.list[i].0);
                }
            }
            (AI_MODEL_NAME, Value::Text(t)) => p.set_ai_model(c, t.trim()),
            (AI_EFFORT, Value::Choice(i)) => {
                let efforts = self.ai_effort_options();
                let e = i.checked_sub(1).and_then(|k| efforts.get(k));
                p.set_ai_effort(c, e.map_or("", String::as_str));
            }
            (AI_REVIEW, Value::Bool(on)) => p.ai_review = on,
            _ => return,
        }
        p.clamp();
        self.set_prefs(p);
        // A key typed for one endpoint is never kept for another.
        if !self.ai.key_draft.is_empty() && self.ai.key_draft_slot != self.ai_key_slot() {
            self.ai.key_draft.clear();
            self.ai.key_draft_slot = None;
            self.ai.key_error = None;
        }
        if ask {
            self.ai_settings_needs();
        } else {
            self.ai_settings_refresh();
        }
    }

    pub(super) fn ai_settings_action(&mut self, id: u32) {
        let c = self.docs.prefs.ai_connection;
        match id {
            AI_CHECK => {
                self.ai.detected.remove(c.key());
                self.ai_settings_needs();
            }
            AI_FETCH => {
                let key = self.ai_models_key();
                self.ai.models.remove(&key);
                self.ai.key_error = None;
                self.ai_settings_needs();
            }
            AI_KEY_SAVE => {
                let Some(slot) = self.ai_key_slot() else {
                    return;
                };
                let key = self.ai.key_draft.trim().to_string();
                if !key.is_empty() && self.ai.key_draft_slot.as_ref() != Some(&slot) {
                    self.ai.key_draft.clear();
                    self.ai_settings_refresh();
                    return;
                }
                if !key.is_empty() && key.chars().count() < 4 {
                    self.ai.key_error = Some("This is too short to be an API key.".into());
                    self.ai_settings_refresh();
                    return;
                }
                let req = if key.is_empty() {
                    if self.ai.keys.get(&slot) != Some(&true) {
                        return;
                    }
                    json!({"op": "key_clear", "slot": slot})
                } else {
                    json!({"op": "key_set", "slot": slot, "key": key})
                };
                let wake = self.docs.waker();
                match self.ai.ask(&wake, Ask::Key(slot.clone()), req) {
                    Ok(_) => {
                        self.ai.key_draft.clear();
                        self.ai.key_error = None;
                        self.ai.keys.remove(&slot);
                        // The list is fetched again with the new key.
                        let k = self.ai_models_key();
                        self.ai.models.remove(&k);
                    }
                    Err(e) => self.ai.key_error = Some(e),
                }
                self.ai_settings_refresh();
            }
            AI_PROMPTS => {
                self.close_settings();
                self.ai_edit_prompts();
            }
            AI_CUSTOM_REMOVE => {
                // The provider goes; its key stays in the store under its URL (another provider
                // may use the same server), as it did when a custom URL was changed.
                let mut p = self.docs.prefs.clone();
                if p.ai_custom_chosen().is_none() {
                    return;
                }
                p.ai_customs.remove(p.ai_custom);
                if p.ai_customs.is_empty() {
                    p.ai_preset = "openai".into();
                }
                p.ai_custom = p.ai_custom.min(p.ai_customs.len().saturating_sub(1));
                self.ai.key_draft.clear();
                self.ai.key_draft_slot = None;
                self.ai.key_error = None;
                self.ai.model_other = false;
                p.clamp();
                self.set_prefs(p);
                self.ai_settings_needs();
            }
            _ => {}
        }
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
