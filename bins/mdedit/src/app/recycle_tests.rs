//! The original in the Recycle Bin (`docs/ai-library-plan.md` 5.4, phase T) through the window:
//! the box on the card asks the window's bin, step 8 hands the source to it, the report says
//! where the source went, and undo waits for it and hands its removals to the bin. Every test
//! gives the window a [`FolderBin`] of its own: the system's bin is never reached from here.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::keys::{Key, KeyName};
use crate::recycle::FolderBin;
use crate::replace_ui::OutRow;

const TEXT: &str = "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n";

fn press(a: &mut App, key: KeyName) {
    a.key_pressed(Key::Name(key), None);
}

fn toast(a: &App) -> String {
    a.docs
        .toast(Instant::now())
        .map(str::to_string)
        .unwrap_or_default()
}

type Calls = std::rc::Rc<std::cell::RefCell<Vec<PathBuf>>>;

/// A window with a test bin in `dir/bin`, a library in `dir/library`, and the plan open on
/// `Trip.md`, which `setup` may change first. The bin's calls are returned, to be counted.
fn planned(
    tag: &str,
    setup: impl FnOnce(&mut FolderBin),
) -> (App, PathBuf, PathBuf, PathBuf, Calls) {
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    let mut bin = FolderBin::new(&dir.join("bin"));
    setup(&mut bin);
    let calls = bin.calls.clone();
    a.recycle = Box::new(bin);
    let source = dir.join("Trip.md");
    std::fs::write(&source, TEXT).expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active(), "the plan opens");
    a.compose();
    (a, dir.join("library"), source, dir, calls)
}

/// Ticks the box, answering "lines in no note" first with the card's own one-click answer.
fn tick(a: &mut App) {
    let p = a.plan.as_mut().expect("open");
    p.keep_the_rest();
    p.toggle_recycle();
    assert!(p.recycle_on(), "the box is ticked: {:?}", p.recycle_why());
}

fn rows(a: &App) -> Vec<String> {
    let outcome =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .expect("the report is there");
    let mut out = vec![outcome.title.clone()];
    out.extend(outcome.rows.iter().map(|r| match r {
        OutRow::Heading(t) => format!("# {t}"),
        OutRow::Changed { name, .. } => format!("+ {name}"),
        OutRow::Skipped { name, why } => format!("~ {name} {why}"),
        OutRow::Failed { name, why } => format!("! {name} {why}"),
    }));
    out
}

#[test]
fn the_card_asks_the_windows_bin_about_the_sources_drive() {
    let (a, _, _, dir, _) = planned("bin-card-none", |b| {
        b.refusal = Some(crate::plan_ui::RECYCLE_NO_BIN.to_string());
    });
    let p = a.plan.as_ref().expect("open");
    assert!(p.recycle.is_some(), "the box is on the card");
    assert_eq!(
        p.recycle_why(),
        Some(crate::plan_ui::RecycleWhy::Refused(
            crate::plan_ui::RECYCLE_NO_BIN.to_string()
        ))
    );
    let _ = std::fs::remove_dir_all(&dir);
    // A drive with a bin: the box waits only for the lines in no note.
    let (mut a, _, _, dir, _) = planned("bin-card-some", |_| {});
    let p = a.plan.as_mut().expect("open");
    assert!(
        matches!(p.recycle_why(), Some(crate::plan_ui::RecycleWhy::Lines(_))),
        "{:?}",
        p.recycle_why()
    );
    p.keep_the_rest();
    assert_eq!(p.recycle_why(), None);
    assert!(!p.recycle_on(), "offered, never ticked for the owner");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_box_left_unticked_leaves_the_source_where_it_was() {
    let (mut a, _, source, dir, calls) = planned("bin-unticked", |_| {});
    a.plan.as_mut().expect("open").keep_the_rest();
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_none(), "written");
    assert!(calls.borrow().is_empty(), "the bin was not asked");
    assert_eq!(std::fs::read_to_string(&source).ok().as_deref(), Some(TEXT));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_ticked_box_puts_the_source_in_the_bin_last_and_the_report_says_so() {
    let (mut a, library, source, dir, calls) = planned("bin-ticked", |_| {});
    tick(&mut a);
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_none(), "written");
    assert_eq!(*calls.borrow(), vec![source.clone()], "the source, once");
    assert!(!source.exists(), "the source is gone from its folder");
    assert_eq!(
        std::fs::read_to_string(FolderBin::held(&dir.join("bin"), &source))
            .ok()
            .as_deref(),
        Some(TEXT),
        "and in the bin, as it was"
    );
    // The notes were written before it went.
    assert!(library.join("Trip/Tokyo/asakusa.md").is_file());
    let runs = mdedit_core::runs::list(&library);
    assert_eq!(runs.len(), 1);
    assert!(runs[0].record.source.recycled, "the record says so");
    assert_eq!(runs[0].record.state, mdedit_core::runs::State::Done);
    let rows = rows(&a);
    assert!(
        rows[0].ends_with(" \u{b7} the original in the Recycle Bin"),
        "{rows:?}"
    );
    assert!(
        rows.contains(&"# In the Recycle Bin (1)".to_string()),
        "{rows:?}"
    );
    assert!(
        rows.contains(&format!("+ {}", source.display())),
        "{rows:?}"
    );
    assert_eq!(toast(&a), "", "nothing went wrong");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_bin_that_refuses_leaves_the_source_and_says_why_in_the_toast_and_the_report() {
    let (mut a, library, source, dir, calls) = planned("bin-refuses", |b| {
        b.refuse = Some("the item is too large for the Recycle Bin".to_string());
    });
    tick(&mut a);
    press(&mut a, KeyName::Enter);
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(std::fs::read_to_string(&source).ok().as_deref(), Some(TEXT));
    assert_eq!(
        toast(&a),
        "The notes are written. Trip.md could not be put in the Recycle Bin: the item is too \
         large for the Recycle Bin. It is where it was."
    );
    let rows = rows(&a);
    assert!(
        rows.contains(&"# Not in the Recycle Bin (1)".to_string()),
        "{rows:?}"
    );
    let runs = mdedit_core::runs::list(&library);
    assert!(!runs[0].record.source.recycled);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_drive_that_lost_its_bin_since_the_card_opened_is_asked_again_before_step_eight() {
    let (mut a, _, source, dir, calls) = planned("bin-asked-again", |_| {});
    tick(&mut a);
    // The card's answer is a while old: the drive is asked again at the write.
    let mut bin = FolderBin::new(&dir.join("bin"));
    bin.calls = calls.clone();
    bin.refusal = Some(crate::plan_ui::RECYCLE_NO_BIN.to_string());
    a.recycle = Box::new(bin);
    press(&mut a, KeyName::Enter);
    assert!(calls.borrow().is_empty(), "nothing was handed to the bin");
    assert!(source.is_file());
    let said = toast(&a);
    assert!(
        said.contains("could not be put in the Recycle Bin: this drive has no Recycle Bin."),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_source_changed_on_disk_after_it_was_read_is_not_put_in_the_bin() {
    let (mut a, _, source, dir, calls) = planned("bin-changed", |_| {});
    tick(&mut a);
    // Someone saves the file after the card read it; the notes are cut from the text that was
    // read, so the file on disk is no longer theirs to throw away.
    std::fs::write(&source, format!("{TEXT}\nmore\n")).expect("the change");
    press(&mut a, KeyName::Enter);
    assert!(calls.borrow().is_empty(), "nothing was handed to the bin");
    assert!(source.is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_source_listed_in_the_library_is_named_in_the_report_as_a_line_that_will_lead_nowhere() {
    let dir = scratch("bin-listed");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    let library = dir.join("library");
    std::fs::create_dir_all(&library).expect("the library");
    // In a folder of its own: the group the split makes is `Journal.md`, beside the folder.
    std::fs::create_dir_all(library.join("Inbox")).expect("a folder");
    let source = library.join("Inbox").join("Journal.md");
    std::fs::write(&source, TEXT).expect("the source");
    std::fs::write(
        library.join("Inbox.md"),
        "# Inbox\n\n- [Journal](Inbox/Journal.md)\n",
    )
    .expect("a group");
    a.library_rows();
    let bin = FolderBin::new(&dir.join("bin"));
    a.recycle = Box::new(bin);
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    a.compose();
    tick(&mut a);
    press(&mut a, KeyName::Enter);
    assert!(!source.exists(), "it went: {}", toast(&a));
    let rows = rows(&a);
    assert!(
        rows.iter()
            .any(|r| r.starts_with("~ \u{2018}Journal.md\u{2019}") && r.contains("Inbox")),
        "{rows:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_waits_for_the_source_offers_the_bin_then_hands_the_notes_to_it() {
    let (mut a, library, source, dir, calls) = planned("bin-undo", |_| {});
    tick(&mut a);
    press(&mut a, KeyName::Enter);
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    // While the source is away, undo is refused, and the question offers the bin.
    a.run_undo_ask(record_path.clone());
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(DialogKind::RunUndo)
    );
    let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(!can, "{question}");
    assert!(a.ui.runs.bin, "Open the Recycle Bin is offered");
    a.run_undo(record_path.clone());
    assert!(
        library.join("Trip/Tokyo/asakusa.md").is_file(),
        "nothing was undone"
    );
    // Restored from the bin: the question has no button for the bin, and says where the notes
    // go.
    std::fs::rename(FolderBin::held(&dir.join("bin"), &source), &source).expect("restored");
    a.run_undo_ask(record_path.clone());
    let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(can, "{question}");
    assert!(
        question.ends_with(" The notes go to the Recycle Bin."),
        "{question}"
    );
    assert!(!a.ui.runs.bin);
    a.run_undo(record_path.clone());
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    let handed = calls.borrow().clone();
    assert!(
        handed.contains(&library.join("Trip/Tokyo/asakusa.md"))
            && handed.contains(&library.join("Trip/Tokyo/nara.md")),
        "{handed:?}"
    );
    assert!(
        dir.join("bin/asakusa.md").is_file(),
        "the note is in the bin, not deleted"
    );
    assert_eq!(std::fs::read_to_string(&source).ok().as_deref(), Some(TEXT));
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record");
    assert_eq!(record.state, mdedit_core::runs::State::Undone);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undo_on_a_drive_without_a_bin_removes_the_notes_as_before() {
    let (mut a, library, _, dir, calls) = planned("bin-undo-none", |_| {});
    a.plan.as_mut().expect("open").keep_the_rest();
    press(&mut a, KeyName::Enter);
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    let mut bin = FolderBin::new(&dir.join("bin"));
    bin.calls = calls.clone();
    bin.refusal = Some(crate::plan_ui::RECYCLE_NO_BIN.to_string());
    a.recycle = Box::new(bin);
    a.run_undo_ask(record_path.clone());
    let (_, question, _) = a.ui.runs.undo.clone().expect("the question");
    assert!(!question.contains("Recycle Bin"), "{question}");
    a.run_undo(record_path);
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists());
    assert!(calls.borrow().is_empty(), "the bin was not asked");
    assert!(!dir.join("bin/asakusa.md").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_note_the_bin_refuses_at_undo_is_kept_and_listed_apart() {
    let (mut a, library, _, dir, calls) = planned("bin-undo-refused", |_| {});
    a.plan.as_mut().expect("open").keep_the_rest();
    press(&mut a, KeyName::Enter);
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    let mut bin = FolderBin::new(&dir.join("bin"));
    bin.calls = calls.clone();
    bin.refuse = Some("the item is too large for the Recycle Bin".to_string());
    a.recycle = Box::new(bin);
    a.run_undo_ask(record_path.clone());
    a.run_undo(record_path.clone());
    assert!(
        library.join("Trip/Tokyo/asakusa.md").is_file(),
        "kept, not deleted"
    );
    let rows = rows(&a);
    assert!(
        rows.iter()
            .any(|r| r.starts_with("# Not put in the Recycle Bin, kept (")),
        "{rows:?}"
    );
    assert!(
        !rows.iter().any(|r| r.starts_with("# Changed since, kept")),
        "{rows:?}"
    );
    // The run is not undone while a note of it is there (audit 4, F2 and F10): Undo again,
    // with a bin that takes it, takes the rest out.
    let record = mdedit_core::runs::load(&record_path, &library).expect("the record");
    assert_eq!(record.state, mdedit_core::runs::State::UndoPending);
    let mut bin = FolderBin::new(&dir.join("bin"));
    bin.calls = calls.clone();
    a.recycle = Box::new(bin);
    a.run_undo_ask(record_path.clone());
    a.run_undo(record_path.clone());
    assert!(!library.join("Trip/Tokyo/asakusa.md").exists(), "taken out");
    assert_eq!(
        mdedit_core::runs::load(&record_path, &library).map(|r| r.state),
        Some(mdedit_core::runs::State::Undone)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 3 C6, audit 4 F5b: undo keeps a note a tab holds with changes not saved, says why,
/// and the run waits for it; once the tab is gone (here closed unsaved), Undo again takes
/// it out. The bin is never handed the note while the tab holds changes.
#[test]
fn undo_keeps_a_note_a_tab_holds_with_changes_not_saved() {
    let (mut a, library, _, dir, calls) = planned("bin-undo-unsaved", |_| {});
    a.plan.as_mut().expect("open").keep_the_rest();
    press(&mut a, KeyName::Enter);
    let record_path =
        a.ui.results
            .replace
            .outcome
            .as_ref()
            .and_then(|o| o.run.clone())
            .expect("the record");
    let note = library.join("Trip/Tokyo/asakusa.md");
    open_now(&mut a, &note);
    a.key_pressed(Key::Name(KeyName::Char('x')), Some("x"));
    assert!(a.docs.active_doc().is_some_and(|d| d.modified()), "unsaved");
    a.run_undo_ask(record_path.clone());
    a.run_undo(record_path.clone());
    assert!(note.is_file(), "kept");
    assert!(
        !calls.borrow().iter().any(|p| p.ends_with("asakusa.md")),
        "the bin was not handed it: {:?}",
        calls.borrow()
    );
    let rows = rows(&a);
    assert!(
        rows.iter()
            .any(|r| r.contains(crate::recycle::UNSAVED)),
        "{rows:?}"
    );
    assert_eq!(
        mdedit_core::runs::load(&record_path, &library).map(|r| r.state),
        Some(mdedit_core::runs::State::UndoPending)
    );
    // The tab's changes are thrown away; Undo again takes the note out.
    if let Some(i) = a.docs.active() {
        a.docs.close(i, None);
    }
    a.run_undo_ask(record_path.clone());
    a.run_undo(record_path.clone());
    assert!(!note.exists(), "taken out");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_report_demos_draw_in_both_themes() {
    for name in crate::plan_demo::REPORTS {
        let mut a = app();
        assert!(a.plan_demo(name), "{name}");
        let rows = rows(&a);
        assert!(
            rows.iter().any(|r| r.contains("Recycle Bin")),
            "{name}: {rows:?}"
        );
        for dark in [false, true] {
            a.pal = if dark { theme::DARK } else { theme::LIGHT };
            a.full = true;
            a.compose();
        }
    }
}
