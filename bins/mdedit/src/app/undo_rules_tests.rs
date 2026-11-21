//! Undo through the window: the gate it writes the library through, the question it asks and the
//! list of runs it opens (audit 3: A2, A3, A10). The rules themselves, on disk, are tested in
//! `crates/mdedit-core/tests/undo_rules.rs`.

use super::files_tests::{app, open_now, scratch};
use super::plan_write::PlanGate;
use super::*;
use crate::keys::{Key, KeyName};
use mdedit_core::runs::{self, Gate, LinkRemoval, State};

/// No heading above the notes: the notes have no section, so no section heading goes with them.
const FLAT: &str = "## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n";

fn press(a: &mut App, key: KeyName) {
    a.key_pressed(Key::Name(key), None);
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn toast(a: &App) -> String {
    a.docs
        .toast(Instant::now())
        .map(str::to_string)
        .unwrap_or_default()
}

/// `Trip.md` holding `text`, split and written through the plan screen.
fn written(tag: &str, text: &str) -> (App, PathBuf, PathBuf, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    let source = dir.join("Trip.md");
    std::fs::write(&source, text).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active(), "the plan opens");
    a.compose();
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_none(), "the card closes when it writes");
    let record =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    (a, dir.join("library"), record, source, dir)
}

fn title(a: &App) -> String {
    a.ui.results
        .replace
        .outcome
        .as_ref()
        .map(|o| o.title.clone())
        .unwrap_or_default()
}

#[test]
fn undo_of_notes_under_no_section_takes_their_lines_out_through_the_window() {
    let (mut a, library, record, _source, dir) = written("undo-flat", FLAT);
    let group = library.join("Trip.md");
    assert!(library.join("Trip/Asakusa.md").is_file());
    assert!(read(&group).contains("[Asakusa]"));
    a.run_undo(record.clone());
    assert!(
        !library.join("Trip/Asakusa.md").exists(),
        "the notes are removed"
    );
    assert!(!library.join("Trip/Nara.md").exists());
    // No line is left naming a file that is gone: the group's file went with them.
    assert!(
        !group.exists(),
        "the lines are out of the library file: {}",
        read(&group)
    );
    assert_eq!(
        runs::load(&record, &library).expect("it").state,
        State::Undone
    );
    assert_eq!(title(&a), "Undid the run \u{00B7} 2 files and 2 lines");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_of_notes_into_a_group_that_was_there_leaves_the_groups_other_lines() {
    // The run's notes go under no section of a group file the owner has had for a while, with a
    // line of the owner's own: only the run's two lines go.
    let dir = scratch("undo-flat-old");
    let library = dir.join("library");
    std::fs::create_dir_all(library.join("Trip")).expect("the folder");
    std::fs::write(library.join("Trip/Mine.md"), "# Mine\n").expect("a note");
    std::fs::write(
        library.join("Trip.md"),
        "# Trip\nhome: Trip\n\n- [Mine](Trip/Mine.md)\n",
    )
    .expect("the group");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    let source = dir.join("Trip.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    a.compose();
    press(&mut a, KeyName::Enter);
    let record =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    assert!(read(&library.join("Trip.md")).contains("[Nara]"));
    a.run_undo(record);
    assert!(!library.join("Trip/Nara.md").exists());
    assert_eq!(
        read(&library.join("Trip.md")),
        "# Trip\nhome: Trip\n\n- [Mine](Trip/Mine.md)\n",
        "the owner's group is as it was"
    );
    assert!(library.join("Trip/Mine.md").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_windows_gate_says_so_when_there_is_no_line_of_the_run_to_take_out() {
    let (mut a, library, _record, _source, dir) = written("undo-nothing", FLAT);
    let group = library.join("Trip.md");
    let before = read(&group);
    let removal = LinkRemoval {
        path: library.join("Trip/Nowhere.md"),
        sections: Vec::new(),
        line: "- [Nowhere](Trip/Nowhere.md)".to_string(),
    };
    let mut gate = PlanGate {
        app: &mut a,
        library: library.clone(),
    };
    let said = gate.remove_links(&group, &[removal], &[]);
    assert!(
        said.is_err(),
        "nothing written is not success: the notes of those lines would be removed under it"
    );
    assert_eq!(read(&group), before, "and the file is as it was");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_question_of_a_run_whose_original_is_gone_is_a_refusal_and_nothing_is_removed() {
    let (mut a, library, record, source, dir) = written("undo-gone", FLAT);
    std::fs::remove_file(&source).expect("the owner deletes the original");
    a.run_undo_ask(record.clone());
    let (_, sentence, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(!can, "there is nothing to answer yes to");
    assert!(sentence.contains("is not where it was"), "{sentence}");
    assert!(sentence.contains("may be the only copy"), "{sentence}");
    assert!(toast(&a).contains("is not where it was"), "{}", toast(&a));
    // Even when the answer is sent as it would be from the keyboard.
    a.run_undo(record.clone());
    assert!(
        library.join("Trip/Asakusa.md").is_file(),
        "the only copy is still there"
    );
    assert!(library.join("Trip/Nara.md").is_file());
    assert!(read(&library.join("Trip.md")).contains("[Asakusa]"));
    assert_eq!(
        runs::load(&record, &library).expect("it").state,
        State::Done
    );
    assert!(title(&a).contains("is not where it was"), "{}", title(&a));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ai_runs_lists_a_record_of_a_later_version_with_nothing_to_press() {
    let (mut a, library, record, _source, dir) = written("undo-newer", FLAT);
    let newer = library
        .join(".runs")
        .join("2026-11-21T14-03-11Z-import.json");
    let text = std::fs::read_to_string(&record)
        .expect("the record")
        .replace("\"kind\": \"split\"", "\"kind\": \"import\"")
        .replace("\"version\": 1", "\"version\": 2");
    std::fs::write(&newer, text).expect("a later version's record");
    a.open_runs();
    assert_eq!(a.ui.runs.list.len(), 1, "the one this version wrote");
    assert_eq!(a.ui.runs.unread.len(), 1);
    assert!(a.ui.runs.unread[0].newer);
    assert_eq!(a.ui.runs.unread[0].started, "2026-11-21T14:03:11Z");
    a.compose();
    // The rows are not selectable: nothing a click or a key reaches is a record that cannot act.
    assert!(
        a.overlay_hits
            .iter()
            .filter(|(h, _)| matches!(h, crate::ui::Hit::DlgOpen(_)))
            .count()
            == 1,
        "only the run this version can read is a row to open"
    );
    assert!(newer.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}
