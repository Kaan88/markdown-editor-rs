//! The AI task from a row of the Notes pane (`docs/ai-pictures-plan.md` 4.2, phase L): the line
//! at the head of each row's menu, the notes it takes, folded or not, the window it opens, and
//! "AI task…" taking the row the keyboard is on.

use super::library_write_tests::{labels, lib, pick, toast};
use super::*;
use crate::commands::CommandId as C;
use crate::task_ui::TaskKind;
use crate::ui::LibAct;

/// The first line of row `i`'s menu, as the app opens it: its label and whether it is on.
fn ai_line(a: &mut App, i: usize) -> (String, bool) {
    a.library_row_menu(i, 0.0, 0.0, false);
    let popup = a.ui.popup.as_ref().expect("the menu is open");
    let first = popup.root().items.first().expect("an item");
    assert_eq!(
        first.id,
        Act::Library(LibAct::AiTask, i),
        "the AI line heads it"
    );
    let line = (first.label.clone(), first.enabled);
    a.ui.popup = None;
    line
}

fn row(a: &App, label: &str) -> usize {
    labels(a)
        .iter()
        .position(|r| r == label)
        .unwrap_or_else(|| panic!("no row {label}"))
}

fn files(a: &App) -> Vec<PathBuf> {
    let s = a.task.as_ref().expect("the window is open");
    s.files.iter().map(|f| f.path.clone()).collect()
}

/// Each row's menu says how many notes it would take: a group and a section all the notes
/// under them, subsections too, and still when folded; a section with none has it greyed.
#[test]
fn each_row_menu_heads_with_the_ai_task_and_says_how_many_notes() {
    let (mut a, l) = lib("libai-menu");
    assert_eq!(
        labels(&a),
        [
            "Personal",
            "Money",
            "Taxes 2026",
            "2025",
            "Last year",
            "Trips"
        ]
    );
    let personal = row(&a, "Personal");
    assert_eq!(
        ai_line(&mut a, personal),
        ("AI tas&k on these 2 notes\u{2026}".to_string(), true)
    );
    let money = row(&a, "Money");
    assert_eq!(
        ai_line(&mut a, money),
        ("AI tas&k on these 2 notes\u{2026}".to_string(), true),
        "its subsection's note too"
    );
    let y2025 = row(&a, "2025");
    assert_eq!(
        ai_line(&mut a, y2025),
        ("AI tas&k on its one note\u{2026}".to_string(), true)
    );
    let trips = row(&a, "Trips");
    assert_eq!(
        ai_line(&mut a, trips),
        ("AI tas&k on its notes\u{2026}".to_string(), false),
        "nothing under it: greyed"
    );
    let taxes = row(&a, "Taxes 2026");
    assert_eq!(
        ai_line(&mut a, taxes),
        ("AI tas&k on this note\u{2026}".to_string(), true)
    );
    // Folded, Money still holds its two.
    let key = a.ui.library.rows[money].key.clone();
    a.ui.library.fold(&key);
    a.library_build();
    assert_eq!(labels(&a), ["Personal", "Money", "Trips"]);
    assert_eq!(
        ai_line(&mut a, money),
        ("AI tas&k on these 2 notes\u{2026}".to_string(), true),
        "folded or not"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// The line opens the window on those notes, on Rewrite them, and the header says where they
/// came from until another file is added.
#[test]
fn the_ai_task_on_a_section_opens_rewrite_on_its_notes_and_says_where_from() {
    let (mut a, l) = lib("libai-open");
    let money = row(&a, "Money");
    pick(&mut a, LibAct::AiTask, money);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.task, TaskKind::Rewrite, "it starts on Rewrite them");
    assert_eq!(
        s.from.as_deref(),
        Some("The 2 notes of Personal \u{203A} Money, from the Notes pane.")
    );
    assert_eq!(files(&a), vec![l.taxes.clone(), l.last.clone()]);
    // Another file added: the sentence no longer covers what is in the window.
    let other = l.dir.join("other.md");
    std::fs::write(&other, "# Other\n").expect("other");
    a.task_add_paths(vec![other]);
    assert_eq!(a.task.as_ref().and_then(|s| s.from.clone()), None);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// A note's own line takes that note; a section with no note says so and opens nothing.
#[test]
fn the_ai_task_on_a_note_takes_it_and_on_an_empty_section_opens_nothing() {
    let (mut a, l) = lib("libai-note");
    let trips = row(&a, "Trips");
    pick(&mut a, LibAct::AiTask, trips);
    assert!(a.task.is_none(), "nothing to work on");
    assert!(toast(&a).contains("no note under it"), "{}", toast(&a));
    let last = row(&a, "Last year");
    pick(&mut a, LibAct::AiTask, last);
    assert_eq!(files(&a), vec![l.last.clone()]);
    assert_eq!(
        a.task.as_ref().and_then(|s| s.from.clone()).as_deref(),
        Some("Last year, a note of Personal, from the Notes pane.")
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// "AI task…" works on the row the keyboard is on while the pane has the keyboard, and opens
/// empty on File them when it does not.
#[test]
fn ai_task_takes_the_row_the_keyboard_is_on() {
    let (mut a, l) = lib("libai-keys");
    a.compose();
    a.command(C::AiTaskFiles);
    let s = a.task.as_ref().expect("the window is open");
    assert!(
        s.files.is_empty() && s.task == TaskKind::File,
        "no keyboard in the pane"
    );
    a.task = None;
    assert!(a.library_focus(), "the pane takes the keyboard");
    let y2025 = row(&a, "2025");
    a.ui.library.set_cur(y2025);
    a.command(C::AiTaskFiles);
    assert_eq!(files(&a), vec![l.last.clone()]);
    assert_eq!(a.task.as_ref().map(|s| s.task), Some(TaskKind::Rewrite));
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Review of phase L, 1: the notes the line opens the window on are the library's own, and they
/// take part in Rewrite them and Split them; only File them leaves them out, as filed already.
#[test]
fn the_notes_from_the_pane_take_part_in_rewrite_and_split() {
    let (mut a, l) = lib("libai-part");
    let money = row(&a, "Money");
    pick(&mut a, LibAct::AiTask, money);
    let s = a.task.as_mut().expect("the window is open");
    let all: Vec<usize> = (0..s.files.len()).collect();
    assert_eq!(
        s.taking_part(),
        all,
        "every note takes part in Rewrite them"
    );
    s.set_task(TaskKind::Split);
    assert_eq!(s.taking_part(), all, "and in Split them");
    s.set_task(TaskKind::File);
    assert!(s.taking_part().is_empty());
    assert_eq!(
        s.files[0].reason_for(TaskKind::File).as_deref(),
        Some("already in the library")
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Review of phase L, 2: "AI task…" chosen with the mouse from the AI menu works on the row the
/// pane had: the press on AI edit does not take the keyboard from the pane.
#[test]
fn ai_task_from_the_ai_menu_by_mouse_takes_the_pane_s_row() {
    let (mut a, l) = lib("libai-mouse");
    a.docs.prefs.ai_connection = mdedit_core::ai::Connection::ClaudeApi;
    a.size = (1280, 800);
    super::files_tests::open_now(&mut a, &l.taxes);
    assert!(a.library_focus(), "the pane takes the keyboard");
    let y2025 = row(&a, "2025");
    a.ui.library.set_cur(y2025);
    a.compose();
    let r = a
        .hit_rect(Hit::Bar(crate::ui::BarItem::AiEdit))
        .expect("AI edit");
    a.ui.mouse = (r.cx(), r.cy());
    a.left_press();
    a.left_release();
    let item =
        a.ui.popup
            .as_ref()
            .expect("the AI menu is open")
            .root()
            .items
            .iter()
            .find(|i| i.label.starts_with("AI tas&k"))
            .map(|i| i.id.clone())
            .expect("AI task… is in the AI menu");
    a.choose(item, false);
    assert_eq!(files(&a), vec![l.last.clone()]);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Review of phase L, 3: with a window parked by "Show what is sent", the line brings that
/// window back as it was, its task and files with it, and does not put the notes into it.
#[test]
fn the_ai_task_on_a_row_brings_a_parked_window_back_as_it_was() {
    let (mut a, l) = lib("libai-parked");
    a.task_open(TaskKind::File, vec![l.taxes.clone()]);
    a.task_parked = a.task.take();
    let money = row(&a, "Money");
    pick(&mut a, LibAct::AiTask, money);
    let s = a.task.as_ref().expect("the window is back");
    assert_eq!(s.task, TaskKind::File, "its task as it was");
    assert_eq!(files(&a), vec![l.taxes.clone()], "its files as they were");
    assert_eq!(a.task.as_ref().and_then(|s| s.from.clone()), None);
    assert!(toast(&a).contains("back as it was"), "{}", toast(&a));
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Review of phase L, 10: "AI task…" with the keyboard on a section that has no note opens the
/// window empty, as from anywhere else, rather than nothing.
#[test]
fn ai_task_on_an_empty_section_from_the_keyboard_opens_the_window_empty() {
    let (mut a, l) = lib("libai-keys-empty");
    assert!(a.library_focus(), "the pane takes the keyboard");
    let trips = row(&a, "Trips");
    a.ui.library.set_cur(trips);
    a.command(C::AiTaskFiles);
    let s = a.task.as_ref().expect("the window opens");
    assert!(s.files.is_empty() && s.task == TaskKind::File);
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Review of phase L, 12: "AI task on this file…" on a note the library lists starts on Rewrite
/// them, which can work on it; File them would only say it is filed already.
#[test]
fn ai_task_on_this_file_starts_on_rewrite_for_a_library_note() {
    let (mut a, l) = lib("libai-tab");
    super::files_tests::open_now(&mut a, &l.taxes);
    a.command(C::AiTaskThisFile);
    let s = a.task.as_ref().expect("the window is open");
    assert_eq!(s.task, TaskKind::Rewrite);
    assert_eq!(files(&a), vec![l.taxes.clone()]);
    a.task = None;
    let other = l.dir.join("other.md");
    std::fs::write(&other, "# Other\n").expect("other");
    super::files_tests::open_now(&mut a, &other);
    a.command(C::AiTaskThisFile);
    assert_eq!(a.task.as_ref().map(|s| s.task), Some(TaskKind::File));
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Fable's review of C1, 4: a press on the main menu takes the keyboard from the pane, so a
/// command chosen there other than "AI task…" works as it always did, and the typing after it
/// goes to the text, not to the pane's rows.
#[test]
fn a_command_from_the_main_menu_gives_the_keyboard_back_to_the_text() {
    let (mut a, l) = lib("libai-menu-new");
    a.size = (1280, 800);
    super::files_tests::open_now(&mut a, &l.taxes);
    assert!(a.library_focus(), "the pane takes the keyboard");
    a.compose();
    let r = a
        .hit_rect(Hit::Bar(crate::ui::BarItem::More))
        .expect("the main menu");
    a.ui.mouse = (r.cx(), r.cy());
    a.left_press();
    a.left_release();
    assert!(a.ui.popup.is_some(), "the main menu is open");
    a.choose(crate::ui::Act::Cmd(C::NewFile), false);
    assert!(!a.ui.library.focused, "the pane no longer has the keyboard");
    super::files_tests::type_text(&mut a, "x");
    let typed = a
        .docs
        .active_ready()
        .expect("the new tab")
        .buffer
        .rope()
        .to_string();
    assert_eq!(typed, "x");
    assert_eq!(a.ui.library.menu_row, None, "the row is forgotten");
    let _ = std::fs::remove_dir_all(&l.dir);
}

/// Fable's review of C1, 4: the row a press on the bar remembered is forgotten when its menu
/// closes without a choice, so a later "AI task…" from the keyboard opens the window empty.
#[test]
fn a_menu_closed_without_a_choice_forgets_the_pane_s_row() {
    let (mut a, l) = lib("libai-menu-esc");
    a.docs.prefs.ai_connection = mdedit_core::ai::Connection::ClaudeApi;
    a.size = (1280, 800);
    super::files_tests::open_now(&mut a, &l.taxes);
    assert!(a.library_focus(), "the pane takes the keyboard");
    let y2025 = row(&a, "2025");
    a.ui.library.set_cur(y2025);
    a.compose();
    let r = a
        .hit_rect(Hit::Bar(crate::ui::BarItem::AiEdit))
        .expect("AI edit");
    a.ui.mouse = (r.cx(), r.cy());
    a.left_press();
    a.left_release();
    assert!(
        a.ui.popup.as_ref().is_some_and(|p| p.library_row.is_some()),
        "the menu holds the row"
    );
    a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Escape), None);
    assert!(a.ui.popup.is_none());
    // The AI menu again, from the keyboard this time: it holds no row.
    a.ui.keyboard = true;
    a.open_popup(crate::ui::PopupKind::Ai);
    let item =
        a.ui.popup
            .as_ref()
            .expect("the AI menu is open")
            .root()
            .items
            .iter()
            .find(|i| i.label.starts_with("AI tas&k"))
            .map(|i| i.id.clone())
            .expect("AI task… is in the AI menu");
    a.choose(item, false);
    assert!(files(&a).is_empty(), "the window opens empty");
    let _ = std::fs::remove_dir_all(&l.dir);
}
