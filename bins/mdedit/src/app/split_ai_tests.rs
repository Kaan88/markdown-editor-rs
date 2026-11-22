//! Split by AI (5.2, phase 4b) through the window, with the stand-in engine of `ai_tests`: the
//! panel over the plan, the request, the answer becoming the plan, the retry, Stop, and what the
//! record of the run keeps.

use super::ai_tests::{engine, wait};
use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::ai::Panel;
use crate::keys::{Key, KeyName};
use crate::plan_ui::AiAct;
use mdedit_core::ai::{Connection, PromptKind};

fn press(a: &mut App, key: KeyName) {
    a.key_pressed(Key::Name(key), None);
}

/// `Trip.md` with two chapters under `# Tokyo`, a library beside it, the AI on, and the plan
/// screen open on it.
fn planned(tag: &str) -> (App, PathBuf) {
    engine();
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    let source = dir.join("Trip.md");
    std::fs::write(
        &source,
        "# Tokyo\n\nintro\n\n## Asakusa\n\nthe river\n\n## Nara\n\nthe deer\n",
    )
    .expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active(), "the plan opens");
    a.compose();
    (a, dir)
}

/// "Ask the AI…", the instruction typed, and Enter: the request is out.
fn ask(a: &mut App, instruction: &str) {
    a.plan_ai(AiAct::Ask);
    assert!(a.plan.is_none(), "the panel stands in front of the plan");
    let i = a.ai.instruction().expect("the split panel");
    assert!(i.split.is_some());
    i.field.set_text(instruction);
    press(a, KeyName::Enter);
}

/// The notes on the screen: first and last line (1-based) and title.
fn notes(a: &App) -> Vec<(usize, usize, String)> {
    let plan = &a.plan.as_ref().expect("the plan screen").plan;
    plan.notes()
        .iter()
        .map(|n| (n.first_line(), n.last_line(), n.title().to_string()))
        .collect()
}

#[test]
fn ask_is_offered_only_with_a_connection_chosen() {
    let (a, dir) = planned("split-ai-offer");
    assert!(a.plan.as_ref().expect("open").ai_offer);
    let _ = std::fs::remove_dir_all(&dir);

    let dir = scratch("split-ai-off");
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    let source = dir.join("Trip.md");
    std::fs::write(&source, "## One\n\na\n\n## Two\n\nb\n").expect("the source");
    open_now(&mut a, &source);
    assert!(a.plan_open_active());
    assert!(!a.plan.as_ref().expect("open").ai_offer, "AI is off");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_answer_becomes_the_plan_and_the_record_keeps_what_was_asked_and_sent() {
    let (mut a, dir) = planned("split-ai-ok");
    assert_eq!(notes(&a).len(), 2, "two chapters at headings");
    ask(&mut a, "SPLITOK one note for Tokyo");
    // What goes out is the outline, inside its tags, in one request.
    let run = a.ai.split.as_ref().expect("the request is out");
    assert!(
        run.prompt.contains("<document name=\"Trip.md\""),
        "{}",
        run.prompt
    );
    assert!(run.prompt.contains("SPLITOK one note for Tokyo"));
    let sent_bytes = run.prompt.len() as u64;
    assert!(a.plan.is_some(), "the plan screen is back while it waits");
    wait(&mut a, |a| a.ai.split.is_none());
    let after = notes(&a);
    // Lines 5-11 start at "## Asakusa": the heading's words are the title (check 8).
    assert_eq!(after.len(), 1, "{after:?}");
    assert_eq!((after[0].0, after[0].2.as_str()), (5, "Asakusa"));
    let cut = a
        .plan
        .as_ref()
        .expect("open")
        .ai_cut
        .as_ref()
        .expect("cut by the AI");
    assert_eq!(cut.requests, 1);
    assert_eq!(cut.bytes, sent_bytes);
    // Written, the record keeps the instruction and what was sent.
    press(&mut a, KeyName::Enter);
    assert!(a.plan.is_none(), "written");
    let runs = mdedit_core::runs::list(&dir.join("library"));
    assert_eq!(runs.len(), 1);
    let record = &runs[0].record;
    assert_eq!(record.instruction, "SPLITOK one note for Tokyo");
    let sent = record.sent.as_ref().expect("sent");
    assert_eq!(sent.connection, Connection::ClaudeApi.key());
    assert_eq!((sent.requests, sent.bytes), (1, sent_bytes));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_answer_that_cannot_be_read_is_asked_for_once_more_then_said() {
    let (mut a, dir) = planned("split-ai-junk");
    let before = notes(&a);
    ask(&mut a, "SPLITJUNK please");
    wait(&mut a, |a| {
        a.ai.split.as_ref().is_none_or(|r| r.requests == 2)
    });
    let run = a.ai.split.as_ref().expect("asked again");
    assert!(run.prompt.contains("SPLITJUNK please"), "the same request");
    wait(&mut a, |a| a.ai.split.is_none());
    let screen = a.plan.as_ref().expect("open");
    let notice = screen.ai_notice.as_ref().expect("said");
    assert_eq!(notice.text, "The AI\u{2019}s answer could not be read.");
    assert!(screen.ai_cut.is_none());
    assert_eq!(notes(&a), before, "the plan is as it was");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn escape_stops_the_request_and_leaves_the_plan_as_it_was() {
    let (mut a, dir) = planned("split-ai-stop");
    let before = notes(&a);
    ask(&mut a, "SPLITSLOW take your time");
    assert!(a.ai.split.is_some());
    assert!(a.plan.as_ref().expect("open").ai_notice.is_some(), "asking");
    press(&mut a, KeyName::Escape);
    assert!(a.ai.split.is_none(), "stopped");
    let screen = a.plan.as_ref().expect("the screen stays open");
    assert!(screen.ai_notice.is_none());
    assert_eq!(notes(&a), before);
    assert!(a.docs.ai_status.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancel_on_the_panel_brings_the_same_plan_back() {
    let (mut a, dir) = planned("split-ai-cancel");
    a.plan.as_mut().expect("open").toggle(1);
    a.plan_ai(AiAct::Ask);
    assert!(a.plan.is_none() && a.plan_behind.is_some());
    press(&mut a, KeyName::Escape);
    assert!(a.ai.panel.is_none());
    let screen = a.plan.as_ref().expect("the plan is back");
    assert!(
        !screen.plan.note(1).expect("a note").ticked(),
        "with its edit"
    );
    assert!(a.ai.split.is_none(), "nothing was sent");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_prompt_saved_from_the_split_panel_is_of_the_kind_split() {
    let (mut a, dir) = planned("split-ai-save");
    a.plan_ai(AiAct::Ask);
    a.ai.instruction()
        .expect("the panel")
        .field
        .set_text("One note per chapter");
    a.ai_save_prompt();
    let file = std::fs::read_to_string(dir.join(mdedit_core::ai::PROMPTS_FILE)).expect("saved");
    assert!(file.contains("kind: split"), "{file}");
    assert!(
        a.docs
            .ai_split_prompts
            .iter()
            .any(|p| p.instruction == "One note per chapter" && p.kind == PromptKind::Split)
    );
    assert!(
        a.docs
            .ai_prompts
            .iter()
            .all(|p| p.instruction != "One note per chapter"),
        "not among the rewrites"
    );
    assert!(matches!(&a.ai.panel, Some(Panel::Instruction(_))));
    let _ = std::fs::remove_dir_all(&dir);
}
