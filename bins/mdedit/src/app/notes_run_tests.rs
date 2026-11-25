//! Write notes in the task window (pictures plan, phase V3), with the stand-in engine of
//! `ai_tests`: the pictures measured when added, the looks four at a time, the arrange, a
//! failed look, a model that reads no pictures, Stop, and the AI off.

use super::ai_tests::{engine, wait};
use super::files_tests::{app, scratch};
use super::*;
use crate::keys::KeyName;
use crate::notes_task::Measure;
use crate::task_ui::{Hit, TaskKind};
use mdedit_core::ai::Connection;
use mdedit_core::notes_ai::HowMany;

/// `n` pictures (the engine is a stand-in: their bytes are never read), named `stem0.png`….
fn pictures(dir: &Path, stem: &str, n: usize) -> Vec<PathBuf> {
    (0..n)
        .map(|i| {
            let p = dir.join(format!("{stem}{i}.png"));
            std::fs::write(&p, b"not read").expect("picture");
            p
        })
        .collect()
}

fn window(tag: &str, on: bool, n: usize, stem: &str) -> (App, PathBuf) {
    engine();
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    if on {
        a.docs.prefs.ai_connection = Connection::ClaudeApi;
    }
    let files = pictures(&dir, stem, n);
    a.task_open(TaskKind::Notes, files);
    (a, dir)
}

fn measured(a: &App) -> bool {
    a.task.as_ref().is_some_and(|s| {
        s.files
            .iter()
            .all(|f| matches!(f.measure, Measure::Ready(_)))
            && s.prepare.is_empty()
    })
}

fn arranged(a: &App) -> Option<mdedit_core::notes_ai::Arranged> {
    a.task.as_ref()?.notes_run.as_ref()?.arranged.clone()
}

/// Pictures plan 5 and 6: each picture is measured by the engine when it is added, so its row
/// and the footer say what goes before anything is sent.
#[test]
fn pictures_are_measured_when_added_and_the_footer_says_what_goes() {
    let (mut a, _dir) = window("notes-measure", true, 2, "shot");
    assert!(!a.task.as_ref().expect("open").prepare.is_empty());
    wait(&mut a, measured);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.summary(), "2 pictures, 2 KB as sent \u{00B7} 2 requests");
    assert_eq!(s.primary_label(), "Send 2 pictures");
    assert!(s.notes_blocked().is_none());
    assert_eq!(a.task_sent.requests, 0, "measuring sends nothing");
}

/// Pictures plan 3: the looks go, each row takes what the model said of its picture, then one
/// arrange request with no picture in it, and the plan is read.
#[test]
fn a_run_looks_at_the_pictures_then_arranges_the_notes() {
    let (mut a, _dir) = window("notes-run", true, 5, "room");
    wait(&mut a, measured);
    a.task_run();
    assert!(a.task.as_ref().expect("open").busy());
    wait(&mut a, |a| arranged(a).is_some());
    let s = a.task.as_ref().expect("open");
    let run = s.notes_run.as_ref().expect("run");
    assert_eq!(run.batches.len(), 2, "four to a request");
    assert_eq!(run.looks.len(), 5);
    assert_eq!(run.look(1).map(|l| l.title.as_str()), Some("Tiles"));
    let plan = arranged(&a).expect("plan");
    assert_eq!(plan.notes[0].title, "Kitchen");
    assert_eq!(plan.notes[0].pictures(), vec![0]);
    assert_eq!(a.task_sent.requests, 3);
    assert!(a.task_sent.bytes > 0);
}

/// Pictures plan 15: a look that fails keeps what was looked at; "Go on with N" goes on without
/// it, and the arrange is asked of what was.
#[test]
fn a_failed_look_keeps_the_answers_and_goes_on_without_it() {
    let (mut a, dir) = window("notes-fail", true, 4, "ok");
    let late = dir.join("LOOKFAIL.png");
    std::fs::write(&late, b"x").expect("picture");
    a.task_add_paths(vec![late]);
    wait(&mut a, measured);
    a.task_run();
    wait(&mut a, |a| {
        a.task
            .as_ref()
            .and_then(|s| s.notes_run.as_ref())
            .is_some_and(|r| r.looks_ended())
    });
    let run = a
        .task
        .as_ref()
        .and_then(|s| s.notes_run.clone())
        .expect("run");
    assert_eq!(run.failed.len(), 1);
    assert!(
        run.failed[0].1.contains("Try again in 30 s."),
        "{:?}",
        run.failed
    );
    assert_eq!(run.looked(), 4);
    assert!(
        arranged(&a).is_none(),
        "nothing is arranged while one failed"
    );
    a.task_act(Hit::GoOn);
    wait(&mut a, |a| arranged(a).is_some());
    assert_eq!(
        a.task
            .as_ref()
            .and_then(|s| s.notes_run.as_ref())
            .map(|r| r.looks.len()),
        Some(4)
    );
}

/// Pictures plan 15: a model that reads no pictures stops the run with the settings button,
/// and nothing is arranged.
#[test]
fn a_model_that_reads_no_pictures_stops_the_run() {
    let (mut a, _dir) = window("notes-novision", true, 2, "p");
    wait(&mut a, measured);
    a.task
        .as_mut()
        .expect("open")
        .instruction
        .set_text("NOVISION");
    a.task_run();
    wait(&mut a, |a| {
        a.task
            .as_ref()
            .and_then(|s| s.notes_run.as_ref())
            .is_some_and(|r| r.fault.is_some())
    });
    let s = a.task.as_ref().expect("open");
    assert!(!s.busy());
    assert!(arranged(&a).is_none());
    assert!(
        s.notes_run
            .as_ref()
            .and_then(|r| r.fault.as_deref())
            .is_some_and(|f| f.starts_with("This model does not read pictures"))
    );
}

/// Esc while the pictures are looked at stops every request out; the window stays, and its
/// button comes back.
#[test]
fn escape_stops_the_looks_and_keeps_the_window() {
    let (mut a, _dir) = window("notes-stop", true, 2, "p");
    wait(&mut a, measured);
    a.task
        .as_mut()
        .expect("open")
        .instruction
        .set_text("LOOKSLOW");
    a.task_run();
    assert!(a.task.as_ref().expect("open").busy());
    a.task_key(Some(KeyName::Escape), None);
    let s = a.task.as_ref().expect("the window stays");
    assert!(!s.busy());
    assert!(a.ai.waiting() == 0, "nothing is waited for");
}

/// Pictures plan 3: with the AI off nothing is measured or sent, and the notes are made here,
/// one per picture.
#[test]
fn with_the_ai_off_the_notes_are_made_here_and_nothing_is_sent() {
    let (mut a, _dir) = window("notes-off", false, 3, "Kitchen sink ");
    let s = a.task.as_ref().expect("open");
    assert!(s.prepare.is_empty(), "nothing is measured");
    assert_eq!(s.primary_label(), "Make the notes");
    if let Some(s) = a.task.as_mut() {
        s.notes.how_many = HowMany::PerPicture;
    }
    a.task_run();
    let plan = arranged(&a).expect("made here");
    assert_eq!(plan.notes.len(), 3);
    assert_eq!(plan.notes[0].title, "Kitchen sink 0");
    assert_eq!(a.task_sent.requests, 0);
    assert!(a.ai.waiting() == 0);
}

/// D-221: Codex is offered no pictures; the reason is where the button is.
#[test]
fn codex_is_offered_no_pictures() {
    let (mut a, _dir) = window("notes-codex", false, 1, "p");
    a.docs.prefs.ai_connection = Connection::Codex;
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
    }
    a.notes_prepare(false);
    let s = a.task.as_ref().expect("open");
    assert!(s.prepare.is_empty(), "nothing is measured for it");
    assert!(
        s.notes_blocked()
            .is_some_and(|w| w.starts_with("Codex cannot"))
    );
}

/// Pictures plan 12: an option is kept in the settings, and one that changes what is sent has
/// the pictures measured again.
#[test]
fn an_option_is_kept_and_a_change_of_what_is_sent_measures_again() {
    let (mut a, _dir) = window("notes-options", true, 1, "p");
    wait(&mut a, measured);
    a.task_act(Hit::Options);
    assert!(a.task.as_ref().expect("open").options_open);
    a.task_act(Hit::Tick(crate::notes_task::Tick::Larger));
    assert!(a.docs.prefs.ai_notes.larger);
    let s = a.task.as_ref().expect("open");
    assert!(s.options_open, "a tick keeps the popover open");
    assert!(!s.prepare.is_empty(), "measured again at 2576 px");
    wait(&mut a, measured);
    a.task_act(Hit::HowMany(HowMany::One));
    assert_eq!(a.docs.prefs.ai_notes.how_many, HowMany::One);
    assert!(
        a.task.as_ref().expect("open").prepare.is_empty(),
        "nothing to measure again"
    );
    a.task_act(Hit::Body);
    assert!(!a.task.as_ref().expect("open").options_open);
}
