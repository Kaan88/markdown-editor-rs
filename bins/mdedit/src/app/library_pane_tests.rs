//! The notes library in the left pane: what the commands show there, folding a row, and opening
//! a note from it.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::commands::CommandId as C;
use crate::library_pane::Kind;
use crate::ui::Hit;

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
fn the_notes_head_button_asks_where_to_add_the_open_file() {
    let dir = scratch("libpane-plus");
    let (_taxes, _last) = library_in(&dir);
    let note = dir.join("new.md");
    std::fs::write(&note, "# A new note\n").unwrap();
    let mut a = app_in(&dir);
    open_now(&mut a, &note);
    a.command(C::ToggleLibraryPane);
    a.activate(Hit::LibraryAdd);
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
    use crate::ui::{BarItem, PaneFace};
    let a = app();
    let tip = |hit| tooltip(hit, &a.docs, false).unwrap_or_default();
    assert_eq!(
        tip(Hit::Bar(BarItem::Library)),
        "Notes library (Ctrl+Shift+3)"
    );
    assert_eq!(
        tip(Hit::PaneFace(PaneFace::Files)),
        "Files pane (Ctrl+Shift+1)"
    );
    assert_eq!(
        tip(Hit::PaneFace(PaneFace::Functions)),
        "Function list (Ctrl+Shift+2)"
    );
    assert_eq!(
        tip(Hit::PaneFace(PaneFace::Notes)),
        "Notes library (Ctrl+Shift+3)"
    );
    assert!(tip(Hit::LibraryAdd).starts_with("Add this file to the notes library"));
}
