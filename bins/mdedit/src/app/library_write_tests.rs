//! The notes library's folder setting, and what the pane's menus write: each write checked
//! against the exact text of the library file and against the rows the pane shows after.

use super::files_tests::{app, open_now, scratch, type_text};
use super::library_tests::choose;
use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use crate::library_pane::{EditKind, Kind};
use crate::ui::{BarItem, LibAct, PopupKind};

pub(super) fn app_in(dir: &Path) -> App {
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a
}

pub(super) fn slash(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub(super) fn press(a: &mut App, key: KeyName) {
    a.key_pressed(Key::Name(key), None);
}

pub(super) fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// Two notes on disk and one library file listing them:
/// rows are Personal, Money, Taxes 2026, 2025, Last year, Trips.
pub(super) struct Lib {
    pub(super) dir: PathBuf,
    pub(super) file: PathBuf,
    pub(super) taxes: PathBuf,
    pub(super) last: PathBuf,
}

impl Lib {
    pub(super) fn text(&self) -> String {
        format!(
            "# Personal\n\n## Money\n\n- [Taxes 2026]({})\n\n### 2025\n\n- [Last year]({})\n\n## Trips\n",
            slash(&self.taxes),
            slash(&self.last)
        )
    }
}

pub(super) fn lib(tag: &str) -> (App, Lib) {
    let dir = scratch(tag);
    let folder = dir.join("library");
    std::fs::create_dir_all(&folder).unwrap();
    let l = Lib {
        file: folder.join("personal.md"),
        taxes: dir.join("taxes.md"),
        last: dir.join("last.md"),
        dir,
    };
    std::fs::write(&l.taxes, "# Taxes 2026\n").unwrap();
    std::fs::write(&l.last, "# Last year\n").unwrap();
    std::fs::write(&l.file, l.text()).unwrap();
    let mut a = app_in(&l.dir);
    a.command(C::ToggleLibraryPane);
    (a, l)
}

pub(super) fn labels(a: &App) -> Vec<String> {
    a.ui.library.rows.iter().map(|r| r.label.clone()).collect()
}

pub(super) fn pick(a: &mut App, op: LibAct, row: usize) {
    a.act(Act::Library(op, row));
}

/// Types `name` into the box over the pane and presses Enter.
pub(super) fn name_it(a: &mut App, name: &str) {
    assert!(a.ui.library.edit.is_some(), "the name box is open");
    type_text(a, name);
    press(a, KeyName::Enter);
    assert!(a.ui.library.edit.is_none(), "Enter closes the box");
}

pub(super) fn toast(a: &App) -> String {
    a.docs
        .toast(Instant::now())
        .map(str::to_string)
        .unwrap_or_default()
}

// ---- the folder setting ----------------------------------------------------------------------

#[test]
fn the_library_folder_is_beside_the_settings_file_while_the_preference_is_empty() {
    let dir = scratch("libdir-default");
    let a = app_in(&dir);
    assert_eq!(a.docs.prefs.library_dir, "");
    assert_eq!(a.library_dir(), Some(dir.join("library")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_library_folder_in_the_preference_is_read_instead_of_the_default() {
    let (mut a, l) = lib("libdir-set");
    assert_eq!(labels(&a)[0], "Personal");
    let other = l.dir.join("onedrive");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(
        other.join("work.md"),
        format!("# Work\n\n- [Taxes work]({})\n", slash(&l.taxes)),
    )
    .unwrap();

    a.set_library_dir(&other.to_string_lossy());
    assert_eq!(a.library_dir(), Some(other.clone()));
    assert_eq!(
        labels(&a),
        vec!["Work", "Taxes work"],
        "the pane shows it now"
    );
    assert_eq!(a.docs.library.dir, other);
    assert!(
        a.docs.prefs.library_dir.ends_with("onedrive"),
        "kept as a preference"
    );

    // A path with no root is taken from the settings folder.
    a.set_library_dir("onedrive");
    assert_eq!(a.library_dir(), Some(l.dir.join("onedrive")));
    // Empty is the default again.
    a.set_library_dir("");
    assert_eq!(labels(&a)[0], "Personal");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn typing_a_folder_in_settings_changes_where_the_library_is_read() {
    let (mut a, l) = lib("libdir-typed");
    let other = l.dir.join("elsewhere");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(other.join("a.md"), "# Elsewhere\n").unwrap();
    a.apply_setting(
        super::modal::LIBRARY_DIR,
        mdshared_ui::settings::Value::Text(other.to_string_lossy().into_owned()),
    );
    assert_eq!(labels(&a), vec!["Elsewhere"]);
    assert_eq!(
        a.session.prefs.library_dir,
        other.to_string_lossy(),
        "the session file will keep it"
    );
    // The folder the picker answered is the same thing.
    a.library_dir_picked(l.dir.join("library"));
    assert_eq!(labels(&a)[0], "Personal");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_library_folder_that_is_not_there_is_an_empty_library() {
    let (mut a, l) = lib("libdir-gone");
    let gone = l.dir.join("no").join("such").join("folder");
    a.set_library_dir(&gone.to_string_lossy());
    assert!(a.ui.library.rows.is_empty(), "the pane's empty state");
    assert!(a.docs.library.is_empty() && a.docs.library.groups.is_empty());
    assert!(
        a.ui.library.checked.is_some(),
        "it was looked at, and said so"
    );
    a.library_tick(Instant::now() + crate::library_pane::RECHECK);
    a.command(C::Library);
    assert!(a.palette_open(), "the note list opens, empty");
    a.key_pressed(Key::Name(KeyName::Escape), None);

    // Edit makes the folder and its first file, as it does for an empty library today.
    a.command(C::LibraryEdit);
    let file = gone.join("notes.md");
    assert!(file.exists(), "made in the folder chosen");
    assert!(read(&file).starts_with("# Notes"));
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- the writes ------------------------------------------------------------------------------

#[test]
fn renaming_a_group_rewrites_its_heading_and_the_pane_shows_it() {
    let (mut a, l) = lib("libw-rename-group");
    pick(&mut a, LibAct::Rename, 0);
    assert_eq!(
        a.ui.library.edit.as_ref().map(|e| e.kind),
        Some(EditKind::RenameGroup)
    );
    assert_eq!(
        a.ui.library.field.text(),
        "Personal",
        "starts on the name it has"
    );
    name_it(&mut a, "Home");
    assert_eq!(read(&l.file), l.text().replacen("# Personal", "# Home", 1));
    assert_eq!(labels(&a)[0], "Home");
    assert_eq!(labels(&a).len(), 6);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn renaming_a_section_keeps_its_level_and_what_is_under_it() {
    let (mut a, l) = lib("libw-rename-section");
    pick(&mut a, LibAct::Rename, 1);
    name_it(&mut a, "Cash");
    assert_eq!(read(&l.file), l.text().replacen("## Money", "## Cash", 1));
    assert_eq!(
        labels(&a),
        vec![
            "Personal",
            "Cash",
            "Taxes 2026",
            "2025",
            "Last year",
            "Trips"
        ]
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn renaming_a_note_changes_the_text_of_its_link_and_not_its_file() {
    let (mut a, l) = lib("libw-rename-note");
    pick(&mut a, LibAct::Rename, 2);
    assert_eq!(a.ui.library.field.text(), "Taxes 2026");
    name_it(&mut a, "Tax return");
    assert_eq!(
        read(&l.file),
        l.text().replacen("[Taxes 2026]", "[Tax return]", 1)
    );
    assert_eq!(labels(&a)[2], "Tax return");
    assert_eq!(read(&l.taxes), "# Taxes 2026\n", "the note is untouched");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_new_section_is_written_under_its_parent_after_the_box_names_it() {
    let (mut a, l) = lib("libw-new-section");
    pick(&mut a, LibAct::NewSection, 1);
    // The box sits in a row of its own under the heading, until it is named.
    assert_eq!(a.ui.library.rows[2].kind, Kind::Editing);
    assert_eq!(a.ui.library.rows.len(), 7);
    name_it(&mut a, "Banks");
    assert_eq!(
        read(&l.file),
        l.text().replacen("### 2025", "### Banks\n\n### 2025", 1)
    );
    assert_eq!(a.ui.library.rows.len(), 7);
    assert!(a.ui.library.rows.iter().all(|r| r.kind != Kind::Editing));
    assert!(labels(&a).contains(&"Banks".to_string()));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_new_group_makes_a_library_file_from_the_template() {
    let (mut a, l) = lib("libw-new-group");
    a.act(Act::Library(LibAct::NewGroup, 0));
    assert_eq!(
        a.ui.library.rows.last().map(|r| r.kind.clone()),
        Some(Kind::Editing)
    );
    name_it(&mut a, "Work: 2026");
    // The file name is the name made fit for a file; the group's name is the heading.
    let made = l.dir.join("library").join("Work_ 2026.md");
    let text = read(&made);
    assert!(text.starts_with("# Work: 2026\n"), "{text}");
    assert_eq!(
        text,
        mdshared_app::library::new_group_with_home("Work: 2026", Some(Path::new("Work_ 2026")))
            .unwrap()
    );
    assert!(labels(&a).contains(&"Work: 2026".to_string()));

    // A name already taken is refused and nothing is overwritten.
    let before = read(&made);
    a.act(Act::Library(LibAct::NewGroup, 0));
    name_it(&mut a, "Work: 2026");
    assert!(toast(&a).contains("already there"), "{}", toast(&a));
    assert_eq!(read(&made), before);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_new_note_is_made_in_its_groups_folder_listed_under_its_heading_and_opened() {
    let (mut a, l) = lib("libw-new-note");
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    let note = l
        .dir
        .join("library")
        .join("personal")
        .join("Money")
        .join("Budget.md");
    assert_eq!(read(&note), "# Budget\n");
    // The group got its `home:` line with its first note, and the note is in the heading's
    // folder inside it.
    let mut want = l
        .text()
        .replacen("# Personal\n", "# Personal\nhome: personal\n", 1);
    want = want.replacen(
        "\n\n### 2025",
        "\n- [Budget](personal/Money/Budget.md)\n\n### 2025",
        1,
    );
    assert_eq!(read(&l.file), want);
    assert!(labels(&a).contains(&"Budget".to_string()));
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()).as_deref(),
        Some(note.as_path()),
        "opened in a tab"
    );
    // The same name again does not overwrite the first note.
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    assert_eq!(read(&note), "# Budget\n");
    assert!(
        l.dir
            .join("library")
            .join("personal")
            .join("Money")
            .join("Budget 2.md")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_new_note_under_a_heading_goes_in_the_folder_that_heading_names() {
    let (mut a, l) = lib("libw-home-note");
    // Inside the library folder, the group's folder is written relative to it.
    std::fs::write(
        &l.file,
        l.text()
            .replacen("# Personal\n", "# Personal\nhome: Personal notes\n", 1),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    let inside = l.dir.join("library").join("Personal notes");
    assert_eq!(
        read(&inside.join("Money").join("Budget.md")),
        "# Budget\n",
        "the folder is made for the note"
    );
    let text = read(&l.file);
    assert!(text.contains("home: Personal notes"), "{text}");
    assert!(
        text.contains("- [Budget](<Personal notes/Money/Budget.md>)"),
        "{text}"
    );

    // Outside it, the line is the whole path and so is the link.
    let outside = l.dir.join("outside notes");
    std::fs::write(
        &l.file,
        l.text().replacen(
            "# Personal\n",
            &format!("# Personal\nhome: {}\n", slash(&outside)),
            1,
        ),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Plans");
    let note = outside.join("Money").join("Plans.md");
    assert_eq!(read(&note), "# Plans\n");
    assert!(
        read(&l.file).contains(&format!("- [Plans](<{}>)", slash(&note))),
        "{}",
        read(&l.file)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_first_note_a_group_gets_writes_its_home_and_says_where_the_notes_go() {
    let (mut a, l) = lib("libw-first-home");
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    let note = l
        .dir
        .join("library")
        .join("personal")
        .join("Money")
        .join("Budget.md");
    assert_eq!(read(&note), "# Budget\n");
    let text = read(&l.file);
    assert!(text.starts_with("# Personal\nhome: personal\n"), "{text}");
    assert!(
        text.contains("- [Budget](personal/Money/Budget.md)"),
        "{text}"
    );
    let says = toast(&a);
    assert!(
        says.contains("Personal") && says.contains("personal"),
        "{says}"
    );
    // Nothing that was already there was moved, and the second note adds no second line.
    assert_eq!(read(&l.taxes), "# Taxes 2026\n");
    assert_eq!(read(&l.last), "# Last year\n");
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Plans");
    assert_eq!(read(&l.file).matches("home:").count(), 1);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_folder_that_cannot_be_made_is_a_toast_and_no_note_is_written() {
    let (mut a, l) = lib("libw-home-blocked");
    // A file where the group's folder goes: the folder cannot be made.
    std::fs::write(l.dir.join("library").join("personal"), "not a folder").unwrap();
    let before = read(&l.file);
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    assert!(toast(&a).contains("cannot be made"), "{}", toast(&a));
    assert_eq!(read(&l.file), before);
    assert!(
        !l.dir
            .join("library")
            .join("personal")
            .join("Money")
            .exists()
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_group_is_pointed_at_a_folder_the_owner_already_has() {
    let (mut a, l) = lib("libw-set-home");
    let notes = l.dir.join("library").join("Japan 2025");
    std::fs::create_dir_all(&notes).unwrap();
    a.library_group_home_picked(&l.file, notes.clone());
    assert_eq!(
        read(&l.file),
        l.text()
            .replacen("# Personal\n", "# Personal\nhome: Japan 2025\n", 1)
    );
    assert!(toast(&a).contains("Japan 2025"), "{}", toast(&a));
    // A folder inside the library folder is named relative to it, and the note follows it.
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Tokyo");
    assert_eq!(read(&notes.join("Money").join("Tokyo.md")), "# Tokyo\n");
    assert!(
        read(&l.file).contains("](<Japan 2025/Money/Tokyo.md>)"),
        "{}",
        read(&l.file)
    );

    // Outside it, the line and the link are whole paths.
    let outside = l.dir.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    a.library_group_home_picked(&l.file, outside.clone());
    assert!(
        read(&l.file).contains(&format!("home: {}\n", slash(&outside))),
        "{}",
        read(&l.file)
    );
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Plans");
    let note = outside.join("Money").join("Plans.md");
    assert_eq!(read(&note), "# Plans\n");
    assert!(
        read(&l.file).contains(&mdshared_app::library::link_line("Plans", &note)),
        "{}",
        read(&l.file)
    );
    // What was written under the old folder is still there.
    assert!(notes.join("Money").join("Tokyo.md").exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn clearing_the_groups_folder_leaves_the_notes_where_they_are() {
    let (mut a, l) = lib("libw-clear-home");
    std::fs::write(
        &l.file,
        l.text()
            .replacen("# Personal\n", "# Personal\nhome: elsewhere\n", 1),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    pick(&mut a, LibAct::ClearHome, 0);
    let text = read(&l.file);
    assert!(!text.contains("home:"), "{text}");
    assert!(text.starts_with("# Personal\n\n## Money"), "{text}");
    assert!(a.docs.library.groups[0].home.is_none());
    assert_eq!(read(&l.taxes), "# Taxes 2026\n");
    assert!(l.taxes.exists() && l.last.exists());
    assert!(toast(&a).contains("not moved"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_new_group_is_written_with_the_folder_its_notes_go_in() {
    let (mut a, l) = lib("libw-new-group-home");
    a.act(Act::Library(LibAct::NewGroup, 0));
    name_it(&mut a, "Japan 2025");
    let made = l.dir.join("library").join("Japan 2025.md");
    let text = read(&made);
    assert!(
        text.starts_with("# Japan 2025\nhome: Japan 2025\n"),
        "{text}"
    );
    // Its first note goes into that folder without a second `home:` line.
    let at =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.kind == Kind::Group && r.label == "Japan 2025")
            .unwrap();
    pick(&mut a, LibAct::NewNote, at);
    name_it(&mut a, "Tokyo");
    assert_eq!(
        read(&l.dir.join("library").join("Japan 2025").join("Tokyo.md")),
        "# Tokyo\n"
    );
    assert_eq!(read(&made).matches("home:").count(), 1);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn removing_a_note_takes_one_line_out_and_leaves_the_note() {
    let (mut a, l) = lib("libw-remove-note");
    pick(&mut a, LibAct::Remove, 2);
    assert_eq!(
        read(&l.file),
        l.text()
            .replacen(&format!("- [Taxes 2026]({})\n", slash(&l.taxes)), "", 1)
    );
    assert!(!labels(&a).contains(&"Taxes 2026".to_string()));
    assert!(l.taxes.exists(), "the file is not deleted");
    assert!(toast(&a).contains("untouched"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn removing_a_section_asks_first_and_takes_what_is_under_it() {
    let (mut a, l) = lib("libw-remove-section");
    pick(&mut a, LibAct::Remove, 1);
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(crate::ui::DialogKind::LibraryRemove)
    );
    let body = a.ui.dialog.as_ref().unwrap().subject.clone();
    assert!(body.contains("Money") && body.contains("2 notes"), "{body}");
    // Escape asks no more and writes nothing.
    press(&mut a, KeyName::Escape);
    assert!(a.ui.dialog.is_none());
    assert_eq!(read(&l.file), l.text());
    // Remove writes.
    pick(&mut a, LibAct::Remove, 1);
    press(&mut a, KeyName::Enter);
    assert!(a.ui.dialog.is_none());
    assert_eq!(read(&l.file), "# Personal\n\n## Trips\n");
    assert_eq!(labels(&a), vec!["Personal", "Trips"]);
    assert!(l.taxes.exists() && l.last.exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_moves_to_another_section_of_its_file_through_the_picker() {
    let (mut a, l) = lib("libw-move-same");
    pick(&mut a, LibAct::MoveTo, 2);
    assert!(a.palette_open(), "the places are listed");
    choose(&mut a, "Trips");
    assert_eq!(
        read(&l.file),
        format!(
            "# Personal\n\n## Money\n\n\n### 2025\n\n- [Last year]({})\n\n## Trips\n- [Taxes 2026]({})\n",
            slash(&l.last),
            slash(&l.taxes)
        )
    );
    assert!(toast(&a).contains("Personal"), "{}", toast(&a));
    assert!(a.ui.library.moving.is_none());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_moves_to_another_library_file_with_the_target_written_first() {
    let (mut a, l) = lib("libw-move-two");
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, "# Work\n\n## Projects\n").unwrap();
    pick(&mut a, LibAct::MoveTo, 2);
    choose(&mut a, "Projects");
    assert_eq!(
        read(&work),
        format!(
            "# Work\n\n## Projects\n- [Taxes 2026]({})\n",
            slash(&l.taxes)
        )
    );
    assert_eq!(
        read(&l.file),
        l.text()
            .replacen(&format!("- [Taxes 2026]({})\n", slash(&l.taxes)), "", 1)
    );
    assert!(toast(&a).contains("Work"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn when_the_removal_fails_the_note_is_in_the_new_place_and_the_toast_says_so() {
    let (mut a, l) = lib("libw-move-half");
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, "# Work\n\n## Projects\n").unwrap();
    a.library_rows();
    // Line 2 of the old file is not the note: the removal finds nothing to take out.
    let moved = a.library_move(
        &l.file,
        2,
        "Taxes 2026",
        &l.taxes,
        &work,
        &["Projects".to_string()],
    );
    assert!(!moved);
    assert_eq!(
        read(&work),
        format!(
            "# Work\n\n## Projects\n- [Taxes 2026]({})\n",
            slash(&l.taxes)
        ),
        "the target was written first"
    );
    assert_eq!(read(&l.file), l.text(), "the old place is as it was");
    let said = toast(&a);
    assert!(
        said.contains("is in Work now")
            && said.contains("could not be taken out")
            && said.contains("both"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_library_file_open_with_unsaved_changes_is_never_written() {
    let (mut a, l) = lib("libw-busy");
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, "# Work\n\n## Projects\n").unwrap();
    open_now(&mut a, &l.file);
    type_text(&mut a, "x");
    assert!(a.docs.active_doc().unwrap().modified());
    a.library_rows();
    let before = read(&l.file);
    let secs = vec!["Money".to_string()];

    a.library_rename_group(&l.file, "Home");
    a.library_rename_section(&l.file, &secs, "Cash");
    a.library_rename_note(&l.file, 5, &l.taxes, "Tax return");
    a.library_new_section(&l.file, &secs, "Banks");
    a.library_new_note(&l.file, &secs, "Budget");
    a.library_remove_note(&l.file, 5, &l.taxes, "Taxes 2026");
    a.library_set_note_path(&l.file, 5, &l.taxes, &l.last);
    a.library_put(&l.file, &secs);
    assert!(!a.library_move(&l.file, 5, "Taxes 2026", &l.taxes, &work, &[]));
    assert!(!a.library_move(&work, 1, "x", &l.taxes, &l.file, &secs));
    a.library_rename_file(&l.file, "other");
    assert_eq!(read(&l.file), before, "not a byte of it");
    assert_eq!(read(&work), "# Work\n\n## Projects\n");
    assert!(
        !l.dir.join("library").join("Budget.md").exists(),
        "no note made for a refused link"
    );
    assert!(l.file.exists());
    assert!(toast(&a).contains("not saved"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_writer_with_nothing_to_do_writes_nothing_and_says_nothing() {
    let (mut a, l) = lib("libw-none");
    let before = read(&l.file);
    let modified = || std::fs::metadata(&l.file).unwrap().modified().unwrap();
    let stamp = modified();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let secs = |s: &str| vec![s.to_string()];
    // Line 1 is the heading, 2 a blank line: neither is a note.
    a.library_remove_note(&l.file, 1, &l.taxes, "x");
    a.library_remove_note(&l.file, 2, &l.taxes, "x");
    a.library_remove_note(&l.file, 99, &l.taxes, "x");
    a.library_remove_note(&l.file, 0, &l.taxes, "x");
    // Another note's line.
    a.library_rename_note(&l.file, 9, &l.taxes, "Wrong");
    a.library_set_note_path(&l.file, 5, &l.last, &l.taxes);
    // A section that is not written, a name that is not one, the name it has already.
    a.library_rename_section(&l.file, &secs("Nope"), "X");
    a.library_rename_section(&l.file, &secs("Money"), "   ");
    a.library_rename_section(&l.file, &secs("Money"), "Money");
    a.library_rename_group(&l.file, "Personal");
    a.library_new_section(&l.file, &secs("Nope"), "X");
    assert_eq!(read(&l.file), before);
    assert_eq!(
        modified(),
        stamp,
        "not even written again with the same text"
    );
    assert_eq!(toast(&a), "", "quietly");
    // A file with no group heading cannot be renamed by its heading.
    let plain = l.dir.join("library").join("plain.md");
    std::fs::write(&plain, "- just text\n").unwrap();
    a.library_rename_group(&plain, "Named");
    assert_eq!(read(&plain), "- just text\n");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_name_is_used_on_enter_dropped_on_escape_and_used_by_a_click_elsewhere() {
    let (mut a, l) = lib("libw-box");
    // Escape drops it.
    pick(&mut a, LibAct::Rename, 1);
    assert!(a.ui.find_focused && a.ui.field == crate::ui::BarField::Library);
    type_text(&mut a, "Dropped");
    press(&mut a, KeyName::Escape);
    assert!(a.ui.library.edit.is_none() && !a.ui.find_focused);
    assert_eq!(read(&l.file), l.text(), "nothing was written");
    assert_eq!(labels(&a)[1], "Money");

    // Enter uses it.
    pick(&mut a, LibAct::Rename, 1);
    type_text(&mut a, "Kept");
    press(&mut a, KeyName::Enter);
    assert_eq!(labels(&a)[1], "Kept");
    assert!(read(&l.file).contains("## Kept\n"));

    // A click elsewhere uses it too.
    pick(&mut a, LibAct::Rename, 1);
    type_text(&mut a, "Clicked");
    a.compose();
    let open = a
        .hit_rect(Hit::Bar(BarItem::Open))
        .expect("the bar is drawn");
    a.ui.mouse = (open.cx(), open.cy());
    a.left_press();
    assert!(a.ui.library.edit.is_none());
    assert!(read(&l.file).contains("## Clicked\n"), "{}", read(&l.file));

    // An empty name writes nothing, for a rename and for a new one.
    pick(&mut a, LibAct::Rename, 1);
    press(&mut a, KeyName::Backspace);
    press(&mut a, KeyName::Enter);
    pick(&mut a, LibAct::NewSection, 0);
    press(&mut a, KeyName::Enter);
    assert!(read(&l.file).contains("## Clicked\n"));
    assert_eq!(labels(&a).len(), 6);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_box_is_gone_when_the_pane_shows_something_else() {
    let (mut a, l) = lib("libw-box-away");
    pick(&mut a, LibAct::Rename, 1);
    a.command(C::ToggleFiles);
    assert!(a.ui.library.edit.is_none() && !a.ui.find_focused);
    assert_eq!(read(&l.file), l.text());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn find_the_file_points_the_link_at_the_file_chosen_and_keeps_its_title() {
    let (mut a, l) = lib("libw-find");
    let moved = l.dir.join("moved").join("taxes-2026.md");
    std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
    std::fs::rename(&l.taxes, &moved).unwrap();
    // The pane does not stat the notes by itself; Refresh reads them again.
    assert!(matches!(
        a.ui.library.rows[2].kind,
        Kind::Note { missing: false, .. }
    ));
    a.act(Act::Library(LibAct::Refresh, 0));
    assert!(matches!(
        a.ui.library.rows[2].kind,
        Kind::Note { missing: true, .. }
    ));
    let Kind::Note { line, .. } = a.ui.library.rows[2].kind else {
        unreachable!()
    };
    a.library_set_note_path(&l.file, line, &l.taxes, &moved);
    assert_eq!(
        read(&l.file),
        l.text().replace(&slash(&l.taxes), &slash(&moved))
    );
    assert!(matches!(
        a.ui.library.rows[2].kind,
        Kind::Note { missing: false, .. }
    ));
    assert_eq!(labels(&a)[2], "Taxes 2026");

    // It is offered for a missing note and only then.
    let menu = |a: &App, i: usize| {
        let Kind::Note { .. } = a.ui.library.rows[i].kind else {
            unreachable!()
        };
        crate::overlays::library_row_menu(&a.ui.library.rows[i].kind, i, true, 0.0, 0.0, false)
    };
    let find = |m: &mdshared_ui::menu::Menu<Act>| {
        m.items
            .iter()
            .any(|i| i.id == Act::Library(LibAct::FindFile, 2))
    };
    assert!(!find(&menu(&a, 2)));
    std::fs::remove_file(&moved).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert!(find(&menu(&a, 2)));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn renaming_the_library_file_keeps_its_group_and_refuses_a_name_in_use() {
    let (mut a, l) = lib("libw-rename-file");
    let other = l.dir.join("library").join("zeta.md");
    std::fs::write(&other, "# Zeta\n").unwrap();
    a.library_rows();
    pick(&mut a, LibAct::RenameFile, 0);
    assert_eq!(a.ui.library.field.text(), "personal");
    name_it(&mut a, "Aardvark");
    let renamed = l.dir.join("library").join("Aardvark.md");
    assert!(renamed.exists() && !l.file.exists());
    assert_eq!(read(&renamed), l.text(), "the group keeps its heading");
    assert_eq!(labels(&a)[0], "Personal");
    // A name in use is refused.
    pick(&mut a, LibAct::RenameFile, 0);
    name_it(&mut a, "zeta");
    assert!(
        renamed.exists() && toast(&a).contains("already there"),
        "{}",
        toast(&a)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- the menus -------------------------------------------------------------------------------

pub(super) fn right_click_at(a: &mut App, x: f32, y: f32) {
    a.compose();
    a.ui.mouse = (x, y);
    a.right_press();
}

#[test]
fn a_right_click_on_a_row_moves_the_pane_to_it_and_opens_its_menu() {
    let (mut a, l) = lib("libm-row");
    a.compose();
    for (i, group_row) in [(0, true), (1, false), (2, false)] {
        let r = a.hit_rect(Hit::LibraryRow(i)).expect("the row is drawn");
        a.ui.popup = None;
        a.ui.library.cursor = None;
        right_click_at(&mut a, r.cx(), r.cy());
        assert_eq!(a.ui.popup_kind(), Some(PopupKind::LibraryRow(i)), "row {i}");
        assert_eq!(
            a.ui.library.cursor.as_deref(),
            Some(l.file.as_path()),
            "the pane's row is the one under the pointer"
        );
        let menu = &a.ui.popup.as_ref().unwrap().menu;
        assert_eq!(
            menu.items
                .iter()
                .any(|m| m.id == Act::Library(LibAct::RenameFile, i)),
            group_row,
            "only a group has its file renamed"
        );
        assert!(!a.ui.popup.as_ref().unwrap().cues, "opened by the pointer");
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_right_click_in_the_empty_space_of_the_pane_opens_the_pane_menu_and_so_does_the_plus_button() {
    let (mut a, l) = lib("libm-empty");
    a.compose();
    let pane = a.areas.files.expect("the pane is drawn");
    right_click_at(&mut a, pane.cx(), pane.bottom() - 20.0);
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::LibraryEmpty));
    a.ui.popup = None;
    // Not over the head, where the switches are.
    right_click_at(&mut a, pane.cx(), pane.y + 8.0);
    assert!(a.ui.popup.is_none());

    a.compose();
    a.activate(Hit::LibraryAdd);
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::LibraryEmpty));
    a.activate(Hit::LibraryAdd);
    assert!(a.ui.popup.is_none(), "again closes it");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_menu_item_runs_what_it_says_and_a_letter_picks_it() {
    let (mut a, l) = lib("libm-letter");
    a.compose();
    let r = a.hit_rect(Hit::LibraryRow(2)).unwrap();
    right_click_at(&mut a, r.cx(), r.cy());
    // "Remove from the library": its letter is y.
    a.key_pressed(Key::Name(KeyName::Char('y')), Some("y"));
    assert!(a.ui.popup.is_none());
    assert!(!labels(&a).contains(&"Taxes 2026".to_string()));
    assert_eq!(read(&l.file).matches("Taxes").count(), 0);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_pane_menus_mark_every_item_with_its_own_letter_and_delete_only_a_library_file() {
    use mdshared_ui::menu::{access_key_of, parse_access};
    let note = |missing: bool| Kind::Note {
        path: PathBuf::from("D:/n.md"),
        missing,
        line: 3,
    };
    let mut menus = Vec::new();
    for can_add in [false, true] {
        for kind in [Kind::Group, Kind::Section, note(false), note(true)] {
            menus.push((
                format!("{kind:?} {can_add}"),
                crate::overlays::library_row_menu(&kind, 1, can_add, 0.0, 0.0, false),
            ));
        }
    }
    for has_file in [false, true] {
        menus.push((
            format!("empty {has_file}"),
            crate::overlays::library_empty_menu(has_file, 0.0, 0.0, false),
        ));
    }
    for (name, menu) in &menus {
        let mut seen = std::collections::HashSet::new();
        for item in &menu.items {
            let key = access_key_of(&item.label)
                .unwrap_or_else(|| panic!("{name}: no key on {:?}", item.label));
            assert!(
                seen.insert(key.to_lowercase().next().unwrap()),
                "{name}: two items with the key {key}: {:?}",
                item.label
            );
            // Nothing deletes a note or a section's file; the one deletion is of the group's
            // own library file, which is the app's index and not a document of the owner's.
            let text = parse_access(&item.label).text.to_lowercase();
            let deletes = text.contains("delete") || text.contains("on disk");
            let library_file = matches!(item.id, Act::Library(LibAct::DeleteFile, _));
            assert!(
                !deletes
                    || (library_file && name.starts_with("Group") && text.contains("library file")),
                "{name}: {text}"
            );
        }
    }
    // What each kind of row offers.
    let has = |kind: &Kind, op: LibAct| {
        crate::overlays::library_row_menu(kind, 1, true, 0.0, 0.0, false)
            .items
            .iter()
            .any(|i| i.id == Act::Library(op, 1))
    };
    for op in [
        LibAct::NewNote,
        LibAct::NewSection,
        LibAct::AddOpen,
        LibAct::SetHome,
        LibAct::ClearHome,
        LibAct::Rename,
        LibAct::RenameFile,
        LibAct::EditFile,
        LibAct::CopyPath,
        LibAct::Reveal,
    ] {
        assert!(has(&Kind::Group, op), "group: {op:?}");
    }
    for op in [
        LibAct::NewNote,
        LibAct::NewSection,
        LibAct::AddOpen,
        LibAct::Rename,
        LibAct::GoToLine,
        LibAct::Remove,
    ] {
        assert!(has(&Kind::Section, op), "section: {op:?}");
    }
    for op in [
        LibAct::Open,
        LibAct::OpenOther,
        LibAct::Rename,
        LibAct::MoveTo,
        LibAct::CopyPath,
        LibAct::Reveal,
        LibAct::GoToLine,
        LibAct::Remove,
    ] {
        assert!(has(&note(false), op), "note: {op:?}");
    }
    assert!(!has(&note(false), LibAct::FindFile) && has(&note(true), LibAct::FindFile));
    // The open document is offered only when it can be written.
    let add = crate::overlays::library_row_menu(&Kind::Group, 0, false, 0.0, 0.0, false);
    assert!(
        !add.items
            .iter()
            .find(|i| i.id == Act::Library(LibAct::AddOpen, 0))
            .unwrap()
            .enabled
    );
}

#[test]
fn go_to_opens_the_library_file_with_the_caret_on_the_note_or_the_heading() {
    let (mut a, l) = lib("libm-goto");
    open_now(&mut a, &l.file);
    let caret_line = |a: &App| {
        let r = a.docs.active_ready().unwrap();
        r.buffer.line_of(r.view.sel.caret) + 1
    };
    // Row 2 is the note on line 5; row 1 is the heading "Money" on line 3.
    pick(&mut a, LibAct::GoToLine, 2);
    assert_eq!(caret_line(&a), 5);
    pick(&mut a, LibAct::GoToLine, 5);
    assert_eq!(caret_line(&a), 11, "## Trips");
    pick(&mut a, LibAct::GoToLine, 1);
    assert_eq!(caret_line(&a), 3);
    pick(&mut a, LibAct::EditFile, 0);
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()).as_deref(),
        Some(l.file.as_path())
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- the name box draws what is typed ---------------------------------------------------------

/// The box on screen, drawn now: its pixels, to tell one drawing of it from another.
fn box_pixels(a: &mut App) -> Vec<u8> {
    a.compose();
    let f = a
        .hit_rect(Hit::LibraryField)
        .expect("the name box is on screen");
    let s = a.scale;
    let (x0, y0) = ((f.x * s).ceil() as usize + 4, (f.y * s).ceil() as usize + 3);
    let (x1, y1) = (
        ((f.right() * s) as usize).saturating_sub(4),
        ((f.bottom() * s) as usize).saturating_sub(3),
    );
    assert!(x1 > x0 && y1 > y0, "the box has room: {f:?}");
    let w = a.screen.width as usize;
    let mut out = Vec::new();
    for y in y0..y1 {
        out.extend_from_slice(&a.screen.pixels[(y * w + x0) * 4..(y * w + x1) * 4]);
    }
    out
}

/// Types in the open box, and checks that the drawing of it follows the text: the typed text
/// is drawn, and not the same as another text or none.
fn assert_box_draws(a: &mut App, what: &str) {
    assert_eq!(
        a.ui.library.field.text(),
        "",
        "{what}: the box starts empty"
    );
    let none = box_pixels(a);
    type_text(a, "Work");
    assert_eq!(
        a.ui.library.field.text(),
        "Work",
        "{what}: the keys reach the box"
    );
    let typed = box_pixels(a);
    assert!(typed != none, "{what}: nothing of what was typed is drawn");
    a.ui.library.field.set_text("Wxrk");
    assert!(
        box_pixels(a) != typed,
        "{what}: the text drawn is not the text typed"
    );
    a.ui.library.field.set_text("Work");
    assert!(
        box_pixels(a) == typed,
        "{what}: the same text draws the same"
    );
}

#[test]
fn a_new_group_box_opened_from_the_menu_draws_what_is_typed() {
    let (mut a, l) = lib("libw-draw-newgroup");
    a.compose();
    let pane = a.areas.files.expect("the pane is drawn");
    right_click_at(&mut a, pane.cx(), pane.bottom() - 20.0);
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::LibraryEmpty));
    a.key_pressed(Key::Name(KeyName::Char('g')), Some("g"));
    assert!(a.ui.library.edit.is_some(), "New group opens the box");
    assert_box_draws(&mut a, "new group by its letter");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn every_way_of_opening_the_name_box_draws_what_is_typed() {
    for (name, op, row) in [
        ("new group", LibAct::NewGroup, 0),
        ("new section", LibAct::NewSection, 1),
        ("new note", LibAct::NewNote, 1),
        ("new section in a group", LibAct::NewSection, 0),
        ("new note in a group", LibAct::NewNote, 0),
    ] {
        let (mut a, l) = lib("libw-draw-new");
        pick(&mut a, op, row);
        assert_box_draws(&mut a, name);
        let _ = std::fs::remove_dir_all(&l.dir);
    }
    for (name, row, text) in [
        ("rename a group", 0, "Personal"),
        ("rename a section", 1, "Money"),
        ("rename a note", 2, "Taxes 2026"),
    ] {
        let (mut a, l) = lib("libw-draw-rename");
        pick(&mut a, LibAct::Rename, row);
        assert_eq!(a.ui.library.field.text(), text);
        a.ui.library.field.set_text("");
        assert_box_draws(&mut a, name);
        let _ = std::fs::remove_dir_all(&l.dir);
    }
}

#[test]
fn a_new_group_box_in_an_empty_library_draws_what_is_typed() {
    let dir = scratch("libw-draw-empty");
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    assert!(a.ui.library.rows.is_empty());
    pick(&mut a, LibAct::NewGroup, 0);
    assert_box_draws(&mut a, "new group, nothing in the library");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- deleting a library file ------------------------------------------------------------------

#[test]
fn deleting_a_library_file_asks_then_removes_only_that_file_and_reads_the_library_again() {
    let (mut a, l) = lib("libw-delete");
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, "# Work\n\n- [Taxes work](x.md)\n").unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert_eq!(labels(&a)[0], "Personal");
    assert!(labels(&a).contains(&"Work".to_string()));

    pick(&mut a, LibAct::DeleteFile, 0);
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(crate::ui::DialogKind::LibraryDelete)
    );
    assert!(l.file.exists(), "nothing is deleted before the answer");
    // Escape is no.
    press(&mut a, KeyName::Escape);
    assert!(a.ui.dialog.is_none() && l.file.exists());
    assert!(a.ui.library.deleting.is_none());

    pick(&mut a, LibAct::DeleteFile, 0);
    press(&mut a, KeyName::Enter);
    assert!(a.ui.dialog.is_none());
    assert!(!l.file.exists(), "the library file is gone");
    assert!(work.exists(), "another library file is not touched");
    assert_eq!(
        labels(&a),
        vec!["Work", "Taxes work"],
        "the pane read again"
    );
    assert!(
        toast(&a).contains("Personal") && toast(&a).contains("untouched"),
        "{}",
        toast(&a)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_notes_a_deleted_library_file_listed_are_still_on_disk() {
    let (mut a, l) = lib("libw-delete-notes");
    pick(&mut a, LibAct::DeleteFile, 0);
    press(&mut a, KeyName::Enter);
    assert!(!l.file.exists());
    assert_eq!(read(&l.taxes), "# Taxes 2026\n");
    assert_eq!(read(&l.last), "# Last year\n");
    assert!(a.ui.library.rows.is_empty(), "no group is left");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_question_about_a_library_file_says_its_name_and_how_many_notes_it_lists() {
    let (mut a, l) = lib("libw-delete-count");
    let none = l.dir.join("library").join("empty.md");
    std::fs::write(&none, "# Empty\n").unwrap();
    let one = l.dir.join("library").join("one.md");
    std::fs::write(&one, format!("# One\n\n- [Taxes](<{}>)\n", slash(&l.taxes))).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    let row_of = |a: &App, name: &str| {
        a.ui.library
            .rows
            .iter()
            .position(|r| r.kind == Kind::Group && r.label == name)
            .unwrap()
    };
    for (group, file, says) in [
        ("Personal", "personal.md", "lists 2 notes"),
        ("One", "one.md", "lists 1 note."),
        ("Empty", "empty.md", "lists no notes"),
    ] {
        let at = row_of(&a, group);
        pick(&mut a, LibAct::DeleteFile, at);
        let body = a.ui.dialog.as_ref().expect("the question").subject.clone();
        assert!(body.contains(group) && body.contains(file), "{body}");
        assert!(body.contains(says), "{group}: {body}");
        assert!(body.contains("themselves are not touched"), "{body}");
        press(&mut a, KeyName::Escape);
    }
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_library_file_open_with_unsaved_changes_is_not_deleted() {
    let (mut a, l) = lib("libw-delete-busy");
    open_now(&mut a, &l.file);
    type_text(&mut a, "x");
    assert!(a.docs.active_doc().unwrap().modified());
    // The menu item refuses before it asks...
    pick(&mut a, LibAct::DeleteFile, 0);
    assert!(
        a.ui.dialog.is_none(),
        "no question for a file that cannot go"
    );
    assert!(toast(&a).contains("not saved"), "{}", toast(&a));
    // ...and the deletion itself refuses too.
    assert!(!a.library_delete_file(&l.file));
    assert!(l.file.exists());
    assert!(
        read(&l.file).starts_with("# Personal"),
        "not a byte of it changed"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn only_a_markdown_file_directly_in_the_library_folder_is_ever_deleted() {
    let (mut a, l) = lib("libw-delete-outside");
    let library = l.dir.join("library");
    // A Markdown file outside the folder, one in a folder inside it, a file that is not Markdown
    // inside it, and one that is not there.
    let outside = l.dir.join("outside.md");
    std::fs::write(&outside, "# Outside\n").unwrap();
    let deeper = library.join("sub");
    std::fs::create_dir_all(&deeper).unwrap();
    let inner = deeper.join("inner.md");
    std::fs::write(&inner, "# Inner\n").unwrap();
    let text = library.join("readme.txt");
    std::fs::write(&text, "not a library file").unwrap();
    // A path that climbs out of the folder and comes back to a file beside it.
    let climbing = library.join("..").join("outside.md");
    for path in [
        &outside,
        &inner,
        &text,
        &climbing,
        &library.join("gone.md"),
        &library,
    ] {
        assert!(!a.library_delete_file(path), "{path:?}");
        assert!(toast(&a).contains("left alone"), "{path:?}: {}", toast(&a));
    }
    assert!(outside.exists() && inner.exists() && text.exists() && l.file.exists());
    assert!(!a.library_own_file(&outside) && a.library_own_file(&l.file));
    // The same refusal from the menu: a group whose file is not one of the folder's.
    a.docs.library.groups[0].file = outside.clone();
    a.ui.library.build(&a.docs.library);
    pick(&mut a, LibAct::DeleteFile, 0);
    assert!(a.ui.dialog.is_none());
    assert!(outside.exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn only_a_group_row_offers_to_delete_a_library_file() {
    let note = Kind::Note {
        path: PathBuf::from("D:/n.md"),
        missing: false,
        line: 3,
    };
    let has = |kind: &Kind| {
        crate::overlays::library_row_menu(kind, 1, true, 0.0, 0.0, false)
            .items
            .iter()
            .any(|i| i.id == Act::Library(LibAct::DeleteFile, 1))
    };
    assert!(has(&Kind::Group));
    assert!(!has(&Kind::Section) && !has(&note));
    let empty = crate::overlays::library_empty_menu(true, 0.0, 0.0, false);
    assert!(
        !empty
            .items
            .iter()
            .any(|i| matches!(i.id, Act::Library(LibAct::DeleteFile, _)))
    );
}
