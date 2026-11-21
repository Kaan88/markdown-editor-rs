//! Headless tests of the window logic for the command palette, compare mode and replace in
//! files: an `App` built without a window, driven by the same functions the events call.

use super::*;
use crate::commands::CommandId as C;
use crate::keys::{Key, KeyName};
use crate::palette::{Mode, Target, mode_of};
use crate::ui::PopupKind;
use mdedit_core::Selection;
use mdshared_ui::gfx::{Gfx, Painter};
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};
use winit::event::MouseScrollDelta;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh scratch folder.
pub(super) fn scratch(tag: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, AtomicOrdering::Relaxed);
    let dir = mdshared_app::paths::canonical_or_absolute(&std::env::temp_dir())
        .join(format!("mdedit-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn app() -> App {
    let waker: Waker = Arc::new(|_| {});
    let docs = Docs::new(1252, waker);
    let mut app = App::new(
        Args::default(),
        docs,
        Trace::new(false, Instant::now()),
        None,
        Session::default(),
    );
    app.size = (1280, 800);
    app
}

/// Opens `path` in a tab and reads it now.
pub(super) fn open_now(app: &mut App, path: &Path) -> usize {
    let i = app.docs.open(path);
    app.docs.load_now(i, None, false);
    i
}

fn press(app: &mut App, key: KeyName, typed: Option<&str>) {
    app.key_pressed(Key::Name(key), typed);
}

pub(super) fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        let s = ch.to_string();
        press(app, KeyName::Char(ch), Some(&s));
    }
}

fn ctrl_p(app: &mut App) {
    app.mods = Mods {
        ctrl: true,
        ..Mods::default()
    };
    press(app, KeyName::Char('p'), None);
    app.mods = Mods::default();
}

fn listed(app: &App) -> Vec<Target> {
    let p = app.palette.open.as_ref().expect("the palette is open");
    p.palette
        .listed_ids()
        .into_iter()
        .filter_map(|id| p.target(id).cloned())
        .collect()
}

/// Waits for the palette's workers, as the event loop's wakes would deliver them.
fn settle(app: &mut App) {
    app.settle_palette();
}

#[test]
fn prefixes_pick_the_source() {
    assert_eq!(mode_of("").0, Mode::Files);
    assert_eq!(mode_of("abc").0, Mode::Files);
    assert_eq!(mode_of(">fo"), (Mode::Commands, 1));
    assert_eq!(mode_of(":12"), (Mode::Line, 1));
    assert_eq!(mode_of("@x"), (Mode::Symbols, 1));
    assert_eq!(mode_of("?"), (Mode::Help, 1));
}

#[test]
fn ctrl_p_opens_the_palette_with_tabs_and_ctrl_p_or_escape_closes_it() {
    let dir = scratch("pal-tabs");
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join("b.txt"), "two\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    open_now(&mut a, &dir.join("b.txt"));
    assert!(a.palette.open.is_none());
    ctrl_p(&mut a);
    assert!(a.palette.open.is_some());
    let ids: Vec<_> = a.docs.tabs.iter().map(|d| d.id).collect();
    assert_eq!(
        listed(&a),
        vec![Target::Tab(ids[0]), Target::Tab(ids[1])],
        "tabs first"
    );
    // While open it takes the keys: a letter is typed in the box, not in the document.
    type_text(&mut a, "x");
    assert_eq!(a.palette.open.as_ref().unwrap().palette.query(), "x");
    assert!(!a.docs.tabs[1].modified());
    press(&mut a, KeyName::Escape, None);
    assert!(a.palette.open.is_none());
    ctrl_p(&mut a);
    assert!(a.palette.open.is_some());
    ctrl_p(&mut a);
    assert!(a.palette.open.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn choosing_a_tab_closes_the_palette_first_then_switches() {
    let dir = scratch("pal-choose");
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    std::fs::write(dir.join("b.txt"), "two\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    open_now(&mut a, &dir.join("b.txt"));
    assert_eq!(a.docs.active(), Some(1));
    ctrl_p(&mut a);
    type_text(&mut a, "a.txt");
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_none());
    assert_eq!(a.docs.active(), Some(0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_command_list_greys_what_cannot_run_and_leaves_out_the_palette_itself() {
    use crate::commands::CommandId as C;
    let dir = scratch("pal-cmds");
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.open_palette(">");
    let targets = listed(&a);
    assert!(targets.contains(&Target::Cmd(C::Save)));
    assert!(!targets.contains(&Target::Cmd(C::CommandPalette)));
    assert!(!targets.contains(&Target::Cmd(C::Escape)));
    // The text is unchanged and has no selection: Save and Cut cannot run; Select all can.
    let on = |c: C| crate::palette::enabled(c, &a.docs, &a.ui, false);
    assert!(!on(C::Save) && !on(C::Cut) && on(C::SelectAll));
    // Choosing a command closes the palette and runs it.
    a.palette
        .open
        .as_mut()
        .unwrap()
        .palette
        .set_query(">word wrap");
    a.palette.sync(&a.docs, &a.ui, &mut a.macros, false);
    let wrap = a.docs.wrap;
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_none());
    assert_ne!(a.docs.wrap, wrap);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_greyed_command_cannot_be_chosen() {
    let dir = scratch("pal-grey");
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.open_palette(">Open all folds");
    // Open all folds needs a fold: the only match is greyed, so Enter chooses nothing.
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn go_to_line_jumps_on_enter_and_names_the_line() {
    let dir = scratch("pal-line");
    let text: String = (1..=30).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.join("a.txt"), text).unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.open_palette(":");
    type_text(&mut a, "12");
    assert_eq!(a.palette.open.as_ref().unwrap().line_query(), Some((12, 1)));
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_none());
    let r = a.docs.active_ready().unwrap();
    assert_eq!(r.buffer.line_of(r.view.sel.caret), 11);
    // Line and column.
    a.open_palette(":3:4");
    assert_eq!(a.palette.open.as_ref().unwrap().line_query(), Some((3, 4)));
    press(&mut a, KeyName::Enter, None);
    let r = a.docs.active_ready().unwrap();
    assert_eq!(r.line_col(r.view.sel.caret), (2, 3));
    // Not a number: Enter does nothing and the palette stays.
    a.open_palette(":x");
    assert_eq!(a.palette.open.as_ref().unwrap().line_query(), None);
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn symbols_come_from_a_worker_and_choosing_one_jumps() {
    let dir = scratch("pal-sym");
    std::fs::write(
        dir.join("a.rs"),
        "fn alpha() {}\n\nfn beta() {\n    let x = 1;\n}\n",
    )
    .unwrap();
    let mut a = app();
    let i = open_now(&mut a, &dir.join("a.rs"));
    // The language comes from the highlighter's set; make sure this file has one.
    assert!(a.docs.tabs[i].ready().unwrap().language.is_some());
    a.open_palette("@");
    settle(&mut a);
    let targets = listed(&a);
    assert_eq!(targets, vec![Target::Symbol(0), Target::Symbol(2)]);
    a.palette.open.as_mut().unwrap().palette.set_query("@bet");
    a.palette.sync(&a.docs, &a.ui, &mut a.macros, false);
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_none());
    let r = a.docs.active_ready().unwrap();
    assert_eq!(r.buffer.line_of(r.view.sel.caret), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_folders_files_are_listed_on_a_worker_once_something_is_typed() {
    let dir = scratch("pal-files");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("alpha.txt"), "a\n").unwrap();
    std::fs::write(dir.join("sub").join("beta.txt"), "b\n").unwrap();
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".git").join("hidden.txt"), "h\n").unwrap();
    let mut a = app();
    a.set_folder(dir.clone());
    a.open_palette("");
    // Nothing is listed, and no worker was started, until the query has text.
    assert!(listed(&a).is_empty());
    assert!(!a.palette.pending());
    a.palette.open.as_mut().unwrap().palette.set_query("beta");
    a.palette.sync(&a.docs, &a.ui, &mut a.macros, false);
    assert!(a.palette.pending());
    settle(&mut a);
    let t = listed(&a);
    assert_eq!(t.len(), 1);
    let Target::File(p) = &t[0] else {
        panic!("a file")
    };
    assert!(p.ends_with("beta.txt"));
    // Closed and opened again: the list is kept, not made again.
    a.close_palette();
    a.open_palette("alpha");
    assert!(!a.palette.pending());
    assert_eq!(listed(&a).len(), 1);
    // Choosing opens the file.
    press(&mut a, KeyName::Enter, None);
    assert!(a.palette.open.is_none());
    assert!(
        a.docs
            .active_doc()
            .unwrap()
            .path
            .as_ref()
            .unwrap()
            .ends_with("alpha.txt")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_question_mark_lists_the_prefixes_and_a_row_puts_one_in_the_box() {
    let mut a = app();
    a.open_palette("?");
    let t = listed(&a);
    assert_eq!(t.len(), 4);
    assert!(t.iter().all(|t| matches!(t, Target::Prefix(_))));
    // Choosing the second row (">") puts it in the box and lists commands.
    press(&mut a, KeyName::Down, None);
    press(&mut a, KeyName::Enter, None);
    let p = a.palette.open.as_ref().expect("still open");
    assert_eq!(p.palette.query(), ">");
    assert!(p.palette.len() > 20);
}

/// Runs what the event loop's wakes would run, until `until` holds (at most ten seconds).
fn pump(app: &mut App, until: fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !until(app) && Instant::now() < deadline {
        app.ui.results.poll();
        app.replace_poll();
        app.docs
            .wait_load(Instant::now() + Duration::from_millis(5), None, false);
    }
    assert!(until(app), "timed out");
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

#[test]
fn replace_in_files_plans_asks_applies_reports_and_undoes() {
    let dir = scratch("rep");
    for n in ["a.txt", "b.txt", "c.txt", "d.txt"] {
        std::fs::write(dir.join(n), "one foo two\nfoo\n").unwrap();
    }
    std::fs::write(dir.join("none.txt"), "nothing here\n").unwrap();
    let mut a = app();
    a.set_folder(dir.clone());
    let ia = open_now(&mut a, &dir.join("a.txt"));
    let ic = open_now(&mut a, &dir.join("c.txt"));
    // c.txt has unsaved changes: it is left out. a.txt is open and unchanged: it reloads.
    a.docs.activate(ic);
    a.docs.edit(EditOp::Insert("X"), None);
    assert!(a.docs.tabs[ic].modified());
    a.docs.activate(ia);
    a.ui.find.set_text("foo");
    a.docs.find_set("foo", a.ui.options());
    a.find_in_files();
    pump(&mut a, |a| {
        matches!(a.ui.results.state, crate::results::State::Done { .. })
    });
    assert_eq!(a.ui.results.files, 4);
    // Nothing is planned until "Replace all…", and nothing is written.
    assert!(a.ui.dialog.is_none());
    a.ui.results.replace.field.set_text("bar");
    a.replace_all_in_files();
    pump(&mut a, |a| a.ui.results.replace.confirm.is_some());
    assert!(matches!(
        a.ui.dialog.as_ref().map(|d| d.kind),
        Some(DialogKind::ReplaceFiles)
    ));
    assert_eq!(read(&dir, "a.txt"), "one foo two\nfoo\n");
    {
        let c = a.ui.results.replace.confirm.as_mut().unwrap();
        // The unsaved tab's file is not in the plan; the others are, all ticked.
        assert_eq!(c.plan.files.len(), 3);
        assert_eq!(c.plan.totals.matches, 6);
        assert_eq!(c.chosen().len(), 3);
        assert!(c.title().starts_with("Replace 6 occurrences in 3 files"));
        // Untick b.txt.
        let b = c.names.iter().position(|n| n == "b.txt").unwrap();
        c.toggle(b);
        assert_eq!(c.chosen().len(), 2);
    }
    // Cancel writes nothing and keeps the plan out of the way.
    a.replace_dialog_button(1);
    assert!(a.ui.dialog.is_none());
    assert_eq!(read(&dir, "a.txt"), "one foo two\nfoo\n");
    a.replace_all_in_files();
    pump(&mut a, |a| a.ui.results.replace.confirm.is_some());
    {
        let c = a.ui.results.replace.confirm.as_mut().unwrap();
        let b = c.names.iter().position(|n| n == "b.txt").unwrap();
        c.toggle(b);
    }
    a.replace_dialog_button(0);
    pump(&mut a, |a| a.ui.results.replace.outcome.is_some());
    assert_eq!(read(&dir, "a.txt"), "one bar two\nbar\n");
    assert_eq!(read(&dir, "b.txt"), "one foo two\nfoo\n");
    assert_eq!(read(&dir, "c.txt"), "one foo two\nfoo\n");
    assert_eq!(read(&dir, "d.txt"), "one bar two\nbar\n");
    assert!(!dir.join("a.txt.bak").exists());
    let o = a.ui.results.replace.outcome.as_ref().unwrap();
    assert_eq!(o.report.as_ref().unwrap().replacements, 4);
    assert!(o.undo_help().contains('2'));
    // The unchanged open tab reloads by itself; the one with unsaved changes keeps its text.
    pump(&mut a, |a| !a.docs.tabs[0].is_loading());
    let mut line = String::new();
    a.docs.tabs[ia]
        .ready()
        .unwrap()
        .buffer
        .line_text(0, &mut line);
    assert_eq!(line, "one bar two");
    assert!(a.docs.tabs[ic].modified());
    // Undo replace puts the files back.
    a.replace_undo();
    pump(&mut a, |a| {
        let r = &a.ui.results.replace;
        !r.busy() && r.outcome.as_ref().is_some_and(|o| o.report.is_none())
    });
    assert_eq!(read(&dir, "a.txt"), "one foo two\nfoo\n");
    assert_eq!(read(&dir, "d.txt"), "one foo two\nfoo\n");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- compare ---------------------------------------------------------------------------------

/// Two files in a split, the first at the left and active, and the comparison running.
fn compared(a_text: &str, b_text: &str) -> (App, PathBuf, usize, usize) {
    let dir = scratch("cmp");
    std::fs::write(dir.join("a.txt"), a_text).unwrap();
    std::fs::write(dir.join("b.txt"), b_text).unwrap();
    let mut a = app();
    let ia = open_now(&mut a, &dir.join("a.txt"));
    let ib = open_now(&mut a, &dir.join("b.txt"));
    a.docs.activate(ia);
    a.docs.other = Some(a.docs.tabs[ib].id);
    a.docs.other_left = false;
    a.command(C::CompareOther);
    assert!(a.compare.is_some());
    settle_compare(&mut a);
    (a, dir, ia, ib)
}

fn settle_compare(a: &mut App) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        a.compare_timer(Instant::now());
        if a.compare
            .as_ref()
            .is_none_or(crate::compare::Compare::settled)
            || Instant::now() > deadline
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    a.compare_sync();
}

fn tabs(a: &App, i: usize) -> &crate::docs::Ready {
    a.docs.tabs[i].ready().unwrap()
}

const TEN: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";

#[test]
fn comparing_the_two_panes_aligns_their_rows_and_stop_gives_everything_back() {
    let (mut a, dir, ia, ib) = compared(TEN, "1\n2\nx\ny\n3\n4\n5\n6\n8\n9\n10\n");
    let (ra, rb) = (tabs(&a, ia), tabs(&a, ib));
    let (la, lb) = (
        ra.view.align.as_ref().unwrap(),
        rb.view.align.as_ref().unwrap(),
    );
    assert!(!la.right && lb.right && la.fresh && lb.fresh);
    // Same rows on both sides: the inserted "x", "y" have fillers opposite, "7" has one in b.
    assert_eq!(
        la.rows(ra.buffer.len_lines()),
        lb.rows(rb.buffer.len_lines())
    );
    assert_eq!(la.summary(), "2 differences");
    assert_eq!(crate::status::fields(&a.docs).0, "2 differences");
    // The panes scroll together by row, whichever moves.
    a.docs.tabs[ib].ready_mut().unwrap().view.top = 3.0;
    a.compare_sync();
    assert_eq!(tabs(&a, ia).view.top, 3.0);
    a.docs.tabs[ia].ready_mut().unwrap().view.top = 1.0;
    a.compare_sync();
    assert_eq!(tabs(&a, ib).view.top, 1.0);
    // Escape stops it: the rows are lines again and the top is a line.
    a.docs.tabs[ia].ready_mut().unwrap().view.top = 4.0;
    a.compare_sync();
    a.escape();
    assert!(a.compare.is_none());
    assert!(tabs(&a, ia).view.align.is_none() && tabs(&a, ib).view.align.is_none());
    assert_eq!(tabs(&a, ia).view.top, 2.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn word_wrap_and_folds_wait_while_comparing() {
    let code = "fn a() {\n    one();\n    two();\n}\nfn b() {\n    three();\n}\n";
    let (mut a, dir, ia, _) =
        compared(code, "fn a() {\n    one();\n}\nfn b() {\n    three();\n}\n");
    a.docs.wrap = true;
    a.docs.sync_wrap();
    assert!(!tabs(&a, ia).wrap);
    a.docs.tabs[ia]
        .ready_mut()
        .unwrap()
        .view
        .folds
        .suspend(false);
    assert!(a.docs.tabs[ia].ready_mut().unwrap().view.folds.fold(0, 2));
    a.docs.sync_wrap();
    assert!(tabs(&a, ia).view.folds.is_empty() && tabs(&a, ia).view.folds.suspended());
    assert_eq!(tabs(&a, ia).view.folds.hidden(), 0);
    a.command(C::StopCompare);
    a.docs.sync_wrap();
    // Wrap is back, and so is the fold.
    assert!(tabs(&a, ia).wrap);
    assert_eq!(tabs(&a, ia).view.folds.ranges(), &[(0, 2)]);
    assert!(!tabs(&a, ia).view.folds.suspended());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn editing_either_pane_makes_the_comparison_again_after_a_rest() {
    let (mut a, dir, ia, ib) = compared(TEN, TEN);
    assert_eq!(
        tabs(&a, ia).view.align.as_ref().unwrap().summary(),
        "No differences"
    );
    a.docs.activate(ib);
    a.docs.edit(EditOp::Insert("z"), None);
    // At once the diff on screen is stale: no marks inside lines, and "Comparing…" in the strip.
    a.compare_timer(Instant::now());
    assert!(!tabs(&a, ib).view.align.as_ref().unwrap().fresh);
    assert_eq!(crate::status::fields(&a.docs).0, "Comparing\u{2026}");
    assert!(!a.compare.as_ref().unwrap().settled());
    settle_compare(&mut a);
    assert!(tabs(&a, ib).view.align.as_ref().unwrap().fresh);
    assert_eq!(
        tabs(&a, ib).view.align.as_ref().unwrap().summary(),
        "1 difference"
    );
    // Both panes are ordinary editors: the other one takes an edit too.
    a.docs.activate(ia);
    assert!(a.docs.edit(EditOp::Insert("q"), None));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_compared_tab_or_ending_the_split_ends_the_comparison() {
    let (mut a, dir, ia, ib) = compared(TEN, "1\n2\n");
    a.docs.close(ib, None);
    a.compare_sync();
    assert!(a.compare.is_none());
    assert!(tabs(&a, ia).view.align.is_none());
    let _ = std::fs::remove_dir_all(&dir);
    let (mut a, dir, ia, ib) = compared(TEN, "1\n2\n");
    a.command(C::ToggleSplit);
    assert!(a.compare.is_none() && a.docs.other().is_none());
    assert!(tabs(&a, ia).view.align.is_none() && tabs(&a, ib).view.align.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn next_and_previous_difference_move_the_caret_and_say_when_there_are_no_more() {
    let (mut a, dir, ia, _) = compared(TEN, "1\nTWO\n3\n4\n5\n6\n7\n8\nNINE\n10\n");
    let line = |a: &App| {
        let r = tabs(a, ia);
        r.buffer.line_of(r.view.sel.caret)
    };
    a.command(C::NextDifference);
    assert_eq!(line(&a), 1);
    a.command(C::NextDifference);
    assert_eq!(line(&a), 8);
    a.command(C::NextDifference);
    assert_eq!(line(&a), 8);
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("No more"))
    );
    a.command(C::PreviousDifference);
    assert_eq!(line(&a), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compare_with_saved_shows_the_file_on_disk_read_only_and_closes_with_the_comparison() {
    let dir = scratch("cmp-saved");
    let path = dir.join("a.txt");
    std::fs::write(&path, TEN).unwrap();
    let mut a = app();
    let ia = open_now(&mut a, &path);
    // Unchanged: nothing to compare.
    assert!(!crate::compare::enabled(C::CompareSaved, &a.docs));
    a.docs.edit(EditOp::Insert("edited "), None);
    assert!(crate::compare::enabled(C::CompareSaved, &a.docs));
    a.command(C::CompareSaved);
    assert_eq!(a.docs.tabs.len(), 2);
    let saved = a.docs.tabs.iter().position(|d| d.title.is_some()).unwrap();
    assert_eq!(a.docs.tabs[saved].name(), "a.txt (saved)");
    assert!(a.docs.other_left && a.docs.other() == Some(saved));
    settle_compare(&mut a);
    assert_eq!(
        tabs(&a, ia).view.align.as_ref().unwrap().summary(),
        "1 difference"
    );
    // The copy cannot be edited, and the file on disk is untouched.
    a.docs.activate(saved);
    assert!(!a.docs.edit(EditOp::Insert("no"), None));
    a.docs.activate(ia);
    a.command(C::StopCompare);
    assert_eq!(a.docs.tabs.len(), 1);
    assert!(a.docs.other.is_none());
    assert_eq!(read(&dir, "a.txt"), TEN);
    // The text still has its unsaved edit.
    assert!(a.docs.tabs[0].modified());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_is_copied_to_the_other_side_or_taken_from_it_in_one_undo_step() {
    let b_text = "1\nTWO\n3\n4\nx\ny\n5\n6\n7\n9\n10\n";
    let (mut a, dir, ia, ib) = compared(TEN, b_text);
    let text = |a: &App, i| tabs(a, i).buffer.rope().to_string();
    // Not in a difference: nothing happens, and it is said.
    assert!(!crate::compare::enabled(C::CopyChangeAcross, &a.docs));
    a.command(C::CopyChangeAcross);
    assert_eq!(text(&a, ib), b_text);
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("not in a difference"))
    );
    // The first difference: line 2 against TWO.
    a.command(C::NextDifference);
    assert!(crate::compare::enabled(C::CopyChangeAcross, &a.docs));
    assert!(crate::compare::enabled(C::TakeChangeAcross, &a.docs));
    a.command(C::CopyChangeAcross);
    assert_eq!(text(&a, ib), "1\n2\n3\n4\nx\ny\n5\n6\n7\n9\n10\n");
    assert_eq!(text(&a, ia), TEN, "the source stays as it was");
    settle_compare(&mut a);
    assert_eq!(
        tabs(&a, ia).view.align.as_ref().unwrap().summary(),
        "2 differences"
    );
    // One undo takes it back in the document that changed.
    assert!(a.docs.edit_in(ib, EditOp::Undo, None));
    assert_eq!(text(&a, ib), b_text);
    assert!(!a.docs.edit_in(ib, EditOp::Undo, None));
    // The next one is the lines x and y the left lacks: taking them brings them in.
    settle_compare(&mut a);
    a.command(C::NextDifference);
    a.command(C::TakeChangeAcross);
    assert_eq!(text(&a, ia), "1\n2\n3\n4\nx\ny\n5\n6\n7\n8\n9\n10\n");
    assert!(a.docs.edit_in(ia, EditOp::Undo, None));
    assert_eq!(text(&a, ia), TEN);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_gutter_arrow_copies_its_block_across_in_one_undo_step() {
    let b_text = "1\nTWO\n3\n4\nx\ny\n5\n6\n7\n9\n10\n";
    let (mut a, dir, ia, ib) = compared(TEN, b_text);
    let text = |a: &App, i| tabs(a, i).buffer.rope().to_string();
    let al = tabs(&a, ia).view.align.clone().unwrap();
    // The block of "2" against "TWO" starts on row 1; the row under it is not a block start.
    let first = al.hunk_starting_at(1).expect("a block starts on row 1");
    assert!(al.hunk_starting_at(0).is_none() && al.hunk_starting_at(2).is_none());
    // The arrow of the pane that has the keyboard copies from it to the other pane's document.
    a.compare_copy_hunk(false, first);
    assert_eq!(text(&a, ib), "1\n2\n3\n4\nx\ny\n5\n6\n7\n9\n10\n");
    assert_eq!(text(&a, ia), TEN, "the source stays as it was");
    assert!(a.docs.edit_in(ib, EditOp::Undo, None));
    assert_eq!(text(&a, ib), b_text);
    assert!(!a.docs.edit_in(ib, EditOp::Undo, None), "one step");
    // The second pane's arrow copies from it into the pane that has the keyboard.
    settle_compare(&mut a);
    let al = tabs(&a, ia).view.align.clone().unwrap();
    let xy = al.hunk_starting_at(4).expect("a block starts on row 4");
    a.compare_copy_hunk(true, xy);
    assert_eq!(text(&a, ia), "1\n2\n3\n4\nx\ny\n5\n6\n7\n8\n9\n10\n");
    assert_eq!(text(&a, ib), b_text, "the source stays as it was");
    assert!(a.docs.edit_in(ia, EditOp::Undo, None));
    assert_eq!(text(&a, ia), TEN);
    // A block that is not there, or an older diff, copies nothing.
    a.compare_copy_hunk(false, 99);
    assert_eq!(text(&a, ib), b_text);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn one_line_of_a_changed_block_goes_across_when_it_has_a_partner() {
    let (mut a, dir, ia, ib) = compared("a\nb\nc\nd\ne\n", "a\nB\nC\nD\nE\nF\ne\n");
    let text = |a: &App, i| tabs(a, i).buffer.rope().to_string();
    let at_line = |a: &mut App, line: usize| {
        let r = a.docs.tabs[ia].ready_mut().unwrap();
        r.view.sel = Selection::at(r.buffer.line_range(line).start);
    };
    // On an equal line there is nothing to copy.
    at_line(&mut a, 0);
    assert!(!crate::compare::enabled(C::CopyLineAcross, &a.docs));
    a.command(C::CopyLineAcross);
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("not in a difference"))
    );
    // "c" shares its row with "C": only that line changes, in one undo step.
    at_line(&mut a, 2);
    assert!(crate::compare::enabled(C::CopyLineAcross, &a.docs));
    a.command(C::CopyLineAcross);
    assert_eq!(text(&a, ib), "a\nB\nc\nD\nE\nF\ne\n");
    assert_eq!(text(&a, ia), "a\nb\nc\nd\ne\n");
    assert!(a.docs.edit_in(ib, EditOp::Undo, None));
    assert_eq!(text(&a, ib), "a\nB\nC\nD\nE\nF\ne\n");
    assert!(!a.docs.edit_in(ib, EditOp::Undo, None), "one step");
    let _ = std::fs::remove_dir_all(&dir);
    // The first file has the surplus: its last line of the block has no partner.
    let (mut a, dir, ia, ib) = compared("a\nb\nc\nd\ne\n", "a\nB\ne\n");
    {
        let r = a.docs.tabs[ia].ready_mut().unwrap();
        r.view.sel = Selection::at(r.buffer.line_range(3).start);
    }
    assert!(!crate::compare::enabled(C::CopyLineAcross, &a.docs));
    a.command(C::CopyLineAcross);
    assert_eq!(text(&a, ib), "a\nB\ne\n");
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("no matching line"))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_one_side_lacks_is_brought_in_or_removed() {
    let text = |a: &App, i| tabs(a, i).buffer.rope().to_string();
    // The caret goes to "c", the line after the filler rows of the block.
    let (mut a, dir, ia, _) = compared("a\nb\nc\n", "a\nb\nX\nY\nc\n");
    a.command(C::NextDifference);
    a.command(C::TakeChangeAcross);
    assert_eq!(text(&a, ia), "a\nb\nX\nY\nc\n");
    let _ = std::fs::remove_dir_all(&dir);
    let (mut a, dir, ia, ib) = compared("a\nb\nc\n", "a\nb\nX\nY\nc\n");
    a.command(C::NextDifference);
    a.command(C::CopyChangeAcross);
    assert_eq!(text(&a, ib), "a\nb\nc\n");
    settle_compare(&mut a);
    assert_eq!(
        tabs(&a, ia).view.align.as_ref().unwrap().summary(),
        "No differences"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_cannot_go_into_the_saved_copy_but_can_come_from_it() {
    let dir = scratch("cmp-take");
    let path = dir.join("a.txt");
    std::fs::write(&path, TEN).unwrap();
    let mut a = app();
    let ia = open_now(&mut a, &path);
    a.docs.edit(EditOp::Insert("edited "), None);
    a.command(C::CompareSaved);
    settle_compare(&mut a);
    // The text as edited is at the right; the saved copy at the left cannot change.
    assert!(!crate::compare::enabled(C::CopyChangeAcross, &a.docs));
    assert!(crate::compare::enabled(C::TakeChangeAcross, &a.docs));
    a.command(C::CopyChangeAcross);
    assert!(
        a.docs
            .toast(Instant::now())
            .is_some_and(|t| t.contains("cannot be changed"))
    );
    a.command(C::TakeChangeAcross);
    assert_eq!(tabs(&a, ia).buffer.rope().to_string(), TEN);
    assert!(a.docs.edit_in(ia, EditOp::Undo, None));
    assert_eq!(
        tabs(&a, ia).buffer.rope().to_string(),
        format!("edited {TEN}")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn compare_commands_are_greyed_until_they_can_run() {
    let mut a = app();
    let dir = scratch("cmp-en");
    std::fs::write(dir.join("a.txt"), "a\n").unwrap();
    std::fs::write(dir.join("b.txt"), "b\n").unwrap();
    let ia = open_now(&mut a, &dir.join("a.txt"));
    assert!(!crate::compare::enabled(C::CompareOther, &a.docs));
    assert!(crate::compare::enabled(C::CompareFile, &a.docs));
    assert!(!crate::compare::enabled(C::StopCompare, &a.docs));
    assert!(!crate::compare::enabled(C::NextDifference, &a.docs));
    let ib = open_now(&mut a, &dir.join("b.txt"));
    a.docs.activate(ia);
    a.docs.other = Some(a.docs.tabs[ib].id);
    assert!(crate::compare::enabled(C::CompareOther, &a.docs));
    a.command(C::CompareOther);
    assert!(!crate::compare::enabled(C::CompareOther, &a.docs));
    assert!(crate::compare::enabled(C::StopCompare, &a.docs));
    assert!(crate::compare::enabled(C::NextDifference, &a.docs));
    let _ = std::fs::remove_dir_all(&dir);
}

fn twinned_app(tag: &str, text: &str) -> (App, PathBuf, usize) {
    let dir = scratch(tag);
    std::fs::write(dir.join("a.txt"), text).unwrap();
    let mut a = app();
    let i = open_now(&mut a, &dir.join("a.txt"));
    a.command(C::ShowInOtherPane);
    assert!(a.docs.same_file);
    (a, dir, i)
}

#[test]
fn showing_a_file_in_the_other_pane_greys_compare_and_the_command_works_from_the_tab_menu() {
    let (mut a, dir, i) = twinned_app("twin-cmds", "one\ntwo\n");
    // One tab, shown twice: no other tab to compare with, and the pane can be focused.
    assert_eq!(a.docs.tabs.len(), 1);
    assert!(!crate::compare::enabled(C::CompareOther, &a.docs));
    assert!(crate::palette::enabled(
        C::FocusOther,
        &a.docs,
        &a.ui,
        false
    ));
    assert!(crate::palette::enabled(
        C::ShowInOtherPane,
        &a.docs,
        &a.ui,
        false
    ));
    // The command is in the View menu (on) and in the tab's menu (on).
    let view = crate::overlays::view_menu(&a.docs, &a.ui, 0.0, 0.0, false);
    let item = view
        .items
        .iter()
        .find(|it| it.id == Act::Cmd(C::ShowInOtherPane))
        .expect("in the View menu");
    assert!(item.enabled && item.label == "Show this file &in the other pane");
    let tab = crate::overlays::tab_menu(&a.docs, i, 0.0, 0.0);
    assert!(
        tab.items
            .iter()
            .any(|it| it.id == Act::TabCmd(C::ShowInOtherPane, i) && it.enabled)
    );
    // Ending the split goes back to one view; the tab menu's entry makes it again.
    a.command(C::ToggleSplit);
    assert!(!a.docs.same_file && a.docs.active_ready().unwrap().twin.is_none());
    a.tab_command(C::ShowInOtherPane, i);
    assert!(a.docs.same_file);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_macro_recorded_in_one_view_plays_in_the_other_where_its_caret_is() {
    let (mut a, dir, i) = twinned_app("twin-macro", "hello\nworld\n");
    // Record typing "Q" at the top of the first view.
    a.command(C::RecordMacro);
    type_text(&mut a, "Q");
    a.command(C::RecordMacro);
    assert_eq!(a.docs.tabs[i].ready().unwrap().buffer.len_bytes(), 13);
    // The other view has its caret in "world"; the keyboard goes there and the macro plays.
    a.docs
        .active_ready_mut()
        .unwrap()
        .twin
        .as_mut()
        .unwrap()
        .sel = Selection::at(9);
    a.command(C::FocusOther);
    a.command(C::PlayMacro);
    let r = a.docs.tabs[i].ready().unwrap();
    let mut text = String::new();
    r.buffer.text(0..r.buffer.len_bytes(), &mut text);
    assert_eq!(text, "Qhello\nwoQrld\n");
    // The first view's caret (after its own "Q") is where it was, in the text that moved.
    assert_eq!(r.twin.as_ref().unwrap().sel.caret, 1);
    assert_eq!(r.view.sel.caret, 10);
    // One undo from either view takes the macro's typing back.
    a.command(C::Undo);
    assert!(a.docs.active_ready().unwrap().buffer.len_bytes() == 13);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_split_of_one_file_is_kept_in_the_session_and_comes_back() {
    let (mut a, dir, _) = twinned_app("twin-session", "one\ntwo\n");
    a.docs.other_left = true;
    a.docs.split_ratio = 0.4;
    let split = a.split_state().expect("a split to keep");
    assert_eq!(split.path, dir.join("a.txt"));
    // The line is the same one a two-file split writes, and it reads back.
    let kept = crate::session::Session {
        tabs: vec![dir.join("a.txt")],
        split: Some(split.clone()),
        ..Default::default()
    };
    let back = crate::session::Session::parse(&kept.serialize());
    assert_eq!(back.split, Some(split.clone()));
    // A fresh run: the file is open and active, the split comes back with both views.
    let mut b = app();
    open_now(&mut b, &dir.join("a.txt"));
    assert!(b.docs.restore_split(&split.path, split.left, split.ratio()));
    assert!(b.docs.same_file && b.docs.other_left);
    assert!((b.docs.split_ratio - 0.4).abs() < 0.01);
    assert!(b.docs.active_ready().unwrap().twin.is_some());
    assert_eq!(b.split_state(), Some(split));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_the_tab_of_a_file_shown_twice_ends_the_split_and_the_comparison_commands_stay_off() {
    let (mut a, dir, i) = twinned_app("twin-closing", "one\n");
    a.docs.close(i, None);
    assert!(a.docs.tabs.is_empty() && !a.docs.same_file && a.docs.other.is_none());
    assert!(a.split_state().is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Opens `kind` for the wheel tests and places its menu in a window of `h` px (a short one makes
/// a tall menu scroll).
fn placed_menu(a: &mut App, kind: PopupKind, h: f32) -> Rect {
    a.open_popup(kind);
    let mut bm = Bitmap::new(1280, 800);
    let mut gfx = Gfx::new();
    let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
    let popup = a.ui.popup.as_mut().expect("the menu opened");
    popup.menu.place(&mut p, 1280.0, h);
    popup.menu.rect()
}

/// The owner: a long saved prompt's name ran under its Modify button in the AI menu. The menu
/// elides a label at its hint, so the blank hint is wider than the button and a gap.
#[test]
fn a_saved_prompt_s_name_stops_before_its_modify_button() {
    use mdshared_ui::gfx::Face;
    let mut bm = Bitmap::new(64, 64);
    let mut gfx = Gfx::new();
    let mut p = Painter::new(&mut bm, &mut gfx, 1.0);
    // As the shared menu measures a hint: its width and 28 px around it. The menu's button,
    // its 4 px margin and a 6 px gap fit in it (the owner: the menu is not to grow for it).
    let hint = crate::overlays::modify_room(&mut p);
    let room = p.measure(Face::Ui, 12.0, &hint) + 28.0;
    let w = crate::ai_ui::MENU_MODIFY_W;
    assert!(room >= w + 4.0 + 6.0, "room {room} for a {w} px button");
    assert!(room < 80.0, "the menu's room is as wide as before: {room}");
    // The word fits in the narrow button, after the pencil, in the Windows UI font (other
    // systems fall back to fonts the runners may not match).
    let word = p.measure(Face::Ui, 12.5, crate::ai_ui::MODIFY);
    if cfg!(windows) {
        assert!(21.0 + word + 4.0 <= w, "\"Modify\" needs {word} px");
    }
}

#[test]
fn the_wheel_while_a_menu_is_open_belongs_to_the_menus() {
    let dir = scratch("wheelmenu");
    let text: String = (0..300).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.join("a.txt"), text).unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.areas.text = Rect::new(0.0, 80.0, 1280.0, 600.0);
    let notch = MouseScrollDelta::LineDelta(0.0, -3.0);
    let top = |a: &App| a.docs.active_ready().unwrap().view.top;
    // No menu: the wheel over the text scrolls it.
    a.ui.mouse = (100.0, 300.0);
    a.wheel(notch);
    let scrolled = top(&a);
    assert!(scrolled > 0.0, "the control: the wheel scrolls the text");

    // A menu that fits: over it, and beside it, the wheel does nothing at all.
    let r = placed_menu(&mut a, PopupKind::View, 800.0);
    assert!(!a.ui.popup.as_mut().unwrap().menu.scrolls());
    a.ui.mouse = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    a.wheel(notch);
    assert!(a.ui.popup.is_some(), "over a menu that does not scroll");
    assert_eq!(top(&a), scrolled);
    a.ui.mouse = (100.0, 300.0);
    a.wheel(notch);
    assert!(a.ui.popup.is_some(), "outside all the menus");
    assert_eq!(top(&a), scrolled);

    // A submenu open as well: outside both, nothing; over the one that fits, nothing.
    a.ui.popup = None;
    let r = placed_menu(&mut a, PopupKind::Main, 800.0);
    a.open_popup(PopupKind::View);
    assert!(
        a.ui.popup
            .as_ref()
            .is_some_and(|p| p.kind == PopupKind::View)
    );
    a.ui.mouse = (5.0, 300.0);
    a.wheel(notch);
    assert!(a.ui.popup.is_some() && top(&a) == scrolled);
    let _ = r;

    // A menu taller than its room scrolls under the wheel, and only that.
    a.ui.popup = None;
    let r = placed_menu(&mut a, PopupKind::Main, 200.0);
    assert!(a.ui.popup.as_mut().unwrap().menu.scrolls());
    a.ui.mouse = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    a.wheel(notch);
    assert!(a.ui.popup.as_mut().unwrap().menu.scroll() > 0.0);
    assert_eq!(top(&a), scrolled);
    a.ui.mouse = (5.0, 300.0);
    let before = a.ui.popup.as_mut().unwrap().menu.scroll();
    a.wheel(notch);
    assert!(a.ui.popup.is_some());
    assert_eq!(a.ui.popup.as_mut().unwrap().menu.scroll(), before);
    assert_eq!(top(&a), scrolled);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_wheel_never_reaches_the_text_while_the_palette_is_open() {
    let dir = scratch("wheelpalette");
    let text: String = (0..300).map(|n| format!("line {n}\n")).collect();
    std::fs::write(dir.join("a.txt"), text).unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.areas.text = Rect::new(0.0, 80.0, 1280.0, 600.0);
    ctrl_p(&mut a);
    assert!(a.palette_open());
    let card = {
        let p = a.palette.open.as_mut().unwrap();
        p.palette.layout(Rect::new(0.0, 0.0, 1280.0, 800.0), 40.0);
        p.palette.rect()
    };
    let scroll = |a: &App| a.palette.open.as_ref().unwrap().palette.scroll();
    let top = |a: &App| a.docs.active_ready().unwrap().view.top;
    let notch = MouseScrollDelta::LineDelta(0.0, -3.0);
    // Beside the card (over the text below it): nothing moves.
    a.ui.mouse = (2.0, 790.0);
    assert!(!card.contains(2.0, 790.0));
    a.wheel(notch);
    assert!(a.palette_open());
    assert_eq!((scroll(&a), top(&a)), (0.0, 0.0));
    // Over it: the text beneath does not move (the list's own range is known only once drawn,
    // so how far it scrolls is the shared palette's test).
    a.ui.mouse = (card.x + card.w / 2.0, card.y + card.h / 2.0);
    a.wheel(notch);
    assert!(a.palette_open());
    assert_eq!(top(&a), 0.0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn replace_over_several_lines_offers_the_selection_after_escape_or_the_close_button() {
    let dir = scratch("replscope");
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    let select = |a: &mut App, from: usize, to: usize| {
        a.docs.active_ready_mut().unwrap().view.sel = Selection {
            anchor: from,
            caret: to,
            goal_col: None,
        };
    };
    let scope = |a: &App| {
        let r = a.docs.active_ready().unwrap();
        (r.view.replace_scope.clone(), r.view.scope_on)
    };
    // Order one: open the strip over a caret, Escape in the find box (the strip stays open, the
    // keyboard goes to the text), select several lines, Ctrl+H again.
    a.command(C::Replace);
    assert!(a.ui.replace_open && scope(&a).0.is_none());
    press(&mut a, KeyName::Escape, None);
    assert!(a.ui.replace_open && !a.ui.find_focused);
    select(&mut a, 1, 10);
    a.command(C::Replace);
    assert_eq!(scope(&a), (Some(1..10), true));
    // Pressed again over a caret-only change of selection, the range is kept.
    select(&mut a, 2, 2);
    a.command(C::Replace);
    assert_eq!(scope(&a), (Some(1..10), true));
    // Order two: close with the button first, then open over the selection.
    a.close_replace();
    assert_eq!(scope(&a), (None, false));
    select(&mut a, 1, 10);
    a.command(C::Replace);
    assert_eq!(scope(&a), (Some(1..10), true));
    // Ctrl+H with the find box focused and the strip closed behaves the same.
    a.close_replace();
    a.command(C::Find);
    a.command(C::Replace);
    assert_eq!(scope(&a), (Some(1..10), true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bookmarked_lines_can_be_deleted_in_one_step_and_the_commands_grey_without_marks() {
    use crate::commands::CommandId as C;
    let dir = scratch("marked-lines");
    std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\nfour\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    let on = |a: &App, c: C| crate::palette::enabled(c, &a.docs, &a.ui, false);
    for c in [
        C::CutBookmarkedLines,
        C::CopyBookmarkedLines,
        C::DeleteBookmarkedLines,
    ] {
        assert!(!on(&a, c));
    }
    let text = |a: &App| a.docs.active_ready().unwrap().buffer.rope().to_string();
    a.docs.active_ready_mut().unwrap().bookmarks = vec![0, 2];
    for c in [
        C::CutBookmarkedLines,
        C::CopyBookmarkedLines,
        C::DeleteBookmarkedLines,
    ] {
        assert!(on(&a, c));
    }
    // Delete does not touch the clipboard; Cut and Copy do and are not run here.
    a.command(C::DeleteBookmarkedLines);
    assert_eq!(text(&a), "two\nfour\n");
    assert!(a.docs.active_ready().unwrap().bookmarks.is_empty());
    a.command(C::Undo);
    assert_eq!(text(&a), "one\ntwo\nthree\nfour\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_results_panel_takes_the_keyboard_moves_its_row_and_gives_it_back() {
    let dir = scratch("results-keys");
    std::fs::write(dir.join("a.txt"), "foo\nbar\nfoo\nbaz\nfoo\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    // Nothing to go to while the panel is closed.
    a.command(C::FocusResults);
    assert!(!a.ui.results.focused && a.text_focused());
    a.ui.find.set_text("foo");
    a.command(C::FindInTabs);
    // The key that searched gave the panel the keyboard, on the first match.
    assert!(a.ui.results.open && a.ui.results.focused && !a.text_focused());
    assert_eq!(a.ui.results.selected, Some(1));
    // A letter goes nowhere; Down and Up move by match rows and stop at the ends.
    let before = a.docs.active_ready().unwrap().buffer.rope().to_string();
    press(&mut a, KeyName::Char('x'), Some("x"));
    assert_eq!(
        a.docs.active_ready().unwrap().buffer.rope().to_string(),
        before
    );
    press(&mut a, KeyName::Down, None);
    assert_eq!(a.ui.results.selected, Some(2));
    press(&mut a, KeyName::Down, None);
    press(&mut a, KeyName::Down, None);
    assert_eq!(a.ui.results.selected, Some(3));
    press(&mut a, KeyName::Up, None);
    press(&mut a, KeyName::Home, None);
    assert_eq!(a.ui.results.selected, Some(1));
    press(&mut a, KeyName::End, None);
    assert_eq!(a.ui.results.selected, Some(3));
    // The panel has no height here, so a page is one row.
    press(&mut a, KeyName::PageUp, None);
    assert_eq!(a.ui.results.selected, Some(2));
    press(&mut a, KeyName::PageDown, None);
    assert_eq!(a.ui.results.selected, Some(3));
    // Moving only highlights: the text's caret has not moved.
    assert_eq!(a.docs.active_ready().unwrap().view.sel.caret, 0);
    // Enter opens the match (the third "foo", line 5) and the keyboard goes to the text.
    press(&mut a, KeyName::Up, None);
    press(&mut a, KeyName::Enter, None);
    let r = a.docs.active_ready().unwrap();
    assert_eq!((r.view.sel.anchor, r.view.sel.caret), (8, 11));
    assert!(!a.ui.results.focused && a.text_focused());
    // F6 brings the keyboard back to the row last gone to; Escape returns it to the text and
    // leaves the panel open.
    a.command(C::FocusResults);
    assert!(a.ui.results.focused && a.ui.results.selected == Some(2));
    press(&mut a, KeyName::Escape, None);
    assert!(a.ui.results.open && !a.ui.results.focused && a.text_focused());
    // Typing in the find box takes the keyboard from the panel, and a closed panel loses it.
    a.command(C::FocusResults);
    a.command(C::Find);
    assert!(a.ui.find_focused && !a.ui.results.focused);
    a.command(C::FocusResults);
    assert!(a.ui.results.focused && !a.ui.find_focused);
    a.command(C::CloseResults);
    assert!(!a.ui.results.focused && a.text_focused());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn results_of_an_edited_tab_are_stale_at_once_and_true_again_after_a_search() {
    let dir = scratch("results-follow");
    std::fs::write(dir.join("a.txt"), "foo\nbar\nfoo\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.ui.find.set_text("foo");
    a.command(C::FindInTabs);
    let now = Instant::now();
    // Nothing is looked for while nothing changed.
    assert_eq!(a.results_sync(now), None);
    assert!(!a.ui.results.is_stale(1));
    // An edit before the matches: the rows say so and cannot be gone to.
    a.docs.edit(EditOp::Insert("zz\n"), None);
    let due = a.results_sync(now);
    assert!(due.is_some() && a.ui.results.is_stale(1) && a.ui.results.is_stale(2));
    press(&mut a, KeyName::Enter, None);
    let r = a.docs.active_ready().unwrap();
    assert_eq!(r.view.sel.caret, r.view.sel.anchor);
    assert!(
        a.ui.results.focused,
        "the refused Enter leaves the keyboard in the panel"
    );
    // After a pause the tab is searched again and the rows are the new places.
    a.results_sync(now + Duration::from_millis(400));
    let until = Instant::now() + Duration::from_secs(20);
    while a.ui.results.is_stale(1) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
        a.ui.results.poll();
    }
    assert!(!a.ui.results.is_stale(1));
    assert_eq!(a.ui.results.target(1).map(|t| t.2), Some(3..6));
    assert_eq!(a.ui.results.target(2).map(|t| t.2), Some(11..14));
    a.ui.results.select(1);
    press(&mut a, KeyName::Enter, None);
    let r = a.docs.active_ready().unwrap();
    assert_eq!((r.view.sel.anchor, r.view.sel.caret), (3, 6));
    // A closed panel is not looked at.
    a.command(C::CloseResults);
    assert_eq!(a.results_sync(now), None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Lets the results panel catch up with the tabs: syncs after the refresh pause and takes in the
/// answers of the workers until `until` holds.
fn results_catch_up(a: &mut App, until: impl Fn(&App) -> bool) {
    let mut now = Instant::now() + Duration::from_millis(400);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !until(a) && Instant::now() < deadline {
        a.results_sync(now);
        a.ui.results.poll();
        std::thread::sleep(Duration::from_millis(5));
        now += Duration::from_millis(400);
    }
    assert!(
        until(a),
        "the panel did not settle: hits {} files {} tabs {}",
        a.ui.results.hits,
        a.ui.results.files,
        a.docs.tabs.len()
    );
}

fn result_files(a: &App) -> usize {
    a.ui.results.files
}

#[test]
fn a_document_whose_last_match_went_gets_its_row_back_when_the_match_returns() {
    let dir = scratch("results-return");
    std::fs::write(dir.join("a.txt"), "foo\nbar\nfoo\n").unwrap();
    std::fs::write(dir.join("b.txt"), "foo\n").unwrap();
    let mut a = app();
    a.set_folder(dir.clone());
    open_now(&mut a, &dir.join("b.txt"));
    open_now(&mut a, &dir.join("a.txt"));
    for tabs in [true, false] {
        a.ui.find.set_text("foo");
        if tabs {
            a.command(C::FindInTabs);
        } else {
            a.find_in_files();
            pump(&mut a, |a| {
                matches!(a.ui.results.state, crate::results::State::Done { .. })
            });
        }
        assert_eq!(result_files(&a), 2, "tabs: {tabs}");
        // The active document (a.txt) loses its matches: its row goes.
        a.command(C::SelectAll);
        a.docs.edit(EditOp::Insert("none\n"), None);
        results_catch_up(&mut a, |a| result_files(a) == 1);
        // Undo brings the matches back: so does its row, among the others.
        a.command(C::Undo);
        results_catch_up(&mut a, |a| result_files(a) == 2);
        assert_eq!(a.ui.results.hits, 3, "tabs: {tabs}");
        // Typing the word again does the same.
        a.command(C::SelectAll);
        a.docs.edit(EditOp::Insert("none\n"), None);
        results_catch_up(&mut a, |a| result_files(a) == 1);
        a.docs.edit(EditOp::Insert("foo"), None);
        results_catch_up(&mut a, |a| result_files(a) == 2);
        assert_eq!(a.ui.results.hits, 2, "tabs: {tabs}");
        a.command(C::Undo);
        a.command(C::Undo);
        a.command(C::Undo);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn closing_a_document_without_saving_takes_its_discarded_matches_off_the_panel() {
    let dir = scratch("results-discard");
    std::fs::write(dir.join("a.txt"), "foo\nbar\n").unwrap();
    for tabs in [true, false] {
        let mut a = app();
        a.set_folder(dir.clone());
        open_now(&mut a, &dir.join("a.txt"));
        a.ui.find.set_text("foo");
        if tabs {
            a.command(C::FindInTabs);
        } else {
            a.find_in_files();
            pump(&mut a, |a| {
                matches!(a.ui.results.state, crate::results::State::Done { .. })
            });
        }
        assert_eq!(a.ui.results.hits, 1);
        // Two more matches typed, the panel follows them.
        a.docs.edit(EditOp::Insert("foo foo\n"), None);
        results_catch_up(&mut a, |a| a.ui.results.hits == 3);
        // Closed with "Don't save": the panel shows what the file holds.
        let id = a.docs.tabs[0].id;
        a.docs.tabs[0].discard = true;
        a.request(After::CloseTab(id));
        assert!(a.docs.tabs.is_empty());
        results_catch_up(&mut a, |a| a.ui.results.hits == 1);
        assert_eq!((a.ui.results.files, a.ui.results.is_stale(1)), (1, false));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn undoing_cut_or_delete_bookmarked_lines_brings_the_bookmarks_back() {
    let dir = scratch("marks-undo");
    std::fs::write(dir.join("a.txt"), "a\nb\nc\nd\ne\n").unwrap();
    for command in [C::DeleteBookmarkedLines, C::CutBookmarkedLines] {
        // Cut needs a clipboard, which the Linux CI machine has not got: the lines stay then.
        if command == C::CutBookmarkedLines && !cfg!(windows) {
            continue;
        }
        let mut a = app();
        open_now(&mut a, &dir.join("a.txt"));
        for line in [2, 4] {
            a.docs.go_to(line, 1);
            a.docs.active_ready_mut().unwrap().toggle_bookmark();
        }
        a.command(command);
        let r = a.docs.active_ready().unwrap();
        assert_eq!(r.buffer.rope().to_string(), "a\nc\ne\n");
        assert!(r.bookmarks.is_empty());
        // Undo: the lines and their bookmarks are back.
        a.command(C::Undo);
        let r = a.docs.active_ready().unwrap();
        assert_eq!(r.buffer.rope().to_string(), "a\nb\nc\nd\ne\n");
        assert_eq!(r.bookmarks, [1, 3]);
        // Redo takes the lines and the marks away again, and a bookmark set meanwhile stays when
        // Undo brings the others back.
        a.command(C::Redo);
        assert!(a.docs.active_ready().unwrap().bookmarks.is_empty());
        a.docs.go_to(1, 1);
        a.docs.active_ready_mut().unwrap().toggle_bookmark();
        a.command(C::Undo);
        assert_eq!(a.docs.active_ready().unwrap().bookmarks, [0, 1, 3]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn typing_at_two_carets_is_undone_as_one_run_like_typing_at_one() {
    let dir = scratch("multi-undo");
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    a.docs
        .active_ready_mut()
        .unwrap()
        .select_column((0, 0), (1, 0));
    assert_eq!(a.docs.active_ready().unwrap().view.extra.len(), 1);
    type_text(&mut a, "XY");
    let text = |a: &App| a.docs.active_ready().unwrap().buffer.rope().to_string();
    assert_eq!(text(&a), "XYone\nXYtwo\n");
    a.mods = Mods {
        ctrl: true,
        ..Mods::default()
    };
    press(&mut a, KeyName::Char('z'), None);
    a.mods = Mods::default();
    assert_eq!(text(&a), "one\ntwo\n", "one Ctrl+Z takes the whole run");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_results_panel_searches_the_folder_and_kinds_typed_in_its_boxes() {
    let dir = scratch("results-scope");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.md"), "needle\n").unwrap();
    std::fs::write(dir.join("b.txt"), "needle\n").unwrap();
    std::fs::write(dir.join("sub").join("c.txt"), "needle\n").unwrap();
    let mut a = app();
    a.set_folder(dir.clone());
    open_now(&mut a, &dir.join("a.md"));
    a.ui.find.set_text("needle");
    let done = |a: &App| matches!(a.ui.results.state, crate::results::State::Done { .. });
    let names = |a: &App| {
        a.ui.results
            .rows
            .iter()
            .filter_map(|r| match r {
                crate::results::Row::File { name, .. } => Some(name.replace('\\', "/")),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    // Boxes empty: the open folder, every file.
    a.find_in_files();
    pump(&mut a, done);
    assert_eq!(names(&a), ["a.md", "b.txt", "sub/c.txt"]);
    assert!(a.ui.results.scope_shown());
    // Kinds typed in its box; Enter there searches again and the panel takes the keyboard.
    a.ui.focus_field(BarField::ResultsKinds);
    a.ui.results.kinds.set_text("*.txt");
    press(&mut a, KeyName::Enter, None);
    pump(&mut a, done);
    assert_eq!(names(&a), ["b.txt", "sub/c.txt"]);
    assert!(a.ui.results.focused && !a.ui.find_focused);
    // A folder (relative to the open one) as well.
    a.ui.results.folder.set_text("sub");
    a.find_in_files();
    pump(&mut a, done);
    assert_eq!(names(&a), ["c.txt"]);
    // One that is not there: nothing is searched, and the old rows stay.
    a.ui.results.folder.set_text("nowhere");
    a.find_in_files();
    assert_eq!(names(&a), ["c.txt"]);
    // The boxes keep what was typed for the next search, and the replacement uses the same files.
    assert_eq!(a.ui.results.kinds.text(), "*.txt");
    assert!(
        a.ui.results
            .replace
            .searched
            .as_ref()
            .is_some_and(|s| !s.filter.is_empty())
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bookmarks_come_back_when_a_closed_file_is_opened_again_and_across_runs() {
    let dir = scratch("marks-back");
    let file = dir.join("a.txt");
    std::fs::write(&file, "0\n1\n2\n3\n4\n5\n").unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    a.docs.active_ready_mut().unwrap().bookmarks = vec![1, 4];
    let (sig, version) = (a.session_sig(), a.docs.marks.version);
    // While it is open the session would keep them, but nothing changed the signature: they are
    // written at exit, or when a file closes.
    let open = |a: &App| {
        a.docs.marks.snapshot(
            a.docs
                .tabs
                .iter()
                .filter_map(|d| Some((d.path.as_deref()?, d.ready()?.bookmarks.as_slice()))),
        )
    };
    assert_eq!(open(&a).len(), 1);
    assert_eq!(a.session_sig(), sig);
    a.docs.close(i, None);
    assert!(a.docs.marks.version > version);
    assert_ne!(
        a.session_sig(),
        sig,
        "closing a file with bookmarks is a change to save"
    );
    // Opened again: they are back.
    let j = open_now(&mut a, &file);
    assert_eq!(a.docs.tabs[j].ready().unwrap().bookmarks, [1, 4]);
    // Through the session file and into another run.
    let session = Session {
        marks: open(&a),
        ..Session::default()
    };
    let text = session.serialize();
    assert!(text.contains("mark=1,4;"));
    let back = Session::parse(&text);
    assert_eq!(back.marks, session.marks);
    let mut b = app();
    b.docs.marks = crate::marks::Marks::from_session(&back.marks);
    let k = open_now(&mut b, &file);
    assert_eq!(b.docs.tabs[k].ready().unwrap().bookmarks, [1, 4]);
    // The file got shorter meanwhile: the bookmarks past its end are dropped.
    b.docs.close(k, None);
    std::fs::write(&file, "0\n1\n2\n").unwrap();
    let k = open_now(&mut b, &file);
    assert_eq!(b.docs.tabs[k].ready().unwrap().bookmarks, [1]);
    // A file closed with none forgets the old ones.
    b.docs.tabs[k].ready_mut().unwrap().bookmarks.clear();
    b.docs.close(k, None);
    let k = open_now(&mut b, &file);
    assert!(b.docs.tabs[k].ready().unwrap().bookmarks.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The owner: the caret (and the scroll) come back where a file was left, when it is opened
/// again and when mdedit starts again.
#[test]
fn the_caret_and_scroll_come_back_when_a_file_is_opened_again_and_across_runs() {
    use crate::places::{Place, Places};
    let dir = scratch("places-back");
    let file = dir.join("a.txt");
    let text: String = (0..60).map(|i| format!("line {i}\n")).collect();
    std::fs::write(&file, &text).unwrap();
    let mut a = app();
    let i = open_now(&mut a, &file);
    let place = |a: &App, k: usize| a.docs.tabs[k].ready().unwrap().place();
    {
        let r = a.docs.active_ready_mut().unwrap();
        let at = r.buffer.line_range(40).start + 3;
        r.view.sel = mdedit_core::Selection::at(at);
        r.view.top = 30.0;
    }
    let left = Place {
        line: 40,
        col: 3,
        top: 30,
        lang: None,
    };
    assert_eq!(place(&a, i), left);
    let (sig, version) = (a.session_sig(), a.docs.places.version);
    // Moving about is not a change to write at once; closing the file is.
    assert_eq!(a.session_sig(), sig);
    a.docs.close(i, None);
    assert!(a.docs.places.version > version);
    assert_ne!(a.session_sig(), sig);
    // Opened again: the caret and the top are back.
    let j = open_now(&mut a, &file);
    assert_eq!(place(&a, j), left);
    let r = a.docs.tabs[j].ready().unwrap();
    assert_eq!(r.view.sel.caret, r.buffer.line_range(40).start + 3);
    // Through the session file and into another run.
    let session = Session {
        places: a.docs.places.snapshot(
            a.docs
                .tabs
                .iter()
                .filter_map(|d| Some((d.path.as_deref()?, d.ready()?.place()))),
        ),
        ..Session::default()
    };
    let saved = session.serialize();
    assert!(saved.contains("place=40,3,30;"), "{saved}");
    let back = Session::parse(&saved);
    assert_eq!(back.places, session.places);
    let mut b = app();
    b.docs.places = Places::from_session(&back.places);
    let k = open_now(&mut b, &file);
    assert_eq!(place(&b, k), left);
    // The file got shorter meanwhile: the caret goes to what is left of it.
    b.docs.close(k, None);
    std::fs::write(&file, "one\ntwo\nthree").unwrap();
    let k = open_now(&mut b, &file);
    let r = b.docs.tabs[k].ready().unwrap();
    assert_eq!(r.view.sel.caret, r.buffer.line_range(2).start + 3);
    assert_eq!(r.view.top, 2.0);
    // A file closed at its start forgets the old place.
    b.docs.tabs[k]
        .ready_mut()
        .unwrap()
        .restore_place(Place::default());
    b.docs.close(k, None);
    assert!(b.docs.places.get(&file).is_none());
    let k = open_now(&mut b, &file);
    assert_eq!(place(&b, k), Place::default());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn text_sent_as_unicode_packets_is_typed_into_the_text_and_the_palette() {
    use super::input::event_key;
    use winit::keyboard::{Key as WKey, NativeKey};
    let dir = scratch("packet");
    std::fs::write(dir.join("a.txt"), "x").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.txt"));
    let packet = WKey::Unidentified(NativeKey::Unidentified);
    let family = "\u{1f468}\u{200d}\u{1f469}";
    let send = |a: &mut App, text: &str| {
        let key = event_key(&packet, &packet, Some(text), a.mods).expect("the text has a key");
        a.key_pressed(key, Some(text));
    };
    send(&mut a, "\u{e9}");
    send(&mut a, family);
    let r = a.docs.active_ready().unwrap();
    assert_eq!(r.buffer.rope().to_string(), format!("\u{e9}{family}x"));
    ctrl_p(&mut a);
    send(&mut a, "\u{e9}");
    send(&mut a, "t");
    let q = a.palette.open.as_ref().unwrap().palette.query().to_string();
    assert_eq!(q, "\u{e9}t");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_preview_button_shows_for_markdown_and_documents_without_a_file() {
    let dir = scratch("preview-button");
    let (md, rs) = (dir.join("a.md"), dir.join("b.rs"));
    std::fs::write(&md, "# A\n").unwrap();
    std::fs::write(&rs, "fn b() {}\n").unwrap();
    let mut a = app();
    open_now(&mut a, &md);
    a.compose();
    assert_eq!(chrome::preview_button(&a.docs), Some(false));
    assert!(a.hit_rect(Hit::Bar(crate::ui::BarItem::Preview)).is_some());
    open_now(&mut a, &rs);
    a.docs.activate(1);
    a.compose();
    assert_eq!(chrome::preview_button(&a.docs), None);
    assert!(a.hit_rect(Hit::Bar(crate::ui::BarItem::Preview)).is_none());
    a.docs.new_untitled();
    assert_eq!(chrome::preview_button(&a.docs), Some(false));
}

/// Audit 1, finding 10: a file changed on disk while an encoding choice is unsaved is offered
/// as changed, not reloaded over the choice.
#[test]
fn a_change_on_disk_keeps_an_unsaved_encoding_choice() {
    use mdedit_core::encoding::Encoding;
    let dir = scratch("encoding-choice");
    let path = dir.join("a.txt");
    std::fs::write(&path, "one\n").unwrap();
    let mut a = app();
    let i = open_now(&mut a, &path);
    a.docs.activate(i);
    assert!(a.docs.save_in(Encoding::Utf16Le));
    std::fs::write(&path, "one and two\n").unwrap();
    a.docs.check_changed();
    let d = &a.docs.tabs[i];
    assert!(!d.is_loading(), "not reloaded");
    assert!(d.changed_on_disk && d.modified());
    assert_eq!(d.ready().unwrap().info.encoding, Encoding::Utf16Le);
    let _ = std::fs::remove_dir_all(&dir);
}
