//! The notes library in the left pane: what the commands show there, folding a row, and opening
//! a note from it.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::commands::CommandId as C;
use crate::library_pane::Kind;
use crate::ui::{Hit, LibAct};

fn app_in(dir: &Path) -> App {
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a
}

/// A library folder with one group, a section and a section inside it.
fn library_in(dir: &Path) -> (PathBuf, PathBuf) {
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let taxes = dir.join("taxes.md");
    let last = dir.join("last.md");
    std::fs::write(&taxes, "# Taxes 2026\n").unwrap();
    std::fs::write(&last, "# Last year\n").unwrap();
    std::fs::write(
        lib.join("personal.md"),
        format!(
            "# Personal\n\n## Money\n\n- {}\n\n### 2025\n\n- {}\n",
            taxes.to_string_lossy().replace('\\', "/"),
            last.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    (taxes, last)
}

fn shown(a: &App) -> Vec<(u16, String)> {
    a.ui.library
        .rows
        .iter()
        .map(|r| (r.depth, r.label.clone()))
        .collect()
}

/// The loop's passes until the read that runs on its own thread has been taken in. The wake a
/// job sends is a no-op in tests (the waker `app()` makes), so the test's own passes are its
/// wake; the real loop is woken by `Wake::Library`.
fn wait_for_read(a: &mut App) {
    let end = Instant::now() + std::time::Duration::from_secs(10);
    while a.library_job.is_some() {
        assert!(Instant::now() < end, "the library read did not finish");
        std::thread::sleep(std::time::Duration::from_millis(1));
        a.library_tick(Instant::now());
    }
}

#[test]
fn the_pane_shows_the_library_as_a_tree_and_folds_a_section() {
    let dir = scratch("libpane-tree");
    let (taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);

    a.command(C::ToggleLibraryPane);
    assert!(a.ui.panes.files && a.ui.library.on, "the pane shows it");
    assert!(!a.ui.outline.on, "not the function list");
    assert_eq!(
        shown(&a),
        vec![
            (0, "Personal".to_string()),
            (1, "Money".to_string()),
            (2, "Taxes 2026".to_string()),
            (2, "2025".to_string()),
            (3, "Last year".to_string()),
        ]
    );

    // Folding the section hides its note and the section inside it.
    a.activate(Hit::LibraryRow(1));
    assert_eq!(
        shown(&a),
        vec![(0, "Personal".to_string()), (1, "Money".to_string())]
    );
    a.activate(Hit::LibraryRow(1));
    assert_eq!(shown(&a).len(), 5, "open again");

    // A note opens in a tab.
    a.activate(Hit::LibraryRow(2));
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()).as_deref(),
        Some(taxes.as_path())
    );

    // The command again hides the pane; the Files tree has its own command.
    a.command(C::ToggleLibraryPane);
    assert!(!a.ui.panes.files);
    a.command(C::ToggleFiles);
    assert!(a.ui.panes.files && !a.ui.library.on);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_group_with_a_folder_shows_it_on_its_row_as_the_home_line_writes_it() {
    let dir = scratch("libpane-home");
    let lib = dir.join("library");
    std::fs::create_dir_all(lib.join("Japan 2025")).unwrap();
    let outside = dir.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    let tokyo = dir.join("tokyo.md");
    let plans = dir.join("plans.md");
    let plain = dir.join("plain.md");
    for note in [&tokyo, &plans, &plain] {
        std::fs::write(note, "# A note\n").unwrap();
    }
    let link = |p: &Path| p.to_string_lossy().replace('\\', "/");
    std::fs::write(
        lib.join("japan.md"),
        format!("# Japan\nhome: Japan 2025\n\n- {}\n", link(&tokyo)),
    )
    .unwrap();
    std::fs::write(
        lib.join("work.md"),
        format!("# Work\nhome: {}\n\n- {}\n", link(&outside), link(&plans)),
    )
    .unwrap();
    std::fs::write(
        lib.join("plain.md"),
        format!("# Plain\n\n- {}\n", link(&plain)),
    )
    .unwrap();
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let home = |a: &App, group: &str| {
        a.ui.library
            .rows
            .iter()
            .find(|r| r.label == group && r.kind == Kind::Group)
            .and_then(|r| r.home.clone())
    };
    assert_eq!(
        home(&a, "Japan"),
        Some("Japan 2025".to_string()),
        "a folder inside the library folder reads as the line's relative form"
    );
    assert_eq!(
        home(&a, "Work"),
        Some(link(&outside)),
        "a folder outside it is the line's whole path"
    );
    assert_eq!(
        home(&a, "Plain"),
        None,
        "a group with no home line shows none"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_groups_new_folder_repaints_the_pane_its_row_is_in() {
    let dir = scratch("libpane-home-sig");
    let (_taxes, _last) = library_in(&dir);
    let file = dir.join("library").join("personal.md");
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    a.compose();
    let sig = |a: &App| a.region_sig(crate::ui::Region::Files, a.areas.files);
    let before = sig(&a);
    // The same group, the same rows, one new `home:` line: the folder now on its row is a
    // reason to draw the pane again, as a note's folder in the same place is.
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        text.replacen("# Personal\n", "# Personal\nhome: Personal\n", 1),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    a.compose();
    assert_ne!(
        before,
        sig(&a),
        "the frame signature must see the folder the group's row shows"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_note_that_is_gone_is_marked_and_says_so_instead_of_opening() {
    let dir = scratch("libpane-missing");
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(
        lib.join("notes.md"),
        format!(
            "# Notes\n\n- [Gone](<{}>)\n",
            dir.join("gone.md").to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    assert!(matches!(
        a.ui.library.rows[1].kind,
        Kind::Note { missing: true, .. }
    ));
    a.activate(Hit::LibraryRow(1));
    assert!(a.docs.tabs.is_empty(), "nothing was opened");
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("not where the library says")),
        "{:?}",
        a.docs.toast(Instant::now())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_library_file_saved_while_the_pane_shows_is_read_again() {
    let dir = scratch("libpane-reread");
    let (_taxes, _last) = library_in(&dir);
    let file = dir.join("library").join("personal.md");
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    assert_eq!(shown(&a).len(), 5);

    let other = dir.join("other.md");
    std::fs::write(&other, "# Another note\n").unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        format!(
            "{text}\n## Later\n\n- {}\n",
            other.to_string_lossy().replace('\\', "/")
        ),
    )
    .unwrap();

    // The pane looks at the folder again, but not before its own interval has passed.
    a.library_tick(Instant::now());
    assert_eq!(shown(&a).len(), 5, "too soon to look again");
    a.library_tick(Instant::now() + crate::library_pane::RECHECK);
    // The read runs on its own thread: the rows on screen stay until its result is ready.
    assert_eq!(
        shown(&a).len(),
        5,
        "the old rows stay while the new ones are read"
    );
    assert!(a.library_job.is_some(), "the read was started");
    wait_for_read(&mut a);
    assert_eq!(
        shown(&a).last().map(|(_, l)| l.as_str()),
        Some("Another note"),
        "{:?}",
        shown(&a)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn adding_the_open_file_from_the_pane_shows_it_there() {
    let dir = scratch("libpane-add");
    let (taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let note = dir.join("new.md");
    std::fs::write(&note, "# A new note\n").unwrap();
    open_now(&mut a, &note);
    let file = dir.join("library").join("personal.md");
    a.library_put(&file, &["Money".to_string()]);
    a.library_rows();
    let rows = shown(&a);
    assert!(
        rows.contains(&(2, "A new note".to_string())),
        "under Money: {rows:?}"
    );
    assert!(rows.contains(&(2, "Taxes 2026".to_string())));
    assert!(taxes.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_added_shows_in_the_tree_at_once_even_under_a_folded_group() {
    let dir = scratch("libpane-add-now");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    // The group is folded when the file is added.
    let file = dir.join("library").join("personal.md");
    let group = a.ui.library.rows[0].key.clone();
    a.ui.library.fold(&group);
    a.ui.library.build(&a.docs.library);
    let note = dir.join("new.md");
    std::fs::write(
        &note,
        "# A new note
",
    )
    .unwrap();
    open_now(&mut a, &note);
    a.library_put(&file, &[]);
    // No `library_rows()` here: the rows are what the pane has now.
    let rows = shown(&a);
    assert!(
        rows.iter().any(|(_, l)| l == "A new note"),
        "shown at once: {rows:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_file_added_shows_in_the_tree_at_once_even_under_a_folded_section() {
    let dir = scratch("libpane-add-now-section");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    // The section is folded when the file is added: its own note, the section inside it and
    // that section's note are all hidden by that fold.
    let file = dir.join("library").join("personal.md");
    assert_eq!(a.ui.library.rows[1].label, "Money");
    let section = a.ui.library.rows[1].key.clone();
    a.ui.library.fold(&section);
    a.ui.library.build(&a.docs.library);
    assert_eq!(
        shown(&a),
        vec![(0, "Personal".to_string()), (1, "Money".to_string())],
        "folded"
    );
    let note = dir.join("new.md");
    std::fs::write(&note, "# A new note\n").unwrap();
    open_now(&mut a, &note);
    a.library_put(&file, &["Money".to_string()]);
    // No `library_rows()` here: the rows are what the pane has now.
    let rows = shown(&a);
    assert!(
        rows.contains(&(2, "A new note".to_string())),
        "shown at once: {rows:?}"
    );
    assert!(
        rows.contains(&(2, "Taxes 2026".to_string())),
        "the section opened with it, and its other note is back: {rows:?}"
    );
    assert!(
        rows.contains(&(3, "Last year".to_string())),
        "and the section inside it: {rows:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_name_being_typed_when_a_file_is_added_goes_and_the_keyboard_returns_to_the_text() {
    use crate::library_pane::{Edit, EditKind};
    use crate::ui::BarField;
    let dir = scratch("libpane-add-now-name");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let file = dir.join("library").join("personal.md");
    let note = dir.join("new.md");
    std::fs::write(&note, "# A new note\n").unwrap();
    open_now(&mut a, &note);
    // A section's name is being typed in the pane's box when the file is added another way.
    a.ui.library.start_edit(
        Edit {
            kind: EditKind::RenameSection,
            at: 1,
            depth: 1,
            file: file.clone(),
            section: vec!["Money".to_string()],
            line: 0,
            note: PathBuf::new(),
        },
        "Money",
        &a.docs.library,
    );
    a.ui.focus_field(BarField::Library);
    assert!(
        a.ui.find_focused && a.ui.field == BarField::Library,
        "typing"
    );
    a.library_put(&file, &[]);
    assert!(a.ui.library.edit.is_none(), "the name went with the rows");
    assert!(
        !a.ui.find_focused && a.ui.field != BarField::Library,
        "the keyboard is back in the text, not in a box that is gone"
    );
    assert!(
        shown(&a).iter().any(|(_, l)| l == "A new note"),
        "and the note still shows: {:?}",
        shown(&a)
    );
    assert!(a.ui.library.rows.iter().all(|r| r.kind != Kind::Editing));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_head_switch_shows_its_face_and_the_face_already_shown_does_not_close_the_pane() {
    use crate::ui::PaneFace;
    let dir = scratch("libpane-switch");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleFiles);
    assert_eq!(a.ui.pane_face(), PaneFace::Files);

    a.activate(Hit::PaneFace(PaneFace::Notes));
    assert!(a.ui.panes.files && a.ui.library.on && !a.ui.outline.on);
    assert_eq!(shown(&a).len(), 5, "the library is read as it is shown");

    a.activate(Hit::PaneFace(PaneFace::Functions));
    assert!(a.ui.panes.files && a.ui.outline.on && !a.ui.library.on);
    assert_eq!(a.ui.pane_face(), PaneFace::Functions);

    // The same switch again leaves the pane as it is; the commands are what close it.
    a.activate(Hit::PaneFace(PaneFace::Functions));
    assert!(a.ui.panes.files && a.ui.outline.on);

    a.activate(Hit::PaneFace(PaneFace::Files));
    assert!(a.ui.panes.files && !a.ui.outline.on && !a.ui.library.on);
    assert_eq!(a.ui.pane_face(), PaneFace::Files);
    a.command(C::ToggleFiles);
    assert!(!a.ui.panes.files);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The window with the left pane `width` wide, composed once.
fn with_pane(width: f32, face: crate::ui::PaneFace) -> App {
    let mut a = app();
    a.ui.set_pane_face(face);
    a.ui.panes.files_w = width;
    a.compose();
    a
}

#[test]
fn the_three_switches_are_hit_targets_in_the_head_at_the_narrowest_pane_and_the_default() {
    use crate::ui::PaneFace;
    for (width, labelled) in [(crate::ui::PANE_MIN, false), (300.0, true)] {
        for shown in PaneFace::ALL {
            let a = with_pane(width, shown);
            let pane = a.areas.files.expect("the pane is on screen");
            assert_eq!(pane.w, width);
            let rects: Vec<Rect> = PaneFace::ALL
                .iter()
                .map(|f| {
                    a.hit_rect(Hit::PaneFace(*f))
                        .unwrap_or_else(|| panic!("{f:?} has no hit at {width} showing {shown:?}"))
                })
                .collect();
            for (f, r) in PaneFace::ALL.iter().zip(&rects) {
                assert!(
                    r.x >= pane.x && r.right() <= pane.right(),
                    "{f:?} inside the pane"
                );
                assert!(r.y >= pane.y && r.bottom() <= pane.y + mdshared_ui::pane::HEAD);
                assert_eq!(
                    a.hit_at(r.cx(), r.cy()),
                    Some(Hit::PaneFace(*f)),
                    "the middle of {f:?} at {width}"
                );
                assert_eq!(
                    r.w > mdshared_ui::pane::SWITCH_ICON_W,
                    labelled,
                    "{f:?} at {width}"
                );
            }
            assert!(rects[0].right() <= rects[1].x && rects[1].right() <= rects[2].x);
            // The button of the face shown is clear of the switches.
            let button = match shown {
                PaneFace::Files => Some(Hit::FilesFolder),
                PaneFace::Functions => None,
                PaneFace::Notes => Some(Hit::LibraryAdd),
            };
            for hit in [Hit::FilesFolder, Hit::LibraryAdd] {
                assert_eq!(
                    a.hit_rect(hit).is_some(),
                    button == Some(hit),
                    "{hit:?} on {shown:?}"
                );
            }
            if let Some(b) = button.and_then(|h| a.hit_rect(h)) {
                assert!(
                    rects[2].right() <= b.x,
                    "{shown:?} at {width}: {rects:?} {b:?}"
                );
                assert!(b.right() <= pane.right());
            }
        }
    }
}

#[test]
fn the_notes_head_button_opens_a_menu_whose_add_item_asks_where_to_add_the_open_file() {
    let dir = scratch("libpane-plus");
    let (_taxes, _last) = library_in(&dir);
    let note = dir.join("new.md");
    std::fs::write(&note, "# A new note\n").unwrap();
    let mut a = app_in(&dir);
    open_now(&mut a, &note);
    a.command(C::ToggleLibraryPane);
    a.compose();
    a.activate(Hit::LibraryAdd);
    assert_eq!(a.ui.popup_kind(), Some(crate::ui::PopupKind::LibraryEmpty));
    assert!(!a.palette_open(), "the menu comes first");
    a.choose(Act::Cmd(C::LibraryAdd), false);
    assert!(a.palette_open(), "it asks where the open file goes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_bar_has_a_notes_button_that_shows_the_library_and_hides_it_again() {
    use crate::ui::BarItem;
    let dir = scratch("libpane-bar");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.compose();
    let files = a.hit_rect(Hit::Bar(BarItem::Files)).expect("Files button");
    let notes = a
        .hit_rect(Hit::Bar(BarItem::Library))
        .expect("Notes button");
    assert_eq!(notes.y, files.y);
    assert!(
        notes.x >= files.right() && notes.x - files.right() <= 4.0,
        "right after Files: {files:?} {notes:?}"
    );

    a.activate(Hit::Bar(BarItem::Library));
    assert!(a.ui.panes.files && a.ui.library.on);
    assert_eq!(shown(&a).len(), 5);
    a.activate(Hit::Bar(BarItem::Library));
    assert!(!a.ui.panes.files && !a.ui.library.on);
    // From the function list it brings the library, not the pane's end.
    a.ui.set_pane_face(crate::ui::PaneFace::Functions);
    a.activate(Hit::Bar(BarItem::Library));
    assert!(a.ui.panes.files && a.ui.library.on && !a.ui.outline.on);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_notes_button_comes_and_goes_with_the_files_button() {
    use crate::ui::BarItem;
    let (mut with, mut without) = (0, 0);
    for width in (300..=1600).step_by(20) {
        let mut a = app();
        a.size = (width, 800);
        a.compose();
        let files = a.hit_rect(Hit::Bar(BarItem::Files)).is_some();
        let notes = a.hit_rect(Hit::Bar(BarItem::Library)).is_some();
        assert_eq!(files, notes, "at {width}");
        if files {
            with += 1;
        } else {
            without += 1;
        }
    }
    assert!(
        with > 0 && without > 0,
        "{with} widths with, {without} without"
    );
}

#[test]
fn the_notes_button_is_lit_only_while_the_library_is_what_the_pane_shows() {
    use crate::chrome::pane_button_on;
    use crate::ui::{BarItem, PaneFace};
    let mut ui = crate::ui::Ui::new();
    let on = |ui: &crate::ui::Ui| {
        (
            pane_button_on(ui, BarItem::Files),
            pane_button_on(ui, BarItem::Library),
        )
    };
    assert_eq!(on(&ui), (false, false), "no pane");
    ui.set_pane_face(PaneFace::Files);
    assert_eq!(on(&ui), (true, false));
    ui.set_pane_face(PaneFace::Functions);
    assert_eq!(on(&ui), (false, false), "the function list lights neither");
    ui.set_pane_face(PaneFace::Notes);
    assert_eq!(on(&ui), (false, true));
    ui.hide_pane();
    assert_eq!(on(&ui), (false, false));
}

#[test]
fn the_notes_button_and_the_head_switches_say_what_they_are_with_their_keys() {
    use crate::chrome::tooltip;
    use crate::commands::hint;
    use crate::ui::{BarItem, PaneFace};
    let a = app();
    let tip = |hit| tooltip(hit, &a.docs, false).unwrap_or_default();
    // The keys as this platform prints them: a Mac says ⌘⇧3 where Windows says Ctrl+Shift+3.
    let files = hint(C::ToggleFiles);
    let functions = hint(C::ToggleOutline);
    let notes = hint(C::ToggleLibraryPane);
    assert!(notes.contains('3'), "{notes}");
    assert_eq!(
        tip(Hit::Bar(BarItem::Library)),
        format!("Notes library ({notes})")
    );
    assert_eq!(
        tip(Hit::PaneFace(PaneFace::Files)),
        format!("Files pane ({files})")
    );
    assert_eq!(
        tip(Hit::PaneFace(PaneFace::Functions)),
        format!("Function list ({functions})")
    );
    assert_eq!(
        tip(Hit::PaneFace(PaneFace::Notes)),
        format!("Notes library ({notes})")
    );
    assert!(tip(Hit::LibraryAdd).starts_with("Add to the notes library"));
}

// ---- a re-read that runs on its own thread ----------------------------------------------------

#[test]
fn a_read_that_finishes_after_the_folder_moved_is_dropped_not_shown() {
    let dir = scratch("libpane-stale");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let before = shown(&a);
    // What a thread read; the file is saved again before its result can be taken.
    let read = mdshared_app::library::read(&dir.join("library"));
    std::thread::sleep(std::time::Duration::from_millis(30));
    let file = dir.join("library").join("personal.md");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{text}\n## Later\n")).unwrap();
    assert!(
        !a.library_accept(read),
        "a read that started before the save is not shown"
    );
    assert_eq!(shown(&a), before, "the rows stayed as they were");
    assert!(
        a.ui.library.checked.is_none(),
        "the next pass reads again at once instead of waiting out the interval"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cursor_stays_on_its_note_when_the_rows_arrive_from_a_thread() {
    let dir = scratch("libpane-cursor-thread");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let at =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.label == "Last year")
            .expect("the note is listed");
    a.ui.library.focused = true;
    a.ui.library.set_cur(at);
    // A section and a note above it move every row below down.
    let file = dir.join("library").join("personal.md");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(
        &file,
        text.replacen(
            "## Money",
            "## Added\n\n- [New one](../new.md)\n\n## Money",
            1,
        ),
    )
    .unwrap();
    a.library_tick(Instant::now() + crate::library_pane::RECHECK);
    wait_for_read(&mut a);
    let now =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.label == "Last year")
            .expect("the note is still listed");
    assert_ne!(now, at, "the rows moved under the cursor");
    assert_eq!(a.ui.library.cur, Some(now), "the cursor followed its note");
    assert!(a.ui.library.focused, "the keyboard stayed in the pane");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_pane_drops_a_read_that_is_running() {
    let dir = scratch("libpane-close-read");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let file = dir.join("library").join("personal.md");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{text}\n## Later\n")).unwrap();
    a.library_tick(Instant::now() + crate::library_pane::RECHECK);
    assert!(a.library_job.is_some(), "the read was started");
    a.command(C::ToggleLibraryPane);
    assert!(!a.ui.panes.files, "the pane was closed");
    assert!(a.library_job.is_none(), "the read was let go");
    let before = shown(&a);
    std::thread::sleep(std::time::Duration::from_millis(50));
    a.library_tick(Instant::now());
    assert_eq!(shown(&a), before, "nothing of the read reached the rows");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn changing_the_face_away_from_the_notes_drops_a_read_that_is_running() {
    let dir = scratch("libpane-face-read");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    let file = dir.join("library").join("personal.md");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{text}\n## Later\n")).unwrap();
    a.library_tick(Instant::now() + crate::library_pane::RECHECK);
    assert!(a.library_job.is_some(), "the read was started");
    a.show_pane_face(crate::ui::PaneFace::Files);
    assert!(a.library_job.is_none(), "the read was let go");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_staleness_copy_holds_the_files_and_stamps_and_no_notes() {
    let dir = scratch("libpane-stamp");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    assert!(!a.docs.library.groups.is_empty());
    assert_eq!(
        a.docs.library_stamp.groups.len(),
        a.docs.library.groups.len(),
        "the same groups, for the comparison"
    );
    for group in &a.docs.library_stamp.groups {
        assert!(
            group.entries.is_empty() && group.sections.is_empty(),
            "only what changed() compares: the file and its stamp"
        );
    }
    // It still answers the question it is kept for.
    std::thread::sleep(std::time::Duration::from_millis(30));
    let file = dir.join("library").join("personal.md");
    let text = std::fs::read_to_string(&file).unwrap();
    std::fs::write(&file, format!("{text}\n")).unwrap();
    assert!(a.docs.library_stamp.changed());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_omitted_count_is_in_the_pane_frame_signature() {
    let dir = scratch("libpane-omitted-sig");
    let (_taxes, _last) = library_in(&dir);
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    a.compose();
    let sig = |a: &App| a.region_sig(crate::ui::Region::Files, a.areas.files);
    let before = sig(&a);
    a.docs.library.omitted = 71;
    assert_ne!(
        before,
        sig(&a),
        "the row saying how many groups are shown must repaint the pane"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
