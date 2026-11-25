//! Write notes' write (pictures plan, phase V5): the plan becomes notes and copies, in the order
//! of 4.3, refused before anything is written when it cannot be; and its undo. The plans are made
//! with the AI off, here, one note per picture (no engine is asked).

use super::files_tests::{app, open_now, scratch, type_text};
use super::*;
use crate::task_ui::{DestChoice, TaskKind};
use mdedit_core::notes_ai::HowMany;
use mdedit_core::runs;

/// Three pictures of distinct bytes (never decoded: a copy is the original byte for byte).
fn pictures(dir: &Path) -> Vec<PathBuf> {
    let desk = dir.join("desk");
    std::fs::create_dir_all(&desk).expect("desk");
    (0..3)
        .map(|i| {
            let p = desk.join(format!("Shot {i}.png"));
            std::fs::write(&p, format!("picture {i} bytes").as_bytes()).expect("picture");
            p
        })
        .collect()
}

/// A window of Write notes over three pictures, the AI off, its plan made (one note per
/// picture), into a new group "Shots".
fn planned(tag: &str) -> (App, PathBuf, Vec<PathBuf>) {
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let shots = pictures(&dir);
    a.task_open(TaskKind::Notes, shots.clone());
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::New("Shots".to_string());
        s.new_group = Some(mdshared_ui::textfield::TextField::new());
        s.new_group.as_mut().expect("field").set_text("Shots");
        s.notes.how_many = HowMany::PerPicture;
    }
    a.task_new_group_commit();
    a.task_run();
    assert!(arranged(&a).is_some(), "the plan is made here");
    (a, dir, shots)
}

fn arranged(a: &App) -> Option<mdedit_core::notes_ai::Arranged> {
    a.task.as_ref()?.notes_run.as_ref()?.arranged.clone()
}

fn toast(a: &App) -> String {
    a.docs.toast(Instant::now()).unwrap_or_default().to_string()
}

fn home(dir: &Path) -> PathBuf {
    dir.join("library").join("Shots")
}

fn group_file(dir: &Path) -> PathBuf {
    dir.join("library").join("Shots.md")
}

fn records(a: &App) -> Vec<runs::Run> {
    runs::list_all(&a.library_dir().expect("library")).0
}

/// Every file under `dir`, for "nothing was written".
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Section 9 and 4.4: each note in the group's folder, its picture copied byte for byte into
/// `pictures/` beside it and linked there; the links in the group's file; a record that holds each
/// copy with its original and digest; the window closed, the Notes pane and a tab on the first
/// note.
#[test]
fn the_notes_are_written_with_their_copies_beside_them() {
    let (mut a, dir, shots) = planned("nw-write");
    a.notes_write();
    assert!(a.task.is_none(), "the window closes");
    let note = home(&dir).join("shot-0.md");
    let text = std::fs::read_to_string(&note).expect("the first note");
    assert!(text.starts_with("# Shot 0\n"), "{text}");
    assert!(text.contains("](pictures/Shot%200.png)"), "{text}");
    let copy = home(&dir).join("pictures").join("Shot 0.png");
    assert_eq!(
        std::fs::read(&copy).expect("the copy"),
        std::fs::read(&shots[0]).expect("the original"),
        "a copy is the original byte for byte"
    );
    assert_eq!(
        std::fs::read(&shots[0]).expect("the original"),
        b"picture 0 bytes",
        "the original is untouched"
    );
    let group = std::fs::read_to_string(group_file(&dir)).expect("the group");
    assert!(group.contains("[Shot 0](Shots/shot-0.md)"), "{group}");
    assert!(group.contains("[Shot 2](Shots/shot-2.md)"), "{group}");
    let list = records(&a);
    assert_eq!(list.len(), 1, "one record");
    let r = &list[0].record;
    assert_eq!(r.kind, runs::Kind::Notes);
    assert_eq!(r.state, runs::State::Done);
    assert!(mdedit_core::notes_write::is_notes_run(r));
    assert!(
        list[0].path.to_string_lossy().ends_with("-notes.json"),
        "{:?}",
        list[0].path
    );
    // No source: nothing was cut, and undo checks none (section 10).
    assert_eq!((r.source.len, r.source.sha256.as_str()), (0, ""));
    assert_eq!(r.notes.len(), 3);
    assert!(r.notes.iter().all(|n| n.created));
    assert_eq!(r.pictures.len(), 3);
    let p = &r.pictures[0];
    assert!(p.created);
    assert_eq!(p.original.as_deref(), Some(shots[0].as_path()));
    assert_eq!(
        p.original_sha256,
        mdshared_app::update::sha256_hex(b"picture 0 bytes")
    );
    assert!(a.ui.panes.files && a.ui.library.on, "the Notes pane shows");
    assert!(
        a.docs
            .active_doc()
            .and_then(|d| d.path.as_deref())
            .is_some_and(|p| mdshared_app::paths::same_path(p, &note)),
        "the first note is open in front"
    );
    let said = toast(&a);
    assert!(
        said.contains("Wrote 3 notes and 3 pictures in Shots"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 15: a note's name that is taken is refused before anything is written, the window
/// stays with its plan, and the reason is said.
#[test]
fn a_taken_note_name_refuses_the_run_and_writes_nothing() {
    let (mut a, dir, _) = planned("nw-taken");
    std::fs::create_dir_all(home(&dir)).expect("home");
    std::fs::write(home(&dir).join("shot-1.md"), "mine").expect("a note of mine");
    let before = files_under(&dir.join("library"));
    let group = std::fs::read(group_file(&dir)).expect("group");
    a.notes_write();
    assert!(a.task.is_some(), "the window stays");
    assert!(arranged(&a).is_some(), "the plan is kept");
    assert_eq!(files_under(&dir.join("library")), before, "nothing written");
    assert_eq!(std::fs::read(group_file(&dir)).expect("group"), group);
    let said = toast(&a);
    assert!(said.contains("shot-1.md is there already"), "{said}");
    assert!(a.notes_write_blocked().is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 15: the group's file open with changes not saved refuses the run before anything is
/// written.
#[test]
fn the_group_file_open_with_changes_refuses_the_run() {
    let (mut a, dir, _) = planned("nw-unsaved");
    let task = a.task.take();
    open_now(&mut a, &group_file(&dir));
    type_text(&mut a, "x");
    a.task = task;
    let before = files_under(&dir.join("library"));
    a.notes_write();
    assert!(a.task.is_some(), "the window stays");
    assert_eq!(files_under(&dir.join("library")), before, "nothing written");
    let said = toast(&a);
    assert!(
        said.contains("Shots.md") && said.contains("not saved"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 11: a group that would pass the library's 2,000 notes refuses the run.
#[test]
fn a_full_group_refuses_the_run() {
    let (mut a, dir, _) = planned("nw-full");
    let mut text = std::fs::read_to_string(group_file(&dir)).expect("group");
    for i in 0..mdshared_app::library::MAX_ENTRIES - 1 {
        text.push_str(&format!("- [N{i}](Shots/n{i}.md)\n"));
    }
    std::fs::write(group_file(&dir), &text).expect("group");
    a.notes_write();
    assert!(a.task.is_some(), "the window stays");
    assert!(!home(&dir).join("shot-0.md").exists());
    let said = toast(&a);
    assert!(said.contains("more than 2000 notes"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 9: "Copy the pictures beside the notes" off links each picture where it is, by a
/// path relative to its note, and copies nothing.
#[test]
fn with_copy_off_the_pictures_are_linked_where_they_are() {
    let (mut a, dir, _) = planned("nw-link");
    if let Some(s) = a.task.as_mut() {
        s.notes.copy = false;
    }
    a.notes_write();
    let text = std::fs::read_to_string(home(&dir).join("shot-0.md")).expect("note");
    assert!(text.contains("](../../desk/Shot%200.png)"), "{text}");
    assert!(!home(&dir).join("pictures").exists(), "nothing copied");
    assert!(records(&a)[0].record.pictures.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The plan card's renames (`Run::copy_names`) name the copies, and the notes link them.
#[test]
fn a_copy_takes_the_name_the_owner_gave_it() {
    let (mut a, dir, _) = planned("nw-named");
    if let Some(run) = a.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
        run.copy_names.insert(1, "login error.png".to_string());
    }
    a.notes_write();
    assert!(
        home(&dir)
            .join("pictures")
            .join("login error.png")
            .is_file()
    );
    let text = std::fs::read_to_string(home(&dir).join("shot-1.md")).expect("note");
    assert!(text.contains("](pictures/login%20error.png)"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 9: a name taken by another file in `pictures/` becomes `-2`; never written over.
#[test]
fn a_copy_never_goes_over_a_file_of_its_name() {
    let (mut a, dir, _) = planned("nw-clash");
    let pics = home(&dir).join("pictures");
    std::fs::create_dir_all(&pics).expect("pictures");
    std::fs::write(pics.join("Shot 0.png"), "someone else's").expect("theirs");
    a.notes_write();
    assert_eq!(
        std::fs::read(pics.join("Shot 0.png")).expect("theirs"),
        b"someone else's"
    );
    assert_eq!(
        std::fs::read(pics.join("Shot 0-2.png")).expect("the copy"),
        b"picture 0 bytes"
    );
    let text = std::fs::read_to_string(home(&dir).join("shot-0.md")).expect("note");
    assert!(text.contains("](pictures/Shot%200-2.png)"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 15: a picture deleted before the write fails to copy; the report says so, its note is
/// written without it and says why, and the record holds what the note holds.
#[test]
fn a_copy_that_fails_leaves_its_note_saying_so() {
    let (mut a, dir, shots) = planned("nw-fail");
    std::fs::remove_file(&shots[1]).expect("deleted");
    a.notes_write();
    let note = home(&dir).join("shot-1.md");
    let text = std::fs::read_to_string(&note).expect("the note is written");
    assert!(
        !text.contains("]("),
        "no link to a copy that is not there: {text}"
    );
    assert!(text.contains("Shot 1.png could not be copied"), "{text}");
    let said = toast(&a);
    assert!(
        said.contains("Shot 1.png could not be copied") && said.contains("written without it"),
        "{said}"
    );
    let r = records(&a).remove(0).record;
    assert_eq!(r.pictures.len(), 2, "the record holds the two copies made");
    let rec = r.notes.iter().find(|n| n.path == note).expect("its note");
    assert_eq!(
        rec.sha256,
        mdshared_app::update::sha256_hex(text.as_bytes()),
        "the record tells the truth"
    );
    assert!(home(&dir).join("shot-0.md").is_file());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 10: undo removes the notes, the copies, the links and the folders the run made, and
/// never an original; the question first says how many notes and pictures go.
#[test]
fn undo_removes_the_notes_copies_and_links_and_never_an_original() {
    let (mut a, dir, shots) = planned("nw-undo");
    let group_before = std::fs::read(group_file(&dir)).expect("group");
    a.notes_write();
    let path = records(&a)[0].path.clone();
    a.run_undo_ask(path.clone());
    let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(can, "{question}");
    assert!(question.contains("3 notes and 3 pictures"), "{question}");
    a.run_undo(path);
    assert!(
        !home(&dir).exists(),
        "the notes, the copies and the folders are gone: {:?}",
        files_under(&dir.join("library"))
    );
    assert_eq!(
        std::fs::read(group_file(&dir)).expect("group"),
        group_before
    );
    for (i, s) in shots.iter().enumerate() {
        assert_eq!(
            std::fs::read(s).expect("the original stays"),
            format!("picture {i} bytes").as_bytes()
        );
    }
    assert_eq!(records(&a)[0].record.state, runs::State::Undone);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 10: a copy whose original is gone is the only one left, and undo keeps it; the rest
/// of the run goes.
#[test]
fn undo_keeps_a_copy_whose_original_is_gone() {
    let (mut a, dir, shots) = planned("nw-undo-gone");
    a.notes_write();
    std::fs::remove_file(&shots[0]).expect("deleted");
    std::fs::remove_file(&shots[2]).expect("deleted");
    let path = records(&a)[0].path.clone();
    a.run_undo(path);
    let pics = home(&dir).join("pictures");
    assert!(
        pics.join("Shot 0.png").is_file(),
        "the only copy of shot 0 stays"
    );
    assert!(
        pics.join("Shot 2.png").is_file(),
        "the only copy of shot 2 stays"
    );
    assert!(
        !pics.join("Shot 1.png").exists(),
        "a copy of an original that is there goes"
    );
    assert!(!home(&dir).join("shot-0.md").exists(), "the notes go");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Section 15: a run that died after its copies, before its notes, is listed in "AI runs…" and
/// its undo takes the copies and folders away.
#[test]
fn a_run_that_died_before_its_notes_is_undone_from_its_record() {
    let (mut a, dir, shots) = planned("nw-died");
    super::notes_write::DIES_AT.with(|d| d.set(Some("before the notes")));
    a.notes_write();
    super::notes_write::DIES_AT.with(|d| d.set(None));
    assert!(home(&dir).join("pictures").join("Shot 0.png").is_file());
    assert!(!home(&dir).join("shot-0.md").exists());
    let list = records(&a);
    assert_eq!(list[0].record.state, runs::State::Writing);
    a.run_undo(list[0].path.clone());
    assert!(
        !home(&dir).exists(),
        "{:?}",
        files_under(&dir.join("library"))
    );
    assert!(shots.iter().all(|s| s.is_file()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of V3: a pasted picture lives in a temp folder that goes with the window, so
/// it is copied beside its note even with "Copy the pictures beside the notes" off, and the note
/// never links the temp folder.
#[test]
fn a_pasted_picture_is_copied_even_with_copy_off() {
    let dir = scratch("nw-pasted");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let pasted_dir = crate::notes_task::pasted_dir();
    std::fs::create_dir_all(&pasted_dir).expect("pasted");
    let pasted = pasted_dir.join(format!("Screenshot nw-{}.png", std::process::id()));
    std::fs::write(&pasted, b"pasted bytes").expect("pasted");
    assert!(crate::notes_task::is_pasted(&pasted));
    a.task_open(TaskKind::Notes, vec![pasted.clone()]);
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::New("Shots".to_string());
        s.new_group = Some(mdshared_ui::textfield::TextField::new());
        s.new_group.as_mut().expect("field").set_text("Shots");
        s.notes.how_many = HowMany::PerPicture;
        s.notes.copy = false;
    }
    a.task_new_group_commit();
    a.task_run();
    a.notes_write();
    let _ = std::fs::remove_file(&pasted);
    let copies: Vec<PathBuf> = files_under(&home(&dir).join("pictures"));
    assert_eq!(copies.len(), 1, "{copies:?}");
    assert_eq!(std::fs::read(&copies[0]).expect("copy"), b"pasted bytes");
    let note = files_under(&home(&dir))
        .into_iter()
        .find(|p| p.extension().is_some_and(|e| e == "md"))
        .expect("the note");
    let text = std::fs::read_to_string(&note).expect("note");
    assert!(text.contains("](pictures/"), "{text}");
    assert!(!text.contains("mdedit-pasted"), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The first defect of the record V5 wrote as a Markdown run: its faked source was the first
/// copy, so undo refused once the owner deleted or edited that copy, saying "the file these
/// notes were cut from". A run of Write notes has no source: undo goes on, and the rest goes.
#[test]
fn undo_runs_when_the_owner_deleted_or_changed_a_copy() {
    for edit in [false, true] {
        let (mut a, dir, shots) = planned(if edit {
            "nw-copy-edited"
        } else {
            "nw-copy-gone"
        });
        a.notes_write();
        let first = home(&dir).join("pictures").join("Shot 0.png");
        if edit {
            std::fs::write(&first, "drawn on").expect("edited");
        } else {
            std::fs::remove_file(&first).expect("deleted");
        }
        let path = records(&a)[0].path.clone();
        a.run_undo_ask(path.clone());
        let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
        assert!(can, "{question}");
        assert!(!question.contains("cut from"), "{question}");
        a.run_undo(path);
        assert!(!home(&dir).join("shot-0.md").exists(), "the notes go");
        assert!(!home(&dir).join("pictures").join("Shot 1.png").exists());
        assert_eq!(
            first.is_file(),
            edit,
            "an edited copy stays, a deleted one is gone"
        );
        assert!(shots.iter().all(|s| s.is_file()), "never an original");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// The same with "Copy the pictures" off: the faked source was the first original, so undo
/// refused once the owner deleted that screenshot. Undo goes on.
#[test]
fn with_copy_off_undo_runs_when_an_original_is_gone() {
    let (mut a, dir, shots) = planned("nw-link-undo");
    if let Some(s) = a.task.as_mut() {
        s.notes.copy = false;
    }
    a.notes_write();
    std::fs::remove_file(&shots[0]).expect("deleted");
    let path = records(&a)[0].path.clone();
    a.run_undo_ask(path.clone());
    let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(can, "{question}");
    a.run_undo(path);
    assert!(
        !home(&dir).exists(),
        "{:?}",
        files_under(&dir.join("library"))
    );
    assert_eq!(records(&a)[0].record.state, runs::State::Undone);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The second defect: a note changed since is kept by undo, and the copy it shows was removed
/// with the others, breaking its picture. It stays with the note, and the question says so.
#[test]
fn undo_keeps_a_changed_note_and_the_copy_it_shows() {
    let (mut a, dir, _) = planned("nw-undo-kept");
    a.notes_write();
    let note = home(&dir).join("shot-0.md");
    let mut text = std::fs::read_to_string(&note).expect("note");
    text.push_str("\nMy own words.\n");
    std::fs::write(&note, &text).expect("edited");
    let path = records(&a)[0].path.clone();
    a.run_undo_ask(path.clone());
    let (_, question, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(can, "{question}");
    assert!(
        question.contains("1 picture stays with the note that shows it."),
        "{question}"
    );
    a.run_undo(path);
    let pics = home(&dir).join("pictures");
    assert_eq!(std::fs::read_to_string(&note).expect("kept"), text);
    assert!(
        pics.join("Shot 0.png").is_file(),
        "the picture it shows stays"
    );
    assert!(!pics.join("Shot 1.png").exists() && !pics.join("Shot 2.png").exists());
    assert!(!home(&dir).join("shot-1.md").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of V5, 1: a run that dies at any step leaves a record that owns everything it
/// made, so undo from that record leaves the library and the desk as they were.
#[test]
fn a_run_that_died_at_any_step_is_undone_from_its_record() {
    for step in [
        "after the record",
        "after the folders",
        "mid-copies",
        "before the notes",
        "mid-notes",
        "before the links",
    ] {
        let (mut a, dir, shots) = planned("nw-died-at");
        let group = std::fs::read(group_file(&dir)).expect("group");
        super::notes_write::DIES_AT.with(|d| d.set(Some(step)));
        a.notes_write();
        super::notes_write::DIES_AT.with(|d| d.set(None));
        let list = records(&a);
        assert_eq!(list.len(), 1, "{step}");
        assert_eq!(list[0].record.state, runs::State::Writing, "{step}");
        a.run_undo(list[0].path.clone());
        assert!(
            !home(&dir).exists(),
            "{step}: {:?}",
            files_under(&dir.join("library"))
        );
        assert_eq!(
            std::fs::read(group_file(&dir)).expect("group"),
            group,
            "{step}"
        );
        assert!(shots.iter().all(|s| s.is_file()), "{step}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A file where the copies' folder would be, so every copy of the run fails to be written.
fn block_the_copies(dir: &Path) {
    std::fs::create_dir_all(home(dir)).expect("home");
    std::fs::write(home(dir).join("pictures"), "a file, not a folder").expect("blocked");
}

/// Fable's review of V5, 9: a note recomposed with the copies that failed said in it is held to
/// the note's limit again, and left out past it; and review 2: when no note is written, the
/// window stays open with its pictures and says why.
#[test]
fn a_note_grown_past_its_limit_by_a_failed_copy_is_left_out() {
    let (mut a, dir, _) = planned("nw-too-large");
    // Each note just under its limit as planned, so that saying its copy failed takes it over.
    let pad = "x".repeat(mdedit_core::notes_write::NOTE_BYTES_MAX - 200);
    if let Some(run) = a.task.as_mut().and_then(|s| s.notes_run.as_mut()) {
        for d in run.arranged.as_mut().expect("plan").notes.iter_mut() {
            d.body.push(mdedit_core::notes_ai::Part::Text(pad.clone()));
            d.body.push(mdedit_core::notes_ai::Part::Text(
                "y".repeat(150).to_string(),
            ));
        }
    }
    assert_eq!(a.notes_write_blocked(), None, "the plan fits as planned");
    block_the_copies(&dir);
    a.notes_write();
    let written: Vec<PathBuf> = files_under(&home(&dir))
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    assert!(written.is_empty(), "{written:?}");
    assert!(a.task.is_some(), "the window stays");
    let said = toast(&a);
    assert!(said.contains("No note could be written"), "{said}");
    assert!(said.contains("KB a note may hold"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A window of Write notes over one pasted picture, Copy off, its plan made.
fn pasted_run(tag: &str) -> (App, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let pasted_dir = crate::notes_task::pasted_dir();
    std::fs::create_dir_all(&pasted_dir).expect("pasted");
    let pasted = pasted_dir.join(format!("Screenshot {tag}-{}.png", std::process::id()));
    std::fs::write(&pasted, b"pasted bytes").expect("pasted");
    a.task_open(TaskKind::Notes, vec![pasted.clone()]);
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::New("Shots".to_string());
        s.new_group = Some(mdshared_ui::textfield::TextField::new());
        s.new_group.as_mut().expect("field").set_text("Shots");
        s.notes.how_many = HowMany::PerPicture;
        s.notes.copy = false;
    }
    a.task_new_group_commit();
    a.task_run();
    (a, dir, pasted)
}

/// Fable's review of V5, 2: a pasted picture whose copy failed is the only copy of that
/// screenshot, so the window does not close (closing deletes the pasted files) and says why.
#[test]
fn a_pasted_picture_whose_copy_failed_keeps_the_window_open() {
    let (mut a, dir, pasted) = pasted_run("nw-pasted-fail");
    block_the_copies(&dir);
    a.notes_write();
    assert!(a.task.is_some(), "the window stays");
    assert!(pasted.is_file(), "the pasted picture is not lost");
    let said = toast(&a);
    assert!(said.contains("pasted picture"), "{said}");
    a.task_close();
    let _ = std::fs::remove_file(&pasted);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of V5, 5: a copy of a pasted picture names no original in the record, since
/// its temp file goes with the window; undo removes it while it is what was written.
#[test]
fn a_pasted_picture_s_copy_is_undone_after_the_window_closed() {
    let (mut a, dir, pasted) = pasted_run("nw-pasted-undo");
    a.notes_write();
    assert!(a.task.is_none(), "the window closed");
    assert!(!pasted.exists(), "its pasted file went with it");
    let list = records(&a);
    let r = &list[0].record;
    assert_eq!(r.pictures.len(), 1);
    assert!(r.pictures[0].created);
    assert_eq!(r.pictures[0].original, None, "{:?}", r.pictures[0]);
    a.run_undo(list[0].path.clone());
    assert!(
        !home(&dir).exists(),
        "{:?}",
        files_under(&dir.join("library"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of V5, 6: with Copy off a picture on a network share is copied, as a pasted
/// one is, since no `file:` link from a note leads to it; one on a drive is linked where it is.
#[test]
fn a_picture_on_a_network_share_is_copied_even_with_copy_off() {
    use super::notes_write::{copied_anyway, on_share};
    for share in [
        r"\\server\share\a.png",
        r"\\?\UNC\server\share\a.png",
        "//server/share/a.png",
    ] {
        assert!(on_share(Path::new(share)), "{share}");
        assert!(copied_anyway(Path::new(share)), "{share}");
    }
    for local in [r"C:\desk\a.png", r"\\?\C:\desk\a.png", "D:/desk/a.png"] {
        assert!(!on_share(Path::new(local)), "{local}");
        assert!(!copied_anyway(Path::new(local)), "{local}");
    }
    assert!(copied_anyway(
        &crate::notes_task::pasted_dir().join("x.png")
    ));
}
