//! "Move the notes…" through the window: the group row's menu opens the card with the plan's
//! rows, a blocked row stays unticked, an unsaved library file refuses the move with nothing
//! moved, a move writes and is listed in "AI runs…", and undo from there puts it back.

use super::files_tests::{open_now, scratch, type_text};
use super::library_write_tests::{app_in, press, slash};
use super::*;
use crate::commands::CommandId as C;
use crate::keys::KeyName;
use crate::ui::LibAct;
use mdedit_core::moves::Verdict;

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn toast(a: &App) -> String {
    a.docs
        .toast(Instant::now())
        .map(str::to_string)
        .unwrap_or_default()
}

/// A group Work whose folder is `dir/Work`, listing two notes under `## Projects` that are not in
/// `Work/Projects`: `plan.md` (free to move) and `budget.md` (a `budget.md` is there already).
struct Lib {
    dir: PathBuf,
    library: PathBuf,
    file: PathBuf,
    plan: PathBuf,
    budget: PathBuf,
    text: String,
}

fn lib(tag: &str) -> (App, Lib) {
    let dir = scratch(tag);
    let library = dir.join("library");
    let home = dir.join("Work");
    std::fs::create_dir_all(&library).unwrap();
    std::fs::create_dir_all(home.join("Inbox")).unwrap();
    std::fs::create_dir_all(home.join("Old")).unwrap();
    std::fs::create_dir_all(home.join("Projects")).unwrap();
    let plan = home.join("Inbox").join("plan.md");
    let budget = home.join("Old").join("budget.md");
    std::fs::write(&plan, "# Plan\n").unwrap();
    std::fs::write(&budget, "# Budget\n").unwrap();
    std::fs::write(
        home.join("Projects").join("budget.md"),
        "# Another budget\n",
    )
    .unwrap();
    let text = format!(
        "# Work\nhome: {}\n\n## Projects\n\n- [Plan]({})\n- [Budget]({})\n",
        slash(&home),
        slash(&plan),
        slash(&budget)
    );
    let file = library.join("Work.md");
    std::fs::write(&file, &text).unwrap();
    let mut a = app_in(&dir);
    a.command(C::ToggleLibraryPane);
    (
        a,
        Lib {
            dir,
            library,
            file,
            plan,
            budget,
            text,
        },
    )
}

/// The group's row menu, "Move the notes…".
fn open_card(a: &mut App) {
    assert!(
        matches!(
            a.ui.library.rows.first().map(|r| &r.kind),
            Some(crate::library_pane::Kind::Group)
        ),
        "the first row is the group"
    );
    a.act(Act::Library(LibAct::MoveNotes, 0));
    a.compose();
}

#[test]
fn the_group_menu_offers_move_the_notes_and_it_opens_the_card_with_the_plans_rows() {
    let (mut a, l) = lib("move-open");
    let row = &a.ui.library.rows[0];
    let menu =
        crate::overlays::library_row_menu(&row.kind, row.home.as_deref(), 0, true, 0.0, 0.0, false);
    assert!(
        menu.items
            .iter()
            .any(|i| i.id == Act::Library(LibAct::MoveNotes, 0)),
        "the group's menu has Move the notes…"
    );
    open_card(&mut a);
    let card = a.mover.as_ref().expect("the card is open");
    assert!(a.plan_is_open(), "the card takes the window's events");
    let rows = &card.plan.rows;
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].title, "Plan");
    assert!(mdshared_app::paths::same_path(&rows[0].from, &l.plan));
    assert!(rows[0].to.ends_with(Path::new("Work/Projects/plan.md")));
    assert_eq!(rows[0].verdict, Verdict::Ready);
    assert!(rows[0].ticked, "a ready row comes ticked");
    assert_eq!(rows[1].title, "Budget");
    assert_eq!(rows[1].verdict, Verdict::Taken);
    assert!(!rows[1].ticked);
    assert_eq!(
        crate::move_ui::title(&card.plan),
        "Move the notes to match the headings"
    );
    // Escape closes it, and nothing moved.
    press(&mut a, KeyName::Escape);
    assert!(a.mover.is_none());
    assert!(l.plan.is_file());
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_group_with_nothing_to_move_says_so_and_opens_nothing() {
    let (mut a, l) = lib("move-nothing");
    // Every note where its heading puts it.
    let home = l.dir.join("Work");
    let at = home.join("Projects").join("plan.md");
    std::fs::rename(&l.plan, &at).unwrap();
    std::fs::write(
        &l.file,
        format!(
            "# Work\nhome: {}\n\n## Projects\n\n- [Plan]({})\n",
            slash(&home),
            slash(&at)
        ),
    )
    .unwrap();
    a.act(Act::Library(LibAct::Refresh, 0));
    open_card(&mut a);
    assert!(a.mover.is_none(), "no card");
    assert!(
        toast(&a).contains("already where it belongs"),
        "{}",
        toast(&a)
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_blocked_row_cannot_be_ticked_in_the_window() {
    let (mut a, l) = lib("move-blocked");
    open_card(&mut a);
    // Down to the taken row, then Space; then a click on its tick box.
    press(&mut a, KeyName::Down);
    assert_eq!(a.mover.as_ref().map(|m| m.sel), Some(1));
    press(&mut a, KeyName::Space);
    assert!(!a.mover.as_ref().unwrap().plan.rows[1].ticked);
    let (x, y) = {
        let card = a.mover.as_ref().unwrap();
        let g = card.geo();
        let r = card.rows().rect(g.list, 1);
        (r.x + 20.0, r.cy())
    };
    a.pointer_moved(x, y);
    a.ui.mouse = (x, y);
    a.plan_press();
    a.plan_release();
    assert!(!a.mover.as_ref().unwrap().plan.rows[1].ticked);
    let request = a.mover.as_ref().unwrap().plan.request();
    assert_eq!(request.rows.len(), 1, "only the ready row would move");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn move_with_the_library_file_open_and_unsaved_is_refused_and_nothing_moves() {
    let (mut a, l) = lib("move-unsaved");
    open_now(&mut a, &l.file);
    type_text(&mut a, "x");
    assert!(a.docs.active_doc().is_some_and(|d| d.modified()));
    open_card(&mut a);
    press(&mut a, KeyName::Enter);
    let card = a.mover.as_ref().expect("the card stays open");
    let why = card.refusal.clone().unwrap_or_default();
    assert!(
        why.starts_with("The move could not start:") && why.contains("Work.md"),
        "{why}"
    );
    assert!(card.plan.rows[0].ticked, "the ticks are kept");
    // Nothing on disk changed.
    assert!(l.plan.is_file());
    assert!(!l.dir.join("Work/Projects/plan.md").exists());
    assert_eq!(read(&l.file), l.text);
    assert!(
        mdedit_core::runs::list_all(&l.library).0.is_empty(),
        "no record"
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn move_moves_the_ticked_note_rewrites_its_line_and_lists_the_run() {
    let (mut a, l) = lib("move-write");
    open_card(&mut a);
    press(&mut a, KeyName::Enter);
    assert!(a.mover.is_none(), "the card closes when it moves");
    let to = l.dir.join("Work").join("Projects").join("plan.md");
    assert_eq!(read(&to), "# Plan\n");
    assert!(!l.plan.exists(), "moved, not copied");
    assert!(l.budget.is_file(), "the blocked note stays");
    let text = read(&l.file);
    assert!(
        text.contains(&format!("- [Plan]({})", slash(&to))),
        "{text}"
    );
    assert!(
        text.contains(&format!("- [Budget]({})", slash(&l.budget))),
        "{text}"
    );
    // The folder it left is the owner's, outside the library: the core removes only folders
    // inside the library, so it stays.
    assert!(l.dir.join("Work").join("Inbox").is_dir());
    // The library the pane shows is the new one.
    assert!(
        a.docs
            .library
            .entries()
            .any(|(_, e)| mdshared_app::paths::same_path(&e.path, &to))
    );
    // The report, with the way to undo it.
    let outcome = a.ui.results.replace.outcome.as_ref().expect("the report");
    assert!(
        outcome.title.starts_with("Moved 1 file"),
        "{}",
        outcome.title
    );
    assert!(outcome.run.is_some());
    assert!(
        outcome.rows.iter().any(|r| matches!(
            r,
            crate::replace_ui::OutRow::Skipped { why, .. } if why == "there is already a file called this"
        )),
        "the kept row says why"
    );
    // "AI runs…" lists it, as a move.
    a.open_runs();
    assert_eq!(a.ui.runs.list.len(), 1);
    assert_eq!(a.ui.runs.list[0].record.kind, mdedit_core::runs::Kind::Move);
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn undo_from_ai_runs_puts_the_move_back() {
    let (mut a, l) = lib("move-undo");
    open_card(&mut a);
    press(&mut a, KeyName::Enter);
    let to = l.dir.join("Work").join("Projects").join("plan.md");
    assert!(to.is_file());
    a.open_runs();
    let path = a.ui.runs.list[0].path.clone();
    a.run_undo_ask(path.clone());
    assert_eq!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(DialogKind::RunUndo)
    );
    let (_, sentence, can) = a.ui.runs.undo.clone().expect("the question");
    assert!(can, "{sentence}");
    assert!(sentence.contains("1 file"), "{sentence}");
    assert!(to.is_file(), "nothing goes back before the answer");
    a.run_undo(path.clone());
    assert!(l.plan.is_file(), "the note is back");
    assert_eq!(read(&l.plan), "# Plan\n");
    assert!(!to.exists());
    assert_eq!(read(&l.file), l.text, "the line says what it said");
    let record = mdedit_core::runs::load(&path, &l.library).expect("the record stays");
    assert_eq!(record.state, mdedit_core::runs::State::Undone);
    let outcome = a.ui.results.replace.outcome.as_ref().expect("the report");
    assert!(
        outcome.title.starts_with("Undid the move"),
        "{}",
        outcome.title
    );
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn a_tab_open_on_a_moved_note_follows_it() {
    let (mut a, l) = lib("move-tab");
    open_now(&mut a, &l.plan);
    open_card(&mut a);
    press(&mut a, KeyName::Enter);
    let to = l.dir.join("Work").join("Projects").join("plan.md");
    let path = a.docs.active_doc().and_then(|d| d.path.clone()).unwrap();
    assert!(mdshared_app::paths::same_path(&path, &to), "{path:?}");
    let _ = std::fs::remove_dir_all(&l.dir);
}

#[test]
fn the_move_demo_draws_and_moves_nothing() {
    let mut a = super::files_tests::app();
    assert!(a.move_demo("move"));
    press(&mut a, KeyName::Enter);
    let card = a.mover.as_ref().expect("the card stays");
    assert!(card.demo);
    assert!(
        card.refusal
            .as_deref()
            .unwrap_or("")
            .contains("moves nothing")
    );
    press(&mut a, KeyName::Escape);
    assert!(a.mover.is_none());
    for name in crate::move_ui::NAMES {
        let mut a = super::files_tests::app();
        assert!(a.move_demo(name), "{name}");
        a.compose();
    }
}
