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
    let first = popup.menu.items.first().expect("an item");
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
