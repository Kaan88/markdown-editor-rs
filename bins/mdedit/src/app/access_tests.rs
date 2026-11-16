//! Headless tests of the menus' access keys: the marks are unique within a menu, a letter picks
//! the marked item as Enter would, and the underlines follow how the menu was opened.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use crate::ui::PopupKind;
use mdedit_core::find::FindOptions;
use mdshared_ui::menu::{Menu, access_key_of, parse_access};

fn press(a: &mut App, key: KeyName, typed: Option<&str>) {
    a.key_pressed(Key::Name(key), typed);
}

fn letter(a: &mut App, ch: char) {
    let s = ch.to_string();
    press(a, KeyName::Char(ch), Some(&s));
}

/// The keys of a menu's items; no two items may share one.
fn keys_of(menu: &Menu<Act>) -> Vec<Option<char>> {
    let keys: Vec<Option<char>> = menu
        .items
        .iter()
        .map(|i| access_key_of(&i.label).map(|c| c.to_lowercase().next().unwrap_or(c)))
        .collect();
    let mut seen = std::collections::HashSet::new();
    for (k, item) in keys.iter().zip(&menu.items) {
        if let Some(k) = k {
            assert!(
                seen.insert(*k),
                "two items with the key {k}: {}",
                item.label
            );
        }
    }
    keys
}

/// Every item of a menu with fixed labels has a key, each its own.
fn assert_all_marked(name: &str, menu: &Menu<Act>) {
    for (k, item) in keys_of(menu).iter().zip(&menu.items) {
        assert!(k.is_some(), "{name}: no key on {:?}", item.label);
    }
}

fn open_app(tag: &str) -> (App, std::path::PathBuf) {
    let dir = scratch(tag);
    std::fs::write(dir.join("a.md"), "# one\n\ntext\n").unwrap();
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    open_now(&mut a, &dir.join("a.md"));
    (a, dir)
}

#[test]
fn every_fixed_menu_marks_each_item_with_its_own_letter() {
    let (a, dir) = open_app("acc-marks");
    let docs = &a.docs;
    assert_all_marked("main", &crate::overlays::main_menu(docs, 0.0, 0.0));
    assert_all_marked(
        "view",
        &crate::overlays::view_menu(docs, &a.ui, 0.0, 0.0, false),
    );
    assert_all_marked("tab", &crate::overlays::tab_menu(docs, 0, 0.0, 0.0));
    assert_all_marked("text", &crate::overlays::text_menu(docs, 0.0, 0.0));
    assert_all_marked("lines", &crate::overlays::lines_menu(docs, 0.0, 0.0, false));
    assert_all_marked(
        "markdown",
        &crate::overlays::markdown_menu(docs, 0.0, 0.0, false),
    );
    assert_all_marked(
        "convert",
        &crate::overlays::convert_menu(docs, 0.0, 0.0, false),
    );
    assert_all_marked("minimap", &crate::overlays::minimap_menu(0.0, 0.0));
    assert_all_marked(
        "find",
        &crate::overlays::find_menu(true, true, FindOptions::default(), 0.0, 0.0),
    );
    for recording in [false, true] {
        let saved = vec!["a & b".to_string(), "Fix".to_string()];
        let m = crate::overlays::macro_menu(recording, true, &saved, 0.0, 0.0, false);
        let keys = keys_of(&m);
        // The fixed items have keys; the saved macros are data and have none, and the "&" in a
        // name is a letter to draw.
        let fixed = m.items.len() - saved.len() - 1;
        assert!(keys[..fixed].iter().all(Option::is_some));
        assert!(keys[fixed..fixed + saved.len()].iter().all(Option::is_none));
        assert!(
            m.items
                .iter()
                .any(|i| parse_access(&i.label).text == "a & b")
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_recent_menu_shows_a_file_name_with_an_ampersand_as_it_is() {
    let (mut a, dir) = open_app("acc-recent");
    let path = dir.join("R&D notes.md");
    std::fs::write(&path, "x").unwrap();
    a.docs.recents.add(&path);
    let mut m = crate::overlays::recent_menu(&a.docs, 0.0, 0.0, false);
    assert!(
        m.items
            .iter()
            .any(|i| parse_access(&i.label).text.starts_with("R&D notes")),
        "{:?}",
        m.items.iter().map(|i| &i.label).collect::<Vec<_>>()
    );
    // The file is not reachable by a letter; the two fixed items are.
    assert_eq!(m.access_key('r'), None);
    assert_eq!(m.access_key('m'), Some(Act::Cmd(C::ManageRecent)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_letter_picks_the_marked_item_as_enter_would() {
    let (mut a, dir) = open_app("acc-pick");
    a.open_popup(PopupKind::Main);
    // "&View" opens its submenu, which has the keyboard.
    letter(&mut a, 'v');
    let p = a.ui.popup.as_ref().expect("the menus stay up");
    assert_eq!(p.sub.as_ref().map(|s| s.kind), Some(PopupKind::View));
    // In it, "&Word wrap" runs and everything closes; the capital letter does the same.
    let wrapped = a.docs.wrap;
    letter(&mut a, 'W');
    assert!(a.ui.popup.is_none());
    assert_ne!(a.docs.wrap, wrapped);
    // A letter no item has leaves the menu as it was.
    a.open_popup(PopupKind::Main);
    letter(&mut a, '1');
    assert!(a.ui.popup.is_some());
    // A disabled item is not picked ("Save" is off: nothing is modified).
    letter(&mut a, 's');
    assert!(a.ui.popup.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_letter_with_ctrl_or_alt_does_nothing_in_a_menu() {
    let (mut a, dir) = open_app("acc-mods");
    for mods in [
        Mods {
            ctrl: true,
            ..Mods::default()
        },
        Mods {
            alt: true,
            ..Mods::default()
        },
    ] {
        a.open_popup(PopupKind::Main);
        a.mods = mods;
        letter(&mut a, 'v');
        a.mods = Mods::default();
        let p = a.ui.popup.as_ref().expect("still the main menu");
        assert!(p.sub.is_none());
        a.ui.popup = None;
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_completion_list_does_not_read_access_keys_and_typing_goes_on() {
    let (mut a, dir) = open_app("acc-complete");
    std::fs::write(dir.join("w.txt"), "parser parameter\npar").unwrap();
    open_now(&mut a, &dir.join("w.txt"));
    a.areas.text = Rect::new(0.0, 80.0, 1280.0, 600.0);
    a.areas.title = Rect::new(0.0, 0.0, 1280.0, 40.0);
    let r = a.docs.active_ready_mut().unwrap();
    let end = r.buffer.len_bytes();
    r.view.sel = mdedit_core::Selection::at(end);
    let chord = mdshared_ui::keys::parse(mdshared_ui::keys::for_platform("Ctrl+Space")).unwrap();
    a.mods = chord.mods;
    press(&mut a, chord.key, None);
    a.mods = Mods::default();
    assert!(a.completion_showing());
    assert_eq!(a.ui.popup.as_mut().unwrap().menu.access_key('p'), None);
    // 's' narrows the list and is typed into the text, as before.
    letter(&mut a, 's');
    assert!(a.completion_showing());
    let r = a.docs.active_ready().unwrap();
    let mut t = String::new();
    r.buffer.text(0..r.buffer.len_bytes(), &mut t);
    assert!(t.ends_with("pars"), "{t:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn underlines_show_when_the_menu_was_opened_by_keyboard_and_not_by_mouse() {
    let (mut a, dir) = open_app("acc-cues");
    // Before any input, and after a key: shown.
    a.open_popup(PopupKind::Main);
    assert!(a.ui.popup.as_ref().unwrap().cues);
    a.ui.popup = None;
    // After a mouse press: hidden, until a key is used in the menu.
    a.ui.keyboard = false;
    a.open_popup(PopupKind::Main);
    assert!(!a.ui.popup.as_ref().unwrap().cues);
    press(&mut a, KeyName::Down, None);
    assert!(a.ui.popup.as_ref().unwrap().cues);
    assert!(a.ui.keyboard);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Right-clicks at the middle of `hit`'s rectangle, as the mouse does (it clears the keyboard
/// flag first).
fn right_click(a: &mut App, hit: Hit) {
    a.compose();
    let r = a.hit_rect(hit).expect("the control is on screen");
    a.ui.mouse = (r.cx(), r.cy());
    a.right_press();
}

fn cues_of(a: &App) -> bool {
    a.ui.popup.as_ref().expect("a menu is open").cues
}

#[test]
fn menus_opened_by_a_right_click_hide_the_underlines_until_a_key() {
    let (mut a, dir) = open_app("acc-right");
    a.docs.minimap = true;
    for hit in [Hit::Tab(0), Hit::Text, Hit::Minimap, Hit::Find] {
        a.ui.popup = None;
        a.ui.keyboard = true;
        right_click(&mut a, hit);
        assert!(a.ui.popup.is_some(), "{hit:?} opened no menu");
        assert!(!cues_of(&a), "{hit:?}: underlines show after a mouse click");
        assert!(!a.ui.keyboard);
        // A key used in the open menu shows them.
        press(&mut a, KeyName::Down, None);
        assert!(cues_of(&a), "{hit:?}: still hidden after a key");
    }
    // The text menu's submenus follow: hidden when the pointer opens them, shown by keyboard.
    a.ui.popup = None;
    right_click(&mut a, Hit::Text);
    a.open_sub(PopupKind::Text, PopupKind::Lines, false);
    assert!(!a.ui.popup.as_ref().unwrap().sub.as_ref().unwrap().cues);
    press(&mut a, KeyName::Down, None);
    assert!(a.ui.popup.as_ref().unwrap().sub.as_ref().unwrap().cues);
    let _ = std::fs::remove_dir_all(&dir);
}
