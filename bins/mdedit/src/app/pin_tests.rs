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
    assert_eq!(a.docs.pinned_count(), 2);
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
    assert_eq!(a.docs.pinned_count(), 2);
    assert!(!a.docs.tabs[2].pinned);

    // Unpinning puts it right after the pinned ones.
    a.tab_command(C::PinTab, 0);
    assert_eq!(a.docs.pinned_count(), 1);
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
