//! The notes pane by keyboard, a note's folder on its row, the empty states with their buttons,
//! and "Show this file in the notes library".

use super::files_tests::{open_now, scratch, type_text};
use super::library_write_tests::{app_in, labels, lib, pick, press, read, right_click_at, toast};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use crate::library_pane::Kind;
use crate::ui::{LibAct, PopupKind};

fn cur_label(a: &App) -> Option<String> {
    a.ui.library
        .cur
        .and_then(|c| a.ui.library.rows.get(c))
        .map(|r| r.label.clone())
}

fn letter(a: &mut App, ch: char, typed: &str) {
    a.key_pressed(Key::Name(KeyName::Char(ch)), Some(typed));
}

/// The pane drawn, and the keyboard in it.
fn focus(a: &mut App) {
    a.compose();
    assert!(a.library_focus(), "the pane takes the keyboard");
    a.compose();
}

fn ctrl_shift_3(a: &mut App) {
    a.mods = Mods {
        ctrl: true,
        shift: true,
        ..Mods::default()
    };
    a.key_pressed(Key::Name(KeyName::Char('3')), None);
    a.mods = Mods::default();
}

fn text_len(a: &App) -> usize {
    a.docs
        .active_ready()
        .map_or(0, |r| r.buffer.snapshot().len_chars())
}

/// The pixels of `r` as drawn now, to tell one drawing from another.
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

// ---- focus -------------------------------------------------------------------------------------

#[test]
fn the_shortcut_gives_a_showing_pane_the_keyboard_and_closes_it_when_it_has_it_already() {
    let (mut a, l) = lib("libk-shortcut");
    a.compose();
    assert!(a.ui.panes.files && a.ui.library.on && !a.ui.library.focused);
    ctrl_shift_3(&mut a);
    assert!(a.ui.panes.files, "the pane stays");
    assert!(a.ui.library.focused && a.ui.library.cur.is_some());
    ctrl_shift_3(&mut a);
    assert!(
        !a.ui.panes.files && !a.ui.library.focused,
        "the second press closes it"
    );
    ctrl_shift_3(&mut a);
    assert!(a.ui.panes.files && a.ui.library.on, "and the next shows it");
    assert!(
        !a.ui.library.focused,
        "showing it does not take the keyboard"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_cursor_starts_on_the_open_note_else_on_the_first_row() {
    let (mut a, l) = lib("libk-start");
    focus(&mut a);
    assert_eq!(cur_label(&a).as_deref(), Some("Personal"));
    a.ui.library.focused = false;
    a.ui.library.cur = None;
    open_now(&mut a, &l.last);
    focus(&mut a);
    assert_eq!(cur_label(&a).as_deref(), Some("Last year"));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_pane_takes_keys_only_while_it_has_the_keyboard_and_escape_gives_it_back() {
    let (mut a, l) = lib("libk-escape");
    open_now(&mut a, &l.last);
    a.compose();
    let before = text_len(&a);
    // Without the keyboard, a letter is text and Down is the caret's.
    letter(&mut a, 'x', "x");
    assert_eq!(text_len(&a), before + 1);
    assert_eq!(a.ui.library.cur, None, "the pane did not move");

    focus(&mut a);
    assert!(!a.text_focused());
    let (len, caret) = (text_len(&a), a.docs.active_ready().unwrap().view.sel.caret);
    letter(&mut a, 'x', "x");
    press(&mut a, KeyName::Backspace);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Right);
    press(&mut a, KeyName::Enter);
    assert_eq!(text_len(&a), len, "nothing reached the text");
    assert_eq!(a.docs.active_ready().unwrap().view.sel.caret, caret);

    press(&mut a, KeyName::Escape);
    assert!(
        !a.ui.library.focused && a.text_focused(),
        "Escape gives it back"
    );
    letter(&mut a, 'x', "x");
    assert_eq!(text_len(&a), len + 1);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_click_on_a_heading_keeps_the_keyboard_in_the_pane_and_a_click_on_a_note_gives_it_to_the_text()
{
    let (mut a, l) = lib("libk-click");
    a.compose();
    a.activate(Hit::LibraryRow(1));
    assert!(
        a.ui.library.focused,
        "a heading folds and the pane keeps the keys"
    );
    assert_eq!(cur_label(&a).as_deref(), Some("Money"));
    a.activate(Hit::LibraryRow(1));
    a.compose();
    a.activate(Hit::LibraryRow(2));
    assert!(
        !a.ui.library.focused,
        "a note opens and the text has the keys"
    );
    assert_eq!(cur_label(&a).as_deref(), Some("Taxes 2026"));
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()).as_deref(),
        Some(l.taxes.as_path())
    );
    // A press anywhere else takes the keyboard away.
    focus(&mut a);
    let open = a.hit_rect(Hit::Bar(crate::ui::BarItem::Open)).unwrap();
    a.ui.mouse = (open.cx(), open.cy());
    a.left_press();
    assert!(!a.ui.library.focused);
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- moving ------------------------------------------------------------------------------------

#[test]
fn up_down_home_end_and_the_page_keys_move_the_cursor_and_stop_at_the_ends() {
    let (mut a, l) = lib("libk-move");
    focus(&mut a);
    let at = |a: &App| cur_label(a).unwrap();
    press(&mut a, KeyName::Down);
    assert_eq!(at(&a), "Money");
    press(&mut a, KeyName::Down);
    assert_eq!(at(&a), "Taxes 2026");
    press(&mut a, KeyName::Up);
    assert_eq!(at(&a), "Money");
    press(&mut a, KeyName::End);
    assert_eq!(at(&a), "Trips");
    press(&mut a, KeyName::Down);
    assert_eq!(at(&a), "Trips", "no going round");
    press(&mut a, KeyName::Home);
    assert_eq!(at(&a), "Personal");
    press(&mut a, KeyName::Up);
    assert_eq!(at(&a), "Personal");
    press(&mut a, KeyName::PageDown);
    assert_eq!(at(&a), "Trips", "a page is more than this library has");
    press(&mut a, KeyName::PageUp);
    assert_eq!(at(&a), "Personal");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn left_folds_or_goes_to_the_parent_and_right_opens_or_goes_to_the_first_child() {
    let (mut a, l) = lib("libk-fold");
    focus(&mut a);
    let at = |a: &App| cur_label(a).unwrap();
    // Left on an open group shuts it, and on a shut one has nowhere to go.
    press(&mut a, KeyName::Left);
    assert_eq!((a.ui.library.rows.len(), at(&a).as_str()), (1, "Personal"));
    press(&mut a, KeyName::Left);
    assert_eq!(at(&a), "Personal");
    // Right opens it, then goes in.
    press(&mut a, KeyName::Right);
    assert_eq!((a.ui.library.rows.len(), at(&a).as_str()), (6, "Personal"));
    press(&mut a, KeyName::Right);
    assert_eq!(at(&a), "Money");
    press(&mut a, KeyName::Right);
    assert_eq!(at(&a), "Taxes 2026");
    press(&mut a, KeyName::Right);
    assert_eq!(at(&a), "Taxes 2026", "a note has nothing inside");
    // Left on a note goes up to its heading; on the open heading it shuts it; again, up.
    press(&mut a, KeyName::Left);
    assert_eq!(at(&a), "Money");
    press(&mut a, KeyName::Left);
    assert_eq!((a.ui.library.rows.len(), at(&a).as_str()), (3, "Money"));
    press(&mut a, KeyName::Left);
    assert_eq!(at(&a), "Personal");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_typed_letter_jumps_to_the_next_row_starting_with_it_going_round() {
    let (mut a, l) = lib("libk-letters");
    focus(&mut a);
    letter(&mut a, 't', "t");
    assert_eq!(cur_label(&a).as_deref(), Some("Taxes 2026"));
    letter(&mut a, 't', "T");
    assert_eq!(cur_label(&a).as_deref(), Some("Trips"));
    letter(&mut a, 't', "t");
    assert_eq!(cur_label(&a).as_deref(), Some("Taxes 2026"), "round again");
    letter(&mut a, 'z', "z");
    assert_eq!(
        cur_label(&a).as_deref(),
        Some("Taxes 2026"),
        "no row with z"
    );
    letter(&mut a, 'l', "l");
    assert_eq!(cur_label(&a).as_deref(), Some("Last year"));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_cursor_row_is_drawn_while_the_pane_has_the_keyboard_and_only_then() {
    let (mut a, l) = lib("libk-drawn");
    a.compose();
    a.ui.library.set_cur(2);
    let row = a.hit_rect(Hit::LibraryRow(2)).expect("the row is drawn");
    let quiet = pixels(&mut a, row);
    a.ui.library.focused = true;
    let lit = pixels(&mut a, row);
    assert!(quiet != lit, "the focused cursor shows on its row");
    let other = a.hit_rect(Hit::LibraryRow(3)).unwrap();
    let other_lit = pixels(&mut a, other);
    a.ui.library.focused = false;
    assert!(pixels(&mut a, other) == other_lit, "no other row changes");
    assert!(
        pixels(&mut a, row) == quiet,
        "and it is gone with the keyboard"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- what the keys do --------------------------------------------------------------------------

#[test]
fn enter_opens_a_note_and_hands_the_keyboard_to_the_text_and_folds_a_heading() {
    let (mut a, l) = lib("libk-enter");
    focus(&mut a);
    press(&mut a, KeyName::Enter);
    assert_eq!(a.ui.library.rows.len(), 1, "Enter on a group folds it");
    assert!(a.ui.library.focused, "and the pane keeps the keys");
    press(&mut a, KeyName::Enter);
    assert_eq!(a.ui.library.rows.len(), 6, "again opens it");
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Enter);
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()).as_deref(),
        Some(l.taxes.as_path())
    );
    assert!(
        !a.ui.library.focused,
        "the note opened: the text has the keys"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn enter_on_a_note_that_is_gone_says_so_and_keeps_the_keyboard() {
    let (mut a, l) = lib("libk-gone");
    std::fs::remove_file(&l.taxes).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    focus(&mut a);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Enter);
    assert!(
        toast(&a).contains("not where the library says"),
        "{}",
        toast(&a)
    );
    assert!(a.ui.library.focused);
    assert!(a.docs.tabs.is_empty());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn f2_renames_the_cursor_row_in_the_box_and_escape_in_the_box_leaves_the_pane_focused() {
    let (mut a, l) = lib("libk-f2");
    focus(&mut a);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::F(2));
    let edit = a.ui.library.edit.clone().expect("the box opened");
    assert_eq!(edit.kind, crate::library_pane::EditKind::RenameNote);
    assert_eq!(a.ui.library.field.text(), "Taxes 2026");
    press(&mut a, KeyName::Escape);
    assert!(
        a.ui.library.edit.is_none() && a.ui.library.focused,
        "back in the pane"
    );
    assert_eq!(read(&l.file), l.text());
    press(&mut a, KeyName::F(2));
    press(&mut a, KeyName::End);
    type_text(&mut a, "!");
    press(&mut a, KeyName::Enter);
    assert!(read(&l.file).contains("[Taxes 2026!]"), "{}", read(&l.file));
    assert_eq!(
        cur_label(&a).as_deref(),
        Some("Taxes 2026!"),
        "the cursor stays on it"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn renaming_a_section_by_f2_keeps_the_cursor_on_it_under_its_new_name() {
    let (mut a, l) = lib("libk-f2-section");
    focus(&mut a);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::F(2));
    name_it_over(&mut a, "Cash");
    assert_eq!(cur_label(&a).as_deref(), Some("Cash"));
    let _ = std::fs::remove_dir_all(&l.dir);
}

fn name_it_over(a: &mut App, name: &str) {
    a.ui.library.field.set_text("");
    type_text(a, name);
    press(a, KeyName::Enter);
}

#[test]
fn delete_takes_a_note_out_of_the_library_and_asks_for_a_section_and_leaves_a_group() {
    let (mut a, l) = lib("libk-delete");
    focus(&mut a);
    // A group: nothing happens (its file is deleted from its menu, after asking).
    press(&mut a, KeyName::Delete);
    assert!(a.ui.dialog.is_none() && a.ui.popup.is_none());
    assert_eq!(read(&l.file), l.text());
    // A section asks.
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Delete);
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(crate::ui::DialogKind::LibraryRemove)
    );
    press(&mut a, KeyName::Escape);
    assert_eq!(read(&l.file), l.text());
    // A note comes out of the file; the note stays.
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Delete);
    assert!(!read(&l.file).contains("Taxes 2026"), "{}", read(&l.file));
    assert!(l.taxes.exists());
    assert!(a.ui.library.focused, "the pane keeps the keys");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn shift_f10_and_the_menu_key_open_the_cursor_rows_menu_by_the_keyboard() {
    let (mut a, l) = lib("libk-menu");
    focus(&mut a);
    press(&mut a, KeyName::Down);
    a.mods = Mods {
        shift: true,
        ..Mods::default()
    };
    press(&mut a, KeyName::F(10));
    a.mods = Mods::default();
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::LibraryRow(1)));
    assert!(
        a.ui.popup.as_ref().unwrap().cues,
        "opened by a key: the letters show"
    );
    a.compose();
    let row = a.hit_rect(Hit::LibraryRow(1)).unwrap();
    let menu = a.ui.popup.as_ref().unwrap().menu.rect();
    assert!(
        menu.y >= row.y && menu.y <= row.bottom() + 8.0,
        "under the row: {menu:?} {row:?}"
    );
    press(&mut a, KeyName::Escape);
    assert!(a.ui.popup.is_none());
    press(&mut a, KeyName::Down);
    a.key_pressed(Key::Menu, None);
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::LibraryRow(2)));
    // A letter of the menu runs its item: "Rename in the library".
    letter(&mut a, 'r', "r");
    assert!(a.ui.library.edit.is_some());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_shortcut_still_works_while_the_pane_has_the_keyboard() {
    let (mut a, l) = lib("libk-shortcuts");
    focus(&mut a);
    a.mods = Mods {
        ctrl: true,
        ..Mods::default()
    };
    a.key_pressed(Key::Name(KeyName::Char('n')), None);
    a.mods = Mods::default();
    assert!(
        a.docs.tabs.iter().any(|d| d.path.is_none()),
        "Ctrl+N made a file"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- the cursor and a library read again -------------------------------------------------------

#[test]
fn the_cursor_stays_on_the_same_note_when_the_library_is_read_again() {
    let (mut a, l) = lib("libk-reread");
    focus(&mut a);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Down);
    assert_eq!(a.ui.library.cur, Some(2));
    // A note above it, written by another window.
    let new = l.dir.join("aardvark.md");
    std::fs::write(&new, "# Aardvark\n").unwrap();
    let text = l.text().replacen(
        "## Money\n\n",
        &format!(
            "## Money\n\n- [Aardvark]({})\n",
            super::library_write_tests::slash(&new)
        ),
        1,
    );
    std::fs::write(&l.file, text).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert_eq!(labels(&a)[2], "Aardvark");
    assert_eq!(a.ui.library.cur, Some(3), "not the same index");
    assert_eq!(
        cur_label(&a).as_deref(),
        Some("Taxes 2026"),
        "the same note"
    );
    // The note is gone: the cursor goes to the heading it was under.
    std::fs::write(
        &l.file,
        l.text().replace(
            &format!(
                "- [Taxes 2026]({})\n",
                super::library_write_tests::slash(&l.taxes)
            ),
            "",
        ),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert_eq!(cur_label(&a).as_deref(), Some("Money"));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_cursor_goes_to_the_heading_when_a_click_folds_what_it_was_in() {
    let (mut a, l) = lib("libk-hidden");
    focus(&mut a);
    for _ in 0..4 {
        press(&mut a, KeyName::Down);
    }
    assert_eq!(cur_label(&a).as_deref(), Some("Last year"));
    // The heading is folded from elsewhere (the pane's own click would move the cursor to it).
    let money = a.ui.library.rows[1].key.clone();
    a.ui.library.fold(&money);
    a.ui.library.build(&a.docs.library);
    assert!(a.ui.library.rows.iter().all(|r| r.label != "Last year"));
    assert_eq!(cur_label(&a).as_deref(), Some("Money"));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn moving_the_cursor_scrolls_the_pane_to_keep_it_in_view() {
    let (mut a, l) = lib("libk-scroll");
    let mut text = String::from("# Long\n\n");
    for n in 0..80 {
        let note = l.dir.join(format!("n{n:02}.md"));
        std::fs::write(&note, format!("# Note {n}\n")).unwrap();
        text.push_str(&format!(
            "- [Note {n:02}]({})\n",
            super::library_write_tests::slash(&note)
        ));
    }
    std::fs::write(l.dir.join("library").join("long.md"), text).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    focus(&mut a);
    let area = crate::library_pane::list(a.areas.files.unwrap());
    assert!(a.ui.library.content > area.h, "more rows than fit");
    assert_eq!(a.ui.library.scroll, 0.0);
    press(&mut a, KeyName::End);
    a.compose();
    let last = a.ui.library.rows.len() - 1;
    assert!(a.ui.library.scroll > 0.0, "scrolled");
    let r = a
        .hit_rect(Hit::LibraryRow(last))
        .expect("the last row is on screen");
    assert!(
        r.bottom() <= area.bottom() + 0.5,
        "inside the pane: {r:?} {area:?}"
    );
    press(&mut a, KeyName::Home);
    a.compose();
    assert_eq!(a.ui.library.scroll, 0.0);
    // One row at a time, the cursor's row is always inside the pane.
    for _ in 0..40 {
        press(&mut a, KeyName::Down);
        a.compose();
        let c = a.ui.library.cur.unwrap();
        let r = a
            .hit_rect(Hit::LibraryRow(c))
            .expect("the cursor row is on screen");
        assert!(
            r.y >= area.y - 0.5 && r.bottom() <= area.bottom() + 0.5,
            "row {c}: {r:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- a note's folder ---------------------------------------------------------------------------

/// How many pixels of `r` are not the colour at its corner: ink.
fn ink(a: &mut App, r: Rect) -> usize {
    let p = pixels(a, r);
    let bg = [p[0], p[1], p[2]];
    p.chunks(4).filter(|px| px[..3] != bg).count()
}

#[test]
fn a_note_s_folder_is_drawn_dimmed_at_the_right_of_its_row_unless_the_pane_is_narrow() {
    let (mut a, l) = lib("libk-folder");
    for (w, shown) in [(300.0, true), (211.0, true), (209.0, false), (180.0, false)] {
        a.ui.panes.files_w = w;
        a.compose();
        let row = a.hit_rect(Hit::LibraryRow(2)).expect("the note's row");
        let pane = a.areas.files.unwrap();
        assert!((pane.w - w).abs() < 1.0, "the pane is {w} wide: {pane:?}");
        // The right end of the row, where the folder is: nothing else is drawn there.
        let end = Rect::new(row.right() - 28.0, row.y + 6.0, 14.0, row.h - 12.0);
        assert_eq!(ink(&mut a, end) > 4, shown, "pane {w}");
        // The title is where it was.
        let title = Rect::new(row.x + 60.0, row.y + 6.0, 40.0, row.h - 12.0);
        assert!(ink(&mut a, title) > 4, "pane {w}: the title");
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn two_notes_with_the_same_title_are_told_apart_by_their_folders() {
    let (mut a, l) = lib("libk-folder-twins");
    let a1 = l.dir.join("one");
    let a2 = l.dir.join("two");
    std::fs::create_dir_all(&a1).unwrap();
    std::fs::create_dir_all(&a2).unwrap();
    std::fs::write(a1.join("taxes.md"), "# Taxes\n").unwrap();
    std::fs::write(a2.join("taxes.md"), "# Taxes\n").unwrap();
    let text = format!(
        "# Twins\n\n- [Taxes]({})\n- [Taxes]({})\n",
        super::library_write_tests::slash(&a1.join("taxes.md")),
        super::library_write_tests::slash(&a2.join("taxes.md"))
    );
    std::fs::write(l.dir.join("library").join("twins.md"), text).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    a.compose();
    let at =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.label == "Taxes" && r.depth == 1)
            .unwrap();
    let (r1, r2) = (
        a.hit_rect(Hit::LibraryRow(at)).unwrap(),
        a.hit_rect(Hit::LibraryRow(at + 1)).unwrap(),
    );
    let end = |r: Rect| Rect::new(r.right() - 46.0, r.y + 6.0, 34.0, r.h - 12.0);
    let (p1, p2) = (pixels(&mut a, end(r1)), pixels(&mut a, end(r2)));
    assert!(p1 != p2, "'one' and 'two' do not draw alike");
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- the empty states --------------------------------------------------------------------------

#[test]
fn with_no_library_folder_the_pane_has_one_button_that_starts_a_group() {
    let dir = scratch("libk-nofolder");
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    a.compose();
    assert!(a.ui.library.rows.is_empty());
    assert_eq!(
        crate::library_pane::empty_state(&a.docs.library.dir),
        crate::library_pane::Empty::NoFolder
    );
    let b = a
        .hit_rect(Hit::LibraryNewGroup)
        .expect("New group… is a button");
    assert!(
        a.hit_rect(Hit::LibraryAddOpen).is_none(),
        "no second button here"
    );
    // Not even with a file open, which is what enables it in an empty library.
    let note = dir.join("receipts.md");
    std::fs::write(&note, "# Receipts\n").unwrap();
    open_now(&mut a, &note);
    a.compose();
    assert!(
        a.hit_rect(Hit::LibraryAddOpen).is_none(),
        "no second button here, with a file open"
    );
    assert!(a.areas.files.unwrap().contains(b.cx(), b.cy()));
    a.activate(Hit::LibraryNewGroup);
    let edit = a.ui.library.edit.as_ref().expect("the name box opened");
    assert_eq!(edit.kind, crate::library_pane::EditKind::NewGroup);
    type_text(&mut a, "Home");
    press(&mut a, KeyName::Enter);
    assert!(
        dir.join("library").join("Home.md").exists(),
        "the folder and its file were made"
    );
    assert_eq!(labels(&a), vec!["Home"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_a_library_folder_and_no_notes_the_pane_has_two_buttons_the_second_needing_a_file() {
    let dir = scratch("libk-nonotes");
    std::fs::create_dir_all(dir.join("library")).unwrap();
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    a.compose();
    assert_eq!(
        crate::library_pane::empty_state(&a.docs.library.dir),
        crate::library_pane::Empty::NoNotes
    );
    assert!(a.hit_rect(Hit::LibraryNewGroup).is_some());
    let add_at = |a: &mut App| {
        a.compose();
        a.hit_rect(Hit::LibraryAddOpen)
    };
    assert!(add_at(&mut a).is_none(), "disabled with no document open");

    let note = dir.join("receipts.md");
    std::fs::write(&note, "# Receipts\n").unwrap();
    open_now(&mut a, &note);
    assert!(add_at(&mut a).is_some(), "enabled with a file open");
    a.activate(Hit::LibraryAddOpen);
    assert!(a.palette_open(), "it asks where the file goes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_empty_pane_looks_different_from_the_full_one_and_the_disabled_button_from_the_enabled() {
    let dir = scratch("libk-empty-look");
    std::fs::create_dir_all(dir.join("library")).unwrap();
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let button = |a: &mut App| {
        a.compose();
        let pane = a.areas.files.unwrap();
        let new = a.hit_rect(Hit::LibraryNewGroup).unwrap();
        // The second button stands below the first.
        let second = Rect::new(new.x, new.bottom() + 8.0, new.w, new.h);
        (pane, second)
    };
    let (_, second) = button(&mut a);
    let off = pixels(&mut a, second);
    let note = dir.join("receipts.md");
    std::fs::write(&note, "# Receipts\n").unwrap();
    open_now(&mut a, &note);
    let (_, second) = button(&mut a);
    let on = pixels(&mut a, second);
    assert!(
        off != on,
        "Add this file is drawn disabled until a file is open"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- show this file in the notes library -------------------------------------------------------

#[test]
fn show_this_file_opens_the_notes_pane_down_to_the_note_and_puts_the_cursor_on_it() {
    let (mut a, l) = lib("libk-show");
    open_now(&mut a, &l.last);
    // The pane shows something else, and the group is folded.
    a.compose();
    a.activate(Hit::LibraryRow(0));
    assert_eq!(a.ui.library.rows.len(), 1);
    a.command(C::ToggleFiles);
    assert!(!a.ui.library.on);

    a.command(C::ShowInLibrary);
    assert!(
        a.ui.panes.files && a.ui.library.on,
        "the pane shows the notes"
    );
    assert_eq!(a.ui.library.rows.len(), 6, "opened down to the note");
    assert_eq!(cur_label(&a).as_deref(), Some("Last year"));
    assert!(!a.ui.library.focused, "the text keeps the keyboard");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn show_this_file_scrolls_the_pane_to_a_note_far_down() {
    let (mut a, l) = lib("libk-show-scroll");
    let mut text = String::from("# Zlong\n\n");
    for n in 0..80 {
        let note = l.dir.join(format!("n{n:02}.md"));
        std::fs::write(&note, format!("# Note {n}\n")).unwrap();
        text.push_str(&format!(
            "- [Note {n:02}]({})\n",
            super::library_write_tests::slash(&note)
        ));
    }
    std::fs::write(l.dir.join("library").join("zlong.md"), text).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    open_now(&mut a, &l.dir.join("n70.md"));
    a.command(C::ShowInLibrary);
    a.compose();
    let c = a.ui.library.cur.expect("a cursor");
    assert_eq!(a.ui.library.rows[c].label, "Note 70");
    let area = crate::library_pane::list(a.areas.files.unwrap());
    let r = a
        .hit_rect(Hit::LibraryRow(c))
        .expect("its row is on screen");
    assert!(
        r.y >= area.y - 0.5 && r.bottom() <= area.bottom() + 0.5,
        "{r:?} {area:?}"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn show_this_file_for_a_file_the_library_does_not_list_says_so() {
    let (mut a, l) = lib("libk-show-unlisted");
    let other = l.dir.join("other.md");
    std::fs::write(&other, "# Other\n").unwrap();
    open_now(&mut a, &other);
    a.command(C::ShowInLibrary);
    assert!(
        toast(&a).contains("not in the notes library"),
        "{}",
        toast(&a)
    );
    assert_eq!(a.ui.library.cur, None);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_tab_menu_and_the_palette_offer_it_only_for_a_file_the_library_lists() {
    let dir = scratch("libk-show-menu");
    let lib_dir = dir.join("library");
    std::fs::create_dir_all(&lib_dir).unwrap();
    let listed = dir.join("listed.md");
    let other = dir.join("other.md");
    std::fs::write(&listed, "# Listed\n").unwrap();
    std::fs::write(&other, "# Other\n").unwrap();
    std::fs::write(
        lib_dir.join("g.md"),
        format!(
            "# G\n\n- [Listed]({})\n",
            super::library_write_tests::slash(&listed)
        ),
    )
    .unwrap();
    let mut a = app_in(&dir);
    open_now(&mut a, &listed);
    open_now(&mut a, &other);
    assert!(
        a.docs.library.groups.is_empty(),
        "the library was not read at the start"
    );

    // A right-click on a tab reads it, then the item is there for the listed file only.
    a.compose();
    let item = |a: &App, i: usize| {
        a.ui.popup
            .as_ref()
            .unwrap()
            .menu
            .items
            .iter()
            .find(|m| m.id == Act::TabCmd(C::ShowInLibrary, i))
            .map(|m| m.enabled)
    };
    let t0 = a.hit_rect(Hit::Tab(0)).unwrap();
    right_click_at(&mut a, t0.cx(), t0.cy());
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::Tab(0)));
    assert_eq!(item(&a, 0), Some(true), "the listed file");
    a.ui.popup = None;
    let t1 = a.hit_rect(Hit::Tab(1)).unwrap();
    right_click_at(&mut a, t1.cx(), t1.cy());
    assert_eq!(item(&a, 1), Some(false), "a file the library does not list");
    a.ui.popup = None;

    // The item runs for the tab it was opened on, not the one in front.
    a.docs.activate(1);
    a.choose(Act::TabCmd(C::ShowInLibrary, 0), false);
    assert_eq!(a.docs.active(), Some(0));
    assert!(a.ui.library.on && cur_label(&a).as_deref() == Some("Listed"));

    // The palette lists the command by the same rule.
    a.docs.activate(1);
    assert!(!crate::palette::enabled(
        C::ShowInLibrary,
        &a.docs,
        &a.ui,
        false
    ));
    a.docs.activate(0);
    assert!(crate::palette::enabled(
        C::ShowInLibrary,
        &a.docs,
        &a.ui,
        false
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_palette_reads_the_library_when_it_opens_so_the_command_is_listed_right() {
    let dir = scratch("libk-show-palette");
    let lib_dir = dir.join("library");
    std::fs::create_dir_all(&lib_dir).unwrap();
    let listed = dir.join("listed.md");
    std::fs::write(&listed, "# Listed\n").unwrap();
    std::fs::write(
        lib_dir.join("g.md"),
        format!(
            "# G\n\n- [Listed]({})\n",
            super::library_write_tests::slash(&listed)
        ),
    )
    .unwrap();
    let mut a = app_in(&dir);
    open_now(&mut a, &listed);
    assert!(a.docs.library.groups.is_empty());
    a.open_palette(">");
    assert_eq!(a.docs.library.groups.len(), 1);
    assert!(crate::palette::enabled(
        C::ShowInLibrary,
        &a.docs,
        &a.ui,
        false
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_kind_check_the_rows_know_their_notes_after_the_cursor_moved_onto_one() {
    let (mut a, l) = lib("libk-kind");
    focus(&mut a);
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Down);
    let c = a.ui.library.cur.unwrap();
    assert!(matches!(a.ui.library.rows[c].kind, Kind::Note { .. }));
    pick(&mut a, LibAct::Remove, c);
    assert!(
        a.ui.library.cur.is_some(),
        "still a cursor after the row went"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- a library that writes its notes as paths from its own folder ------------------------------

/// A library whose links are `../notes/a.md`, as a hand-written library file has them: the
/// path is not the one the tab was opened by until its `..` is folded away.
fn relative_lib(tag: &str) -> (App, PathBuf, PathBuf) {
    let dir = scratch(tag);
    std::fs::create_dir_all(dir.join("library")).unwrap();
    std::fs::create_dir_all(dir.join("notes")).unwrap();
    let note = dir.join("notes").join("taxes.md");
    std::fs::write(&note, "# Taxes\n").unwrap();
    std::fs::write(
        dir.join("library").join("personal.md"),
        "# Personal\n\n## Money\n\n- [Taxes](../notes/taxes.md)\n",
    )
    .unwrap();
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    (a, dir, note)
}

#[test]
fn a_note_linked_with_dots_in_its_path_is_the_open_note() {
    let (mut a, dir, note) = relative_lib("libk-relative");
    open_now(&mut a, &note);
    a.compose();
    // It is the note that is open: lit in the pane, where the keyboard starts, and listed.
    let at =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.label == "Taxes")
            .unwrap();
    let row = a.hit_rect(Hit::LibraryRow(at)).unwrap();
    let lit = pixels(&mut a, row);
    open_now(&mut a, &dir.join("library").join("personal.md"));
    a.compose();
    assert!(
        pixels(&mut a, row) != lit,
        "the open note's row is lit and the other file's is not"
    );
    a.docs.activate(0);
    a.ui.library.cur = None;
    focus(&mut a);
    assert_eq!(
        cur_label(&a).as_deref(),
        Some("Taxes"),
        "the keyboard starts on it"
    );
    assert!(crate::palette::enabled(
        C::ShowInLibrary,
        &a.docs,
        &a.ui,
        false
    ));
    a.command(C::ShowInLibrary);
    assert_eq!(cur_label(&a).as_deref(), Some("Taxes"));
    assert!(
        !toast(&a).contains("not in the notes library"),
        "{}",
        toast(&a)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn folding_dots_out_of_a_path_is_by_its_text_and_keeps_other_files_apart() {
    use crate::library_pane::same_note;
    use std::path::Path;
    assert!(same_note(
        Path::new("D:/lib/../notes/a.md"),
        Path::new("D:/notes/a.md")
    ));
    assert!(same_note(
        Path::new("D:/notes/./a.md"),
        Path::new("D:/notes/a.md")
    ));
    assert!(!same_note(
        Path::new("D:/lib/../notes/a.md"),
        Path::new("D:/notes/b.md")
    ));
    assert!(!same_note(
        Path::new("D:/lib/notes/a.md"),
        Path::new("D:/notes/a.md")
    ));
    assert!(!same_note(
        Path::new("D:/a/b/../../c.md"),
        Path::new("D:/a/c.md")
    ));
    // A path that climbs above where it starts keeps the climb.
    assert!(!same_note(Path::new("../x/a.md"), Path::new("x/a.md")));
    assert!(same_note(Path::new("x/../a.md"), Path::new("a.md")));
}
