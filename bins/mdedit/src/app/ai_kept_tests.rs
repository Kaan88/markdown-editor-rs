//! Escape on an AI review keeps its result, as a stray click beside it does: it is shown again
//! by "AI: Show the last result" until it is applied, discarded, replaced, or the program ends.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::ai::{Hit as AiHit, Panel};
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use mdedit_core::ai::Connection;
use mdshared_ui::menu::{access_key_of, parse_access};

fn demo(tag: &str) -> (App, PathBuf) {
    let dir = scratch(tag);
    std::fs::write(dir.join("a.md"), "plain words\nand more\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.md"));
    a.ai_demo("review");
    assert!(
        matches!(a.ai.panel, Some(Panel::Review(_))),
        "a review is open"
    );
    a.compose();
    (a, dir)
}

fn esc(a: &mut App) {
    a.key_pressed(Key::Name(KeyName::Escape), None);
    a.compose();
}

fn frame(a: &mut App) -> Vec<u8> {
    a.compose();
    a.screen.pixels.to_vec()
}

fn text(a: &App) -> String {
    a.docs.active_ready().unwrap().buffer.rope().to_string()
}

#[test]
fn escape_closes_a_review_and_the_last_result_brings_it_back_as_it_was() {
    let (mut a, dir) = demo("aik-escape");
    let before = frame(&mut a);
    assert!(!a.docs.ai_kept);

    esc(&mut a);
    assert!(a.ai.panel.is_none(), "the review is closed");
    assert!(a.ai.kept.is_some() && a.docs.ai_kept, "its result is kept");
    assert_eq!(text(&a), "plain words\nand more\n", "nothing was applied");
    assert!(frame(&mut a) != before, "the panel is gone from the window");

    a.command(C::AiShowResult);
    assert!(matches!(a.ai.panel, Some(Panel::Review(_))));
    assert!(a.ai.kept.is_none() && !a.docs.ai_kept);
    assert!(frame(&mut a) == before, "the same pixels as before Escape");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_status_strip_says_a_result_is_ready_while_one_is_kept() {
    let (mut a, dir) = demo("aik-strip");
    assert_ne!(crate::status::fields(&a.docs).0, "AI result ready");
    esc(&mut a);
    assert_eq!(crate::status::fields(&a.docs).0, "AI result ready");
    a.command(C::AiShowResult);
    assert_ne!(crate::status::fields(&a.docs).0, "AI result ready");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stray_click_beside_the_review_keeps_it_open_as_escape_keeps_its_result() {
    let (mut a, dir) = demo("aik-click");
    let before = frame(&mut a);
    a.ui.mouse = (2.0, 2.0);
    assert!(a.ai.hit_at(2.0, 2.0).is_none());
    a.ai_press();
    a.ai_release();
    assert!(matches!(a.ai.panel, Some(Panel::Review(_))), "still open");
    assert!(frame(&mut a) == before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discard_throws_the_result_away_and_nothing_is_left_to_show() {
    let (mut a, dir) = demo("aik-discard");
    a.ai_review_action(AiHit::Discard);
    assert!(a.ai.panel.is_none() && a.ai.kept.is_none() && !a.docs.ai_kept);
    assert!(!crate::palette::enabled(
        C::AiShowResult,
        &a.docs,
        &a.ui,
        false
    ));
    a.command(C::AiShowResult);
    assert!(a.ai.panel.is_none(), "nothing came back");

    // Kept, shown again, then discarded: gone for good.
    a.ai_demo("review");
    esc(&mut a);
    a.command(C::AiShowResult);
    a.ai_review_action(AiHit::Discard);
    assert!(a.ai.kept.is_none() && !a.docs.ai_kept);
    a.command(C::AiShowResult);
    assert!(a.ai.panel.is_none());
    assert_eq!(text(&a), "plain words\nand more\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_result_shown_again_can_be_applied_and_then_none_is_kept() {
    let (mut a, dir) = demo("aik-apply");
    esc(&mut a);
    a.command(C::AiShowResult);
    a.key_pressed(Key::Name(KeyName::Enter), None);
    assert!(a.ai.panel.is_none() && a.ai.kept.is_none() && !a.docs.ai_kept);
    assert_ne!(text(&a), "plain words\nand more\n", "the result went in");
    assert!(!crate::palette::enabled(
        C::AiShowResult,
        &a.docs,
        &a.ui,
        false
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_result_replaces_the_kept_one() {
    let (mut a, dir) = demo("aik-replace");
    esc(&mut a);
    assert!(a.docs.ai_kept);
    a.ai_demo("review");
    assert!(
        a.ai.kept.is_none() && !a.docs.ai_kept,
        "the old result is gone"
    );
    assert!(
        matches!(a.ai.panel, Some(Panel::Review(_))),
        "the new one is shown"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_command_is_offered_only_while_a_result_is_kept() {
    let (mut a, dir) = demo("aik-offered");
    assert!(!crate::palette::enabled(
        C::AiShowResult,
        &a.docs,
        &a.ui,
        false
    ));
    esc(&mut a);
    assert!(crate::palette::enabled(
        C::AiShowResult,
        &a.docs,
        &a.ui,
        false
    ));
    // It is in the palette under its name.
    let named = crate::commands::COMMANDS
        .iter()
        .any(|c| c.id == C::AiShowResult && c.label == "AI: Show the last result");
    assert!(named);

    // And in the AI menu, with a key of its own, only then.
    let keys = |menu: &mdshared_ui::menu::Menu<Act>| {
        let mut seen = std::collections::HashSet::new();
        for item in &menu.items {
            if let Some(k) = access_key_of(&item.label) {
                assert!(
                    seen.insert(k.to_lowercase().next()),
                    "two with {k}: {}",
                    item.label
                );
            }
        }
    };
    let has = |menu: &mdshared_ui::menu::Menu<Act>| {
        menu.items.iter().any(|i| {
            i.id == Act::Cmd(C::AiShowResult)
                && parse_access(&i.label).text == "Show the last result"
        })
    };
    let menu = crate::overlays::ai_menu(&a.docs, 0.0, 0.0, false);
    assert!(has(&menu));
    keys(&menu);
    a.command(C::AiShowResult);
    assert!(!has(&crate::overlays::ai_menu(&a.docs, 0.0, 0.0, false)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn escape_on_the_instruction_panel_closes_it_and_leaves_a_kept_result_alone() {
    let (mut a, dir) = demo("aik-instruction");
    esc(&mut a);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.ai_open_instruction();
    assert!(matches!(a.ai.panel, Some(Panel::Instruction(_))));
    esc(&mut a);
    assert!(a.ai.panel.is_none());
    assert!(
        a.ai.kept.is_some() && a.docs.ai_kept,
        "the result is still kept"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
