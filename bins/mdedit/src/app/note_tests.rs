//! "A Markdown note from a file…" through the window (5.8): the group row's menu offers it, a
//! file picked becomes a card that says what is sent, the AI's answer becomes a new note in the
//! group's folder with the original untouched, the run is in "AI runs…" and its undo takes the
//! note out again. Every refusal it makes is here too.

use super::ai_tests::{engine, wait};
use super::files_tests::{open_now, scratch, type_text};
use super::library_write_tests::{app_in, press, read};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::KeyName;
use crate::note_ui::{Phase, View, demo_texts};
use crate::ui::LibAct;
use mdedit_core::ai::Connection;
use mdedit_core::runs::Kind;

struct Paths {
    dir: PathBuf,
    library: PathBuf,
    group: PathBuf,
    source: PathBuf,
}

/// A library with one group, `Recipes`, whose folder is `library/Recipes`, and a text file
/// beside the settings: the card's two ends.
fn lib(tag: &str) -> (App, Paths) {
    let dir = scratch(tag);
    let library = dir.join("library");
    std::fs::create_dir_all(library.join("Recipes")).unwrap();
    let group = library.join("Recipes.md");
    std::fs::write(&group, "# Recipes\nhome: Recipes\n").unwrap();
    let source = dir.join("pasta notes.txt");
    std::fs::write(&source, "PASTA\n\nboil it.\n").unwrap();
    let mut a = app_in(&dir);
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.command(C::ToggleLibraryPane);
    let paths = Paths {
        dir,
        library,
        group,
        source,
    };
    (a, paths)
}

/// Picks `p.source` for the group, as the picker would.
fn open_card(a: &mut App, p: &Paths) {
    a.note_file_picked(p.group.clone(), Some(p.source.clone()));
    assert!(a.note.is_some(), "the card is open: {:?}", toast(a));
    a.compose();
}

/// The answer, as the engine's would have come back.
fn answer(a: &mut App, text: &str) {
    let eol = a.note.as_ref().unwrap().source.eol.clone();
    a.note.as_mut().unwrap().set_answer(text.to_string(), &eol);
    a.compose();
}

fn toast(a: &App) -> String {
    a.docs
        .toast(Instant::now())
        .map(str::to_string)
        .unwrap_or_default()
}

fn target(a: &App) -> PathBuf {
    a.note.as_ref().expect("the card").target.path.clone()
}

#[test]
fn the_group_menu_offers_it_and_a_text_file_becomes_a_note_with_the_original_untouched() {
    let (mut a, p) = lib("note-write");
    // The row menu of the group has the command (the picker itself needs a desktop).
    let menu = crate::overlays::library_row_menu(
        &crate::library_pane::Kind::Group,
        Some("Recipes"),
        0,
        true,
        0.0,
        0.0,
        false,
    );
    assert!(
        menu.items
            .iter()
            .any(|i| i.id == Act::Library(LibAct::MarkdownNote, 0)),
        "the group's menu has Markdown note from a file…"
    );
    engine();
    open_card(&mut a, &p);
    let card = a.note.as_ref().unwrap();
    assert!(card.blocked.is_none(), "{:?}", card.blocked);
    assert_eq!(
        card.target.path.file_name().unwrap().to_string_lossy(),
        "pasta-notes.md",
        "the note is named from the file, in lower case with hyphens"
    );
    assert_eq!(
        card.target.path.parent(),
        Some(p.library.join("Recipes").as_path())
    );
    let note = target(&a);
    // The whole file is what is sent, with the built-in instruction: the request is on the card.
    assert!(
        card.prompt.contains("PASTA\n\nboil it.\n"),
        "{}",
        card.prompt
    );
    assert!(
        card.prompt.contains("Format this text as Markdown"),
        "{}",
        card.prompt
    );
    // Enter sends it to the engine; the answer becomes the note's text.
    press(&mut a, KeyName::Enter);
    assert_eq!(a.note.as_ref().unwrap().phase, Phase::Asking);
    wait(&mut a, |a| {
        a.note.as_ref().is_some_and(|c| c.phase == Phase::Changes)
    });
    assert_eq!(a.note.as_ref().unwrap().result, "# Done\n");
    // Enter writes it: the note is in the group's folder, listed in the group's own notes.
    press(&mut a, KeyName::Enter);
    assert!(a.note.is_none(), "the card closes when it writes");
    assert_eq!(read(&note), "# Done\n");
    let group = read(&p.group);
    assert!(
        group.contains("- [pasta notes](Recipes/pasta-notes.md)"),
        "{group}"
    );
    assert_eq!(
        read(&p.source),
        "PASTA\n\nboil it.\n",
        "the original is never changed"
    );
    // The run is a record of its own kind, and "AI runs…" lists it.
    let library = a.library_dir().unwrap();
    let (list, _) = mdedit_core::runs::list_all(&library);
    assert_eq!(list.len(), 1, "{list:?}");
    assert_eq!(list[0].record.kind, Kind::Markdown);
    assert_eq!(list[0].record.notes.len(), 1);
    assert!(list[0].record.notes[0].created);
    assert_eq!(list[0].record.source.path, p.source);
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn undo_from_ai_runs_takes_the_note_and_its_line_out_again() {
    let (mut a, p) = lib("note-undo");
    open_card(&mut a, &p);
    answer(&mut a, "# Pasta\n\n- Boil it.\n");
    let note = target(&a);
    press(&mut a, KeyName::Enter);
    assert!(note.is_file());
    let library = a.library_dir().unwrap();
    let (list, _) = mdedit_core::runs::list_all(&library);
    let record = list.first().expect("a record").path.clone();
    // The question first: it names what goes.
    a.run_undo_ask(record.clone());
    let sentence =
        a.ui.runs
            .undo
            .as_ref()
            .map(|u| u.1.clone())
            .unwrap_or_default();
    assert!(sentence.contains("1 note"), "{sentence}");
    a.run_undo(record);
    assert!(!note.exists(), "the note goes");
    let group = read(&p.group);
    assert!(!group.contains("pasta-notes"), "{group}");
    assert_eq!(read(&p.source), "PASTA\n\nboil it.\n");
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn undo_is_refused_while_the_original_is_not_the_file_the_note_was_made_from() {
    let (mut a, p) = lib("note-undo-changed");
    open_card(&mut a, &p);
    answer(&mut a, "# Pasta\n");
    let note = target(&a);
    press(&mut a, KeyName::Enter);
    assert!(note.is_file());
    // The file is edited after the note was made: the note may be the only copy of its text.
    std::fs::write(&p.source, "PASTA, but changed\n").unwrap();
    let library = a.library_dir().unwrap();
    let (list, _) = mdedit_core::runs::list_all(&library);
    let record = list.first().expect("a record").path.clone();
    a.run_undo_ask(record.clone());
    let sentence =
        a.ui.runs
            .undo
            .as_ref()
            .map(|u| u.1.clone())
            .unwrap_or_default();
    assert!(
        sentence.contains("not where it was") || sentence.contains("may be the only copy"),
        "{sentence}"
    );
    a.run_undo(record);
    assert!(
        note.is_file(),
        "nothing is removed while the original differs"
    );
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn a_name_that_is_taken_gets_a_second_one_and_the_line_names_that_one() {
    let (mut a, p) = lib("note-taken");
    std::fs::write(
        p.library.join("Recipes").join("pasta-notes.md"),
        "# Another note\n",
    )
    .unwrap();
    open_card(&mut a, &p);
    let note = target(&a);
    assert_eq!(note.file_name().unwrap(), "pasta-notes 2.md");
    answer(&mut a, "# Pasta\n");
    press(&mut a, KeyName::Enter);
    assert_eq!(read(&note), "# Pasta\n");
    assert_eq!(
        read(&p.library.join("Recipes").join("pasta-notes.md")),
        "# Another note\n",
        "the note that was there is never written over"
    );
    assert!(
        read(&p.group).contains("- [pasta notes](<Recipes/pasta-notes 2.md>)"),
        "{}",
        read(&p.group)
    );
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn with_the_ai_off_the_card_says_why_and_sends_nothing() {
    let (mut a, p) = lib("note-off");
    a.docs.prefs.ai_connection = Connection::Off;
    open_card(&mut a, &p);
    let why = a.note.as_ref().unwrap().blocked.clone().unwrap_or_default();
    assert!(why.contains("AI is off"), "{why}");
    press(&mut a, KeyName::Enter);
    assert_eq!(a.note.as_ref().unwrap().phase, Phase::Ready);
    assert!(!a.ai.running(), "no request was made");
    assert!(!p.library.join("Recipes").join("pasta-notes.md").exists());
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn a_file_too_long_for_one_request_is_refused_before_anything_opens() {
    let (mut a, p) = lib("note-long");
    std::fs::write(&p.source, "x".repeat(200_001)).unwrap();
    a.note_file_picked(p.group.clone(), Some(p.source.clone()));
    assert!(a.note.is_none());
    let why = toast(&a);
    assert!(why.contains("too long for one request"), "{why}");
    assert!(why.contains("200 KB"), "{why}");
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn a_file_that_is_not_text_is_refused() {
    let (mut a, p) = lib("note-binary");
    std::fs::write(&p.source, b"PASTA\0\0not text\0").unwrap();
    a.note_file_picked(p.group.clone(), Some(p.source.clone()));
    assert!(a.note.is_none());
    let why = toast(&a);
    assert!(why.contains("not a text file"), "{why}");
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn a_group_file_open_with_unsaved_changes_refuses_the_note_until_it_is_saved() {
    let (mut a, p) = lib("note-unsaved");
    open_now(&mut a, &p.group);
    type_text(&mut a, "x");
    open_card(&mut a, &p);
    let why = a.note.as_ref().unwrap().blocked.clone().unwrap_or_default();
    assert!(why.contains("saved"), "{why}");
    press(&mut a, KeyName::Enter);
    assert_eq!(a.note.as_ref().unwrap().phase, Phase::Ready);
    assert!(!p.library.join("Recipes").join("pasta-notes.md").exists());
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn a_group_whose_folder_is_the_library_folder_itself_is_refused() {
    let (mut a, p) = lib("note-flat");
    std::fs::write(&p.group, "# Recipes\nhome: .\n").unwrap();
    a.library_rows();
    open_card(&mut a, &p);
    let why = a.note.as_ref().unwrap().blocked.clone().unwrap_or_default();
    assert!(why.contains("the library folder itself"), "{why}");
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn escape_stops_the_request_and_keeps_the_card_and_a_second_escape_closes_it() {
    let (mut a, p) = lib("note-escape");
    open_card(&mut a, &p);
    a.note.as_mut().unwrap().set_asking(9_999);
    press(&mut a, KeyName::Escape);
    assert_eq!(
        a.note.as_ref().unwrap().phase,
        Phase::Ready,
        "the card stays"
    );
    press(&mut a, KeyName::Escape);
    assert!(a.note.is_none(), "with nothing out, Escape closes it");
    let _ = std::fs::remove_dir_all(&p.dir);
}

#[test]
fn the_result_switch_repaints_the_body_and_cancel_writes_nothing() {
    let (mut a, p) = lib("note-switch");
    open_card(&mut a, &p);
    answer(&mut a, "# Pasta\n\n- Boil it.\n");
    let body = a.note.as_ref().unwrap().geo().body;
    let before = body_pixels(&a.screen, body);
    let (x, y) = {
        let card = a.note.as_ref().unwrap();
        let (hit, r) = card
            .spots
            .iter()
            .find(|(h, _)| *h == crate::note_ui::Hit::Result)
            .expect("the Result switch is drawn");
        assert_eq!(*hit, crate::note_ui::Hit::Result);
        (r.cx(), r.cy())
    };
    a.pointer_moved(x, y);
    a.ui.mouse = (x, y);
    a.plan_press();
    a.plan_release();
    assert_eq!(a.note.as_ref().unwrap().view, View::Result);
    a.compose();
    let after = body_pixels(&a.screen, body);
    assert_ne!(before, after, "the switch repaints the body");
    // Cancel throws the answer away, and the file it would have become is not there.
    press(&mut a, KeyName::Escape);
    assert!(a.note.is_none());
    assert!(!p.library.join("Recipes").join("pasta-notes.md").exists());
    let _ = std::fs::remove_dir_all(&p.dir);
}

/// The screen's pixels in `area`.
fn body_pixels(screen: &Bitmap, area: Rect) -> Vec<u8> {
    let x0 = area.x.max(0.0) as u32;
    let y0 = area.y.max(0.0) as u32;
    let x1 = (area.right().max(0.0) as u32).min(screen.width);
    let y1 = (area.bottom().max(0.0) as u32).min(screen.height);
    let mut out = Vec::new();
    for y in y0..y1 {
        let o = ((y * screen.width + x0) * 4) as usize;
        let n = ((x1 - x0) * 4) as usize;
        out.extend_from_slice(&screen.pixels[o..o + n]);
    }
    out
}

#[test]
fn every_demonstration_opens_its_state_and_refuses_to_send_or_write() {
    let (mut a, p) = lib("note-demo");
    for name in crate::note_ui::NAMES {
        assert!(a.note_demo(name), "{name}");
        a.compose();
        let card = a.note.as_ref().expect("the card");
        assert!(card.demo);
        // Enter on a ready card sends nothing and says so; on a written one it writes nothing.
        press(&mut a, KeyName::Enter);
        assert!(a.note.is_some(), "{name}: the card stays");
        assert!(!a.ai.running(), "{name}: nothing was sent");
        assert!(!p.library.join("Recipes").join("pasta-notes.md").exists());
        a.note = None;
    }
    // The demonstration's own texts are Markdown made from prose, and the shots use them.
    let (from, to) = demo_texts();
    assert!(from.contains("PASTA NOTES"));
    assert!(to.contains("# Pasta notes"));
    let _ = std::fs::remove_dir_all(&p.dir);
}
