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
