//! The plan screen as the window hosts it: an `App` built without a window, driven by the same
//! functions the events call, and composed as the window draws it.

use super::files_tests::{app, open_now, scratch, type_text};
use super::*;
use crate::keys::{Key, KeyName};
use crate::plan_demo;
use winit::event::MouseScrollDelta;

fn press(a: &mut App, k: KeyName) {
    a.key_pressed(Key::Name(k), None);
}

fn opened(name: &str) -> App {
    let mut a = app();
    assert!(a.plan_demo(name), "{name} is a demo");
    a.compose();
    a
}

fn click(a: &mut App, x: f32, y: f32) {
    a.pointer_moved(x, y);
    a.left_press();
    a.left_release();
    a.compose();
}

#[test]
fn every_demo_opens_a_sound_plan_and_draws_in_both_themes() {
    for name in plan_demo::NAMES {
        let mut a = app();
        assert!(a.plan_demo(name), "{name}");
        // `split-ai-ask` is the split panel, with the screen behind it.
        let screen = a.plan.as_ref().or(a.plan_behind.as_ref());
        let screen = screen.expect("the screen is open");
        assert_eq!(screen.plan.check(), Ok(()), "{name}");
        assert!(!screen.edited, "{name} opens unchanged");
        for dark in [false, true] {
            a.pal = if dark { theme::DARK } else { theme::LIGHT };
            a.full = true;
            a.compose();
        }
    }
    let mut a = app();
    assert!(!a.plan_demo("no-such-demo"));
    assert!(a.plan.is_none());
}

#[test]
fn a_tick_repaints_the_card_because_the_screen_is_in_the_overlays_frame_signature() {
    let mut a = opened("split-gaps");
    let before = a.screen.pixels.clone();
    press(&mut a, KeyName::Space);
    assert!(
        !a.plan
            .as_ref()
            .expect("open")
            .plan
            .note(5)
            .expect("a note")
            .ticked()
    );
    a.compose();
    assert_ne!(
        before, a.screen.pixels,
        "the tick was made, and nothing was repainted"
    );
    // The pointer resting on a row is a change of look as well.
    let before = a.screen.pixels.clone();
    let g = a.plan.as_ref().expect("open").geo();
    let r = a.plan.as_ref().expect("open").rows().rect(g.list, 2);
    a.pointer_moved(r.x + 200.0, r.cy());
    a.compose();
    assert_ne!(before, a.screen.pixels, "the hovered row lights");
}

#[test]
fn escape_keeps_a_plan_that_was_changed_and_show_draws_the_same_plan() {
    let mut a = opened("split-gaps");
    press(&mut a, KeyName::Space);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Down);
    a.compose();
    let shown = a.screen.pixels.clone();
    let (sel, scroll) = {
        let p = a.plan.as_ref().expect("open");
        (p.sel, p.scroll)
    };
    press(&mut a, KeyName::Escape);
    assert!(a.plan.is_none(), "the card closes");
    assert!(a.plan_parked.is_some(), "and the plan is kept");
    a.compose();
    assert_ne!(shown, a.screen.pixels, "the card is gone");
    assert!(a.plan_show());
    a.compose();
    assert!(a.plan_parked.is_none());
    let p = a.plan.as_ref().expect("open again");
    assert_eq!((p.sel, p.scroll), (sel, scroll));
    assert!(
        !p.plan.note(5).expect("a note").ticked(),
        "the edit is still there"
    );
    assert_eq!(shown, a.screen.pixels, "the same plan, to the pixel");
    // Drop forgets it.
    press(&mut a, KeyName::Escape);
    assert!(a.plan_parked.is_some());
    a.plan_drop();
    assert!(a.plan_parked.is_none() && !a.plan_show());
}

#[test]
fn escape_on_a_plan_nobody_changed_loses_nothing_and_keeps_nothing() {
    let mut a = opened("split");
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Escape);
    assert!(a.plan.is_none() && a.plan_parked.is_none());
}

#[test]
fn the_window_gives_the_screen_its_keys_its_pointer_its_wheel_and_its_typing() {
    let mut a = opened("split-gaps");
    let g = a.plan.as_ref().expect("open").geo();
    // A click on a row's tick box.
    let r = a.plan.as_ref().expect("open").rows().rect(g.list, 3);
    click(&mut a, r.x + 28.0, r.cy());
    {
        let p = a.plan.as_ref().expect("open");
        assert!(!p.plan.note(3).expect("a note").ticked());
        assert_eq!(p.sel, 3);
    }
    // The wheel scrolls the list under the pointer, and the page below does not move.
    let page = a.docs.active().map(|i| a.docs.tabs[i].id);
    a.pointer_moved(r.x + 100.0, r.cy());
    a.wheel(MouseScrollDelta::LineDelta(0.0, -3.0));
    assert!(a.plan.as_ref().expect("open").scroll > 0.0);
    assert_eq!(a.docs.active().map(|i| a.docs.tabs[i].id), page);
    // A click outside the card does nothing, and does not close it either.
    click(&mut a, 3.0, 3.0);
    assert!(a.plan.is_some());
    // F2, typing, Enter: the name changes.
    press(&mut a, KeyName::F(2));
    a.key_pressed(Key::Name(KeyName::Char('z')), Some("z"));
    a.compose();
    assert!(
        a.plan
            .as_ref()
            .expect("open")
            .edit
            .as_ref()
            .is_some_and(|e| e.field.text().ends_with('z') || e.field.text() == "z"),
        "what is typed goes to the name"
    );
    press(&mut a, KeyName::Enter);
    let title = a
        .plan
        .as_ref()
        .expect("open")
        .plan
        .note(3)
        .expect("a note")
        .title()
        .to_string();
    assert!(title.contains('z') || title == "z", "{title}");
    // The right click opens the row's menu; the menu key does too.
    a.pointer_moved(r.x + 100.0, r.cy());
    a.right_press();
    a.compose();
    assert!(a.plan.as_ref().expect("open").menu.is_some());
    press(&mut a, KeyName::Escape);
    assert!(
        a.plan.as_ref().expect("open").menu.is_none(),
        "Escape closes the menu only"
    );
    a.key_pressed(Key::Menu, None);
    assert!(a.plan.as_ref().expect("open").menu.is_some());
}

#[test]
fn an_input_method_composes_into_the_name_being_typed() {
    let mut a = opened("split-gaps");
    press(&mut a, KeyName::F(2));
    a.ime_preedit("\u{3042}".to_string(), Some((0, 0)));
    let e = a
        .plan
        .as_ref()
        .expect("open")
        .edit
        .as_ref()
        .expect("the field");
    assert!(e.field.preedit().is_some());
}

#[test]
fn split_into_notes_needs_the_file_on_disk_to_be_what_is_in_the_tab() {
    let dir = scratch("plan-open");
    let path = dir.join("Trip.md");
    std::fs::write(&path, "# One\n\ntext\n\n# Two\n\nmore\n").expect("a file");
    let mut a = app();
    // The window needs a settings folder, as a build does (rule 9): no session, no library.
    a.session_path = Some(dir.join("settings.ini"));
    open_now(&mut a, &path);
    assert!(a.plan_open_active(), "a saved document opens a plan");
    {
        let p = a.plan.as_ref().expect("open");
        assert_eq!(p.source.name, "Trip.md");
        assert_eq!(p.plan.notes().len(), 2);
        assert!(p.dest.is_new, "a new group, named after the file");
        assert_eq!(p.dest.name, "Trip");
        assert_eq!(p.plan.check(), Ok(()));
    }
    a.plan = None;
    // With a change not saved, it says so and opens nothing.
    type_text(&mut a, "x");
    assert!(
        a.docs.active_doc().is_some_and(|d| d.modified()),
        "the tab was changed"
    );
    assert!(!a.plan_open_active());
    assert!(a.plan.is_none());
    let toast = a.docs.toast(Instant::now()).unwrap_or_default().to_string();
    assert_eq!(
        toast,
        "Save Trip.md first: the notes are made from the file on disk."
    );
    // Nothing was written anywhere.
    assert_eq!(
        std::fs::read_dir(&dir).expect("the folder").count(),
        1,
        "only the file that was there"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
