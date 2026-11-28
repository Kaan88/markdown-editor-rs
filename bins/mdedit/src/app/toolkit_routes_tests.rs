//! The toolkit's own screens on their pointer routes (toolkit `84c0fc8`): the command palette's
//! query box and the plan screen's rename take the whole of a text field's mouse (the run of
//! clicks, the drag past an end, the edit menu), and a press the window lost acts on none of
//! them, nor on Settings.

use super::files_tests::{app, open_now, scratch, type_text};
use super::*;
use crate::keys::{Key, KeyName};

fn key(a: &mut App, k: KeyName) {
    a.key_pressed(Key::Name(k), None);
}

fn shift_f10(a: &mut App) {
    a.mods.shift = true;
    key(a, KeyName::F(10));
    a.mods.shift = false;
}

fn click_at(a: &mut App, x: f32, y: f32) {
    a.pointer_moved(x, y);
    a.left_press();
    a.left_release();
}

/// The palette open on the commands, with `query` typed after the `>`, laid out.
fn palette_with(query: &str) -> App {
    let mut a = app();
    a.size = (900, 600);
    a.compose();
    a.open_palette(">");
    type_text(&mut a, query);
    a.compose();
    a
}

fn query(a: &App) -> String {
    a.palette
        .open
        .as_ref()
        .expect("the palette is open")
        .palette
        .query()
        .to_string()
}

fn field(a: &App) -> Rect {
    a.palette
        .open
        .as_ref()
        .expect("the palette is open")
        .palette
        .field_rect()
}

fn palette_menu_open(a: &App) -> bool {
    a.palette
        .open
        .as_ref()
        .is_some_and(|p| p.palette.edit_menu_open())
}

/// A triple click in the query box takes all of it, so what is typed next replaces the whole
/// query; on the old route the third click took a word, as the second does.
#[test]
fn a_triple_click_in_the_palettes_box_takes_all_of_it() {
    let mut a = palette_with("alpha beta");
    let f = field(&a);
    for _ in 0..3 {
        click_at(&mut a, f.cx(), f.cy());
    }
    type_text(&mut a, "x");
    assert_eq!(query(&a), "x");
}

/// A right press on the query box opens its edit menu, Escape closes the menu and leaves the
/// palette open, and Shift+F10 opens it from the keys.
#[test]
fn the_palettes_box_has_an_edit_menu_by_the_right_press_and_shift_f10() {
    let mut a = palette_with("alpha");
    let f = field(&a);
    a.pointer_moved(f.cx(), f.cy());
    a.right_press();
    assert!(palette_menu_open(&a), "the right press opens the menu");
    // While it is open everything is under the palette: the menu can reach past the card.
    a.compose();
    assert!(
        a.overlay_bounds
            .iter()
            .any(|b| b.w >= a.size.0 as f32 / a.scale - 1.0),
        "{:?}",
        a.overlay_bounds
    );
    key(&mut a, KeyName::Escape);
    assert!(!palette_menu_open(&a), "Escape closes the menu");
    assert!(a.palette.open.is_some(), "and only the menu");
    shift_f10(&mut a);
    assert!(palette_menu_open(&a), "Shift+F10 opens it at the caret");
    // A right press outside the card closes the open menu first, then the palette, as a left
    // press does.
    a.pointer_moved(2.0, a.size.1 as f32 / a.scale - 2.0);
    a.right_press();
    assert!(!palette_menu_open(&a) && a.palette.open.is_some());
    a.right_press();
    assert!(a.palette.open.is_none());
}

/// A selection dragged past the query box's start goes on with the text each time the loop's
/// timer comes, until all of it is selected. (Typing leaves the box scrolled to its end.)
#[test]
fn a_drag_past_the_palettes_box_start_scrolls_on_at_rest() {
    let long = "word ".repeat(60);
    let long = long.trim_end();
    let mut a = palette_with(long);
    let f = field(&a);
    a.pointer_moved(f.cx(), f.cy());
    a.left_press();
    a.pointer_moved(f.x - 30.0, f.cy());
    let first = a
        .palette_text_deadline()
        .expect("the scroll waits for its timer");
    let before = first - Duration::from_millis(1);
    assert!(
        a.timers(before).is_some_and(|t| t <= first),
        "the loop is woken for it"
    );
    let mut now = before;
    let mut steps = 0;
    while let Some(due) = a.palette_text_deadline() {
        now = now.max(due);
        a.timers(now);
        steps += 1;
        assert!(steps < 4000, "the scroll ends");
    }
    assert!(steps > 0, "the timer scrolled the text");
    a.left_release();
    type_text(&mut a, "x");
    let q = query(&a);
    assert!(
        q.starts_with('x') && q.len() < long.len() / 2,
        "the start was selected: {q:?}"
    );
}

/// A press on a row, then the window loses the focus (no release comes on Windows): the
/// release after coming back chooses nothing.
#[test]
fn a_palette_row_pressed_when_the_focus_goes_is_not_chosen() {
    let dir = scratch("pal-lost");
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join("b.txt"), "two\n").unwrap();
    let mut a = app();
    a.size = (900, 600);
    open_now(&mut a, &dir.join("a.txt"));
    open_now(&mut a, &dir.join("b.txt"));
    a.open_palette("");
    a.compose();
    let r = a.palette.open.as_ref().unwrap().palette.row_rect(0);
    a.pointer_moved(r.cx(), r.cy());
    a.left_press();
    a.end_held();
    a.left_release();
    assert!(a.palette.open.is_some(), "nothing was chosen");
    assert_eq!(a.docs.active(), Some(1));
    let _ = std::fs::remove_dir_all(&dir);
}

fn plan_open() -> App {
    let mut a = app();
    assert!(a.plan_demo("split-gaps"));
    a.compose();
    a
}

/// A rename on the plan screen has the edit menu: Shift+F10 and a right press on it open it
/// (not the row's menu), and Escape closes the menu and leaves the rename open.
#[test]
fn a_rename_on_the_plan_screen_has_the_edit_menu() {
    let mut a = plan_open();
    let g = a.plan.as_ref().unwrap().geo();
    let r = a.plan.as_ref().unwrap().rows().rect(g.list, 3);
    click_at(&mut a, r.x + 100.0, r.cy());
    key(&mut a, KeyName::F(2));
    a.compose();
    shift_f10(&mut a);
    let s = a.plan.as_ref().unwrap();
    assert!(s.edit_menu_open() && s.edit.is_some());
    key(&mut a, KeyName::Escape);
    let s = a.plan.as_ref().unwrap();
    assert!(!s.edit_menu_open(), "Escape closes the menu");
    assert!(s.edit.is_some(), "and leaves the rename open");
    a.compose();
    a.pointer_moved(r.x + 100.0, r.cy());
    a.right_press();
    let s = a.plan.as_ref().unwrap();
    assert!(s.edit_menu_open(), "the right press on the name opens it");
    assert!(s.menu.is_none(), "not the row's menu");
}

/// A tick box pressed when the window loses the focus is not ticked by the release after it.
#[test]
fn a_plan_tick_box_pressed_when_the_focus_goes_is_not_ticked() {
    let mut a = plan_open();
    let g = a.plan.as_ref().unwrap().geo();
    let r = a.plan.as_ref().unwrap().rows().rect(g.list, 3);
    let ticked = |a: &App| a.plan.as_ref().unwrap().plan.note(3).unwrap().ticked();
    let was = ticked(&a);
    a.pointer_moved(r.x + 28.0, r.cy());
    a.left_press();
    a.end_held();
    a.left_release();
    assert_eq!(ticked(&a), was);
    // Without the lost focus the same click ticks it, so the test is on the box.
    click_at(&mut a, r.x + 28.0, r.cy());
    assert_ne!(ticked(&a), was);
}

/// A category of Settings pressed when the window loses the focus is not opened by the
/// release after it.
#[test]
fn a_settings_category_pressed_when_the_focus_goes_is_not_opened() {
    let mut a = app();
    a.size = (900, 600);
    a.compose();
    a.open_settings();
    a.compose();
    let r = a.ui.settings.as_ref().unwrap().list_item_rect(1);
    a.ui.mouse = (r.cx(), r.cy());
    a.settings_press();
    a.end_held();
    a.settings_release();
    assert_eq!(a.ui.settings.as_ref().unwrap().page(), 0);
    a.settings_press();
    a.settings_release();
    assert_eq!(
        a.ui.settings.as_ref().unwrap().page(),
        1,
        "the click itself works"
    );
}
