//! Headless tests of AI rewriting with a stand-in engine: a shell script that speaks the
//! engine's protocol and answers every rewrite with "# Done", or with a refused key when the
//! prompt says FAILPLEASE. A split (`split_ai_tests`) is answered with a plan when its prompt
//! says SPLITOK, with words that are not one when it says SPLITJUNK, and never for SPLITSLOW.

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
    *'"op":"run"'*SPLITOK*)
      printf '%s%s%s\n' '{"ev":"done","id":' "$id" ',"text":"Here it is:\n{\"version\":1,\"notes\":[{\"from\":5,\"to\":11,\"title\":\"Both\",\"sections\":[\"Tokyo\"]}]}"}';;
    *'"op":"run"'*SPLITJUNK*)
      printf '%s%s%s\n' '{"ev":"done","id":' "$id" ',"text":"I would rather not."}';;
    *'"op":"run"'*SPLITSLOW*)
      ;;
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

pub(super) fn engine() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let dir = scratch("ai-engine");
        let script = dir.join("fake-engine");
        std::fs::write(&script, ENGINE).unwrap();
        #[cfg(unix)]
        let path = {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            script
        };
        // Windows: the same script, run by Git's bash from a batch file (MDEDIT_TEST_BASH
        // names another bash).
        #[cfg(windows)]
        let path = {
            let bash = std::env::var_os("MDEDIT_TEST_BASH")
                .map(PathBuf::from)
                .or_else(|| {
                    let pf = std::env::var_os("ProgramFiles")?;
                    Some(PathBuf::from(pf).join("Git").join("bin").join("bash.exe"))
                })
                .filter(|p| p.is_file())
                .expect("the AI tests need Git's bash (or MDEDIT_TEST_BASH)");
            let cmd = dir.join("fake-engine.cmd");
            let line = format!(
                "@\"{}\" \"{}\"
",
                bash.display(),
                script.display()
            );
            std::fs::write(&cmd, line).unwrap();
            cmd
        };
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
pub(super) fn wait(a: &mut App, done: impl Fn(&App) -> bool) {
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

/// Audit 1, round 2 (N3): Rewrite sends only what the panel showed. Another tab made active
/// meanwhile (a file opened or dropped) is shown first and waits for Rewrite again.
#[test]
fn rewrite_sends_only_the_scope_the_panel_showed() {
    let (mut a, dir) = ai_app("alpha\n");
    std::fs::write(dir.join("b.md"), "beta\n").unwrap();
    a.command(C::AiRewrite);
    for c in "shorter".chars() {
        let s = c.to_string();
        a.key_pressed(keys::Key::Name(KeyName::Char(c)), Some(&s));
    }
    let b = open_now(&mut a, &dir.join("b.md"));
    a.docs.activate(b);
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert!(
        !a.ai.running(),
        "nothing sent for a scope the panel did not show"
    );
    let Some(Panel::Instruction(ins)) = &a.ai.panel else {
        panic!("the panel stays")
    };
    assert!(
        ins.changed && ins.scope.contains("Changed since"),
        "{}",
        ins.scope
    );
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert!(a.ai.running());
    let run = a.ai.run.as_ref().unwrap();
    assert_eq!(
        (run.doc, run.original.as_str()),
        (a.docs.tabs[b].id, "beta\n")
    );
    wait(&mut a, |a| !a.ai.running());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 1, round 2 (N4): a key typed for one custom URL is dropped when the URL changes, and
/// the new URL's key is looked up once typing pauses.
#[test]
fn a_key_draft_does_not_follow_a_new_url() {
    use super::modal::{AI_BASE_URL, AI_KEY};
    use mdshared_ui::settings::Value;
    let (mut a, dir) = ai_app("t\n");
    a.docs.prefs.ai_connection = Connection::OpenAi;
    a.docs.prefs.ai_preset = "custom".into();
    a.docs.prefs.ai_customs = vec![mdedit_core::ai::CustomProvider {
        base_url: "https://a.example/v1".into(),
        ..Default::default()
    }];
    a.command(C::AiSettings);
    a.ai_setting(AI_KEY, Value::Text("sk-typed-for-a".into()));
    assert_eq!(a.ai.key_draft, "sk-typed-for-a");
    a.ai_setting(AI_BASE_URL, Value::Text("https://b.example/v1".into()));
    assert!(a.ai.key_draft.is_empty(), "the draft was for A");
    let slot_b = "openai:custom:https://b.example/v1";
    assert!(!a.ai.keys.contains_key(slot_b) && !a.ai.pending_key(slot_b));
    // A's answer comes in while B's pause runs: B is not looked up before it, nor its models
    // asked for (audit 1 round 3, #8).
    wait(&mut a, |a| {
        a.ai.keys.contains_key("openai:custom:https://a.example/v1")
    });
    assert!(a.ai.url_due.is_some());
    assert!(!a.ai.keys.contains_key(slot_b) && !a.ai.pending_key(slot_b));
    assert!(a.ai.fetching.is_none());
    a.timers(Instant::now() + Duration::from_secs(1));
    wait(&mut a, |a| a.ai.keys.contains_key(slot_b));
    let _ = std::fs::remove_dir_all(&dir);
}

/// The user: custom providers need no API key, and there can be more than one. Each is added
/// from the Provider list, has its own name, URL and model, is chosen like any other provider
/// and can be removed; with no key stored its models are still asked for, and the run goes to
/// its URL with no key.
#[test]
fn custom_providers_are_added_chosen_and_removed_and_need_no_key() {
    use super::modal::{AI_BASE_URL, AI_CUSTOM_NAME, AI_CUSTOM_REMOVE, AI_MODEL_NAME, AI_PROVIDER};
    use mdshared_ui::settings::{Row, Value};
    let (mut a, dir) = ai_app("t\n");
    a.docs.prefs.ai_connection = Connection::OpenAi;
    a.command(C::AiSettings);
    let presets = mdedit_core::ai::PRESETS
        .iter()
        .filter(|x| !x.base_url.is_empty())
        .count();
    let rows_of = |a: &mut App| -> Vec<String> {
        a.settings_rows()
            .iter()
            .filter_map(|r| match r {
                Row::Choice { options, id, .. } if *id == AI_PROVIDER => {
                    Some(format!("Provider: {}", options.join("|")))
                }
                Row::Text { label, .. } | Row::Action { label, .. } => Some(label.to_string()),
                _ => None,
            })
            .collect()
    };
    let provider = |rows: &[String]| {
        rows.iter()
            .find(|r| r.starts_with("Provider: "))
            .cloned()
            .unwrap()
    };
    assert!(provider(&rows_of(&mut a)).ends_with("|Add a custom provider…"));
    // Add two.
    a.ai_setting(AI_PROVIDER, Value::Choice(presets));
    a.ai_setting(AI_CUSTOM_NAME, Value::Text("LM Studio".into()));
    a.ai_setting(AI_BASE_URL, Value::Text("http://localhost:1234/v1".into()));
    a.ai_setting(AI_MODEL_NAME, Value::Text("qwen".into()));
    a.ai_setting(AI_PROVIDER, Value::Choice(presets + 1));
    a.ai_setting(AI_BASE_URL, Value::Text("http://box:11434/v1".into()));
    let p = &a.docs.prefs;
    assert_eq!(p.ai_customs.len(), 2);
    assert_eq!((p.ai_custom, p.ai_url()), (1, "http://box:11434/v1"));
    // Named by the first free number: "LM Studio" leaves 1 free.
    assert_eq!(p.ai_customs[1].name, "Custom provider 1");
    let rows = rows_of(&mut a);
    assert!(provider(&rows).ends_with("|LM Studio|Custom provider 1|Add a custom provider…"));
    assert!(rows.iter().any(|r| r == "Remove this provider"));
    // Choose the first again: its own URL and model.
    a.ai_setting(AI_PROVIDER, Value::Choice(presets));
    assert_eq!(a.docs.prefs.ai_url(), "http://localhost:1234/v1");
    assert_eq!(a.docs.prefs.ai_model(Connection::OpenAi), "qwen");
    assert!(a.ai_via().starts_with("LM Studio"));
    // No key stored: the key row says it is optional and the models are still asked for.
    a.timers(Instant::now() + Duration::from_secs(1));
    let slot = "openai:custom:http://localhost:1234/v1";
    a.ai.keys.insert(slot.into(), false);
    a.ai.models.clear();
    a.ai.fetching = None;
    a.ai_settings_needs();
    assert!(
        a.ai.fetching.is_some(),
        "the models of a server that takes no key"
    );
    let help = a
        .settings_rows()
        .iter()
        .find_map(|r| match r {
            Row::Text { label, help, .. } if label.as_str() == "API key" => Some(help.to_string()),
            _ => None,
        })
        .unwrap();
    assert!(help.starts_with("Optional."), "{help}");
    // What a run is sent: this URL, and its slot (the engine sends a key only if one is stored).
    let conn = crate::ai::conn_json(Connection::OpenAi, &a.docs.prefs);
    assert_eq!(conn["base_url"], "http://localhost:1234/v1");
    assert_eq!(conn["slot"], slot);
    // A local preset is never looked up in the store.
    a.docs.prefs.ai_preset = "ollama".into();
    assert_eq!(
        crate::ai::conn_json(Connection::OpenAi, &a.docs.prefs)["slot"],
        ""
    );
    a.ai_setting(AI_PROVIDER, Value::Choice(presets + 1));
    // Remove the chosen one: no other provider uses its server, so its key is cleared from the
    // store (a provider added later on that URL must not send it unasked).
    let box_slot = "openai:custom:http://box:11434/v1";
    wait(&mut a, |a| !a.ai.pending_key(box_slot));
    a.ai_settings_action(AI_CUSTOM_REMOVE);
    assert!(a.ai.pending_key(box_slot), "the key is cleared");
    assert_eq!(a.docs.prefs.ai_customs.len(), 1);
    assert_eq!(a.docs.prefs.ai_url(), "http://localhost:1234/v1");
    // A new one is named by the first free number, not the count.
    a.ai_setting(AI_PROVIDER, Value::Choice(presets + 1));
    assert_eq!(a.docs.prefs.ai_customs[1].name, "Custom provider 1");
    // On the first one's server too: removing it leaves the key the other still uses.
    a.ai_setting(AI_BASE_URL, Value::Text("http://localhost:1234/v1".into()));
    a.timers(Instant::now() + Duration::from_secs(1));
    wait(&mut a, |a| !a.ai.pending_key(slot));
    a.ai_settings_action(AI_CUSTOM_REMOVE);
    assert!(
        !a.ai.pending_key(slot),
        "another provider still uses the key"
    );
    // The last one: the provider falls back to the first preset.
    a.ai_settings_action(AI_CUSTOM_REMOVE);
    assert!(a.docs.prefs.ai_customs.is_empty());
    assert_eq!(a.docs.prefs.ai_preset, "openai");
    let _ = std::fs::remove_dir_all(&dir);
}

fn hit_shown(a: &App, h: AiHit) -> bool {
    a.ai.hits.iter().any(|(x, _)| *x == h)
}

/// Audit 1, round 2 (N6): a document closed while its rewrite runs still gets the review, to
/// copy the result; closed while the review is open, Run again keeps the review.
#[test]
fn a_closed_document_keeps_its_result_to_copy() {
    let (mut a, dir) = ai_app("one\n");
    a.command(C::AiFormat);
    let i = a.docs.active().unwrap();
    a.docs.close(i, None);
    wait(&mut a, |a| a.ai.panel.is_some());
    assert_eq!(
        review(&mut a).blocked.as_deref(),
        Some(mdedit_core::ai::BLOCKED_CLOSED)
    );
    assert_eq!(review(&mut a).result, "# Done\n");
    a.compose();
    assert!(hit_shown(&a, AiHit::Copy));
    assert!(!hit_shown(&a, AiHit::RunAgain) && !hit_shown(&a, AiHit::Apply));
    // Closed while the review is open: Run again does not drop it.
    a.ai.panel = None;
    open_now(&mut a, &dir.join("a.md"));
    a.command(C::AiFormat);
    wait(&mut a, |a| a.ai.panel.is_some());
    let i = a.docs.active().unwrap();
    a.docs.close(i, None);
    // Drawn before anything is pressed: the review already knows (audit 1 round 3, #9).
    a.compose();
    assert!(hit_shown(&a, AiHit::Copy));
    assert!(!hit_shown(&a, AiHit::Apply), "Apply for a closed document");
    assert_eq!(
        review(&mut a).blocked.as_deref(),
        Some(mdedit_core::ai::BLOCKED_CLOSED)
    );
    // A copy that fails keeps the review and its result (audit 1 round 3, #3).
    super::ai_events::CLIPBOARD_FAILS.with(|f| f.set(true));
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    super::ai_events::CLIPBOARD_FAILS.with(|f| f.set(false));
    assert_eq!(review(&mut a).result, "# Done\n");
    a.ai_review_action(AiHit::RunAgain);
    assert_eq!(
        review(&mut a).blocked.as_deref(),
        Some(mdedit_core::ai::BLOCKED_CLOSED)
    );
    a.compose();
    assert!(!hit_shown(&a, AiHit::RunAgain));
    let _ = std::fs::remove_dir_all(&dir);
}

/// What is typed in the instruction box is saved as a prompt (Ctrl+S, or Save prompt), named by
/// its first words, and listed at once; the same text again is not saved twice.
#[test]
fn the_instruction_is_saved_as_a_prompt() {
    let (mut a, dir) = ai_app("text\n");
    a.session_path = Some(dir.join("session.ini"));
    a.command(C::AiRewrite);
    for c in "make the prices a table".chars() {
        let s = c.to_string();
        a.key_pressed(keys::Key::Name(KeyName::Char(c)), Some(&s));
    }
    a.compose();
    assert!(a.ai.hits.iter().any(|(h, _)| *h == AiHit::SavePrompt));
    a.mods.ctrl = true;
    a.key_pressed(keys::Key::Name(KeyName::Char('s')), None);
    a.mods.ctrl = false;
    let file = std::fs::read_to_string(dir.join(mdedit_core::ai::PROMPTS_FILE)).unwrap();
    assert!(
        file.contains("## make the prices a table\n\nmake the prices a table\n"),
        "{file}"
    );
    let last = a.docs.ai_prompts.last().unwrap();
    assert_eq!(last.name, "make the prices a table");
    // The panel stays open, the prompt picked, nothing sent.
    assert!(a.ai_panel_open() && !a.ai.running());
    assert_eq!(
        a.ai.instruction().unwrap().selected,
        Some(a.docs.ai_prompts.len() - 1)
    );
    let n = a.docs.ai_prompts.len();
    a.ai_save_prompt();
    assert_eq!(a.docs.ai_prompts.len(), n);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The owner: a saved prompt's Edit puts it in the instruction box to be changed, and runs
/// nothing: the button in the instruction panel's list, and the word at the right of its row in
/// the AI menu (where the rest of the row, its play icon included, runs it).
#[test]
fn a_saved_prompts_pencil_puts_it_in_the_box_to_be_changed() {
    use crate::ui::{Act, PopupKind};
    let (mut a, dir) = ai_app("text\n");
    a.session_path = Some(dir.join("session.ini"));
    std::fs::write(
        dir.join(mdedit_core::ai::PROMPTS_FILE),
        "# Prompts\n\n## Tables\n\nmake the prices a table\n\n## Short\n\nshorten it\n",
    )
    .unwrap();
    let instruction = |a: &mut App| {
        a.ai.instruction()
            .map(|i| (i.field.text().to_string(), i.selected))
    };
    // The panel's pencil.
    a.command(C::AiRewrite);
    a.compose();
    let pencil =
        a.ai.hits
            .iter()
            .find(|(h, _)| *h == AiHit::EditPrompt(1))
            .map(|(_, r)| *r)
            .expect("a pencil on the second prompt");
    a.pointer_moved(pencil.cx(), pencil.cy());
    a.left_press();
    a.left_release();
    assert_eq!(
        instruction(&mut a),
        Some(("shorten it".to_string(), Some(1)))
    );
    assert!(!a.ai.running(), "nothing sent");
    // A click on the row beside the pencil still runs the prompt.
    let row =
        a.ai.hits
            .iter()
            .find(|(h, _)| *h == AiHit::Prompt(0))
            .map(|(_, r)| *r)
            .unwrap();
    a.pointer_moved(row.x + 20.0, row.cy());
    a.left_press();
    a.left_release();
    assert!(a.ai.running() && !a.ai_panel_open());
    wait(&mut a, |a| !a.ai.running());

    // The AI menu: the word Edit at the row's right opens the panel with the prompt in it.
    a.ai.panel = None;
    a.compose();
    let text_area = a.hit_rect(crate::ui::Hit::Text).expect("the text");
    a.ui.mouse = (text_area.cx(), text_area.cy());
    a.right_press();
    a.open_sub(PopupKind::Text, PopupKind::Ai, false);
    a.compose();
    let row =
        a.ui.popup
            .as_ref()
            .unwrap()
            .sub
            .as_ref()
            .unwrap()
            .menu
            .row_rect(3);
    a.pointer_moved(row.right() - 20.0, row.cy());
    a.left_release();
    assert_eq!(
        instruction(&mut a),
        Some(("make the prices a table".to_string(), Some(0)))
    );
    assert!(!a.ai.running(), "nothing sent");
    // The play icon at the left runs the prompt, as the rest of the row does.
    a.ai.panel = None;
    a.compose();
    a.ui.mouse = (text_area.cx(), text_area.cy());
    a.right_press();
    a.open_sub(PopupKind::Text, PopupKind::Ai, false);
    a.compose();
    let menu = &a.ui.popup.as_ref().unwrap().sub.as_ref().unwrap().menu;
    let row = menu.row_rect(4);
    a.pointer_moved(row.x + 18.0, row.cy());
    a.left_release();
    assert!(a.ai.running() && !a.ai_panel_open());
    wait(&mut a, |a| !a.ai.running());
    // The command it is.
    a.ai.panel = None;
    a.act(Act::AiEditPrompt(1));
    assert_eq!(
        instruction(&mut a),
        Some(("shorten it".to_string(), Some(1)))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// An instruction panel with `typed` in its field, its prompts file in `dir`.
fn r5_panel(typed: &str) -> (App, PathBuf, PathBuf) {
    let (mut a, dir) = ai_app("text\n");
    a.session_path = Some(dir.join("session.ini"));
    a.command(C::AiRewrite);
    a.ai.instruction().unwrap().field.set_text(typed);
    let file = dir.join(mdedit_core::ai::PROMPTS_FILE);
    (a, dir, file)
}

/// Audit 1 round 5: with no prompts file yet, an instruction the template has is found in it
/// once the file is made, not saved a second time.
#[test]
fn a_prompt_the_template_has_is_not_saved_twice() {
    let (mut a, dir, file) = r5_panel("Shorten");
    let shorten = mdedit_core::ai::parse_prompts(mdedit_core::ai::PROMPTS_TEMPLATE)
        .into_iter()
        .find(|p| p.name == "Shorten")
        .unwrap();
    a.ai.instruction()
        .unwrap()
        .field
        .set_text(&shorten.instruction);
    assert!(!file.exists());
    a.ai_save_prompt();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        mdedit_core::ai::PROMPTS_TEMPLATE
    );
    assert!(
        a.docs
            .toast(Instant::now())
            .unwrap()
            .starts_with("Already saved")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 1 round 5: the prompts file open in a tab with changes not saved is not written under
/// them; nothing is saved until the tab is saved or closed.
#[test]
fn a_prompt_is_not_saved_under_unsaved_changes_to_the_file() {
    let (mut a, dir, file) = r5_panel("Say it twice");
    std::fs::write(&file, "## A\n\nDo a.\n").unwrap();
    let k = a.docs.open(&file);
    a.docs.load_now(k, None, false);
    a.docs.activate(k);
    assert!(a.docs.edit(EditOp::Insert("edited "), None));
    a.ai_save_prompt();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "## A\n\nDo a.\n");
    assert!(
        a.docs
            .toast(Instant::now())
            .unwrap()
            .starts_with("Save or close")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 1 round 5: a file left inside a code block, or one the prompt would take over the
/// size the list reads, is not written, and says why.
#[test]
fn a_prompt_that_would_not_read_back_is_not_saved() {
    let (mut a, dir, file) = r5_panel("Say it twice");
    let open_fence = "## A\n\nDo a.\n```\ncode\n";
    std::fs::write(&file, open_fence).unwrap();
    a.ai_save_prompt();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), open_fence);
    assert!(
        a.docs
            .toast(Instant::now())
            .unwrap()
            .contains("would not read")
    );
    // Near the limit.
    let big = format!("## A\n\n{}\n", "x".repeat(256 * 1024 - 20));
    std::fs::write(&file, &big).unwrap();
    a.ai_save_prompt();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), big);
    assert!(a.docs.toast(Instant::now()).unwrap().contains("KB"));
    // A CRLF file keeps its bytes and gets CRLF.
    let crlf = "## A\r\n\r\nDo a.   \r\n";
    std::fs::write(&file, crlf).unwrap();
    a.ai_save_prompt();
    let out = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        out,
        format!("{crlf}\r\n## Say it twice\r\n\r\nSay it twice\r\n")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A text file the AI formats as Markdown is Markdown at once (preview, tools), and is offered
/// a `.md` name: a name that is taken is refused, the rename keeps the tab and its undo.
#[test]
fn a_text_file_formatted_as_markdown_is_markdown_and_offered_its_name() {
    engine();
    let dir = scratch("ai-md");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "plain words\n").unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    assert!(!a.docs.tabs[i].is_markdown());
    a.command(C::AiFormat);
    wait(&mut a, |a| a.ai.panel.is_some());
    a.compose();
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert_eq!(text(&a), "# Done\n");
    let id = a.docs.tabs[i].id;
    assert!(a.docs.tabs[i].is_markdown());
    assert!(super::preview::previewable(&a.docs.tabs[i]));
    let (name, to) = a.docs.rename_bar().expect("the rename bar");
    assert_eq!(name, "notes.txt");
    assert_eq!(to.file_name().unwrap(), "notes.md");
    a.compose();
    assert!(a.hit_rect(Hit::RenameGo).is_some());

    // The name is taken: nothing is renamed, and it says so.
    let md = dir.join("notes.md");
    std::fs::write(&md, "another file").unwrap();
    a.activate(Hit::RenameGo);
    assert!(file.exists());
    assert_eq!(std::fs::read_to_string(&md).unwrap(), "another file");
    assert!(
        a.docs.tabs[i]
            .path
            .as_deref()
            .is_some_and(|p| p.ends_with("notes.txt"))
    );
    assert!(
        a.docs
            .toast(Instant::now())
            .unwrap()
            .contains("already exists")
    );
    assert!(a.docs.rename_bar().is_none());

    // Free again and offered again: the file is renamed; the tab, its unsaved text and its undo
    // stay.
    std::fs::remove_file(&md).unwrap();
    a.docs.rename_offer = Some(id);
    a.activate(Hit::RenameGo);
    assert!(!file.exists() && md.exists());
    assert_eq!(a.docs.tabs[i].id, id);
    assert!(
        a.docs.tabs[i]
            .path
            .as_deref()
            .is_some_and(|p| p.ends_with("notes.md"))
    );
    assert!(a.docs.tabs[i].modified());
    let r = a.docs.tabs[i].ready().unwrap();
    assert!(!r.language_set, "the name gives Markdown now");
    assert!(a.docs.rename_bar().is_none());
    // Saved under the new name without a question.
    a.docs.save(i, false).unwrap();
    assert_eq!(std::fs::read_to_string(&md).unwrap(), "# Done\n");
    a.command(C::Undo);
    assert_eq!(text(&a), "plain words\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A new note the AI makes Markdown has no file to rename, so Save offers it a `.md` name
/// instead of the `.txt` every new document used to be offered.
#[test]
fn a_new_note_made_markdown_is_offered_a_md_name() {
    engine();
    let mut a = app();
    a.command(C::NewFile);
    let i = a.docs.active().unwrap();
    super::files_tests::type_text(&mut a, "plain words");
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    let offered = |a: &App| {
        let doc = a.docs.active_doc().unwrap();
        a.offered_save_name(doc)
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_string)
    };
    assert!(!a.docs.tabs[i].is_markdown());
    assert_eq!(offered(&a).as_deref(), Some("txt"));

    a.command(C::AiFormat);
    wait(&mut a, |a| a.ai.panel.is_some());
    a.compose();
    a.key_pressed(keys::Key::Name(KeyName::Enter), None);
    assert_eq!(text(&a), "# Done");
    assert!(a.docs.tabs[i].is_markdown());
    // Nothing to rename (no file yet), and the name Save opens on is the note's, as Markdown.
    assert!(a.docs.rename_bar().is_none());
    assert_eq!(offered(&a).as_deref(), Some("md"));
    let doc = a.docs.active_doc().unwrap();
    let offered = a.offered_save_name(doc);
    assert_eq!(
        offered.file_stem().and_then(|s| s.to_str()),
        Some(doc.name().as_str())
    );
}

/// Fable review #3: undoing the AI's change takes back the switch to Markdown and the bar, and
/// nothing is kept for the file.
#[test]
fn undoing_the_conversion_takes_the_switch_back() {
    engine();
    let dir = scratch("ai-md-undo");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "plain words\n").unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.docs.prefs.ai_review = false;
    a.command(C::AiFormat);
    wait(&mut a, |a| !a.ai.running());
    assert!(a.docs.tabs[i].is_markdown() && a.docs.rename_bar().is_some());
    a.command(C::Undo);
    a.compose();
    assert_eq!(text(&a), "plain words\n");
    let r = a.docs.tabs[i].ready().unwrap();
    assert_eq!(crate::languages::name_of(r.language), "Plain text");
    assert!(r.place().lang.is_none());
    assert!(!a.docs.tabs[i].is_markdown());
    assert!(a.docs.rename_bar().is_none() && a.docs.rename_offer.is_none());
    // Redone, the text is back but the language stays as chosen since (plain).
    a.command(C::Redo);
    a.compose();
    assert_eq!(text(&a), "# Done\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Fable review #5 and #9: a file no longer on disk is renamed by its tab's name alone; the
/// bookmarks the old name left and the find results follow the new name.
#[test]
fn a_rename_follows_through_and_needs_no_file_on_disk() {
    engine();
    let dir = scratch("ai-md-gone");
    let file = dir.join("notes.txt");
    std::fs::write(&file, "plain words\n").unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    let file = a.docs.tabs[i].path.clone().unwrap();
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.docs.prefs.ai_review = false;
    a.command(C::AiFormat);
    wait(&mut a, |a| !a.ai.running());
    a.docs.marks.put(&file, &[0]);
    a.ui.results.rows.push(crate::results::Row::File {
        path: Some(file.clone()),
        doc: None,
        name: "notes.txt".into(),
        count: 1,
        more: false,
        stale: false,
    });
    std::fs::remove_file(&file).unwrap();
    a.compose();
    a.activate(Hit::RenameGo);
    let md = dir.join("notes.md");
    let path = a.docs.tabs[i].path.clone().unwrap();
    assert!(path.ends_with("notes.md"), "{path:?}");
    assert!(!md.exists(), "nothing written until it is saved");
    assert!(a.docs.toast(Instant::now()).unwrap().starts_with("Renamed"));
    assert_eq!(a.docs.marks.lines(&path), Some(&[0][..]));
    assert!(a.docs.marks.lines(&file).is_none());
    match &a.ui.results.rows[0] {
        crate::results::Row::File { path: p, name, .. } => {
            assert_eq!(p.as_deref(), Some(path.as_path()));
            assert_eq!(name, "notes.md");
        }
        _ => panic!("a file row"),
    }
    a.docs.save(i, false).unwrap();
    assert_eq!(std::fs::read_to_string(&md).unwrap(), "# Done\n");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Markdown formatted as Markdown, or a part of a text file, changes no language.
#[test]
fn only_a_whole_text_file_formatted_is_switched() {
    let (mut a, dir) = ai_app("plain words\n");
    a.docs.prefs.ai_review = false;
    a.command(C::AiFormat);
    wait(&mut a, |a| !a.ai.running());
    assert!(a.docs.rename_bar().is_none() && a.docs.rename_offer.is_none());

    let file = dir.join("part.txt");
    std::fs::write(&file, "one\ntwo\n").unwrap();
    let i = open_now(&mut a, &file);
    a.docs.tabs[i].ready_mut().unwrap().view.sel = mdedit_core::Selection {
        anchor: 0,
        caret: 3,
        goal_col: None,
    };
    a.command(C::AiFormat);
    wait(&mut a, |a| !a.ai.running());
    assert!(!a.docs.tabs[i].is_markdown());
    assert!(a.docs.rename_offer.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_run_drops_the_result_escape_kept() {
    let (mut a, dir) = ai_app("plain words\n");
    a.command(C::AiFormat);
    wait(&mut a, |a| a.ai.panel.is_some());
    a.key_pressed(keys::Key::Name(KeyName::Escape), None);
    assert!(a.ai.panel.is_none() && a.docs.ai_kept && a.ai.kept.is_some());
    // Another run: the old result is no longer the last one, from the moment it starts.
    a.command(C::AiFormat);
    assert!(a.ai.running());
    assert!(a.ai.kept.is_none() && !a.docs.ai_kept);
    wait(&mut a, |a| a.ai.panel.is_some());
    assert!(a.ai.kept.is_none() && !a.docs.ai_kept);
    let _ = std::fs::remove_dir_all(&dir);
}
