//! A note is not a group: "New note here…" must not make one, nothing may delete a note as a
//! library file, links inside the library folder are relative to it, and one rule names the
//! files that are written.

use super::files_tests::{open_now, scratch};
use super::library_write_tests::{app_in, labels, lib, name_it, pick, read, slash, toast};
use super::*;
use crate::commands::CommandId as C;
use crate::library_pane::Kind;
use crate::ui::LibAct;

/// The names of the Markdown files directly in `dir`, sorted.
fn md_files(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".md"))
        .collect();
    names.sort();
    names
}

fn group_rows(a: &App) -> Vec<String> {
    a.ui.library
        .rows
        .iter()
        .filter(|r| r.kind == Kind::Group)
        .map(|r| r.label.clone())
        .collect()
}

/// The row of the note labelled `label`.
fn note_row(a: &App, label: &str) -> usize {
    a.ui.library
        .rows
        .iter()
        .position(|r| matches!(r.kind, Kind::Note { .. }) && r.label == label)
        .unwrap_or_else(|| panic!("no note row {label:?} in {:?}", labels(a)))
}

// ---- the leak ----------------------------------------------------------------------------------

#[test]
fn a_note_made_with_new_note_here_does_not_become_a_group() {
    let (mut a, l) = lib("libl-no-group");
    assert_eq!(group_rows(&a), vec!["Personal"]);
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    assert_eq!(
        group_rows(&a),
        vec!["Personal"],
        "no second group: {:?}",
        labels(&a)
    );
    assert_eq!(a.docs.library.groups.len(), 1);
    // It is a note, listed under the heading it was made under.
    let at = note_row(&a, "Budget");
    assert!(matches!(
        a.ui.library.rows[at].kind,
        Kind::Note { missing: false, .. }
    ));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_made_with_new_note_here_is_not_written_in_the_library_folder() {
    let (mut a, l) = lib("libl-folder");
    let library = l.dir.join("library");
    assert_eq!(md_files(&library), vec!["personal.md"]);
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    assert_eq!(
        md_files(&library),
        vec!["personal.md"],
        "only library files are directly in the library folder"
    );
    // In a folder of its own beside them, named for the group's file.
    assert_eq!(
        read(&library.join("personal").join("Budget.md")),
        "# Budget\n"
    );
    // A second note of the same name does not overwrite the first.
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    assert_eq!(md_files(&library), vec!["personal.md"]);
    assert!(library.join("personal").join("Budget 2.md").exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn many_new_notes_leave_the_library_files_inside_the_limit_of_files_read() {
    let (mut a, l) = lib("libl-limit");
    let library = l.dir.join("library");
    // A library file that sorts after every note's name would be the first to drop out.
    std::fs::write(
        library.join("zzz.md"),
        "# Last group\n\n- [Taxes](../taxes.md)\n",
    )
    .unwrap();
    for n in 0..mdshared_app::library::MAX_FILES + 6 {
        pick(&mut a, LibAct::NewNote, 1);
        name_it(&mut a, &format!("Note {n:03}"));
    }
    assert_eq!(md_files(&library).len(), 2);
    assert_eq!(group_rows(&a), vec!["Personal", "Last group"]);
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- a note already in the library folder ------------------------------------------------------

/// Adds `Budget.md` directly in the library folder with `body`, listed by the first group under
/// Money as `target`.
fn flat_note(
    a: &mut App,
    l: &super::library_write_tests::Lib,
    body: &str,
    target: &str,
) -> PathBuf {
    let flat = l.dir.join("library").join("Budget.md");
    std::fs::write(&flat, body).unwrap();
    let text = l.text().replacen(
        "## Money\n\n",
        &format!("## Money\n\n- [Budget]({target})\n"),
        1,
    );
    std::fs::write(&l.file, text).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    flat
}

#[test]
fn a_note_already_in_the_library_folder_is_listed_as_a_note_and_not_as_a_group() {
    let (mut a, l) = lib("libl-flat");
    flat_note(&mut a, &l, "# Budget\n", "Budget.md");
    assert_eq!(group_rows(&a), vec!["Personal"], "{:?}", labels(&a));
    assert!(labels(&a).contains(&"Budget".to_string()), "as a note");
    assert_eq!(
        a.docs.library.groups.len(),
        1,
        "for every list that reads the groups"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_note_in_the_library_folder_stays_hidden_after_it_is_written_in() {
    // The note is the first thing a user writes in: headings are the usual start of a note, and
    // the reader takes them for sections of a group.
    let (mut a, l) = lib("libl-flat-written");
    flat_note(
        &mut a,
        &l,
        "# Budget\n\n## Agenda\n\n### Later\n\nwords words\n",
        "Budget.md",
    );
    assert_eq!(group_rows(&a), vec!["Personal"], "{:?}", labels(&a));
    assert!(
        !labels(&a).contains(&"Agenda".to_string()),
        "{:?}",
        labels(&a)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_library_file_with_notes_of_its_own_stays_a_group_even_when_another_lists_it() {
    let (mut a, l) = lib("libl-linked-group");
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, format!("# Work\n\n- [Taxes]({})\n", slash(&l.taxes))).unwrap();
    std::fs::write(
        &l.file,
        l.text()
            .replacen("## Money\n\n", "## Money\n\n- [Work](work.md)\n", 1),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert_eq!(group_rows(&a), vec!["Personal", "Work"]);
    // And a new, empty group nobody lists is a group.
    std::fs::write(l.dir.join("library").join("empty.md"), "# Empty\n").unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert_eq!(group_rows(&a), vec!["Empty", "Personal", "Work"]);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_library_with_a_note_among_its_files_is_not_read_again_until_something_changes() {
    let (mut a, l) = lib("libl-stamps");
    flat_note(&mut a, &l, "# Budget\n", "Budget.md");
    a.library_load();
    // Marked in memory: a library read again would have its own name back.
    a.docs.library.groups[0].name = "marker".into();
    a.library_load();
    a.library_load();
    assert_eq!(
        a.docs.library.groups[0].name, "marker",
        "read again with nothing changed"
    );
    // A file saved since is read.
    std::thread::sleep(std::time::Duration::from_millis(30));
    std::fs::write(&l.file, format!("{}\n", read(&l.file))).unwrap();
    a.library_load();
    assert_eq!(a.docs.library.groups[0].name, "Personal");
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- delete refuses a note ---------------------------------------------------------------------

#[test]
fn delete_never_removes_a_file_the_library_lists_as_a_note() {
    let (mut a, l) = lib("libl-delete-note");
    let flat = flat_note(&mut a, &l, "# Budget\n", "Budget.md");
    assert!(!a.library_delete_file(&flat));
    assert_eq!(
        read(&flat),
        "# Budget\n",
        "the note is still there, as it was"
    );
    let says = toast(&a);
    assert!(
        says.contains("Budget.md") && says.contains("Personal") && says.contains("Money"),
        "{says}"
    );
    assert!(
        says.contains("not a library file") && says.contains("Remove it there"),
        "{says}"
    );
    assert!(a.ui.dialog.is_none());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn delete_knows_a_note_by_any_spelling_of_its_path() {
    for (tag, target) in [
        ("dot", "./Budget.md"),
        ("up", "../library/Budget.md"),
        ("absolute", "ABS"),
    ] {
        let (mut a, l) = lib(&format!("libl-delete-spelling-{tag}"));
        let flat = l.dir.join("library").join("Budget.md");
        let target = if target == "ABS" {
            slash(&flat)
        } else {
            target.to_string()
        };
        flat_note(&mut a, &l, "# Budget\n", &target);
        assert!(!a.library_delete_file(&flat), "{target}");
        assert!(flat.exists(), "{target}");
        assert!(
            toast(&a).contains("not a library file"),
            "{target}: {}",
            toast(&a)
        );
        let _ = std::fs::remove_dir_all(&l.dir);
    }
}

#[test]
fn delete_is_refused_from_the_menu_of_a_group_that_is_a_note_with_links_in_it() {
    // A note holding a list of links reads as a group with notes of its own, so it is shown;
    // its menu must still not delete it.
    let (mut a, l) = lib("libl-delete-linky");
    let index = l.dir.join("library").join("Index.md");
    std::fs::write(
        &index,
        format!("# Index\n\n- [Taxes]({})\n", slash(&l.taxes)),
    )
    .unwrap();
    std::fs::write(
        &l.file,
        l.text()
            .replacen("## Money\n\n", "## Money\n\n- [Index](Index.md)\n", 1),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    let at =
        a.ui.library
            .rows
            .iter()
            .position(|r| r.kind == Kind::Group && r.label == "Index")
            .expect("a note with notes of its own is shown");
    pick(&mut a, LibAct::DeleteFile, at);
    assert!(a.ui.dialog.is_none(), "no question about deleting a note");
    assert!(toast(&a).contains("not a library file"), "{}", toast(&a));
    assert!(index.exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_delete_guard_reads_the_library_as_it_is_on_disk_and_not_as_last_drawn() {
    // The pane was drawn before another window listed the file as a note.
    let (mut a, l) = lib("libl-delete-stale");
    let other = l.dir.join("library").join("other.md");
    std::fs::write(&other, "# Other\n").unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert_eq!(group_rows(&a), vec!["Other", "Personal"]);
    std::fs::write(
        &l.file,
        l.text()
            .replacen("## Money\n\n", "## Money\n\n- [Other](other.md)\n", 1),
    )
    .unwrap();
    assert!(!a.library_delete_file(&other));
    assert!(other.exists());
    let _ = std::fs::remove_dir_all(&l.dir);
}

// ---- links inside the library folder -----------------------------------------------------------

#[test]
fn a_note_made_inside_the_library_folder_is_linked_without_a_drive_and_survives_a_move() {
    let (mut a, l) = lib("libl-relative");
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "Budget");
    let text = read(&l.file);
    assert!(text.contains("- [Budget](personal/Budget.md)"), "{text}");
    let link = text.lines().find(|t| t.contains("[Budget]")).unwrap();
    assert!(!link.contains(':') && !link.contains("//"), "{link}");

    // The folder is renamed (moved, synced elsewhere): the note is still found.
    let moved = l.dir.join("library-moved");
    std::fs::rename(l.dir.join("library"), &moved).unwrap();
    let read_again = mdshared_app::library::read(&moved);
    let note = read_again
        .entries()
        .find(|(_, e)| e.title == "Budget")
        .map(|(_, e)| e.clone())
        .expect("the note is listed");
    assert!(!note.missing, "{note:?}");
    assert_eq!(std::fs::read_to_string(&note.path).unwrap(), "# Budget\n");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_link_to_a_file_outside_the_library_folder_stays_absolute() {
    let (mut a, l) = lib("libl-absolute");
    let outside = l.dir.join("receipts.md");
    std::fs::write(&outside, "# Receipts\n").unwrap();
    open_now(&mut a, &outside);
    a.command(C::LibraryAdd);
    super::library_tests::choose(&mut a, "Money");
    let text = read(&l.file);
    assert!(
        text.contains(&format!("- [Receipts]({})", slash(&outside))),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_file_inside_the_library_folder_added_to_the_library_is_linked_relative_to_it() {
    let (mut a, l) = lib("libl-add-inside");
    let inside = l.dir.join("library").join("sub").join("plan.md");
    std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
    std::fs::write(&inside, "# Plan\n").unwrap();
    open_now(&mut a, &inside);
    a.command(C::LibraryAdd);
    super::library_tests::choose(&mut a, "Money");
    let text = read(&l.file);
    assert!(text.contains("- [Plan](sub/plan.md)"), "{text}");
    assert!(
        !a.docs
            .library
            .entries()
            .any(|(_, e)| e.title == "Plan" && e.missing)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn find_the_file_inside_the_library_folder_writes_a_relative_link() {
    let (mut a, l) = lib("libl-find-inside");
    let found = l.dir.join("library").join("found").join("taxes.md");
    std::fs::create_dir_all(found.parent().unwrap()).unwrap();
    std::fs::rename(&l.taxes, &found).unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    let Kind::Note { line, .. } = a.ui.library.rows[2].kind else {
        unreachable!()
    };
    a.library_set_note_path(&l.file, line, &l.taxes, &found);
    let text = read(&l.file);
    assert!(text.contains("](found/taxes.md)"), "{text}");
    assert!(matches!(
        a.ui.library.rows[2].kind,
        Kind::Note { missing: false, .. }
    ));
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_relative_link_is_written_with_forward_slashes_and_brackets_for_a_space() {
    let (mut a, l) = lib("libl-relative-space");
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "My plan");
    assert!(
        read(&l.file).contains("](<personal/My plan.md>)"),
        "{}",
        read(&l.file)
    );
    assert!(
        a.docs
            .library
            .entries()
            .any(|(_, e)| e.title == "My plan" && !e.missing)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_new_note_in_a_library_folder_that_is_not_there_yet_is_made_with_its_folder() {
    let dir = scratch("libl-no-folder");
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    pick(&mut a, LibAct::NewGroup, 0);
    name_it(&mut a, "Home");
    pick(&mut a, LibAct::NewNote, 0);
    name_it(&mut a, "Budget");
    assert_eq!(
        read(&dir.join("library").join("Home").join("Budget.md")),
        "# Budget\n"
    );
    assert!(read(&dir.join("library").join("Home.md")).contains("](Home/Budget.md)"));
    assert_eq!(group_rows(&a), vec!["Home"]);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- one name rule -----------------------------------------------------------------------------

#[test]
fn a_note_named_with_a_percent_is_written_under_a_name_the_reader_reads_back() {
    let (mut a, l) = lib("libl-percent");
    pick(&mut a, LibAct::NewNote, 1);
    name_it(&mut a, "50%20off");
    let note = l.dir.join("library").join("personal").join("50_20off.md");
    assert_eq!(read(&note), "# 50%20off\n");
    let at = note_row(&a, "50%20off");
    let Kind::Note { path, missing, .. } = &a.ui.library.rows[at].kind else {
        unreachable!()
    };
    assert!(!missing, "{path:?}");
    assert_eq!(
        path.file_name().and_then(|n| n.to_str()),
        Some("50_20off.md")
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn every_name_that_is_written_goes_through_the_one_rule() {
    use super::library_write::file_stem_for;
    // Windows device names, all of them, in any case and with any extension.
    for device in [
        "CON", "prn", "Aux", "NUL", "COM1", "com7", "COM9", "LPT3", "lpt9",
    ] {
        let stem = file_stem_for(device).unwrap();
        assert_eq!(stem, format!("{device}_"), "{device}");
    }
    assert_eq!(file_stem_for("COM7.txt").as_deref(), Some("COM7_.txt"));
    assert_eq!(file_stem_for("COM10").as_deref(), Some("COM10"));
    // The percent sign, and what Windows forbids.
    assert_eq!(file_stem_for("50%20off").as_deref(), Some("50_20off"));
    assert_eq!(file_stem_for("a/b\\c:d").as_deref(), Some("a_b_c_d"));
    assert_eq!(file_stem_for("  Budget. ").as_deref(), Some("Budget"));
    // Nothing to name a file by.
    assert_eq!(file_stem_for(""), None);
    assert_eq!(file_stem_for("   "), None);
    assert_eq!(file_stem_for("..."), None);
    assert_eq!(file_stem_for("///"), None);
    // The same names, by the shared function the section folders use.
    for name in ["CON", "COM7", "50%20off", "a:b"] {
        assert_eq!(
            file_stem_for(name).unwrap(),
            mdshared_app::library::folder_name(name),
            "{name}"
        );
    }
}

#[test]
fn a_new_group_named_with_a_device_name_gets_a_safe_file() {
    let dir = scratch("libl-device-group");
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    pick(&mut a, LibAct::NewGroup, 0);
    name_it(&mut a, "CON");
    assert!(dir.join("library").join("CON_.md").exists());
    assert_eq!(group_rows(&a), vec!["CON"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_note_moved_to_another_library_file_is_linked_by_the_same_rule() {
    let (mut a, l) = lib("libl-move-relative");
    let inside = l.dir.join("library").join("sub").join("plan.md");
    std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
    std::fs::write(&inside, "# Plan\n").unwrap();
    // Listed by hand with its whole path, in one file; moved to another.
    let work = l.dir.join("library").join("work.md");
    std::fs::write(&work, "# Work\n\n## Projects\n").unwrap();
    std::fs::write(
        &l.file,
        l.text().replacen(
            "## Money\n\n",
            &format!("## Money\n\n- [Plan]({})\n", slash(&inside)),
            1,
        ),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    let at = note_row(&a, "Plan");
    pick(&mut a, LibAct::MoveTo, at);
    super::library_tests::choose(&mut a, "Projects");
    assert_eq!(
        read(&work),
        "# Work\n\n## Projects\n- [Plan](sub/plan.md)\n"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[cfg(windows)]
#[test]
fn delete_knows_a_note_listed_through_a_junction_to_the_library_folder() {
    let (mut a, l) = lib("libl-junction");
    let junction = l.dir.join("via");
    let made = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(l.dir.join("library"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    assert!(made, "a junction could not be made");
    let flat = l.dir.join("library").join("Budget.md");
    std::fs::write(&flat, "# Budget\n").unwrap();
    // Listed as a different path to the same file.
    let through = junction.join("Budget.md");
    std::fs::write(
        &l.file,
        l.text().replacen(
            "## Money\n\n",
            &format!("## Money\n\n- [Budget]({})\n", slash(&through)),
            1,
        ),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    assert!(!a.library_delete_file(&flat));
    assert!(flat.exists());
    assert!(toast(&a).contains("not a library file"), "{}", toast(&a));
    let _ = std::process::Command::new("cmd")
        .args(["/C", "rmdir"])
        .arg(&junction)
        .output();
    let _ = std::fs::remove_dir_all(&l.dir);
}
