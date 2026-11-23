//! The settings dialog as the window hosts it: the form is `mdshared_ui::settings::pages::Form`,
//! a list of pages at the left and one page at a time (docs/settings-redesign.md on
//! `mock/settings`). It is built from the preferences when it opens and again after every change,
//! so a switch turned off disables the rows that depend on it; every change comes back as an
//! outcome, is applied to every open document at once and is written to the settings file (the
//! `[preferences]` section). Keys, the pointer and the wheel go to the form while it is open, and
//! the page does not see them.

use super::*;
use crate::ai::Ask;
use crate::commands::CommandId as C;
use crate::{keys, updater};
use mdedit_core::ai::{self as core_ai, Connection};
use mdedit_core::prefs::{
    BRIEF_SECS_MAX, BRIEF_SECS_MIN, FONT_PT_MAX, FONT_PT_MIN, INDENT_SIZE_MAX, MINIMAP_WIDTH_MAX,
    MINIMAP_WIDTH_MIN, Prefs, PreviewMode, STALE_DAYS_MAX, STALE_DAYS_MIN, TAB_WIDTH_MAX,
};
use mdshared_ui::corners::{self, Corners};
use mdshared_ui::glyph;
use mdshared_ui::keys::{KeyName, Mods};
use mdshared_ui::settings::pages::{Form, Item, Page};
use mdshared_ui::settings::{Outcome as Out, Row, Value};
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
/// What the Preview button opens (issue #7), and Ctrl+V's rich text as Markdown (issue #8).
pub const PREVIEW_MODE: u32 = 71;
pub const PASTE_RICH: u32 = 72;
pub const BACKDROP: u32 = 80;
pub const START_ON_PINNED: u32 = 23;
pub const OFFER_STALE: u32 = 24;
pub const STALE_DAYS: u32 = 25;
pub const SKIP_BRIEF: u32 = 26;
pub const BRIEF_SECS: u32 = 27;
/// The seconds' label under "Don't reopen tabs barely looked at". Shorter than the shared
/// `brief::SECONDS` ("Looked at for less than"), which needs 145 px of the row's 139 with Linux's
/// fonts at the smallest window.
const BRIEF_SECS_LABEL: &str = "Shown for less than";
pub const BRIEF_LAST: u32 = 28;
pub const LIBRARY_DIR: u32 = 29;
pub const LIBRARY_CHOOSE: u32 = 30;
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
/// "Requests at once", added after the first ids so no existing row's id moved.
pub const AI_PARALLEL: u32 = 105;
/// The last of the AI rows' ids.
pub const AI_LAST: u32 = AI_PARALLEL;

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

/// Whether "Make mdedit the default" is offered: off Windows, from an AppImage, until it is the
/// default (as the main menu has it).
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

// The pages, in the list's order. The two AI pages are made by the window, which knows what the
// engine said (`App::ai_pages`); they go between Tabs and Updates.
pub const APPEARANCE: &str = "Appearance";
pub const EDITOR: &str = "Editor";
pub const INDENTATION: &str = "Indentation";
pub const TYPING: &str = "Typing";
pub const NOTES_LIBRARY: &str = "Notes library";
pub const TABS: &str = "Tabs";
pub const AI_SERVICE: &str = "AI service";
pub const AI_REWRITE: &str = "AI rewrite";
pub const UPDATES: &str = "Updates";
const PAGES: [&str; 9] = [
    APPEARANCE,
    EDITOR,
    INDENTATION,
    TYPING,
    NOTES_LIBRARY,
    TABS,
    AI_SERVICE,
    AI_REWRITE,
    UPDATES,
];

/// The reason a row under a switch gives while the switch is off.
pub const NEEDS_SWITCH: &str = "Needs the switch above.";
/// The reason the AI rewrite page gives while the connection is off.
pub const NEEDS_CONNECTION: &str = "Choose a connection on AI service first.";

/// The page `--settings-section` names: a page's name, as it is or with hyphens for spaces
/// (`ai-service`), or the group the setting was in before the pages (`Preview`, `View`).
pub fn page_for_section(name: &str) -> Option<&'static str> {
    let key = name.trim().to_ascii_lowercase().replace('-', " ");
    if let Some(p) = PAGES.iter().find(|p| p.to_ascii_lowercase() == key) {
        return Some(p);
    }
    Some(match key.as_str() {
        "view" => APPEARANCE,
        "preview" => EDITOR,
        "editing" | "saving" => TYPING,
        "session" => TABS,
        "ai" => AI_SERVICE,
        "system" => UPDATES,
        _ => return None,
    })
}

/// The pages but the two AI ones, from the preferences and what else the window knows. `corners`
/// is the window's corner setting, or `None` where the system gives the window no way to round
/// them (the row is left out); `offer_default` adds "Make mdedit the default" to Updates.
pub fn pages(
    prefs: &Prefs,
    theme: ThemeChoice,
    corners: Option<Corners>,
    check_updates: bool,
    look: &str,
    offer_default: bool,
) -> Vec<Page> {
    let mut appearance = vec![Item::new(Row::choice(
        THEME,
        "Theme",
        &THEMES,
        theme_index(theme),
    ))];
    // The backdrop is Windows 11's: the row is offered where it can do something.
    #[cfg(windows)]
    {
        use mdshared_app::backdrop::Backdrop;
        appearance.push(Item::new(
            Row::choice(
                BACKDROP,
                Backdrop::TITLE,
                &Backdrop::LABELS,
                prefs.backdrop.index(),
            )
            .help(Backdrop::HELP),
        ));
    }
    if let Some(c) = corners {
        appearance.push(Item::new(
            Row::choice(
                WINDOW_CORNERS,
                Corners::SETTING_LABEL,
                &Corners::LABELS,
                c.index(),
            )
            .help(corners::setting_help()),
        ));
    }
    appearance.extend([
        Item::gap(),
        Item::new(
            Row::text(FONT_FAMILY, "Font", prefs.font_family.clone())
                .placeholder("Default")
                .help("Any installed monospace family."),
        ),
        Item::new(
            Row::number(
                FONT_SIZE,
                "Font size",
                prefs.font_pt.round() as i32,
                FONT_PT_MIN as i32,
                FONT_PT_MAX as i32,
            )
            .unit("pt")
            .help("At 100 % zoom. Zoom (Ctrl and the wheel) scales it."),
        ),
    ]);

    let editor = vec![
        Item::new(Row::toggle(
            LINE_NUMBERS,
            "Line numbers",
            prefs.line_numbers,
        )),
        Item::new(Row::toggle(
            HIGHLIGHT_LINE,
            "Highlight the current line",
            prefs.highlight_line,
        )),
        Item::new(
            Row::toggle(WORD_WRAP, "Word wrap for new documents", prefs.word_wrap).help(format!(
                "{} changes the document you are in.",
                crate::commands::hint(crate::commands::CommandId::ToggleWrap)
            )),
        ),
        Item::new(
            Row::toggle(MINIMAP, "Minimap for new windows", prefs.minimap)
                .help("View > Minimap changes the window you are in."),
        ),
        // The width a minimap gets, wherever one is shown: it stays enabled with the switch off.
        Item::new(
            Row::number(
                MINIMAP_WIDTH,
                "Minimap width",
                prefs.minimap_width as i32,
                MINIMAP_WIDTH_MIN as i32,
                MINIMAP_WIDTH_MAX as i32,
            )
            .unit("px"),
        )
        .child(),
        Item::gap(),
        Item::new(
            Row::toggle(
                LIVE_PREVIEW,
                "Live preview while typing",
                prefs.live_preview,
            )
            .help("mdview shows the text as you type it. Off: it shows the file when you save."),
        ),
        // Short, to fit the row at 520 px: the pane in this window, mdview, or mdview beside
        // this window. The button's arrow says them in full.
        Item::new(Row::choice(
            PREVIEW_MODE,
            "Preview button",
            &["Here", "mdview", "Beside"],
            prefs.preview_mode.index(),
        )),
    ];

    // The two rows under detection are what a file with no clear style gets: never disabled.
    let indentation = vec![
        Item::new(
            Row::number(
                TAB_WIDTH,
                "Tab width",
                prefs.tab_width as i32,
                1,
                TAB_WIDTH_MAX as i32,
            )
            .unit("columns"),
        ),
        Item::new(
            Row::toggle(
                DETECT_INDENT,
                "Detect indentation from the file",
                prefs.detect_indent,
            )
            .help("The two rows below apply when a file has no clear style."),
        ),
        Item::new(
            Row::toggle(
                INDENT_SPACES,
                "Indent with spaces",
                prefs.indent_with_spaces,
            )
            .help("Tab and the automatic indent add spaces, not a tab character."),
        )
        .child(),
        Item::new(
            Row::number(
                INDENT_SIZE,
                "Indent size",
                prefs.indent_size as i32,
                1,
                INDENT_SIZE_MAX as i32,
            )
            .unit("spaces"),
        )
        .child(),
    ];

    let typing = vec![
        Item::new(
            Row::toggle(AUTO_PAIR, "Close brackets and quotes", prefs.auto_pair)
                .help("Typing ( or \" adds the other half."),
        ),
        Item::new(
            Row::toggle(AUTO_INDENT, "Automatic indent", prefs.auto_indent)
                .help("Enter keeps the indent and opens a block after a bracket."),
        ),
        Item::new(
            Row::toggle(
                COMPLETE_AUTO,
                "Suggest words while typing",
                prefs.complete_auto,
            )
            .help(format!(
                "A list of words opens after three letters. {} opens it anyway.",
                crate::commands::hint(crate::commands::CommandId::CompleteWord)
            )),
        ),
        Item::new(
            Row::toggle(PASTE_RICH, "Paste rich text as Markdown", prefs.paste_rich).help(format!(
                "In Markdown files. {} pastes plain text.",
                crate::commands::hint(crate::commands::CommandId::PastePlain)
            )),
        ),
        Item::gap(),
        Item::new(Row::toggle(
            TRIM_ON_SAVE,
            "Trim trailing spaces on save",
            prefs.trim_on_save,
        )),
        Item::new(Row::toggle(
            FINAL_NEWLINE,
            "End the file with a line break on save",
            prefs.final_newline_on_save,
        )),
    ];

    let library = vec![
        Item::new(
            Row::text(LIBRARY_DIR, "Library folder", prefs.library_dir.clone())
                .placeholder("Beside the settings file")
                .help("Such as a OneDrive folder."),
        )
        .stacked()
        .button(LIBRARY_CHOOSE, "Choose\u{2026}", true),
    ];

    // A row under a switch is disabled, with the reason, while the switch is off. The help lines
    // are the design's shorter wording, one line each at the smallest window.
    let under = |it: Item, on: bool| if on { it } else { it.disabled(NEEDS_SWITCH) };
    // The days row is the one row under its switch, so it is dimmed without saying why: with
    // both rules off, a reason here too is the 11 px the smallest window does not have.
    let days_under = |it: Item, on: bool| if on { it } else { it.disabled("") };
    let tabs = vec![
        Item::new(Row::toggle(
            RESTORE_TABS,
            "Restore tabs at start",
            prefs.restore_session,
        )),
        Item::new(
            Row::toggle(
                START_ON_PINNED,
                "Start on the last pinned tab",
                prefs.start_on_pinned,
            )
            .help("Not the tab you left on."),
        ),
        Item::new(
            Row::toggle(
                OFFER_STALE,
                "Offer to close tabs left unopened",
                prefs.offer_stale,
            )
            .help("Pinned and changed tabs are never offered."),
        ),
        days_under(
            Item::new(
                Row::number(
                    STALE_DAYS,
                    "Offer them after",
                    prefs.stale_days as i32,
                    STALE_DAYS_MIN as i32,
                    STALE_DAYS_MAX as i32,
                )
                .unit("days used"),
            )
            .child(),
            prefs.offer_stale,
        ),
        Item::new(
            Row::toggle(SKIP_BRIEF, mdshared_app::brief::SWITCH, prefs.skip_brief)
                .help("Pinned tabs and unsaved text always come back."),
        ),
        under(
            Item::new(
                Row::number(
                    BRIEF_SECS,
                    BRIEF_SECS_LABEL,
                    prefs.brief_secs as i32,
                    BRIEF_SECS_MIN as i32,
                    BRIEF_SECS_MAX as i32,
                )
                .unit("sec")
                .help("Counted only while in front."),
            )
            .child(),
            prefs.skip_brief,
        ),
        under(
            Item::new(Row::toggle(
                BRIEF_LAST,
                "Leave out the last tab too",
                prefs.brief_last,
            ))
            .child(),
            prefs.skip_brief,
        ),
    ];

    let mut updates = vec![
        Item::new(Row::toggle(
            CHECK_AT_LAUNCH,
            "Check for updates at launch",
            check_updates,
        )),
        Item::new(Row::action(CHECK_NOW, "Look for a newer version", look)),
    ];
    if offer_default {
        updates.push(Item::new(
            Row::action(DEFAULT_APP, "Make mdedit the default", "Make default")
                .help("For text files."),
        ));
    }

    vec![
        Page::new(APPEARANCE, appearance),
        Page::new(EDITOR, editor),
        Page::new(INDENTATION, indentation),
        Page::new(TYPING, typing),
        Page::new(NOTES_LIBRARY, library),
        Page::new(TABS, tabs),
        Page::new(UPDATES, updates),
    ]
}

/// Every setting and action row of `pages`, in order, gaps left out: what a test reads.
#[cfg(test)]
pub fn rows_of(pages: &[Page]) -> Vec<Row> {
    pages
        .iter()
        .flat_map(|p| p.items.iter())
        .map(|it| it.row().clone())
        .filter(|r| r.id().is_some())
        .collect()
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
        let pages = self.settings_pages();
        let mut form = Form::new("Settings", pages);
        // The page last shown this run; the keyboard starts in the list.
        form.show_page(self.ui.settings_page);
        self.ui.settings = Some(form);
        self.ai_settings_needs();
        self.modal_changed();
    }

    /// Opens Settings on the page called `name` (`AI service`), or on the page it was last on
    /// when there is none.
    /// `--settings-section`: shows the page it names in the open dialog. False when no page has
    /// that name or the dialog is not open.
    pub(super) fn show_settings_section(&mut self, name: &str) -> bool {
        let Some(page) = page_for_section(name) else {
            return false;
        };
        let Some(f) = self.ui.settings.as_mut() else {
            return false;
        };
        let Some(c) = f.page_names().iter().position(|n| *n == page) else {
            return false;
        };
        f.show_page(c);
        self.modal_changed();
        true
    }

    pub(super) fn open_settings_at(&mut self, name: &str) {
        self.open_settings();
        if let Some(f) = self.ui.settings.as_mut()
            && let Some(c) = f.page_names().iter().position(|n| *n == name)
        {
            f.show_page(c);
        }
    }

    /// The form's pages from the preferences, with the two AI pages before Updates.
    pub(super) fn settings_pages(&mut self) -> Vec<Page> {
        self.settings_pages_offering(can_offer_default_app())
    }

    /// The pages, with "Make mdedit the default" or without it (a test asks for the most rows).
    pub(super) fn settings_pages_offering(&mut self, offer_default: bool) -> Vec<Page> {
        // The button is 96 px wide: the About box's "Check for updates" and "Update to 1.4.0"
        // are said in fewer words here, beside a label that says the rest.
        let look = match self.docs.updates.stage() {
            updater::Stage::Available(_) => "Update".to_string(),
            _ => match self.docs.updates.about_label() {
                l if l == mdshared_app::updater::CHECK => "Check now".to_string(),
                l => l,
            },
        };
        let mut pages = pages(
            &self.docs.prefs,
            self.docs.theme,
            self.corners_offered().then_some(self.session.corners),
            self.docs.check_updates,
            &look,
            offer_default,
        );
        self.ai_load_prompts();
        let at = pages
            .iter()
            .position(|p| p.name == UPDATES)
            .unwrap_or(pages.len());
        pages.splice(at..at, self.ai_pages());
        pages
    }

    /// Every row the form has now, in order: what a test reads.
    #[cfg(test)]
    pub(super) fn settings_rows(&mut self) -> Vec<Row> {
        rows_of(&self.settings_pages())
    }

    pub(super) fn close_settings(&mut self) {
        if let Some(f) = &self.ui.settings {
            self.ui.settings_page = f.page();
        }
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
                // A switch turned off or on disables or enables the rows under it.
                self.ai_settings_refresh();
            }
            // The folder picker opens over the form, which stays to show the answer.
            Out::Action(LIBRARY_CHOOSE) => {
                self.library_choose_folder();
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
            (SKIP_BRIEF, Value::Bool(on)) => p.skip_brief = on,
            (BRIEF_SECS, Value::Number(n)) => p.brief_secs = n.max(0) as u32,
            (BRIEF_LAST, Value::Bool(on)) => p.brief_last = on,
            (LIVE_PREVIEW, Value::Bool(on)) => p.live_preview = on,
            (COMPLETE_AUTO, Value::Bool(on)) => p.complete_auto = on,
            (PREVIEW_MODE, Value::Choice(i)) => p.preview_mode = PreviewMode::from_index(i),
            (PASTE_RICH, Value::Bool(on)) => p.paste_rich = on,
            (LIBRARY_DIR, Value::Text(t)) => {
                self.set_library_dir(&t);
                return;
            }
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

/// A path for a help line, which wraps only between words: one longer than a line at the
/// smallest window keeps its last folders and file, after an ellipsis.
fn short_path(path: &str) -> String {
    const MAX: usize = 40;
    if path.chars().count() <= MAX {
        return path.to_string();
    }
    let mut tail = String::new();
    for part in path.split_inclusive(['/', '\\']).rev() {
        if tail.chars().count() + part.chars().count() > MAX - 1 {
            break;
        }
        tail.insert_str(0, part);
    }
    if tail.is_empty() {
        let n = path.chars().count();
        tail = path.chars().skip(n - (MAX - 1)).collect();
    }
    format!("\u{2026}{tail}")
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

    /// The two AI pages: AI service (who the text is sent to and how you sign in) and AI rewrite
    /// (what the rewrite does once connected). Their help is written at run time (where the text
    /// goes, what the server said), so it is always under its row: a new message does not re-lay
    /// the page.
    fn ai_pages(&self) -> Vec<Page> {
        let p = &self.docs.prefs;
        let c = p.ai_connection;
        let labels: Vec<&str> = Connection::ALL.iter().map(|c| c.label()).collect();
        let at = Connection::ALL.iter().position(|x| *x == c).unwrap_or(0);
        let mut service = vec![Item::new(
            Row::choice(AI_CONNECTION, "Connection", &labels, at).help(core_ai::destination(
                c,
                &p.ai_preset,
                p.ai_url(),
            )),
        )];
        if c.is_cli() {
            let (value, name) = match c {
                Connection::ClaudeCode => (&p.ai_claude_path, "claude"),
                _ => (&p.ai_codex_path, "codex"),
            };
            let help = match self.ai.detected.get(c.key()) {
                Some(Ok((path, version))) if version.is_empty() => {
                    format!("Found at {}.", short_path(path))
                }
                Some(Ok((path, version))) => format!("Found {version} at {}.", short_path(path)),
                Some(Err(why)) => why.clone(),
                None => "Looking for it…".into(),
            };
            service.push(
                Item::new(
                    Row::text(AI_PROGRAM, "Program", value.clone())
                        .placeholder(format!("Found by itself: {name}"))
                        .help(help),
                )
                .stacked()
                .button(AI_CHECK, "Check", true),
            );
        }
        // A custom provider's key is optional: a local server (LM Studio, Ollama) takes none, and
        // is then sent no key at all.
        let optional = p.ai_custom_chosen().is_some();
        if c == Connection::OpenAi {
            let (names, at) = ai_providers(p);
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            // The list's last item adds a custom provider; the button beside it removes the one
            // chosen.
            let mut provider = Item::new(Row::choice(AI_PROVIDER, "Provider", &names, at));
            if optional {
                provider = provider.icon_button(
                    AI_CUSTOM_REMOVE,
                    glyph::DELETE,
                    "Remove this provider",
                    true,
                );
            }
            service.push(provider);
            if let Some(x) = p.ai_custom_chosen() {
                service.push(
                    Item::new(
                        Row::text(AI_CUSTOM_NAME, "Name", x.name.clone())
                            .placeholder(x.label(p.ai_custom)),
                    )
                    .child(),
                );
                service.push(
                    Item::new(
                        Row::text(AI_BASE_URL, "Base URL", x.base_url.clone())
                            .placeholder("http://localhost:1234/v1")
                            .help("The address before /chat/completions."),
                    )
                    .stacked(),
                );
            }
        }
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
            let typed = !self.ai.key_draft.trim().is_empty();
            // Save stores what is typed; with nothing typed, Remove takes a stored key out of the
            // store. With neither, there is nothing to do.
            let (button, live) = match (typed, stored) {
                (false, Some(true)) => ("Remove", true),
                (typed, _) => ("Save", typed),
            };
            service.push(
                Item::new(
                    Row::text(AI_KEY, "API key", self.ai.key_draft.clone())
                        .secret()
                        .placeholder(placeholder)
                        .help(help),
                )
                .stacked()
                .button(AI_KEY_SAVE, button, live),
            );
        }

        // The rewrite's rows stay while the connection is off, disabled with the reason, so the
        // page keeps its shape.
        let off = c == Connection::Off;
        let why = |it: Item| {
            if off {
                it.disabled(NEEDS_CONNECTION)
            } else {
                it
            }
        };
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
        let mut model =
            Item::new(Row::choice(AI_MODEL, "Model", &labels, models.value).help(help)).dropdown();
        if matches!(c, Connection::ClaudeApi | Connection::OpenAi) {
            model = model.icon_button(
                AI_FETCH,
                glyph::RELOAD,
                "Fetch the list of models again",
                true,
            );
        }
        let mut rewrite = vec![why(model)];
        if models.other && !off {
            rewrite.push(
                Item::new(
                    Row::text(AI_MODEL_NAME, "Model name", p.ai_model(c).to_string())
                        .placeholder("The model's id"),
                )
                .child(),
            );
        }
        let efforts = self.ai_effort_options();
        let mut labels = vec!["Default".to_string()];
        labels.extend(efforts.iter().map(|e| core_ai::effort_label(e)));
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        let at = efforts
            .iter()
            .position(|e| e == p.ai_effort(c))
            .map_or(0, |i| i + 1);
        rewrite.push(why(Item::new(
            Row::choice(AI_EFFORT, "Effort", &labels, at)
                .help("Higher thinks longer before answering. Default is the model's own."),
        )
        .dropdown()));
        rewrite.push(why(Item::new(
            Row::number(
                AI_PARALLEL,
                "Requests at once",
                p.ai_parallel(c) as i32,
                mdedit_core::prefs::AI_PARALLEL_MIN as i32,
                mdedit_core::prefs::AI_PARALLEL_MAX as i32,
            )
            .help("How many requests the AI may be asked at once. A local model starts with one."),
        )));
        rewrite.push(why(Item::new(
            Row::toggle(AI_REVIEW, "Show changes before applying", p.ai_review)
                .help("Off: the result replaces the text at once, and Undo brings it back."),
        )));
        let n = self.docs.ai_prompts.len();
        let s = if n == 1 { "" } else { "s" };
        rewrite.push(why(Item::new(Row::action(
            AI_PROMPTS,
            format!("Saved prompts ({n} prompt{s})"),
            "Edit…",
        ))));
        vec![
            Page::new(AI_SERVICE, service).help_under(),
            Page::new(AI_REWRITE, rewrite).help_under(),
        ]
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

    /// The pages are made again from what is known now (the AI pages from what the engine said).
    pub(super) fn ai_settings_refresh(&mut self) {
        if self.ui.settings.is_none() {
            return;
        }
        let pages = self.settings_pages();
        if let Some(f) = self.ui.settings.as_mut() {
            f.set_pages(pages);
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
                        name: core_ai::new_custom_name(&p.ai_customs),
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
            (AI_PARALLEL, Value::Number(n)) => p.set_ai_parallel(c, n.max(0) as usize),
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
                // The provider goes. Its key goes from the store too, unless another provider
                // uses the same server: a provider added later on that URL must not send it
                // unasked.
                let mut p = self.docs.prefs.clone();
                if p.ai_custom_chosen().is_none() {
                    return;
                }
                let gone = p.ai_customs.remove(p.ai_custom);
                let slot = core_ai::key_slot(Connection::OpenAi, "custom", &gone.base_url);
                let shared = p
                    .ai_customs
                    .iter()
                    .any(|c| core_ai::key_slot(Connection::OpenAi, "custom", &c.base_url) == slot);
                if let Some(slot) = slot.filter(|_| !shared) {
                    let wake = self.docs.waker();
                    let req = json!({"op": "key_clear", "slot": slot});
                    self.ai.keys.remove(&slot);
                    if let Err(e) = self.ai.ask(&wake, Ask::Key(slot), req) {
                        self.ai.key_error = Some(e);
                    }
                }
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
    use super::super::files_tests::app;
    use super::*;
    use mdshared_text::Bitmap;
    use mdshared_ui::gfx::{Gfx, Painter, Rect};
    use std::collections::HashSet;

    /// The states that give each page its most rows and its longest help, made up as the engine
    /// would have answered: the tab rules on and off, every AI connection (a custom provider with
    /// no key stored, a program found at a long path, a model list that failed with a long
    /// message and a typed model name).
    fn tallest_states() -> Vec<(&'static str, Vec<Page>)> {
        let mut out = Vec::new();
        let mut state = |name: &'static str, set: &dyn Fn(&mut App)| {
            let mut a = app();
            set(&mut a);
            out.push((name, a.settings_pages_offering(true)));
        };
        state("the tab rules off, an update offered", &|a| {
            a.docs.prefs.offer_stale = false;
            a.docs.prefs.skip_brief = false;
            a.docs.updates.preview("available");
        });
        state("the tab rules on", &|a| {
            a.docs.prefs.offer_stale = true;
            a.docs.prefs.skip_brief = true;
        });
        state("Claude Code found at a long path", &|a| {
            a.docs.prefs.ai_connection = Connection::ClaudeCode;
            a.ai.detected.insert(
                Connection::ClaudeCode.key(),
                Ok((
                    r"C:\Users\someone\AppData\Local\Programs\claude-code\bin\claude.exe".into(),
                    "2.1.328".into(),
                )),
            );
        });
        state("Codex not found", &|a| {
            a.docs.prefs.ai_connection = Connection::Codex;
            a.ai.detected.insert(
                Connection::Codex.key(),
                Err("Codex was not found. Install it, or type where it is.".into()),
            );
        });
        state("Claude API, no key stored", &|a| {
            a.docs.prefs.ai_connection = Connection::ClaudeApi;
            a.ai.keys.insert("claude-api".into(), false);
        });
        state("a custom provider, a failed list, a typed model", &|a| {
            a.docs.prefs.ai_connection = Connection::OpenAi;
            a.docs.prefs.ai_preset = "custom".into();
            a.docs.prefs.ai_customs = vec![core_ai::CustomProvider {
                name: "LM Studio (local)".into(),
                base_url: "http://localhost:1234/v1".into(),
                ..Default::default()
            }];
            a.docs
                .prefs
                .set_ai_model(Connection::OpenAi, "gpt-4.1-mini-2025-04-14");
            let slot = a.ai_key_slot().expect("a custom provider's key slot");
            a.ai.keys.insert(slot, false);
            let key = a.ai_models_key();
            a.ai.models.insert(
                key,
                Err(
                    "Could not list the models: connection refused (os error 10061). Is the \
                     server running?"
                        .into(),
                ),
            );
        });
        state("a key the store refused", &|a| {
            a.docs.prefs.ai_connection = Connection::OpenAi;
            a.docs.prefs.ai_preset = "openai".into();
            a.ai.key_error =
                Some("The system's credential store refused the key: the store is locked.".into());
        });
        out
    }

    /// The design's promise, tested: every page fits its area at the default window and at the
    /// smallest, at scale 1, 1.5 and 2, in its tallest state; nothing on it is cut, and the list
    /// holds every page.
    #[test]
    fn every_page_fits_at_the_default_and_the_minimum_at_every_scale() {
        let states = tallest_states();
        // Every row the dialog has is in one of the states above.
        let mut seen = HashSet::new();
        for (_, pages) in &states {
            seen.extend(rows_of(pages).iter().filter_map(Row::id));
        }
        for id in (FONT_FAMILY..=LIBRARY_CHOOSE).chain([
            COMPLETE_AUTO,
            PREVIEW_MODE,
            PASTE_RICH,
            BACKDROP,
        ]) {
            // The folder's button is not a row; the window rows are offered on Windows.
            let windows_only = [BACKDROP, WINDOW_CORNERS].contains(&id);
            if id == LIBRARY_CHOOSE || (windows_only && cfg!(not(windows))) {
                continue;
            }
            assert!(seen.contains(&id), "row {id} is in no state");
        }
        for id in AI_CONNECTION..=AI_LAST {
            // The four action rows that are buttons now are not rows.
            if [AI_CHECK, AI_KEY_SAVE, AI_FETCH, AI_CUSTOM_REMOVE].contains(&id) {
                continue;
            }
            assert!(seen.contains(&id), "AI row {id} is in no state");
        }
        for (name, pages) in states {
            let names: Vec<&str> = pages.iter().map(|p| p.name.as_str()).collect();
            assert_eq!(names, PAGES, "{name}");
            let form = Form::new("Settings", pages);
            for (w, h) in [(1280.0, 800.0), (520.0, 360.0)] {
                for scale in [1.0f32, 1.5, 2.0] {
                    let mut bmp = Bitmap::new((w * scale) as u32, (h * scale) as u32);
                    let mut gfx = Gfx::new();
                    let mut p = Painter::new(&mut bmp, &mut gfx, scale);
                    let fit = form.fit(&mut p, Rect::new(0.0, 0.0, w, h));
                    let problems = fit.problems();
                    assert!(
                        problems.is_empty(),
                        "{name}, {w}x{h} at {scale}: {problems:#?}"
                    );
                }
            }
        }
    }

    fn press(a: &mut App, k: KeyName) {
        a.key_pressed(keys::Key::Name(k), None);
    }

    fn page(a: &App) -> &str {
        let f = a.ui.settings.as_ref().expect("the dialog");
        f.page_names()[f.page()]
    }

    /// A value set on its page is applied and saved, and a switch turned on enables the rows
    /// under it at once (the pages are made again after each change).
    #[test]
    fn a_value_set_on_a_page_is_saved_and_enables_the_rows_under_it() {
        let mut a = app();
        a.docs.prefs.skip_brief = false;
        a.open_settings();
        let f = a.ui.settings.as_mut().unwrap();
        assert!(f.focus_in_list(), "the keyboard starts in the list");
        assert!(!f.focus_row(BRIEF_SECS), "disabled while the switch is off");
        assert!(f.focus_row(SKIP_BRIEF));
        assert_eq!(page(&a), TABS);
        press(&mut a, KeyName::Space);
        assert!(a.docs.prefs.skip_brief && a.session.prefs.skip_brief);
        assert!(a.session.serialize().contains("skip_brief=1"));
        let f = a.ui.settings.as_mut().unwrap();
        assert!(f.focus_row(BRIEF_SECS), "enabled once the switch is on");
        press(&mut a, KeyName::Right);
        assert_eq!(
            a.session.prefs.brief_secs,
            Prefs::default().brief_secs + 1,
            "the stepper's value is kept"
        );
        // A page shown from the list, then Tab to its first switch.
        let f = a.ui.settings.as_mut().unwrap();
        let typing = f.page_names().iter().position(|n| *n == TYPING).unwrap();
        f.show_page(typing);
        let was = a.docs.prefs.auto_pair;
        press(&mut a, KeyName::Tab);
        assert_eq!(a.ui.settings.as_ref().unwrap().focus_id(), Some(AUTO_PAIR));
        press(&mut a, KeyName::Space);
        assert_eq!(a.session.prefs.auto_pair, !was);
    }

    /// Model and Effort are dropdowns even with two options ("Default", "Other…"), so the rows
    /// keep their shape when a model list is fetched.
    #[test]
    fn model_and_effort_are_dropdowns_whatever_their_option_count() {
        let mut a = app();
        a.open_settings();
        let f = a.ui.settings.as_mut().unwrap();
        let at = f
            .page_names()
            .iter()
            .position(|n| *n == AI_REWRITE)
            .unwrap();
        f.show_page(at);
        let mut bmp = Bitmap::new(1280, 800);
        let mut gfx = Gfx::new();
        let mut p = Painter::new(&mut bmp, &mut gfx, 1.0);
        f.layout(&mut p, Rect::new(0.0, 0.0, 1280.0, 800.0));
        for id in [AI_MODEL, AI_EFFORT] {
            assert_ne!(f.control_rect(id), Rect::default(), "{id} is on the page");
            assert_eq!(
                f.segment_rect(id, 0),
                Rect::default(),
                "{id} has no segments"
            );
        }
    }

    /// `--settings-section` names a page, or the group a row was in before the pages.
    #[test]
    fn a_section_names_a_page_or_the_group_before_the_pages() {
        assert_eq!(page_for_section("Preview"), Some(EDITOR));
        assert_eq!(page_for_section("view"), Some(APPEARANCE));
        assert_eq!(page_for_section("ai-rewrite"), Some(AI_REWRITE));
        assert_eq!(page_for_section("Notes library"), Some(NOTES_LIBRARY));
        assert_eq!(page_for_section("nonsense"), None);
    }

    /// A program's path that is longer than a help line keeps its end.
    #[test]
    fn a_long_path_keeps_its_last_folders() {
        assert_eq!(short_path("/usr/local/bin/claude"), "/usr/local/bin/claude");
        assert_eq!(
            short_path(r"C:\Users\someone\AppData\Local\Programs\claude-code\bin\claude.exe"),
            "\u{2026}Programs\\claude-code\\bin\\claude.exe"
        );
        let long = "x".repeat(60);
        assert_eq!(short_path(&long).chars().count(), 40);
    }

    /// The dialog shows the page a section names, and opens again on the page it was closed on.
    #[test]
    fn a_section_shows_its_page_and_the_dialog_reopens_on_the_last_page() {
        let mut a = app();
        assert!(!a.show_settings_section("Tabs"), "no dialog open");
        a.open_settings();
        assert_eq!(page(&a), APPEARANCE, "the first page the first time");
        assert!(a.show_settings_section("Tabs"));
        assert_eq!(page(&a), TABS);
        assert!(a.show_settings_section("ai-service"));
        assert_eq!(page(&a), AI_SERVICE);
        assert!(!a.show_settings_section("nonsense"));
        assert_eq!(page(&a), AI_SERVICE);
        press(&mut a, KeyName::Escape);
        assert!(a.ui.settings.is_none());
        a.open_settings();
        assert_eq!(page(&a), AI_SERVICE);
        assert!(a.ui.settings.as_ref().unwrap().focus_in_list());
    }

    /// The library folder's Choose… is a button on the folder's row now, on its page.
    #[test]
    fn the_library_folder_row_has_its_choose_button() {
        let mut a = app();
        a.open_settings();
        let f = a.ui.settings.as_mut().unwrap();
        assert!(f.focus_row(LIBRARY_CHOOSE));
        assert_eq!(page(&a), NOTES_LIBRARY);
        assert_eq!(
            a.ui.settings.as_ref().unwrap().page_of(LIBRARY_DIR),
            a.ui.settings.as_ref().unwrap().page_of(LIBRARY_CHOOSE)
        );
    }
}
