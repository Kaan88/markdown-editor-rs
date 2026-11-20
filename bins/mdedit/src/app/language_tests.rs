//! The language switch: the status bar's language field and the palette's "Language" open a
//! list above the field; a choice is the document's language, kept for the file and nothing
//! written to it. (A file the AI made Markdown: `ai_tests`.)

use super::files_tests::{app, open_now, scratch, type_text};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};

/// Opens the list from the status bar's field, types `name` and chooses the first row.
fn choose(a: &mut App, name: &str) {
    a.compose();
    a.activate(Hit::StatusLanguage);
    assert!(a.palette_open());
    type_text(a, name);
    a.key_pressed(Key::Name(KeyName::Enter), None);
    assert!(!a.palette_open());
}

fn lang(a: &App) -> &'static str {
    crate::languages::name_of(a.docs.active_ready().unwrap().language)
}

fn note(a: &App) -> String {
    a.palette.open.as_ref().unwrap().note.clone().unwrap()
}

#[test]
fn a_language_chosen_is_the_documents_and_comes_back_with_the_file() {
    let dir = scratch("language");
    let file = dir.join("notes.txt");
    let bytes = "# Notes\r\n\r\n- one\r\n";
    std::fs::write(&file, bytes).unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    assert_eq!(lang(&a), "Plain text");
    assert!(!a.docs.tabs[i].is_markdown());
    assert!(!super::preview::previewable(&a.docs.tabs[i]));

    // The status bar's field opens the list, standing just above it.
    a.compose();
    let field = a.hit_rect(Hit::StatusLanguage).expect("the language field");
    a.activate(Hit::StatusLanguage);
    let p = a.palette.open.as_ref().unwrap();
    assert!(p.anchored);
    // Plain text, then Markdown (the one with a preview), then the rest by name.
    let first: Vec<_> = p.palette.listed_ids()[..3]
        .iter()
        .map(|&id| p.target(id).cloned())
        .collect();
    use crate::palette::Target::Language as L;
    assert_eq!(first[0], Some(L(None)));
    assert_eq!(first[1], Some(L(crate::languages::markdown())));
    assert_eq!(p.palette.label_of(2), Some("ActionScript"));
    assert_eq!(
        note(&a),
        "From the extension: Plain text. A choice is kept for this file."
    );
    a.compose();
    let card = a.palette.open.as_ref().unwrap().palette.rect();
    assert!(
        card.bottom() <= field.y && card.bottom() > field.y - 12.0,
        "{card:?} {field:?}"
    );
    assert!(
        (card.right() - field.right()).abs() < 1.0,
        "{card:?} {field:?}"
    );
    a.key_pressed(Key::Name(KeyName::Escape), None);
    assert!(!a.palette_open());

    // Markdown: its preview, its tools; nothing written to the file.
    choose(&mut a, "Markdown");
    assert_eq!(lang(&a), "Markdown");
    let doc = &a.docs.tabs[i];
    assert!(doc.is_markdown() && super::preview::previewable(doc));
    assert!(!doc.modified());
    assert_eq!(std::fs::read_to_string(&file).unwrap(), bytes);
    assert_eq!(doc.ready().unwrap().place().lang, Some("Markdown"));
    // The list says so, and the palette's "Language" opens the same list.
    a.command(C::Language);
    assert_eq!(
        note(&a),
        "Kept for this file. Back to the extension's: Plain text."
    );
    a.key_pressed(Key::Name(KeyName::Escape), None);

    // Closed and opened again (as a session would), it is Markdown again.
    a.docs.close(i, None);
    let i = open_now(&mut a, &file);
    assert_eq!(lang(&a), "Markdown");
    assert!(a.docs.tabs[i].is_markdown());
    let snap = a.docs.places.snapshot(std::iter::once((
        file.as_path(),
        a.docs.tabs[i].ready().unwrap().place(),
    )));
    assert!(snap[0].1.to_setting().ends_with(",Markdown"));

    // The extension's own clears the choice.
    choose(&mut a, "Plain text");
    assert_eq!(lang(&a), "Plain text");
    let r = a.docs.active_ready().unwrap();
    assert!(!r.language_set && r.place().lang.is_none());
    assert!(!a.docs.tabs[i].is_markdown());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_chosen_language_outlasts_a_reload_and_follows_save_as() {
    let dir = scratch("language-reload");
    let file = dir.join("data.txt");
    std::fs::write(&file, "a = 1\n").unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    choose(&mut a, "TOML");
    assert_eq!(lang(&a), "TOML");
    a.docs.reload_active();
    a.docs.load_now(i, None, false);
    assert_eq!(lang(&a), "TOML");
    // A copy under another name with the same extension keeps the choice.
    a.docs
        .save_as(i, &dir.join("copy.txt"), None, false)
        .unwrap();
    assert_eq!(lang(&a), "TOML");
    assert!(a.docs.active_ready().unwrap().language_set);
    // Fable review #8: a new extension takes over.
    a.docs
        .save_as(i, &dir.join("data.json"), None, false)
        .unwrap();
    assert_eq!(lang(&a), "JSON");
    assert!(!a.docs.active_ready().unwrap().language_set);
    // A new name that gives the chosen language anyway clears the choice.
    choose(&mut a, "TOML");
    a.docs
        .save_as(i, &dir.join("data.toml"), None, false)
        .unwrap();
    let r = a.docs.active_ready().unwrap();
    assert_eq!(lang(&a), "TOML");
    assert!(!r.language_set);
    let _ = std::fs::remove_dir_all(&dir);
}
