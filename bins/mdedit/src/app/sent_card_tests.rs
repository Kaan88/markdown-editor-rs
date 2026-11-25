//! "What is sent" for Write notes (pictures plan 13): the card over the window, a tab per
//! request, the text as "Open the text in a tab" writes it, the selection, and the first-time
//! sentence said once ever.

use super::ai_tests::{engine, wait};
use super::files_tests::{app, scratch};
use super::sent_card::COPIED;
use super::*;
use crate::keys::KeyName;
use crate::notes_task::Measure;
use crate::sent_card::{CardHit, SentCard, wrap};
use crate::task_ui::TaskKind;
use mdedit_core::ai::Connection;

/// `n` pictures (the stand-in engine never reads their bytes) and a text file.
fn files(dir: &Path, n: usize) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = (0..n)
        .map(|i| {
            let p = dir.join(format!("room{i}.png"));
            std::fs::write(&p, b"not read").expect("picture");
            p
        })
        .collect();
    let list = dir.join("list.md");
    std::fs::write(&list, "# List\n\n- Grout, blue\n").expect("list");
    out.push(list);
    out
}

/// Write notes over five pictures and a text file, measured, with an instruction.
fn window(tag: &str, on: bool) -> (App, PathBuf) {
    engine();
    let dir = scratch(tag);
    let mut a = app();
    a.session_path = Some(dir.join("settings.ini"));
    a.library_rows();
    std::fs::create_dir_all(dir.join("library")).expect("library");
    if on {
        a.docs.prefs.ai_connection = Connection::ClaudeApi;
    }
    let files = files(&dir, 5);
    a.task_open(TaskKind::Notes, files);
    if on {
        wait(&mut a, |a| {
            a.task.as_ref().is_some_and(|s| {
                s.prepare.is_empty()
                    && s.files
                        .iter()
                        .filter(|f| f.kind == crate::task_ui::FileKind::Picture)
                        .all(|f| matches!(f.measure, Measure::Ready(_)))
            })
        });
    }
    if let Some(s) = a.task.as_mut() {
        s.instruction.set_text("SENTCARD: one note per room");
    }
    (a, dir)
}

fn card(a: &App) -> &SentCard {
    a.task
        .as_ref()
        .and_then(|s| s.sent.as_ref())
        .expect("the card is open")
}

fn hit(a: &App, h: CardHit) -> Rect {
    card(a)
        .hits
        .iter()
        .find(|(k, _)| *k == h)
        .map(|(_, r)| *r)
        .unwrap_or_else(|| panic!("{h:?} is drawn"))
}

fn click(a: &mut App, r: Rect) {
    a.ui.mouse = (r.cx(), r.cy());
    a.task_pointer(r.cx(), r.cy());
    a.task_press();
    a.task_release();
}

/// Pictures plan 13: on Write notes "Show what is sent" is a card over the window, a tab per
/// request (the looks of four, then the arrange), and no tab is opened; the window stays as it
/// was under it, and nothing is sent.
#[test]
fn show_what_is_sent_on_write_notes_is_a_card_with_a_tab_per_request() {
    let (mut a, _dir) = window("sent-card", true);
    let tabs_before = a.docs.tabs.len();
    a.task_show_sent();
    assert!(a.task_parked.is_none(), "the window is not parked");
    assert_eq!(a.docs.tabs.len(), tabs_before, "no tab is opened");
    let c = card(&a);
    let labels: Vec<&str> = c.tabs.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels, ["Look 1 of 2", "Look 2 of 2", "Arrange"]);
    assert_eq!(c.tabs[0].pictures, [0, 1, 2, 3]);
    assert_eq!(c.tabs[1].pictures, [4]);
    assert!(
        c.tabs[2].pictures.is_empty(),
        "no picture is in the arrange"
    );
    assert_eq!(a.task_sent.requests, 0, "the card sends nothing");
}

/// "Open the text in a tab" keeps what exists: each tab's text is, word for word, in what the
/// text tab shows, the arrange with the group's sections as the arrange request sends them.
#[test]
fn each_tab_s_text_is_word_for_word_what_the_text_tab_shows() {
    let (mut a, _dir) = window("sent-words", true);
    a.task_show_sent();
    let whole = a.task_request_text().expect("the text");
    let c = card(&a);
    for t in &c.tabs {
        assert!(whole.contains(t.system), "{}", t.label);
        assert!(whole.contains(&t.prompt), "{}: {}", t.label, t.prompt);
        assert!(t.prompt.contains("SENTCARD: one note per room"));
    }
    assert!(
        c.tabs[2].prompt.contains("# List"),
        "the text file goes whole"
    );
    assert!(c.text.starts_with("SYSTEM:\n"));
}

/// The button does what "Show what is sent" did before the card: the window parked, the text
/// in a read-only tab.
#[test]
fn open_the_text_in_a_tab_closes_the_card_and_opens_the_text() {
    let (mut a, _dir) = window("sent-tab", true);
    a.task_show_sent();
    a.compose();
    let b = hit(&a, CardHit::OpenTab);
    click(&mut a, b);
    assert!(a.task.is_none(), "the window is parked behind the tab");
    let parked = a.task_parked.as_ref().expect("parked");
    assert!(parked.sent.is_none(), "the card is closed");
    let text = a
        .docs
        .active_doc()
        .and_then(|d| d.ready())
        .map(|r| r.buffer.rope().to_string())
        .expect("a tab");
    assert!(text.contains("LOOK REQUEST 1 of 2"), "{text}");
}

/// Close and Escape go back to the window as it was; a click over the window while the card is
/// open reaches the card, never a row under it.
#[test]
fn escape_closes_the_card_and_leaves_the_window_as_it_was() {
    let (mut a, _dir) = window("sent-esc", true);
    a.compose();
    let row = a
        .task
        .as_ref()
        .and_then(|s| {
            s.hits
                .iter()
                .find(|(h, _)| *h == crate::task_ui::Hit::File(0))
                .map(|(_, r)| *r)
        })
        .expect("a picture's row");
    a.task_show_sent();
    a.compose();
    click(&mut a, row);
    assert!(
        a.task.as_ref().expect("open").files[0].ticked,
        "the row under the card was not clicked"
    );
    a.task_key(Some(KeyName::Escape), None);
    let s = a.task.as_ref().expect("the window stays");
    assert!(s.sent.is_none());
    assert_eq!(s.instruction.text(), "SENTCARD: one note per room");
    a.task_show_sent();
    a.compose();
    let close = hit(&a, CardHit::Close);
    click(&mut a, close);
    assert!(a.task.as_ref().is_some_and(|s| s.sent.is_none()));
}

/// With the AI off the card says nothing is sent, and has no request to show.
#[test]
fn with_the_ai_off_the_card_says_nothing_is_sent() {
    let (mut a, _dir) = window("sent-off", false);
    a.task_show_sent();
    let c = card(&a);
    assert!(c.off);
    assert!(c.tabs.is_empty(), "no request");
    assert!(c.text.is_empty());
}

/// A tab clicked shows its request: the arrange's text, from its top.
#[test]
fn a_click_on_a_tab_shows_that_request() {
    let (mut a, _dir) = window("sent-tabs", true);
    a.task_show_sent();
    a.compose();
    let arrange = hit(&a, CardHit::Tab(2));
    click(&mut a, arrange);
    let c = card(&a);
    assert_eq!(c.tab, 2);
    assert!(c.text.contains(mdedit_core::notes_ai::ARRANGE_SYSTEM));
    a.task_key(Some(KeyName::Left), None);
    assert_eq!(card(&a).tab, 1, "Left goes to the request before");
}

/// The text is selectable: a drag over it selects what it went over, and Ctrl+C copies that;
/// with nothing selected Ctrl+C copies the whole request.
#[test]
fn a_drag_over_the_text_selects_it_and_ctrl_c_copies_it() {
    let (mut a, _dir) = window("sent-select", true);
    a.task_show_sent();
    a.compose();
    let text = hit(&a, CardHit::Text);
    let (x0, y0) = (card(&a).text_x, text.y + crate::sent_card::TEXT_PAD + 4.0);
    // From the start of the first line to the middle of the third.
    let (x1, y1) = (x0 + 120.0, y0 + 2.0 * crate::sent_card::LINE_H);
    a.ui.mouse = (x0 - 2.0, y0);
    a.task_press();
    a.task_pointer(x1, y1);
    a.ui.mouse = (x1, y1);
    a.task_release();
    let c = card(&a);
    let (lo, hi) = c.selection().expect("a selection");
    assert_eq!(lo, 0);
    let selected = c.selected_text().expect("text").to_string();
    assert!(
        selected.starts_with("SYSTEM:\nYou look at pictures"),
        "{selected}"
    );
    assert!(hi > c.lines[1].1, "it reaches the third line");
    a.mods.ctrl = true;
    a.task_key(Some(KeyName::Char('c')), None);
    assert_eq!(COPIED.with(|c| c.borrow().clone()), selected);
    // Nothing selected: the whole request.
    if let Some(c) = a.task.as_mut().and_then(|s| s.sent.as_mut()) {
        c.sel = None;
    }
    a.task_key(Some(KeyName::Char('c')), None);
    assert_eq!(COPIED.with(|c| c.borrow().clone()), card(&a).text);
}

/// Long lines wrap at their last space that fits, or between two characters when none does,
/// and no byte is lost or doubled.
#[test]
fn a_long_line_wraps_at_a_space_and_keeps_every_byte() {
    let text = "one two three\n\nabcdefghij k\u{00B7}l";
    let mut w = |_: char| 1.0;
    let lines = wrap(text, 5.0, &mut w);
    let got: Vec<&str> = lines.iter().map(|&(a, b)| &text[a..b]).collect();
    assert_eq!(
        got,
        ["one ", "two ", "three", "", "abcde", "fghij", " k\u{00B7}l"]
    );
    // Narrower than a character: one a line, and it ends.
    let lines = wrap("abc", 0.5, &mut w);
    assert_eq!(lines, [(0, 1), (1, 2), (2, 3)]);
}

/// Pictures plan 13: the first window ever of Write notes with a picture and the AI on says
/// once that a picture is sent whole; the settings keep it said, and no later window says it.
/// With the AI off it is not said, and not kept as said.
#[test]
fn a_picture_is_sent_whole_is_said_by_the_first_window_only() {
    let (mut a, dir) = window("sent-whole-off", false);
    assert!(!a.task.as_ref().expect("open").whole_note);
    assert!(!a.docs.prefs.ai_pictures_told, "not said with the AI off");
    a.task_close();
    a.docs.prefs.ai_connection = Connection::ClaudeApi;
    a.task_open(TaskKind::Notes, files(&dir, 1));
    let s = a.task.as_ref().expect("open");
    assert!(s.whole_note && crate::sent_card::shows_whole(s));
    assert!(a.docs.prefs.ai_pictures_told, "kept as said");
    a.task_close();
    a.task_open(TaskKind::Notes, files(&dir, 1));
    assert!(
        !a.task.as_ref().expect("open").whole_note,
        "a later window does not say it"
    );
}

/// Fable's review of V4 and V5, round 2: after "Arrange again…" the arrange tab shows the
/// instruction it was asked with, not the window's.
#[test]
fn the_arrange_tab_shows_the_arrange_again_instruction() {
    let (mut a, _dir) = window("sent-arrange-again", true);
    if let Some(s) = a.task.as_mut() {
        s.notes_run = Some(crate::notes_task::Run {
            arrange_instruction: Some("By room, please".to_string()),
            ..Default::default()
        });
        let tabs = crate::sent_card::tabs(s, "The window's own", &[]);
        let arrange = tabs.last().expect("the arrange tab");
        assert!(
            arrange.prompt.contains("By room, please"),
            "{}",
            arrange.prompt
        );
    }
}
