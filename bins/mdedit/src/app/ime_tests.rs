//! What an input method is composing: shown where it will go by whatever has the keyboard, and
//! never part of the text until it is committed.

use super::files_tests::{app, open_now, scratch};
use super::*;

/// A frame with the caret shown, as it is just after a key.
fn frame(a: &mut App) -> Vec<u8> {
    a.caret_reset();
    a.compose();
    a.screen.pixels.clone()
}

#[test]
fn a_composition_is_drawn_at_the_caret_and_not_typed() {
    let dir = scratch("ime");
    let path = dir.join("a.txt");
    std::fs::write(&path, "one\n").unwrap();
    let mut a = app();
    open_now(&mut a, &path);
    a.focused = true;
    let before = frame(&mut a);

    a.ime_preedit("かな".to_string(), Some((6, 6)));
    assert_eq!(
        a.view.preedit.as_ref().map(|p| p.text.as_str()),
        Some("かな")
    );
    let composing = frame(&mut a);
    assert!(before != composing, "the composition is drawn");
    let text = |a: &App| {
        let b = &a.docs.active_ready().unwrap().buffer;
        let mut s = String::new();
        b.text(0..b.len_bytes(), &mut s);
        s
    };
    assert_eq!(text(&a), "one\n", "a composition is not text yet");

    // The empty composition that ends it takes the drawing away.
    a.ime_preedit(String::new(), None);
    assert_eq!(a.view.preedit, None);
    assert!(
        frame(&mut a) == before,
        "the frame is as before the composition"
    );

    // Committed, it is typed as any text.
    a.ime_preedit("か".to_string(), None);
    a.ime_preedit(String::new(), None);
    a.ime_commit("家");
    assert_eq!(text(&a), "家one\n");
}

#[test]
fn a_composition_goes_to_the_box_with_the_keyboard() {
    let dir = scratch("ime-box");
    let path = dir.join("a.txt");
    std::fs::write(&path, "one\n").unwrap();
    let mut a = app();
    open_now(&mut a, &path);
    a.ui.find_focused = true;
    a.ime_preedit("ç".to_string(), None);
    assert_eq!(a.view.preedit, None, "not in the text");
    assert_eq!(a.ui.field().preedit().map(|p| p.text.as_str()), Some("ç"));
    assert_eq!(a.ui.field().text(), "");
}
