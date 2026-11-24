//! Dragging a note in the notes pane (D-130): where it goes, where nothing happens, Escape, the
//! edge scroll, what the drag draws, and that a press and release is still a click.

use super::files_tests::{open_now, type_text};
use super::library_write_tests::{Lib, labels, lib, read, slash, toast};
use super::*;
use crate::keys::{Key, KeyName};
use crate::ui::Drag;
use mdshared_ui::gfx::Rect;

/// Presses the row `from`, drags it over the row `to` and lets go there.
fn drag_from_to(a: &mut App, from: usize, to: usize) {
    a.compose();
    let source = a.hit_rect(Hit::LibraryRow(from)).expect("the note's row");
    let target = a
        .hit_rect(Hit::LibraryRow(to))
        .expect("where it is dropped");
    a.ui.mouse = (source.cx(), source.cy());
    a.left_press();
    assert!(
        matches!(a.ui.drag, Some(Drag::LibraryNote { .. })),
        "a press on a note is a drag waiting to move"
    );
    let drag = a.ui.drag.unwrap();
    a.drag_to(drag, target.cx(), target.cy());
    assert!(
        matches!(a.ui.drag, Some(Drag::LibraryNote { moving: true, .. })),
        "a few px of movement make it one"
    );
    a.ui.mouse = (target.cx(), target.cy());
    a.left_release();
}

fn row_of(a: &App, label: &str) -> usize {
    a.ui.library
        .rows
        .iter()
        .position(|r| r.label == label)
        .unwrap_or_else(|| panic!("no row called {label}"))
}

fn parent_label(a: &App, at: usize) -> String {
    a.ui.library
        .parent_of(at)
        .map(|p| a.ui.library.rows[p].label.clone())
        .unwrap_or_default()
}

/// The pixels of `r` as drawn now.
fn pixels(a: &mut App, r: Rect) -> Vec<u8> {
    a.compose();
    let s = a.scale;
    let w = a.screen.width as usize;
    let (x0, x1) = ((r.x * s) as usize, (r.right() * s) as usize);
    let mut out = Vec::new();
    for y in (r.y * s) as usize..(r.bottom() * s) as usize {
        out.extend_from_slice(&a.screen.pixels[(y * w + x0) * 4..(y * w + x1) * 4]);
    }
    out
}

#[test]
fn a_note_dragged_onto_a_section_is_written_under_it() {
    let (mut a, l) = lib("libd-section");
    // Rows: Personal, Money, Taxes 2026, 2025, Last year, Trips.
    drag_from_to(&mut a, 2, 5);
    let text = read(&l.file);
    let parsed = mdshared_app::library::parse(&text, &l.dir.join("library"));
    let taxes = parsed
        .entries
        .iter()
        .find(|e| e.title == "Taxes 2026")
        .unwrap_or_else(|| panic!("the note is gone: {text}"));
    assert_eq!(taxes.sections, vec!["Trips".to_string()], "{text}");
    assert_eq!(text.matches("Taxes 2026").count(), 1, "not listed twice");
    assert!(toast(&a).contains("is in Personal now"), "{}", toast(&a));
    // Its row is under the section it went into, and the keyboard's row is on it.
    let at = row_of(&a, "Taxes 2026");
    assert_eq!(parent_label(&a, at), "Trips");
    assert_eq!(a.ui.library.cur, Some(at));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_dropped_on_another_note_goes_under_the_heading_it_is_written_in() {
    let (mut a, l) = lib("libd-note");
    // "Last year" is under 2025: dropping on a note stands for its heading.
    drag_from_to(&mut a, 2, 4);
    let text = read(&l.file);
    let parsed = mdshared_app::library::parse(&text, &l.dir.join("library"));
    let taxes = parsed
        .entries
        .iter()
        .find(|e| e.title == "Taxes 2026")
        .unwrap_or_else(|| panic!("the note is gone: {text}"));
    assert_eq!(
        taxes.sections,
        vec!["Money".to_string(), "2025".to_string()],
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_dragged_across_groups_is_listed_in_the_other_one_only() {
    let (mut a, l) = lib("libd-cross");
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, "# Work\n\n## Projects\n").unwrap();
    a.library_rows();
    a.compose();
    let projects = row_of(&a, "Projects");
    drag_from_to(&mut a, 2, projects);
    let text = read(&work);
    assert!(text.contains("- [Taxes 2026]("), "{text}");
    let parsed = mdshared_app::library::parse(&text, &l.dir.join("library"));
    let taxes = parsed
        .entries
        .iter()
        .find(|e| e.title == "Taxes 2026")
        .unwrap_or_else(|| panic!("not in Work: {text}"));
    assert_eq!(taxes.sections, vec!["Projects".to_string()]);
    assert!(
        !read(&l.file).contains("Taxes 2026"),
        "the old file has no line for it: {}",
        read(&l.file)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_dropped_on_the_heading_it_is_already_under_writes_nothing() {
    let (mut a, l) = lib("libd-same");
    let before = read(&l.file);
    drag_from_to(&mut a, 2, 1);
    assert_eq!(read(&l.file), before, "not a byte");
    assert_eq!(toast(&a), "", "and not a word");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_drag_let_go_outside_the_pane_moves_nothing_and_opens_nothing() {
    let (mut a, l) = lib("libd-out");
    a.compose();
    let source = a.hit_rect(Hit::LibraryRow(2)).unwrap();
    a.ui.mouse = (source.cx(), source.cy());
    a.left_press();
    let text = a.areas.text;
    a.drag_to(a.ui.drag.unwrap(), text.cx(), text.cy());
    assert!(a.ui.library.drop.is_none(), "no heading is marked");
    a.ui.mouse = (text.cx(), text.cy());
    a.left_release();
    assert_eq!(read(&l.file), l.text());
    assert!(
        a.docs.active_doc().and_then(|d| d.path.clone()) != Some(l.taxes.clone()),
        "a drag that moved is not a click: the note is not opened"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn escape_abandons_the_drag_and_the_note_stays_where_it_was() {
    let (mut a, l) = lib("libd-esc");
    a.compose();
    let source = a.hit_rect(Hit::LibraryRow(2)).unwrap();
    let target = a.hit_rect(Hit::LibraryRow(5)).unwrap();
    a.ui.mouse = (source.cx(), source.cy());
    a.left_press();
    a.drag_to(a.ui.drag.unwrap(), target.cx(), target.cy());
    assert_eq!(a.ui.library.drop, Some(5), "Trips is marked");
    a.key_pressed(Key::Name(KeyName::Escape), None);
    assert!(a.ui.drag.is_none(), "the drag is gone");
    assert!(a.ui.library.drop.is_none(), "and its mark with it");
    a.ui.mouse = (target.cx(), target.cy());
    a.left_release();
    assert_eq!(read(&l.file), l.text(), "nothing was written");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_press_and_release_on_a_note_is_still_a_click() {
    let (mut a, l) = lib("libd-click");
    a.compose();
    let row = a.hit_rect(Hit::LibraryRow(2)).unwrap();
    a.ui.mouse = (row.cx(), row.cy());
    a.left_press();
    a.ui.mouse = (row.cx(), row.cy());
    a.left_release();
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()),
        Some(l.taxes.clone()),
        "the note opened"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_drop_is_refused_while_the_library_file_has_unsaved_changes() {
    let (mut a, l) = lib("libd-busy");
    open_now(&mut a, &l.file);
    type_text(&mut a, "x");
    assert!(a.docs.active_doc().unwrap().modified());
    a.compose();
    let source = a.hit_rect(Hit::LibraryRow(2)).unwrap();
    let target = a.hit_rect(Hit::LibraryRow(5)).unwrap();
    a.ui.mouse = (source.cx(), source.cy());
    a.left_press();
    a.drag_to(a.ui.drag.unwrap(), target.cx(), target.cy());
    a.ui.mouse = (target.cx(), target.cy());
    a.left_release();
    assert_eq!(read(&l.file), l.text(), "the file on disk is untouched");
    assert!(toast(&a).contains("not saved"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn holding_a_note_at_the_pane_edge_scrolls_the_list() {
    let (mut a, l) = lib("libd-scroll");
    // Enough rows to have something to scroll to.
    let many = l.dir.join("library").join("zz-many.md");
    let mut text = String::from("# Many\n\n");
    for i in 0..60 {
        text.push_str(&format!("- [Note {i}](missing{i}.md)\n"));
    }
    std::fs::write(&many, text).unwrap();
    a.library_rows();
    a.compose();
    assert!(
        a.ui.library.content > a.ui.library.view,
        "the list is longer than the pane"
    );
    let source = a.hit_rect(Hit::LibraryRow(2)).unwrap();
    let list = crate::library_pane::list(a.areas.files.unwrap());
    a.ui.mouse = (source.cx(), source.cy());
    a.left_press();
    a.ui.mouse = (source.cx(), list.bottom() - 2.0);
    a.drag_to(a.ui.drag.unwrap(), source.cx(), list.bottom() - 2.0);
    assert!(
        a.ui.autoscroll_due.is_some(),
        "a hold at the edge asks for the next scroll step"
    );
    let before = a.ui.library.scroll;
    a.library_drag_scroll();
    assert!(
        a.ui.library.scroll > before,
        "the list moved under the note: {} then {}",
        before,
        a.ui.library.scroll
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_heading_a_dragged_note_would_go_into_is_drawn_and_so_is_the_note() {
    let (mut a, l) = lib("libd-pixels");
    a.compose();
    let rect = |a: &App, i: usize| a.hit_rect(Hit::LibraryRow(i)).unwrap();
    let untouched = rect(&a, 0);
    let target_row = rect(&a, 3);
    let source_row = rect(&a, 2);
    let over = rect(&a, 4);
    let chip = Rect::new(over.x, over.cy(), 160.0, 26.0);
    let before_untouched = pixels(&mut a, untouched);
    let before_target = pixels(&mut a, target_row);
    let before_source = pixels(&mut a, source_row);
    let before_chip = pixels(&mut a, chip);
    a.ui.mouse = (source_row.cx(), source_row.cy());
    a.left_press();
    a.drag_to(a.ui.drag.unwrap(), over.cx(), over.cy());
    a.ui.mouse = (over.cx(), over.cy());
    a.compose();
    assert_eq!(a.ui.library.drop, Some(3), "2025 is where it would go");
    let target_now = pixels(&mut a, target_row);
    let source_now = pixels(&mut a, source_row);
    let chip_now = pixels(&mut a, chip);
    let untouched_now = pixels(&mut a, untouched);
    assert_ne!(
        target_now, before_target,
        "the heading it would go into is marked"
    );
    assert_ne!(source_now, before_source, "the note held is set aside");
    assert_ne!(chip_now, before_chip, "the chip is drawn");
    assert_eq!(
        untouched_now, before_untouched,
        "a row the drag does not touch is as it was"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Presses the row `from` and carries it, moving, to `(x, y)`, without letting go.
fn hold_over(a: &mut App, from: usize, x: f32, y: f32) {
    a.compose();
    let source = a.hit_rect(Hit::LibraryRow(from)).expect("the note's row");
    a.ui.mouse = (source.cx(), source.cy());
    a.left_press();
    a.ui.mouse = (x, y);
    a.drag_to(a.ui.drag.unwrap(), x, y);
    assert!(
        matches!(a.ui.drag, Some(Drag::LibraryNote { moving: true, .. })),
        "the note is moving"
    );
}

/// The library file with three notes under two sections, and a third section to drop on:
/// rows are Personal, A, One, B, Two, Three, Trips.
fn three(l: &Lib) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for n in ["one", "two", "three"] {
        let p = l.dir.join(format!("{n}.md"));
        std::fs::write(&p, format!("# {n}\n")).unwrap();
        paths.push(p);
    }
    std::fs::write(
        &l.file,
        format!(
            "# Personal\n\n## A\n\n- [One]({})\n\n## B\n\n- [Two]({})\n- [Three]({})\n\n## Trips\n",
            slash(&paths[0]),
            slash(&paths[1]),
            slash(&paths[2])
        ),
    )
    .unwrap();
    paths
}

/// The headings the note called `title` is written under, as the library file reads now.
fn sections_of(l: &Lib, title: &str) -> Vec<String> {
    let text = read(&l.file);
    let parsed = mdshared_app::library::parse(&text, &l.dir.join("library"));
    parsed
        .entries
        .iter()
        .find(|e| e.title == title)
        .unwrap_or_else(|| panic!("no {title} in {text}"))
        .sections
        .clone()
}

/// Audit 4, C1: the drop is found from the pointer's x as well as its y. Let go over the editor
/// at the height of a row of the pane, the note stays where it is, and no scroll is armed there.
#[test]
fn a_note_let_go_over_the_editor_at_a_rows_height_is_not_moved() {
    let (mut a, l) = lib("libd-out-row");
    a.compose();
    let trips = a.hit_rect(Hit::LibraryRow(5)).unwrap();
    let text = a.areas.text;
    assert!(text.cx() > trips.right(), "the point is outside the pane");
    hold_over(&mut a, 2, text.cx(), trips.cy());
    assert_eq!(
        a.ui.library.drop, None,
        "no heading is marked from beside the pane"
    );
    a.ui.mouse = (text.cx(), trips.cy());
    a.left_release();
    assert_eq!(read(&l.file), l.text(), "nothing was written");
    assert_eq!(toast(&a), "", "and nothing said");
    // Over the editor at the height of the pane's bottom edge, the list is not scrolled.
    let list = crate::library_pane::list(a.areas.files.unwrap());
    hold_over(&mut a, 2, text.cx(), list.bottom() - 2.0);
    assert!(
        a.ui.autoscroll_due.is_none(),
        "the edge scroll is the pane's, not the editor's"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Audit 4, C2: while a note is held and moving, a key does not reach the pane. Enter on the
/// keyboard's row would fold "A" and slide every row under it up by one.
#[test]
fn a_key_while_a_note_is_held_reaches_nothing_and_the_note_held_is_the_one_moved() {
    let (mut a, l) = lib("libd-key");
    three(&l);
    a.library_rows();
    a.compose();
    let two = row_of(&a, "Two");
    let trips = a.hit_rect(Hit::LibraryRow(row_of(&a, "Trips"))).unwrap();
    a.ui.library.focused = true;
    a.ui.library.set_cur(row_of(&a, "A"));
    hold_over(&mut a, two, trips.cx(), trips.cy());
    let before = labels(&a);
    for key in [
        KeyName::Enter,
        KeyName::Left,
        KeyName::Delete,
        KeyName::F(2),
    ] {
        a.key_pressed(Key::Name(key), None);
    }
    assert_eq!(labels(&a), before, "no row moved");
    assert!(a.ui.library.edit.is_none(), "F2 opened no name box");
    assert!(
        matches!(a.ui.drag, Some(Drag::LibraryNote { moving: true, .. })),
        "the note is still held"
    );
    a.compose();
    let trips = a.hit_rect(Hit::LibraryRow(row_of(&a, "Trips"))).unwrap();
    a.ui.mouse = (trips.cx(), trips.cy());
    a.left_release();
    assert_eq!(sections_of(&l, "Two"), vec!["Trips".to_string()]);
    assert_eq!(sections_of(&l, "Three"), vec!["B".to_string()]);
    assert_eq!(sections_of(&l, "One"), vec!["A".to_string()]);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Audit 4, C2 and A F15: a read of the library that finishes while a note is held makes the
/// rows again. The drag holds the note by what it is, so the note moved is the one pressed, the
/// pane draws that note as the one held, and the heading marked is still the one under the
/// pointer.
#[test]
fn rows_made_again_under_a_held_note_still_move_the_note_held() {
    let (mut a, l) = lib("libd-reread");
    let paths = three(&l);
    a.library_rows();
    a.compose();
    let two = row_of(&a, "Two");
    let trips = a.hit_rect(Hit::LibraryRow(row_of(&a, "Trips"))).unwrap();
    hold_over(&mut a, two, trips.cx(), trips.cy());
    assert_eq!(a.ui.library.drop, Some(row_of(&a, "Trips")));
    // Another window adds a note above the one held, and the pane takes the read.
    let zero = l.dir.join("zero.md");
    std::fs::write(&zero, "# zero\n").unwrap();
    std::fs::write(
        &l.file,
        format!(
            "# Personal\n\n## A\n\n- [Zero]({})\n- [One]({})\n\n## B\n\n- [Two]({})\n- [Three]({})\n\n## Trips\n",
            slash(&zero),
            slash(&paths[0]),
            slash(&paths[1]),
            slash(&paths[2])
        ),
    )
    .unwrap();
    let read_now = mdshared_app::library::read(&l.dir.join("library"));
    assert!(a.library_accept(read_now), "the read is shown");
    assert_eq!(row_of(&a, "Two"), two + 1, "the rows moved under the note");
    assert_eq!(
        a.ui.library.held_at,
        Some(row_of(&a, "Two")),
        "held by what it is"
    );
    assert_eq!(
        a.ui.library.drop,
        Some(row_of(&a, "Trips")),
        "the mark followed its heading"
    );
    a.compose();
    let trips = a.hit_rect(Hit::LibraryRow(row_of(&a, "Trips"))).unwrap();
    a.ui.mouse = (trips.cx(), trips.cy());
    a.left_release();
    assert_eq!(sections_of(&l, "Two"), vec!["Trips".to_string()]);
    assert_eq!(
        sections_of(&l, "One"),
        vec!["A".to_string()],
        "not the row now at its index"
    );
    assert_eq!(sections_of(&l, "Three"), vec!["B".to_string()]);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Audit 4, C2: a press that has not moved yet is a click on the note pressed, even when the
/// rows were made again before the button came up; and a note that left the rows meanwhile is
/// neither opened nor moved.
#[test]
fn a_click_on_a_note_opens_the_note_pressed_after_the_rows_were_made_again() {
    let (mut a, l) = lib("libd-click-reread");
    let paths = three(&l);
    a.library_rows();
    a.compose();
    let two = row_of(&a, "Two");
    let row = a.hit_rect(Hit::LibraryRow(two)).unwrap();
    a.ui.mouse = (row.cx(), row.cy());
    a.left_press();
    assert!(matches!(
        a.ui.drag,
        Some(Drag::LibraryNote { moving: false, .. })
    ));
    // A read in which "One" is gone moves "Two" up a row before the button comes up.
    std::fs::write(
        &l.file,
        format!(
            "# Personal\n\n## A\n\n## B\n\n- [Two]({})\n- [Three]({})\n\n## Trips\n",
            slash(&paths[1]),
            slash(&paths[2])
        ),
    )
    .unwrap();
    assert!(a.library_accept(mdshared_app::library::read(&l.dir.join("library"))));
    assert_eq!(row_of(&a, "Two"), two - 1);
    a.left_release();
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()),
        Some(paths[1].clone()),
        "the note pressed opened, not the one now at its row"
    );
    // A note pressed and then taken out of the library is not acted on at all.
    a.compose();
    let row = a.hit_rect(Hit::LibraryRow(row_of(&a, "Three"))).unwrap();
    a.ui.mouse = (row.cx(), row.cy());
    a.left_press();
    std::fs::write(
        &l.file,
        format!(
            "# Personal\n\n## A\n\n## B\n\n- [Two]({})\n\n## Trips\n",
            slash(&paths[1])
        ),
    )
    .unwrap();
    assert!(a.library_accept(mdshared_app::library::read(&l.dir.join("library"))));
    assert_eq!(a.ui.library.held_at, None, "the note held is gone");
    a.left_release();
    assert_ne!(
        a.docs.active_doc().and_then(|d| d.path.clone()),
        Some(paths[2].clone()),
        "a note no longer listed is not opened"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}
