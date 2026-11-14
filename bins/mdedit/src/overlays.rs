//! Popup menus (built here, drawn by `mdshared_ui::menu`), the modal dialogs and the empty state.
//! Adapted from the viewer's `bins/mdv-app/src/overlays.rs` and `page.rs`.

use crate::chrome::{Ctx, glyph, logo};
use crate::commands::{CommandId as C, command, hint, shortcut_help, strings};
use crate::docs::Docs;
use crate::session::ThemeChoice;
use crate::ui::{Act, Dialog, DialogKind, EditAct, FindOpt, Hit, Scroller, Ui};
use mdedit_core::find::FindOptions;
use mdshared_ui::gfx::{Face, Rect, mix};
use mdshared_ui::menu::{Item, Menu};
pub use mdshared_ui::overlay::Bubble;
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_card, draw_keycaps,
};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};

// ---- menus -----------------------------------------------------------------------------------

fn cmd(id: C, label: &str, icon: &'static str) -> Item<Act> {
    Item::new(Act::Cmd(id), label).hint(hint(id)).icon(icon)
}

/// The "⋯" menu: everything the bar does not show.
pub fn main_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let has_tab = docs.active().is_some();
    let ready = docs.active_ready().is_some();
    let modified = docs.active_doc().is_some_and(|d| d.modified());
    let theme = |choice: ThemeChoice, label: &str, icon: &'static str| {
        Item::new(Act::Theme(choice), label)
            .icon(icon)
            .checked(docs.theme == choice)
    };
    #[cfg_attr(windows, allow(unused_mut))]
    let mut items = vec![
        cmd(C::NewFile, "New file", glyph::NEW_FILE),
        cmd(C::OpenFiles, "Open files…", glyph::OPEN),
        cmd(C::OpenFolder, "Open folder…", glyph::FOLDER_OPEN),
        Item::new(Act::RecentMenu, "Open recent")
            .icon(glyph::RECENT)
            .hint("›")
            .enabled(!docs.recents.is_empty()),
        cmd(C::Save, "Save", glyph::SAVE).sep().enabled(modified),
        cmd(C::SaveAs, "Save as…", glyph::SAVE_AS).enabled(ready),
        cmd(C::Reload, "Reload", glyph::RELOAD).enabled(has_tab),
        cmd(C::CloseTab, "Close tab", glyph::CLOSE).enabled(has_tab),
        cmd(C::CloseAllTabs, "Close all tabs", glyph::CLOSE_ALL).enabled(has_tab),
        cmd(C::Find, "Find…", glyph::SEARCH).sep().enabled(has_tab),
        cmd(C::Replace, "Replace…", glyph::REPLACE).enabled(has_tab),
        cmd(C::GoToLine, "Go to line…", glyph::GO_TO_LINE).enabled(has_tab),
        Item::new(Act::ViewMenu, "View").icon(glyph::VIEW).hint("›"),
        Item::new(Act::MacroMenu, "Macro")
            .icon(glyph::MACRO)
            .hint("›"),
        theme(ThemeChoice::System, "System theme", glyph::SYSTEM).sep(),
        theme(ThemeChoice::Light, "Light theme", glyph::SUN),
        theme(ThemeChoice::Dark, "Dark theme", glyph::MOON),
        cmd(C::Settings, "Settings…", glyph::SLIDERS).sep(),
        cmd(C::ShowShortcuts, "Keyboard shortcuts", glyph::KEYBOARD),
    ];
    // Off Windows, from an AppImage (the desktop entry exists), until mdedit is the default.
    #[cfg(not(windows))]
    if mdshared_app::appimage::path().is_some()
        && !mdshared_ui::desktop_entry::is_default(
            strings::APP_NAME,
            mdshared_ui::desktop_entry::EDITOR_MIME_TYPES,
        )
    {
        items.push(cmd(C::MakeDefault, strings::MAKE_DEFAULT, ""));
    }
    items.push(cmd(C::About, "About mdedit", glyph::INFO));
    items.push(cmd(C::Exit, "Exit", glyph::POWER).sep());
    Menu::new(items, x, y, true)
}

pub fn recent_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let mut items: Vec<Item<Act>> = Vec::new();
    let entries = docs.recents.entries();
    if entries.is_empty() {
        items.push(Item::new(Act::Cmd(C::OpenRecent), strings::RECENT_EMPTY).enabled(false));
    }
    for e in entries {
        let folder = e
            .path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let icon = if e.missing { glyph::WARN } else { glyph::DOC };
        items.push(
            Item::new(Act::Recent(e.path.clone()), e.label)
                .hint(folder)
                .icon(icon),
        );
    }
    let any = !docs.recents.is_empty();
    items.push(
        Item::new(Act::Cmd(C::ManageRecent), "Manage recent files…")
            .icon(glyph::SLIDERS)
            .sep(),
    );
    items.push(
        Item::new(Act::Cmd(C::ClearRecent), "Clear recent files")
            .icon(glyph::DELETE)
            .enabled(any),
    );
    Menu::new(items, x, y, align_right)
}

/// The Macro menu, opened from the main menu in its place: record, play, save the macro, then
/// the saved ones (most recently used first) and the dialog that deletes them.
pub fn macro_menu(
    recording: bool,
    any: bool,
    saved: &[String],
    x: f32,
    y: f32,
    align_right: bool,
) -> Menu<Act> {
    let record = if recording {
        "Stop recording"
    } else {
        "Record a macro"
    };
    let record_icon = if recording {
        glyph::STOP
    } else {
        glyph::RECORD
    };
    let mut items = vec![
        cmd(C::RecordMacro, record, record_icon),
        cmd(C::PlayMacro, "Play the macro", glyph::PLAY).enabled(any && !recording),
        cmd(
            C::PlayMacroToEnd,
            "Play it to the end of the file",
            glyph::PLAY_END,
        )
        .enabled(any && !recording),
        cmd(C::SaveMacroAs, "Save the macro as…", glyph::SAVE_AS)
            .sep()
            .enabled(any && !recording),
    ];
    for (i, name) in saved.iter().enumerate() {
        let item = Item::new(Act::PlaySaved(i), name.as_str())
            .icon(glyph::PLAY)
            .enabled(!recording);
        items.push(if i == 0 { item.sep() } else { item });
    }
    if !saved.is_empty() {
        items.push(cmd(C::DeleteMacros, "Delete saved macros…", glyph::DELETE).sep());
    }
    Menu::new(items, x, y, align_right)
}

/// The View menu, opened from the main menu in its place.
pub fn view_menu(docs: &Docs, ui: &Ui, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let items = vec![
        cmd(C::ToggleFiles, "Files pane", glyph::SIDEBAR).checked(ui.panes.files && !ui.outline.on),
        cmd(C::ToggleOutline, "Function list", glyph::FUNCTIONS)
            .checked(ui.panes.files && ui.outline.on),
        cmd(C::ToggleWhitespace, "Show whitespace", glyph::WHITESPACE).checked(docs.whitespace),
        cmd(C::ToggleWrap, "Word wrap", glyph::WRAP).checked(docs.wrap),
        cmd(C::ToggleMinimap, "Minimap", glyph::MINIMAP).checked(docs.minimap),
        cmd(C::ToggleSplit, "Two files side by side", glyph::SPLIT).checked(docs.other().is_some()),
        cmd(C::FocusOther, "Go to the other pane", glyph::OTHER_PANE)
            .enabled(docs.other().is_some()),
        cmd(C::ToggleFold, "Fold or open the block", glyph::FOLD).sep(),
        cmd(C::FoldAll, "Fold all blocks", glyph::FOLD_ALL),
        cmd(C::UnfoldAll, "Open all folds", glyph::UNFOLD)
            .enabled(docs.active_ready().is_some_and(|r| !r.folds.is_empty())),
        cmd(C::ZoomIn, "Zoom in", glyph::ZOOM).sep(),
        cmd(C::ZoomOut, "Zoom out", glyph::ZOOM_OUT),
        cmd(C::ZoomReset, "Reset zoom", glyph::ZOOM_RESET).enabled(docs.zoom != 100),
    ];
    Menu::new(items, x, y, align_right)
}

/// The minimap's right-click menu: one item.
pub fn minimap_menu(x: f32, y: f32) -> Menu<Act> {
    Menu::new(
        vec![cmd(C::ToggleMinimap, "Hide minimap", glyph::MINIMAP)],
        x,
        y,
        false,
    )
}

pub fn tab_menu(docs: &Docs, index: usize, x: f32, y: f32) -> Menu<Act> {
    let many = docs.tabs.len() > 1;
    let has_file = docs.tabs.get(index).is_some_and(|d| d.path.is_some());
    let items = vec![
        Item::new(Act::TabCmd(C::CloseTab, index), "Close tab").icon(glyph::CLOSE),
        Item::new(Act::TabCmd(C::CloseOtherTabs, index), "Close other tabs")
            .icon(glyph::CLOSE_OTHERS)
            .enabled(many),
        Item::new(Act::TabCmd(C::CloseAllTabs, index), "Close all tabs").icon(glyph::CLOSE_ALL),
        Item::new(Act::TabCmd(C::CopyFilePath, index), "Copy file path")
            .icon(glyph::COPY)
            .sep()
            .enabled(has_file),
        Item::new(
            Act::TabCmd(C::ShowInFileManager, index),
            command(C::ShowInFileManager).label,
        )
        .icon(glyph::FOLDER_OPEN)
        .enabled(has_file),
    ];
    Menu::new(items, x, y, false)
}

/// The text area's right-click menu.
pub fn text_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let has_selection = docs
        .active_ready()
        .is_some_and(|r| !r.sel.range().is_empty());
    let ready = docs.active_ready().is_some();
    let history = docs.active_ready().map(|r| &r.history);
    let has_comment = docs
        .active_ready()
        .and_then(|r| r.language)
        .is_some_and(|l| l.line_comment().is_some());
    let markdown = docs.active_ready().is_some_and(|r| r.assist.markdown);
    let items = vec![
        cmd(C::Undo, "Undo", glyph::UNDO).enabled(history.is_some_and(|h| h.can_undo())),
        cmd(C::Redo, "Redo", glyph::REDO).enabled(history.is_some_and(|h| h.can_redo())),
        cmd(C::Cut, "Cut", glyph::CUT).sep().enabled(has_selection),
        Item::new(Act::Cmd(C::Copy), "Copy")
            .hint(hint(C::Copy))
            .icon(glyph::COPY)
            .enabled(has_selection),
        cmd(C::Paste, "Paste", glyph::PASTE).enabled(ready),
        Item::new(Act::Cmd(C::SelectAll), "Select all")
            .hint(hint(C::SelectAll))
            .icon(glyph::SELECT_ALL)
            .enabled(ready),
        if markdown {
            Item::new(Act::MarkdownMenu, "Markdown")
                .icon(glyph::EDIT)
                .hint("›")
                .sep()
        } else {
            cmd(C::ToggleComment, "Toggle comment", glyph::COMMENT)
                .sep()
                .enabled(has_comment)
        },
        Item::new(Act::LinesMenu, "Lines")
            .icon(glyph::DUPLICATE_LINE)
            .hint("›")
            .enabled(ready),
        Item::new(Act::ConvertMenu, "Convert")
            .icon(glyph::UPPERCASE)
            .hint("›")
            .enabled(ready),
        cmd(C::Find, "Find…", glyph::SEARCH).sep().enabled(ready),
        cmd(C::Replace, "Replace…", glyph::REPLACE).enabled(ready),
        cmd(C::Reload, "Reload", glyph::RELOAD),
    ];
    Menu::new(items, x, y, false)
}

/// The text menu's Lines submenu: the line commands, then the ones that sort and join them.
pub fn lines_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let items = vec![
        cmd(C::DuplicateLine, "Duplicate line", glyph::DUPLICATE_LINE).enabled(ready),
        cmd(C::DeleteLine, "Delete line", glyph::DELETE_LINE).enabled(ready),
        cmd(C::MoveLineUp, "Move line up", glyph::UP).enabled(ready),
        cmd(C::MoveLineDown, "Move line down", glyph::DOWN).enabled(ready),
        // No sort icon in the shared set: these have none rather than a borrowed one.
        cmd(C::SortLines, "Sort lines", "").sep().enabled(ready),
        cmd(C::SortLinesDescending, "Sort lines descending", "").enabled(ready),
        cmd(C::SortLinesIgnoreCase, "Sort lines ignoring case", "").enabled(ready),
        cmd(C::ReverseLines, "Reverse lines", "").enabled(ready),
        cmd(C::UniqueLines, "Remove duplicate lines", "").enabled(ready),
        cmd(C::JoinLines, "Join lines", "").enabled(ready),
        cmd(C::ToggleBookmark, "Toggle bookmark", glyph::BOOKMARK)
            .sep()
            .enabled(ready),
    ];
    Menu::new(items, x, y, false)
}

/// The text menu's Markdown submenu, for Markdown documents.
pub fn markdown_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let items = vec![
        cmd(C::MdBold, "Bold", "").enabled(ready),
        cmd(C::MdItalic, "Italic", "").enabled(ready),
        cmd(C::MdCode, "Code", "").enabled(ready),
        cmd(C::MdStrike, "Strikethrough", "").enabled(ready),
        cmd(C::MdHeading, "Heading level", "").sep().enabled(ready),
        cmd(C::MdLink, "Link", glyph::LINK).enabled(ready),
        cmd(C::MdTable, "Align table", "").sep().enabled(ready),
    ];
    Menu::new(items, x, y, false)
}

/// The text menu's Convert submenu: case, tabs and trailing spaces.
pub fn convert_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let has_selection = docs
        .active_ready()
        .is_some_and(|r| !r.sel.range().is_empty());
    let items = vec![
        cmd(C::UpperCase, "UPPERCASE", glyph::UPPERCASE).enabled(has_selection),
        cmd(C::LowerCase, "lowercase", glyph::LOWERCASE).enabled(has_selection),
        cmd(C::TabsToSpaces, "Tabs to spaces", glyph::TABS)
            .sep()
            .enabled(ready),
        cmd(C::TrimTrailing, "Trim trailing spaces", glyph::TRIM).enabled(ready),
    ];
    Menu::new(items, x, y, false)
}

/// The menu of the bar's text boxes: the editing commands a text box has everywhere, and the
/// search options.
pub fn find_menu(
    has_text: bool,
    has_selection: bool,
    options: FindOptions,
    x: f32,
    y: f32,
) -> Menu<Act> {
    let items = vec![
        Item::new(Act::Edit(EditAct::Cut), "Cut")
            .hint("Ctrl+X")
            .icon(glyph::CUT)
            .enabled(has_selection),
        Item::new(Act::Edit(EditAct::Copy), "Copy")
            .hint("Ctrl+C")
            .icon(glyph::COPY)
            .enabled(has_selection),
        Item::new(Act::Edit(EditAct::Paste), "Paste")
            .hint("Ctrl+V")
            .icon(glyph::PASTE),
        Item::new(Act::Edit(EditAct::SelectAll), "Select all")
            .hint("Ctrl+A")
            .icon(glyph::SELECT_ALL)
            .enabled(has_text)
            .sep(),
        Item::new(Act::Opt(FindOpt::Case), "Match case")
            .icon(glyph::CASE)
            .checked(options.match_case)
            .sep(),
        Item::new(Act::Opt(FindOpt::Word), "Whole word")
            .icon(glyph::WORD)
            .checked(options.whole_word),
        Item::new(Act::Opt(FindOpt::Regex), "Regular expression")
            .icon(glyph::REGEX)
            .checked(options.regex),
        Item::new(Act::Cmd(C::FindInFiles), "Find in files")
            .hint("Ctrl+Shift+F")
            .icon(glyph::SEARCH)
            .enabled(has_text)
            .sep(),
        Item::new(Act::Cmd(C::FindInTabs), "Find in open tabs")
            .icon(glyph::FIND_TABS)
            .enabled(has_text),
    ];
    Menu::new(items, x, y, false)
}

// ---- dialogs ---------------------------------------------------------------------------------

const PAD: f32 = DIALOG_PAD;
const FOOTER: f32 = DIALOG_FOOTER;
const KEY_ROW: f32 = 34.0;
const RECENT_ROW: f32 = 52.0;
const MACRO_ROW: f32 = 48.0;

/// The card for a dialog in a `w` x `h` window, sized to its list where it has one.
pub fn card(kind: DialogKind, rows: usize, w: f32, h: f32) -> Rect {
    let list =
        |row: f32, min: f32| (PAD + 36.0 + (rows as f32 * row + 12.0).max(min) + FOOTER).round();
    let (cw, ch): (f32, f32) = match kind {
        DialogKind::About => (520.0, 256.0),
        DialogKind::Shortcuts => (580.0, list(KEY_ROW, 0.0).min(700.0)),
        DialogKind::Recent => (600.0, list(RECENT_ROW, 80.0).min(560.0)),
        DialogKind::Macros => (480.0, list(MACRO_ROW, 80.0).min(560.0)),
        DialogKind::Unsaved | DialogKind::Overwrite | DialogKind::Discard => (460.0, 196.0),
    };
    dialog_card(cw, ch, w, h)
}

fn button(c: &mut Ctx, r: Rect, hit: Hit, label: &str, primary: bool) {
    let hovered = c.hover == Some(hit);
    draw_dialog_button(
        c.p,
        c.pal,
        r,
        label,
        primary,
        hovered,
        c.pressed == Some(hit),
    );
    c.hit(hit, r);
}

fn title(c: &mut Ctx, r: Rect, text: &str) {
    let pal = *c.pal;
    c.p.text(
        Face::Semibold,
        17.0,
        r.x + PAD,
        r.y + PAD + 10.0,
        text,
        pal.text,
        r.w - 2.0 * PAD,
    );
}

fn list_rect(r: Rect, footer: Rect) -> Rect {
    Rect::new(
        r.x + 8.0,
        r.y + PAD + 36.0,
        r.w - 16.0,
        footer.y - (r.y + PAD + 36.0) - 4.0,
    )
}

pub fn draw_dialog(
    c: &mut Ctx,
    docs: &Docs,
    macros: &[crate::macros::Named],
    d: &mut Dialog,
    w: f32,
    h: f32,
    hover_row: Option<usize>,
) {
    let pal = *c.pal;
    c.hit(Hit::Backdrop, Rect::new(0.0, 0.0, w, h));
    draw_backdrop(c.p, &pal, w, h);
    let rows = match d.kind {
        DialogKind::About | DialogKind::Unsaved | DialogKind::Overwrite | DialogKind::Discard => 0,
        DialogKind::Shortcuts => shortcut_help().len(),
        DialogKind::Recent => docs.recents.paths().len(),
        DialogKind::Macros => macros.len(),
    };
    let r = card(d.kind, rows, w, h);
    c.hit(Hit::DlgCard, r);
    let footer = draw_dialog_card(c.p, &pal, r);
    let by = footer.y + (FOOTER - 32.0) / 2.0;
    match d.kind {
        DialogKind::About => {
            logo(c.p, Rect::new(r.x + PAD, r.y + PAD + 4.0, 56.0, 56.0));
            let x = r.x + PAD + 56.0 + 20.0;
            let tw = r.right() - x - PAD;
            c.p.text(
                Face::Semibold,
                18.0,
                x,
                r.y + PAD + 14.0,
                strings::APP_NAME,
                pal.text,
                tw,
            );
            c.p.text(
                Face::Ui,
                12.5,
                x,
                r.y + PAD + 38.0,
                &format!("mdedit {}", env!("CARGO_PKG_VERSION")),
                pal.muted,
                tw,
            );
            let y = r.y + PAD + 60.0;
            let used =
                c.p.paragraph(Face::Ui, 13.5, x, y, tw, strings::ABOUT, pal.text);
            c.p.paragraph(
                Face::Ui,
                12.5,
                x,
                y + used + 6.0,
                tw,
                strings::ABOUT_DETAIL,
                pal.muted,
            );
            let bw = dialog_button_w(c.p, "OK");
            button(
                c,
                Rect::new(r.right() - PAD - bw, by, bw, 32.0),
                Hit::DlgButton(0),
                "OK",
                true,
            );
            // Updates live here, beside the version, as in mdview: look now (or install what
            // was found, which stays one click away after its bar is closed), and the
            // at-launch setting.
            let look = docs.updates.about_label();
            let lw = dialog_button_w(c.p, &look);
            button(
                c,
                Rect::new(r.x + PAD, by, lw, 32.0),
                Hit::DlgButton(1),
                &look,
                false,
            );
            let launch = crate::updater::at_launch_label(docs.check_updates);
            let aw = dialog_button_w(c.p, launch);
            button(
                c,
                Rect::new(r.x + PAD + lw + 8.0, by, aw, 32.0),
                Hit::DlgButton(2),
                launch,
                false,
            );
        }
        DialogKind::Unsaved | DialogKind::Overwrite | DialogKind::Discard => {
            let (heading, body, buttons): (&str, String, &[&str]) = match d.kind {
                DialogKind::Unsaved => (
                    strings::UNSAVED_TITLE,
                    strings::unsaved_body(&d.subject),
                    &[strings::SAVE, strings::DONT_SAVE, strings::CANCEL],
                ),
                DialogKind::Overwrite => (
                    strings::OVERWRITE_TITLE,
                    strings::overwrite_body(&d.subject),
                    &[strings::OVERWRITE, strings::CANCEL],
                ),
                _ => (
                    strings::DISCARD_TITLE,
                    strings::discard_body(&d.subject),
                    &[strings::RELOAD, strings::CANCEL],
                ),
            };
            title(c, r, heading);
            c.p.paragraph(
                Face::Ui,
                13.5,
                r.x + PAD,
                r.y + PAD + 40.0,
                r.w - 2.0 * PAD,
                &body,
                pal.text,
            );
            // The answers sit at the right in reading order, the first one the default.
            let mut x = r.right() - PAD;
            for (n, label) in buttons.iter().enumerate().rev() {
                let bw = dialog_button_w(c.p, label);
                x -= bw;
                button(
                    c,
                    Rect::new(x, by, bw, 32.0),
                    Hit::DlgButton(n),
                    label,
                    n == 0,
                );
                x -= 8.0;
            }
        }
        DialogKind::Shortcuts => {
            title(c, r, strings::SHORTCUTS_TITLE);
            let list = list_rect(r, footer);
            let help = shortcut_help();
            d.list = list;
            d.view = list.h;
            d.content = help.len() as f32 * KEY_ROW + 8.0;
            d.scroll = d.scroll.clamp(0.0, (d.content - d.view).max(0.0));
            let old = c.p.clip(list);
            for (i, (keys, label)) in help.iter().enumerate() {
                let y = list.y + i as f32 * KEY_ROW - d.scroll;
                if y + KEY_ROW < list.y || y > list.bottom() {
                    continue;
                }
                let row = Rect::new(list.x + 8.0, y, list.w - 16.0, KEY_ROW);
                if i % 2 == 1 {
                    c.p.rbox(row, 5.0, mix(pal.popup, pal.text, 0.025));
                }
                c.p.text(
                    Face::Ui,
                    13.0,
                    row.x + 12.0,
                    row.cy(),
                    label,
                    pal.text,
                    row.w * 0.5,
                );
                draw_keycaps(c.p, &pal, row, keys);
            }
            c.p.restore(old);
            dialog_scrollbar(c, d);
            let bw = dialog_button_w(c.p, "Close");
            button(
                c,
                Rect::new(r.right() - PAD - bw, by, bw, 32.0),
                Hit::DlgButton(0),
                "Close",
                true,
            );
        }
        DialogKind::Macros => {
            title(c, r, strings::MACROS_TITLE);
            let list = list_rect(r, footer);
            d.list = list;
            d.view = list.h;
            d.content = macros.len() as f32 * MACRO_ROW + 8.0;
            d.scroll = d.scroll.clamp(0.0, (d.content - d.view).max(0.0));
            if macros.is_empty() {
                c.p.text_center(
                    Face::Ui,
                    13.5,
                    Rect::new(list.x, list.y + 30.0, list.w, 24.0),
                    strings::MACROS_EMPTY,
                    pal.faint,
                );
            }
            let old = c.p.clip(list);
            for (i, m) in macros.iter().enumerate() {
                let y = list.y + i as f32 * MACRO_ROW - d.scroll;
                if y + MACRO_ROW < list.y || y > list.bottom() {
                    continue;
                }
                let row = Rect::new(list.x + 8.0, y + 2.0, list.w - 16.0, MACRO_ROW - 4.0);
                if hover_row == Some(i) {
                    c.p.rbox(row, 6.0, mix(pal.popup, pal.text, 0.05));
                }
                c.p.icon(glyph::MACRO, 15.0, row.x + 20.0, row.cy(), pal.accent);
                let tx = row.x + 40.0;
                let tw = row.w - 40.0 - 120.0;
                c.p.text(Face::Ui, 13.5, tx, row.y + 14.0, &m.name, pal.text, tw);
                let keys = match m.steps.len() {
                    1 => "1 key".to_string(),
                    n => format!("{n} keys"),
                };
                c.p.text(Face::Ui, 11.5, tx, row.y + 31.0, &keys, pal.faint, tw);
                let play = Rect::new(row.right() - 112.0, row.cy() - 15.0, 70.0, 30.0);
                button(c, play, Hit::DlgOpen(i), "Play", false);
                let rm = Rect::new(row.right() - 36.0, row.cy() - 15.0, 30.0, 30.0);
                let hit = Hit::DlgRemove(i);
                if c.hover == Some(hit) {
                    let fill = if c.pressed == Some(hit) {
                        pal.press
                    } else {
                        pal.hover
                    };
                    c.p.rbox(rm, 5.0, fill);
                }
                c.p.icon(glyph::DELETE, 14.0, rm.cx(), rm.cy(), pal.muted);
                c.hit(hit, rm);
            }
            c.p.restore(old);
            dialog_scrollbar(c, d);
            let bw = dialog_button_w(c.p, strings::RECENT_DONE);
            button(
                c,
                Rect::new(r.right() - PAD - bw, by, bw, 32.0),
                Hit::DlgButton(0),
                strings::RECENT_DONE,
                true,
            );
            if !macros.is_empty() {
                let cw = dialog_button_w(c.p, strings::MACROS_DELETE_ALL);
                button(
                    c,
                    Rect::new(r.x + PAD, by, cw, 32.0),
                    Hit::DlgButton(1),
                    strings::MACROS_DELETE_ALL,
                    false,
                );
            }
        }
        DialogKind::Recent => {
            title(c, r, strings::MANAGE_RECENT_TITLE);
            let list = list_rect(r, footer);
            let entries = docs.recents.entries();
            d.list = list;
            d.view = list.h;
            d.content = entries.len() as f32 * RECENT_ROW + 8.0;
            d.scroll = d.scroll.clamp(0.0, (d.content - d.view).max(0.0));
            if entries.is_empty() {
                c.p.text_center(
                    Face::Ui,
                    13.5,
                    Rect::new(list.x, list.y + 30.0, list.w, 24.0),
                    strings::RECENT_EMPTY,
                    pal.faint,
                );
            }
            let old = c.p.clip(list);
            for (i, e) in entries.iter().enumerate() {
                let y = list.y + i as f32 * RECENT_ROW - d.scroll;
                if y + RECENT_ROW < list.y || y > list.bottom() {
                    continue;
                }
                let row = Rect::new(list.x + 8.0, y + 2.0, list.w - 16.0, RECENT_ROW - 4.0);
                if hover_row == Some(i) {
                    c.p.rbox(row, 6.0, mix(pal.popup, pal.text, 0.05));
                }
                c.p.icon(
                    if e.missing { glyph::WARN } else { glyph::DOC },
                    15.0,
                    row.x + 20.0,
                    row.cy(),
                    if e.missing { pal.warn } else { pal.accent },
                );
                let tx = row.x + 40.0;
                let tw = row.w - 40.0 - 150.0;
                let name = e
                    .path
                    .file_name()
                    .map_or_else(|| e.label.clone(), |n| n.to_string_lossy().into_owned());
                let shown = if e.missing {
                    format!("{name}  (missing)")
                } else {
                    name
                };
                c.p.text(
                    Face::Ui,
                    13.5,
                    tx,
                    row.y + 15.0,
                    &shown,
                    if e.missing { pal.muted } else { pal.text },
                    tw,
                );
                let dir = e
                    .path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                c.p.text(Face::Ui, 11.5, tx, row.y + 33.0, &dir, pal.faint, tw);
                let open = Rect::new(row.right() - 112.0, row.cy() - 15.0, 70.0, 30.0);
                if !e.missing {
                    button(c, open, Hit::DlgOpen(i), "Open", false);
                }
                let rm = Rect::new(row.right() - 36.0, row.cy() - 15.0, 30.0, 30.0);
                let hit = Hit::DlgRemove(i);
                if c.hover == Some(hit) {
                    let fill = if c.pressed == Some(hit) {
                        pal.press
                    } else {
                        pal.hover
                    };
                    c.p.rbox(rm, 5.0, fill);
                }
                c.p.icon(glyph::DELETE, 14.0, rm.cx(), rm.cy(), pal.muted);
                c.hit(hit, rm);
            }
            c.p.restore(old);
            dialog_scrollbar(c, d);
            let bw = dialog_button_w(c.p, strings::RECENT_DONE);
            button(
                c,
                Rect::new(r.right() - PAD - bw, by, bw, 32.0),
                Hit::DlgButton(0),
                strings::RECENT_DONE,
                true,
            );
            if !entries.is_empty() {
                let cw = dialog_button_w(c.p, strings::RECENT_CLEAR_LIST);
                button(
                    c,
                    Rect::new(r.x + PAD, by, cw, 32.0),
                    Hit::DlgButton(1),
                    strings::RECENT_CLEAR_LIST,
                    false,
                );
            }
        }
    }
}

fn dialog_scrollbar(c: &mut Ctx, d: &Dialog) {
    if let Some((track, thumb)) = scrollbar(d.list, d.content, d.view, d.scroll) {
        let hit = Hit::ScrollBar(Scroller::Dialog);
        let pal = *c.pal;
        draw_scrollbar(c.p, &pal, track, thumb, c.hover == Some(hit));
        c.hit(hit, track);
    }
}

/// The Recent dialog row under `(x, y)`, for hover.
pub fn dialog_row_at(d: &Dialog, x: f32, y: f32) -> Option<usize> {
    let row = match d.kind {
        DialogKind::Recent => RECENT_ROW,
        DialogKind::Macros => MACRO_ROW,
        _ => return None,
    };
    if !d.list.contains(x, y) {
        return None;
    }
    Some(((y - d.list.y + d.scroll) / row).floor() as usize)
}

// ---- empty state -----------------------------------------------------------------------------

/// Recent files the empty state lists.
pub const EMPTY_RECENTS: usize = 6;

/// No tab open: the logo, a hint, an Open button and the most recent files.
pub fn draw_empty(c: &mut Ctx, docs: &Docs, r: Rect) {
    let pal = *c.pal;
    c.p.fill(r, pal.page);
    let recents: Vec<&std::path::PathBuf> =
        docs.recents.paths().iter().take(EMPTY_RECENTS).collect();
    let col_w = (r.w - 48.0).clamp(200.0, 460.0);
    let list_h = if recents.is_empty() {
        0.0
    } else {
        44.0 + recents.len() as f32 * 40.0
    };
    let block_h = 64.0 + 24.0 + 30.0 + 26.0 + 28.0 + 36.0 + list_h;
    let mut y = (r.y + (r.h - block_h) * 0.42).max(r.y + 24.0).round();
    let cx = r.cx();
    logo(c.p, Rect::new((cx - 32.0).round(), y, 64.0, 64.0));
    y += 64.0 + 24.0;
    c.p.text_center(
        Face::Semibold,
        20.0,
        Rect::new(r.x, y, r.w, 30.0),
        strings::APP_NAME,
        pal.text,
    );
    y += 30.0 + 6.0;
    c.p.text_center(
        Face::Ui,
        13.5,
        Rect::new(r.x + 16.0, y, r.w - 32.0, 20.0),
        strings::EMPTY_HINT,
        pal.muted,
    );
    y += 20.0 + 28.0;
    let label = strings::EMPTY_BUTTON;
    let bw = c.p.measure(Face::Semibold, 13.5, label) + 56.0;
    let b = Rect::new((cx - bw / 2.0).round(), y, bw.round(), 36.0);
    let fill = match (
        c.hover == Some(Hit::EmptyOpen),
        c.pressed == Some(Hit::EmptyOpen),
    ) {
        (true, true) => mix(pal.accent, pal.page, 0.2),
        (true, false) => mix(pal.accent, pal.page, 0.1),
        _ => pal.accent,
    };
    c.p.rbox(b, 6.0, fill);
    c.p.icon(glyph::OPEN, 14.0, b.x + 24.0, b.cy(), pal.on_accent);
    c.p.text(
        Face::Semibold,
        13.5,
        b.x + 38.0,
        b.cy(),
        label,
        pal.on_accent,
        f32::INFINITY,
    );
    c.hit(Hit::EmptyOpen, b);
    y += 36.0;
    if recents.is_empty() {
        return;
    }
    y += 28.0;
    let x = (cx - col_w / 2.0).round();
    c.p.text(
        Face::Semibold,
        11.0,
        x + 10.0,
        y + 8.0,
        "RECENT",
        pal.muted,
        col_w,
    );
    c.p.fill(
        Rect::new(x + 70.0, y + 8.0, col_w - 80.0, c.p.px()),
        pal.line,
    );
    y += 20.0;
    for (i, path) in recents.iter().enumerate() {
        let row = Rect::new(x, y, col_w, 36.0);
        let hit = Hit::EmptyRecent(i);
        if c.hover == Some(hit) {
            let fill = if c.pressed == Some(hit) {
                pal.press
            } else {
                pal.hover
            };
            c.p.rbox(row, 6.0, fill);
        }
        let missing = !path.exists();
        c.p.icon(
            if missing { glyph::WARN } else { glyph::DOC },
            14.0,
            x + 20.0,
            row.cy(),
            if missing { pal.warn } else { pal.accent },
        );
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        let nw = c.p.text(
            Face::Ui,
            13.5,
            x + 38.0,
            row.cy(),
            &name,
            if missing { pal.faint } else { pal.text },
            col_w - 60.0,
        );
        if let Some(dir) = path.parent() {
            let left = x + 38.0 + nw + 12.0;
            c.p.text(
                Face::Ui,
                12.0,
                left,
                row.cy() + 0.5,
                &dir.display().to_string(),
                pal.faint,
                row.right() - left - 12.0,
            );
        }
        c.hit(hit, row);
        y += 40.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_fit_small_windows() {
        for kind in [
            DialogKind::About,
            DialogKind::Shortcuts,
            DialogKind::Recent,
            DialogKind::Macros,
            DialogKind::Unsaved,
            DialogKind::Overwrite,
            DialogKind::Discard,
        ] {
            let r = card(kind, 30, 520.0, 360.0);
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.right() <= 520.0 && r.bottom() <= 360.0);
        }
    }
}
