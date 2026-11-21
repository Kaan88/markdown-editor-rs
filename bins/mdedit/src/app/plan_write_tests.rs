//! "Write the notes": the order of 4.3 through the window, the gate every library write goes
//! through, the report's "Undo this run", and the Notes pane's "Plan ready" row.

use super::files_tests::{app, open_now, scratch, type_text};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use crate::ui::Hit;

fn press(a: &mut App, key: KeyName) {
    a.key_pressed(Key::Name(key), None);
}

fn app_in(dir: &Path) -> App {
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a
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

/// A document with two notes under `# Tokyo`, a library folder, and the plan open on it.
fn planned(tag: &str) -> (App, PathBuf, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active(), "the plan opens");
    a.compose();
    (a, dir.join("library"), source, dir)
}

#[test]
fn write_the_notes_writes_the_notes_their_links_and_a_record_and_not_the_source() {
    let (mut a, library, source, dir) = planned("plan-write");
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_none(), "the card closes when it writes");
    assert!(a.plan_parked.is_none());
    // The notes, with their headings promoted, in the group's folder and its section's.
    assert_eq!(
        read(&library.join("Trip").join("Tokyo").join("Asakusa.md")),
        "# Asakusa\n\nthe river\n"
    );
    assert_eq!(
        read(&library.join("Trip").join("Tokyo").join("Nara.md")),
        "# Nara\n\nthe deer\n"
    );
    // The group's library file and its links.
    let group = read(&library.join("Trip.md"));
    assert!(group.starts_with("# Trip\n"), "{group}");
    assert!(group.contains("home:"), "{group}");
    assert!(
        group.contains("- [Asakusa](<Trip/Tokyo/Asakusa.md>)")
            || group.contains("- [Asakusa](Trip/Tokyo/Asakusa.md)"),
        "{group}"
    );
    // The record: state done, the source's digest, every note's digest.
    let runs = mdedit_core::runs::list(&library);
    assert_eq!(runs.len(), 1, "one record");
    let record = &runs[0].record;
    assert_eq!(record.state, mdedit_core::runs::State::Done);
    assert_eq!(record.notes.len(), 2);
    assert_eq!(record.notes[0].line, "- [Asakusa](Trip/Tokyo/Asakusa.md)");
    assert_eq!(
        record.notes[0].sha256,
        mdshared_app::update::sha256_hex(b"# Asakusa\n\nthe river\n")
    );
    assert_eq!(
        record.source.sha256,
        mdshared_app::update::sha256_hex(&std::fs::read(&source).expect("the source"))
    );
    // The report takes the panel, with the way to undo it.
    let outcome =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .expect("the report is there");
    assert!(outcome.run.is_some(), "Undo this run is offered");
    assert!(a.ui.results.open, "the panel is open");
    // The source is exactly what it was.
    assert_eq!(
        read(&source),
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_from_the_report_takes_the_run_back_and_marks_the_record() {
    let (mut a, library, source, dir) = planned("plan-undo");
    press(&mut a, KeyName::Enter);
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    // The question says what goes, and nothing has gone yet.
    a.run_undo_ask(record_path.clone());
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(DialogKind::RunUndo)
    );
    let question = a.ui.runs.undo.clone().expect("the question").1;
    assert!(question.contains("2 notes"), "{question}");
    assert!(question.contains("Trip"), "{question}");
    assert!(library.join("Trip/Tokyo/Asakusa.md").is_file());
    // The answer.
    a.run_undo(record_path.clone());
    assert!(!library.join("Trip/Tokyo/Asakusa.md").exists());
    assert!(!library.join("Trip/Tokyo/Nara.md").exists());
    assert!(!library.join("Trip.md").exists());
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record stays");
    assert_eq!(record.state, mdedit_core::runs::State::Undone);
    assert_eq!(
        read(&source),
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_group_file_that_cannot_be_made_leaves_the_notes_unlinked_and_the_record_says_so() {
    let dir = scratch("plan-links");
    let mut a = app_in(&dir);
    // A folder where the group's library file would go: the library cannot be updated.
    std::fs::create_dir_all(dir.join("library").join("Trip.md")).expect("the folder");
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    press(&mut a, KeyName::Enter);
    // The note is there, with nothing listing it.
    assert_eq!(
        read(&dir.join("library/Trip/Tokyo/Asakusa.md")),
        "# Asakusa\n\nthe river\n"
    );
    let runs = mdedit_core::runs::list(&dir.join("library"));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].record.state, mdedit_core::runs::State::LinksPending);
    assert!(runs[0].record.notes[0].path.is_file());
    assert_eq!(runs[0].record.notes.len(), 2);
    // The toast says so, in the words of 4.3.
    let said = toast(&a);
    assert!(
        said.starts_with("The notes are written, but Trip.md could not be updated"),
        "{said}"
    );
    assert!(said.ends_with("Try again / Undo"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plan_that_is_a_demonstration_says_it_writes_nothing_and_a_parked_plan_is_offered_in_the_pane()
{
    let mut a = app();
    a.command(C::ToggleLibraryPane);
    assert!(a.ui.panes.files && a.ui.library.on, "the Notes pane is up");
    assert!(a.plan_demo("split-gaps"));
    {
        let p = a.plan.as_ref().expect("open");
        assert_eq!(
            p.problem().as_deref(),
            Some("A demonstration of the plan screen: it writes nothing.")
        );
    }
    a.compose();
    // Enter on a disabled button writes nothing.
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_some(), "the card stays");
    {
        let p = a.plan.as_ref().expect("open");
        assert_eq!(p.write_pressed, 0, "nothing was pressed");
    }
    // A change, then Escape: the pane offers the plan back at its foot.
    press(&mut a, KeyName::Space);
    press(&mut a, KeyName::Escape);
    assert!(a.plan.is_none() && a.plan_parked.is_some());
    let notes = a
        .plan_parked
        .as_ref()
        .map(|p| p.plan.notes().len())
        .expect("a parked plan");
    let before = a.screen.pixels.clone();
    a.compose();
    assert!(
        before != a.screen.pixels,
        "the pane is painted with the row at its foot"
    );
    let files = Region::Files.index();
    let hit = |a: &App, hit: Hit| {
        a.hits[files]
            .iter()
            .find(|(h, _)| *h == hit)
            .map(|(_, r)| *r)
    };
    let show = hit(&a, Hit::LibraryPlanShow).expect("Show is drawn");
    let drop = hit(&a, Hit::LibraryPlanDrop).expect("Drop is drawn");
    assert!(show.x < drop.x, "Show comes before Drop");
    // Show brings the same plan back.
    a.pointer_moved(show.cx(), show.cy());
    a.left_press();
    a.left_release();
    assert!(a.plan.is_some(), "Show opens the plan again");
    assert!(a.plan.as_ref().expect("open").plan.notes().len() == notes);
    press(&mut a, KeyName::Escape);
    assert!(a.plan_parked.is_some(), "and it is kept again");
    // Drop forgets it, and the row goes.
    a.compose();
    let drop = hit(&a, Hit::LibraryPlanDrop).expect("Drop is drawn");
    a.pointer_moved(drop.cx(), drop.cy());
    a.left_press();
    a.left_release();
    assert!(a.plan_parked.is_none());
    a.compose();
    assert!(hit(&a, Hit::LibraryPlanShow).is_none(), "the row is gone");
}

#[test]
fn the_plan_ready_row_is_in_the_notes_panes_repaint_signature() {
    let mut a = app();
    a.command(C::ToggleLibraryPane);
    assert!(a.plan_demo("split-gaps"));
    press(&mut a, KeyName::Space);
    press(&mut a, KeyName::Escape);
    assert!(a.plan_parked.is_some());
    a.compose();
    let with = a.screen.pixels.clone();
    // Only the parked plan goes: nothing else changes, so a region not repainted keeps its
    // pixels, and the row must be gone from the frame.
    a.plan_parked = None;
    a.compose();
    assert!(
        with != a.screen.pixels,
        "the pane is painted again when the row goes"
    );
}

#[test]
fn split_into_notes_is_disabled_with_its_reason_until_the_file_on_disk_is_what_the_tab_holds() {
    let dir = scratch("split-reason");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, "# One\n\ntext\n\n## Two\n\nmore\n").expect("the file");
    open_now(&mut a, &source);
    assert!(crate::palette::enabled(
        C::SplitIntoNotes,
        &a.docs,
        &a.ui,
        true
    ));
    type_text(&mut a, "x");
    assert!(a.docs.active_doc().is_some_and(|d| d.modified()));
    assert!(!crate::palette::enabled(
        C::SplitIntoNotes,
        &a.docs,
        &a.ui,
        true
    ));
    let reason = crate::palette::split_reason(&a.docs).expect("the reason");
    assert_eq!(
        reason,
        "Save Trip.md first: the notes are made from the file on disk."
    );
    // The command says the same thing and opens nothing.
    a.command(C::SplitIntoNotes);
    assert!(a.plan.is_none());
    assert_eq!(
        toast(&a),
        "Save Trip.md first: the notes are made from the file on disk."
    );
    // Saved, it works.
    a.command(C::Save);
    a.plan_open_active();
    assert!(a.plan.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_undo_whose_library_file_has_unsaved_changes_keeps_the_notes_and_can_be_pressed_again() {
    let (mut a, library, _source, dir) = planned("plan-undo-again");
    press(&mut a, KeyName::Enter);
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    // The owner opens the group's library file and types in it: unsaved changes, which is the
    // ordinary reason the gate refuses to write that file.
    let file = library.join("Trip.md");
    open_now(&mut a, &file);
    a.mods = crate::keys::Mods {
        ctrl: true,
        ..Default::default()
    };
    press(&mut a, KeyName::End);
    a.mods = crate::keys::Mods::default();
    type_text(&mut a, "mine\n");
    let on_disk = read(&file);
    a.run_undo(record_path.clone());
    // Nothing went: the lines are still there, so the notes stay with them.
    assert!(library.join("Trip/Tokyo/Asakusa.md").is_file());
    assert!(library.join("Trip/Tokyo/Nara.md").is_file());
    assert_eq!(read(&file), on_disk, "the library file was not written");
    assert!(read(&file).contains("[Asakusa]"));
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record");
    assert_eq!(record.state, mdedit_core::runs::State::LinksPending);
    // The report says what happened, and why, and that it can be tried again.
    let outcome = a.ui.results.replace.outcome.as_ref().expect("the report");
    assert!(
        outcome.title.starts_with("The run is not undone"),
        "{}",
        outcome.title
    );
    assert!(
        outcome.title.contains("Save or close Trip.md first"),
        "{}",
        outcome.title
    );
    // Saved, the same press finishes the job.
    a.command(C::Save);
    assert!(!a.docs.active_doc().is_some_and(|d| d.modified()));
    a.run_undo(record_path.clone());
    assert!(!library.join("Trip/Tokyo/Asakusa.md").exists());
    assert!(!library.join("Trip/Tokyo/Nara.md").exists());
    assert!(!read(&file).contains("[Asakusa]"), "{}", read(&file));
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record");
    assert_eq!(record.state, mdedit_core::runs::State::Undone);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_run_that_cannot_start_keeps_the_plan_and_its_edits_on_screen() {
    let (mut a, library, source, dir) = planned("plan-start");
    std::fs::create_dir_all(&library).expect("the library");
    // A file where the records' folder would go: step 1 cannot make the record, so the run
    // cannot start and nothing is written.
    std::fs::write(library.join(".runs"), "not a folder").expect("in the way");
    {
        let s = a.plan.as_mut().expect("the plan is open");
        s.rename(0, "Mine");
        assert!(
            s.target_text(0).ends_with("Mine.md"),
            "{}",
            s.target_text(0)
        );
    }
    let notes = a.plan.as_ref().expect("open").plan.notes().len();
    press(&mut a, KeyName::Enter);
    // The card stays, with the plan and the rename still in it, and says why.
    let s = a.plan.as_ref().expect("the card stays");
    assert_eq!(s.plan.notes().len(), notes);
    assert!(
        s.target_text(0).ends_with("Mine.md"),
        "the rename is still there: {}",
        s.target_text(0)
    );
    let said = toast(&a);
    assert!(
        said.starts_with("The run could not start: "),
        "the reason is on the bar: {said}"
    );
    assert!(!library.join("Trip").exists(), "no note was written");
    assert_eq!(
        read(&source),
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
