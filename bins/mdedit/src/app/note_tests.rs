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
    let before = read(&p.group);
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
    // Byte for byte what it was, with no blank line left where the link was (audit 4, C11).
    assert_eq!(group, before);
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

/// Review of the audit 4 fixes, A F14: the group's folder is compared with the library folder as
/// one folder on disk, not as spelled: `home: Recipes/..` is the library folder itself.
#[test]
fn a_group_whose_folder_is_the_library_folder_by_another_spelling_is_refused() {
    let (mut a, p) = lib("note-flat-dotdot");
    std::fs::create_dir_all(p.library.join("Recipes")).unwrap();
    std::fs::write(&p.group, "# Recipes\nhome: Recipes/..\n").unwrap();
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
fn a_source_edited_while_the_answer_was_out_is_not_written_from() {
    let (mut a, p) = lib("note-changed");
    open_card(&mut a, &p);
    answer(&mut a, "# Pasta\n");
    // Someone edited the file while the request was out: the note is not written from it.
    std::fs::write(&p.source, "PASTA, and now something else\n").unwrap();
    press(&mut a, KeyName::Enter);
    let card = a
        .note
        .as_ref()
        .expect("the card stays open, with the reason");
    let why = card.blocked.clone().unwrap_or_default();
    assert!(why.contains("has changed since the note was made"), "{why}");
    assert!(!p.library.join("Recipes").join("pasta-notes.md").exists());
    let _ = std::fs::remove_dir_all(&p.dir);
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

/// Audit 4, C3: the name the card chose was free when it opened; a file that takes it while the
/// answer is out is never written over, and the note gets the next free name, which its line
/// names. Before the fix the write failed, the card and the answer were gone, and a record of a
/// note never made was left in "AI runs…".
#[test]
fn a_name_taken_while_the_answer_was_out_gives_the_note_the_next_free_name() {
    let (mut a, p) = lib("note-taken-late");
    open_card(&mut a, &p);
    answer(&mut a, "# Pasta\n\nBoil it.\n");
    let first = target(&a);
    assert_eq!(first.file_name().unwrap(), "pasta-notes.md");
    std::fs::write(&first, "someone else's file\n").unwrap();
    press(&mut a, KeyName::Enter);
    assert!(a.note.is_none(), "written: {:?}", toast(&a));
    assert_eq!(read(&first), "someone else's file\n", "never written over");
    let second = p.library.join("Recipes").join("pasta-notes 2.md");
    assert_eq!(read(&second), "# Pasta\n\nBoil it.\n");
    assert!(
        read(&p.group).contains("(<Recipes/pasta-notes 2.md>)"),
        "{}",
        read(&p.group)
    );
    let (list, _) = mdedit_core::runs::list_all(&a.library_dir().unwrap());
    assert_eq!(list.len(), 1);
    assert!(list[0].record.notes[0].created);
    assert_eq!(list[0].record.notes[0].path, second);
    let _ = std::fs::remove_dir_all(&p.dir);
}

/// Audit 4, C3: a write that starts and then makes no note (here its folder cannot be made: a
/// file is where the folder's parent would be) keeps the card, the answer and a button that can
/// be pressed again, says why, writes no line, and leaves no record of a run that did nothing.
#[test]
fn a_write_that_makes_no_note_keeps_the_card_and_the_answer_and_leaves_no_record() {
    let (mut a, p) = lib("note-no-folder");
    // The group's folder is under `library/Recipes`, which is a file, not a folder.
    std::fs::remove_dir_all(p.library.join("Recipes")).unwrap();
    std::fs::write(p.library.join("Recipes"), "a file\n").unwrap();
    std::fs::write(&p.group, "# Recipes\nhome: Recipes/Pasta\n").unwrap();
    a.library_rows();
    let group_before = read(&p.group);
    open_card(&mut a, &p);
    assert!(
        a.note.as_ref().unwrap().blocked.is_none(),
        "nothing refuses it before the write: {:?}",
        a.note.as_ref().unwrap().blocked
    );
    answer(&mut a, "# Pasta\n\nBoil it.\n");
    press(&mut a, KeyName::Enter);
    let card = a.note.as_ref().expect("the card stays, with its answer");
    assert_eq!(card.result, "# Pasta\n\nBoil it.\n");
    assert_eq!(card.phase, Phase::Changes);
    let why = card.notice.clone().unwrap_or_default();
    assert!(why.contains("could not be written"), "{why}");
    assert!(why.contains("Pasta"), "the reason names what failed: {why}");
    assert!(card.problem().is_none(), "the button can be pressed again");
    assert_eq!(toast(&a), why, "and the toast says it");
    assert_eq!(read(&p.group), group_before, "no line was written");
    let library = a.library_dir().unwrap();
    let (list, newer) = mdedit_core::runs::list_all(&library);
    assert!(
        list.is_empty() && newer.is_empty(),
        "no record of a run that made nothing: {list:?}"
    );
    let runs_dir = library.join(".runs");
    let left: Vec<String> = std::fs::read_dir(&runs_dir)
        .map(|d| {
            d.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    assert!(left.is_empty(), "nothing left in .runs: {left:?}");
    // With the folder in its place, a second press writes the note.
    std::fs::remove_file(p.library.join("Recipes")).unwrap();
    press(&mut a, KeyName::Enter);
    assert!(a.note.is_none(), "written the second time: {:?}", toast(&a));
    assert_eq!(
        read(
            &p.library
                .join("Recipes")
                .join("Pasta")
                .join("pasta-notes.md")
        ),
        "# Pasta\n\nBoil it.\n"
    );
    let _ = std::fs::remove_dir_all(&p.dir);
}

/// Audit 4, C3: what a run that made no note leaves is taken away whole: the folders it made
/// (while they are empty) and its record with the record's log.
#[test]
fn a_run_that_made_no_note_takes_its_record_and_its_empty_folders_away() {
    let dir = scratch("note-discard");
    let runs = dir.join(".runs");
    std::fs::create_dir_all(&runs).unwrap();
    let record = runs.join("2026-11-24T10-00-00Z-markdown.json");
    std::fs::write(&record, "{}").unwrap();
    std::fs::write(record.with_extension("done"), "started\n").unwrap();
    let made = dir.join("Recipes");
    let deeper = made.join("Pasta");
    std::fs::create_dir_all(&deeper).unwrap();
    let kept = dir.join("Kept");
    std::fs::create_dir_all(&kept).unwrap();
    std::fs::write(kept.join("a.md"), "mine\n").unwrap();
    // Parents first, as a record lists the folders its run made.
    super::note::discard_run(Some(&record), &[made.clone(), deeper.clone(), kept.clone()]);
    assert!(!record.exists() && !record.with_extension("done").exists());
    assert!(
        !deeper.exists() && !made.exists(),
        "the empty folders the run made go"
    );
    assert!(
        kept.join("a.md").is_file(),
        "a folder holding anything stays"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Audit 4, C12: a source with bare CR line ends (an old Mac file) gives a note with LF ends,
/// and its lines are counted as lines.
#[test]
fn a_source_with_bare_carriage_returns_gives_a_note_with_line_feeds() {
    let (mut a, p) = lib("note-cr");
    std::fs::write(&p.source, "PASTA\r\rboil it.\rdrain it.\r").unwrap();
    open_card(&mut a, &p);
    let card = a.note.as_ref().unwrap();
    assert_eq!(card.source.eol, "\n");
    assert_eq!(card.source.lines, 4);
    answer(&mut a, "# Pasta\r\r- Boil it.\r- Drain it.\r");
    let note = target(&a);
    press(&mut a, KeyName::Enter);
    let written = std::fs::read(&note).unwrap();
    assert!(
        !written.contains(&b'\r'),
        "{:?}",
        String::from_utf8_lossy(&written)
    );
    assert_eq!(written, b"# Pasta\n\n- Boil it.\n- Drain it.\n");
    let _ = std::fs::remove_dir_all(&p.dir);
}

/// Audit 4, C12: the text on the card and the digest the write compares are of one read of the
/// file, so a file that changes between the two cannot give a card whose check passes against
/// the wrong text.
#[test]
fn the_cards_text_and_digest_are_of_the_same_bytes() {
    let (mut a, p) = lib("note-one-read");
    open_card(&mut a, &p);
    let card = a.note.as_ref().unwrap();
    let bytes = std::fs::read(&p.source).unwrap();
    assert_eq!(card.source.text.as_bytes(), bytes.as_slice());
    assert_eq!(card.source.len, bytes.len() as u64);
    assert_eq!(card.source.sha256, mdshared_app::update::sha256_hex(&bytes));
    let _ = std::fs::remove_dir_all(&p.dir);
}

/// Audit 4, C12: a refusal at the write that can pass on a second try (here the group filled to
/// 2,000 notes while the answer was out, then emptied again) keeps the card and its button, and
/// the second press writes the note. Before, it disabled the button for good.
#[test]
fn a_refusal_at_the_write_that_can_pass_leaves_the_button_on() {
    let (mut a, p) = lib("note-refused-then");
    open_card(&mut a, &p);
    answer(&mut a, "# Pasta\n\n- Boil it.\n");
    let note = target(&a);
    let before = read(&p.group);
    let mut full = before.clone();
    for n in 0..mdshared_app::library::MAX_ENTRIES {
        full.push_str(&format!("- [Note {n}](note-{n}.md)\n"));
    }
    std::fs::write(&p.group, &full).unwrap();
    press(&mut a, KeyName::Enter);
    let card = a.note.as_ref().expect("the card stays");
    assert!(card.blocked.is_none(), "the button stays on");
    assert!(card.notice.is_some(), "the footer says why");
    assert!(!note.exists());
    std::fs::write(&p.group, &before).unwrap();
    press(&mut a, KeyName::Enter);
    assert!(note.is_file(), "the second press writes it");
    let _ = std::fs::remove_dir_all(&p.dir);
}

/// D-211, pictures plan 4.4: after the note is written it opens in a tab with its preview
/// shown the Preview button's way, by default; "First note only" opens it and leaves the preview;
/// "Nothing" opens nothing.
#[test]
fn a_written_note_opens_with_its_preview_as_the_setting_says() {
    use mdedit_core::prefs::AfterRun;
    for (after, opened, preview) in [
        (AfterRun::Preview, true, vec![C::PreviewPane]),
        (AfterRun::Open, true, Vec::new()),
        (AfterRun::Nothing, false, Vec::new()),
    ] {
        let (mut a, p) = lib(&format!("note-after-{}", after.key()));
        a.docs.prefs.ai_after_run = after;
        engine();
        open_card(&mut a, &p);
        let note = target(&a);
        answer(&mut a, "# Done\n");
        let _ = super::preview::preview_asked();
        press(&mut a, KeyName::Enter);
        assert!(a.note.is_none(), "written");
        assert_eq!(read(&note), "# Done\n");
        let front = a.docs.active_doc().and_then(|d| d.path.clone());
        assert_eq!(
            front.as_deref() == Some(note.as_path()),
            opened,
            "{after:?}"
        );
        assert_eq!(super::preview::preview_asked(), preview, "{after:?}");
        let _ = std::fs::remove_dir_all(&p.dir);
    }
}
