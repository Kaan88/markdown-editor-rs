//! Issues #7 and #8: the Preview button's arrow and its default mode, New file on the empty
//! screen, plain-text paste on Ctrl+Shift+V, the New, Save and AI edit buttons, and rich text
//! pasted as Markdown.

use super::commands::{Clip, preview_command};
use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::commands::CommandId as C;
use crate::ui::{Act, BarItem, PopupKind};
use mdedit_core::ai::Connection;
use mdedit_core::prefs::PreviewMode;
use mdshared_ui::menu::Menu;

fn text(a: &App) -> String {
    let r = a.docs.active_ready().unwrap();
    let mut s = String::new();
    r.buffer.text(0..r.buffer.len_bytes(), &mut s);
    // A new file takes the platform's line ending; the tests read the lines.
    s.replace("\r\n", "\n")
}

fn md_doc(tag: &str, body: &str) -> (App, PathBuf) {
    let dir = scratch(tag);
    let md = dir.join("a.md");
    std::fs::write(&md, body).unwrap();
    let mut a = app();
    open_now(&mut a, &md);
    a.compose();
    (a, dir)
}

fn commands_in(m: &Menu<Act>) -> Vec<C> {
    m.items
        .iter()
        .filter_map(|it| match it.id {
            Act::Cmd(c) | Act::TabCmd(c, _) => Some(c),
            _ => None,
        })
        .collect()
}

// ---- #7 ----------------------------------------------------------------------------------------

#[test]
fn the_arrow_opens_each_mode_once_and_chooses_the_buttons_own_as_a_radio() {
    let (mut a, dir) = md_doc("arrow", "# T\n");
    assert!(a.hit_rect(Hit::Bar(BarItem::Preview)).is_some());
    a.activate(Hit::Bar(BarItem::PreviewMenu));
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::Preview));
    let menu = &a.ui.popup.as_ref().unwrap().menu;
    // The three modes, as actions: no tick on any.
    assert_eq!(
        commands_in(menu),
        [C::PreviewPane, C::PreviewInViewer, C::PreviewSideBySide]
    );
    assert!(menu.items.iter().all(|it| !it.checked && !it.checkable));
    // The button's own, a radio: the pane by default.
    let radio: Vec<(PreviewMode, bool)> = menu
        .items
        .iter()
        .filter(|it| it.enabled)
        .filter_map(|it| match it.id {
            Act::PreviewDefault(m) => Some((m, it.icon == crate::chrome::RADIO_ON)),
            _ => None,
        })
        .collect();
    assert_eq!(
        radio,
        [
            (PreviewMode::Window, true),
            (PreviewMode::Viewer, false),
            (PreviewMode::SideBySide, false)
        ]
    );
    // Choosing one keeps it and closes the menu; it opens nothing.
    a.choose(Act::PreviewDefault(PreviewMode::Viewer), false);
    assert!(a.ui.popup.is_none());
    assert_eq!(a.docs.prefs.preview_mode, PreviewMode::Viewer);
    assert!(a.session.serialize().contains("preview_mode=mdview"));
    assert!(a.docs.links.is_empty() && a.preview.is_none());
    // The button now opens mdview: its command and its tooltip say so.
    assert_eq!(
        preview_command(crate::chrome::preview_mode(&a.docs)),
        C::PreviewInViewer
    );
    let tip = crate::chrome::tooltip(Hit::Bar(BarItem::Preview), &a.docs, false).unwrap();
    assert!(tip.starts_with("Preview in mdview"), "{tip}");
    // Without mdview the button falls back to the pane, and the arrow has nothing to choose.
    a.docs.viewer = false;
    assert_eq!(crate::chrome::preview_mode(&a.docs), PreviewMode::Window);
    let m = crate::overlays::preview_menu(&a.docs, 0.0, 0.0, false);
    assert_eq!(commands_in(&m), [C::PreviewPane]);
    assert!(
        !m.items
            .iter()
            .any(|it| matches!(it.id, Act::PreviewDefault(_)))
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_view_menu_has_no_preview_switches_and_the_tab_menu_says_what_a_click_does() {
    let (mut a, dir) = md_doc("switches", "# T\n");
    let view = crate::overlays::view_menu(&a.docs, &a.ui, 0.0, 0.0, false);
    let previews = [C::PreviewPane, C::PreviewInViewer, C::PreviewSideBySide];
    assert!(!commands_in(&view).iter().any(|c| previews.contains(c)));
    let label = |a: &App, id: C| {
        crate::overlays::tab_menu(&a.docs, 0, 0.0, 0.0)
            .items
            .into_iter()
            .find(|it| it.id == Act::TabCmd(id, 0))
            .map(|it| (it.label, it.checkable))
            .unwrap()
    };
    assert_eq!(
        label(&a, C::PreviewPane),
        ("Preview in t&his window".into(), false)
    );
    // Open, it offers to close: still no tick.
    a.docs.pane_open = true;
    assert_eq!(
        label(&a, C::PreviewPane),
        ("Close t&he preview".into(), false)
    );
    let m = crate::overlays::preview_menu(&a.docs, 0.0, 0.0, false);
    assert_eq!(m.items[0].label, "&Close the preview");
    let _ = std::fs::remove_dir_all(dir);
}

// ---- #8: New file on the empty screen ----------------------------------------------------------

#[test]
fn new_file_on_the_empty_screen_puts_the_caret_in_a_new_tab() {
    let mut a = app();
    a.compose();
    assert!(a.docs.tabs.is_empty());
    assert!(a.hit_rect(Hit::EmptyOpen).is_some());
    let new = a
        .hit_rect(Hit::EmptyNew)
        .expect("New file beside Open file");
    let open = a.hit_rect(Hit::EmptyOpen).unwrap();
    assert!(new.x > open.right() && (new.y - open.y).abs() < 0.5);
    a.ui.focus_field(crate::ui::BarField::Find);
    a.activate(Hit::EmptyNew);
    assert_eq!(a.docs.tabs.len(), 1);
    let doc = a.docs.active_doc().unwrap();
    assert!(doc.path.is_none() && doc.ready().is_some());
    // The keyboard is in the new document: typing goes into it.
    assert!(!a.ui.find_focused);
    super::files_tests::type_text(&mut a, "hi");
    assert_eq!(text(&a), "hi");
}

// ---- #8: plain-text paste ----------------------------------------------------------------------

#[test]
fn ctrl_shift_v_pastes_plain_text_and_preview_in_mdview_moved_to_ctrl_shift_l() {
    use crate::keys::{Key, KeyName, Mods, command_for};
    let both = Mods {
        ctrl: true,
        shift: true,
        ..Mods::default()
    };
    let key = |c| Key::Name(KeyName::Char(c));
    assert_eq!(command_for(&key('v'), both), Some(C::PastePlain));
    assert_eq!(command_for(&key('l'), both), Some(C::PreviewInViewer));
    // As this platform prints it: "Ctrl+Shift+L", or the Mac's symbols for Shift+Command+L.
    let label = |k| mdshared_ui::keys::label(mdshared_ui::keys::for_platform(k));
    assert_eq!(
        crate::commands::hint(C::PreviewInViewer),
        label("Ctrl+Shift+L")
    );
    assert_eq!(crate::commands::hint(C::PastePlain), label("Ctrl+Shift+V"));
    // In a saved Markdown file the path of a picture is a link with Ctrl+V, its text with
    // Ctrl+Shift+V.
    let (mut a, dir) = md_doc("plain", "");
    let pic = dir.join("pic.png");
    std::fs::write(&pic, b"\x89PNG\r\n\x1a\n").unwrap();
    let path = pic.display().to_string();
    a.paste_clip(Clip::text(path.clone()), true);
    assert_eq!(text(&a), path);
    a.command(C::SelectAll);
    a.paste_clip(Clip::text(path.clone()), false);
    assert_eq!(text(&a), "![](pic.png)");
    // Rich text too: plain paste never converts it, whatever the setting.
    a.docs.prefs.paste_rich = true;
    a.command(C::SelectAll);
    let rich = Clip {
        text: "Bold".into(),
        html: Some("<b>Bold</b>".into()),
    };
    a.paste_clip(rich, true);
    assert_eq!(text(&a), "Bold");
    let _ = std::fs::remove_dir_all(dir);
}

// ---- #8: rich text as Markdown -----------------------------------------------------------------

#[test]
fn rich_text_is_pasted_as_markdown_only_when_the_setting_is_on_in_markdown() {
    let clip = || Clip {
        text: "Title\nBold and link".into(),
        html: Some(
            r#"<h2>Title</h2><p><b>Bold</b> and <a href="https://x.org">link</a></p>"#.into(),
        ),
    };
    let (mut a, dir) = md_doc("rich", "");
    assert!(!a.docs.prefs.paste_rich, "off by default");
    a.paste_clip(clip(), false);
    assert_eq!(text(&a), "Title\nBold and link");
    a.docs.prefs.paste_rich = true;
    a.command(C::SelectAll);
    a.paste_clip(clip(), false);
    assert_eq!(text(&a), "## Title\n\n**Bold** and [link](https://x.org)");
    // HTML with nothing to mark: the plain text.
    a.command(C::SelectAll);
    a.paste_clip(
        Clip {
            text: "just text".into(),
            html: Some("<span>just text</span>".into()),
        },
        false,
    );
    assert_eq!(text(&a), "just text");
    // A file that is not Markdown takes the plain text.
    let txt = dir.join("b.txt");
    std::fs::write(&txt, "").unwrap();
    open_now(&mut a, &txt);
    a.paste_clip(clip(), false);
    assert_eq!(text(&a), "Title\nBold and link");
    // A new document has no kind yet: a note begun there gets the Markdown.
    a.command(C::NewFile);
    a.paste_clip(clip(), false);
    assert!(text(&a).starts_with("## Title"), "{}", text(&a));
    // One whose language was chosen, other than Markdown, takes the plain text as a saved file
    // of that language does.
    a.command(C::NewFile);
    choose_language(&mut a, "Python");
    assert!(a.docs.active_ready().unwrap().language_set);
    a.paste_clip(clip(), false);
    assert_eq!(text(&a), "Title\nBold and link");
    // Chosen as Markdown, it gets the Markdown.
    a.command(C::NewFile);
    choose_language(&mut a, "Markdown");
    assert!(a.docs.active_doc().unwrap().is_markdown());
    a.paste_clip(clip(), false);
    assert!(text(&a).starts_with("## Title"), "{}", text(&a));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn ctrl_shift_v_and_paste_as_plain_text_put_the_clipboard_in_as_it_is() {
    let (mut a, dir) = md_doc("plain-cmd", "");
    a.docs.prefs.paste_rich = true;
    let pic = dir.join("pic.png");
    std::fs::write(&pic, b"\x89PNG\r\n\x1a\n").unwrap();
    let path = pic.display().to_string();
    // The path of a picture: Ctrl+V links it, Ctrl+Shift+V puts the path in.
    super::commands::clipboard::set(Clip::text(path.clone()));
    a.command(C::Paste);
    assert_eq!(text(&a), "![](pic.png)");
    a.command(C::SelectAll);
    a.command(C::PastePlain);
    assert_eq!(text(&a), path);
    // Rich text: Ctrl+V converts it with the setting on, Ctrl+Shift+V never does.
    let rich = || Clip {
        text: "Bold".into(),
        html: Some("<h2>T</h2><b>Bold</b>".into()),
    };
    super::commands::clipboard::set(rich());
    a.command(C::SelectAll);
    a.command(C::Paste);
    assert!(text(&a).starts_with("## T"), "{}", text(&a));
    a.command(C::SelectAll);
    a.command(C::PastePlain);
    assert_eq!(text(&a), "Bold");
    // The text menu's "Paste as plain text" is the same command.
    super::commands::clipboard::set(Clip {
        text: "Plain".into(),
        html: Some("<h2>Rich</h2>".into()),
    });
    a.command(C::SelectAll);
    let menu = crate::overlays::text_menu(&a.docs, 0.0, 0.0);
    let item = menu
        .items
        .iter()
        .find(|it| it.id == Act::Cmd(C::PastePlain))
        .expect("Paste as plain text in the text menu");
    assert!(item.enabled && item.label.contains("plain"));
    a.choose(Act::Cmd(C::PastePlain), false);
    assert_eq!(text(&a), "Plain");
    let _ = std::fs::remove_dir_all(dir);
}

/// Chooses the active document's language from the status bar's field, as the user does.
fn choose_language(a: &mut App, name: &str) {
    use crate::keys::{Key, KeyName};
    a.compose();
    a.activate(Hit::StatusLanguage);
    assert!(a.palette_open());
    super::files_tests::type_text(a, name);
    a.key_pressed(Key::Name(KeyName::Enter), None);
    assert!(!a.palette_open());
}

// ---- #8: New, Save and AI edit on the bar ------------------------------------------------------

#[test]
fn the_bar_has_new_save_and_ai_edit_with_the_menus_availability() {
    let (mut a, dir) = md_doc("bar", "one\n");
    // Save only with something to save; AI edit only while the AI is on.
    assert!(a.hit_rect(Hit::Bar(BarItem::New)).is_some());
    assert!(a.hit_rect(Hit::Bar(BarItem::Save)).is_none());
    assert!(a.hit_rect(Hit::Bar(BarItem::AiEdit)).is_none());
    super::files_tests::type_text(&mut a, "x");
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.compose();
    assert!(a.hit_rect(Hit::Bar(BarItem::Save)).is_some());
    let ai = a.hit_rect(Hit::Bar(BarItem::AiEdit)).expect("AI edit");
    assert!(ai.x > a.hit_rect(Hit::Bar(BarItem::Preview)).unwrap().right());
    // Save writes the file, as Ctrl+S does.
    a.activate(Hit::Bar(BarItem::Save));
    assert_eq!(std::fs::read_to_string(dir.join("a.md")).unwrap(), "xone\n");
    assert!(!a.docs.active_doc().unwrap().modified());
    // AI edit opens the AI menu under itself.
    a.activate(Hit::Bar(BarItem::AiEdit));
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::Ai));
    a.compose();
    let menu = a.ui.popup.as_ref().unwrap().menu.rect();
    assert!(
        (menu.x - ai.x).abs() < 1.0 && menu.y >= ai.bottom(),
        "{menu:?} {ai:?} {:?}",
        a.size
    );
    assert!(commands_in(&a.ui.popup.as_ref().unwrap().menu).contains(&C::AiRewrite));
    a.ui.popup = None;
    // New makes an untitled tab, as Ctrl+N.
    a.activate(Hit::Bar(BarItem::New));
    assert_eq!(a.docs.tabs.len(), 2);
    assert!(a.docs.active_doc().unwrap().path.is_none());
    // Narrow: the Files group, AI edit with it, goes; New, Open and Save stay.
    a.size = (560, 600);
    a.compose();
    for item in [BarItem::New, BarItem::Open] {
        assert!(a.hit_rect(Hit::Bar(item)).is_some(), "{item:?}");
    }
    assert!(a.hit_rect(Hit::Bar(BarItem::AiEdit)).is_none());
    assert!(a.hit_rect(Hit::Bar(BarItem::Files)).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

// ---- the settings ------------------------------------------------------------------------------

#[test]
fn the_settings_rows_set_the_preview_mode_and_the_rich_paste() {
    use super::modal::{PASTE_RICH, PREVIEW_MODE};
    use mdshared_ui::settings::Value;
    let mut a = app();
    a.open_settings();
    let f = a.ui.settings.as_mut().unwrap();
    assert!(f.focus_row(PREVIEW_MODE) && f.focus_row(PASTE_RICH));
    a.apply_setting(PREVIEW_MODE, Value::Choice(2));
    a.apply_setting(PASTE_RICH, Value::Bool(true));
    assert_eq!(a.session.prefs.preview_mode, PreviewMode::SideBySide);
    assert!(a.session.prefs.paste_rich);
    let s = a.session.serialize();
    assert!(s.contains("preview_mode=side-by-side") && s.contains("paste_rich=1"));
}

// ---- review of issues #7 and #8 ----------------------------------------------------------------

/// The bar is drawn again only when its signature changes: an edit alone lights Save, and a save
/// alone puts it out.
#[test]
fn save_on_the_bar_follows_an_edit_and_a_save_with_nothing_else_changing() {
    let (mut a, dir) = md_doc("bar-sig", "one\n");
    assert!(a.hit_rect(Hit::Bar(BarItem::Save)).is_none());
    super::files_tests::type_text(&mut a, "x");
    a.compose();
    assert!(a.hit_rect(Hit::Bar(BarItem::Save)).is_some());
    a.command(C::Save);
    assert!(!a.docs.active_doc().unwrap().modified());
    a.compose();
    assert!(a.hit_rect(Hit::Bar(BarItem::Save)).is_none());
    let _ = std::fs::remove_dir_all(dir);
}

/// The Preview button is lit when a click on it closes or stops the preview of its mode, and
/// its tooltip and its command say the same, in every mode.
#[test]
fn the_preview_button_shows_what_a_click_does_in_every_mode() {
    use crate::chrome::{button_mode, preview_button, tooltip};
    let (mut a, dir) = md_doc("button-modes", "# T\n");
    let id = a.docs.active_doc().unwrap().id;
    let path = a.docs.active_doc().unwrap().path.clone().unwrap();
    let tip = |a: &App| tooltip(Hit::Bar(BarItem::Preview), &a.docs, false).unwrap();
    let link = |a: &mut App, side: bool| {
        let target = crate::link::Target::File(path.clone());
        a.docs.links.link(id, target, Some(1), None, Instant::now());
        a.docs.links.side_by_side = side;
    };
    // In this window.
    assert_eq!(preview_button(&a.docs), Some(false));
    assert_eq!(preview_command(button_mode(&a.docs)), C::PreviewPane);
    assert!(tip(&a).starts_with("Preview in this window"), "{}", tip(&a));
    a.docs.pane_open = true;
    assert_eq!(preview_button(&a.docs), Some(true));
    assert_eq!(tip(&a), "Close the preview");
    a.docs.pane_open = false;
    // In mdview: lit while the document is previewed there, side by side or not, since a click
    // stops either.
    a.docs.prefs.preview_mode = PreviewMode::Viewer;
    assert_eq!(preview_button(&a.docs), Some(false));
    assert_eq!(preview_command(button_mode(&a.docs)), C::PreviewInViewer);
    assert!(tip(&a).starts_with("Preview in mdview"), "{}", tip(&a));
    for side in [false, true] {
        link(&mut a, side);
        assert_eq!(preview_button(&a.docs), Some(true), "side by side {side}");
        assert!(
            tip(&a).starts_with("Stop the preview in mdview"),
            "{}",
            tip(&a)
        );
        a.activate(Hit::Bar(BarItem::Preview));
        assert!(a.docs.links.is_empty(), "the click stopped it");
        assert_eq!(preview_button(&a.docs), Some(false));
    }
    // Side by side: lit while the windows are arranged so, which a click gives up; previewed
    // in mdview but not arranged, a click arranges them.
    a.docs.prefs.preview_mode = PreviewMode::SideBySide;
    assert_eq!(preview_command(button_mode(&a.docs)), C::PreviewSideBySide);
    assert_eq!(preview_button(&a.docs), Some(false));
    assert_eq!(tip(&a), "Preview side by side");
    link(&mut a, false);
    assert_eq!(preview_button(&a.docs), Some(false));
    assert_eq!(tip(&a), "Preview side by side");
    link(&mut a, true);
    assert_eq!(preview_button(&a.docs), Some(true));
    assert_eq!(tip(&a), "Stop side by side");
    // The arrow's labels agree with the button.
    let m = crate::overlays::preview_menu(&a.docs, 0.0, 0.0, false);
    let label = |c: C| {
        m.items
            .iter()
            .find(|it| it.id == Act::Cmd(c))
            .map(|it| it.label.clone())
            .unwrap()
    };
    assert_eq!(label(C::PreviewInViewer), "&Stop the preview in mdview");
    assert_eq!(label(C::PreviewSideBySide), "Stop side &by side");
    let _ = std::fs::remove_dir_all(dir);
}

/// Side by side, the button is lit only while the windows are still arranged as a click finds
/// them (`App::side_by_side`): a window moved since, or never placed, is not, and the next frame
/// shows the button, its tooltip and the menus' mark off before anything is clicked.
#[test]
fn the_side_by_side_button_is_lit_only_while_a_click_would_give_the_arrangement_up() {
    use crate::chrome::{preview_button, tooltip};
    let (mut a, dir) = md_doc(
        "button-tiling",
        "# T
",
    );
    let id = a.docs.active_doc().unwrap().id;
    let path = a.docs.active_doc().unwrap().path.clone().unwrap();
    a.docs.prefs.preview_mode = PreviewMode::SideBySide;
    a.docs.viewer = true;
    let target = crate::link::Target::File(path);
    a.docs.links.link(id, target, Some(1), None, Instant::now());
    // Marked side by side, but this window is not where an arrangement put it (here: it has
    // none, as after the user moved it): a click would arrange the windows, not stop.
    a.docs.links.side_by_side = true;
    a.compose();
    assert_eq!(preview_button(&a.docs), Some(false));
    let tip = tooltip(Hit::Bar(BarItem::Preview), &a.docs, false).unwrap();
    assert_eq!(tip, "Preview side by side");
    let m = crate::overlays::preview_menu(&a.docs, 0.0, 0.0, false);
    let side = m
        .items
        .iter()
        .find(|it| it.id == Act::Cmd(C::PreviewSideBySide));
    assert!(!side.unwrap().checked);
    // As a click finds it; and the preview itself goes on.
    assert!(!a.side_by_side());
    assert!(a.docs.links.contains(id));
    let _ = std::fs::remove_dir_all(dir);
}

/// A file that is not Markdown is not offered the preview in mdview: with the pane open (from a
/// Markdown tab) the button only closes the pane, and the arrow lists the pane alone, as the
/// tab menu lists nothing. Previewed in mdview already, it can be stopped.
#[test]
fn a_file_that_is_not_markdown_is_offered_only_the_pane_it_can_close() {
    use crate::chrome::{button_mode, preview_button};
    let (mut a, dir) = md_doc("not-md", "# T\n");
    let rs = dir.join("b.rs");
    std::fs::write(&rs, "fn b() {}\n").unwrap();
    a.docs.prefs.preview_mode = PreviewMode::Viewer;
    a.docs.pane_open = true;
    open_now(&mut a, &rs);
    assert!(!a.docs.active_doc().unwrap().is_markdown());
    assert_eq!(preview_button(&a.docs), Some(true));
    assert_eq!(button_mode(&a.docs), PreviewMode::Window);
    assert_eq!(preview_command(button_mode(&a.docs)), C::PreviewPane);
    let tip = crate::chrome::tooltip(Hit::Bar(BarItem::Preview), &a.docs, false).unwrap();
    assert_eq!(tip, "Close the preview");
    let m = crate::overlays::preview_menu(&a.docs, 0.0, 0.0, false);
    assert_eq!(commands_in(&m), [C::PreviewPane]);
    // The button's own choice is a preference, and stays on offer.
    assert!(
        m.items
            .iter()
            .any(|it| it.id == Act::PreviewDefault(PreviewMode::Viewer))
    );
    let tab = a.docs.active().unwrap();
    let tab_menu = crate::overlays::tab_menu(&a.docs, tab, 0.0, 0.0);
    assert!(
        !commands_in(&tab_menu)
            .iter()
            .any(|c| [C::PreviewInViewer, C::PreviewSideBySide].contains(c))
    );
    // Linked, it is offered the way to stop it.
    let id = a.docs.active_doc().unwrap().id;
    let target = crate::link::Target::File(rs.clone());
    a.docs.links.link(id, target, Some(1), None, Instant::now());
    assert_eq!(preview_button(&a.docs), Some(true));
    assert_eq!(button_mode(&a.docs), PreviewMode::Viewer);
    let m = crate::overlays::preview_menu(&a.docs, 0.0, 0.0, false);
    assert!(commands_in(&m).contains(&C::PreviewInViewer));
    // Closed pane, not linked: no button.
    a.docs.links.unlink(id);
    a.docs.pane_open = false;
    assert_eq!(preview_button(&a.docs), None);
    let _ = std::fs::remove_dir_all(dir);
}

/// The empty screen's hint is one line in the narrowest text area: a 520 px window, the
/// smallest, with the Files pane open.
#[test]
fn the_empty_screen_hint_fits_the_narrowest_text_area() {
    use mdshared_ui::gfx::{Face, Gfx, Painter};
    let areas = crate::ui::Areas::compute(520.0, 400.0, true, 250.0, 0.0);
    assert!(areas.files.is_some());
    assert_eq!(areas.text.w, crate::ui::PAGE_MIN);
    let mut bm = Bitmap::new(64, 64);
    let mut gfx = Gfx::new();
    let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
    let w = p.measure(Face::Ui, 13.5, crate::commands::strings::EMPTY_HINT);
    // As overlays::draw_empty gives it: 16 px each side. The Windows UI font is measured; other
    // systems fall back to fonts the runners may not match.
    if cfg!(windows) {
        assert!(w <= areas.text.w - 32.0, "the hint needs {w} px");
    }
    assert!(w > 0.0);
}
