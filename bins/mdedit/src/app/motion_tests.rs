//! The window's animator and the menus' fade: the deadline that wakes the window while a menu
//! enters and none after it (or at idle), the frames that draw only the menus, and everything
//! that keeps a menu instant (reduced motion, the Animations setting, the completion list, a
//! shot or a test).

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::motion::{MENUS, Motion};
use mdshared_ui::anim::{ManualClock, Policy};
use mdshared_ui::menu::{Item, Menu};

/// A small file of its own.
fn file() -> std::path::PathBuf {
    let path = scratch("motion").join("a.txt");
    std::fs::write(
        &path,
        "one
two
three
",
    )
    .unwrap();
    path
}

/// A window with `path` open, whose motion moves on a clock moved by hand.
fn moving_on(path: &std::path::Path) -> App {
    let mut a = app();
    a.motion = Motion::moving(ManualClock::new());
    open_now(&mut a, path);
    a
}

fn moving() -> App {
    moving_on(&file())
}

/// The same window, still (as every test and shot is).
fn still_on(path: &std::path::Path) -> App {
    let mut a = moving_on(path);
    a.motion = Motion::new(true);
    a
}

fn still() -> App {
    still_on(&file())
}

fn tick(a: &mut App, ms: u64) {
    if let Some(c) = &a.motion.clock {
        c.advance_ms(ms);
    }
}

fn shown(a: &App) -> f32 {
    a.motion.drawn.map_or(-1.0, |m| m.main.1)
}

/// Opens the main menu and composes frames 16 ms apart until the animator asks for none;
/// returns each frame's damage after the first.
fn open_and_settle(a: &mut App) -> Vec<Vec<(i32, i32, i32, i32)>> {
    a.open_popup(PopupKind::Main);
    a.compose();
    let mut frames = Vec::new();
    for _ in 0..60 {
        if a.motion.due.is_none() {
            break;
        }
        tick(a, 16);
        frames.push(a.compose().0);
    }
    frames
}

#[test]
fn a_menu_fades_in_on_a_timer_that_stops_when_it_arrives() {
    let mut a = moving();
    a.compose();
    assert_eq!(a.motion.due, None, "nothing moves at idle");
    a.open_popup(PopupKind::Main);
    a.compose();
    assert!(
        a.motion.due.is_some(),
        "a frame is asked for while the menu enters"
    );
    assert!(shown(&a) < 1.0, "the menu starts on its way in");
    assert!(
        a.motion
            .anim
            .group_deadlines()
            .iter()
            .any(|(g, _)| *g == mdshared_ui::anim::Id::new(MENUS)),
        "the menus' group is what moves"
    );
    let mut frames = 0;
    while a.motion.due.is_some() && frames < 60 {
        tick(&mut a, 16);
        a.compose();
        frames += 1;
    }
    // 117 ms (the owner's) at 16 ms a frame, and the frame that shows it arrived.
    assert!((7..=9).contains(&frames), "{frames} frames");
    assert_eq!(shown(&a), 1.0);
    assert_eq!(a.motion.due, None, "no frame once it is there");
    tick(&mut a, 500);
    a.compose();
    assert_eq!(a.motion.due, None, "and none after, with the menu open");
    assert!(a.motion.backing.is_none(), "the pixels under it are let go");
    // Closing is instant: the menu is gone the next frame, and nothing is asked for.
    a.ui.popup = None;
    a.compose();
    assert_eq!((a.motion.drawn, a.motion.due), (None, None));
}

/// The deadline is a timer: it wakes the window once and is cleared when it fires.
#[test]
fn the_deadline_is_a_timer_cleared_when_it_fires() {
    let mut a = moving();
    let now = Instant::now();
    a.motion.due = Some(now + Duration::from_millis(16));
    a.wants_frame = false;
    assert!(
        a.timers(now)
            .is_some_and(|t| t <= now + Duration::from_millis(16))
    );
    assert!(!a.wants_frame, "not before its time");
    let later = now + Duration::from_millis(17);
    a.timers(later);
    assert!(a.wants_frame, "a frame when it comes");
    assert_eq!(a.motion.due, None, "and the timer is gone");
}

/// While the menu enters, a frame draws and presents only the menus' bounds, and the menu ends
/// on exactly the pixels a menu opened at once has.
#[test]
fn while_a_menu_fades_only_its_bounds_are_drawn_and_it_ends_as_drawn_at_once() {
    let mut a = moving();
    a.compose();
    let frames = open_and_settle(&mut a);
    assert!(frames.len() > 5);
    let bounds = {
        let popup = a.ui.popup.as_ref().unwrap();
        let p = Painter::new(&mut a.screen, &mut a.gfx, a.scale);
        p.dev_rect(crate::motion::popup_bounds(popup))
    };
    for (i, damage) in frames.iter().enumerate() {
        assert_eq!(damage.len(), 1, "frame {i}: one rectangle");
        let d = damage[0];
        assert!(
            d.0 >= bounds.0.max(0) && d.1 >= bounds.1.max(0) && d.2 <= bounds.2 && d.3 <= bounds.3,
            "frame {i}: {d:?} inside the menus' bounds {bounds:?}"
        );
    }
    let mut b = still();
    b.compose();
    b.open_popup(PopupKind::Main);
    b.compose();
    assert!(
        a.screen.pixels == b.screen.pixels,
        "the faded menu is the menu"
    );
}

/// Half way in, the menu is neither absent nor there.
#[test]
fn half_way_the_menu_is_part_drawn() {
    let mut b = still();
    b.compose();
    let before = b.screen.pixels.clone();
    b.open_popup(PopupKind::Main);
    b.compose();
    let rest = b.screen.pixels.clone();
    let mut a = moving();
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    for _ in 0..4 {
        tick(&mut a, 16);
        a.compose();
    }
    let t = shown(&a);
    assert!(t > 0.0 && t < 1.0, "{t}");
    assert!(a.screen.pixels != rest && a.screen.pixels != before);
}

#[test]
fn reduced_motion_opens_a_menu_at_once() {
    let mut a = moving();
    a.motion.set_reduced(true);
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    assert_eq!(shown(&a), 1.0);
    assert_eq!(a.motion.due, None);
}

#[test]
fn the_animations_setting_off_opens_a_menu_at_once() {
    let mut a = moving();
    a.docs.prefs.animations = false;
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    assert_eq!(shown(&a), 1.0);
    assert_eq!(a.motion.due, None);
    assert_eq!(a.motion.policy(), Policy::Never);
    // Back on, the next menu enters again.
    a.docs.prefs.animations = true;
    a.ui.popup = None;
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    assert!(shown(&a) < 1.0 && a.motion.due.is_some());
}

/// The switch on Appearance turns it off, and the file keeps it.
#[test]
fn the_animations_switch_is_saved() {
    let mut a = moving();
    assert!(a.docs.prefs.animations, "on by default");
    a.open_settings();
    let f = a.ui.settings.as_mut().unwrap();
    assert!(f.focus_row(crate::app::modal::ANIMATIONS));
    a.key_pressed(
        crate::keys::Key::Name(mdshared_ui::keys::KeyName::Space),
        None,
    );
    assert!(!a.docs.prefs.animations && !a.session.prefs.animations);
    assert!(a.session.serialize().contains("animations=0"));
}

/// The word completion list follows the typing: it never fades.
#[test]
fn the_completion_list_never_fades() {
    let mut a = moving();
    a.compose();
    let items = vec![Item::new(Act::Complete(0), "three".to_string())];
    a.ui.popup = Some(crate::ui::Popup::new(
        PopupKind::Complete,
        Menu::new(items, 80.0, 120.0, false),
    ));
    a.compose();
    assert_eq!(a.motion.drawn, None, "not a menu that fades");
    assert_eq!(a.motion.due, None, "no frame asked for");
    assert!(a.motion.anim.group_deadlines().is_empty());
}

/// Shots and tests never move; a window does, unless the setting is off.
#[test]
fn shots_and_tests_never_move() {
    assert_eq!(app().motion.policy(), Policy::Never, "a test");
    // A shot holds still, a window does not: what App::new is given besides `cfg(test)`.
    let shot = crate::args::Args {
        shot: Some(std::path::PathBuf::from("x.png")),
        ..Default::default()
    };
    assert!(crate::motion::still_for(&shot), "a shot");
    assert!(!crate::motion::still_for(&Default::default()), "a window");
    assert_eq!(Motion::new(true).policy(), Policy::Never);
    let mut m = Motion::new(false);
    assert_eq!(m.policy(), Policy::System, "a window follows the system");
    m.set_setting(false);
    assert_eq!(m.policy(), Policy::Never, "the setting off");
}

/// A window that draws no frame (covered, minimised: no redraw comes) is woken once by the
/// animator's timer and is not woken again for it: the timer is gone once it fired.
#[test]
fn a_window_that_draws_nothing_is_woken_once() {
    let mut a = moving();
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    let due = a.motion.due.expect("a menu entering asks for a frame");
    a.wants_frame = false;
    a.timers(due);
    assert!(a.wants_frame, "woken when it comes");
    // No frame is drawn: no compose, so no new deadline. (Unfocused: no caret blinks.)
    a.focused = false;
    a.wants_frame = false;
    let later = due + Duration::from_secs(5);
    a.timers(later);
    assert_eq!(a.motion.due, None, "the timer fired once and is gone");
    assert!(!a.wants_frame, "and wakes the window no more");
}

/// The system's wish changed: the wake reads it again and draws.
#[test]
fn a_motion_wake_reads_the_system_again() {
    let mut a = moving();
    a.motion.set_reader(|| true);
    assert!(
        a.motion.anim.animates(),
        "nothing asked for less motion yet"
    );
    a.wants_frame = false;
    a.woken(Wake::Motion);
    assert!(
        !a.motion.anim.animates(),
        "the system's new answer is taken"
    );
    assert!(a.wants_frame, "and a frame to show it");
    a.motion.set_reader(|| false);
    a.woken(Wake::Motion);
    assert!(a.motion.anim.animates(), "and the next one");
}

/// Nothing reads or watches the system's wish while no menu that fades has opened (the window
/// starts the watch once it settled): not composing with no menu, nor with the completion list.
#[test]
fn no_watch_without_a_menu_that_fades() {
    let mut a = moving();
    a.motion = Motion::new(false);
    a.compose();
    assert!(!a.motion.watching(), "no menu");
    let items = vec![Item::new(Act::Complete(0), "three".to_string())];
    a.ui.popup = Some(crate::ui::Popup::new(
        PopupKind::Complete,
        Menu::new(items, 80.0, 120.0, false),
    ));
    a.compose();
    assert!(!a.motion.watching(), "the completion list");
    // Motion off: the settled window starts no watch either.
    a.docs.prefs.animations = false;
    a.watch_motion();
    assert!(!a.motion.watching(), "the setting off");
}

/// A shot's clock reaches its time in frames of at most 16 ms, and stops at a second.
#[test]
fn a_shot_steps_its_clock_in_frames() {
    let steps: Vec<u64> = crate::motion::shot_steps(60).collect();
    assert_eq!(steps, [16, 16, 16, 12]);
    assert_eq!(crate::motion::shot_steps(0).count(), 0);
    let all: u64 = crate::motion::shot_steps(u32::MAX).sum();
    assert_eq!(all, u64::from(crate::motion::SHOT_FADE_MAX_MS));
    assert!(crate::motion::shot_steps(u32::MAX).all(|s| s <= 16));
}

// ---- a menu entering while something else changes: always the pixels of a still window ----

/// Composes 16 ms frames until the animator asks for none.
fn settle(a: &mut App) {
    for _ in 0..80 {
        if a.motion.due.is_none() {
            break;
        }
        tick(a, 16);
        a.compose();
    }
    assert_eq!(a.motion.due, None, "the menus arrived");
}

/// Opens `kind` in the moving window and composes `frames` frames of its entrance.
fn part_way(a: &mut App, kind: PopupKind, frames: usize) {
    a.open_popup(kind);
    a.compose();
    for _ in 0..frames {
        tick(a, 16);
        a.compose();
    }
    let t = shown(a);
    assert!(t > 0.0 && t < 1.0, "part way: {t}");
}

/// Both windows show the same pixels.
fn same(a: &App, b: &App, what: &str) {
    assert_eq!(a.screen.width, b.screen.width, "{what}: width");
    assert_eq!(a.screen.height, b.screen.height, "{what}: height");
    let w = a.screen.width as usize;
    let (mut differ, mut lo, mut hi) = (0, (usize::MAX, usize::MAX), (0, 0));
    for (i, (x, y)) in a
        .screen
        .pixels
        .chunks(4)
        .zip(b.screen.pixels.chunks(4))
        .enumerate()
    {
        if x != y {
            differ += 1;
            let (px, py) = (i % w, i / w);
            lo = (lo.0.min(px), lo.1.min(py));
            hi = (hi.0.max(px), hi.1.max(py));
        }
    }
    assert_eq!(
        differ, 0,
        "{what}: {differ} pixels differ from the still window, from {lo:?} to {hi:?}"
    );
}

/// The moving window and a still one, both composed once with the file open.
fn pair() -> (App, App) {
    // One file for both: its folder shows in the Recent menu.
    let path = file();
    let (mut a, mut b) = (moving_on(&path), still_on(&path));
    a.compose();
    b.compose();
    (a, b)
}

#[test]
fn the_text_changing_under_an_entering_menu_is_drawn() {
    let (mut a, mut b) = pair();
    part_way(&mut a, PopupKind::Main, 3);
    // Whitespace marks: every line of the text under the menu is drawn anew.
    a.docs.whitespace = true;
    a.compose();
    settle(&mut a);
    b.open_popup(PopupKind::Main);
    b.docs.whitespace = true;
    b.compose();
    same(&a, &b, "whitespace shown mid-fade");
}

#[test]
fn a_toast_that_comes_and_goes_during_the_entrance_leaves_nothing() {
    let (mut a, mut b) = pair();
    part_way(&mut a, PopupKind::Main, 2);
    a.docs.set_toast("Saved");
    a.compose();
    tick(&mut a, 16);
    a.compose();
    tick(&mut a, 16);
    a.compose();
    a.docs.clear_toast();
    a.compose();
    settle(&mut a);
    b.open_popup(PopupKind::Main);
    b.compose();
    same(&a, &b, "toast shown and gone mid-fade");
}

#[test]
fn the_pointer_moving_over_an_entering_menu_lights_its_row() {
    let (mut a, mut b) = pair();
    part_way(&mut a, PopupKind::Main, 3);
    let r = a.ui.popup.as_ref().unwrap().menu.rect();
    let at = (r.x + r.w / 2.0, r.y + 60.0);
    a.pointer_moved(at.0, at.1);
    a.compose();
    settle(&mut a);
    b.open_popup(PopupKind::Main);
    b.compose();
    b.pointer_moved(at.0, at.1);
    b.compose();
    assert!(
        a.ui.popup.as_ref().unwrap().menu.hover.is_some(),
        "a row is lit"
    );
    same(&a, &b, "hover moved mid-fade");
}

#[test]
fn a_resize_and_a_new_scale_during_the_entrance() {
    for (size, scale) in [((1100, 700), 1.0), ((1600, 1000), 1.25)] {
        let (mut a, mut b) = pair();
        part_way(&mut a, PopupKind::Main, 3);
        (a.size, a.scale) = (size, scale);
        a.compose();
        settle(&mut a);
        // Opened before the resize, where the window's corner was then.
        b.open_popup(PopupKind::Main);
        b.compose();
        (b.size, b.scale) = (size, scale);
        b.compose();
        same(&a, &b, &format!("{size:?} at {scale} mid-fade"));
    }
}

/// Main with View beside it, opened by keys as the shots do.
fn open_view(a: &mut App) {
    if let Some(p) = a.ui.popup.as_mut() {
        p.menu.hover = p.menu.items.iter().position(|i| i.id == Act::ViewMenu);
    }
    a.open_sub(PopupKind::Main, PopupKind::View, true);
}

#[test]
fn a_submenu_enters_beside_a_menu_at_rest_and_one_still_entering() {
    for frames in [None, Some(3)] {
        let (mut a, mut b) = pair();
        match frames {
            Some(n) => part_way(&mut a, PopupKind::Main, n),
            None => {
                a.open_popup(PopupKind::Main);
                a.compose();
                settle(&mut a);
            }
        }
        open_view(&mut a);
        a.compose();
        let sub = a.motion.drawn.and_then(|m| m.sub).map(|s| s.1);
        assert!(sub.is_some_and(|t| t < 1.0), "the submenu enters: {sub:?}");
        settle(&mut a);
        b.open_popup(PopupKind::Main);
        b.compose();
        open_view(&mut b);
        b.compose();
        same(&a, &b, &format!("submenu after {frames:?} frames"));
    }
}

#[test]
fn a_menu_closed_while_entering_is_gone_at_once() {
    let (mut a, b) = pair();
    part_way(&mut a, PopupKind::Main, 3);
    a.ui.popup = None;
    a.compose();
    assert_eq!(a.motion.due, None, "nothing more to draw");
    same(&a, &b, "closed mid-fade");
}

#[test]
fn recent_replacing_the_main_menu_while_it_enters() {
    let (mut a, mut b) = pair();
    part_way(&mut a, PopupKind::Main, 3);
    a.open_popup(PopupKind::Recent);
    a.compose();
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::Recent));
    assert!(shown(&a) < 1.0, "Recent enters in its turn");
    settle(&mut a);
    b.open_popup(PopupKind::Main);
    b.compose();
    b.open_popup(PopupKind::Recent);
    b.compose();
    same(&a, &b, "Recent over Main mid-fade");
}

#[test]
fn a_menu_above_its_anchor_rises_into_place() {
    let (mut a, mut b) = pair();
    part_way(&mut a, PopupKind::Eol, 3);
    assert!(a.ui.popup.as_ref().unwrap().menu.opened_above());
    settle(&mut a);
    b.open_popup(PopupKind::Eol);
    b.compose();
    same(&a, &b, "the line-ending menu");
}

#[test]
fn the_ai_menus_modify_buttons_enter_with_it() {
    let (mut a, mut b) = pair();
    for w in [&mut a, &mut b] {
        // No prompts file is read: these are the prompts.
        w.ai.prompts_stamp = Some(None);
        w.docs.ai_prompts = vec![mdedit_core::ai::SavedPrompt {
            name: "Tidy".into(),
            instruction: "Tidy it".into(),
            kind: Default::default(),
        }];
    }
    part_way(&mut a, PopupKind::Ai, 3);
    let menu = &a.ui.popup.as_ref().unwrap().menu;
    assert!(menu.items.iter().any(|i| matches!(i.id, Act::AiPrompt(_))));
    settle(&mut a);
    b.open_popup(PopupKind::Ai);
    b.compose();
    same(&a, &b, "the AI menu");
}

/// A toast standing over an entering menu stays over it: the frames that move only the menu
/// must not, since the toast is drawn after the menus.
#[test]
fn a_toast_over_an_entering_menu_stays_on_top() {
    let (mut a, mut b) = pair();
    for w in [&mut a, &mut b] {
        w.docs
            .set_toast("Saved, and a long enough message to lie under the menu");
        w.compose();
        // The text's context menu, opened where the toast stands.
        let text = w.areas.text;
        w.ui.mouse = (text.x + text.w / 2.0 - 40.0, text.bottom() - 30.0);
        w.right_press();
        w.compose();
    }
    assert_eq!(a.ui.popup_kind(), Some(PopupKind::Text));
    let toast = {
        let mut p = Painter::new(&mut a.screen, &mut a.gfx, a.scale);
        let page = a.areas.text;
        Bubble::toast(
            &mut p,
            "Saved, and a long enough message to lie under the menu",
            page,
        )
        .bounds()
    };
    let menu = crate::motion::popup_bounds(a.ui.popup.as_ref().unwrap());
    assert!(
        overlaps(toast, menu),
        "the toast lies under the menu's bounds"
    );
    settle(&mut a);
    same(&a, &b, "a toast over the entering menu");
}
