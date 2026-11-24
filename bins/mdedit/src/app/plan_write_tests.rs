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
        read(&library.join("Trip").join("Tokyo").join("asakusa.md")),
        "# Asakusa\n\nthe river\n"
    );
    assert_eq!(
        read(&library.join("Trip").join("Tokyo").join("nara.md")),
        "# Nara\n\nthe deer\n"
    );
    // The group's library file and its links.
    let group = read(&library.join("Trip.md"));
    assert!(group.starts_with("# Trip\n"), "{group}");
    assert!(group.contains("home:"), "{group}");
    assert!(
        group.contains("- [Asakusa](<Trip/Tokyo/asakusa.md>)")
            || group.contains("- [Asakusa](Trip/Tokyo/asakusa.md)"),
        "{group}"
    );
    // The record: state done, the source's digest, every note's digest.
    let runs = mdedit_core::runs::list(&library);
    assert_eq!(runs.len(), 1, "one record");
    let record = &runs[0].record;
    assert_eq!(record.state, mdedit_core::runs::State::Done);
    assert_eq!(record.notes.len(), 2);
    assert_eq!(record.notes[0].line, "- [Asakusa](Trip/Tokyo/asakusa.md)");
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
    assert!(library.join("Trip/Tokyo/asakusa.md").is_file());
    // The answer.
    a.run_undo(record_path.clone());
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    assert!(!library.join("Trip/Tokyo/nara.md").exists());
    assert!(!library.join("Trip.md").exists());
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record stays");
    assert_eq!(record.state, mdedit_core::runs::State::Undone);
    assert_eq!(
        read(&source),
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, RB3: step 6 writes the group's file through `files::replace`, which refuses a
/// read-only file on every system. A read-only group is refused before anything is written, as
/// D2 refuses everything it can know beforehand, rather than leaving the notes unlinked.
#[test]
fn a_read_only_group_file_is_refused_before_the_notes_are_written() {
    let dir = scratch("plan-read-only");
    let library = dir.join("library");
    let group = library.join("Trip.md");
    std::fs::create_dir_all(&library).expect("the library");
    std::fs::write(&group, "# Trip\nhome: Trip\n").expect("the group");
    let set_read_only = |on: bool| {
        let mut perms = std::fs::metadata(&group).expect("the group").permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(on);
        std::fs::set_permissions(&group, perms).expect("its permissions");
    };
    set_read_only(true);
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    a.compose();
    // The card says so, and "Write the notes" cannot be pressed.
    let said = blocked(&a);
    assert!(said.contains("Trip.md is read-only"), "{said}");
    press(&mut a, KeyName::Enter);
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    // The write itself refuses it too, should it be reached.
    a.plan_write();
    let said = toast(&a);
    assert!(
        said.starts_with("The run could not start") && said.contains("is read-only"),
        "{said}"
    );
    assert!(a.plan.is_some(), "the plan stays on screen");
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    assert!(mdedit_core::runs::list(&library).is_empty(), "no record");
    assert_eq!(read(&group), "# Trip\nhome: Trip\n");
    set_read_only(false);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_group_file_that_cannot_be_written_leaves_the_notes_unlinked_and_the_record_says_so() {
    let dir = scratch("plan-links");
    // The group is there already, and folders stand where step 6's write would put its
    // temporary file (`files::replace`: `Trip.md.part`, then names with the process id): every
    // thing D2 checks passes, and that write is the one thing that fails -- a race, in the field.
    // Were the names to change, the write would go through and the state below would say so.
    // (Audit 4, RB3: before, one `Trip.md.tmp` folder did it, and the group was a new one; a new
    // group's file is made through the same temporary names, so it could not be made here, and
    // the group made before its notes is the test `the_groups_file_is_made_before_its_notes...`.)
    let group = dir.join("library").join("Trip.md");
    std::fs::create_dir_all(dir.join("library")).expect("the library");
    std::fs::write(&group, "# Trip\nhome: Trip\n").expect("the group");
    std::fs::create_dir_all(dir.join("library").join("Trip.md.part")).expect("the folder");
    for attempt in 1..16 {
        let name = format!("Trip.md.{}.{attempt}.part", std::process::id());
        std::fs::create_dir_all(dir.join("library").join(name)).expect("the folder");
    }
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    assert_eq!(
        a.plan.as_ref().expect("open").problem(),
        None,
        "nothing is known against it before the write"
    );
    press(&mut a, KeyName::Enter);
    // The note is there, with nothing listing it.
    assert_eq!(
        read(&dir.join("library/Trip/Tokyo/asakusa.md")),
        "# Asakusa\n\nthe river\n"
    );
    let runs = mdedit_core::runs::list(&dir.join("library"));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].record.state, mdedit_core::runs::State::LinksPending);
    assert!(runs[0].record.notes[0].path.is_file());
    assert_eq!(runs[0].record.notes.len(), 2);
    // The group's file is as it was, with no link written into it, and the record does not say
    // this run made it.
    assert_eq!(read(&group), "# Trip\nhome: Trip\n");
    assert!(!runs[0].record.group.created);
    // The toast says so, in the words of 4.3 as audit 4 reworded them.
    let said = toast(&a);
    assert!(
        said.starts_with("The notes are written, but Trip.md could not be updated"),
        "{said}"
    );
    // Its way on is named: AI runs, where Finish and Undo are (audit 4, F3).
    assert!(
        said.ends_with(crate::commands::strings::LINKS_PENDING_NEXT),
        "{said}"
    );
    // AI runs offers Finish for it, and Finish, once the write can be made, links both notes
    // (audit 3 C4, audit 4 F3).
    assert!(crate::overlays::offers_finish(&runs[0].record));
    for entry in std::fs::read_dir(dir.join("library")).expect("the library") {
        let path = entry.expect("an entry").path();
        if path.is_dir() && path.to_string_lossy().ends_with(".part") {
            std::fs::remove_dir(&path).expect("the folder goes");
        }
    }
    a.run_finish(runs[0].path.clone());
    let linked = read(&group);
    assert!(
        linked.contains("[Asakusa](Trip/Tokyo/asakusa.md)")
            && linked.contains("[Nara](Trip/Tokyo/nara.md)"),
        "{linked}"
    );
    let record = mdedit_core::runs::load(&runs[0].path, &dir.join("library")).expect("the record");
    assert_eq!(record.state, mdedit_core::runs::State::Done);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of the audit 4 fixes, T4: links pending, with a group file this run made. The window
/// makes the group's file before the notes; step 6's write is then refused (`REFUSE_LINKS`, as a
/// race would); and the record says the run made the file, so AI runs offers Finish for it, and
/// Undo takes the notes away and the group's file with them.
#[test]
fn links_pending_with_a_group_file_this_run_made_is_the_runs_to_take_away() {
    let (mut a, library, _source, dir) = planned("plan-links-made");
    let group = library.join("Trip.md");
    assert!(!group.exists(), "the group is a new one");
    super::plan_write::REFUSE_LINKS.with(|r| r.set(true));
    press(&mut a, KeyName::Enter);
    super::plan_write::REFUSE_LINKS.with(|r| r.set(false));
    let made = read(&group);
    assert!(
        made.contains("home: Trip"),
        "the group's file is made: {made}"
    );
    assert!(!made.contains("]("), "no link is in it: {made}");
    assert!(library.join("Trip/Tokyo/asakusa.md").is_file());
    let runs = mdedit_core::runs::list(&library);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].record.state, mdedit_core::runs::State::LinksPending);
    assert!(
        runs[0].record.group.created,
        "the record says this run made it"
    );
    let said = toast(&a);
    assert!(
        said.starts_with("The notes are written, but Trip.md could not be updated"),
        "{said}"
    );
    assert!(crate::overlays::offers_finish(&runs[0].record));
    a.run_undo(runs[0].path.clone());
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    assert!(!library.join("Trip/Tokyo/nara.md").exists());
    assert!(
        !group.exists(),
        "the group's file this run made goes with it"
    );
    let record = mdedit_core::runs::load(&runs[0].path, &library).expect("the record stays");
    assert_eq!(record.state, mdedit_core::runs::State::Undone);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of the audit 4 fixes, S3, through the window: undo gives the owner's group file back
/// byte for byte, its byte order mark, CRLF line ends and the blank line at its end, because the
/// window's gate opts in (`remove_links_added`).
#[test]
fn undo_gives_the_owners_group_file_back_byte_for_byte() {
    let dir = scratch("plan-undo-bytes");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let group = library.join("Trip.md");
    let before = "\u{FEFF}# Trip\r\nhome: Trip\r\n\r\nThe owner's line.\r\n\r\n";
    std::fs::write(&group, before).expect("the group");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    press(&mut a, KeyName::Enter);
    let linked = read(&group);
    assert!(
        linked.contains("[Asakusa](Trip/Tokyo/asakusa.md)"),
        "{linked}"
    );
    let runs = mdedit_core::runs::list(&library);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].record.state, mdedit_core::runs::State::Done);
    a.run_undo(runs[0].path.clone());
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    assert_eq!(
        String::from_utf8_lossy(&std::fs::read(&group).expect("the group")),
        before
    );
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
    assert!(library.join("Trip/Tokyo/asakusa.md").is_file());
    assert!(library.join("Trip/Tokyo/nara.md").is_file());
    assert_eq!(read(&file), on_disk, "the library file was not written");
    assert!(read(&file).contains("[Asakusa]"));
    // The undo began and could not do it all: the record says so, and offers Undo again but not
    // Finish, since the owner asked for the run to go (audit 4, A F2 and F3; before, it went back
    // to "links pending", which offered Finish).
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record");
    assert_eq!(record.state, mdedit_core::runs::State::UndoPending);
    assert!(!record.state.finishable() && record.state.live());
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
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    assert!(!library.join("Trip/Tokyo/nara.md").exists());
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
            s.target_text(0).ends_with("mine.md"),
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
        s.target_text(0).ends_with("mine.md"),
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

// ---- D2: everything that can be refused is refused before the first file ------------------------

/// A heading above the notes: the run writes a section heading into the library file.
const SECTIONED: &str = "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n";
/// No heading above the notes: the notes have no section.
const FLAT: &str = "## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n";

/// `Trip.md` holding `text`, split and written through the plan screen: the window, the library
/// folder, the record and the scratch folder.
fn written(tag: &str, text: &str) -> (App, PathBuf, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let mut a = app_in(&dir);
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
    (a, dir.join("library"), record, dir)
}

/// The blocked reason the open plan gives, or a panic naming what was written instead.
fn blocked(a: &App) -> String {
    a.plan
        .as_ref()
        .expect("the plan is open")
        .problem()
        .expect("the plan says why it cannot be written")
}

/// D2: the group's file is made before the notes, so a note that could not be written still
/// leaves a group one can live with (and undo takes away).
#[test]
fn the_groups_file_is_made_before_its_notes_even_when_every_note_fails() {
    let dir = scratch("plan-group-first");
    let library = dir.join("library");
    // A file where the notes' section folder would go: both notes cannot be written at step 4.
    std::fs::create_dir_all(library.join("Trip")).expect("the group's folder");
    std::fs::write(library.join("Trip").join("Tokyo"), "not a folder\n").expect("a file");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, SECTIONED).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    press(&mut a, KeyName::Enter);
    // The notes could not be written, and the file in their way is untouched.
    assert_eq!(
        read(&library.join("Trip").join("Tokyo")),
        "not a folder\n",
        "the file in the way is not touched"
    );
    // The group's file was made first, and the record says this run made it.
    let group = library.join("Trip.md");
    assert!(group.is_file(), "the group's file is made before the notes");
    assert!(read(&group).starts_with("# Trip\n"), "{}", read(&group));
    let runs = mdedit_core::runs::list(&library);
    assert_eq!(runs.len(), 1);
    assert!(runs[0].record.group.created);
    assert_eq!(runs[0].record.state, mdedit_core::runs::State::Done);
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A7: a group whose folder is the library folder, refused where it is picked.
#[test]
fn a_group_folder_that_is_the_library_folder_is_refused_where_it_is_picked() {
    let dir = scratch("plan-home-pick");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let file = library.join("Work.md");
    std::fs::write(&file, "# Work\n").expect("a library file");
    let mut a = app_in(&dir);
    // "Set the group's folder…" with the library folder itself picked: refused, nothing written.
    a.library_group_home_picked(&file, library.clone());
    assert_eq!(read(&file), "# Work\n", "the group file is untouched");
    assert!(
        toast(&a).contains("cannot be a group's folder"),
        "{}",
        toast(&a)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A7: a `home:` line that names the library folder, refused where it is read.
#[test]
fn a_home_line_that_names_the_library_folder_makes_no_note_there() {
    let dir = scratch("plan-home-read");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let file = library.join("Work.md");
    std::fs::write(&file, "# Work\nhome: .\n").expect("a home line");
    let mut a = app_in(&dir);
    a.library_rows();
    a.library_new_note(&file, &[], "Idea");
    assert!(
        !library.join("idea.md").exists(),
        "no library file was made"
    );
    assert!(
        toast(&a).contains("cannot be a group's folder"),
        "{}",
        toast(&a)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A7: and the plan refuses to send its notes there.
#[test]
fn a_plan_refuses_a_group_whose_folder_is_the_library_folder() {
    let dir = scratch("plan-home-plan");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let file = library.join("Work.md");
    std::fs::write(&file, "# Work\nhome: .\n").expect("a home line");
    let mut a = app_in(&dir);
    let source = dir.join("Work.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    // Enter, the owner's one press that writes: nothing is written, and the card stays.
    press(&mut a, KeyName::Enter);
    assert!(
        !library.join("asakusa.md").exists(),
        "no note was written in the library folder"
    );
    assert!(a.plan.is_some(), "the card stays");
    let said = blocked(&a);
    assert!(said.contains("library folder itself"), "{said}");
    assert_eq!(read(&file), "# Work\nhome: .\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, F14: a `home:` line that reaches the library folder by `..` is the library folder,
/// whether the folder it goes through is there or not: "New note here…" and the plan refuse it.
#[test]
fn a_home_line_that_reaches_the_library_folder_by_dot_dot_is_refused() {
    for (tag, trip) in [("plan-home-up-gone", false), ("plan-home-up", true)] {
        let dir = scratch(tag);
        let library = dir.join("library");
        std::fs::create_dir_all(&library).expect("the library");
        if trip {
            std::fs::create_dir_all(library.join("Trip")).expect("Trip");
        }
        let file = library.join("Work.md");
        let text = "# Work\nhome: Trip/..\n";
        std::fs::write(&file, text).expect("a home line");
        let mut a = app_in(&dir);
        a.library_rows();
        a.library_new_note(&file, &[], "Idea");
        assert!(!library.join("idea.md").exists(), "{tag}: no library file");
        assert!(
            toast(&a).contains("cannot be a group's folder"),
            "{tag}: {}",
            toast(&a)
        );
        let source = dir.join("Work.md");
        std::fs::write(&source, FLAT).expect("the source");
        open_now(&mut a, &source);
        assert!(a.plan_open_active(), "{tag}");
        press(&mut a, KeyName::Enter);
        assert!(!library.join("asakusa.md").exists(), "{tag}: no note there");
        assert!(a.plan.is_some(), "{tag}: the card stays");
        let said = blocked(&a);
        assert!(said.contains("library folder itself"), "{tag}: {said}");
        assert_eq!(read(&file), text, "{tag}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// C2: a new group whose file name is taken, found before the notes are written.
#[test]
fn a_new_group_whose_file_is_taken_is_refused_before_the_notes_are_written() {
    let dir = scratch("plan-taken");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    std::fs::write(library.join("Trip.md"), "# Japan 2025\n").expect("a library file");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    assert!(a.plan.as_ref().expect("open").dest.is_new);
    // The card's button is disabled: Enter does nothing, and nothing is written.
    press(&mut a, KeyName::Enter);
    assert!(!library.join("Trip").exists(), "no folder, no note");
    assert!(a.plan.is_some(), "the card stays");
    assert_eq!(read(&library.join("Trip.md")), "# Japan 2025\n");
    assert!(mdedit_core::runs::list(&library).is_empty(), "no record");
    let said = blocked(&a);
    assert!(
        said.contains("Trip.md") && said.contains("already"),
        "{said}"
    );
    // A press of the button that is disabled anyway (the keyboard path) says the same, and is
    // refused by the write itself as well.
    a.plan_write();
    let said = toast(&a);
    assert!(
        said.starts_with("The run could not start: ") && said.contains("already"),
        "{said}"
    );
    assert!(!library.join("Trip").exists(), "still nothing");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C2: the unsaved-change check is made when the destination changes, not only when the screen
/// opens.
#[test]
fn a_group_file_with_unsaved_changes_is_found_when_the_destination_changes() {
    let dir = scratch("plan-dest");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    std::fs::write(library.join("Trip.md"), "# Trip\n").expect("a library file");
    let mut a = app_in(&dir);
    // The group's library file is open with changes not saved.
    open_now(&mut a, &library.join("Trip.md"));
    type_text(&mut a, "mine\n");
    assert!(a.docs.active_doc().is_some_and(|d| d.modified()));
    // A plan on a new group of its own, then the destination is moved to Trip, as the menu does.
    let source = dir.join("Day.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    assert_eq!(a.plan.as_ref().expect("open").problem(), None);
    let dest = a
        .plan
        .as_ref()
        .expect("open")
        .groups
        .iter()
        .find(|g| g.name == "Trip")
        .cloned()
        .expect("the group");
    a.plan.as_mut().expect("open").set_dest(dest);
    // The next key goes through the screen, as any event does: the reason is worked out again.
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Enter);
    assert!(!library.join("Day").exists(), "no note was written");
    assert!(a.plan.is_some(), "the card stays");
    assert_eq!(read(&library.join("Trip.md")), "# Trip\n", "untouched");
    assert!(mdedit_core::runs::list(&library).is_empty(), "no record");
    let said = blocked(&a);
    assert!(said.contains("changes that are not saved"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C1: a note with no section goes into a group that already has sections, in the group's own
/// notes and before its first section, instead of being refused.
#[test]
fn a_note_with_no_section_is_written_into_the_groups_own_notes_before_its_first_section() {
    let (mut a, library, _record, dir) = written("plan-again", SECTIONED);
    let group = library.join("Trip.md");
    let before = read(&group);
    assert!(before.contains("## Tokyo"), "{before}");
    // Another document for the same group, with no heading above its notes: every note of it has
    // no section, and goes in the group's own notes, above its first section.
    let more = dir.join("more");
    std::fs::create_dir_all(&more).expect("a folder");
    let second = more.join("Trip.md");
    std::fs::write(&second, FLAT).expect("a file");
    open_now(&mut a, &second);
    assert!(a.plan_open_active(), "the plan opens");
    assert!(!a.plan.as_ref().expect("open").dest.is_new);
    assert_eq!(
        a.plan.as_ref().expect("open").problem(),
        None,
        "nothing is known against it"
    );
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_none(), "the card closes when it writes");
    // The notes are written, in the group's own folder.
    assert!(library.join("Trip/asakusa.md").is_file());
    assert!(library.join("Trip/nara.md").is_file());
    // Their links are above the group's first section, not under its last one.
    let after = read(&group);
    let first_link = after
        .find("(Trip/asakusa.md)")
        .unwrap_or_else(|| panic!("no link for the note of the rest: {after}"));
    let first_section = after
        .find("## Tokyo")
        .unwrap_or_else(|| panic!("the group lost its section: {after}"));
    assert!(first_link < first_section, "{after}");
    // They read back with no section, and the notes the file already had did not move.
    let parsed = mdshared_app::library::parse(&after, &library);
    let own: Vec<&str> = parsed
        .entries
        .iter()
        .filter(|e| e.sections.is_empty())
        .map(|e| e.title.as_str())
        .collect();
    assert_eq!(own, vec!["Asakusa", "Nara"], "{after}");
    assert_eq!(
        parsed
            .entries
            .iter()
            .filter(|e| e.sections == ["Tokyo".to_string()])
            .count(),
        2,
        "the first run's notes are still under Tokyo: {after}"
    );
    // The run is done, and its record says so.
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the second record");
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record reads back");
    assert_eq!(record.state, mdedit_core::runs::State::Done);
    assert_eq!(record.notes.len(), 2);
    assert_eq!(mdedit_core::runs::list(&library).len(), 2, "two records");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C2: the library's two caps, found before the first note instead of after the last.
#[test]
fn the_librarys_two_caps_are_found_before_anything_is_written() {
    // The files the library holds.
    let dir = scratch("plan-cap-files");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    for i in 0..mdshared_app::library::MAX_FILES {
        std::fs::write(library.join(format!("G{i}.md")), format!("# G{i}\n")).expect("a file");
    }
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    press(&mut a, KeyName::Enter);
    assert!(!library.join("Trip.md").exists(), "no group file was made");
    assert!(a.plan.is_some(), "the card stays");
    let said = blocked(&a);
    assert!(
        said.contains(&format!("{} files", mdshared_app::library::MAX_FILES)),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    // The notes one group holds.
    let dir = scratch("plan-cap-notes");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let mut text = String::from("# Trip\nhome: Trip\n");
    for i in 0..mdshared_app::library::MAX_ENTRIES {
        text.push_str(&format!("- [N{i}](Trip/N{i}.md)\n"));
    }
    std::fs::write(library.join("Trip.md"), &text).expect("a library file");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    press(&mut a, KeyName::Enter);
    assert!(!library.join("Trip").exists(), "no note was written");
    assert!(a.plan.is_some(), "the card stays");
    assert_eq!(read(&library.join("Trip.md")), text);
    let said = blocked(&a);
    assert!(
        said.contains(&format!(
            "more than {} notes",
            mdshared_app::library::MAX_ENTRIES
        )),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// C3: a heading that ends in `#` does not read back; the dry run says so before the notes exist.
#[test]
fn a_section_called_c_sharp_is_refused_before_the_notes_are_written() {
    let dir = scratch("plan-csharp");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# C#\n\nthe language\n\n## Generics\n\nthe lists\n\n## Records\n\nthe shapes\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    press(&mut a, KeyName::Enter);
    let library = dir.join("library");
    assert!(!library.join("Trip").exists(), "no note was written");
    assert!(a.plan.is_some(), "the card stays");
    assert!(!library.join("Trip.md").exists(), "no group file was made");
    assert!(mdedit_core::runs::list(&library).is_empty(), "no record");
    let said = blocked(&a);
    assert!(said.contains("ends in '#'"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C5: two titles that differ only in case are one file on the disk, so the second gets another
/// name before anything is written.
#[test]
fn two_titles_that_differ_only_in_case_are_given_two_files() {
    let dir = scratch("plan-case");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, "## Notes\n\nfirst\n\n## notes\n\nsecond\n").expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    {
        let p = a.plan.as_ref().expect("open");
        assert_eq!(p.targets.len(), 2);
        assert!(
            !mdshared_app::paths::same_path(&p.targets[0].path, &p.targets[1].path),
            "the two notes are two files: {:?} {:?}",
            p.targets[0].path,
            p.targets[1].path
        );
        // Both titles are named `notes.md` (D-075), on every disk, so the second is numbered.
        assert_eq!(p.targets[1].renamed.as_deref(), Some("notes 2.md"));
        assert_eq!(
            p.mark(1),
            Some(crate::plan_ui::Mark::Renamed("notes 2.md".to_string()))
        );
    }
    press(&mut a, KeyName::Enter);
    let folder = dir.join("library").join("Trip");
    let files: Vec<String> = std::fs::read_dir(&folder)
        .expect("the folder")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files.len(), 2, "{files:?}");
    assert!(files.iter().any(|f| f == "notes.md"), "{files:?}");
    assert!(files.iter().any(|f| f == "notes 2.md"), "{files:?}");
    assert_eq!(read(&folder.join("notes.md")), "# Notes\n\nfirst\n");
    assert_eq!(read(&folder.join("notes 2.md")), "# notes\n\nsecond\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// C7: two groups of one name are two files; the notes and their links go to the one chosen.
#[test]
fn two_groups_of_one_name_send_the_notes_and_the_links_to_the_chosen_file() {
    let dir = scratch("plan-two-groups");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    std::fs::write(library.join("A.md"), "# Trip\nhome: A\n").expect("a library file");
    std::fs::write(library.join("B.md"), "# Trip\nhome: B\n").expect("a library file");
    let mut a = app_in(&dir);
    let source = dir.join("Doc.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    let dest = a
        .plan
        .as_ref()
        .expect("open")
        .groups
        .iter()
        .find(|g| g.file.ends_with("B.md"))
        .cloned()
        .expect("the second group");
    a.plan.as_mut().expect("open").set_dest(dest);
    press(&mut a, KeyName::Down);
    assert_eq!(a.plan.as_ref().expect("open").problem(), None);
    press(&mut a, KeyName::Enter);
    assert_eq!(
        read(&library.join("A.md")),
        "# Trip\nhome: A\n",
        "untouched"
    );
    let b = read(&library.join("B.md"));
    assert!(b.contains("[Asakusa]") && b.contains("[Nara]"), "{b}");
    assert!(library.join("B/asakusa.md").is_file(), "the note is in B");
    assert!(!library.join("A/asakusa.md").exists(), "nothing went to A");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A8: a plan kept by Escape, shown again after the document was saved with a change, refuses to
/// write the text it was made from.
#[test]
fn a_kept_plan_refuses_to_write_after_the_document_changed() {
    let dir = scratch("plan-parked");
    let library = dir.join("library");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    std::fs::write(&source, "## One\n\nalpha\n\n## Two\n\nbeta\n").expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    a.compose();
    // A change, so that Escape keeps the plan: the second note is unticked.
    press(&mut a, KeyName::Down);
    press(&mut a, KeyName::Space);
    press(&mut a, KeyName::Escape);
    assert!(a.plan.is_none() && a.plan_parked.is_some());
    // The owner goes on writing in the document, and saves it.
    type_text(&mut a, "x");
    a.command(C::Save);
    let now = std::fs::read(&source).expect("the source");
    assert!(
        now.starts_with(b"x## One"),
        "{:?}",
        String::from_utf8_lossy(&now)
    );
    // Show: Enter writes nothing, and the card says the document changed.
    assert!(a.plan_show());
    press(&mut a, KeyName::Enter);
    assert!(!library.join("Trip").exists(), "no note was written");
    assert!(a.plan.is_some(), "the card stays; toast: {:?}", toast(&a));
    assert!(mdedit_core::runs::list(&library).is_empty(), "no record");
    let library = dir.join("library");
    assert!(!library.join("Trip").exists(), "no note was written");
    assert!(mdedit_core::runs::list(&library).is_empty(), "no record");
    let said = blocked(&a);
    assert!(
        said.contains("has changed since this plan was made"),
        "{said}"
    );
    // The write itself refuses too, with the same words on the bar.
    a.plan_write();
    let said = toast(&a);
    assert!(
        said.starts_with("The run could not start: ") && said.contains("has changed since"),
        "{said}"
    );
    assert!(!library.join("Trip").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A8: a document whose bytes are not text is refused, as plan 3.11 requires.
#[test]
fn a_document_whose_bytes_are_not_text_is_refused_by_split_into_notes() {
    let dir = scratch("plan-lossy");
    let mut a = app_in(&dir);
    let source = dir.join("Trip.md");
    // Not text: UTF-16's byte-order mark and a NUL, which the reader fills in with U+FFFD.
    std::fs::write(&source, [0xFF, 0xFE, b'a', 0, b'b']).expect("the source");
    open_now(&mut a, &source);
    assert!(
        a.docs
            .active_doc()
            .and_then(|d| d.ready())
            .is_some_and(|r| r.info.lossy),
        "the document is lossy"
    );
    assert!(!a.plan_open_active());
    assert!(a.plan.is_none());
    assert_eq!(toast(&a), crate::commands::strings::split_lossy("Trip.md"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A11: a build with no session has no library at all -- nothing of the owner's is read or
/// written -- and Split into notes refuses for want of a folder.
#[test]
fn a_build_with_no_session_has_no_library() {
    let dir = scratch("plan-no-session");
    let mut a = app();
    a.args.no_session = true;
    assert_eq!(a.library_dir(), None, "a build with no session has none");
    let source = dir.join("Trip.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(!a.plan_open_active(), "no library folder, no plan");
    assert!(a.plan.is_none());
    assert_eq!(toast(&a), crate::commands::strings::LIBRARY_NO_FOLDER);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A6: the plan uses the library folder the window knows, read as the plan opens, even when
/// nothing has read the library yet.
#[test]
fn split_into_notes_reads_the_library_first() {
    let dir = scratch("plan-read-first");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    let source = dir.join("Trip.md");
    std::fs::write(&source, FLAT).expect("the source");
    open_now(&mut a, &source);
    assert!(
        a.docs.library_stamp.dir.as_os_str().is_empty(),
        "the library was never read"
    );
    assert!(a.plan_open_active(), "the plan opens");
    assert_eq!(a.plan.as_ref().expect("open").root(), dir.join("library"));
    assert_eq!(a.docs.library.dir, dir.join("library"), "read as it opened");
    a.compose();
    press(&mut a, KeyName::Enter);
    assert!(
        dir.join("library")
            .join("Trip")
            .join("asakusa.md")
            .is_file()
    );
    assert!(!dir.join("Trip").exists(), "nothing in the starting folder");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A6: "AI runs…" lists the records of the one library folder, read now, before anything else
/// loaded the library.
#[test]
fn ai_runs_lists_the_library_folders_records_before_the_library_was_read() {
    let (_a, library, _record, dir) = written("plan-open-runs", SECTIONED);
    assert_eq!(mdedit_core::runs::list(&library).len(), 1);
    // A fresh window that never showed the Notes pane: its library folder is empty until now.
    let mut b = app();
    b.session_path = Some(dir.join("settings.ini"));
    assert!(b.docs.library_stamp.dir.as_os_str().is_empty());
    b.open_runs();
    assert_eq!(b.ui.runs.list.len(), 1, "the record is listed");
    assert_eq!(b.ui.dialog.as_ref().map(|d| d.kind), Some(DialogKind::Runs));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A record of a kind this build does not write - a word from a later version - is listed with
/// the newer-version records, with nothing to press, and nothing in it is shown or undone.
#[test]
fn a_record_of_a_kind_this_build_does_not_write_is_listed_as_a_newer_versions() {
    let (mut a, library, record_path, dir) = written("plan-future-record", SECTIONED);
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record");
    // A kind word no build knows, written into a record's own JSON. `Kind::Rewrite` no longer
    // stands for this: phase W writes it, so it is read, shown and undone here (D-145).
    let text = record
        .to_json(&library)
        .replace("\"kind\": \"split\"", "\"kind\": \"from-a-later-version\"");
    assert!(text.contains("from-a-later-version"), "{text}");
    let path = library
        .join(".runs")
        .join("2099-01-01T00-00-00Z-future.json");
    std::fs::write(&path, &text).expect("the future record");
    // The split's own record is still there and is listed; the unknown kind is not.
    assert_eq!(mdedit_core::runs::list(&library).len(), 1);
    a.open_runs();
    assert_eq!(a.ui.runs.list.len(), 1, "only the split can be undone here");
    assert!(
        a.ui.runs.unread.iter().any(|u| u.newer && u.path == path),
        "the unknown record is listed as a later version's"
    );
    // And nothing in it is acted on: this build cannot read it, so there is no question and the
    // file is left as it is.
    assert!(
        mdedit_core::runs::load(&path, &library).is_none(),
        "a kind this build does not write is not a record it can use"
    );
    a.run_undo_ask(path.clone());
    assert!(a.ui.runs.undo.is_none(), "there is no undo question");
    assert!(path.is_file(), "the record is left as it is");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, A F8: a rewrite's record may name a file anywhere, not only in the library (D-162),
/// so the question before its undo names each file it would put back, where it is.
#[test]
fn the_undo_question_of_a_rewrite_names_each_file_it_puts_back() {
    let dir = scratch("plan-undo-rewrite-names");
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    let mut a = app_in(&dir);
    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("a folder outside the library");
    let one = elsewhere.join("letter.md");
    let two = library.join("draft.md");
    let (path, mut record) =
        mdedit_core::runs::start_rewrite(&library, "Shorter.", None, &one).expect("the record");
    for file in [&one, &two] {
        std::fs::write(file, "the old words\n").expect("a file");
        let rewrite = mdedit_core::runs::RewriteFile {
            path: file.to_path_buf(),
            before: "the old words\n".to_string(),
            after: "new words\n".to_string(),
            bom: false,
        };
        mdedit_core::runs::rewrite_into(&path, &mut record, &library, &rewrite).expect("rewritten");
    }
    a.run_undo_ask(path);
    let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(can, "{question}");
    for file in [&one, &two] {
        assert!(
            question.contains(&*file.display().to_string()),
            "{} is named: {question}",
            file.display()
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, B11, the core's half: a write that starts while the plan's own split is out stops
/// that split, as closing the card does. In "Split them" the next file's card opens at once, and
/// the answer for this one must not reach it.
#[test]
fn writing_the_plan_stops_its_split_that_is_still_out() {
    super::ai_tests::engine();
    let dir = scratch("plan-write-split-out");
    let mut a = app_in(&dir);
    a.docs.prefs.ai_connection = mdedit_core::ai::Connection::ClaudeApi;
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    a.compose();
    a.plan_ai(crate::plan_ui::AiAct::Ask);
    let i = a.ai.instruction().expect("the split panel");
    i.field.set_text("SPLITSLOW take your time");
    press(&mut a, KeyName::Enter);
    assert!(a.ai.split.is_some(), "the request is out");
    assert!(a.plan.is_some(), "the plan is back while it waits");
    assert!(a.docs.ai_status.is_some(), "the status line says it is out");
    a.plan_write();
    assert!(a.plan.is_none(), "the plan was written");
    assert!(a.ai.split.is_none(), "and its split stopped with it");
    // The status line no longer says the split is out (found at the merge with the AI agent's
    // half, whose own test asks the same in "Split them").
    assert!(a.docs.ai_status.is_none(), "{:?}", a.docs.ai_status);
    assert!(
        dir.join("library")
            .join("Trip")
            .join("Tokyo")
            .join("asakusa.md")
            .is_file()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
