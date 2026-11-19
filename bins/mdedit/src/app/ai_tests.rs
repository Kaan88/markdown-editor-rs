//! Headless tests of AI rewriting with a stand-in engine: a shell script that speaks the
//! engine's protocol and answers every rewrite with "# Done", or with a refused key when the
//! prompt says FAILPLEASE.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::ai::{BarAction, Hit as AiHit, Panel};
use crate::commands::CommandId as C;
use crate::keys::{self, KeyName};
use mdedit_core::ai::Connection;
use std::sync::Once;

const ENGINE: &str = r##"#!/bin/sh
echo '{"ev":"hello","version":1,"engine":"fake"}'
while IFS= read -r line; do
  id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
  case "$line" in
    *'"op":"run"'*FAILPLEASE*)
      echo "{\"ev\":\"error\",\"id\":$id,\"kind\":\"auth\",\"message\":\"Claude API refused the key (401).\"}";;
    *'"op":"run"'*)
      echo "{\"ev\":\"delta\",\"id\":$id,\"chars\":7}"
      printf '%s\n' "{\"ev\":\"done\",\"id\":$id,\"text\":\"# Done\\n\"}";;
    *'"op":"key_has"'*)
      echo "{\"ev\":\"key\",\"id\":$id,\"slot\":\"claude-api\",\"stored\":true}";;
    *'"op":"models"'*)
      echo "{\"ev\":\"models\",\"id\":$id,\"list\":[{\"id\":\"claude-test-1\",\"name\":\"Claude Test 1\"}]}";;
  esac
done
"##;

fn engine() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("ai-engine");
        let path = dir.join("fake-engine");
        std::fs::write(&path, ENGINE).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        // SAFETY: set once, before any test of this file starts an engine.
        unsafe { std::env::set_var("MDEDIT_AI_ENGINE", &path) };
    });
}

fn ai_app(text: &str) -> (App, PathBuf) {
    engine();
    let dir = scratch("ai");
    std::fs::write(dir.join("a.md"), text).unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.md"));
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    (a, dir)
}

/// Takes the engine's answers until `done` holds.
fn wait(a: &mut App, done: impl Fn(&App) -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !done(a) {
        assert!(Instant::now() < until, "the engine did not answer");
        a.ai_wake();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn text(a: &App) -> String {
    a.docs.active_ready().unwrap().buffer.rope().to_string()
}

#[test]
fn a_rewrite_is_reviewed_then_applied_as_one_undo_step() {
    let (mut a, dir) = ai_app("plain words\n");
    a.command(C::AiFormat);
    assert!(a.docs.ai_status.is_some());
    wait(&mut a, |a| a.ai.panel.is_some());
    assert!(a.docs.ai_status.is_none());
    let Some(Panel::Review(rv)) = &a.ai.panel else {
        panic!("a review")
    };
    assert!(rv.blocked.is_none());
    assert_eq!((rv.removed, rv.added), (1, 1));
    // The panel is drawn and takes the keyboard: Enter applies.
    a.compose();
    assert!(a.ai.hits.iter().any(|(h, _)| *h == AiHit::Apply));
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert!(a.ai.panel.is_none());
    assert_eq!(text(&a), "# Done\n");
    a.command(C::Undo);
    assert_eq!(text(&a), "plain words\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_review_the_result_goes_in_at_once() {
    let (mut a, dir) = ai_app("a\r\nb\r\n");
    a.docs.prefs.ai_review = false;
    a.command(C::AiFix);
    wait(&mut a, |a| !a.ai.running());
    assert!(a.ai.panel.is_none());
    // The file's line breaks are kept.
    assert_eq!(text(&a), "# Done\r\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn text_changed_meanwhile_is_not_overwritten() {
    let (mut a, dir) = ai_app("one\n");
    a.docs.prefs.ai_review = false;
    a.command(C::AiFormat);
    a.edit(EditOp::Paste("x"));
    wait(&mut a, |a| a.ai.panel.is_some());
    let Some(Panel::Review(rv)) = &a.ai.panel else {
        panic!("a review")
    };
    assert_eq!(
        rv.blocked.as_deref(),
        Some(mdedit_core::ai::BLOCKED_CHANGED)
    );
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    // Enter copies; nothing is put in.
    assert_eq!(text(&a), "xone\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failure_shows_the_bar_and_off_points_to_settings() {
    let (mut a, dir) = ai_app("text\n");
    a.ai_start_for_test("FAILPLEASE");
    wait(&mut a, |a| !a.ai.running());
    let bar = a.ai.bar.clone().expect("a bar");
    assert!(bar.text.contains("401"));
    assert_eq!(bar.action, Some(BarAction::OpenSettings));
    assert_eq!(text(&a), "text\n");
    a.ai.bar = None;
    a.docs.prefs.ai_connection = Connection::Off;
    a.command(C::AiFormat);
    assert!(!a.ai.running());
    assert_eq!(
        a.ai.bar.as_ref().and_then(|b| b.action),
        Some(BarAction::OpenSettings)
    );
    a.compose();
    assert!(a.hit_rect(Hit::AiBarGo).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_instruction_panel_runs_what_is_typed_and_escape_closes_it() {
    let (mut a, dir) = ai_app("text\n");
    a.command(C::AiRewrite);
    assert!(a.ai_panel_open());
    a.compose();
    a.key_pressed(keys::Key::Name(KeyName::Escape), None);
    assert!(!a.ai_panel_open());
    a.command(C::AiRewrite);
    for c in "shorter".chars() {
        let s = c.to_string();
        a.key_pressed(keys::Key::Name(KeyName::Char(c)), Some(&s));
    }
    // Typing went to the box, not the text.
    assert_eq!(text(&a), "text\n");
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert!(a.ai.running());
    assert_eq!(a.ai.last.as_ref().unwrap().instruction, "shorter");
    wait(&mut a, |a| a.ai.panel.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn settings_has_an_ai_group_that_asks_for_the_key() {
    let (mut a, dir) = ai_app("text\n");
    a.command(C::AiSettings);
    let f = a.ui.settings.as_ref().expect("settings");
    assert!(f.categories().contains(&"AI"));
    wait(&mut a, |a| a.ai.keys.get("claude-api") == Some(&true));
    // Audit 1: the key found, the models are asked for and arrive.
    wait(&mut a, |a| {
        a.ai.models.values().any(|m| {
            m.as_ref()
                .is_ok_and(|l| l.iter().any(|m| m.id == "claude-test-1"))
        })
    });
    let f = a.ui.settings.as_ref().unwrap();
    assert!(
        f.rows()
            .iter()
            .any(|r| r.id() == Some(super::modal::AI_KEY))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn review(a: &mut App) -> &mut crate::ai::Review {
    a.ai.review().expect("a review")
}

/// Audit 1, finding 2: text reloaded from disk after the review opened is not overwritten by
/// Apply; the review says why and offers Copy result.
#[test]
fn apply_does_not_overwrite_text_reloaded_meanwhile() {
    let (mut a, dir) = ai_app("one\n");
    a.command(C::AiFormat);
    wait(&mut a, |a| a.ai.panel.is_some());
    assert!(review(&mut a).blocked.is_none());
    // The file changes on disk and the clean tab reloads it.
    std::fs::write(dir.join("a.md"), "two\n").unwrap();
    let i = a.docs.active().unwrap();
    a.docs.load_now(i, None, false);
    assert_eq!(text(&a), "two\n");
    a.ai_review_action(AiHit::Apply);
    assert_eq!(text(&a), "two\n", "not overwritten");
    assert_eq!(
        review(&mut a).blocked.as_deref(),
        Some(mdedit_core::ai::BLOCKED_CHANGED)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 1, finding 3: Run again goes back to the review's document and waits for Rewrite;
/// it never sends whichever tab is active.
#[test]
fn run_again_is_for_the_reviewed_document_and_asks_first() {
    let (mut a, dir) = ai_app("alpha\n");
    let first = a.docs.active().unwrap();
    std::fs::write(dir.join("b.md"), "secret other text\n").unwrap();
    a.command(C::AiFormat);
    a.edit(EditOp::Paste("x"));
    let other = open_now(&mut a, &dir.join("b.md"));
    a.docs.activate(other);
    wait(&mut a, |a| a.ai.panel.is_some());
    let title = review(&mut a).spec.title.clone();
    a.ai_review_action(AiHit::RunAgain);
    assert_eq!(a.docs.active(), Some(first), "the reviewed document");
    assert!(!a.ai.running(), "nothing sent before Rewrite");
    let Some(Panel::Instruction(ins)) = &a.ai.panel else {
        panic!("the instruction panel")
    };
    assert_eq!(ins.scope, "The whole document");
    assert_eq!(
        ins.field.text(),
        a.ai.last.as_ref().unwrap().instruction.trim()
    );
    // Rewrite: the same instruction under its own title, on this document.
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert!(a.ai.running());
    let run = a.ai.run.as_ref().unwrap();
    assert_eq!(run.spec.title, title);
    assert_eq!(run.doc, a.docs.tabs[first].id);
    assert_eq!(run.original, "xalpha\n");
    wait(&mut a, |a| !a.ai.running());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 1: a read-only document with review off says the result cannot go in and shows it to
/// copy, rather than a "done" toast with nothing changed.
#[test]
fn a_read_only_document_shows_the_result_to_copy() {
    let (mut a, dir) = ai_app("one\n");
    a.docs.prefs.ai_review = false;
    let i = a.docs.active().unwrap();
    a.docs.tabs[i].ready_mut().unwrap().read_only = true;
    a.command(C::AiFormat);
    wait(&mut a, |a| !a.ai.running());
    assert_eq!(text(&a), "one\n");
    assert_eq!(
        review(&mut a).blocked.as_deref(),
        Some(mdedit_core::ai::BLOCKED_READ_ONLY)
    );
    let toast = a.docs.toast(Instant::now()).unwrap_or_default().to_string();
    assert!(!toast.contains("done"), "{toast}");
    let _ = std::fs::remove_dir_all(&dir);
}
