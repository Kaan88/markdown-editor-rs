//! Pinned tabs: kept first, closed only once unpinned, and back after a restart, notes
//! without a file included.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::session::Pin;

fn names(a: &App) -> Vec<String> {
    a.docs.tabs.iter().map(|d| d.name()).collect()
}

fn toast(a: &App) -> Option<String> {
    a.docs.toast(Instant::now()).map(|t| t.to_string())
}

/// An app with tabs a, b, c (files) and d (no file), c active.
fn four() -> App {
    let dir = scratch("pin");
    let mut a = app();
    for n in ["a", "b", "c"] {
        let p = dir.join(format!("{n}.txt"));
        std::fs::write(&p, n).unwrap();
        open_now(&mut a, &p);
    }
    a.docs.new_untitled();
    a.docs.activate(2);
    a
}

#[test]
fn a_pinned_tab_goes_first_and_an_unpinned_one_after_the_pinned() {
    let mut a = four();
    a.tab_command(C::PinTab, 2);
    assert_eq!(names(&a)[..3], ["c.txt", "a.txt", "b.txt"]);
    assert!(a.docs.tabs[0].pinned);
    assert_eq!(a.docs.active(), Some(0), "the active tab stays active");
    a.tab_command(C::PinTab, 3);
    assert_eq!(mdshared_ui::pins::count(&a.docs.tabs), 2);
    assert!(names(&a)[1].starts_with("Untitled"));

    // Dragging keeps each group on its side.
    let first = a.docs.tabs[0].id;
    assert!(a.docs.move_tab(0, 3));
    assert_eq!(
        a.docs.tabs[1].id, first,
        "a pinned tab stays among the pinned"
    );
    assert!(
        a.docs.move_tab(3, 0),
        "an unpinned one goes no further left than them"
    );
    assert_eq!(mdshared_ui::pins::count(&a.docs.tabs), 2);
    assert!(!a.docs.tabs[2].pinned);

    // Unpinning puts it right after the pinned ones.
    a.tab_command(C::PinTab, 0);
    assert_eq!(mdshared_ui::pins::count(&a.docs.tabs), 1);
    assert!(!a.docs.tabs[1].pinned);
}

#[test]
fn closing_passes_by_pinned_tabs_and_their_pin_unpins() {
    let mut a = four();
    a.tab_command(C::PinTab, 0);
    a.tab_command(C::PinTab, 3);
    let pinned = [a.docs.tabs[0].id, a.docs.tabs[1].id];

    // Closing a pinned tab says why it stays.
    a.close_tab(0);
    assert_eq!(a.docs.tabs.len(), 4);
    assert_eq!(toast(&a).as_deref(), Some(crate::commands::strings::PINNED));

    // Close others from an unpinned tab keeps it and the pinned ones.
    a.tab_command(C::CloseOtherTabs, 3);
    assert_eq!(a.docs.tabs.len(), 3);
    assert_eq!([a.docs.tabs[0].id, a.docs.tabs[1].id], pinned);
    // Close all keeps only the pinned ones, with no question about the note's text.
    a.tab_command(C::CloseAllTabs, 2);
    assert_eq!(a.docs.tabs.iter().map(|d| d.id).collect::<Vec<_>>(), pinned);

    // The button where the × was unpins; then the tab closes like any other.
    a.activate(Hit::TabClose(0));
    assert!(!a.docs.tabs.iter().any(|d| d.id == pinned[0] && d.pinned));
    let i = a.docs.index_of_id(pinned[0]).unwrap();
    a.close_tab(i);
    assert!(a.docs.index_of_id(pinned[0]).is_none());
}

#[test]
fn pins_come_back_in_their_order_with_a_note_that_has_no_file() {
    let mut a = four();
    let note = a.docs.tabs[3].untitled;
    a.tab_command(C::PinTab, 3);
    a.tab_command(C::PinTab, 3); // b.txt, after the note
    let pins = a.docs.pins();
    assert_eq!(pins.len(), 2);
    assert_eq!(pins[0], Pin::Untitled(note));

    // Through the session file and back.
    let s = Session {
        pinned: pins.clone(),
        ..Session::default()
    };
    let read = Session::parse(&s.serialize());
    assert_eq!(read.pinned, pins);

    // A new start: files first as the session lists them, then the note from its backup.
    let dir = scratch("pin-restore");
    let mut b = app();
    for n in ["a", "b"] {
        let p = dir.join(format!("{n}.txt"));
        std::fs::write(&p, n).unwrap();
        open_now(&mut b, &p);
    }
    // The paths differ from the first app's; name this one's b.txt.
    let pins = vec![
        Pin::Untitled(note),
        Pin::File(b.docs.tabs[1].path.clone().unwrap()),
    ];
    b.docs.restore_untitled(note, "kept");
    b.docs.activate(0);
    b.docs.mark_pinned(&pins);
    b.docs.pinned_first(&pins);
    assert_eq!(names(&b)[1], "b.txt");
    assert_eq!(b.docs.tabs[0].untitled, note);
    assert!(b.docs.tabs[0].pinned && b.docs.tabs[1].pinned && !b.docs.tabs[2].pinned);
    assert_eq!(names(&b)[2], "a.txt");
    assert_eq!(b.docs.active(), Some(2), "a.txt stays the active tab");
}

#[test]
fn the_tab_left_showing_comes_back_even_a_pinned_note_and_the_setting_picks_the_pin() {
    // Pinned: the note d and c; showing: the note.
    let mut a = four();
    let note = a.docs.tabs[3].untitled;
    a.tab_command(C::PinTab, 3);
    a.tab_command(C::PinTab, 3); // c.txt
    let at = |a: &App, name: &str| a.docs.tabs.iter().position(|d| d.name() == name);
    let note_name = a.docs.tabs[0].name();
    a.docs.activate(at(&a, &note_name).unwrap());
    // Then b, which is not pinned: the note stays the pinned tab used last.
    a.docs.activate(at(&a, "b.txt").unwrap());
    let mut s = Session {
        tabs: a.docs.paths(),
        pinned: a.docs.pins(),
        active: a.docs.active_among_paths(),
        active_tab: a.docs.active_pin(),
        last_pinned: a.docs.last_pinned(),
        ..Session::default()
    };
    assert_eq!(s.last_pinned, Some(Pin::Untitled(note)));
    s = Session::parse(&s.serialize());

    // A new start with the same tabs, in the order a start opens them (files, then the note).
    let restart = |s: &Session| {
        let mut b = app();
        for p in &s.tabs {
            open_now(&mut b, p);
        }
        b.docs.restore_untitled(note, "kept");
        b.docs.mark_pinned(&s.pinned);
        let fallback = Some(s.active);
        // As a start does: show it, then put the pins first (the shown tab stays shown).
        if let Some(i) = b.docs.start_tab(s, fallback) {
            b.docs.activate(i);
        }
        b.docs.pinned_first(&s.pinned);
        let start = b.docs.active();
        (b, start)
    };
    let (b, start) = restart(&s);
    assert_eq!(b.docs.tabs[start.unwrap()].name(), "b.txt");

    // The note was showing when the window closed: it comes back showing, not the tab beside it.
    a.docs.activate(at(&a, &note_name).unwrap());
    s.active = a.docs.active_among_paths();
    s.active_tab = a.docs.active_pin();
    let (b, start) = restart(&s);
    let shown = &b.docs.tabs[start.unwrap()];
    assert!(
        shown.path.is_none() && shown.untitled == note,
        "{}",
        shown.name()
    );

    // With the setting, the pinned tab used last shows, whichever was showing.
    s.active_tab = Some(Pin::File(
        b.docs.tabs[at(&b, "a.txt").unwrap()].path.clone().unwrap(),
    ));
    s.last_pinned = Some(Pin::File(
        b.docs.tabs[at(&b, "c.txt").unwrap()].path.clone().unwrap(),
    ));
    let (b, start) = restart(&s);
    assert_eq!(b.docs.tabs[start.unwrap()].name(), "a.txt");
    s.prefs.start_on_pinned = true;
    let (b, start) = restart(&s);
    assert_eq!(b.docs.tabs[start.unwrap()].name(), "c.txt");
    // A tab that is no longer pinned is not the pinned tab used last.
    s.pinned.retain(|p| matches!(p, Pin::Untitled(_)));
    let (b, start) = restart(&s);
    assert_eq!(b.docs.tabs[start.unwrap()].name(), "a.txt");
}

/// Tabs not shown in the last week of use are offered at start: never a pinned one, the one
/// showing, one with changes, or a note without a file. Keep starts their count again; Close
/// closes them.
#[test]
fn tabs_not_opened_for_a_week_of_use_are_offered_and_closed_or_kept() {
    let mut a = four(); // a b c d(note), c showing
    a.tab_command(C::PinTab, 1); // b pinned: b a c d
    let mut s = Session {
        tabs: a.docs.paths(),
        pinned: a.docs.pins(),
        use_days: (10, 0), // another date: this start is day 11
        seen: a.docs.seen().into_iter().map(|(p, _)| (p, 2)).collect(),
        ..Session::default()
    };
    let start = |s: &mut Session| {
        let mut b = app();
        for p in s.tabs.clone() {
            open_now(&mut b, &p);
        }
        b.docs.start_day(s);
        b.docs.mark_pinned(&s.pinned);
        b.docs.activate(2); // c
        b.docs.pinned_first(&s.pinned);
        b.docs.offer_stale_tabs();
        b
    };
    let mut b = start(&mut s.clone());
    let offered = |b: &App| -> Vec<String> {
        (b.docs.stale.iter())
            .map(|&id| b.docs.tabs[b.docs.index_of_id(id).unwrap()].name())
            .collect()
    };
    assert_eq!(offered(&b), ["a.txt"], "not b (pinned) nor c (showing)");
    b.stale_bar(Hit::StaleKeep);
    assert!(b.docs.stale.is_empty());
    s.seen = b.docs.seen();
    assert!(
        s.seen
            .iter()
            .any(|(p, d)| matches!(p, Pin::File(f) if f.ends_with("a.txt")) && *d == 11)
    );

    // A changed tab is not offered; Close closes the others.
    s.seen = s.seen.iter().map(|(p, _)| (p.clone(), 2)).collect();
    s.use_days = (20, 0);
    let mut b = start(&mut s.clone());
    assert_eq!(offered(&b), ["a.txt"]);
    let i = b
        .docs
        .tabs
        .iter()
        .position(|d| d.name() == "a.txt")
        .unwrap();
    b.docs.activate(i);
    b.docs.edit(crate::docs::EditOp::Insert("x"), None);
    b.docs.activate(2);
    b.stale_bar(Hit::StaleClose);
    assert_eq!(
        names(&b),
        ["b.txt", "a.txt", "c.txt"],
        "a has changes: kept"
    );
    let mut b = start(&mut s.clone());
    b.stale_bar(Hit::StaleClose);
    assert_eq!(names(&b), ["b.txt", "c.txt"]);
}

/// The owner: the days before a tab is offered are a setting (Settings > Session), kept in the
/// session file. A tab last shown 9 days of use ago is offered after 9 days, not after 10.
#[test]
fn the_days_before_a_tab_is_offered_are_a_setting() {
    use crate::app::modal::STALE_DAYS;
    use mdshared_ui::settings::Value;
    let mut a = four(); // a b c d(note), c showing
    let mut s = Session {
        tabs: a.docs.paths(),
        use_days: (10, 0), // this start is day 11
        seen: a.docs.seen().into_iter().map(|(p, _)| (p, 2)).collect(),
        ..Session::default()
    };
    a.docs.start_day(&mut s);
    a.docs.activate(2); // c
    a.apply_setting(STALE_DAYS, Value::Number(10));
    assert_eq!(a.docs.prefs.stale_days, 10);
    a.docs.offer_stale_tabs();
    assert!(a.docs.stale.is_empty(), "9 days of use is under 10");
    a.apply_setting(STALE_DAYS, Value::Number(9));
    a.docs.offer_stale_tabs();
    assert_eq!(
        a.docs.stale.len(),
        2,
        "a and b; not c (showing) nor the note"
    );
    // Kept for the next start, and in its range.
    let back = Session::parse(&a.session.serialize());
    assert_eq!(back.prefs.stale_days, 9);
    a.apply_setting(STALE_DAYS, Value::Number(0));
    assert_eq!(a.docs.prefs.stale_days, 1);
    assert_eq!(
        crate::chrome::stale_label(2, 9),
        "2 tabs have not been opened in the last 9 days you used mdedit."
    );
    assert_eq!(
        crate::chrome::stale_label(1, 1),
        "1 tab has not been opened on the last day you used mdedit."
    );
}
