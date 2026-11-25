//! Popup menus (built here, drawn by `mdshared_ui::menu`), the modal dialogs and the empty state.
//! Adapted from the viewer's `bins/mdv-app/src/overlays.rs` and `page.rs`.

use crate::chrome::{Ctx, glyph, logo};
use crate::commands::{CommandId as C, command, hint, shortcut_help, strings};
use crate::docs::Docs;
use crate::library_pane::Kind;
use crate::session::ThemeChoice;
use crate::ui::{Act, Dialog, DialogKind, EditAct, FindOpt, Hit, Scroller, Ui};
use mdedit_core::find::FindOptions;
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::menu::{Item, Menu, escape_access};
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
        cmd(C::NewFile, "Ne&w file", glyph::NEW_FILE),
        cmd(C::OpenFiles, "&Open files…", glyph::OPEN),
        cmd(C::OpenFolder, "Open fo&lder…", glyph::FOLDER_OPEN),
        Item::new(Act::RecentMenu, "Ope&n recent")
            .icon(glyph::RECENT)
            .hint("›")
            .enabled(!docs.recents.is_empty()),
        cmd(C::Save, "&Save", glyph::SAVE).sep().enabled(modified),
        cmd(C::SaveAs, "Save &as…", glyph::SAVE_AS).enabled(ready),
        cmd(C::Reload, "&Reload", glyph::RELOAD).enabled(has_tab),
        cmd(C::CloseTab, "&Close tab", glyph::CLOSE).enabled(has_tab),
        cmd(C::CloseAllTabs, "Close all ta&bs", glyph::CLOSE_ALL).enabled(has_tab),
        cmd(C::Find, "&Find…", glyph::SEARCH).sep().enabled(has_tab),
        cmd(C::Replace, "Re&place…", glyph::REPLACE).enabled(has_tab),
        cmd(C::GoToLine, "&Go to line…", glyph::GO_TO_LINE).enabled(has_tab),
        cmd(C::CommandPalette, "Comman&d palette…", glyph::SEARCH),
        Item::new(Act::ViewMenu, "&View")
            .icon(glyph::VIEW)
            .hint("›"),
        Item::new(Act::MacroMenu, "&Macro")
            .icon(glyph::MACRO)
            .hint("›"),
        Item::new(Act::LibraryMenu, "Not&es library")
            .icon(glyph::BOOKMARK)
            .hint("›"),
        theme(ThemeChoice::System, "S&ystem theme", glyph::SYSTEM).sep(),
        theme(ThemeChoice::Light, "Lig&ht theme", glyph::SUN),
        theme(ThemeChoice::Dark, "Dar&k theme", glyph::MOON),
        cmd(C::Settings, "Se&ttings…", glyph::SLIDERS).sep(),
        cmd(C::ShowShortcuts, "Keyboard shortc&uts", glyph::KEYBOARD),
    ];
    // Off Windows, from an AppImage (the desktop entry exists), until mdedit is the default.
    #[cfg(not(windows))]
    if mdshared_app::appimage::path().is_some()
        && !mdshared_ui::desktop_entry::is_default(
            strings::APP_NAME,
            mdshared_ui::desktop_entry::EDITOR_MIME_TYPES,
        )
    {
        items.push(cmd(C::MakeDefault, strings::MAKE_DEFAULT, glyph::CHECK));
    }
    items.push(cmd(C::About, "About mded&it", glyph::INFO));
    // "E&xit": the notes library took "e", and Alt+X is the other common letter for Exit.
    items.push(cmd(C::Exit, "E&xit", glyph::POWER).sep());
    Menu::new(items, x, y, true).access_keys(true)
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
            Item::new(Act::Recent(e.path.clone()), escape_access(&e.label))
                .hint(folder)
                .icon(icon),
        );
    }
    let any = !docs.recents.is_empty();
    items.push(
        Item::new(Act::Cmd(C::ManageRecent), "&Manage recent files…")
            .icon(glyph::SLIDERS)
            .sep(),
    );
    items.push(
        Item::new(Act::Cmd(C::ClearRecent), "&Clear recent files")
            .icon(glyph::DELETE)
            .enabled(any),
    );
    Menu::new(items, x, y, align_right).access_keys(true)
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
        "Stop &recording"
    } else {
        "&Record a macro"
    };
    let record_icon = if recording {
        glyph::STOP
    } else {
        glyph::RECORD
    };
    let mut items = vec![
        cmd(C::RecordMacro, record, record_icon),
        cmd(C::PlayMacro, "&Play the macro", glyph::PLAY).enabled(any && !recording),
        cmd(
            C::PlayMacroToEnd,
            "Play &it to the end of the file",
            glyph::PLAY_END,
        )
        .enabled(any && !recording),
        cmd(C::SaveMacroAs, "&Save the macro as…", glyph::SAVE_AS)
            .sep()
            .enabled(any && !recording),
    ];
    for (i, name) in saved.iter().enumerate() {
        let item = Item::new(Act::PlaySaved(i), escape_access(name))
            .icon(glyph::PLAY)
            .enabled(!recording);
        items.push(if i == 0 { item.sep() } else { item });
    }
    if !saved.is_empty() {
        items.push(cmd(C::DeleteMacros, "&Delete saved macros…", glyph::DELETE).sep());
    }
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The notes library's menu, opened from the main menu in its place: the list of notes, the
/// open file into it, and the library files themselves. `has_file` greys the middle one: the
/// library keeps links, so a document with no file cannot go in it yet.
pub fn library_menu(has_file: bool, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let items = vec![
        cmd(C::Library, "&Open a note…", glyph::BOOKMARK),
        cmd(C::LibraryAdd, "&Add this file…", glyph::NEW_FILE).enabled(has_file),
        cmd(C::LibraryAddFolder, "Add a &folder…", glyph::FOLDER_OPEN),
        cmd(
            C::ToggleLibraryPane,
            "Show in the side &pane",
            glyph::SIDEBAR,
        )
        .sep(),
        cmd(C::LibraryEdit, "&Edit a library file…", glyph::DOC),
        cmd(C::SplitIntoNotes, "Spli&t into notes…", glyph::SPLIT)
            .sep()
            .enabled(has_file),
        cmd(C::AiTaskFiles, "AI tas&k…", crate::chrome::SPARKLE),
        cmd(C::AiRuns, "AI &runs…", glyph::MACRO),
    ];
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The label of "Show in the file manager", with its access key on the first letter that no
/// other item of the menu (`taken`: their keys) has.
/// The buttons of the "Undo this run?" question, in the order every other question in this
/// program uses: the answer first, because `Hit::DlgButton(0)` is the one the handler acts on,
/// and Cancel after it. Drawn the other way round, Cancel was the button that undid the run.
/// A refused undo has nothing to answer, so Close is its only button.
pub fn run_undo_buttons(can: bool) -> [(&'static str, bool); 2] {
    if can {
        [("Undo", true), (strings::CANCEL, false)]
    } else {
        [("Close", true), ("", true)]
    }
}

fn reveal_label(taken: &str) -> String {
    let label = command(C::ShowInFileManager).label;
    let at = label
        .char_indices()
        .skip(1)
        .find(|(_, ch)| ch.is_alphabetic() && !taken.contains(ch.to_ascii_lowercase()))
        .map(|(i, _)| i);
    match at {
        Some(i) => format!("{}&{}", &label[..i], &label[i..]),
        None => label.to_string(),
    }
}

/// The right-click menu of a row of the notes library in the left pane (row `i`): what a group,
/// a section or a note can be asked to do. `home` is where a group's new notes go, as its
/// `home:` line writes it; "Clear" is offered only when there is a line to clear. `can_add` says
/// the open file can be written under it (it has a file the library does not list yet). Nothing
/// in it deletes a file on disk: a note or a section goes from the library file only, and
/// "Reveal" is the way to the file. The app shows [`library_row_menu_with_ai`]; this is its
/// rows without the AI line, for the tests of the rest.
#[cfg(test)]
pub fn library_row_menu(
    kind: &Kind,
    home: Option<&str>,
    i: usize,
    can_add: bool,
    x: f32,
    y: f32,
    align_right: bool,
) -> Menu<Act> {
    Menu::new(library_row_items(kind, home, i, can_add), x, y, align_right).access_keys(true)
}

/// The same menu with "AI task on this note / these N notes…" at its head (pictures plan 4.2):
/// `notes` is how many notes the row holds, folded or not, so what will be taken is known
/// before the click. A row with none has the line greyed.
#[allow(clippy::too_many_arguments)]
pub fn library_row_menu_with_ai(
    kind: &Kind,
    home: Option<&str>,
    i: usize,
    can_add: bool,
    notes: usize,
    x: f32,
    y: f32,
    align_right: bool,
) -> Menu<Act> {
    let mut items = library_row_items(kind, home, i, can_add);
    if !items.is_empty() {
        let label = match kind {
            Kind::Note { .. } => "AI tas&k on this note\u{2026}".to_string(),
            _ if notes == 1 => "AI tas&k on its one note\u{2026}".to_string(),
            _ if notes == 0 => "AI tas&k on its notes\u{2026}".to_string(),
            _ => format!("AI tas&k on these {notes} notes\u{2026}"),
        };
        let mut ai = Item::new(Act::Library(crate::ui::LibAct::AiTask, i), label)
            .icon(crate::chrome::SPARKLE)
            .enabled(notes > 0);
        if notes == 0 {
            ai = ai.hint(if matches!(kind, Kind::Note { .. }) {
                "Its file is gone"
            } else {
                "No note under it"
            });
        }
        items[0].separator_before = true;
        items.insert(0, ai);
    }
    Menu::new(items, x, y, align_right).access_keys(true)
}

fn library_row_items(kind: &Kind, home: Option<&str>, i: usize, can_add: bool) -> Vec<Item<Act>> {
    use crate::ui::LibAct as L;
    let item =
        |op: L, label: &str, icon: &'static str| Item::new(Act::Library(op, i), label).icon(icon);
    match kind {
        Kind::Group => {
            // Where the notes go is a statement, not a mode: the folder is in the item when
            // there is one. `g` stays its access key, on "go".
            let set_home = match home {
                Some(home) => format!(
                    "New notes &go in: {} \u{2014} Change\u{2026}",
                    escape_access(home)
                ),
                None => "Choose where new notes &go\u{2026}".to_string(),
            };
            let mut items = vec![
                item(L::NewNote, "New &note here\u{2026}", glyph::NEW_FILE),
                item(L::AddOpen, "&Add the open document", glyph::BOOKMARK).enabled(can_add),
                item(L::NewSection, "New &section\u{2026}", glyph::PLUS),
                item(L::SetHome, &set_home, glyph::FOLDER).sep(),
            ];
            if home.is_some() {
                items.push(item(
                    L::ClearHome,
                    "Clear the group's fo&lder",
                    glyph::CLOSE,
                ));
            }
            items.push(item(
                L::MoveNotes,
                "&Move the notes\u{2026}",
                glyph::FORWARD,
            ));
            items.push(item(
                L::MarkdownNote,
                "Markdo&wn note from a file\u{2026}",
                glyph::HEADING,
            ));
            items.extend([
                item(L::Rename, "&Rename group", glyph::EDIT).sep(),
                item(
                    L::RenameFile,
                    "Rename the library &file\u{2026}",
                    glyph::EDIT,
                ),
                item(L::EditFile, "&Edit this library file", glyph::DOC).sep(),
                item(L::CopyPath, "&Copy path", glyph::COPY),
                item(L::Reveal, &reveal_label("nasrfec"), glyph::FOLDER_OPEN),
                item(
                    L::DeleteFile,
                    "&Delete this library file\u{2026}",
                    glyph::DELETE,
                )
                .sep(),
            ]);
            items
        }
        Kind::Section => vec![
            item(L::NewNote, "New &note here\u{2026}", glyph::NEW_FILE),
            item(L::AddOpen, "&Add the open document here", glyph::BOOKMARK).enabled(can_add),
            item(L::NewSection, "New su&bsection\u{2026}", glyph::PLUS),
            item(L::Rename, "&Rename section", glyph::EDIT).sep(),
            item(L::GoToLine, "&Go to this heading", glyph::GO_TO_LINE).sep(),
            item(L::Remove, "Remove this &section\u{2026}", glyph::DELETE).sep(),
        ],
        Kind::Note { missing, .. } => {
            let mut items = vec![
                item(L::Open, "&Open", glyph::DOC).enabled(!missing),
                item(L::OpenOther, "Open in the other &pane", glyph::OTHER_PANE).enabled(!missing),
                item(L::Rename, "&Rename in the library", glyph::EDIT).sep(),
                item(L::MoveTo, "&Move to\u{2026}", glyph::FORWARD),
            ];
            if *missing {
                items.push(item(L::FindFile, "&Find the file\u{2026}", glyph::SEARCH));
            }
            items.extend([
                item(L::CopyPath, "Copy pat&h", glyph::COPY).sep(),
                item(L::Reveal, &reveal_label("orpmfhly"), glyph::FOLDER_OPEN).enabled(!missing),
                item(
                    L::GoToLine,
                    "Go to its &line in the library file",
                    glyph::GO_TO_LINE,
                ),
                item(L::Remove, "Remove from the librar&y", glyph::DELETE).sep(),
            ]);
            items
        }
        Kind::Editing => Vec::new(),
    }
}

/// The notes library pane's menu for its empty space, and the Notes head's plus button.
/// `has_file` greys "Add the open document": it needs a file to link to.
pub fn library_empty_menu(has_file: bool, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    use crate::ui::LibAct as L;
    let items = vec![
        Item::new(Act::Library(L::NewGroup, 0), "New &group\u{2026}").icon(glyph::PLUS),
        cmd(
            C::LibraryAdd,
            "&Add the open document\u{2026}",
            glyph::BOOKMARK,
        )
        .enabled(has_file),
        cmd(C::LibraryAddFolder, "Add a f&older…", glyph::FOLDER_OPEN),
        cmd(C::LibraryEdit, "&Edit a library file\u{2026}", glyph::DOC).sep(),
        Item::new(
            Act::Library(L::RevealFolder, 0),
            "Reveal the library fol&der",
        )
        .icon(glyph::FOLDER_OPEN),
        Item::new(Act::Library(L::Refresh, 0), "Re&fresh")
            .icon(glyph::RELOAD)
            .sep(),
    ];
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// "Preview in mdview" (checked while the file is previewed) and "Preview side by side" for the
/// active tab, or for tab `index`. Listed for Markdown files, which is what the viewer is for,
/// for a document that has no file yet (it has no name to tell its kind by, and a note begun
/// in a new tab is the common case), and
/// for a file that is being previewed so that it can be stopped; the commands work for any
/// document, with a file or without one yet, from the shortcut and the palette.
fn preview_items(docs: &Docs, index: Option<usize>) -> Vec<Item<Act>> {
    let Some(doc) = index.or(docs.active()).and_then(|i| docs.tabs.get(i)) else {
        return Vec::new();
    };
    let linked = docs.links.contains(doc.id);
    if !doc.is_markdown() && !linked && doc.path.is_some() {
        return Vec::new();
    }
    // Each item says what a click does (issue #7): no tick, which read as a setting.
    let [here, preview, side] = preview_labels(docs, linked, TAB_LETTERS);
    let item = |id: C, label: &str| {
        let item = match index {
            Some(i) => Item::new(Act::TabCmd(id, i), label),
            None => Item::new(Act::Cmd(id), label).hint(hint(id)),
        };
        item.icon(glyph::VIEW)
    };
    let mut items = Vec::new();
    if docs.pane {
        items.push(item(C::PreviewPane, here));
        // Visual editing is a way of working, not a window: it keeps its tick.
        if index.is_some() {
            items.push(item(C::VisualEdit, "Edit on the pa&ge").checked(docs.visual_open));
        }
    }
    if docs.viewer {
        items.push(item(C::PreviewInViewer, preview));
        items.push(item(C::PreviewSideBySide, side));
    }
    if let Some(first) = items.first_mut() {
        first.separator_before = true;
    }
    items
}

/// The labels of the three preview commands, each as what a click on it does now: open the
/// preview that way, or close the one that is open. `letters` are the access-key spellings,
/// opening then closing, for a menu whose other items take other letters.
type Letters = [[&'static str; 2]; 3];

const TAB_LETTERS: Letters = [
    ["Preview in t&his window", "Close t&he preview"],
    ["&Preview in mdview", "Sto&p the preview in mdview"],
    ["Preview side &by side", "Stop side &by side"],
];
const MENU_LETTERS: Letters = [
    ["Preview in this &window", "&Close the preview"],
    ["Preview in &mdview", "&Stop the preview in mdview"],
    ["Preview side &by side", "Stop side &by side"],
];

fn preview_labels(docs: &Docs, linked: bool, letters: Letters) -> [&'static str; 3] {
    let open = [docs.pane_open, linked, linked && docs.links.side_by_side];
    [0, 1, 2].map(|k| letters[k][usize::from(open[k])])
}

/// The menu of the Preview button's arrow (issue #7): the modes, each of which opens the
/// preview that way once (or closes it, when it is open so), then which one the button itself
/// opens, a choice kept in the preferences. Modes this window cannot use are left out (mdview
/// not found), and the choice with them when one is left. For a document the preview is not
/// offered for (a file that is not Markdown, shown here only while the pane is open), the
/// mdview modes are left out as the tab menu leaves them out; the choice stays.
pub fn preview_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    use mdedit_core::prefs::PreviewMode;
    let linked = docs.active_doc().is_some_and(|d| docs.links.contains(d.id));
    let [here, preview, side] = preview_labels(docs, linked, MENU_LETTERS);
    let mut items = Vec::new();
    let mut modes = Vec::new();
    if docs.pane {
        items.push(cmd(C::PreviewPane, here, glyph::VIEW));
        modes.push((PreviewMode::Window, "In &this window"));
    }
    if docs.viewer {
        if crate::chrome::previewable(docs) {
            items.push(cmd(C::PreviewInViewer, preview, glyph::VIEW));
            items.push(cmd(C::PreviewSideBySide, side, glyph::VIEW));
        }
        modes.push((PreviewMode::Viewer, "In m&dview"));
        modes.push((PreviewMode::SideBySide, "Side by s&ide"));
    }
    if modes.len() > 1 {
        let chosen = crate::chrome::preview_mode(docs);
        items.push(
            Item::new(Act::PreviewDefault(chosen), "The Preview button opens")
                .sep()
                .enabled(false),
        );
        for (mode, label) in modes {
            let icon = if mode == chosen {
                crate::chrome::RADIO_ON
            } else {
                crate::chrome::RADIO_OFF
            };
            items.push(Item::new(Act::PreviewDefault(mode), label).icon(icon));
        }
    }
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The View menu, opened from the main menu in its place.
pub fn view_menu(docs: &Docs, ui: &Ui, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let mut items = vec![
        cmd(C::ToggleFiles, "&Files pane", glyph::SIDEBAR)
            .checked(ui.panes.files && !ui.outline.on),
        cmd(C::ToggleOutline, "Function &list", glyph::FUNCTIONS)
            .checked(ui.panes.files && ui.outline.on),
        cmd(C::ToggleLibraryPane, "Notes librar&y", glyph::BOOKMARK)
            .checked(ui.panes.files && ui.library.on),
        cmd(C::ToggleWhitespace, "S&how whitespace", glyph::WHITESPACE).checked(docs.whitespace),
        cmd(C::ToggleWrap, "&Word wrap", glyph::WRAP).checked(docs.wrap),
        cmd(C::ToggleMinimap, "&Minimap", glyph::MINIMAP).checked(docs.minimap),
        cmd(C::ToggleFormatBar, "Format bar (Mar&kdown)", glyph::BOLD).checked(docs.format_bar),
        cmd(C::ToggleSplit, "Two files si&de by side", glyph::SPLIT)
            .checked(docs.other().is_some() || docs.same_file),
        cmd(C::FocusOther, "&Go to the other pane", glyph::OTHER_PANE)
            .enabled(docs.other().is_some() || docs.same_file),
        cmd(
            C::ShowInOtherPane,
            "Show this file &in the other pane",
            glyph::SPLIT,
        )
        .enabled(docs.active_ready().is_some()),
    ];
    // While two files are compared the commands that start a comparison are of no use (and
    // greyed); the ones that work on it take their place, so the menu stays short.
    if docs.active_ready().is_some_and(|r| r.view.align.is_some()) {
        items.extend([
            cmd(C::NextDifference, "Ne&xt difference", glyph::DOWN).sep(),
            cmd(C::PreviousDifference, "Previo&us difference", glyph::UP),
            cmd(
                C::CopyChangeAcross,
                "Cop&y this change to the other side",
                glyph::FORWARD,
            )
            .enabled(crate::compare::enabled(C::CopyChangeAcross, docs)),
            cmd(
                C::TakeChangeAcross,
                "Ta&ke this change from the other side",
                glyph::BACK,
            )
            .enabled(crate::compare::enabled(C::TakeChangeAcross, docs)),
            cmd(
                C::CopyLineAcross,
                "&Copy this line to the other side",
                glyph::FORWARD,
            )
            .enabled(crate::compare::enabled(C::CopyLineAcross, docs)),
            cmd(C::StopCompare, "&Stop comparing", glyph::CLOSE),
        ]);
    } else {
        items.extend([
            cmd(
                C::CompareOther,
                "Compar&e with the other pane",
                glyph::SPLIT,
            )
            .sep()
            .enabled(crate::compare::enabled(C::CompareOther, docs)),
            cmd(C::CompareFile, "&Compare with file\u{2026}", glyph::OPEN)
                .enabled(crate::compare::enabled(C::CompareFile, docs)),
            cmd(C::CompareSaved, "Compare wi&th saved", glyph::SAVE)
                .enabled(crate::compare::enabled(C::CompareSaved, docs)),
        ]);
    }
    // The preview's three ways are on the Preview button's arrow (issue #7), the tab menu, the
    // palette and their keys: here they read as switches, which they are not.
    items.extend([
        cmd(C::ToggleFold, "Fold or ope&n the block", glyph::FOLD).sep(),
        cmd(C::FoldAll, "Fold all &blocks", glyph::FOLD_ALL),
        cmd(C::UnfoldAll, "Open &all folds", glyph::UNFOLD).enabled(
            docs.active_ready()
                .is_some_and(|r| !r.view.folds.is_empty()),
        ),
        cmd(C::ZoomIn, "&Zoom in", glyph::ZOOM).sep(),
        cmd(C::ZoomOut, "Zoom &out", glyph::ZOOM_OUT),
        cmd(C::ZoomReset, "&Reset zoom", glyph::ZOOM_RESET).enabled(docs.zoom != 100),
    ]);
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The minimap's right-click menu: one item.
pub fn minimap_menu(x: f32, y: f32) -> Menu<Act> {
    Menu::new(
        vec![cmd(C::ToggleMinimap, "&Hide minimap", glyph::MINIMAP)],
        x,
        y,
        false,
    )
    .access_keys(true)
}

/// The preview pane's right-click menu: the page's selection, as mdview's page menu has it.
pub fn page_menu(selected: bool, x: f32, y: f32) -> Menu<Act> {
    Menu::new(
        vec![
            Item::new(Act::Cmd(C::Copy), "&Copy")
                .hint(hint(C::Copy))
                .icon(glyph::COPY)
                .enabled(selected),
            Item::new(Act::Cmd(C::SelectAll), "Select &all")
                .hint(hint(C::SelectAll))
                .icon(glyph::SELECT_ALL),
        ],
        x,
        y,
        false,
    )
    .access_keys(true)
}

pub fn tab_menu(docs: &Docs, index: usize, x: f32, y: f32) -> Menu<Act> {
    let many = docs.tabs.len() > 1;
    let has_file = docs.tabs.get(index).is_some_and(|d| d.path.is_some());
    let compared = docs.active_ready().is_some_and(|r| r.view.align.is_some());
    let mut items = vec![
        Item::new(Act::TabCmd(C::CloseTab, index), "&Close tab").icon(glyph::CLOSE),
        Item::new(Act::TabCmd(C::CloseOtherTabs, index), "Close &other tabs")
            .icon(glyph::CLOSE_OTHERS)
            .enabled(many),
        Item::new(Act::TabCmd(C::CloseAllTabs, index), "Close &all tabs").icon(glyph::CLOSE_ALL),
        Item::new(
            Act::TabCmd(C::PinTab, index),
            if docs.tabs.get(index).is_some_and(|d| d.pinned) {
                "U&npin tab"
            } else {
                "Pi&n tab"
            },
        )
        .icon(glyph::PIN),
        Item::new(
            Act::TabCmd(C::ShowInOtherPane, index),
            "Show &in the other pane",
        )
        .icon(glyph::SPLIT)
        .sep()
        .enabled(docs.tabs.get(index).is_some_and(|d| d.ready().is_some())),
        Item::new(
            Act::TabCmd(C::CompareFile, index),
            "Compare &with file\u{2026}",
        )
        .icon(glyph::SPLIT)
        .enabled(docs.tabs.get(index).is_some_and(|d| d.ready().is_some()) && !compared),
        Item::new(Act::TabCmd(C::CompareSaved, index), "Compare with &saved")
            .icon(glyph::SAVE)
            .enabled(
                !compared
                    && docs
                        .tabs
                        .get(index)
                        .is_some_and(|d| d.path.is_some() && d.modified()),
            ),
    ];
    items.extend(preview_items(docs, Some(index)));
    items.extend([
        Item::new(Act::TabCmd(C::CopyFilePath, index), "Copy &file path")
            .icon(glyph::COPY)
            .sep()
            .enabled(has_file),
        Item::new(
            Act::TabCmd(C::ShowInFileManager, index),
            command(C::ShowInFileManager)
                .label
                .replace("Explorer", "&Explorer")
                .replace("manager", "&manager")
                .replace("Finder", "Fin&der"),
        )
        .icon(glyph::FOLDER_OPEN)
        .enabled(has_file),
        Item::new(
            Act::TabCmd(C::ShowInLibrary, index),
            "Show in the notes &library",
        )
        .icon(glyph::BOOKMARK)
        .enabled(
            docs.tabs
                .get(index)
                .and_then(|d| d.path.as_deref())
                .is_some_and(|p| crate::library_pane::lists(&docs.library, p)),
        ),
        // The document in front, cut at its headings. A document that is not saved has no file
        // on disk to make the notes from, and the item says so rather than going away.
        {
            let doc = docs.tabs.get(index);
            let on = doc.is_some_and(|d| d.path.is_some() && !d.modified());
            let mut item = Item::new(
                Act::TabCmd(C::SplitIntoNotes, index),
                "Spli&t into notes\u{2026}",
            )
            .icon(glyph::SPLIT)
            .enabled(on);
            if !on && let Some(d) = doc {
                item = item.hint(format!("Save {} first", d.name()));
            }
            item
        },
        // The AI task window on this tab's file (D-207): the one place the command names a
        // file, the one under the pointer.
        Item::new(
            Act::TabCmd(C::AiTaskThisFile, index),
            "AI tas&k on this file\u{2026}",
        )
        .icon(crate::chrome::SPARKLE)
        .enabled(has_file),
    ]);
    Menu::new(items, x, y, false).access_keys(true)
}

/// The text area's right-click menu.
pub fn text_menu(docs: &Docs, x: f32, y: f32) -> Menu<Act> {
    let has_selection = docs
        .active_ready()
        .is_some_and(|r| !r.view.sel.range().is_empty());
    let ready = docs.active_ready().is_some();
    let history = docs.active_ready().map(|r| &r.history);
    let has_comment = docs
        .active_ready()
        .and_then(|r| r.language)
        .is_some_and(|l| l.line_comment().is_some());
    let markdown = docs.active_ready().is_some_and(|r| r.assist.markdown);
    let mut items = vec![
        cmd(C::Undo, "&Undo", glyph::UNDO).enabled(history.is_some_and(|h| h.can_undo())),
        cmd(C::Redo, "&Redo", glyph::REDO).enabled(history.is_some_and(|h| h.can_redo())),
        cmd(C::Cut, "&Cut", glyph::CUT).sep().enabled(has_selection),
        Item::new(Act::Cmd(C::Copy), "Co&py")
            .hint(hint(C::Copy))
            .icon(glyph::COPY)
            .enabled(has_selection),
        cmd(C::Paste, "Pa&ste", glyph::PASTE).enabled(ready),
        cmd(C::PastePlain, "Paste as plain te&xt", glyph::PASTE).enabled(ready),
        Item::new(Act::Cmd(C::SelectAll), "Select &all")
            .hint(hint(C::SelectAll))
            .icon(glyph::SELECT_ALL)
            .enabled(ready),
        Item::new(Act::Cmd(C::SelectNextOccurrence), "Select next &occurrence")
            .hint(hint(C::SelectNextOccurrence))
            .icon(glyph::SELECT_ALL)
            .enabled(ready),
        if markdown {
            Item::new(Act::MarkdownMenu, "&Markdown")
                .icon(glyph::EDIT)
                .hint("›")
                .sep()
        } else {
            cmd(C::ToggleComment, "&Toggle comment", glyph::COMMENT)
                .sep()
                .enabled(has_comment)
        },
    ];
    if markdown {
        items.push(
            Item::new(Act::TableMenu, "Ta&ble")
                .icon(glyph::TABLE)
                .hint("›"),
        );
    }
    items.extend([
        Item::new(Act::LinesMenu, "&Lines")
            .icon(glyph::DUPLICATE_LINE)
            .hint("›")
            .enabled(ready),
        Item::new(Act::ConvertMenu, "Co&nvert")
            .icon(glyph::UPPERCASE)
            .hint("›")
            .enabled(ready),
    ]);
    if docs.prefs.ai_connection != mdedit_core::ai::Connection::Off {
        items.push(
            Item::new(Act::AiMenu, "A&I")
                .icon(crate::chrome::SPARKLE)
                .hint("›")
                .enabled(ready),
        );
    }
    items.extend([
        cmd(C::CompleteWord, "Complete &word", glyph::EDIT).enabled(ready),
        cmd(C::Find, "&Find…", glyph::SEARCH).sep().enabled(ready),
        cmd(C::Replace, "R&eplace…", glyph::REPLACE).enabled(ready),
        cmd(C::Reload, "Reloa&d", glyph::RELOAD),
    ]);
    Menu::new(items, x, y, false).access_keys(true)
}

/// The text menu's Lines submenu: the line commands, then the ones that sort and join them.
pub fn lines_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let marked = docs.active_ready().is_some_and(|r| !r.bookmarks.is_empty());
    let items = vec![
        cmd(C::DuplicateLine, "Du&plicate line", glyph::DUPLICATE_LINE).enabled(ready),
        cmd(C::DeleteLine, "&Delete line", glyph::DELETE_LINE).enabled(ready),
        cmd(C::MoveLineUp, "&Move line up", glyph::UP).enabled(ready),
        cmd(C::MoveLineDown, "Move &line down", glyph::DOWN).enabled(ready),
        cmd(C::SortLines, "&Sort lines", glyph::SORT)
            .sep()
            .enabled(ready),
        cmd(
            C::SortLinesDescending,
            "Sort li&nes descending",
            glyph::SORT_DESC,
        )
        .enabled(ready),
        cmd(
            C::SortLinesIgnoreCase,
            "Sort lines &ignoring case",
            glyph::SORT_CASE,
        )
        .enabled(ready),
        cmd(C::ReverseLines, "&Reverse lines", glyph::REVERSE).enabled(ready),
        cmd(C::UniqueLines, "Remo&ve duplicate lines", glyph::DEDUPE).enabled(ready),
        cmd(C::JoinLines, "&Join lines", glyph::JOIN).enabled(ready),
        cmd(C::ToggleBookmark, "&Toggle bookmark", glyph::BOOKMARK)
            .sep()
            .enabled(ready),
        cmd(C::CutBookmarkedLines, "&Cut bookmarked lines", glyph::CUT).enabled(marked),
        cmd(
            C::CopyBookmarkedLines,
            "Copy &bookmarked lines",
            glyph::COPY,
        )
        .enabled(marked),
        cmd(
            C::DeleteBookmarkedLines,
            "Delete boo&kmarked lines",
            glyph::DELETE,
        )
        .enabled(marked),
    ];
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The text menu's Markdown submenu, for Markdown documents; also what the format bar's
/// overflow button opens.
pub fn markdown_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let items = vec![
        cmd(C::MdBold, "&Bold", glyph::BOLD).enabled(ready),
        cmd(C::MdItalic, "&Italic", glyph::ITALIC).enabled(ready),
        cmd(C::MdCode, "&Code", glyph::CODE).enabled(ready),
        cmd(C::MdStrike, "&Strikethrough", glyph::STRIKE).enabled(ready),
        cmd(C::MdHeading, "&Heading level", glyph::HEADING)
            .sep()
            .enabled(ready),
        cmd(C::MdHeading1, "Heading &1", glyph::HEADING).enabled(ready),
        cmd(C::MdHeading2, "Heading &2", glyph::HEADING).enabled(ready),
        cmd(C::MdHeading3, "Heading &3", glyph::HEADING).enabled(ready),
        cmd(C::MdBulletList, "Bull&eted list", glyph::OUTLINE)
            .sep()
            .enabled(ready),
        cmd(C::MdNumberedList, "&Numbered list", glyph::OUTLINE).enabled(ready),
        cmd(C::MdTaskList, "&Task list", glyph::TASK).enabled(ready),
        cmd(C::MdQuote, "&Quote", glyph::EDIT).enabled(ready),
        cmd(C::MdAlert, "Ale&rt (again for the next kind)", glyph::INFO).enabled(ready),
        cmd(C::MdLink, "&Link", glyph::LINK).sep().enabled(ready),
        cmd(C::MdImage, "Ima&ge", glyph::IMAGE).enabled(ready),
        cmd(C::MdCodeBlock, "C&ode block", glyph::CODE).enabled(ready),
        cmd(C::MdMath, "&Math", glyph::FUNCTIONS).enabled(ready),
        cmd(C::MdMathBlock, "Math bloc&k", glyph::FUNCTIONS).enabled(ready),
        cmd(C::MdDiagram, "&Diagram (Mermaid)", glyph::FUNCTIONS).enabled(ready),
        cmd(C::MdFootnote, "&Footnote", glyph::EDIT).enabled(ready),
        cmd(C::MdDetails, "Colla&psible section", glyph::EDIT).enabled(ready),
        cmd(C::MdRule, "Hori&zontal rule", glyph::EDIT).enabled(ready),
    ];
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The text menu's Table submenu, for Markdown documents: the pipe table the caret is in.
pub fn table_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let items = vec![
        cmd(C::MdTable, "&Align table", glyph::TABLE).enabled(ready),
        cmd(C::MdRowBelow, "Insert &row below", glyph::PLUS)
            .sep()
            .enabled(ready),
        cmd(C::MdRowAbove, "I&nsert row above", glyph::PLUS).enabled(ready),
        cmd(C::MdDeleteRow, "&Delete row", glyph::DELETE).enabled(ready),
        cmd(C::MdMoveRowUp, "Move row &up", glyph::UP).enabled(ready),
        cmd(C::MdMoveRowDown, "Move row do&wn", glyph::DOWN).enabled(ready),
        cmd(C::MdColumnRight, "Insert column ri&ght", glyph::PLUS)
            .sep()
            .enabled(ready),
        cmd(C::MdColumnLeft, "Insert column le&ft", glyph::PLUS).enabled(ready),
        cmd(C::MdDeleteColumn, "Dele&te column", glyph::DELETE).enabled(ready),
        cmd(C::MdMoveColumnLeft, "&Move column left", glyph::BACK).enabled(ready),
        cmd(C::MdMoveColumnRight, "Mo&ve column right", glyph::FORWARD).enabled(ready),
        cmd(C::MdSortColumn, "Sort table b&y this column", glyph::EDIT)
            .sep()
            .enabled(ready),
    ];
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// A saved AI prompt's hint in the AI menu: room kept at the row's right, under the Modify
/// button drawn there (`ai_ui::draw_menu_modify`), so a long name ends in "…" a little before it
/// (the menu elides a label at its hint) and never runs beneath it.
/// The spaces are a marker: [`fit_modify_room`] widens or narrows them to the font in use before
/// the menu is placed, as a space's width differs between fonts.
pub const MENU_MODIFY_ROOM: &str = "              ";

/// The hint giving the Modify button its room in the font in use: the fewest spaces whose width,
/// with the 28 px the menu adds around a hint, holds the button, its 4 px margin and a 6 px gap.
pub fn modify_room(p: &mut mdshared_ui::gfx::Painter) -> String {
    let need = crate::ai_ui::MENU_MODIFY_W + 4.0 + 6.0;
    let mut room = String::from(" ");
    while p.measure(mdshared_ui::gfx::Face::Ui, 12.0, &room) + 28.0 < need && room.len() < 64 {
        room.push(' ');
    }
    room
}

/// Gives each saved prompt's row in `menu` its Modify room in the font in use ([`modify_room`]).
pub fn fit_modify_room(menu: &mut Menu<Act>, p: &mut mdshared_ui::gfx::Painter) {
    if !menu.items.iter().any(|i| i.hint == MENU_MODIFY_ROOM) {
        return;
    }
    let room = modify_room(p);
    for item in menu.items.iter_mut().filter(|i| i.hint == MENU_MODIFY_ROOM) {
        item.hint = room.clone();
    }
}

/// The text menu's AI submenu: the built-in requests (the AI mark), then the saved prompts (play:
/// the row runs one; its Modify button, at the right, puts it in the instruction panel to be
/// changed).
/// Whether the document in front can be split into notes: it has a file and no unsaved changes.
fn split_ok(docs: &Docs) -> bool {
    docs.active_doc()
        .is_some_and(|d| d.path.is_some() && !d.modified())
}

/// The ai menu: the built-in requests, the saved ones, the settings, and what a run left behind.
pub fn ai_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let idle = docs.active_ready().is_some() && docs.ai_status.is_none();
    let mut items = vec![
        cmd(C::AiFormat, "&Format as Markdown", crate::chrome::SPARKLE).enabled(idle),
        cmd(C::AiFix, "Fi&x Markdown", crate::chrome::SPARKLE).enabled(idle),
        cmd(
            C::AiRewrite,
            "&Rewrite with instruction…",
            crate::chrome::SPARKLE,
        )
        .enabled(idle),
    ];
    if docs.ai_kept {
        items.push(cmd(
            C::AiShowResult,
            "Show the &last result",
            crate::chrome::SPARKLE,
        ));
    }
    for (k, p) in docs.ai_prompts.iter().enumerate() {
        let mut item = Item::new(Act::AiPrompt(k), p.name.replace('&', "&&"))
            .icon(glyph::PLAY)
            .hint(MENU_MODIFY_ROOM)
            .enabled(idle);
        if k == 0 {
            item = item.sep();
        }
        items.push(item);
    }
    items.push(cmd(C::AiEditPrompts, "&Edit saved prompts", glyph::DOC).sep());
    items.push(
        cmd(C::SplitIntoNotes, "Spli&t into notes\u{2026}", glyph::SPLIT).enabled(split_ok(docs)),
    );
    items.push(cmd(
        C::AiTaskFiles,
        "AI tas&k\u{2026}",
        crate::chrome::SPARKLE,
    ));
    items.push(cmd(C::AiRuns, "AI runs\u{2026}", glyph::MACRO));
    if docs.ai_status.is_some() {
        items.push(cmd(C::AiCancel, "&Stop rewriting", glyph::STOP));
    }
    Menu::new(items, x, y, align_right).access_keys(true)
}

/// The text menu's Convert submenu: case, tabs and trailing spaces.
pub fn convert_menu(docs: &Docs, x: f32, y: f32, align_right: bool) -> Menu<Act> {
    let ready = docs.active_ready().is_some();
    let has_selection = docs
        .active_ready()
        .is_some_and(|r| !r.view.sel.range().is_empty());
    let items = vec![
        cmd(C::UpperCase, "&UPPERCASE", glyph::UPPERCASE).enabled(has_selection),
        cmd(C::LowerCase, "&lowercase", glyph::LOWERCASE).enabled(has_selection),
        cmd(C::TabsToSpaces, "&Tabs to spaces", glyph::TABS)
            .sep()
            .enabled(ready),
        cmd(C::TrimTrailing, "Trim trailing &spaces", glyph::TRIM).enabled(ready),
    ];
    Menu::new(items, x, y, align_right).access_keys(true)
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
        Item::new(Act::Edit(EditAct::Cut), "&Cut")
            .hint("Ctrl+X")
            .icon(glyph::CUT)
            .enabled(has_selection),
        Item::new(Act::Edit(EditAct::Copy), "Co&py")
            .hint("Ctrl+C")
            .icon(glyph::COPY)
            .enabled(has_selection),
        Item::new(Act::Edit(EditAct::Paste), "Pa&ste")
            .hint("Ctrl+V")
            .icon(glyph::PASTE),
        Item::new(Act::Edit(EditAct::SelectAll), "Select &all")
            .hint("Ctrl+A")
            .icon(glyph::SELECT_ALL)
            .enabled(has_text)
            .sep(),
        Item::new(Act::Opt(FindOpt::Case), "&Match case")
            .icon(glyph::CASE)
            .checked(options.match_case)
            .sep(),
        Item::new(Act::Opt(FindOpt::Word), "&Whole word")
            .icon(glyph::WORD)
            .checked(options.whole_word),
        Item::new(Act::Opt(FindOpt::Regex), "&Regular expression")
            .icon(glyph::REGEX)
            .checked(options.regex),
        Item::new(Act::Cmd(C::FindInFiles), "&Find in files")
            .hint("Ctrl+Shift+F")
            .icon(glyph::SEARCH)
            .enabled(has_text)
            .sep(),
        Item::new(Act::Cmd(C::FindInTabs), "Find &in open tabs")
            .icon(glyph::FIND_TABS)
            .enabled(has_text),
    ];
    Menu::new(items, x, y, false).access_keys(true)
}

// ---- dialogs ---------------------------------------------------------------------------------

const PAD: f32 = DIALOG_PAD;
const FOOTER: f32 = DIALOG_FOOTER;
const KEY_ROW: f32 = 34.0;
const RECENT_ROW: f32 = 52.0;
const MACRO_ROW: f32 = 48.0;
/// The width of a yes-or-no question's card.
const QUESTION_W: f32 = 460.0;
/// The floor of a yes-or-no question's card: the height its title, two lines of body and footer
/// need -- the question that lists no notes. One that lists notes takes a third line and grows.
const QUESTION_H: f32 = 196.0;
/// The face and size a question's body is written in: measured here, drawn below.
const BODY_FACE: Face = Face::Ui;
const BODY_SIZE: f32 = 13.5;
/// The body starts this far below the card's top edge (its title holds the rest).
const BODY_TOP: f32 = PAD + 40.0;
/// Clearance between the last line of a body and the footer's divider.
const BODY_GAP: f32 = 4.0;

/// The card for a dialog in a `w` x `h` window, sized to its list where it has one. `body_h` is
/// the height the body's words wrap to, for the library's question, which names a group and a
/// file and can be long; the other questions ignore it.
pub fn card(kind: DialogKind, rows: usize, body_h: f32, w: f32, h: f32) -> Rect {
    let list =
        |row: f32, min: f32| (PAD + 36.0 + (rows as f32 * row + 12.0).max(min) + FOOTER).round();
    let (cw, ch): (f32, f32) = match kind {
        DialogKind::About => (520.0, 256.0),
        DialogKind::Shortcuts => (580.0, list(KEY_ROW, 0.0).min(700.0)),
        DialogKind::Recent => (600.0, list(RECENT_ROW, 80.0).min(560.0)),
        DialogKind::Macros => (480.0, list(MACRO_ROW, 80.0).min(560.0)),
        DialogKind::Runs => (660.0, list(RUN_ROW, 96.0).min(620.0)),
        DialogKind::RunUndo => (520.0, 250.0),
        // Rounded up, so the card's own rounding to whole pixels cannot close the gap the
        // last line keeps from the divider. The question before a section goes names it, and a
        // section's name can be as long as a file's (audit 4, F19).
        DialogKind::LibraryDelete | DialogKind::LibraryRemove => (
            QUESTION_W,
            (BODY_TOP + body_h + BODY_GAP + FOOTER)
                .max(QUESTION_H)
                .ceil(),
        ),
        DialogKind::Unsaved | DialogKind::Overwrite | DialogKind::Discard => {
            (QUESTION_W, QUESTION_H)
        }
        DialogKind::ReplaceFiles => return crate::replace_ui::card(w, h),
    };
    dialog_card(cw, ch, w, h)
}

/// The height a question's body takes wrapped into a `w`-wide column. It is laid out at an empty
/// clip, so the words are measured by the same font that draws them and nothing is painted.
fn body_height(p: &mut Painter, w: f32, text: &str, ink: Rgb) -> f32 {
    let old = p.clip(Rect::new(0.0, 0.0, 0.0, 0.0));
    let used = p.paragraph(BODY_FACE, BODY_SIZE, 0.0, 0.0, w, text, ink);
    p.restore(old);
    used
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

#[allow(clippy::too_many_arguments)] // what each dialog needs, as `ai_ui`'s screens do
pub fn draw_dialog(
    c: &mut Ctx,
    docs: &Docs,
    macros: &[crate::macros::Named],
    runs: &mut crate::ui::RunsDialog,
    d: &mut Dialog,
    w: f32,
    h: f32,
    hover_row: Option<usize>,
) {
    let pal = *c.pal;
    c.hit(Hit::Backdrop, Rect::new(0.0, 0.0, w, h));
    draw_backdrop(c.p, &pal, w, h);
    let rows = match d.kind {
        DialogKind::About
        | DialogKind::Unsaved
        | DialogKind::Overwrite
        | DialogKind::Discard
        | DialogKind::LibraryRemove
        | DialogKind::LibraryDelete
        | DialogKind::RunUndo => 0,
        // Drawn by `replace_ui::draw_confirm`, which has the plan.
        DialogKind::ReplaceFiles => return,
        DialogKind::Shortcuts => shortcut_help().len(),
        DialogKind::Recent => docs.recents.paths().len(),
        DialogKind::Macros => macros.len(),
        DialogKind::Runs => runs.list.len() + runs.unread.len(),
    };
    // The library's questions name a group and a file, or a section, and any can be long: the
    // card is grown from the lines the words wrap to before it is drawn.
    let body_h = match d.kind {
        DialogKind::LibraryDelete | DialogKind::LibraryRemove => {
            // The width the card will get, a narrow window included, so the lines measured are
            // the lines drawn.
            let bw = dialog_card(QUESTION_W, QUESTION_H, w, h).w - 2.0 * PAD;
            body_height(c.p, bw, &d.subject, pal.text)
        }
        _ => 0.0,
    };
    let r = card(d.kind, rows, body_h, w, h);
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
        DialogKind::ReplaceFiles => {}
        DialogKind::Unsaved
        | DialogKind::Overwrite
        | DialogKind::Discard
        | DialogKind::LibraryRemove
        | DialogKind::LibraryDelete => {
            let (heading, body, buttons): (&str, String, &[&str]) = match d.kind {
                DialogKind::LibraryDelete => (
                    strings::LIBRARY_DELETE_TITLE,
                    d.subject.clone(),
                    &[strings::LIBRARY_DELETE, strings::CANCEL],
                ),
                DialogKind::Unsaved => (
                    strings::UNSAVED_TITLE,
                    strings::unsaved_body(&d.subject),
                    &[strings::SAVE, strings::DONT_SAVE, strings::CANCEL],
                ),
                DialogKind::LibraryRemove => (
                    strings::LIBRARY_REMOVE_TITLE,
                    d.subject.clone(),
                    &[strings::LIBRARY_REMOVE, strings::CANCEL],
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
            // A card capped at the window's height holds fewer lines than the words wrap to:
            // the body is cut at the divider rather than drawn into the buttons (audit 4, F19).
            let room = Rect::new(r.x, r.y, r.w, (footer.y - BODY_GAP - r.y).max(0.0));
            let old = c.p.clip(room);
            c.p.paragraph(
                BODY_FACE,
                BODY_SIZE,
                r.x + PAD,
                r.y + BODY_TOP,
                r.w - 2.0 * PAD,
                &body,
                pal.text,
            );
            c.p.restore(old);
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
        DialogKind::Runs => {
            title(c, r, "AI runs");
            let list = list_rect(r, footer);
            d.list = list;
            d.view = list.h;
            d.content = (runs.list.len() + runs.unread.len()) as f32 * RUN_ROW + 8.0;
            d.scroll = d.scroll.clamp(0.0, (d.content - d.view).max(0.0));
            if runs.list.is_empty() && runs.unread.is_empty() {
                c.p.text_center(
                    Face::Ui,
                    13.5,
                    Rect::new(list.x, list.y + 30.0, list.w, 24.0),
                    "No run has written notes yet.",
                    pal.faint,
                );
            }
            runs.sel = runs.sel.min(runs.list.len().saturating_sub(1));
            let old = c.p.clip(list);
            for (i, run) in runs.list.iter().enumerate() {
                let y = list.y + i as f32 * RUN_ROW - d.scroll;
                if y + RUN_ROW < list.y || y > list.bottom() {
                    continue;
                }
                let row = Rect::new(list.x + 8.0, y + 2.0, list.w - 16.0, RUN_ROW - 4.0);
                if i == runs.sel {
                    c.p.rbox(row, 6.0, pal.selection);
                } else if hover_row == Some(i) {
                    c.p.rbox(row, 6.0, mix(pal.popup, pal.text, 0.05));
                }
                c.hit(Hit::DlgOpen(i), row);
                c.p.icon(glyph::MACRO, 15.0, row.x + 20.0, row.cy(), pal.accent);
                let tx = row.x + 40.0;
                let tw = (row.w - 52.0).max(0.0);
                let first = match run.record.kind {
                    // "Move · the notes of Work into their folder · 5 files".
                    mdedit_core::runs::Kind::Move => mdedit_core::moves::headline(&run.record),
                    // A file rewritten in place (phase W): the files it wrote, and how many.
                    mdedit_core::runs::Kind::Rewrite => format!(
                        "Rewrite \u{00B7} {} \u{00B7} {}",
                        run.record.source_name(),
                        if run.record.originals.len() == 1 {
                            "1 file".to_string()
                        } else {
                            format!("{} files", run.record.originals.len())
                        }
                    ),
                    // Files linked into a group (File them, a folder added): no source.
                    mdedit_core::runs::Kind::Links => format!(
                        "Linked \u{00B7} {} \u{2192} {}",
                        if run.record.notes.len() == 1 {
                            "1 file".to_string()
                        } else {
                            format!("{} files", run.record.notes.len())
                        },
                        run.record.group_name(),
                    ),
                    // Files copied into a group (File them on "Copy them in", D-209).
                    mdedit_core::runs::Kind::Copies => format!(
                        "Copied \u{00B7} {} \u{2192} {}",
                        if run.record.notes.len() == 1 {
                            "1 file".to_string()
                        } else {
                            format!("{} files", run.record.notes.len())
                        },
                        run.record.group_name(),
                    ),
                    kind => {
                        let word = match kind {
                            mdedit_core::runs::Kind::Markdown => "Markdown",
                            _ => "Split",
                        };
                        format!(
                            "{} \u{00B7} {} \u{2192} {} \u{00B7} {}",
                            word,
                            run.record.source_name(),
                            run.record.group_name(),
                            if run.record.notes.len() == 1 {
                                "1 note".to_string()
                            } else {
                                format!("{} notes", run.record.notes.len())
                            }
                        )
                    }
                };
                c.p.text(Face::Ui, 13.5, tx, row.y + 15.0, &first, pal.text, tw);
                let second = format!(
                    "{} \u{00B7} {}",
                    when(&run.record.started),
                    state_label(run.record.state)
                );
                c.p.text(
                    Face::Ui,
                    11.5,
                    tx,
                    row.y + 33.0,
                    &second,
                    if i == runs.sel { pal.text } else { pal.faint },
                    tw,
                );
            }
            // The record files this version cannot use, after the ones it can: shown so that a
            // run of a later version is known to be there, with nothing in them to press.
            for (n, file) in runs.unread.iter().enumerate() {
                let y = list.y + (runs.list.len() + n) as f32 * RUN_ROW - d.scroll;
                if y + RUN_ROW < list.y || y > list.bottom() {
                    continue;
                }
                let row = Rect::new(list.x + 8.0, y + 2.0, list.w - 16.0, RUN_ROW - 4.0);
                c.p.icon(glyph::MACRO, 15.0, row.x + 20.0, row.cy(), pal.faint);
                let tx = row.x + 40.0;
                let tw = (row.w - 52.0).max(0.0);
                let name = file
                    .path
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let first = if file.newer {
                    format!("Written by a newer version of mdedit \u{00B7} {name}")
                } else {
                    format!("Not a record this version can read \u{00B7} {name}")
                };
                c.p.text(Face::Ui, 13.5, tx, row.y + 15.0, &first, pal.faint, tw);
                let second = if file.started.is_empty() {
                    "Not shown or undone here".to_string()
                } else {
                    format!("{} \u{00B7} not shown or undone here", when(&file.started))
                };
                c.p.text(Face::Ui, 11.5, tx, row.y + 33.0, &second, pal.faint, tw);
            }
            c.p.restore(old);
            dialog_scrollbar(c, d);
            let mut x = r.right() - PAD;
            let close = strings::RECENT_DONE;
            let bw = dialog_button_w(c.p, close);
            x -= bw;
            button(
                c,
                Rect::new(x, by, bw, 32.0),
                Hit::DlgButton(0),
                close,
                false,
            );
            if let Some(run) = runs.list.get(runs.sel) {
                let finish = offers_finish(&run.record);
                let undo = run.record.state.live();
                let mut items: Vec<(&str, Hit, bool)> = Vec::new();
                items.push(("Show the notes", Hit::DlgRunShow, true));
                items.push(("Undo this run\u{2026}", Hit::DlgRunUndo, undo));
                if finish {
                    items.push(("Finish", Hit::DlgRunFinish, true));
                }
                for (label, hit, enabled) in items.into_iter().rev() {
                    let w = dialog_button_w(c.p, label);
                    x -= w + 8.0;
                    if enabled {
                        button(c, Rect::new(x, by, w, 32.0), hit, label, false);
                    } else {
                        let b = Rect::new(x, by, w, 32.0);
                        let pal = *c.pal;
                        c.p.frame(b, 6.0, pal.field, pal.line);
                        c.p.text_center(Face::Ui, 12.5, b, label, pal.faint);
                    }
                }
            }
        }
        DialogKind::RunUndo => {
            title(c, r, "Undo this run?");
            let (body, can) = runs
                .undo
                .as_ref()
                .map(|(_, sentence, can)| (sentence.clone(), *can))
                .unwrap_or_default();
            c.p.paragraph(
                Face::Ui,
                13.5,
                r.x + PAD,
                r.y + PAD + 40.0,
                r.w - 2.0 * PAD,
                &body,
                pal.text,
            );
            let mut x = r.right() - PAD;
            let labels = run_undo_buttons(can);
            let count = if can { 2 } else { 1 };
            for (n, (label, primary)) in labels.iter().enumerate().take(count).rev() {
                let bw = dialog_button_w(c.p, label);
                x -= bw;
                button(
                    c,
                    Rect::new(x, by, bw, 32.0),
                    Hit::DlgButton(if can { n } else { 0 }),
                    label,
                    *primary,
                );
                x -= 8.0;
            }
            // The original is in the Recycle Bin and undo waits for it: the bin, opened (5.4).
            if runs.bin {
                let label = "Open the Recycle Bin";
                let bw = dialog_button_w(c.p, label);
                button(
                    c,
                    Rect::new(r.x + PAD, by, bw, 32.0),
                    Hit::DlgOpenBin,
                    label,
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
        DialogKind::Runs => RUN_ROW,
        _ => return None,
    };
    if !d.list.contains(x, y) {
        return None;
    }
    Some(((y - d.list.y + d.scroll) / row).floor() as usize)
}

/// The height of one row of "AI runs…": two lines.
pub const RUN_ROW: f32 = 48.0;

/// A run's time as its row says it: "today 14:03", or the day and the time.
fn when(started: &str) -> String {
    let date = started.get(..10).unwrap_or(started);
    let time = started.get(11..16).unwrap_or("");
    let today = mdedit_core::runs::utc(mdshared_ui::stale::today() * 86_400);
    if today.get(..10) == Some(date) {
        format!("today {time}")
    } else {
        format!("{date} {time}")
    }
}

/// Whether "AI runs…" offers Finish for a run: a split or a Markdown run whose links did not
/// all go in, stopped, still writing or left "links pending" (audit 3 C4, audit 4 F3). A move
/// and a rewrite have nothing to finish: they are put back or undone. A run partly undone is
/// not finished: Undo takes the rest of it out.
pub(crate) fn offers_finish(record: &mdedit_core::runs::Record) -> bool {
    matches!(
        record.kind,
        mdedit_core::runs::Kind::Split | mdedit_core::runs::Kind::Markdown
    ) && record.state.finishable()
}

/// A run's state as its row says it (5.9).
fn state_label(state: mdedit_core::runs::State) -> &'static str {
    use mdedit_core::runs::State;
    match state {
        State::Writing => "writing",
        State::LinksPending => "links pending",
        State::Done => "done",
        State::Stopped => "stopped part-way",
        State::UndoPending => "partly undone",
        State::Undone => "undone",
    }
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
    // "Open file…" leads; "New file" beside it is the quieter of the two (issue #8).
    let label = strings::EMPTY_BUTTON;
    let new_label = strings::EMPTY_NEW;
    let bw = (c.p.measure(Face::Semibold, 13.5, label) + 56.0).round();
    let nw = (c.p.measure(Face::Semibold, 13.5, new_label) + 56.0).round();
    let left = (cx - (bw + 12.0 + nw) / 2.0).round();
    let b = Rect::new(left, y, bw, 36.0);
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
    let n = Rect::new(b.right() + 12.0, y, nw, 36.0);
    let fill = match (
        c.hover == Some(Hit::EmptyNew),
        c.pressed == Some(Hit::EmptyNew),
    ) {
        (true, true) => pal.press,
        (true, false) => pal.hover,
        _ => mix(pal.page, pal.text, 0.04),
    };
    c.p.frame(n, 6.0, fill, pal.line);
    c.p.icon(glyph::NEW_FILE, 14.0, n.x + 24.0, n.cy(), pal.text);
    c.p.text(
        Face::Semibold,
        13.5,
        n.x + 38.0,
        n.cy(),
        new_label,
        pal.text,
        f32::INFINITY,
    );
    c.hit(Hit::EmptyNew, n);
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

    /// Audit 4, F3: Finish is offered for a split or Markdown run whose links are pending, as for
    /// one that stopped; never for a run partly undone, a done one, a move or a rewrite.
    #[test]
    fn finish_is_offered_for_a_run_whose_links_are_pending() {
        use mdedit_core::runs::{Group, Kind, Record, Source, State};
        use std::path::PathBuf;
        let mut record = Record {
            version: 1,
            kind: Kind::Split,
            started: String::new(),
            started_secs: 0,
            state: State::Done,
            instruction: String::new(),
            sent: None,
            source: Source {
                path: PathBuf::from("Trip.md"),
                len: 0,
                sha256: String::new(),
                recycled: false,
            },
            group: Group {
                name: "Trip".to_string(),
                file: PathBuf::from("Trip.md"),
                created: false,
                home: PathBuf::from("Trip"),
            },
            folders: Vec::new(),
            notes: Vec::new(),
            pictures: Vec::new(),
            sections_added: Vec::new(),
            moved: None,
            originals: Vec::new(),
            added: Vec::new(),
            copied: Vec::new(),
            removed: Vec::new(),
        };
        let offered = |r: &mdedit_core::runs::Record, kind: Kind, state: State| {
            let mut r = r.clone();
            r.kind = kind;
            r.state = state;
            offers_finish(&r)
        };
        for kind in [Kind::Split, Kind::Markdown] {
            for state in [State::Writing, State::LinksPending, State::Stopped] {
                assert!(offered(&record, kind, state), "{kind:?} {state:?}");
            }
            for state in [State::Done, State::UndoPending, State::Undone] {
                assert!(!offered(&record, kind, state), "{kind:?} {state:?}");
            }
        }
        for kind in [Kind::Move, Kind::Rewrite] {
            assert!(!offered(&record, kind, State::LinksPending), "{kind:?}");
        }
        record.state = State::UndoPending;
        assert_eq!(state_label(record.state), "partly undone");
    }

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
            DialogKind::LibraryRemove,
            DialogKind::LibraryDelete,
        ] {
            let r = card(kind, 30, 0.0, 520.0, 360.0);
            assert!(r.x >= 0.0 && r.y >= 0.0 && r.right() <= 520.0 && r.bottom() <= 360.0);
        }
    }

    fn painter() -> (mdshared_text::Bitmap, mdshared_ui::gfx::Gfx) {
        (
            mdshared_text::Bitmap::new(8, 8),
            mdshared_ui::gfx::Gfx::new(),
        )
    }

    /// The question before deleting a library file names a group and a file, and a long pair
    /// wraps onto more lines than the fixed 196 card holds: at the fixed 196 those lines ran
    /// through the divider into the buttons. The card is grown from the lines the words wrap to,
    /// and a short question keeps its two-line card.
    ///
    /// The names are long enough that the question overruns a 196 card in every font the three
    /// platforms measure it in: the first pair of names wrapped to three lines at macOS's
    /// metrics and to four on Windows, so this test passed here and failed in CI.
    #[test]
    fn a_long_library_question_grows_its_card_to_its_lines() {
        let (mut bm, mut gfx) = painter();
        let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
        let ink = [0x1f, 0x1f, 0x1f];
        let long = strings::library_delete_body(
            "Japan 2025, Tokyo, Kyoto, Osaka, Nara, Sapporo and the long way home, with every stop \
             written down and every ticket kept",
            "japan-2025-tokyo-kyoto-osaka-nara-sapporo-and-the-long-way-home-with-every-stop-written-down-and-every-ticket-kept.md",
            23,
        );
        let used = body_height(&mut p, QUESTION_W - 2.0 * PAD, &long, ink);
        let room = QUESTION_H - BODY_TOP - FOOTER;
        assert!(
            used > room,
            "the words need more than the 196 card's room: {used} vs {room}"
        );
        let r = card(DialogKind::LibraryDelete, 0, used, 1200.0, 900.0);
        assert!(r.h > QUESTION_H, "the card grew: {} for {used}", r.h);
        assert!(
            r.y + BODY_TOP + used + BODY_GAP <= r.bottom() - FOOTER,
            "the last line ends above the divider: {} in {}",
            used,
            r.h
        );

        let short = strings::library_delete_body("A", "a.md", 0);
        let used = body_height(&mut p, QUESTION_W - 2.0 * PAD, &short, ink);
        assert_eq!(
            card(DialogKind::LibraryDelete, 0, used, 1200.0, 900.0).h,
            QUESTION_H,
            "a question that fits two lines keeps today's card"
        );
    }

    /// Audit 4, F19: the question before a section is taken out names the section, whose name
    /// can be as long as a file's. Its card was the fixed 196 the delete question outgrew: it
    /// grows the same way, and a short one keeps its card.
    #[test]
    fn a_long_section_question_grows_its_card_to_its_lines() {
        let (mut bm, mut gfx) = painter();
        let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
        let ink = [0x1f, 0x1f, 0x1f];
        let long = strings::library_remove_body(
            "Japan 2025, Tokyo, Kyoto, Osaka, Nara, Sapporo and the long way home, with every stop \
             written down and every ticket kept, and the friends we met on the way",
            23,
        );
        let used = body_height(&mut p, QUESTION_W - 2.0 * PAD, &long, ink);
        assert!(
            used > QUESTION_H - BODY_TOP - FOOTER,
            "the words need more than the 196 card's room: {used}"
        );
        let r = card(DialogKind::LibraryRemove, 0, used, 1200.0, 900.0);
        assert!(r.h > QUESTION_H, "the card grew: {} for {used}", r.h);
        assert!(r.y + BODY_TOP + used + BODY_GAP <= r.bottom() - FOOTER);
        // A short name: never below the floor, and its lines above the divider too.
        let short = strings::library_remove_body("Money", 2);
        let used = body_height(&mut p, QUESTION_W - 2.0 * PAD, &short, ink);
        let r = card(DialogKind::LibraryRemove, 0, used, 1200.0, 900.0);
        assert!(r.h >= QUESTION_H && r.y + BODY_TOP + used + BODY_GAP <= r.bottom() - FOOTER);
    }
}

/// The widest a submenu is (its longest label plus the icon column), used to decide on which side
/// of its parent it stands before it has been measured.
pub const SUB_WIDTH: f32 = 260.0;

/// Where a submenu stands beside its parent `parent` in a window `win_w` wide: to the right of it
/// when there is room, otherwise to the left. Returns the menu's anchor x and whether that x is
/// the menu's right edge. One rule for the main menu (which hangs at the window's right edge) and
/// for a context menu anywhere in the window.
pub fn sub_side(parent: Rect, win_w: f32) -> (f32, bool) {
    if parent.right() + SUB_WIDTH + 4.0 <= win_w {
        (parent.right(), false)
    } else {
        (parent.x, true)
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;

    #[test]
    fn a_submenu_stands_beside_its_parent_on_the_side_with_room() {
        let win = 1000.0;
        // Parent near the left edge: the submenu goes to its right.
        let (x, right) = sub_side(Rect::new(10.0, 50.0, 220.0, 300.0), win);
        assert!(!right && x == 230.0);
        // In the middle: still to the right.
        let (x, right) = sub_side(Rect::new(400.0, 50.0, 220.0, 300.0), win);
        assert!(!right && x == 620.0);
        // Hugging the right edge (the main menu, or a context menu there): to the left,
        // its right edge at the parent's left edge.
        let (x, right) = sub_side(Rect::new(770.0, 50.0, 220.0, 300.0), win);
        assert!(right && x == 770.0);
        // Neither side overlaps the parent.
        for px in [4.0, 100.0, 300.0, 500.0, 700.0, 776.0] {
            let parent = Rect::new(px, 0.0, 220.0, 100.0);
            let (x, right) = sub_side(parent, win);
            if right {
                assert!(x - SUB_WIDTH >= 0.0 && x == parent.x);
            } else {
                assert!(x == parent.right() && x + SUB_WIDTH <= win);
            }
        }
    }
}
