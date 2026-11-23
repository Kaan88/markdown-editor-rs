//! The AI task window over files (`docs/ai-library-plan.md` 5.6): what it takes, what it sends,
//! where it writes, and the undo of a rewrite.

use super::ai_tests::{engine, wait};
use super::files_tests::{app, scratch};
use super::*;
use crate::ai::Panel;
use crate::keys::KeyName;
use crate::task_ui::{DestChoice, FileKind, Hit, TaskKind};
use mdedit_core::ai::Connection;
use mdedit_core::runs;

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

/// The same, with the AI on and 5.5's review not opening its panes.
fn ai_open(tag: &str, kind: TaskKind, files: Vec<PathBuf>) -> (App, PathBuf) {
    engine();
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.docs.prefs.ai_review_preview = false;
    a.task_open(kind, files);
    (a, dir)
}

fn pasta(dir: &Path) -> PathBuf {
    let p = dir.join("pasta.md");
    std::fs::write(&p, "# Pasta\n\nBoil it.\n").expect("pasta");
    p
}

/// A group made and chosen, so "Add N to the library" has somewhere to go.
fn with_group(a: &mut App, name: &str) {
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::New(name.to_string());
        s.new_group = Some(mdshared_ui::textfield::TextField::new());
        s.new_group.as_mut().expect("field").set_text(name);
    }
    a.task_new_group_commit();
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
    with_group(&mut a, "Recipes");
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::Decide;
        s.planned = true;
        s.files[0].title = Some("Pasta".to_string());
    }
    a.task_file_write();
    let body = std::fs::read_to_string(dir.join("library").join("Recipes.md")).expect("group");
    assert!(!body.contains("[Pasta]"), "{body}");
    assert!(a.task.is_some(), "the window stays");
}

#[test]
fn file_them_with_the_ai_off_plans_then_writes_the_link() {
    let dir = scratch("task-off");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text.clone()]);
    with_group(&mut a, "Recipes");
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert!(s.planned, "the plan is made without a request");
    assert!(s.asking.is_none());
    a.task_file_write();
    assert!(a.task.is_none(), "the window closes when the links are in");
    let group = dir.join("library").join("Recipes.md");
    let body = std::fs::read_to_string(&group).expect("the group file");
    assert!(body.contains("[Pasta]"), "{body}");
    let lib = mdshared_app::library::parse(&body, &dir.join("library"));
    assert_eq!(lib.entries.len(), 1);
    assert!(crate::library_pane::same_note(&lib.entries[0].path, &text));
}

#[test]
fn file_them_sends_names_and_digests_and_writes_what_the_model_placed() {
    let dir = scratch("task-file-ai");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let picture = dir.join("lasagne.jpg");
    std::fs::write(&picture, [0xFF, 0xD8, 0xFF, 0xE0]).expect("picture");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text.clone(), picture.clone()]);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    engine();
    with_group(&mut a, "Recipes");
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
        s.instruction.set_text("TASKFILE: by dish");
        s.files[1].ticked = true;
    }
    a.task_run();
    wait(&mut a, |a| a.task.as_ref().is_some_and(|s| s.planned));
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files[0].title.as_deref(), Some("Pasta"));
    assert_eq!(s.files[0].sections, ["Food", "Dinner"]);
    assert_eq!(
        s.files[1].with,
        Some(0),
        "the picture is placed with the text"
    );
    a.task_file_write();
    let group = dir.join("library").join("Recipes.md");
    let body = std::fs::read_to_string(&group).expect("the group file");
    assert!(body.contains("## Food"), "{body}");
    assert!(body.contains("### Dinner"), "{body}");
    assert!(body.contains("[Pasta]"), "{body}");
    assert!(body.contains("[lasagne]"), "{body}");
    assert!(
        body.contains("lasagne.jpg"),
        "the picture is linked by name"
    );
    assert!(crate::library_pane::same_note(
        &mdshared_app::library::parse(&body, &dir.join("library")).entries[0].path,
        &text
    ));
}

#[test]
fn a_forty_first_file_waits_for_a_second_request() {
    let dir = scratch("task-batch");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let mut files = Vec::new();
    for i in 0..41 {
        let p = dir.join(format!("note-{i}.md"));
        std::fs::write(&p, format!("# Note {i}\n")).expect("note");
        files.push(p);
    }
    let (mut a, _dir) = ai_open("task-batch-ai", TaskKind::File, files);
    if let Some(s) = a.task.as_mut() {
        s.instruction.set_text("TASKFILE: by number");
    }
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.asking.as_ref().expect("out").batch.len(), 40);
    assert_eq!(s.batch_queue.len(), 2, "the 41st is in a second request");
}

#[test]
fn unticked_files_are_not_sent() {
    let (mut a, dir) = ai_open("task-tick-send", TaskKind::File, Vec::new());
    let first = pasta(&dir);
    let extra = dir.join("extra.md");
    std::fs::write(&extra, "# Extra\n").expect("extra");
    a.task_add_paths(vec![first, extra]);
    if let Some(s) = a.task.as_mut() {
        s.instruction.set_text("TASKFILE: by dish");
        s.files[1].ticked = false;
    }
    a.task_run();
    let s = a.task.as_ref().expect("open");
    let batch = &s.asking.as_ref().expect("out").batch;
    assert_eq!(batch.len(), 1);
    assert_eq!(batch[0], 0);
}

#[test]
fn a_rewrite_is_reviewed_then_written_in_place_with_its_original_in_the_record() {
    let dir = scratch("task-rewrite");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::Rewrite, vec![text.clone()]);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    engine();
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
        s.instruction.set_text("Tidy it");
    }
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    assert_eq!(
        std::fs::read_to_string(&text).expect("unchanged"),
        "# Pasta\n\nBoil it.\n",
        "nothing is written before Apply"
    );
    let Some(Panel::Review(rv)) = &a.ai.panel else {
        panic!("a review");
    };
    assert!(rv.file.is_some(), "it is a review of a file");
    assert!(!rv.hunks.is_empty(), "the changes are listed");
    a.task_review_apply();
    assert_eq!(std::fs::read_to_string(&text).expect("written"), "# Done\n");
    assert!(a.ai.panel.is_none(), "the review is closed");
    assert!(a.task.is_none(), "the run is over");
    let root = dir.join("library");
    let runs = runs::list(&root);
    assert_eq!(runs.len(), 1, "the run's record");
    assert_eq!(runs[0].record.kind, runs::Kind::Rewrite);
    assert_eq!(runs[0].record.originals.len(), 1);
    assert_eq!(runs[0].record.originals[0].text, "# Pasta\n\nBoil it.\n");
}

struct NoGate;
impl runs::Gate for NoGate {
    fn add_links(&mut self, _file: &Path, _links: &[runs::Link]) -> Result<(), String> {
        Ok(())
    }
    fn remove_links(
        &mut self,
        _file: &Path,
        _links: &[runs::LinkRemoval],
        _headings: &[Vec<String>],
    ) -> Result<(), String> {
        Ok(())
    }
}

#[test]
fn undo_of_a_rewrites_record_puts_the_original_back() {
    let dir = scratch("task-undo");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::Rewrite, vec![text.clone()]);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    engine();
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
        s.instruction.set_text("Tidy it");
    }
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    a.task_review_apply();
    assert_eq!(std::fs::read_to_string(&text).expect("written"), "# Done\n");
    let root = dir.join("library");
    let run = runs::list(&root).remove(0);
    let report = runs::undo(&run.path, &run.record, &root, &mut NoGate);
    assert!(report.refusal.is_none(), "{:?}", report.refusal);
    assert_eq!(report.restored.len(), 1);
    assert_eq!(
        std::fs::read_to_string(&text).expect("back"),
        "# Pasta\n\nBoil it.\n"
    );
}

#[test]
fn a_file_changed_since_the_answer_is_not_written_and_says_so() {
    let dir = scratch("task-changed");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::Rewrite, vec![text.clone()]);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    engine();
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
        s.instruction.set_text("Tidy it");
    }
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    std::fs::write(&text, "# Pasta\n\nSomeone else edited this.\n").expect("edit");
    a.task_review_apply();
    let Some(Panel::Review(rv)) = &a.ai.panel else {
        panic!("the review stays");
    };
    assert!(rv.blocked.is_some(), "it says why it cannot write");
    assert_eq!(
        std::fs::read_to_string(&text).expect("unchanged"),
        "# Pasta\n\nSomeone else edited this.\n"
    );
    assert!(runs::list(&dir.join("library")).is_empty());
}

#[test]
fn the_second_file_waits_for_the_first_files_review() {
    let dir = scratch("task-two");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let one = pasta(&dir);
    let two = dir.join("bread.md");
    std::fs::write(&two, "# Bread\n\nBake it.\n").expect("bread");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::Rewrite, vec![one.clone(), two.clone()]);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    engine();
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
        s.instruction.set_text("Tidy them");
    }
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    assert_eq!(a.task_sent.requests, 1, "one request at a time");
    assert_eq!(
        std::fs::read_to_string(&two).expect("bread"),
        "# Bread\n\nBake it.\n",
        "the second file is untouched"
    );
    a.task_review_apply();
    wait(&mut a, |a| a.ai.panel.is_some());
    assert_eq!(a.task_sent.requests, 2);
    a.task_review_apply();
    assert!(a.task.is_none());
    let runs = runs::list(&dir.join("library"));
    assert_eq!(runs.len(), 1, "one run, two files");
    assert_eq!(runs[0].record.originals.len(), 2);
}

#[test]
fn split_them_opens_a_plan_on_the_file_and_no_tab() {
    let dir = scratch("task-split");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = dir.join("trip.md");
    std::fs::write(&text, "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n").expect("trip");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    let tabs = a.docs.tabs.len();
    a.task_open(TaskKind::Split, vec![text]);
    with_group(&mut a, "Trips");
    a.task_run();
    assert!(a.plan.is_some(), "the plan screen is open");
    assert!(a.task.is_none(), "the window waits behind it");
    assert_eq!(a.docs.tabs.len(), tabs, "nothing was opened in a tab");
    a.plan_close();
    let s = a.task.as_ref().expect("the window is back");
    assert!(!s.files[0].done, "Escape left the file alone");
}

#[test]
fn show_what_is_sent_puts_the_request_in_a_read_only_tab_and_parks_the_window() {
    let (mut a, dir) = ai_open("task-sent", TaskKind::File, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text]);
    if let Some(s) = a.task.as_mut() {
        s.instruction.set_text("TASKFILE: by dish");
    }
    a.task_show_sent();
    assert!(a.task.is_none(), "the window is parked");
    assert!(a.task_parked.is_some());
    let doc = a.docs.active_doc().expect("a tab");
    match &doc.state {
        crate::docs::DocState::Ready(r) => assert!(r.read_only),
        _ => panic!("the tab is ready"),
    }
    let text = doc.ready().expect("read").buffer.rope().to_string();
    assert!(text.contains("TASKFILE: by dish"), "{text}");
    assert!(text.contains("pasta.md"));
    assert!(
        !text.contains(&dir.to_string_lossy().to_string()),
        "no path"
    );
    a.task_open(TaskKind::File, Vec::new());
    assert!(a.task.is_some());
    assert!(a.task_parked.is_none());
}

#[test]
fn rewrite_with_the_ai_off_opens_the_settings_and_sends_nothing() {
    let (mut a, dir) = open("task-off-rw", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    a.task_run();
    assert!(a.task.as_ref().expect("open").asking.is_none());
    assert!(a.ui.settings.is_some(), "Settings opens on AI");
}

#[test]
fn escape_stops_the_request_then_closes_the_window() {
    let dir = scratch("task-stop");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let mut files = Vec::new();
    for i in 0..2 {
        let p = dir.join(format!("n{i}.md"));
        std::fs::write(&p, "# N\n").expect("n");
        files.push(p);
    }
    let (mut a, _dir) = ai_open("task-stop-ai", TaskKind::File, files);
    if let Some(s) = a.task.as_mut() {
        s.instruction.set_text("TASKFILE: by name");
    }
    a.task_run();
    assert!(a.task.as_ref().expect("open").asking.is_some());
    a.task_key(Some(KeyName::Escape), None);
    let s = a.task.as_ref().expect("the window stays");
    assert!(s.asking.is_none(), "the request is stopped");
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task.is_none(), "a second Escape closes it");
}

#[test]
fn a_file_the_model_did_not_place_is_not_written() {
    let dir = scratch("task-unplaced");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let other = dir.join("orphan.md");
    std::fs::write(&other, "# Orphan\n").expect("orphan");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text, other.clone()]);
    with_group(&mut a, "Recipes");
    if let Some(s) = a.task.as_mut() {
        s.planned = true;
        s.files[0].title = Some("Pasta".to_string());
        s.files[0].sections = vec!["Food".to_string()];
        // The second is left as the model left it: no title, no section, no place, still ticked.
        s.files[1].ticked = true;
    }
    assert_eq!(a.task.as_ref().expect("open").count(), 1);
    a.task_file_write();
    let body = std::fs::read_to_string(dir.join("library").join("Recipes.md")).expect("group");
    assert!(body.contains("[Pasta]"), "{body}");
    assert!(!body.contains("Orphan"), "{body}");
}
