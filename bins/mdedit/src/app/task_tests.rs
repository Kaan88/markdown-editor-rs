//! The AI task window over files (`docs/ai-library-plan.md` 5.6): what it takes, what it sends,
//! where it writes, and the undo of a rewrite.

use super::ai_tests::{engine, wait};
use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::ai::Panel;
use crate::keys::KeyName;
use crate::task_ui::{DestChoice, FileKind, Focus, Hit, TaskKind};
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

/// THE FILES on "Link them" (D-209): for the tests of what linking does; the default is to
/// copy the files in.
fn link_them(a: &mut App) {
    a.docs.prefs.ai_files = mdedit_core::prefs::FilesGo::Link;
    if let Some(s) = a.task.as_mut() {
        s.files_go = mdedit_core::prefs::FilesGo::Link;
    }
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
    let pdf = dir.join("menu.pdf");
    std::fs::write(&pdf, "%PDF-1.4\n").expect("pdf");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text.clone(), pdf.clone()]);
    with_group(&mut a, "Recipes");
    link_them(&mut a);
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert!(s.planned, "the plan is made without a request");
    assert!(s.asking.is_none());
    a.task_file_write();
    assert!(a.task.is_none(), "the window closes when the links are in");
    let group = dir.join("library").join("Recipes.md");
    let body = std::fs::read_to_string(&group).expect("the group file");
    assert!(body.contains("[Pasta]"), "{body}");
    assert!(body.contains("[menu]"), "the PDF is linked by name: {body}");
    let lib = mdshared_app::library::parse(&body, &dir.join("library"));
    assert_eq!(lib.entries.len(), 2);
    assert!(
        lib.entries
            .iter()
            .any(|e| crate::library_pane::same_note(&e.path, &text))
    );
    assert!(
        lib.entries
            .iter()
            .any(|e| crate::library_pane::same_note(&e.path, &pdf))
    );
}

#[test]
fn file_them_sends_digests_and_writes_what_the_model_placed_and_never_a_picture() {
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
    // A picture is never a library entry of its own (D-204): ticked or not, it takes no part,
    // so nothing of it is sent.
    let sent = a.task_request_text().unwrap_or_default();
    assert!(sent.contains("Pasta"), "{sent}");
    assert!(!sent.contains("lasagne"), "the picture is not sent: {sent}");
    link_them(&mut a);
    a.task_run();
    wait(&mut a, |a| a.task.as_ref().is_some_and(|s| s.planned));
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files[0].title.as_deref(), Some("Pasta"));
    assert_eq!(s.files[0].sections, ["Food", "Dinner"]);
    assert_eq!(
        s.files[1].reason_for(TaskKind::File).as_deref(),
        Some(crate::task_ui::PICTURE_NOT_FILED)
    );
    a.task_file_write();
    let group = dir.join("library").join("Recipes.md");
    let body = std::fs::read_to_string(&group).expect("the group file");
    assert!(body.contains("## Food"), "{body}");
    assert!(body.contains("### Dinner"), "{body}");
    assert!(body.contains("[Pasta]"), "{body}");
    assert!(
        !body.contains("lasagne"),
        "the picture is not filed: {body}"
    );
    let lib = mdshared_app::library::parse(&body, &dir.join("library"));
    assert_eq!(lib.entries.len(), 1, "{body}");
    assert!(crate::library_pane::same_note(&lib.entries[0].path, &text));
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
    // Then (the hand-driven run, finding 2) Escape says once what closing drops, and closes.
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task.as_ref().expect("open").esc_warned);
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task.is_none(), "then Escape closes it");
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

/// The shot `task-files-plan` shows the plan face as an answer leaves it: the file the model did
/// not place is unticked under "NOT PLACED", never ticked beside a button that leaves it out.
#[test]
fn the_plan_shot_leaves_the_file_not_placed_unticked_as_an_answer_does() {
    let dir = scratch("task-demo-plan");
    let text = pasta(&dir);
    let pdf = dir.join("menu.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n").expect("pdf");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.task_open(TaskKind::File, vec![text, pdf]);
    let s = a.task.as_mut().expect("the window is open");
    assert_eq!(s.files[1].kind, FileKind::Other);
    assert!(
        s.files[1].ticked,
        "File them links a PDF by name: ticked to start"
    );
    super::task::demo_plan(s);
    assert!(s.is_placed(0) && s.files[0].ticked, "the placed file");
    assert!(!s.is_placed(1), "the PDF is not placed");
    assert!(!s.files[1].ticked, "and is left out, unticked");
    assert_eq!(s.to_write(), vec![0]);
}

#[test]
fn a_rewrites_record_is_listed_shown_and_not_a_later_versions() {
    let dir = scratch("task-rewrite-listed");
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
    let root = dir.join("library");
    let runs = runs::list(&root);
    let path = runs[0].path.clone();
    a.open_runs();
    assert_eq!(
        a.ui.runs.list.len(),
        1,
        "the rewrite is a run this build lists"
    );
    assert!(a.ui.runs.unread.is_empty(), "not a later version's record");
    assert_eq!(a.ui.runs.list[0].record.kind, runs::Kind::Rewrite);
    // Show the notes opens the file the run wrote.
    a.run_show(&path);
    assert_eq!(
        a.docs.active_doc().and_then(|d| d.path.clone()),
        Some(text),
        "the rewritten file is open"
    );
}

/// D-207: "AI task on this file…" is in a tab's menu, where the pointer names the file, and
/// opens the window with that tab's file, which need not be the one in front.
#[test]
fn task_on_this_file_from_a_tab_opens_the_window_with_that_tab_s_file() {
    use crate::commands::CommandId as C;
    use crate::ui::Act;
    let dir = scratch("task-this-file");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let other = dir.join("bread.md");
    std::fs::write(&other, "# Bread\n\nBake it.\n").expect("bread");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.open(&text);
    a.open(&other);
    let i = a
        .docs
        .tabs
        .iter()
        .position(|d| d.path.as_deref() == Some(text.as_path()))
        .expect("pasta's tab");
    assert_ne!(a.docs.active(), Some(i), "pasta is not the tab in front");
    let menu = crate::overlays::tab_menu(&a.docs, i, 0.0, 0.0);
    assert!(
        menu.items
            .iter()
            .any(|it| it.id == Act::TabCmd(C::AiTaskThisFile, i) && it.enabled),
        "the tab's menu has it"
    );
    a.tab_command(C::AiTaskThisFile, i);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.task, TaskKind::File, "it starts on File them");
    assert_eq!(s.files.len(), 1);
    assert_eq!(s.files[0].path, text);
}

/// D-207: one command, "AI task…". The palette and the menus no longer list "AI task on this
/// file…", which read as a duplicate of it.
#[test]
fn there_is_one_ai_task_command_in_the_palette_and_the_menus() {
    use crate::commands::CommandId as C;
    use crate::ui::Act;
    let dir = scratch("task-one-command");
    let text = pasta(&dir);
    let mut a = app();
    a.open(&text);
    assert!(crate::palette::listed(C::AiTaskFiles, &a.docs));
    assert!(!crate::palette::listed(C::AiTaskThisFile, &a.docs));
    for menu in [
        crate::overlays::ai_menu(&a.docs, 0.0, 0.0, false),
        crate::overlays::library_menu(true, 0.0, 0.0, false),
    ] {
        assert!(
            !menu
                .items
                .iter()
                .any(|it| it.id == Act::Cmd(C::AiTaskThisFile)),
            "not in a menu bar menu"
        );
        let item = menu
            .items
            .iter()
            .find(|it| it.id == Act::Cmd(C::AiTaskFiles))
            .expect("AI task… is there");
        assert_eq!(item.label, "AI tas&k\u{2026}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// D-207: "Add…" opens a menu of the open tabs' files, the one in front first and said so,
/// those already in the window left out, then "Files…"; choosing a tab adds its file.
#[test]
fn the_add_menu_offers_the_tab_in_front_first_and_leaves_out_what_is_added() {
    use crate::task_ui::AddItem;
    let dir = scratch("task-add-menu");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let one = pasta(&dir);
    let two = dir.join("bread.md");
    std::fs::write(&two, "# Bread\n\nBake it.\n").expect("bread");
    let three = dir.join("soup.md");
    std::fs::write(&three, "# Soup\n\nStir it.\n").expect("soup");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.open(&one);
    a.open(&two);
    a.open(&three);
    a.command(crate::commands::CommandId::NewFile);
    // The untitled tab is in front now; bring soup back to the front.
    let soup = a
        .docs
        .tabs
        .iter()
        .position(|d| d.path.as_deref() == Some(three.as_path()))
        .expect("soup's tab");
    a.docs.activate(soup);
    a.task_open(TaskKind::Rewrite, vec![two.clone()]);
    a.task_act(Hit::AddFiles);
    let s = a.task.as_ref().expect("the window is open");
    assert!(s.add_open, "the menu is open");
    let labels: Vec<String> = s.add_items.iter().map(AddItem::label).collect();
    assert_eq!(
        labels,
        vec![
            "soup.md \u{b7} in front".to_string(),
            "pasta.md".to_string(),
            "Files\u{2026}".to_string(),
        ],
        "the tab in front first; bread is in the window already; the untitled tab has no file"
    );
    a.task_act(Hit::AddItem(1));
    let s = a.task.as_ref().expect("the window is open");
    assert!(!s.add_open, "choosing closes the menu");
    let paths: Vec<&Path> = s.files.iter().map(|f| f.path.as_path()).collect();
    assert_eq!(paths, vec![two.as_path(), one.as_path()]);
    // Escape closes the menu and leaves the window open.
    a.task_act(Hit::AddFiles);
    escape(&mut a);
    let s = a.task.as_ref().expect("the window stays open");
    assert!(!s.add_open);
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- audit 4 ----------------------------------------------------------------------------------

fn instruct(a: &mut App, text: &str) {
    if let Some(s) = a.task.as_mut() {
        s.instruction.set_text(text);
    }
}

fn escape(a: &mut App) {
    a.key_pressed(crate::keys::Key::Name(KeyName::Escape), None);
}

/// What the window said, or a toast when it is closed: a message about what was done in the
/// window is said in it while it is open (the hand-driven run, finding 7).
fn said(a: &App) -> String {
    a.task
        .as_ref()
        .and_then(|s| s.notice.clone())
        .unwrap_or_else(|| a.docs.toast(Instant::now()).unwrap_or_default().to_string())
}

/// Audit 4, B3: Discard leaves the file as it was and the run asks for the next file, not for
/// the same one again. (Copy result goes the same way, through `task_review_left`; it is not
/// run here because it takes the clipboard.)
#[test]
fn discard_leaves_the_file_and_the_run_asks_for_the_next() {
    let (mut a, dir) = ai_open("task-discard", TaskKind::Rewrite, Vec::new());
    let one = pasta(&dir);
    let two = dir.join("bread.md");
    std::fs::write(&two, "# Bread\n\nBake it.\n").expect("bread");
    a.task_add_paths(vec![one.clone(), two]);
    instruct(&mut a, "Tidy them");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    a.task_review_discard();
    assert!(a.ai.panel.is_none());
    let s = a.task.as_ref().expect("the window stays");
    assert!(!s.files[0].ticked && !s.files[0].done, "left as it was");
    assert_eq!(
        s.asking.as_ref().map(|q| q.index),
        Some(1),
        "the request out is for the second file"
    );
    assert_eq!(a.task_sent.requests, 2);
    assert_eq!(
        std::fs::read_to_string(&one).expect("pasta"),
        "# Pasta\n\nBoil it.\n"
    );
}

/// Audit 4, B4: a review kept from a window since closed, applied from another window, writes
/// its own file under a record of its own. The other window's files are not touched and nothing
/// is sent from it.
#[test]
fn a_review_kept_from_a_closed_window_writes_its_file_and_sends_nothing() {
    let (mut a, dir) = ai_open("task-kept-other", TaskKind::Rewrite, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    escape(&mut a);
    assert!(
        a.ai.panel.is_none() && a.ai.kept.is_some(),
        "the review is kept"
    );
    a.task_close();
    let b = dir.join("b.md");
    let c = dir.join("c.md");
    std::fs::write(&b, "# B\n\nprivate b\n").expect("b");
    std::fs::write(&c, "# C\n\nprivate c\n").expect("c");
    a.task_open(TaskKind::File, vec![b, c]);
    a.ai_show_kept();
    assert!(matches!(a.ai.panel, Some(Panel::Review(_))));
    a.task_review_apply();
    assert_eq!(std::fs::read_to_string(&text).expect("written"), "# Done\n");
    assert!(a.ai.panel.is_none());
    let s = a.task.as_ref().expect("the File them window stays");
    assert_eq!(s.task, TaskKind::File);
    assert!(
        s.files.iter().all(|f| f.ticked && !f.done),
        "its files are as they were"
    );
    assert!(s.asking.is_none(), "nothing is sent from it");
    assert_eq!(a.task_sent.requests, 0);
    let runs = runs::list(&dir.join("library"));
    assert_eq!(runs.len(), 1, "a record of its own");
    assert_eq!(runs[0].record.originals.len(), 1);
    assert_eq!(runs[0].record.state, runs::State::Done);
    assert!(
        a.task_record.is_none(),
        "the window's record is not that one"
    );
}

/// Audit 4, B5: while a request is out, a click on the task switch is refused; and the answer
/// is read as the task that asked, whatever the window shows.
#[test]
fn a_request_out_keeps_its_task_and_its_answer_is_read_as_that_task() {
    let (mut a, dir) = ai_open("task-switch", TaskKind::File, Vec::new());
    let picture = dir.join("lasagne.jpg");
    std::fs::write(&picture, [0xFF, 0xD8, 0xFF, 0xE0]).expect("picture");
    a.task_add_paths(vec![pasta(&dir), picture]);
    with_group(&mut a, "Recipes");
    instruct(&mut a, "TASKFILE: by dish");
    if let Some(s) = a.task.as_mut() {
        s.files[1].ticked = true;
    }
    a.task_run();
    assert!(a.task.as_ref().expect("open").asking.is_some());
    a.compose();
    let at = a.task.as_ref().and_then(|s| {
        s.hits
            .iter()
            .find(|(h, _)| *h == Hit::Kind(TaskKind::Rewrite))
            .map(|(_, r)| (r.x + r.w / 2.0, r.y + r.h / 2.0))
    });
    let (x, y) = at.expect("the switch is drawn");
    a.ui.mouse = (x, y);
    a.task_press();
    a.task_release();
    assert_eq!(
        a.task.as_ref().expect("open").task,
        TaskKind::File,
        "the click is refused"
    );
    // A window that shows another task still reads the filing it asked for as a filing.
    if let Some(s) = a.task.as_mut() {
        s.task = TaskKind::Rewrite;
    }
    // Waited for until the answer is taken, whatever it was read as, so a wrong reading fails at
    // its own assertion and not by the wait running out.
    wait(&mut a, |a| {
        a.task.as_ref().is_some_and(|s| s.asking.is_none())
    });
    assert!(a.ai.panel.is_none(), "no review of the filing's JSON");
    let s = a.task.as_ref().expect("open");
    assert!(s.planned, "the filing is planned");
    assert!(s.asking.is_none());
    assert_eq!(s.files[0].title.as_deref(), Some("Pasta"));
}

/// Audit 4, B7: a Rewrite window parked by "Show what is sent" comes back as Rewrite them,
/// whichever command brings it back.
#[test]
fn a_parked_rewrite_window_comes_back_as_rewrite_them() {
    let (mut a, dir) = ai_open("task-park-rewrite", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "Tidy it");
    a.task_show_sent();
    assert!(a.task_parked.is_some());
    a.task_open(TaskKind::File, Vec::new());
    let s = a.task.as_ref().expect("back");
    assert_eq!(s.task, TaskKind::Rewrite);
    assert_eq!(s.instruction.text(), "Tidy it");
}

/// Audit 4, B10: Run on the window shows the review Escape kept, as the command that would
/// cannot be reached while the window is open; and Run again on a file's review whose window is
/// closed keeps the review and says why.
#[test]
fn run_shows_the_kept_review_and_run_again_without_its_window_keeps_it() {
    let (mut a, dir) = ai_open("task-kept-run", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    escape(&mut a);
    assert!(a.ai.panel.is_none() && a.ai.kept.is_some());
    a.task_run();
    assert!(
        matches!(a.ai.panel, Some(Panel::Review(_))),
        "Run shows the kept review"
    );
    assert!(a.ai.kept.is_none());
    assert_eq!(a.task_sent.requests, 1, "and sends nothing more");
    escape(&mut a);
    a.task_close();
    a.ai_show_kept();
    a.task_review_again();
    assert!(
        matches!(a.ai.panel, Some(Panel::Review(_))),
        "the review stays, with its result"
    );
    assert!(said(&a).contains("is closed"), "{}", said(&a));
}

/// Audit 4, B12: the file is read again when its request is made, so an edit made after it was
/// added is what is sent and its review can be applied; a file that is no longer UTF-8 text is
/// not sent at all.
#[test]
fn a_file_edited_after_it_was_added_is_sent_as_it_is_now() {
    let (mut a, dir) = ai_open("task-fresh", TaskKind::Rewrite, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    std::fs::write(&text, "# Pasta\n\nBoil it well.\n").expect("edited");
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    let Some(Panel::Review(rv)) = &a.ai.panel else {
        panic!("a review");
    };
    assert_eq!(rv.original, "# Pasta\n\nBoil it well.\n");
    assert!(rv.blocked.is_none(), "{:?}", rv.blocked);
    a.task_review_apply();
    assert_eq!(std::fs::read_to_string(&text).expect("written"), "# Done\n");
    // Another window, on a file that stops being UTF-8 after it was added.
    let other = dir.join("other.md");
    std::fs::write(&other, "# Other\n\nete\n").expect("other");
    a.task_open(TaskKind::Rewrite, vec![other.clone()]);
    std::fs::write(&other, b"# Other\n\n\xE9t\xE9\n").expect("Latin-1");
    instruct(&mut a, "Tidy it");
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert!(s.asking.is_none(), "nothing is sent");
    assert_eq!(a.task_sent.requests, 0);
    assert_eq!(
        s.blocked.as_deref(),
        Some("other.md: not UTF-8: save it as UTF-8 first. Nothing was sent.")
    );
}

/// Audit 4, B14: Rewrite them with nothing in the instruction box sends nothing and says why.
#[test]
fn rewrite_them_with_no_instruction_sends_nothing_and_says_why() {
    let (mut a, dir) = ai_open("task-no-instruction", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "   ");
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert!(s.asking.is_none());
    assert_eq!(
        s.blocked.as_deref(),
        Some(crate::task_ui::REWRITE_NEEDS_INSTRUCTION)
    );
    assert_eq!(a.task_sent.requests, 0);
}

/// Audit 4, B2: a run whose originals pass what one record holds goes on in a second record, so
/// every record can be read back and undone; the end of the run counts the files of both.
#[test]
fn a_rewrite_run_past_what_one_record_holds_goes_on_in_a_second_record() {
    let dir = scratch("task-record-full");
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).expect("library");
    // A quote is two bytes in a record, so each original takes twice its size there.
    let line = format!("{}\n", "\"".repeat(99));
    let body = |name: &str| format!("# {name}\n\n{}", line.repeat(1900));
    let file_of = |path: &Path, before: String| runs::RewriteFile {
        path: path.to_path_buf(),
        before,
        after: "# Done\n".to_string(),
        bom: false,
    };
    let last = dir.join("last.md");
    std::fs::write(&last, body("Last")).expect("last");
    // The window's record is filled with other files' originals, until this one would not fit.
    let first = dir.join("f0.md");
    std::fs::write(&first, body("F0")).expect("f0");
    let (record_path, mut record) =
        runs::start_rewrite(&lib, "Tidy it", None, &first).expect("a record");
    let mut fillers = 0;
    while runs::rewrite_fits(&record, &lib, &file_of(&last, body("Last"))) {
        let p = dir.join(format!("f{fillers}.md"));
        std::fs::write(&p, body("F")).expect("filler");
        runs::rewrite_into(&record_path, &mut record, &lib, &file_of(&p, body("F")))
            .expect("a filler is rewritten");
        fillers += 1;
    }
    assert!(fillers > 1, "{fillers}");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.task_open(TaskKind::Rewrite, vec![last.clone()]);
    a.task_record = Some((record_path, record));
    instruct(&mut a, "Tidy it");
    if let Some(s) = a.task.as_mut() {
        assert!(s.files[0].text.is_some(), "the file can be rewritten");
        s.asking = Some(crate::task_ui::Asking {
            request: 0,
            task: TaskKind::Rewrite,
            what: String::new(),
            chars: 0,
            index: 0,
            prompt: String::new(),
            batch: vec![0],
        });
    }
    a.task_done("# Done\n");
    assert!(matches!(a.ai.panel, Some(Panel::Review(_))));
    a.task_review_apply();
    // Compared as a yes or no: the file is 380 KB of quotes, too long to print when it fails.
    let written = std::fs::read_to_string(&last).expect("written") == "# Done\n";
    assert!(
        written,
        "not written: {:?}",
        a.ai.review().map(|rv| rv.blocked.clone())
    );
    assert!(a.task.is_none(), "the run is over");
    let listed = runs::list(&lib);
    assert_eq!(listed.len(), 2, "two records, each read back");
    let originals: usize = listed.iter().map(|r| r.record.originals.len()).sum();
    assert_eq!(originals, fillers + 1);
    assert!(listed.iter().all(|r| r.record.state == runs::State::Done));
    assert_eq!(
        said(&a),
        format!("{} files rewritten. Undo is in AI runs.", fillers + 1)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, B6: a file that could not be written leaves no record behind when the run ends,
/// and nothing beside the file.
#[test]
fn a_rewrite_that_could_not_be_written_leaves_no_record_when_the_run_ends() {
    let (mut a, dir) = ai_open("task-read-only", TaskKind::Rewrite, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    let mut perms = std::fs::metadata(&text).expect("meta").permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&text, perms.clone()).expect("read-only");
    a.task_review_apply();
    let blocked = a.ai.review().and_then(|rv| rv.blocked.clone());
    assert!(blocked.is_some(), "the review says why");
    assert_eq!(
        std::fs::read_to_string(&text).expect("unchanged"),
        "# Pasta\n\nBoil it.\n"
    );
    a.task_review_discard();
    assert!(a.task.is_none(), "the run is over");
    assert!(
        runs::list(&dir.join("library")).is_empty(),
        "no record of a file that was not rewritten"
    );
    let left: Vec<String> = std::fs::read_dir(&dir)
        .expect("the folder")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp") || n.ends_with(".part"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    let _ = std::fs::set_permissions(&text, perms);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, RB3: "Add N to the library" writes the group's file through a temporary of its own.
/// A file of the owner's called `Recipes.md.tmp` beside it is not truncated or renamed away, and
/// nothing else is left in the library folder.
#[test]
fn file_them_leaves_a_tmp_file_of_the_owners_beside_the_group_file_alone() {
    let (mut a, dir) = open("task-tmp", TaskKind::File, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    with_group(&mut a, "Recipes");
    let library = dir.join("library");
    let group = library.join("Recipes.md");
    assert!(group.is_file(), "the group's file is made");
    let theirs = library.join("Recipes.md.tmp");
    std::fs::write(&theirs, "the owner's own notes\n").expect("theirs");
    link_them(&mut a);
    a.task_run();
    assert!(a.task.as_ref().expect("open").planned);
    a.task_file_write();
    assert!(a.task.is_none(), "the links are in");
    let body = std::fs::read_to_string(&group).expect("the group's file");
    assert!(body.contains("[Pasta]"), "{body}");
    assert_eq!(
        std::fs::read_to_string(&theirs).expect("still there"),
        "the owner's own notes\n"
    );
    let mut names: Vec<String> = std::fs::read_dir(&library)
        .expect("the library")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !n.ends_with(".json") && n != ".runs")
        .collect();
    names.sort();
    assert_eq!(names, ["Recipes.md", "Recipes.md.tmp"]);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A trip of eleven lines: the stand-in engine's split answer (lines 5 to 11) fits it.
fn trip(dir: &Path, name: &str, city: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(
        &p,
        format!("# {city}\n\nintro\n\n## Old town\n\nthe lanes\n\n## Hills\n\nthe view\n"),
    )
    .expect("trip");
    p
}

/// The plan screen in front was cut by the AI.
fn cut_by_ai(a: &App) -> bool {
    a.plan.as_ref().expect("a plan screen").ai_cut.is_some()
}

/// Audit 4, B11: in Split them, a split asked on one file's card and still out when that plan is
/// written stops there. The next file's card opens with its own plan, and the first file's answer
/// never becomes it.
#[test]
fn a_split_out_when_a_files_plan_is_written_never_reaches_the_next_files_plan() {
    let (mut a, dir) = ai_open("task-split-out", TaskKind::Split, Vec::new());
    let kyoto = trip(&dir, "kyoto.md", "Kyoto");
    let nara = trip(&dir, "nara.md", "Nara");
    a.task_add_paths(vec![kyoto, nara]);
    with_group(&mut a, "Trips");
    a.task_run();
    assert!(a.plan.is_some(), "kyoto.md's plan");
    assert!(a.ai.split.is_none(), "an empty box asks nothing");
    // The card's own "Ask the AI…", then Write before the answer is taken.
    a.split_start(crate::ai::Spec {
        title: "Split".into(),
        instruction: "SPLITOK one note".into(),
    });
    assert!(a.ai.split.is_some(), "the split is out");
    a.plan_write();
    let screen = a.plan.as_ref().expect("nara.md's plan opens");
    assert_eq!(screen.source.name, "nara.md");
    assert!(
        a.ai.split.is_none(),
        "the split for kyoto.md stopped with its plan"
    );
    assert!(a.docs.ai_status.is_none(), "{:?}", a.docs.ai_status);
    // Whatever the engine still sends is not read.
    let until = Instant::now() + Duration::from_millis(400);
    while Instant::now() < until {
        a.ai_wake();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!cut_by_ai(&a), "nara.md's plan is its own");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, B11: a split's answer goes only to the plan screen it was asked for. A plan written
/// while its split was out, and another document's plan opened after it, takes nothing of the
/// answer; the same answer on the screen that asked becomes its plan.
#[test]
fn a_split_answer_is_read_only_on_the_plan_it_was_asked_for() {
    engine();
    let dir = scratch("task-split-tag");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    let kyoto = trip(&dir, "kyoto.md", "Kyoto");
    let nara = trip(&dir, "nara.md", "Nara");
    super::files_tests::open_now(&mut a, &kyoto);
    assert!(a.plan_open_active());
    let spec = crate::ai::Spec {
        title: "Split".into(),
        instruction: "SPLITOK one note".into(),
    };
    a.split_start(spec.clone());
    assert!(a.ai.split.is_some(), "the split is out");
    a.plan_write();
    assert!(a.plan.is_none(), "kyoto.md's plan is written");
    super::files_tests::open_now(&mut a, &nara);
    assert!(a.plan_open_active(), "nara.md's plan");
    wait(&mut a, |a| a.ai.split.is_none());
    assert!(!cut_by_ai(&a), "kyoto.md's answer is not nara.md's plan");
    assert_eq!(
        a.plan.as_ref().expect("open").plan.notes().len(),
        2,
        "nara.md is cut at its headings"
    );
    // Asked here, the same answer is this plan's.
    a.split_start(spec);
    wait(&mut a, |a| a.ai.split.is_none());
    assert!(cut_by_ai(&a), "the answer to nara.md's own request");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of the audit 4 fixes, T2: the guard itself, `split_asked_here`. Every way a plan
/// leaves the screen stops its split first, so no window path reaches it today; here the split
/// asked for one document is kept out of the window's reach while another document's plan opens,
/// then answered. That plan takes nothing of it: not the answer, not the failure.
#[test]
fn a_split_asked_for_another_document_gives_this_plan_nothing() {
    engine();
    let dir = scratch("task-split-guard");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    for (tag, instruction) in [("answer", "SPLITOK one note"), ("failure", "FAILPLEASE")] {
        let kyoto = trip(&dir, &format!("kyoto-{tag}.md"), "Kyoto");
        let nara = trip(&dir, &format!("nara-{tag}.md"), "Nara");
        super::files_tests::open_now(&mut a, &kyoto);
        assert!(a.plan_open_active(), "kyoto's plan ({tag})");
        a.split_start(crate::ai::Spec {
            title: "Split".into(),
            instruction: instruction.into(),
        });
        let run = a.ai.split.take().expect("the split is out");
        a.plan = None;
        super::files_tests::open_now(&mut a, &nara);
        assert!(a.plan_open_active(), "nara's plan ({tag})");
        a.ai.split = Some(run);
        wait(&mut a, |a| a.ai.split.is_none());
        let screen = a.plan.as_ref().expect("nara's plan stays");
        assert_eq!(screen.source.name, format!("nara-{tag}.md"));
        assert!(screen.ai_cut.is_none(), "kyoto's {tag} is not nara's plan");
        assert!(
            screen.ai_notice.is_none(),
            "kyoto's {tag} is not said on nara's plan"
        );
        assert_eq!(screen.plan.notes().len(), 2, "nara is cut at its headings");
        a.plan = None;
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A File them window over `group`, an existing library file, with pasta.md placed in it.
fn file_them_into(tag: &str, group_text: &[u8]) -> (App, PathBuf, PathBuf) {
    let dir = scratch(tag);
    let lib = dir.join("library");
    std::fs::create_dir_all(&lib).expect("library");
    let group = lib.join("Recipes.md");
    std::fs::write(&group, group_text).expect("group");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    (a, dir, group)
}

fn file_pasta(a: &mut App, dir: &Path) {
    a.task_open(TaskKind::File, Vec::new());
    a.task_add_paths(vec![pasta(dir)]);
    if let Some(s) = a.task.as_mut() {
        // The UTF-16 group is named "Recipes é".
        let at = s.groups.iter().position(|g| g.name.starts_with("Recipes"));
        s.dest = DestChoice::Group(at.expect("the group is offered"));
    }
    a.task_run();
    assert_eq!(a.task.as_ref().map(|s| s.planned), Some(true));
    a.task_file_write();
}

/// Review of the audit 4 fixes, W1: "File them" is refused while the group's library file is
/// open in a tab with changes not saved, as every library write is. The file is not written,
/// the window stays for the owner to save and add again, and the toast says why.
#[test]
fn file_them_is_refused_while_the_group_file_has_unsaved_changes() {
    let before = b"# Recipes\nhome: Recipes\n";
    let (mut a, dir, group) = file_them_into("task-file-unsaved", before);
    super::files_tests::open_now(&mut a, &group);
    super::files_tests::type_text(&mut a, "x");
    assert!(a.docs.active_doc().is_some_and(|d| d.modified()));
    file_pasta(&mut a, &dir);
    assert_eq!(std::fs::read(&group).expect("the group"), before);
    assert!(a.task.is_some(), "the window stays");
    let said = said(&a);
    assert!(
        said.contains("Recipes.md") && said.contains("not saved"),
        "{said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// W1, the reading: a group file with a byte order mark keeps it, and one in UTF-16 is refused
/// and left as it was, as the pane's own writers treat them.
#[test]
fn file_them_keeps_a_byte_order_mark_and_refuses_a_utf16_file() {
    let (mut a, dir, group) = file_them_into(
        "task-file-bom",
        "\u{FEFF}# Recipes\nhome: Recipes\n".as_bytes(),
    );
    file_pasta(&mut a, &dir);
    let bytes = std::fs::read(&group).expect("the group");
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "{bytes:?}");
    assert!(
        !bytes[3..].starts_with(&[0xEF, 0xBB, 0xBF]),
        "one mark, not two"
    );
    assert!(
        String::from_utf8_lossy(&bytes).contains("[Pasta]"),
        "{bytes:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);

    let mut utf16 = vec![0xFF, 0xFE];
    for u in "# Recipes \u{e9}\nhome: Recipes\n".encode_utf16() {
        utf16.extend_from_slice(&u.to_le_bytes());
    }
    let (mut a, dir, group) = file_them_into("task-file-utf16", &utf16);
    file_pasta(&mut a, &dir);
    assert_eq!(std::fs::read(&group).expect("the group"), utf16);
    let said = said(&a);
    // Refused as every library writer refuses it, in its words.
    assert!(said.contains("is not UTF-8 text"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// File them leaves a record of kind links, and "AI runs…" undoes it: the group's file back to
/// the byte, and the files it linked untouched (the pictures plan, section 14).
#[test]
fn file_them_leaves_a_record_that_undoes_its_links() {
    let (mut a, dir) = open("task-record", TaskKind::File, Vec::new());
    let text = pasta(&dir);
    let pdf = dir.join("menu.pdf");
    std::fs::write(&pdf, "%PDF-1.4\n").expect("pdf");
    a.task_add_paths(vec![text.clone(), pdf.clone()]);
    with_group(&mut a, "Recipes");
    let group = dir.join("library").join("Recipes.md");
    let before = std::fs::read_to_string(&group).expect("the group's file");
    link_them(&mut a);
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the links are in");
    let body = std::fs::read_to_string(&group).expect("the group's file");
    assert!(
        body.contains("[Pasta]") && body.contains("[menu]"),
        "{body}"
    );
    let library = a.library_dir().expect("the library");
    let (list, _) = mdedit_core::runs::list_all(&library);
    assert_eq!(list.len(), 1, "one record");
    let record = &list[0];
    assert_eq!(record.record.kind, mdedit_core::runs::Kind::Links);
    assert_eq!(record.record.notes.len(), 2);
    assert!(record.record.notes.iter().all(|n| !n.created));
    a.run_undo(record.path.clone());
    assert_eq!(
        std::fs::read_to_string(&group).expect("the group's file"),
        before,
        "the group's file is given back to the byte"
    );
    assert_eq!(
        std::fs::read_to_string(&text).expect("pasta"),
        "# Pasta\n\nBoil it.\n"
    );
    assert_eq!(std::fs::read_to_string(&pdf).expect("pdf"), "%PDF-1.4\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Pictures plan 4.4: a rewrite applied to a file open in a tab shows the new text there at
/// once, with nothing to save.
#[test]
fn a_rewrite_applied_reloads_the_tab_that_holds_the_file() {
    let dir = scratch("task-rewrite-tab");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let text = pasta(&dir);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    let tab = super::files_tests::open_now(&mut a, &text);
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
    let shown = |a: &App| {
        a.docs.tabs[tab]
            .ready()
            .map(|r| r.buffer.rope().to_string())
            .unwrap_or_default()
    };
    let until = Instant::now() + std::time::Duration::from_secs(10);
    while shown(&a) != "# Done\n" && Instant::now() < until {
        a.docs.wait_load(until, None, false);
    }
    assert_eq!(shown(&a), "# Done\n", "the tab shows the new text");
    assert!(!a.docs.tabs[tab].has_changes(), "nothing to save");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Pictures plan 4.4: after File them the Notes pane shows the group, its cursor on the first
/// line the run added; no tab opens, since the files were not changed.
#[test]
fn file_them_puts_the_notes_pane_on_the_first_new_line() {
    let (mut a, dir) = open("task-file-pane", TaskKind::File, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    with_group(&mut a, "Recipes");
    let tabs = a.docs.tabs.len();
    link_them(&mut a);
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the links are in");
    assert!(a.ui.panes.files && a.ui.library.on, "the Notes pane shows");
    let cur = a.ui.library.cur.expect("a row has the cursor");
    match &a.ui.library.rows[cur].kind {
        crate::library_pane::Kind::Note { path, .. } => {
            assert!(mdshared_app::paths::same_path(path, &text), "{path:?}")
        }
        other => panic!("the cursor is on {other:?}"),
    }
    assert_eq!(a.docs.tabs.len(), tabs, "no tab opened");
    let _ = std::fs::remove_dir_all(&dir);
}

/// D-203: the window's limits are the settings': files in a rewrite run, the largest file a
/// rewrite sends, and files in one filing request.
#[test]
fn the_window_takes_its_limits_from_the_settings() {
    let dir = scratch("task-limits");
    std::fs::create_dir_all(dir.join("library")).expect("library");
    let mut paths = Vec::new();
    for i in 0..5 {
        let p = dir.join(format!("n{i}.md"));
        std::fs::write(&p, format!("# N{i}\n")).expect("note");
        paths.push(p);
    }
    let big = dir.join("big.md");
    std::fs::write(&big, "x".repeat(12_000)).expect("big");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.docs.prefs.ai_rewrite_files = 3;
    a.docs.prefs.ai_rewrite_kb = 10;
    a.docs.prefs.ai_task_batch = 2;
    let mut all = paths.clone();
    all.push(big.clone());
    a.task_open(TaskKind::Rewrite, all);
    let s = a.task.as_ref().expect("the window is open");
    let ticked = s
        .files
        .iter()
        .filter(|f| f.takes_part(TaskKind::Rewrite))
        .count();
    assert_eq!(ticked, 3, "three files a run");
    assert_eq!(
        s.files[3].reason_for(TaskKind::Rewrite).as_deref(),
        Some("only 3 files a run")
    );
    let big_row = s.files.iter().find(|f| f.path == big).expect("big's row");
    assert_eq!(
        big_row.reason_for(TaskKind::Rewrite).as_deref(),
        Some("too long for one request: 12 KB of 10 KB")
    );
    // File them: six files, two to a request.
    a.task_act(Hit::Kind(TaskKind::File));
    let s = a.task.as_mut().expect("the window is open");
    s.ai_on = true;
    assert!(s.summary().contains("in 3 requests"), "{}", s.summary());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- THE FILES: Copy them in (pictures plan 4.3, phase C1, D-209) ------------------------------

/// A Markdown file outside the library, with a picture it shows from its own folder.
fn trip_with_picture(dir: &Path) -> PathBuf {
    let desk = dir.join("desk");
    std::fs::create_dir_all(desk.join("img")).expect("desk");
    let md = desk.join("trip.md");
    std::fs::write(&md, "# Trip\n\n![Beach](img/beach.png)\n").expect("trip");
    std::fs::write(desk.join("img").join("beach.png"), b"\x89PNG beach").expect("beach");
    md
}

/// By default File them copies the files in: the line leads to the copy in the group's folder,
/// the picture it shows goes beside it, the original is left as it was, and the run is a run of
/// copies that AI runs… can undo.
#[test]
fn file_them_copies_the_files_in_by_default_and_the_line_leads_to_the_copy() {
    let (mut a, dir) = open("task-copy", TaskKind::File, Vec::new());
    assert_eq!(
        a.task.as_ref().map(|s| s.files_go),
        Some(mdedit_core::prefs::FilesGo::Copy)
    );
    let md = trip_with_picture(&dir);
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the copies and the line are in");
    let library = dir.join("library");
    let body = std::fs::read_to_string(library.join("Trips.md")).expect("the group file");
    let lib = mdshared_app::library::parse(&body, &library);
    assert_eq!(lib.entries.len(), 1, "{body}");
    let copy = lib.entries[0].path.clone();
    assert!(
        copy.starts_with(&library) && !crate::library_pane::same_note(&copy, &md),
        "the line leads to the copy: {body}"
    );
    assert_eq!(
        std::fs::read_to_string(&copy).expect("the copy"),
        "# Trip\n\n![Beach](img/beach.png)\n"
    );
    let beside = copy
        .parent()
        .expect("its folder")
        .join("img")
        .join("beach.png");
    assert_eq!(
        std::fs::read(&beside).expect("the picture"),
        b"\x89PNG beach"
    );
    assert_eq!(
        std::fs::read_to_string(&md).expect("the original"),
        "# Trip\n\n![Beach](img/beach.png)\n"
    );
    let runs: Vec<String> = std::fs::read_dir(library.join(".runs"))
        .expect("the record")
        .filter_map(|e| e.ok())
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .collect();
    assert!(
        runs.iter().any(|r| r.contains("\"kind\": \"copies\"")),
        "{runs:?}"
    );
    assert!(
        said(&a).contains("copied in with 1 picture"),
        "{}",
        said(&a)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// THE FILES is remembered once changed, and a window opened later starts on it.
#[test]
fn the_files_switch_is_remembered() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-files-go", TaskKind::File, Vec::new());
    a.task_act(Hit::FilesGo(FilesGo::Link));
    assert_eq!(a.task.as_ref().map(|s| s.files_go), Some(FilesGo::Link));
    assert_eq!(a.docs.prefs.ai_files, FilesGo::Link);
    a.task_close();
    a.task_open(TaskKind::File, Vec::new());
    assert_eq!(a.task.as_ref().map(|s| s.files_go), Some(FilesGo::Link));
    let _ = std::fs::remove_dir_all(&dir);
}

/// On the plan face a row says what THE FILES does to it: copied in, or linked where it is when
/// it is inside the library's folder already; nothing when the files are linked.
#[test]
fn a_row_says_whether_its_file_is_copied_in_or_linked_where_it_is() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-copy-note", TaskKind::File, Vec::new());
    let outside = pasta(&dir);
    let inside = dir.join("library").join("Old").join("kept.md");
    std::fs::create_dir_all(inside.parent().expect("folder")).expect("folder");
    std::fs::write(&inside, "# Kept\n").expect("kept");
    a.task_add_paths(vec![outside, inside]);
    let s = a.task.as_mut().expect("open");
    assert_eq!(s.copy_note(0), Some("copied in"));
    assert_eq!(
        s.copy_note(1),
        Some("linked where it is: already inside the library's folder")
    );
    s.files_go = FilesGo::Link;
    assert_eq!(s.copy_note(0), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// When the library's line cannot be written, the copies made for it are taken away again.
#[test]
fn a_refused_line_takes_the_copies_away_again() {
    let before = b"# Recipes\nhome: Recipes\n";
    let (mut a, dir, group) = file_them_into("task-copy-refused", before);
    super::files_tests::open_now(&mut a, &group);
    super::files_tests::type_text(&mut a, "x");
    file_pasta(&mut a, &dir);
    assert_eq!(std::fs::read(&group).expect("the group"), before);
    let copy = dir.join("library").join("Recipes").join("pasta.md");
    assert!(!copy.exists(), "the copy is taken away");
    assert!(
        !dir.join("library").join("Recipes").exists(),
        "and its folder"
    );
    assert!(dir.join("pasta.md").is_file(), "the original stays");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of phase E, 5: with more tabs than the card has room for, the Add menu lists as many
/// as fit, the one in front first, and "Files…" is its last line, inside the window.
#[test]
fn the_add_menu_keeps_files_on_screen_with_many_tabs() {
    let (mut a, dir) = open("task-add-many", TaskKind::File, Vec::new());
    for i in 0..40 {
        let p = dir.join(format!("t{i}.txt"));
        std::fs::write(&p, "x\n").expect("a tab's file");
        super::files_tests::open_now(&mut a, &p);
    }
    if let Some(s) = a.task.as_mut() {
        s.win = (1280.0, 800.0);
    }
    a.task_add_menu();
    let s = a.task.as_ref().expect("open");
    assert!(matches!(
        s.add_items.last(),
        Some(crate::task_ui::AddItem::Files)
    ));
    assert!(
        matches!(
            s.add_items.first(),
            Some(crate::task_ui::AddItem::Tab { front: true, .. })
        ),
        "the one in front first"
    );
    let lines = crate::task_ui::add_menu_lines(1280.0, 800.0);
    assert!(lines < 41, "{lines}");
    assert_eq!(s.add_items.len(), lines);
    let card = crate::task_ui::card(1280.0, 800.0);
    let bottom = card.y + 53.0 + 4.0 + crate::task_ui::ADD_ROW_H * lines as f32;
    assert!(bottom <= card.bottom(), "{bottom} {card:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of phase F, 7: "Show the notes" on a run of File them opens its first file as a click
/// on its line does: a PDF with its own program, never as text in a tab.
#[test]
fn show_the_notes_of_a_filing_hands_a_pdf_to_the_system() {
    let (mut a, dir) = open("task-show-pdf", TaskKind::File, Vec::new());
    let pdf = dir.join("menu.pdf");
    std::fs::write(
        &pdf,
        "%PDF-1.4
",
    )
    .expect("pdf");
    a.task_add_paths(vec![pdf.clone()]);
    with_group(&mut a, "Recipes");
    link_them(&mut a);
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the line is in");
    let runs = runs::list(&dir.join("library"));
    let path = runs[0].path.clone();
    let tabs = a.docs.tabs.len();
    let _ = super::library::handed_to_system();
    a.run_show(&path);
    assert_eq!(
        super::library::handed_to_system(),
        vec![("open", pdf.clone())]
    );
    assert_eq!(a.docs.tabs.len(), tabs, "no tab of its bytes");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of phase O, 8: a window parked with a file too long to rewrite, brought back after the
/// largest rewrite was raised, reads the file again: it takes part, not "not UTF-8".
#[test]
fn a_parked_window_reads_a_long_file_again_when_the_limit_is_raised() {
    let (mut a, dir) = ai_open("task-park-limit", TaskKind::Rewrite, Vec::new());
    a.docs.prefs.ai_rewrite_kb = 10;
    if let Some(s) = a.task.as_mut() {
        s.limits = crate::task_ui::Limits::of(&a.docs.prefs);
    }
    let long = dir.join("long.md");
    std::fs::write(&long, "# Long\n\n".to_string() + &"word ".repeat(4000)).expect("long");
    a.task_add_paths(vec![long]);
    let reason = |a: &App| {
        a.task
            .as_ref()
            .and_then(|s| s.files[0].reason_for(TaskKind::Rewrite))
    };
    assert!(
        reason(&a).is_some_and(|r| r.contains("too long")),
        "{:?}",
        reason(&a)
    );
    a.task_parked = a.task.take();
    a.docs.prefs.ai_rewrite_kb = 500;
    a.task_open(TaskKind::Rewrite, Vec::new());
    assert_eq!(reason(&a), None, "it takes part now");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of phase O, 9: "Files per filing request" is what File them sends in each request,
/// and what "Show what is sent" shows: five files at two a request are three requests.
#[test]
fn file_them_sends_as_many_files_a_request_as_the_setting_says() {
    let (mut a, dir) = ai_open("task-batch", TaskKind::File, Vec::new());
    a.docs.prefs.ai_task_batch = 2;
    let mut paths = Vec::new();
    for i in 0..5 {
        let p = dir.join(format!("n{i}.md"));
        std::fs::write(
            &p,
            format!(
                "# N{i}
"
            ),
        )
        .expect("note");
        paths.push(p);
    }
    a.task_close();
    a.task_open(TaskKind::File, paths);
    with_group(&mut a, "Recipes");
    if let Some(s) = a.task.as_mut() {
        s.ai_on = true;
        s.instruction.set_text("TASKFILE: by dish");
    }
    let shown = a.task_request_text().unwrap_or_default();
    assert!(shown.contains("REQUEST 1 of 3"), "{shown}");
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.batch_queue.len(), 3, "{:?}", s.batch_queue);
    assert_eq!(s.batch_queue[0].len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Review of phase O, 12: Split them over two files writes two runs and opens only the first
/// run's note, not one tab per file.
#[test]
fn split_them_over_two_files_opens_one_tab() {
    let (mut a, dir) = open("task-split-tabs", TaskKind::Split, Vec::new());
    let kyoto = trip(&dir, "kyoto.md", "Kyoto");
    let nara = trip(&dir, "nara.md", "Nara");
    a.task_add_paths(vec![kyoto, nara]);
    with_group(&mut a, "Trips");
    // A pristine untitled tab gives its place to the first file opened, so the files in tabs
    // are counted.
    let files = |a: &App| a.docs.tabs.iter().filter(|d| d.path.is_some()).count();
    let tabs = files(&a);
    a.task_run();
    assert!(a.plan.is_some(), "kyoto.md's plan");
    a.plan_write();
    assert_eq!(files(&a), tabs + 1, "the first run's note opens");
    assert!(a.plan.is_some(), "nara.md's plan");
    a.plan_write();
    assert_eq!(files(&a), tabs + 1, "the second run opens no tab");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C1, 6: the window keeps the size it is drawn at, so what is placed from it
/// after a resize (the group menu, the Add menu) is placed in the window as it is now.
#[test]
fn the_window_keeps_the_size_it_is_drawn_at() {
    let (mut a, dir) = open("task-resized", TaskKind::File, Vec::new());
    a.size = (900, 640);
    a.compose();
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.win, a.logical());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C1, nit: a file another task of the same window has done (Rewrite them,
/// say) does not stop the first run of Split them from opening its note.
#[test]
fn split_after_another_task_in_the_window_still_opens_its_note() {
    let (mut a, dir) = open("task-split-after", TaskKind::Split, Vec::new());
    let kyoto = trip(&dir, "kyoto.md", "Kyoto");
    let done = trip(&dir, "done.md", "Done");
    a.task_add_paths(vec![kyoto, done]);
    with_group(&mut a, "Trips");
    if let Some(s) = a.task.as_mut() {
        s.files[1].done = true;
    }
    let tabs = a.docs.tabs.len();
    a.task_run();
    assert!(a.plan.is_some(), "kyoto.md's plan");
    a.plan_write();
    assert_eq!(
        a.docs.tabs.len(),
        tabs + 1,
        "the first split run's note opens"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C1, 3: a copy never goes into the library folder itself (A7), where the
/// reader would take it for a library file: a group whose folder is the library's is refused,
/// and nothing is copied.
#[test]
fn file_them_never_copies_into_the_library_folder_itself() {
    let (mut a, dir) = open("task-copy-a7", TaskKind::File, Vec::new());
    let desk = dir.join("desk");
    std::fs::create_dir_all(&desk).expect("desk");
    let from = desk.join("notes.md");
    std::fs::write(&from, "# Notes\n").expect("notes");
    a.task_add_paths(vec![from]);
    with_group(&mut a, "Here");
    let library = dir.join("library");
    if let Some(g) = a.task.as_mut().and_then(|s| s.groups.last_mut()) {
        g.home = library.clone();
    }
    a.task_run();
    a.task_file_write();
    assert_eq!(said(&a), crate::commands::strings::LIBRARY_HOME_IS_LIBRARY);
    assert!(!library.join("notes.md").exists(), "nothing is copied");
    assert!(a.task.is_some(), "the window stays");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C1, nit: a file whose very bytes the group's folder holds already, and the
/// library lists, is not listed a second time.
#[test]
fn a_copy_the_library_lists_already_is_not_listed_twice() {
    let (mut a, dir) = open("task-copy-twice", TaskKind::File, Vec::new());
    let md = trip_with_picture(&dir);
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the first run is in");
    let group = dir.join("library").join("Trips.md");
    let once = std::fs::read_to_string(&group).expect("the group file");
    a.task_open(TaskKind::File, vec![md]);
    assert!(
        matches!(a.task.as_ref().map(|s| &s.dest), Some(DestChoice::Group(_))),
        "the one group is chosen"
    );
    a.task_run();
    a.task_file_write();
    assert_eq!(
        std::fs::read_to_string(&group).expect("the group file"),
        once
    );
    assert!(said(&a).contains("lists every one"), "{}", said(&a));
    let _ = std::fs::remove_dir_all(&dir);
}

/// File them on "Move them in" (phase C2): a file filed with its picture goes in as File them's
/// copy does, and then the original and the picture are in the Recycle Bin; "AI runs…" puts them
/// back where they were and takes the copies away.
#[test]
fn move_them_in_moves_the_files_and_undo_puts_them_back() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move", TaskKind::File, Vec::new());
    a.recycle = Box::new(crate::recycle::FolderBin::new(&dir.join("bin")));
    let md = trip_with_picture(&dir);
    let picture = dir.join("desk").join("img").join("beach.png");
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_act(Hit::FilesGo(FilesGo::Move));
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the move is in: {}", said(&a));
    assert!(!md.exists() && !picture.exists(), "moved: {}", said(&a));
    assert!(
        said(&a).contains("1 moved in with 1 picture"),
        "{}",
        said(&a)
    );
    let library = dir.join("library");
    let copy = library.join("Trips").join("trip.md");
    assert_eq!(
        std::fs::read_to_string(&copy).expect("the copy"),
        "# Trip\n\n![Beach](img/beach.png)\n"
    );
    let (list, _) = runs::list_all(&library);
    assert_eq!(list.len(), 1, "one record");
    a.run_undo(list[0].path.clone());
    assert_eq!(
        std::fs::read_to_string(&md).expect("put back"),
        "# Trip\n\n![Beach](img/beach.png)\n"
    );
    assert_eq!(std::fs::read(&picture).expect("put back"), b"\x89PNG beach");
    assert!(!copy.exists(), "the copy goes once its original is back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C2, 3: a file a tab holds with changes not saved is copied in, not moved:
/// it stays under its tab with its picture, and the message says why.
#[test]
fn a_file_open_with_changes_not_saved_is_copied_not_moved() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move-unsaved", TaskKind::File, Vec::new());
    a.recycle = Box::new(crate::recycle::FolderBin::new(&dir.join("bin")));
    let md = trip_with_picture(&dir);
    let picture = dir.join("desk").join("img").join("beach.png");
    let i = open_now(&mut a, &md);
    a.docs.activate(i);
    a.docs.edit(crate::docs::EditOp::Insert("x"), None);
    assert!(a.docs.tabs[i].modified());
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_act(Hit::FilesGo(FilesGo::Move));
    a.task_run();
    a.task_file_write();
    assert!(md.exists() && picture.exists(), "{}", said(&a));
    assert!(
        said(&a).contains(mdedit_core::moves::copy_in::UNSAVED),
        "{}",
        said(&a)
    );
    assert!(dir.join("library").join("Trips").join("trip.md").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C2, 2: a move whose record would be too large to read back moves nothing:
/// the files are copied in, the copies' record is written, and the message says why.
#[test]
fn a_move_too_large_for_its_record_is_a_copy() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move-full", TaskKind::File, Vec::new());
    a.recycle = Box::new(crate::recycle::FolderBin::new(&dir.join("bin")));
    let md = trip_with_picture(&dir);
    let library = dir.join("library");
    let note = library.join("Refs").join("ref.md");
    std::fs::create_dir_all(note.parent().expect("folder")).expect("refs");
    let big = format!(
        "# Ref

[Trip](../../desk/trip.md)

{}",
        "\"
"
        .repeat(1_200_000)
    );
    std::fs::write(&note, &big).expect("note");
    std::fs::write(
        library.join("refs.md"),
        format!(
            "# Refs

{}
",
            mdshared_app::library::link_line("Ref", Path::new("Refs/ref.md"))
        ),
    )
    .expect("refs");
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_act(Hit::FilesGo(FilesGo::Move));
    a.task_run();
    a.task_file_write();
    assert!(md.exists(), "{}", said(&a));
    assert_eq!(std::fs::read_to_string(&note).expect("note"), big);
    assert!(
        said(&a).contains(mdedit_core::moves::copy_in::RECORD_FULL),
        "{}",
        said(&a)
    );
    let (list, _) = runs::list_all(&library);
    assert_eq!(list.len(), 1, "the copies' record");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C2, nit: a tab on a library note the move rewrote is read again at
/// once.
#[test]
fn a_tab_on_a_note_the_move_rewrote_is_read_again() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move-tab", TaskKind::File, Vec::new());
    a.recycle = Box::new(crate::recycle::FolderBin::new(&dir.join("bin")));
    let md = trip_with_picture(&dir);
    let library = dir.join("library");
    let note = library.join("Refs").join("ref.md");
    std::fs::create_dir_all(note.parent().expect("folder")).expect("refs");
    std::fs::write(
        &note,
        "# Ref

[Trip](../../desk/trip.md)
",
    )
    .expect("note");
    std::fs::write(
        library.join("refs.md"),
        format!(
            "# Refs

{}
",
            mdshared_app::library::link_line("Ref", Path::new("Refs/ref.md"))
        ),
    )
    .expect("refs");
    let i = open_now(&mut a, &note);
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_act(Hit::FilesGo(FilesGo::Move));
    a.task_run();
    a.task_file_write();
    assert!(!md.exists(), "{}", said(&a));
    let on_disk = std::fs::read_to_string(&note).expect("note");
    assert!(on_disk.contains("../Trips/trip.md"), "{on_disk}");
    let i = a
        .docs
        .tabs
        .iter()
        .position(|d| d.path.as_deref() == Some(note.as_path()))
        .unwrap_or(i);
    // Its tab is read again now, not at the next look at the disk.
    assert!(a.docs.tabs[i].is_loading(), "the tab reloads");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's review of C2, 3: undo of a move does not write a library note a tab holds with
/// changes not saved, and keeps the copy it leads to.
#[test]
fn undo_of_a_move_leaves_a_note_open_with_changes_not_saved() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move-undo-unsaved", TaskKind::File, Vec::new());
    a.recycle = Box::new(crate::recycle::FolderBin::new(&dir.join("bin")));
    let md = trip_with_picture(&dir);
    let library = dir.join("library");
    let note = library.join("Refs").join("ref.md");
    std::fs::create_dir_all(note.parent().expect("folder")).expect("refs");
    std::fs::write(
        &note,
        "# Ref

[Trip](../../desk/trip.md)
",
    )
    .expect("note");
    std::fs::write(
        library.join("refs.md"),
        format!(
            "# Refs

{}
",
            mdshared_app::library::link_line("Ref", Path::new("Refs/ref.md"))
        ),
    )
    .expect("refs");
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_act(Hit::FilesGo(FilesGo::Move));
    a.task_run();
    a.task_file_write();
    assert!(!md.exists(), "{}", said(&a));
    let relinked = std::fs::read_to_string(&note).expect("note");
    let i = open_now(&mut a, &note);
    a.docs.activate(i);
    a.docs.edit(crate::docs::EditOp::Insert("x"), None);
    let (list, _) = runs::list_all(&library);
    a.run_undo(list[0].path.clone());
    assert!(md.exists(), "the original is back");
    assert_eq!(std::fs::read_to_string(&note).expect("note"), relinked);
    assert!(
        library.join("Trips").join("trip.md").exists(),
        "the copy stays"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// On "Move them in" a row says its file is moved in, and the footer warns that files outside
/// the library that link to them are not looked for.
#[test]
fn a_row_says_its_file_is_moved_in() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move-note", TaskKind::File, Vec::new());
    let outside = pasta(&dir);
    a.task_add_paths(vec![outside]);
    a.task_act(Hit::FilesGo(FilesGo::Move));
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.copy_note(0), Some("moved in"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's second review of C1, nit: a second Split them in the same window opens its first
/// note too; only the later files of one run open none.
#[test]
fn a_second_split_in_the_same_window_opens_its_note() {
    let (mut a, dir) = open("task-split-twice", TaskKind::Split, Vec::new());
    let kyoto = trip(&dir, "kyoto.md", "Kyoto");
    let nara = trip(&dir, "nara.md", "Nara");
    a.task_add_paths(vec![kyoto, nara]);
    with_group(&mut a, "Trips");
    a.task_run();
    a.plan_write();
    assert!(a.plan.is_some(), "nara.md's plan");
    a.plan_write();
    let tabs = a.docs.tabs.len();
    let window = a.task_parked.take().or_else(|| a.task.take());
    assert!(
        window.as_ref().is_some_and(|w| w.split_opened),
        "the first run opened its note"
    );
    a.task = window;
    let osaka = trip(&dir, "osaka.md", "Osaka");
    a.task_add_paths(vec![osaka]);
    a.task_run();
    assert!(a.plan.is_some(), "osaka.md's plan");
    a.plan_write();
    assert_eq!(a.docs.tabs.len(), tabs + 1, "the second run's note opens");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable's re-review of C2: a move of a file the library already lists with these very bytes,
/// beside a new one, leaves the listed one's original where it is, moves the other, and undo of
/// the run puts the other back.
#[test]
fn a_move_beside_a_file_listed_already_can_be_undone() {
    use mdedit_core::prefs::FilesGo;
    let (mut a, dir) = open("task-move-listed", TaskKind::File, Vec::new());
    a.recycle = Box::new(crate::recycle::FolderBin::new(&dir.join("bin")));
    let md = trip_with_picture(&dir);
    a.task_add_paths(vec![md.clone()]);
    with_group(&mut a, "Trips");
    a.task_run();
    a.task_file_write();
    assert!(a.task.is_none(), "the copy is in: {}", said(&a));
    let other = dir.join("desk").join("other.md");
    std::fs::write(&other, "# Other\n").expect("other");
    a.task_open(TaskKind::File, vec![md.clone(), other.clone()]);
    a.task_act(Hit::FilesGo(FilesGo::Move));
    a.task_run();
    a.task_file_write();
    assert!(md.exists(), "listed already, left: {}", said(&a));
    assert!(!other.exists(), "moved: {}", said(&a));
    let library = dir.join("library");
    let (list, _) = runs::list_all(&library);
    let moved = list
        .iter()
        .find(|r| runs::load(&r.path, &library).is_some_and(|x| !x.removed.is_empty()))
        .expect("the move's record");
    a.run_undo(moved.path.clone());
    assert!(other.exists(), "put back: {}", said(&a));
    assert!(md.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the hand-driven run of 2026-11-25 ---------------------------------------------------------

/// The centre of what the window last drew for `hit`.
fn centre_of(a: &App, hit: Hit) -> Option<(f32, f32)> {
    a.task.as_ref().and_then(|s| {
        s.hits
            .iter()
            .find(|(h, _)| *h == hit)
            .map(|(_, r)| (r.x + r.w / 2.0, r.y + r.h / 2.0))
    })
}

/// Finding 1: "Open AI settings" from the window put Settings under it, and the window took the
/// clicks. Settings is in front: the window takes no pointer while it is open, and comes back
/// when it closes with the connection read again.
#[test]
fn settings_opened_from_the_window_is_in_front_and_the_window_comes_back() {
    let (mut a, dir) = open("task-settings-front", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    a.compose();
    let (x, y) = centre_of(&a, Hit::Cancel).expect("Cancel was drawn");
    a.task_act(Hit::OpenSettings);
    assert!(a.ui.settings.is_some(), "Settings opens");
    assert!(!a.task_is_open(), "the window is not in front of it");
    // Nothing of the window is drawn while Settings is open: the same pixels as Settings with
    // no window at all (review of the task fixes, 2).
    a.compose();
    let with = a.screen.pixels.to_vec();
    let kept = a.task.take();
    a.compose();
    let without = a.screen.pixels.to_vec();
    a.task = kept;
    assert!(
        with == without,
        "the window is not drawn over or under Settings"
    );
    a.pointer_moved(x, y);
    assert_eq!(
        a.task.as_ref().expect("kept").hover,
        None,
        "the pointer over Settings is Settings', not the window's"
    );
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.close_settings();
    assert!(a.task_is_open(), "the window comes back");
    let s = a.task.as_ref().expect("kept");
    assert_eq!(s.files.len(), 1, "as it was");
    assert!(s.ai_on, "with the connection Settings turned on");
}

/// Finding 2: Esc in the new group's box leaves the box (its hint says so), not the window.
#[test]
fn escape_in_the_new_group_box_leaves_the_box() {
    let (mut a, _dir) = open("task-esc-group", TaskKind::File, Vec::new());
    assert_eq!(a.task.as_ref().expect("open").focus, Focus::NewGroup);
    a.task_key(Some(KeyName::Char('R')), Some("R"));
    a.task_key(Some(KeyName::Escape), None);
    let s = a.task.as_ref().expect("the window stays");
    assert_eq!(s.focus, Focus::None);
    assert_eq!(s.new_group.as_ref().map(|f| f.text()), Some("R"), "kept");
}

/// Finding 2: Esc in the instruction leaves it; then a window holding files says once, in the
/// window, what closing drops; any other key takes that back; the next Esc closes.
#[test]
fn escape_leaves_the_instruction_then_says_what_closing_drops() {
    let (mut a, dir) = open("task-esc-setup", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "Tidy it");
    a.task_act(Hit::Field);
    a.task_key(Some(KeyName::Escape), None);
    let s = a.task.as_ref().expect("the window stays");
    assert_eq!(s.focus, Focus::None, "the field is left");
    assert_eq!(s.instruction.text(), "Tidy it");
    a.task_key(Some(KeyName::Escape), None);
    let s = a.task.as_ref().expect("the window stays");
    assert_eq!(
        s.notice.as_deref(),
        Some("Esc again closes the window, and the file and the instruction in it go with it.")
    );
    a.task_key(Some(KeyName::Down), None);
    assert!(!a.task.as_ref().expect("open").esc_warned, "taken back");
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task.is_some(), "it says so again first");
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task.is_none(), "and closes");
}

/// Finding 4: Back on a rewrite's review goes back to the window with nothing written and the
/// file still ticked; nothing more is asked until Run.
#[test]
fn back_on_a_rewrite_review_returns_to_the_window_as_it_was() {
    let (mut a, dir) = ai_open("task-review-back", TaskKind::Rewrite, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    let sent = a.task_sent.requests;
    a.ai_review_action(crate::ai::Hit::Back);
    assert!(a.ai.panel.is_none(), "the review goes");
    let s = a.task.as_ref().expect("the window stays");
    assert!(s.files[0].ticked && !s.files[0].done, "the file as it was");
    assert!(s.asking.is_none());
    assert_eq!(a.task_sent.requests, sent, "nothing more is asked");
    assert_eq!(
        std::fs::read_to_string(&text).expect("pasta"),
        "# Pasta\n\nBoil it.\n"
    );
}

/// Finding 4: Cancel on a split's plan goes back to the window with the file still ticked.
#[test]
fn closing_a_split_plan_unwritten_keeps_the_file_ticked() {
    let (mut a, dir) = open("task-split-back", TaskKind::Split, Vec::new());
    let text = dir.join("trip.md");
    std::fs::write(&text, "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n").expect("trip");
    a.task_add_paths(vec![text]);
    with_group(&mut a, "Trips");
    a.task_run();
    assert!(a.plan.is_some());
    a.plan_close();
    let s = a.task.as_ref().expect("the window is back");
    assert!(s.files[0].ticked && !s.files[0].done, "as it was");
}

/// Finding 5: Write notes' Files… lists pictures; the other tasks' lists text and code.
#[test]
fn the_files_dialog_of_write_notes_lists_pictures() {
    let (label, list) = crate::platform::task_filter(true);
    assert_eq!(label, "Pictures and text");
    assert!(list.contains(&"png") && list.contains(&"jpg") && list.contains(&"md"));
    let (_, list) = crate::platform::task_filter(false);
    assert!(!list.contains(&"png") && list.contains(&"md"));
}

/// Finding 6: closing the tab "Show what is sent" opened brings the window back; the tab says
/// "would send" before anything was sent.
#[test]
fn closing_the_what_is_sent_tab_brings_the_window_back() {
    let (mut a, dir) = ai_open("task-sent-back", TaskKind::File, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    let text = a.task_request_text().expect("text");
    assert!(text.starts_with("What the AI task window would send. Nothing has been sent yet."));
    a.task_show_sent();
    assert!(a.task.is_none() && a.task_parked.is_some(), "parked");
    let i = a.docs.active().expect("the tab");
    a.close_tab(i);
    let s = a.task.as_ref().expect("the window is back");
    assert_eq!(s.files.len(), 1);
    assert!(s.sent_tab.is_none());
}

/// Finding 10: "AI task…" opens on the task the window last closed on.
#[test]
fn the_window_opens_again_on_the_last_task() {
    let (mut a, _dir) = open("task-last", TaskKind::File, Vec::new());
    a.task_act(Hit::Kind(TaskKind::Split));
    a.task_close();
    a.command(C::AiTaskFiles);
    assert_eq!(a.task.as_ref().expect("open").task, TaskKind::Split);
}

/// Finding 12: on the plan face a picture File them never sent is under NOT SENT (LEFT OUT with
/// the AI off), not under NOT PLACED, which says the model did not place it.
#[test]
fn what_was_never_sent_is_not_called_not_placed() {
    let (mut a, dir) = open("task-not-sent", TaskKind::File, Vec::new());
    let picture = dir.join("lasagne.jpg");
    std::fs::write(&picture, [0xFF, 0xD8, 0xFF, 0xE0]).expect("picture");
    a.task_add_paths(vec![pasta(&dir), picture]);
    with_group(&mut a, "Recipes");
    a.task_run();
    let s = a.task.as_ref().expect("open");
    assert!(s.planned);
    let rows = crate::task_ui::rows(s);
    assert!(rows.contains(&crate::task_ui::ListRow::NotSent));
    assert!(!rows.contains(&crate::task_ui::ListRow::Unplaced));
}

/// Finding 8: "Add N to the library" with no group is off, with the reason.
#[test]
fn add_to_the_library_with_no_group_is_off_with_the_reason() {
    let (mut a, dir) = open("task-no-group-off", TaskKind::File, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    a.task_run();
    let s = a.task.as_mut().expect("open");
    assert!(s.planned);
    s.dest = DestChoice::Decide;
    assert_eq!(
        s.primary_blocked().as_deref(),
        Some(crate::task_ui::CHOOSE_GROUP)
    );
}

/// Review of the task fixes, 1: a review kept from a window since closed has no Back, and Back
/// pressed anyway keeps the review and says why; nothing is thrown away.
#[test]
fn back_on_a_review_whose_window_closed_keeps_the_review() {
    let (mut a, dir) = ai_open("task-back-orphan", TaskKind::Rewrite, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text.clone()]);
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    escape(&mut a);
    assert!(a.ai.kept.is_some(), "the review is kept");
    a.task_close();
    a.command(C::AiShowResult);
    let back = |a: &App| match a.ai.panel.as_ref() {
        Some(Panel::Review(rv)) => rv.file.as_ref().map(|f| f.back),
        _ => None,
    };
    assert_eq!(
        back(&a),
        Some(false),
        "no Back is drawn: no window to go back to"
    );
    a.ai_review_action(crate::ai::Hit::Back);
    assert!(
        matches!(a.ai.panel, Some(Panel::Review(_))),
        "the review stays, its answer with it"
    );
    assert!(said(&a).contains("nothing to go back to"));
    assert_eq!(
        std::fs::read_to_string(&text).expect("pasta"),
        "# Pasta\n\nBoil it.\n"
    );
}

/// Review of the task fixes, 5: Back on a review names the window's button as it reads.
#[test]
fn back_on_a_review_names_the_button_as_it_reads() {
    let (mut a, dir) = ai_open("task-back-label", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "Tidy it");
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    a.ai_review_action(crate::ai::Hit::Back);
    assert_eq!(
        said(&a),
        "pasta.md was not rewritten. \u{201C}Rewrite 1 file\u{201D} asks for it again; the files \
         rewritten before it stay so."
    );
}

/// Review of the task fixes, 4: what Esc says agrees with what the window holds.
#[test]
fn what_esc_says_agrees_with_what_the_window_holds() {
    let (mut a, dir) = open("task-esc-words", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    let s = a.task.as_ref().expect("open");
    assert_eq!(
        s.esc_warning(),
        "Esc again closes the window, and the file in it goes with it."
    );
    let two = dir.join("two.md");
    std::fs::write(&two, "# Two\n").expect("two");
    a.task_add_paths(vec![two]);
    let s = a.task.as_ref().expect("open");
    assert_eq!(
        s.esc_warning(),
        "Esc again closes the window, and the 2 files in it go with it."
    );
    let (mut b, _dir) = open("task-esc-words-none", TaskKind::Rewrite, Vec::new());
    let s = b.task.as_mut().expect("open");
    assert_eq!(
        s.esc_warning(),
        "Esc again closes the window, and what is typed in it goes with it."
    );
    s.instruction.set_text("Tidy it");
    assert_eq!(
        s.esc_warning(),
        "Esc again closes the window, and the instruction in it goes with it."
    );
}

/// Review of the task fixes, 7: a rewrite run that ends closes the window, and "AI task…"
/// opens next on Rewrite them as Cancel would have left it.
#[test]
fn a_rewrite_run_that_ends_remembers_its_task() {
    let (mut a, dir) = ai_open("task-last-rewrite", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "Tidy it");
    assert_eq!(a.docs.prefs.ai_task, mdedit_core::prefs::AiTask::File);
    a.task_run();
    wait(&mut a, |a| a.ai.panel.is_some());
    a.task_review_apply();
    assert!(a.task.is_none(), "the run is over and the window closed");
    assert_eq!(a.docs.prefs.ai_task, mdedit_core::prefs::AiTask::Rewrite);
}

/// Review of the task fixes, 8: Settings closed over the window reads the saved prompts again,
/// and a file kept without its text for the largest rewrite then is read again.
#[test]
fn settings_closed_over_the_window_reads_the_prompts_and_the_limit_again() {
    let (mut a, dir) = open("task-settings-again", TaskKind::Rewrite, Vec::new());
    a.docs.prefs.ai_rewrite_kb = mdedit_core::prefs::AI_REWRITE_KB_MIN;
    let big = dir.join("big.md");
    std::fs::write(&big, "# Big\n\n".to_string() + &"words ".repeat(3000)).expect("big");
    if let Some(s) = a.task.as_mut() {
        s.limits = crate::task_ui::Limits::of(&a.docs.prefs);
    }
    a.task_add_paths(vec![big]);
    assert!(
        a.task.as_ref().expect("open").files[0].text.is_none(),
        "too long then"
    );
    a.task_act(Hit::OpenSettings);
    a.docs.prefs.ai_rewrite_kb = mdedit_core::prefs::AI_REWRITE_KB_DEFAULT;
    std::fs::write(
        dir.join(mdedit_core::ai::PROMPTS_FILE),
        mdedit_core::ai::append_prompt("", "Tidy", "Tidy it up."),
    )
    .expect("prompts");
    a.close_settings();
    let s = a.task.as_ref().expect("back");
    assert_eq!(s.prompts.len(), 1, "the prompt saved meanwhile is listed");
    assert!(s.files[0].text.is_some(), "the file is read again");
}

/// Review of the task fixes, 9: Close all tabs, which closes "Show what is sent"'s tab too,
/// brings the window back as closing that tab does.
#[test]
fn closing_all_tabs_brings_the_window_back() {
    let (mut a, dir) = ai_open("task-sent-close-all", TaskKind::File, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    a.task_show_sent();
    assert!(a.task.is_none() && a.task_parked.is_some(), "parked");
    a.command(C::CloseAllTabs);
    let s = a.task.as_ref().expect("the window is back");
    assert_eq!(s.files.len(), 1);
}

/// Review of the task fixes, 10: once a request was made, the tab says what is sent for the
/// files ticked now, and how many requests went.
#[test]
fn what_is_sent_after_a_request_says_how_many_went() {
    let (mut a, dir) = ai_open("task-sent-after", TaskKind::File, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    a.task_sent.requests = 2;
    let text = a.task_request_text().expect("text");
    assert!(text.starts_with(
        "What the AI task window sends for the files ticked now. It has made 2 requests already."
    ));
    a.task_sent.requests = 1;
    let text = a.task_request_text().expect("text");
    assert!(text.contains("It has made 1 request already."));
}

/// Review of the task fixes, 10: "Add to the library" with no group is drawn off, and on with
/// one.
#[test]
fn add_to_the_library_is_drawn_off_without_a_group() {
    let (mut a, dir) = open("task-add-drawn-off", TaskKind::File, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    if let Some(s) = a.task.as_mut() {
        s.dest = DestChoice::Decide;
        s.new_group = None;
        s.focus = Focus::Instruction;
    }
    a.task_run();
    assert!(a.task.as_ref().expect("open").planned);
    let pixel = |a: &mut App| {
        a.compose();
        let (x, y) = centre_of(a, Hit::Primary).expect("the button is drawn");
        let w = a.screen.width as usize;
        let s = a.scale;
        let at = ((y * s) as usize * w + (x * s) as usize) * 4;
        a.screen.pixels[at..at + 4].to_vec()
    };
    let off = pixel(&mut a);
    with_group(&mut a, "Recipes");
    let on = pixel(&mut a);
    assert_ne!(off, on, "off without a group, on with one");
}

/// Review of the task fixes, 14, 17, 18: the words of a file left out, of the header and of
/// an empty Saved prompts.
#[test]
fn the_windows_words_say_what_is_so() {
    let (mut a, _dir) = open("task-words", TaskKind::File, Vec::new());
    let s = a.task.as_mut().expect("open");
    s.ai_on = false;
    assert_eq!(
        s.left_out_note(),
        "unticked",
        "nothing is sent with the AI off"
    );
    s.ai_on = true;
    assert_eq!(s.left_out_note(), "unticked: not sent");
    assert_eq!(
        crate::task_ui::prompts_empty(TaskKind::File),
        "None of this kind yet. Add one in Settings \u{203A} AI \u{203A} Saved prompts \u{203A} \
         Edit, with the line \u{201C}kind: file\u{201D}."
    );
    assert!(!crate::task_ui::HEADER_UNDER.contains("opened"));
}

/// Review of the task fixes, 16: after Esc left the fields a character typed goes into the
/// instruction, and Enter runs nothing: it puts the keyboard back in the instruction.
#[test]
fn after_esc_left_the_fields_typing_goes_to_the_instruction() {
    let (mut a, dir) = open("task-esc-type", TaskKind::Rewrite, Vec::new());
    a.task_add_paths(vec![pasta(&dir)]);
    instruct(&mut a, "Tidy");
    a.task_act(Hit::Field);
    a.task_key(Some(KeyName::End), None);
    a.task_key(Some(KeyName::Escape), None);
    assert_eq!(a.task.as_ref().expect("open").focus, Focus::None);
    a.task_key(Some(KeyName::Char('!')), Some("!"));
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.focus, Focus::Instruction);
    assert_eq!(s.instruction.text(), "Tidy!");
    a.task_key(Some(KeyName::Escape), None);
    a.task_key(Some(KeyName::Enter), None);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.focus, Focus::Instruction, "Enter puts the keyboard back");
    assert!(
        s.asking.is_none() && a.ai.panel.is_none(),
        "and runs nothing"
    );
}

// ---- a folder added: its whole tree, on its own thread (D-247, D-248) -------------------------

/// Runs the window's folder reads to their end and takes each result, as the event loop would.
fn read_folder(a: &mut App) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        while a.task_job.as_ref().is_some_and(|j| j.running()) {
            assert!(std::time::Instant::now() < until, "the folder was not read");
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        a.task_add_take();
        if a.task_job.is_none() && a.task_queue.is_empty() {
            return;
        }
        assert!(std::time::Instant::now() < until, "the reads did not end");
    }
}

/// The owner's report of 2026-11-25: a folder added to the window found only the files directly
/// in it. Its whole tree is walked, the folder's own files first, then each subfolder's by name,
/// and every kind of file is kept for `task_probe` to judge.
#[test]
fn a_folder_added_brings_its_whole_tree_its_own_files_first() {
    let (mut a, dir) = open("task-walk-tree", TaskKind::File, Vec::new());
    let folder = dir.join("Dishes");
    std::fs::create_dir_all(folder.join("A/B/C")).expect("folders");
    std::fs::write(folder.join("top.md"), "# Top\n").expect("top");
    std::fs::write(folder.join("A/B/mid.md"), "# Mid\n").expect("mid");
    std::fs::write(folder.join("A/B/C/pic.png"), [0x89, b'P', b'N', b'G']).expect("picture");
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the folder is read on its thread");
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    let names: Vec<&str> = s.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["top.md", "mid.md", "pic.png"], "{names:?}");
    assert_eq!(s.files[0].kind, FileKind::Text);
    assert_eq!(s.files[1].kind, FileKind::Text);
    assert_eq!(s.files[2].kind, FileKind::Picture, "every kind is kept");
    assert!(s.files[2].text.is_none(), "a picture is never read");
    assert_eq!(s.notice, None, "nothing was left out");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The import walk's rules: a dot name, the hidden attribute and `target`/`node_modules` folders
/// are left out. A dot file at the top catches the one-level walk, which took it.
#[test]
fn a_folder_added_skips_dot_names_and_the_folders_the_files_pane_leaves_out() {
    let (mut a, dir) = open("task-walk-skips", TaskKind::File, Vec::new());
    let folder = dir.join("Code");
    std::fs::create_dir_all(folder.join(".git")).expect("dot folder");
    std::fs::create_dir_all(folder.join("node_modules")).expect("node_modules");
    std::fs::create_dir_all(folder.join("target")).expect("target");
    std::fs::write(folder.join("real.md"), "# Real\n").expect("real");
    std::fs::write(folder.join(".hidden.md"), "# Hidden\n").expect("hidden file");
    std::fs::write(folder.join(".git/inside.md"), "# Inside\n").expect("inside");
    std::fs::write(folder.join("node_modules/dep.js"), "dep\n").expect("dep");
    std::fs::write(folder.join("target/out.rs"), "out\n").expect("out");
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    let names: Vec<&str> = s.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["real.md"], "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder over the window's cap stops at it and says the rest was left out; the same folder
/// added again adds nothing and says the same, because the rest of it is still left out.
#[test]
fn a_folder_added_over_the_cap_stops_at_it_and_a_second_add_says_the_rest_was_left_out() {
    let (mut a, dir) = open("task-walk-cap", TaskKind::File, Vec::new());
    let folder = dir.join("Many");
    std::fs::create_dir_all(&folder).expect("folders");
    for i in 0..=crate::task_ui::FILES_MAX {
        std::fs::write(folder.join(format!("n{i:05}.md")), "# N\n").expect("a file");
    }
    a.task_add_paths(vec![folder.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    // 2,000 files are more than the 200 the question is for (D-249): answer it, and the cap's
    // line is the window's own.
    assert_eq!(
        a.task.as_ref().expect("open").ask.as_deref(),
        Some(
            format!(
                "Add {} files from Many?",
                mdedit_core::import::group_digits(crate::task_ui::FILES_MAX)
            )
            .as_str()
        )
    );
    answer_question(&mut a);
    read_folder(&mut a);
    let note = format!(
        "Stopped at {} files; the rest of Many was left out.",
        mdedit_core::import::group_digits(crate::task_ui::FILES_MAX)
    );
    {
        let s = a.task.as_ref().expect("the window is open");
        assert_eq!(s.files.len(), crate::task_ui::FILES_MAX);
        assert_eq!(s.notice.as_deref(), Some(note.as_str()));
    }
    // The window has no room left: the same folder adds nothing, and says so, because the files
    // it did not walk are left out all the same (D-250).
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), crate::task_ui::FILES_MAX);
    assert_eq!(s.notice.as_deref(), Some(note.as_str()));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The cap is the room the window has left, and it counts only files it does not hold already:
/// with one of the folder's files in and room for two, the next two come and the fourth is named
/// as left out.
#[test]
fn the_cap_counts_only_the_files_that_are_new() {
    let (a, dir) = open("task-cap-new", TaskKind::File, Vec::new());
    let folder = dir.join("Many");
    std::fs::create_dir_all(&folder).expect("folders");
    for i in 0..4 {
        std::fs::write(folder.join(format!("f{i}.md")), "# F\n").expect("a file");
    }
    let mut known = std::collections::HashSet::new();
    known.insert(super::task::path_key(&folder.join("f0.md")));
    // The walk only, so the test counts the paths it takes before anything is read (D-250).
    let add = super::task::task_read(
        vec![folder.clone()],
        1,
        a.probe(),
        None,
        known,
        2,
        0,
        false,
        false,
        &|| false,
    );
    let names: Vec<&str> = add
        .walked
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap())
        .collect();
    assert_eq!(names, ["f1.md", "f2.md"], "{names:?}");
    assert!(!add.notes.is_empty(), "f3.md is there and there is no room");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file added first, then its folder: the file is in once.
#[test]
fn a_file_added_before_its_folder_is_not_added_twice() {
    let (mut a, dir) = open("task-walk-dedup", TaskKind::File, Vec::new());
    let folder = dir.join("Shots");
    std::fs::create_dir_all(&folder).expect("folders");
    let one = folder.join("one.md");
    std::fs::write(&one, "# One\n").expect("one");
    std::fs::write(folder.join("two.md"), "# Two\n").expect("two");
    a.task_add_paths(vec![one]);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 1);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    let names: Vec<&str> = s.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["one.md", "two.md"], "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// While a folder is read the window says so over its list's foot; the line goes when the files
/// are in.
#[test]
fn while_a_folder_is_read_the_window_says_so() {
    let (mut a, dir) = open("task-walk-reading", TaskKind::File, Vec::new());
    let folder = dir.join("Dishes");
    std::fs::create_dir_all(&folder).expect("folders");
    std::fs::write(folder.join("one.md"), "# One\n").expect("one");
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.reading.as_deref(), Some("Dishes"));
    assert_eq!(s.reading_line().as_deref(), Some("Reading Dishes\u{2026}"));
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert!(s.reading_line().is_none(), "the line goes when it is read");
    assert_eq!(s.files.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Esc while a folder is read stops it: nothing of it is added, and the window says so.
#[test]
fn escaping_while_a_folder_is_read_stops_it_and_adds_nothing() {
    let (mut a, dir) = open("task-walk-escape", TaskKind::File, Vec::new());
    let folder = dir.join("Dishes");
    std::fs::create_dir_all(&folder).expect("folders");
    for i in 0..50 {
        std::fs::write(folder.join(format!("f{i}.md")), "# F\n").expect("a file");
    }
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the folder is read on its thread");
    a.task_key(Some(KeyName::Escape), None);
    assert!(a.task_job.is_none(), "the read is stopped");
    {
        let s = a.task.as_ref().expect("the window stays open");
        assert!(s.files.is_empty(), "nothing of it is added");
        assert_eq!(s.notice.as_deref(), Some("Stopped: Dishes was not added."));
        assert!(s.reading.is_none());
    }
    // Its thread ends and hands nothing back.
    std::thread::sleep(std::time::Duration::from_millis(60));
    a.task_add_take();
    let s = a.task.as_ref().expect("the window stays open");
    assert!(s.files.is_empty(), "nothing of it is added");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- a big folder add asks first, before anything is read (D-250) -----------------------------

/// Enter on the question from the instruction field, where the owner would be typing: the field
/// of a new group and the dropdowns have the key first, as they always had (D-250).
fn answer_question(a: &mut App) {
    if let Some(s) = a.task.as_mut() {
        s.focus = Focus::Instruction;
        s.dest_open = false;
        s.add_open = false;
    }
    a.task_key(Some(KeyName::Enter), None);
}

/// A folder of `n` small text files, `prefix` in each name, made under `dir`.
fn many(dir: &Path, name: &str, n: usize) -> PathBuf {
    let folder = dir.join(name);
    std::fs::create_dir_all(&folder).expect("folders");
    for i in 0..n {
        std::fs::write(folder.join(format!("{i:04}.md")), "# A file\n").expect("a file");
    }
    folder
}

/// 201 new files: the walk counts them and stops there, not one of them read; the window asks,
/// and Enter reads and adds them all, ticked.
#[test]
fn a_folder_add_of_201_files_asks_before_reading_anything() {
    let (mut a, dir) = open("task-ask-201", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    {
        let s = a.task.as_ref().expect("the window is open");
        assert!(s.files.is_empty(), "nothing is in before the answer");
        assert_eq!(s.ask.as_deref(), Some("Add 201 files from Many?"));
        let held = a.task_held.as_ref().expect("the paths are held");
        assert_eq!(held.paths.len(), 201);
    }
    // The job, asked to read a small set, still reads none of these: more than ASK_FILES is
    // walked and counted only (D-250).
    let walked = super::task::task_read(
        vec![folder],
        1,
        a.probe(),
        None,
        Default::default(),
        2_000,
        0,
        true,
        false,
        &|| false,
    );
    assert_eq!(walked.walked.len(), 201);
    assert_eq!(walked.reads, 0, "not one file was read");
    assert!(walked.files.is_empty());
    answer_question(&mut a);
    assert!(a.task_job.is_some(), "the files are read after the answer");
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), 201);
    assert!(s.files.iter().all(|f| f.ticked), "all of them are ticked");
    assert!(s.ask.is_none(), "the question goes");
    assert!(a.task_held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Leave them out" adds none of them, and not one of them was read.
#[test]
fn leave_them_out_on_the_question_adds_none() {
    let (mut a, dir) = open("task-ask-leave", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(a.task.as_ref().expect("open").ask.is_some());
    a.task_act(Hit::AskLeave);
    let s = a.task.as_ref().expect("the window stays open");
    assert!(s.files.is_empty(), "nothing is added");
    assert!(s.ask.is_none(), "the question goes");
    assert!(a.task_held.is_none(), "and nothing is kept");
    assert!(a.task_job.is_none(), "and nothing is read");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Esc on the question leaves them out, and it does not close the window.
#[test]
fn escape_on_the_question_leaves_them_out_and_keeps_the_window() {
    let (mut a, dir) = open("task-ask-esc", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(a.task.as_ref().expect("open").ask.is_some());
    a.task_key(Some(KeyName::Escape), None);
    let s = a
        .task
        .as_ref()
        .expect("the window stays open on the question");
    assert!(s.files.is_empty(), "nothing is added");
    assert!(s.ask.is_none(), "the question goes");
    assert!(a.task_held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// 200 new files are read and added at once: the question is only for more than 200.
#[test]
fn two_hundred_files_are_added_with_no_question() {
    let (mut a, dir) = open("task-ask-200", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 200);
    a.task_add_paths(vec![folder.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), 200);
    assert!(s.ask.is_none(), "no question at 200");
    assert!(a.task_held.is_none());
    // At the threshold the same job carries on to the probes, in one read (D-250).
    let read = super::task::task_read(
        vec![folder],
        1,
        a.probe(),
        None,
        Default::default(),
        2_000,
        0,
        true,
        false,
        &|| false,
    );
    assert!(read.probed);
    assert_eq!(read.reads, 200);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two folders dropped together, as winit sends them (one `DroppedFile` per path), make one read
/// and one question, with the total and the number of folders.
#[test]
fn two_dropped_folders_make_one_question() {
    let (mut a, dir) = open("task-ask-two", TaskKind::File, Vec::new());
    let one = many(&dir, "One", 120);
    let two = many(&dir, "Two", 81);
    a.dropped(one);
    a.dropped(two);
    a.task_flush_drops();
    read_folder(&mut a);
    {
        let s = a.task.as_ref().expect("the window is open");
        assert!(s.files.is_empty(), "nothing is in before the answer");
        assert_eq!(s.ask.as_deref(), Some("Add 201 files from 2 folders?"));
    }
    answer_question(&mut a);
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), 201);
    assert!(s.ask.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder dropped while another is still being read is not lost: it waits for that read, and
/// the question that follows covers both.
#[test]
fn a_drop_during_a_read_is_read_after_it_and_never_lost() {
    let (mut a, dir) = open("task-ask-during", TaskKind::File, Vec::new());
    let first = many(&dir, "First", 300);
    let late = many(&dir, "Late", 50);
    a.dropped(first);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the first folder is read");
    a.dropped(late);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the first read goes on");
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.ask.as_deref(), Some("Add 350 files from 2 folders?"));
    assert!(s.files.is_empty());
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 350);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A drop while the question stands is folded into it, so the question covers the whole set
/// (D-250): the same small add that would go straight in goes into the count instead.
#[test]
fn a_drop_while_the_question_stands_is_folded_into_it() {
    let (mut a, dir) = open("task-ask-fold", TaskKind::File, Vec::new());
    let first = many(&dir, "First", 201);
    a.task_add_paths(vec![first]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert_eq!(
        a.task.as_ref().expect("open").ask.as_deref(),
        Some("Add 201 files from First?")
    );
    let extra = many(&dir, "Extra", 5);
    a.dropped(extra);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.ask.as_deref(), Some("Add 206 files from 2 folders?"));
    assert!(s.files.is_empty(), "still nothing is in");
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 206);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The bar says both while a read goes on beside the question: the question, then "Reading…".
#[test]
fn the_bar_shows_the_read_beside_the_question() {
    let mut s = crate::task_ui::TaskScreen::new(
        TaskKind::File,
        Vec::new(),
        crate::task_ui::DestChoice::New("Recipes".to_string()),
        Vec::new(),
        "Claude".to_string(),
    );
    s.ask = Some("Add 201 files from Many?".to_string());
    assert_eq!(s.ask_line().as_deref(), Some("Add 201 files from Many?"));
    s.reading = Some("Extra".to_string());
    assert_eq!(
        s.ask_line().as_deref(),
        Some("Add 201 files from Many? Reading Extra\u{2026}")
    );
}

/// A run that ends takes the question and the read with it; the next window starts with nothing
/// of them.
#[test]
fn a_run_that_ends_drops_the_question() {
    let (mut a, dir) = open("task-forget-question", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(a.task_held.is_some());
    a.task_rewrite_finish();
    assert!(a.task.is_none(), "the window ends");
    assert!(a.task_held.is_none() && a.task_job.is_none());
    assert!(a.task_queue.is_empty() && a.task_batch.is_empty());
    a.task_open(TaskKind::File, Vec::new());
    let s = a.task.as_ref().expect("a new window");
    assert!(s.files.is_empty() && s.ask.is_none() && s.reading.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A read started for a window that is gone never reaches the next one: its result is dropped.
#[test]
fn a_new_window_never_takes_an_old_reads_result() {
    let (mut a, dir) = open("task-forget-read", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 2_000);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    let job = a.task_job.take().expect("the read runs");
    a.task_rewrite_finish();
    a.task_open(TaskKind::File, Vec::new());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while job.running() {
        assert!(std::time::Instant::now() < until, "the read did not end");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    a.task_job = Some(job);
    a.task_add_take();
    let s = a.task.as_ref().expect("the new window");
    assert!(s.files.is_empty(), "the old read did not reach it");
    assert!(s.ask.is_none() && s.reading.is_none());
    assert!(a.task_held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A split that parks the window stops its read and its question, so neither hangs on a window
/// that is not in front.
#[test]
fn a_split_that_parks_the_window_stops_its_read() {
    let (mut a, dir) = open("task-split-read", TaskKind::Split, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text]);
    let folder = many(&dir, "Many", 700);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the folder is read");
    a.task_split_start();
    assert!(a.plan_is_open(), "the plan opens");
    assert!(a.task.is_none(), "the window is parked");
    assert!(a.task_job.is_none(), "its read stopped");
    let parked = a.task_parked.as_ref().expect("the window is parked");
    assert!(parked.reading.is_none() && parked.ask.is_none());
    assert!(a.task_held.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the question holds takes its room: a window of 2,000 takes no more, and says so.
#[test]
fn a_full_window_stops_at_its_cap_with_the_note() {
    let (mut a, dir) = open("task-cap-held", TaskKind::File, Vec::new());
    let one = many(&dir, "One", 2_000);
    let two = many(&dir, "Two", 2_000);
    a.task_add_paths(vec![one]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(
        a.task.as_ref().expect("open").ask.is_some(),
        "2,000 is asked about"
    );
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 2_000);
    a.task_add_paths(vec![two]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), crate::task_ui::FILES_MAX);
    assert_eq!(
        s.notice.as_deref(),
        Some("Stopped at 2,000 files; the rest of Two was left out.")
    );
    assert!(s.ask.is_none(), "nothing was held");
    let _ = std::fs::remove_dir_all(&dir);
}

/// What the question holds takes the window's room too: 1,500 in and 500 held leave none, so a
/// further folder is left out and named, and the window never passes FILES_MAX (D-250).
#[test]
fn the_question_hands_take_their_room() {
    let (mut a, dir) = open("task-held-room", TaskKind::File, Vec::new());
    let big = many(&dir, "Big", 1_500);
    a.task_add_paths(vec![big]);
    a.task_flush_drops();
    read_folder(&mut a);
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 1_500);
    let held = many(&dir, "Held", 500);
    a.task_add_paths(vec![held]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert_eq!(
        a.task.as_ref().expect("open").ask.as_deref(),
        Some("Add 500 files from Held?")
    );
    let extra = many(&dir, "Extra", 100);
    a.task_add_paths(vec![extra]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert_eq!(
        a.task_held.as_ref().map_or(0, |h| h.paths.len()),
        500,
        "the 100 have no room and are not held"
    );
    assert_eq!(
        a.task.as_ref().expect("open").ask.as_deref(),
        Some("Add 500 files from Held?"),
        "{:?}",
        a.task.as_ref().expect("open").ask
    );
    answer_question(&mut a);
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.files.len(), crate::task_ui::FILES_MAX);
    assert_eq!(
        s.notice.as_deref(),
        Some("Stopped at 2,000 files; the rest of Extra was left out.")
    );
    let _ = std::fs::remove_dir_all(&dir);
}
/// A dropdown or the new group's field has Enter and Esc first; only with neither does the
/// question take them.
#[test]
fn a_dropdown_takes_enter_and_escape_from_the_question() {
    let (mut a, dir) = open("task-ask-menu", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    if let Some(s) = a.task.as_mut() {
        s.dest_open = true;
        s.dest_hot = Some(0);
    }
    a.task_key(Some(KeyName::Enter), None);
    {
        let s = a.task.as_ref().expect("open");
        assert!(!s.dest_open, "the dropdown took the key");
        assert!(s.ask.is_some(), "the question is still to answer");
        assert!(s.files.is_empty());
    }
    if let Some(s) = a.task.as_mut() {
        s.dest_open = true;
        s.dest_hot = Some(0);
    }
    a.task_key(Some(KeyName::Escape), None);
    {
        let s = a.task.as_ref().expect("open");
        assert!(!s.dest_open);
        assert!(s.ask.is_some(), "Esc closed the dropdown, not the question");
    }
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 201);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The new group's field takes Enter as it always did, with the question standing.
#[test]
fn the_new_groups_field_takes_enter_from_the_question() {
    let (mut a, dir) = open("task-ask-field", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    if let Some(s) = a.task.as_mut() {
        let mut field = mdshared_ui::textfield::TextField::new();
        field.set_text("Recipes");
        s.new_group = Some(field);
        s.focus = Focus::NewGroup;
    }
    a.task_key(Some(KeyName::Enter), None);
    let s = a.task.as_ref().expect("open");
    assert!(s.ask.is_some(), "the field had the key");
    assert!(s.files.is_empty());
    assert!(s.new_group.is_none() || s.focus != Focus::NewGroup);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file added while a folder is read leaves the bar's "Reading…" line alone.
#[test]
fn a_file_added_during_a_read_keeps_the_line() {
    let (mut a, dir) = open("task-ask-file", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 1_000);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    assert!(a.task_job.is_some());
    let late = dir.join("late.md");
    std::fs::write(&late, "# Late\n").expect("late");
    a.task_add_paths(vec![late]);
    {
        let s = a.task.as_ref().expect("open");
        assert!(
            s.files.iter().any(|f| f.name == "late.md"),
            "the file is in"
        );
        assert!(s.reading.is_some(), "the folder's line stays");
    }
    read_folder(&mut a);
    assert_eq!(
        a.task_held.as_ref().map_or(0, |h| h.paths.len()),
        1_000,
        "the folder's 1,000 are held, the file is not counted twice"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The question standing keeps "Make the plan" from running on the window as it is, leaving the
/// held files out silently (D-250).
#[test]
fn the_plan_is_not_made_while_the_question_stands() {
    let (mut a, dir) = open("task-ask-plan", TaskKind::File, Vec::new());
    let text = pasta(&dir);
    a.task_add_paths(vec![text]);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 1);
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(a.task.as_ref().expect("open").ask.is_some());
    a.task_primary();
    {
        let s = a.task.as_ref().expect("open");
        assert!(!s.planned, "no plan was made");
        assert!(s.asking.is_none(), "and no run started");
        assert!(s.ask.is_some(), "the question is still to answer");
    }
    answer_question(&mut a);
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 202);
    assert!(!s.planned, "the question answered; no plan ran");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder that cannot be read at all adds nothing and says why, in the window (D-250).
#[test]
fn a_folder_that_cannot_be_read_says_so() {
    let (mut a, dir) = open("task-ask-unreadable", TaskKind::File, Vec::new());
    let folder = dir.join("Locked");
    std::fs::create_dir_all(&folder).expect("folders");
    // Queued as a folder and gone before its read: its turn leaves it unreadable.
    a.task_batch.push(folder.clone());
    std::fs::remove_dir_all(&folder).expect("gone");
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    assert!(s.files.is_empty(), "nothing is added");
    assert_eq!(s.notice.as_deref(), Some("Could not read Locked."));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder added to the window leaves the library's own folder out (audit 4, C5; D-250).
#[test]
fn a_folder_added_leaves_the_library_folder_out() {
    let (mut a, dir) = open("task-ask-library", TaskKind::File, Vec::new());
    let library = dir.join("library");
    std::fs::write(dir.join("a.md"), "# A\n").expect("a.md");
    std::fs::write(library.join("note.md"), "# Note\n").expect("note");
    a.task_add_paths(vec![dir.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("the window is open");
    let names: Vec<&str> = s.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["a.md"], "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Add them" before a fold read ends waits for it: neither the read nor the answer is thrown
/// away (D-251). The late folder then has a question of its own.
#[test]
fn an_answer_while_a_folded_read_runs_is_not_lost() {
    let (mut a, dir) = open("task-answer-wait", TaskKind::File, Vec::new());
    let first = many(&dir, "First", 201);
    a.task_add_paths(vec![first]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert_eq!(
        a.task.as_ref().expect("open").ask.as_deref(),
        Some("Add 201 files from First?")
    );
    let late = many(&dir, "Late", 300);
    a.dropped(late);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the late folder is read");
    answer_question(&mut a);
    assert!(
        a.task_answer.is_some(),
        "the answer waits for the read to end"
    );
    assert!(a.task.as_ref().expect("open").ask.is_none());
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 201, "the answer went in");
    assert_eq!(
        s.ask.as_deref(),
        Some("Add 300 files from Late?"),
        "and the read that waited is asked about"
    );
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 501);
    let _ = std::fs::remove_dir_all(&dir);
}

/// "Leave them out" while a folded read runs drops what that read finds, and the window says it
/// is still reading (D-251).
#[test]
fn a_late_question_declined_keeps_its_folded_read_out() {
    let (mut a, dir) = open("task-decline-fold", TaskKind::File, Vec::new());
    let first = many(&dir, "First", 201);
    a.task_add_paths(vec![first]);
    a.task_flush_drops();
    read_folder(&mut a);
    let late = many(&dir, "Late", 300);
    a.dropped(late);
    a.task_flush_drops();
    a.task_key(Some(KeyName::Escape), None);
    {
        let s = a.task.as_ref().expect("open");
        assert!(s.ask.is_none(), "the question is left out");
        assert!(s.reading.is_some(), "the read keeps its line");
        assert!(a.task_declined, "what that read finds is left out");
    }
    read_folder(&mut a);
    {
        let s = a.task.as_ref().expect("open");
        assert!(s.files.is_empty(), "nothing of either set is in");
        assert!(s.reading.is_none(), "and the line goes");
    }
    // A drop after the decline is a set of its own.
    let after = many(&dir, "After", 5);
    a.task_add_paths(vec![after]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 5);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A stop takes the paths waiting on the question as well, and says how many (D-251).
#[test]
fn a_stop_leaves_the_questions_paths_out_too() {
    let (mut a, dir) = open("task-stop-held", TaskKind::File, Vec::new());
    let first = many(&dir, "First", 201);
    a.task_add_paths(vec![first]);
    a.task_flush_drops();
    read_folder(&mut a);
    let late = many(&dir, "Late", 300);
    a.dropped(late);
    a.task_flush_drops();
    assert!(a.task_job.is_some(), "the late folder is read");
    assert!(a.task_add_stop(), "a read was stopped");
    let s = a.task.as_ref().expect("open");
    assert_eq!(
        s.notice.as_deref(),
        Some(
            "Stopped: Late was not added, and the 201 files waiting for the question were left out."
        )
    );
    assert!(s.ask.is_none());
    assert!(a.task_held.is_none() && a.task_answer.is_none());
    assert!(a.task_queue.is_empty());
    // What comes next is a set of its own, not folded into the 201.
    let after = many(&dir, "After", 5);
    a.task_add_paths(vec![after]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    let names: Vec<&str> = s.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names.len(), 5, "{names:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A walk keeps both what it could not read and where it stopped (D-251).
#[test]
fn a_walk_keeps_its_unreadable_folder_and_its_stop() {
    let (a, dir) = open("task-notes-both", TaskKind::File, Vec::new());
    let many_here = many(&dir, "Many", 4);
    let gone = dir.join("Gone");
    let add = super::task::task_read(
        vec![gone, many_here],
        1,
        a.probe(),
        None,
        Default::default(),
        2,
        0,
        false,
        false,
        &|| false,
    );
    let said = add.notes.join(" ");
    assert!(said.contains("Could not read Gone."), "{said}");
    assert!(said.contains("rest of Many was left out"), "{said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A window with no room takes no more single files, and says why (D-251).
#[test]
fn a_single_file_beyond_the_windows_room_is_left_out() {
    let (mut a, dir) = open("task-room-single", TaskKind::File, Vec::new());
    let sample = a.task_probe(&pasta(&dir));
    if let Some(s) = a.task.as_mut() {
        for i in 0..crate::task_ui::FILES_MAX {
            let mut f = sample.clone();
            f.path = dir.join(format!("fill-{i}.md"));
            s.files.push(f);
        }
        s.recount_limit();
    }
    let late = dir.join("late.md");
    std::fs::write(&late, "# Late\n").expect("late");
    a.task_add_paths(vec![late]);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), crate::task_ui::FILES_MAX);
    assert_eq!(
        s.notice.as_deref(),
        Some("Stopped at 2,000 files; the rest was left out.")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file dropped by name from the folder the question holds is counted once: it goes in, and
/// the 200 left are at the threshold, so they come in with it rather than waiting on a question
/// (D-251, D-253).
#[test]
fn a_file_from_the_held_folder_is_counted_once() {
    let (mut a, dir) = open("task-held-one", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    a.task_add_paths(vec![folder.join("0001.md")]);
    {
        let s = a.task.as_ref().expect("open");
        assert_eq!(s.files.len(), 1, "the file is in");
        assert!(s.ask.is_none(), "no question for 200 (D-253)");
    }
    assert!(a.task_probing, "the 200 are read at once");
    assert_eq!(a.task_held.as_ref().map_or(0, |h| h.paths.len()), 0);
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 201);
    assert!(s.files.iter().all(|f| f.ticked));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Esc during the probes of an answered set stops them and gives the button back: the plan can be
/// made on what the window holds (D-253).
#[test]
fn escape_stops_the_probes_and_the_plan_comes_back() {
    let (mut a, dir) = open("task-stop-probes", TaskKind::File, Vec::new());
    let one = pasta(&dir);
    a.task_add_paths(vec![one]);
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    answer_question(&mut a);
    assert!(a.task_probing, "the answer's probes are out");
    a.task_key(Some(KeyName::Escape), None);
    assert!(!a.task_probing, "the stop ends the probes");
    {
        let s = a.task.as_ref().expect("open");
        assert_eq!(s.files.len(), 1, "nothing of them is in");
        assert!(
            s.notice
                .as_deref()
                .is_some_and(|n| n.starts_with("Stopped")),
            "{:?}",
            s.notice
        );
    }
    a.task_primary();
    assert!(
        a.task.as_ref().expect("open").planned,
        "the plan is made on the one file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder that brings nothing new says so, rather than adding nothing in silence (D-251).
#[test]
fn a_folder_that_brings_nothing_new_says_so() {
    let (mut a, dir) = open("task-nothing-new", TaskKind::File, Vec::new());
    let folder = dir.join("Solo");
    std::fs::create_dir_all(&folder).expect("folder");
    let one = folder.join("one.md");
    std::fs::write(&one, "# One\n").expect("one");
    a.task_add_paths(vec![one]);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 1);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 1, "nothing is added twice");
    assert_eq!(s.notice.as_deref(), Some("Nothing new in Solo."));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A file added again is not a file left out: the window takes the "Stopped…" away rather than
/// saying it stopped at the cap (D-252).
#[test]
fn a_file_added_again_is_not_said_to_be_left_out() {
    let (mut a, dir) = open("task-file-again", TaskKind::File, Vec::new());
    let one = pasta(&dir);
    a.task_add_paths(vec![one.clone()]);
    a.task_add_paths(vec![one]);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 1, "it is in once");
    assert_eq!(s.notice, None, "and nothing was left out");
    let _ = std::fs::remove_dir_all(&dir);
}

/// "AI task on this file…" for a file the parked window already holds takes nothing away and
/// says nothing about a cap (D-252).
#[test]
fn a_parked_window_takes_its_own_file_again_quietly() {
    let (mut a, dir) = open("task-file-parked", TaskKind::File, Vec::new());
    let one = pasta(&dir);
    a.task_add_paths(vec![one.clone()]);
    a.task_show_sent();
    assert!(a.task.is_none() && a.task_parked.is_some(), "it is parked");
    a.task_open_on_file(one);
    let s = a.task.as_ref().expect("the window is back");
    assert_eq!(s.files.len(), 1, "it is in once");
    assert_eq!(s.notice, None, "and nothing was left out");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An answer's probes start with no folded question behind them, and any later result takes a
/// decline mark away: nothing stale can silence the drop after it (D-252).
#[test]
fn a_probe_job_leaves_no_stale_marks() {
    let (mut a, dir) = open("task-stale-decline", TaskKind::File, Vec::new());
    let first = many(&dir, "First", 201);
    a.task_add_paths(vec![first]);
    a.task_flush_drops();
    read_folder(&mut a);
    let late = many(&dir, "Late", 300);
    a.dropped(late);
    a.task_flush_drops();
    answer_question(&mut a);
    assert!(a.task_answer.is_some(), "the answer waits for the read");
    // The late read's result only: its files join the question, and the answer's probes start.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while a.task_job.as_ref().is_some_and(|j| j.running()) {
        assert!(
            std::time::Instant::now() < until,
            "the late folder was not read"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    a.task_add_take();
    assert!(a.task_probing, "the answer's probes are out");
    assert!(
        !a.task_fold,
        "the probes start with no folded question behind them"
    );
    // A mark left by anything else does not survive the probes' result.
    a.task_declined = true;
    read_folder(&mut a);
    assert!(!a.task_declined, "the probes' result takes it away");
    // And what comes next under a question goes in.
    let big = many(&dir, "Big", 500);
    a.task_add_paths(vec![big]);
    a.task_flush_drops();
    read_folder(&mut a);
    let small = many(&dir, "Small", 5);
    a.dropped(small);
    a.task_flush_drops();
    read_folder(&mut a);
    assert_eq!(
        a.task.as_ref().expect("open").ask.as_deref(),
        Some("Add 805 files from 3 folders?")
    );
    answer_question(&mut a);
    read_folder(&mut a);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 1_006);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The plan waits while answered files wait or are being probed: no run starts with the files the
/// window already holds and these left out (D-252).
#[test]
fn the_plan_waits_for_the_answered_files_to_be_probed() {
    let (mut a, dir) = open("task-plan-wait", TaskKind::File, Vec::new());
    let one = pasta(&dir);
    a.task_add_paths(vec![one]);
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    answer_question(&mut a);
    assert!(a.task_probing, "the answer's probes are out");
    a.task_primary();
    {
        let s = a.task.as_ref().expect("open");
        assert!(!s.planned, "no plan was made on the one file alone");
        assert!(s.asking.is_none(), "and no run started");
    }
    read_folder(&mut a);
    assert!(!a.task_probing);
    assert_eq!(a.task.as_ref().expect("open").files.len(), 202);
    a.task_primary();
    assert!(
        a.task.as_ref().expect("open").planned,
        "with them in, the plan is made"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A held folder dropped again under its own question does not say it brought nothing: the
/// question already accounts for it (D-252).
#[test]
fn a_held_folder_dropped_again_says_nothing() {
    let (mut a, dir) = open("task-held-again", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(a.task.as_ref().expect("open").ask.is_some());
    a.task_add_paths(vec![folder]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.notice, None, "nothing new is not said under its question");
    assert_eq!(s.ask.as_deref(), Some("Add 201 files from Many?"));
    answer_question(&mut a);
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 201);
    assert_eq!(s.notice, None, "nor after Add them");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two folders dropped together speak for themselves: the one that brought files goes in and the
/// one that brought nothing says so (D-252).
#[test]
fn every_folder_of_a_drop_speaks_for_itself() {
    let (mut a, dir) = open("task-each-folder", TaskKind::File, Vec::new());
    let solo = dir.join("Solo");
    std::fs::create_dir_all(&solo).expect("folder");
    let one = solo.join("one.md");
    std::fs::write(&one, "# One\n").expect("one");
    a.task_add_paths(vec![one]);
    let other = many(&dir, "Other", 3);
    a.task_add_paths(vec![solo, other]);
    a.task_flush_drops();
    read_folder(&mut a);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 4, "the other folder's three came in");
    assert_eq!(s.notice.as_deref(), Some("Nothing new in Solo."));
    let _ = std::fs::remove_dir_all(&dir);
}

/// When every file the question holds is added by name, there is nothing left to ask (D-252).
#[test]
fn a_question_with_nothing_left_behind_goes() {
    let (mut a, dir) = open("task-question-empty", TaskKind::File, Vec::new());
    let folder = many(&dir, "Many", 201);
    a.task_add_paths(vec![folder.clone()]);
    a.task_flush_drops();
    read_folder(&mut a);
    assert!(a.task.as_ref().expect("open").ask.is_some());
    let all: Vec<PathBuf> = (0..201)
        .map(|i| folder.join(format!("{i:04}.md")))
        .collect();
    a.task_add_paths(all);
    let s = a.task.as_ref().expect("open");
    assert_eq!(s.files.len(), 201, "every one of them came in by name");
    assert_eq!(a.task_held.as_ref().map_or(0, |h| h.paths.len()), 0);
    assert_eq!(s.ask, None, "the question goes with its files");
    let _ = std::fs::remove_dir_all(&dir);
}
