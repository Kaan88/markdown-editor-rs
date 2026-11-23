//! The AI review with a tick per change (`docs/ai-library-plan.md` 5.5, D-077), driven through
//! the whole app: Apply puts in what is ticked as one undo step, Space and F7 move through the
//! changes, the scrollbar and F7 reach the last change of a long list, a long line is drawn whole,
//! an unticked change draws dimmed, Copy result copies what Apply would put in, and the preview's
//! page is made again 150 ms after a tick. The text of every case is a file and its rewrite
//! `<name>-after.md` beside it, which `--ai-demo review-hunks` reads.

use super::ai_events::COPIED;
use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::ai::{Hit as AiHit, Panel};
use crate::commands::CommandId as C;
use crate::hunks_ui::{Hit as ListHit, Item, Kind};
use crate::keys::{self, KeyName};

/// Twelve one-letter lines; the rewrite changes four of them, apart from each other.
const OLD: &str = "a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n";
const NEW: &str = "a\nB\nc\nd\nE\nf\ng\nH\ni\nj\nK\nl\n";

/// A review of `old` → `new` on a document of its own, drawn once.
fn reviewing(tag: &str, old: &str, new: &str, kind: &str) -> (App, PathBuf) {
    let dir = scratch(tag);
    std::fs::write(dir.join("a.md"), old).unwrap();
    std::fs::write(dir.join("a-after.md"), new).unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.md"));
    a.ai_demo(kind);
    assert!(matches!(a.ai.panel, Some(Panel::Review(_))), "a review");
    a.compose();
    (a, dir)
}

fn rv(a: &mut App) -> &mut crate::ai::Review {
    a.ai.review().expect("a review")
}

fn text(a: &App) -> String {
    a.docs.active_ready().unwrap().buffer.rope().to_string()
}

fn tick(a: &mut App, i: usize) {
    a.ai_review_action(AiHit::List(ListHit::Tick(i)));
}

fn key(a: &mut App, name: KeyName, shift: bool) {
    a.mods.shift = shift;
    a.key_pressed(keys::Key::Name(name), None);
    a.mods.shift = false;
    a.compose();
}

#[test]
fn apply_puts_in_what_is_ticked_as_one_undo_step() {
    let (mut a, dir) = reviewing("rv-apply", OLD, NEW, "review-hunks");
    assert_eq!(rv(&mut a).hunks.len(), 4);
    tick(&mut a, 1);
    tick(&mut a, 3);
    assert_eq!(rv(&mut a).hunks.ticked(), 2);
    a.compose();
    key(&mut a, KeyName::Enter, false);
    assert!(a.ai.panel.is_none(), "applied");
    assert_eq!(text(&a), "a\nB\nc\nd\ne\nf\ng\nH\ni\nj\nk\nl\n");
    a.command(C::Undo);
    assert_eq!(text(&a), OLD, "one Ctrl+Z brings everything back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_every_change_ticked_apply_is_the_result_to_the_byte() {
    let (mut a, dir) = reviewing("rv-all", OLD, NEW, "review-hunks");
    key(&mut a, KeyName::Enter, false);
    assert_eq!(text(&a), NEW);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn with_nothing_ticked_apply_cannot_be_pressed_and_changes_nothing() {
    let (mut a, dir) = reviewing("rv-none", OLD, NEW, "review-hunks");
    a.ai_review_action(AiHit::UntickAll);
    a.compose();
    assert!(
        !a.ai.hits.iter().any(|(h, _)| *h == AiHit::Apply),
        "no Apply to press"
    );
    key(&mut a, KeyName::Enter, false);
    assert!(a.ai.review().is_some(), "the review stays");
    assert_eq!(text(&a), OLD);
    a.ai_review_action(AiHit::TickAll);
    a.compose();
    assert!(a.ai.hits.iter().any(|(h, _)| *h == AiHit::Apply));
    assert_eq!(rv(&mut a).hunks.ticked(), 4);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn f7_moves_between_changes_and_space_ticks_the_current_one() {
    let (mut a, dir) = reviewing("rv-keys", OLD, NEW, "review-hunks");
    // The demo starts on the third change.
    assert_eq!(rv(&mut a).list.current, 2);
    key(&mut a, KeyName::F(7), false);
    assert_eq!(rv(&mut a).list.current, 3);
    key(&mut a, KeyName::F(7), false);
    assert_eq!(rv(&mut a).list.current, 3, "stops at the last");
    key(&mut a, KeyName::F(7), true);
    key(&mut a, KeyName::F(7), true);
    assert_eq!(rv(&mut a).list.current, 1);
    key(&mut a, KeyName::Space, false);
    assert!(!rv(&mut a).hunks.is_ticked(1));
    assert!(rv(&mut a).hunks.is_ticked(0) && rv(&mut a).hunks.is_ticked(2));
    key(&mut a, KeyName::Space, false);
    assert!(rv(&mut a).hunks.is_ticked(1));
    let _ = std::fs::remove_dir_all(&dir);
}

/// 400 lines with a change every 20 lines: 20 changes, far longer than the list is high.
fn long() -> (String, String) {
    let old: String = (1..=400)
        .map(|i| format!("Line {i} of the notes.\n"))
        .collect();
    let new: String = (1..=400)
        .map(|i| {
            if i % 20 == 0 {
                format!("Line {i} of the NOTES.\n")
            } else {
                format!("Line {i} of the notes.\n")
            }
        })
        .collect();
    (old, new)
}

#[test]
fn the_last_change_of_a_400_line_file_is_reached_by_f7() {
    let (old, new) = long();
    let (mut a, dir) = reviewing("rv-f7", &old, &new, "review-long");
    let n = rv(&mut a).hunks.len();
    assert_eq!(n, 20);
    let last = n - 1;
    // The demo starts far down; back to the first change.
    a.ai_review_action(AiHit::Mark(0));
    a.compose();
    assert_eq!(rv(&mut a).list.current, 0);
    assert!(!rv(&mut a).list.visible(last), "not in view to begin with");
    for _ in 0..n {
        key(&mut a, KeyName::F(7), false);
    }
    assert_eq!(rv(&mut a).list.current, last);
    assert!(rv(&mut a).list.visible(last), "scrolled to it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_last_change_is_reached_by_dragging_the_scrollbar() {
    let (old, new) = long();
    let (mut a, dir) = reviewing("rv-drag", &old, &new, "review-long");
    let (track, thumb) = {
        let r = rv(&mut a);
        crate::hunks_ui::bar(r.list.area, &r.list).expect("a scrollbar: the list is long")
    };
    let (x, y) = (thumb.cx(), thumb.cy());
    a.ui.mouse = (x, y);
    a.ai_press();
    assert!(rv(&mut a).grab.is_some(), "the thumb is taken");
    a.ui.mouse = (x, track.bottom());
    a.ai_pointer(x, track.bottom());
    a.ai_release();
    a.compose();
    let r = rv(&mut a);
    assert_eq!(r.list.scroll, r.list.max_scroll(), "at the end");
    assert!(r.list.visible(r.hunks.len() - 1));
    assert!(r.grab.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_press_on_the_track_brings_the_thumb_there() {
    let (old, new) = long();
    let (mut a, dir) = reviewing("rv-track", &old, &new, "review-long");
    let track = {
        let r = rv(&mut a);
        crate::hunks_ui::bar(r.list.area, &r.list).unwrap().0
    };
    // The demo starts far down the list; a press near the top of the track brings it back up.
    let before = rv(&mut a).list.scroll;
    assert!(before > 0.0);
    a.ui.mouse = (track.cx(), track.y + 4.0);
    a.ai_press();
    a.ai_release();
    assert!(rv(&mut a).list.scroll < before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_300_character_line_is_drawn_whole() {
    let words: String = (0..60).map(|i| format!("word{i} ")).collect();
    let long_line = words.trim_end().to_string();
    assert!(long_line.len() > 300);
    let old = "short\nkeep\n".to_string();
    let new = format!("{long_line}\nkeep\n");
    let (mut a, dir) = reviewing("rv-wrap", &old, &new, "review-hunks");
    let lay = rv(&mut a).list.layout().expect("drawn").clone();
    let added = lay
        .items
        .iter()
        .find_map(|p| match &p.item {
            Item::Line {
                kind: Kind::Added,
                text,
                rows,
                ..
            } => Some((p.h, text.clone(), rows.clone())),
            _ => None,
        })
        .expect("the long line");
    assert!(added.2.len() >= 2, "it wraps: {:?}", added.2);
    assert!(
        added.0 >= 2.0 * crate::hunks_ui::LINE_H,
        "its row is taller"
    );
    let joined: String = added.2.iter().map(|r| &added.1[r.clone()]).collect();
    assert_eq!(joined, long_line, "every character is in some row");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The colour at the middle of the first added line of change `i`, as the window drew it.
fn added_pixel(a: &mut App, i: usize) -> [u8; 3] {
    let r = rv(a);
    let (area, scroll) = (r.list.area, r.list.scroll);
    let lay = r.list.layout().expect("drawn");
    let y = lay
        .items
        .iter()
        .find_map(|p| match &p.item {
            Item::Line {
                change: Some(c),
                kind: Kind::Added,
                ..
            } if *c == i => Some(p.y),
            _ => None,
        })
        .expect("an added line");
    // Far to the right of the text, on the tint alone.
    let (x, y) = (area.right() - 60.0, area.y + y - scroll + 10.0);
    let scale = a.scale;
    let (px, py) = ((x * scale) as u32, (y * scale) as u32);
    let o = ((py * a.screen.width + px) * 4) as usize;
    a.screen.pixels[o..o + 3].try_into().unwrap()
}

#[test]
fn a_change_that_is_not_ticked_is_drawn_dimmed_and_says_so() {
    let (mut a, dir) = reviewing("rv-dim", OLD, NEW, "review-hunks");
    // Change 2 (the third) is in view and ticked.
    let on = added_pixel(&mut a, 2);
    tick(&mut a, 2);
    a.compose();
    let off = added_pixel(&mut a, 2);
    assert_ne!(on, off, "the tint fades when the change is not taken");
    tick(&mut a, 2);
    a.compose();
    assert_eq!(added_pixel(&mut a, 2), on, "and comes back");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn copy_result_copies_what_apply_would_put_in() {
    let (mut a, dir) = reviewing("rv-copy", OLD, NEW, "review-hunks");
    tick(&mut a, 0);
    tick(&mut a, 2);
    a.ai_review_action(AiHit::Copy);
    let copied = COPIED.with(|c| c.borrow().clone());
    // Changes 0 and 2 (B, H) are not taken; 1 and 3 (E, K) are.
    assert_eq!(copied, "a\nb\nc\nd\nE\nf\ng\nh\ni\nj\nK\nl\n");
    assert!(a.ai.panel.is_none(), "closed once copied, as before");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_mark_on_the_strip_goes_to_its_change_and_the_arrows_step() {
    let (old, new) = long();
    let (mut a, dir) = reviewing("rv-strip", &old, &new, "review-long");
    a.ai_review_action(AiHit::Mark(5));
    a.compose();
    assert_eq!(rv(&mut a).list.current, 5);
    assert!(rv(&mut a).list.visible(5));
    a.ai_review_action(AiHit::Next);
    a.compose();
    assert_eq!(rv(&mut a).list.current, 6);
    a.ai_review_action(AiHit::Prev);
    a.ai_review_action(AiHit::Prev);
    assert_eq!(rv(&mut a).list.current, 4);
    // Every change has a mark that takes the pointer.
    let marks =
        a.ai.hits
            .iter()
            .filter(|(h, _)| matches!(h, AiHit::Mark(_)))
            .count();
    assert_eq!(marks, 20);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_preview_page_is_made_again_150_ms_after_a_tick() {
    let (mut a, dir) = reviewing("rv-preview", OLD, NEW, "review-preview-partial");
    assert!(rv(&mut a).preview);
    // The demo left four of them unticked, so the page was first made with those.
    let sent = rv(&mut a).sent;
    let before = Instant::now();
    tick(&mut a, 0);
    let due = rv(&mut a).due.expect("a tick not yet shown");
    assert!(
        due >= before + crate::compare::DEBOUNCE,
        "150 ms after the tick, not at once"
    );
    assert_eq!(rv(&mut a).sent, sent, "not at once");
    // Not yet.
    assert!(a.ai_review_due(due - Duration::from_millis(100)).is_some());
    assert_eq!(rv(&mut a).sent, sent);
    // Several ticks in a row make one page.
    tick(&mut a, 2);
    let due = rv(&mut a).due.unwrap();
    assert!(a.ai_review_due(due + Duration::from_millis(1)).is_none());
    assert_eq!(rv(&mut a).sent, sent + 1, "once");
    assert!(rv(&mut a).due.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
