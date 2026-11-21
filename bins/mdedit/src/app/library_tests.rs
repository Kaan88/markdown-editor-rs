//! The notes library: the list of notes the palette shows, opening one, putting the open file
//! in it under a heading, and the library files themselves.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use crate::palette::Target;

/// An app whose settings folder is `dir`, so its library folder is `dir\library`.
fn app_in(dir: &Path) -> App {
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a
}

fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Types `text` in the open palette and presses Enter.
fn choose(a: &mut App, text: &str) {
    a.compose();
    for ch in text.chars() {
        a.key_pressed(Key::Name(KeyName::Char(ch)), Some(&ch.to_string()));
    }
    a.key_pressed(Key::Name(KeyName::Enter), None);
}

fn listed(a: &App) -> Vec<Target> {
    let p = a.palette.open.as_ref().expect("the palette is open");
    p.palette
        .listed_ids()
        .into_iter()
        .filter_map(|id| p.target(id).cloned())
        .collect()
}

#[test]
fn the_library_lists_the_notes_of_its_files_and_opens_one() {
    let dir = scratch("library-list");
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let taxes = dir.join("taxes.md");
    std::fs::write(&taxes, "# Taxes 2026\n\nwhat is due\n").unwrap();
    std::fs::write(dir.join("japan.md"), "# Japan\n").unwrap();
    std::fs::write(
        lib.join("personal.md"),
        format!(
            "# Personal\n\n## Money\n\n- {}\n- [Gone](gone.md)\n\n## Trips\n\n- [Japan](<{}>)\n",
            slash(&taxes),
            slash(&dir.join("japan.md"))
        ),
    )
    .unwrap();

    let mut a = app_in(&dir);
    a.command(C::Library);
    assert!(a.palette_open());
    let rows = listed(&a);
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert_eq!(
        rows[0],
        Target::Note(taxes.clone()),
        "the title from its heading"
    );
    assert_eq!(a.docs.library.groups.len(), 1);
    assert_eq!(a.docs.library.groups[0].name, "Personal");
    let entries = &a.docs.library.groups[0].entries;
    assert_eq!(entries[0].title, "Taxes 2026");
    assert_eq!(entries[0].sections, vec!["Money".to_string()]);
    assert!(entries[1].missing, "the note that is not on disk");
    assert_eq!(entries[2].title, "Japan");

    // Typing a title and pressing Enter opens that note.
    choose(&mut a, "Taxes");
    assert!(!a.palette_open());
    let open = a.docs.active_doc().and_then(|d| d.path.clone());
    assert_eq!(open.as_deref(), Some(taxes.as_path()));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_open_file_is_added_to_the_library_under_a_chosen_heading() {
    let dir = scratch("library-add");
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let file = lib.join("personal.md");
    std::fs::write(&file, "# Personal\n\n## Money\n\n## Trips\n\n").unwrap();
    let note = dir.join("receipts.md");
    std::fs::write(&note, "# Receipts\n\nkept here\n").unwrap();

    let mut a = app_in(&dir);
    open_now(&mut a, &note);
    a.command(C::LibraryAdd);
    assert!(a.palette_open());
    let rows = listed(&a);
    assert_eq!(rows.len(), 3, "the two headings and the end: {rows:?}");
    assert_eq!(
        rows[0],
        Target::NotePlace(file.clone(), vec!["Money".to_string()])
    );
    assert_eq!(rows[2], Target::NotePlace(file.clone(), Vec::new()));

    choose(&mut a, "Money");
    assert!(!a.palette_open());
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(
        written.contains(&format!("- [Receipts]({})", slash(&note))),
        "{written}"
    );
    // Under its heading, and the file still reads as the library file it was.
    let parsed = mdshared_app::library::parse(&written, &lib);
    assert_eq!(parsed.name.as_deref(), Some("Personal"));
    assert_eq!(parsed.entries.len(), 1);
    assert_eq!(parsed.entries[0].sections, vec!["Money".to_string()]);
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("Receipts") && t.contains("Personal")),
        "{:?}",
        a.docs.toast(Instant::now())
    );
    // The list has it now, and it is not offered twice.
    a.command(C::Library);
    assert_eq!(listed(&a), vec![Target::Note(note.clone())]);
    a.key_pressed(Key::Name(KeyName::Escape), None);
    a.command(C::LibraryAdd);
    assert!(!a.palette_open(), "already in the library");
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("already")),
        "{:?}",
        a.docs.toast(Instant::now())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_empty_library_is_started_by_the_first_file_added_or_edited() {
    let dir = scratch("library-empty");
    let note = dir.join("note.md");
    std::fs::write(&note, "# A note\n").unwrap();
    let mut a = app_in(&dir);

    // Nothing there yet: the list says so.
    a.command(C::Library);
    assert!(listed(&a).is_empty());
    a.key_pressed(Key::Name(KeyName::Escape), None);

    // Adding the open file makes the first library file from its template.
    open_now(&mut a, &note);
    a.command(C::LibraryAdd);
    let file = dir.join("library").join("notes.md");
    assert_eq!(
        listed(&a),
        vec![Target::NotePlace(file.clone(), Vec::new())]
    );
    choose(&mut a, "notes");
    let written = std::fs::read_to_string(&file).unwrap();
    assert!(written.starts_with("# Notes"), "{written}");
    assert!(written.contains("- [A note]("), "{written}");
    assert_eq!(a.docs.library.len(), 1);

    // "Edit the notes library" opens the library file itself.
    a.command(C::LibraryEdit);
    assert!(
        a.docs
            .tabs
            .iter()
            .any(|d| d.path.as_deref() == Some(file.as_path())),
        "the library file is open"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_library_file_saved_meanwhile_is_read_again() {
    let dir = scratch("library-reread");
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let file = lib.join("notes.md");
    let one = dir.join("one.md");
    let two = dir.join("two.md");
    std::fs::write(&one, "# One\n").unwrap();
    std::fs::write(&two, "# Two\n").unwrap();
    std::fs::write(&file, format!("# Notes\n\n- {}\n", slash(&one))).unwrap();

    let mut a = app_in(&dir);
    a.command(C::Library);
    assert_eq!(listed(&a), vec![Target::Note(one.clone())]);
    a.key_pressed(Key::Name(KeyName::Escape), None);

    std::fs::write(
        &file,
        format!("# Notes\n\n- {}\n- {}\n", slash(&one), slash(&two)),
    )
    .unwrap();
    a.command(C::Library);
    assert_eq!(
        listed(&a),
        vec![Target::Note(one.clone()), Target::Note(two.clone())],
        "the file was saved again"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A library of two groups, Personal (taxes) and Work (a plan), the notes being files in `dir`.
/// Returns the two library files and the two notes.
fn two_groups(dir: &Path) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let taxes = dir.join("taxes.md");
    let plan = dir.join("plan.md");
    std::fs::write(&taxes, "# Taxes\n").unwrap();
    std::fs::write(&plan, "# Plan\n").unwrap();
    let personal = lib.join("personal.md");
    let work = lib.join("work.md");
    std::fs::write(&personal, format!("# Personal\n\n- {}\n", slash(&taxes))).unwrap();
    std::fs::write(&work, format!("# Work\n\n- {}\n", slash(&plan))).unwrap();
    (personal, work, taxes, plan)
}

fn open_paths(a: &App) -> Vec<PathBuf> {
    a.docs.tabs.iter().filter_map(|d| d.path.clone()).collect()
}

fn active_path(a: &App) -> Option<PathBuf> {
    a.docs.active_doc().and_then(|d| d.path.clone())
}

#[test]
fn edit_a_library_file_opens_only_the_group_of_the_note_in_front() {
    let dir = scratch("library-edit-front");
    let (personal, _work, taxes, _plan) = two_groups(&dir);
    let mut a = app_in(&dir);
    open_now(&mut a, &taxes);

    a.command(C::LibraryEdit);
    assert!(!a.palette_open(), "the note's group is no question");
    assert_eq!(open_paths(&a), vec![taxes, personal.clone()]);
    assert_eq!(
        active_path(&a),
        Some(personal),
        "and it is the one in front"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_a_library_file_opens_the_group_of_the_row_last_clicked_in_the_pane() {
    let dir = scratch("library-edit-pane");
    let (personal, work, taxes, _plan) = two_groups(&dir);
    let mut a = app_in(&dir);
    open_now(&mut a, &taxes);
    a.command(C::ToggleLibraryPane);
    let row =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.label == "Work")
            .expect("the Work group is a row");
    a.activate(crate::ui::Hit::LibraryRow(row));

    a.command(C::LibraryEdit);
    assert_eq!(
        active_path(&a),
        Some(work.clone()),
        "the row wins over the note in front"
    );
    assert_eq!(open_paths(&a), vec![taxes.clone(), work]);

    // With the pane shut the row is forgotten: the note in front decides again.
    a.command(C::ToggleLibraryPane);
    a.command(C::ToggleLibraryPane);
    assert_eq!(a.ui.library.cursor, None);
    a.docs.activate(0);
    a.command(C::LibraryEdit);
    assert_eq!(active_path(&a), Some(personal));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_a_library_file_asks_which_when_nothing_points_at_one_and_opens_the_one_chosen() {
    let dir = scratch("library-edit-ask");
    let (personal, work, _taxes, _plan) = two_groups(&dir);
    let other = dir.join("other.md");
    std::fs::write(&other, "# Not in the library\n").unwrap();
    let mut a = app_in(&dir);
    open_now(&mut a, &other);

    a.command(C::LibraryEdit);
    assert!(a.palette_open(), "two groups and no hint: a list");
    assert_eq!(
        listed(&a),
        vec![
            Target::LibraryFile(personal.clone()),
            Target::LibraryFile(work.clone())
        ]
    );
    assert_eq!(open_paths(&a), vec![other.clone()], "nothing opened yet");

    choose(&mut a, "work");
    assert!(!a.palette_open());
    assert_eq!(open_paths(&a), vec![other, work.clone()]);
    assert_eq!(active_path(&a), Some(work));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn edit_a_library_file_opens_the_only_group_without_asking() {
    let dir = scratch("library-edit-only");
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).unwrap();
    let file = lib.join("notes.md");
    std::fs::write(&file, "# Notes\n").unwrap();
    let mut a = app_in(&dir);

    a.command(C::LibraryEdit);
    assert!(!a.palette_open());
    assert_eq!(open_paths(&a), vec![file]);
    let _ = std::fs::remove_dir_all(&dir);
}
