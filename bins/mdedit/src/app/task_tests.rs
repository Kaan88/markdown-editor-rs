//! The AI task window over files (`docs/ai-library-plan.md` 5.6): what it takes, what it sends,
//! where it writes, and the undo of a rewrite.

use super::files_tests::{app, scratch};
use super::*;
use crate::keys::KeyName;
use crate::task_ui::{DestChoice, FileKind, Hit, TaskKind};

/// A window over a scratch folder with a library beside its settings.
fn open(tag: &str, kind: TaskKind, files: Vec<PathBuf>) -> (App, PathBuf) {
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    a.task_open(kind, files);
    (a, dir)
}

fn pasta(dir: &Path) -> PathBuf {
    let p = dir.join("pasta.md");
    std::fs::write(&p, "# Pasta\n\nBoil it.\n").expect("pasta");
    p
}

#[test]
fn a_window_opens_with_no_files_and_no_request() {
    let (a, _dir) = open("task-open", TaskKind::File, Vec::new());
    let s = a.task.as_ref().expect("the window is open");
    assert!(s.files.is_empty());
    assert!(s.asking.is_none());
}

#[test]
fn a_file_added_is_probed_and_a_picture_is_never_read() {
    let dir = scratch("task-probe");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let picture = dir.join("lasagne.jpg");
    std::fs::write(&picture, [0xFF, 0xD8, 0xFF, 0xE0]).expect("picture");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::Rewrite, vec![text, picture]);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), 2);
    assert_eq!(s.files[0].kind, FileKind::Text);
    assert_eq!(s.files[0].heading.as_deref(), Some("Pasta"));
    assert!(s.files[0].text.is_some());
    assert_eq!(s.files[1].kind, FileKind::Picture);
    assert!(s.files[1].text.is_none());
    assert_eq!(
        s.files[1].reason_for(TaskKind::Rewrite).as_deref(),
        Some("a picture: not sent for this task")
    );
}

#[test]
fn the_filing_request_sends_names_and_digests_and_no_path() {
    let dir = scratch("task-digest");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text]);
    let s = a.task.as_ref().expect("open");
    let ids = s.taking_part();
    let prompt = crate::task_ui::filing_prompt("By dish", &ids, &s.files);
    assert!(prompt.contains("pasta.md"));
    assert!(prompt.contains("first heading: Pasta"));
    assert!(!prompt.contains(&dir.to_string_lossy().to_string()));
    assert!(prompt.contains("By dish"));
}

#[test]
fn a_new_group_is_made_in_the_library_and_chosen() {
    let (mut a, dir) = open("task-group", TaskKind::File, Vec::new());
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::New("Recipes".to_string());
        s.new_group = Some(mdshared_ui::textfield::TextField::new());
        s.new_group.as_mut().expect("field").set_text("Recipes");
    }
    a.task_new_group_commit();
    let s = a.task.as_ref().expect("open");
    assert!(matches!(s.dest, DestChoice::Group(_)));
    assert_eq!(s.groups.last().map(|g| g.name.as_str()), Some("Recipes"));
    assert!(dir.join("library").join("Recipes.md").is_file());
}

#[test]
fn escape_closes_the_window() {
    let (mut a, _dir) = open("task-escape", TaskKind::File, Vec::new());
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task.is_none());
}

#[test]
fn a_click_on_a_file_toggles_its_tick() {
    let dir = scratch("task-tick");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text]);
    a.compose();
    assert!(a.task.as_ref().expect("open").files[0].ticked);
    let centre = a.task.as_ref().and_then(|s| {
        s.hits
            .iter()
            .find(|(h, _)| *h == Hit::File(0))
            .map(|(_, r)| (r.x + r.w / 2.0, r.y + r.h / 2.0))
    });
    let (x, y) = centre.expect("the row was drawn");
    a.ui.mouse = (x, y);
    a.task_press();
    a.task_release();
    assert!(!a.task.as_ref().expect("open").files[0].ticked);
}

#[test]
fn no_group_chosen_writes_nothing() {
    let dir = scratch("task-nogroup");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text]);
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::Decide;
        s.planned = true;
        s.files[0].title = Some("Pasta".to_string());
    }
    a.task_file_write();
    assert!(!dir.join("library").join("Recipes.md").exists());
    assert!(a.task.is_some(), "the window stays");
}
