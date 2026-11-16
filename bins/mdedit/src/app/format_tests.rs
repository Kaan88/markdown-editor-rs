//! Headless tests of the format bar: where it shows, what a button does, what shows pressed.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::commands::CommandId as C;
use crate::format_bar::FORMAT_H;

fn press_hit(a: &mut App, hit: Hit) {
    a.compose();
    let r = a.hit_rect(hit).expect("the button is on screen");
    a.ui.mouse = (r.cx(), r.cy());
    a.left_press();
    a.left_release();
}

fn text(a: &App) -> String {
    a.docs.active_ready().unwrap().buffer.rope().to_string()
}

#[test]
fn the_bar_shows_for_markdown_only_and_goes_with_its_toggle() {
    let dir = scratch("fmt-show");
    std::fs::write(dir.join("a.md"), "word\n").unwrap();
    std::fs::write(dir.join("b.txt"), "word\n").unwrap();
    let mut a = app();
    a.size = (1100, 700);
    open_now(&mut a, &dir.join("b.txt"));
    a.compose();
    let plain_top = a.areas.text.y;
    assert!(a.hit_rect(Hit::Format(C::MdBold)).is_none());
    open_now(&mut a, &dir.join("a.md"));
    a.compose();
    assert_eq!(a.areas.text.y, plain_top + FORMAT_H);
    // Every button fits at this width: no overflow button.
    assert!(a.hit_rect(Hit::Format(C::MdRule)).is_some());
    assert!(a.hit_rect(Hit::FormatMore).is_none());
    a.command(C::ToggleFormatBar);
    a.compose();
    assert_eq!(a.areas.text.y, plain_top);
    assert!(a.hit_rect(Hit::Format(C::MdBold)).is_none());
    a.command(C::ToggleFormatBar);
    // A narrow window keeps the first groups and offers the Markdown menu for the rest.
    a.size = (560, 700);
    a.compose();
    assert!(a.hit_rect(Hit::Format(C::MdBold)).is_some());
    assert!(a.hit_rect(Hit::Format(C::MdRule)).is_none());
    press_hit(&mut a, Hit::FormatMore);
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::Markdown));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_button_edits_the_text_and_shows_pressed_where_the_caret_is() {
    let dir = scratch("fmt-press");
    std::fs::write(dir.join("a.md"), "word\n").unwrap();
    let mut a = app();
    a.size = (1100, 700);
    open_now(&mut a, &dir.join("a.md"));
    // The find box had the keyboard: a button gives it back to the text.
    a.ui.find_focused = true;
    press_hit(&mut a, Hit::Format(C::MdHeading2));
    assert_eq!(text(&a), "## word\n");
    assert!(!a.ui.find_focused);
    let state = crate::format_bar::shown(&a.docs).unwrap();
    assert_eq!(state.line.heading, 2);
    press_hit(&mut a, Hit::Format(C::MdTaskList));
    assert_eq!(text(&a), "- [ ] ## word\n");
    a.command(C::Undo);
    a.command(C::Undo);
    assert_eq!(text(&a), "word\n");
    // Bold at a caret: the markers go in, the caret between them, and Bold shows pressed.
    if let Some(r) = a.docs.active_ready_mut() {
        r.view.sel = mdedit_core::Selection::at(4);
    }
    press_hit(&mut a, Hit::Format(C::MdBold));
    assert_eq!(text(&a), "word****\n");
    assert!(crate::format_bar::shown(&a.docs).unwrap().bold);
    let _ = std::fs::remove_dir_all(&dir);
}
