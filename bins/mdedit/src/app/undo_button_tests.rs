//! The two buttons of the "Undo this run?" question, by where each one is drawn.
//!
//! Audit 3 found that Cancel undid the run and Undo closed the question: the handler acts on
//! Hit::DlgButton(0) and the drawing put Cancel there. The rest of that audit's probes are
//! kept as a patch in `docs/review-prompts/audit3-probes.patch`, each to become the test for
//! its own fix.
use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::keys::{Key, KeyName};
use crate::ui::Hit;
use mdedit_core::runs::{self, State};

/// A heading above the notes: the run writes a section heading into the library file.
const SECTIONED: &str = "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n";
fn press(a: &mut App, key: KeyName) {
    a.key_pressed(Key::Name(key), None);
}

fn app_in(dir: &Path) -> App {
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a
}

fn hit_rect(a: &App, hit: Hit) -> Option<Rect> {
    a.overlay_hits
        .iter()
        .chain(a.hits.iter().flatten())
        .find(|(h, _)| *h == hit)
        .map(|(_, r)| *r)
}

fn click(a: &mut App, r: Rect) {
    a.pointer_moved(r.cx(), r.cy());
    a.left_press();
    a.left_release();
}

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

// ---- N23: the question before an undo answers the wrong way round ---------------------------------

#[test]
fn the_right_hand_button_of_undo_this_run_is_cancel_and_undoes_nothing() {
    let (mut a, library, record, dir) = written("probe-cancel", SECTIONED);
    let note = library.join("Trip").join("Tokyo").join("Asakusa.md");
    assert!(note.is_file());
    a.run_undo_ask(record.clone());
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(DialogKind::RunUndo)
    );
    a.compose();
    let zero = hit_rect(&a, Hit::DlgButton(0)).expect("button 0 is drawn");
    let one = hit_rect(&a, Hit::DlgButton(1)).expect("button 1 is drawn");
    // The answer is drawn first, as in every other question, so it sits at the left and Cancel
    // is the right-hand button.
    assert!(zero.x < one.x, "{zero:?} {one:?}");
    click(&mut a, one);
    assert!(note.is_file(), "Cancel deleted the note");
    assert!(library.join("Trip.md").is_file(), "and the group's file");
    assert_eq!(
        runs::load(&record, &library).expect("the record").state,
        State::Done
    );
    assert!(a.ui.dialog.is_none(), "the question closed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_left_hand_button_of_undo_this_run_is_undo_and_does_it() {
    let (mut a, library, record, dir) = written("probe-undo", SECTIONED);
    let note = library.join("Trip").join("Tokyo").join("Asakusa.md");
    a.run_undo_ask(record.clone());
    a.compose();
    let zero = hit_rect(&a, Hit::DlgButton(0)).expect("button 0 is drawn");
    click(&mut a, zero);
    assert!(!note.exists(), "Undo left the note");
    assert_eq!(
        runs::load(&record, &library).expect("the record").state,
        State::Undone
    );
    // The labels themselves, so a swap of the array alone is caught: the answer is first.
    assert_eq!(crate::overlays::run_undo_buttons(true)[0].0, "Undo");
    assert_eq!(crate::overlays::run_undo_buttons(true)[1].0, "Cancel");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- N1: undo through the window's own gate -------------------------------------------------------
