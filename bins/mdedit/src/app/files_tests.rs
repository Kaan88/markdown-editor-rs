//! Headless tests of the window logic for the command palette, compare mode and replace in
//! files: an `App` built without a window, driven by the same functions the events call.

use super::*;
use crate::keys::{Key, KeyName};
use crate::palette::{Mode, Target, mode_of};
use std::sync::atomic::{AtomicU32, Ordering as AtomicOrdering};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A fresh scratch folder.
pub(super) fn scratch(tag: &str) -> PathBuf {
    let n = NEXT.fetch_add(1, AtomicOrdering::Relaxed);
    let dir = std::env::temp_dir().join(format!("mdedit-{tag}-{}-{n}", std::process::id()));
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

fn type_text(app: &mut App, text: &str) {
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
    assert_eq!(r.buffer.line_of(r.sel.caret), 11);
    // Line and column.
    a.open_palette(":3:4");
    assert_eq!(a.palette.open.as_ref().unwrap().line_query(), Some((3, 4)));
    press(&mut a, KeyName::Enter, None);
    let r = a.docs.active_ready().unwrap();
    assert_eq!(r.line_col(r.sel.caret), (2, 3));
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
    assert_eq!(r.buffer.line_of(r.sel.caret), 2);
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
