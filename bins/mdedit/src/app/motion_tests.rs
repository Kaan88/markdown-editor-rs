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

// ---- toasts: they rise in and leave of themselves; a status replaces, a confirmation stacks ----

/// The toasts this frame: text, presence and lift, the bottom one first.
fn toasts(a: &App) -> Vec<(String, f32, f32)> {
    (a.motion.toast_frame.iter())
        .map(|s| (s.text.clone(), s.t, s.lift))
        .collect()
}

/// Composes 16 ms frames for `ms`.
fn frames_for(a: &mut App, ms: u64) {
    for _ in 0..ms / 16 {
        tick(a, 16);
        a.compose();
    }
}

/// Past a toast's time.
const PAST: u64 = crate::commands::strings::TOAST_MS + 100;

/// A toast's lift above the one under it: its height and the gap.
const STEP: f32 = mdshared_ui::overlay::TOAST_H + crate::motion::TOAST_GAP;

#[test]
fn a_toast_rises_in_stays_then_leaves_of_itself_with_no_timer_left() {
    let mut a = moving();
    a.compose();
    a.docs.set_toast("Saved");
    a.compose();
    let first = toasts(&a);
    assert_eq!(first.len(), 1);
    assert!(first[0].1 < 1.0, "it starts on its way in: {first:?}");
    assert!(
        a.motion.due.is_some(),
        "a frame is asked for while it rises"
    );
    settle(&mut a);
    assert_eq!(toasts(&a), [("Saved".to_string(), 1.0, 0.0)]);
    assert!(
        a.toast_until.is_some(),
        "the toast's own time is the one timer left"
    );
    // Its time comes: it fades and sinks where it stands, then it is gone with no timer left.
    tick(&mut a, PAST);
    a.compose();
    tick(&mut a, 16);
    a.compose();
    let leaving = toasts(&a);
    assert!(leaving.len() == 1 && leaving[0].1 < 1.0, "{leaving:?}");
    settle(&mut a);
    assert!(toasts(&a).is_empty());
    assert_eq!((a.motion.due, a.toast_until), (None, None));
    assert!(a.motion.anim.group_deadlines().is_empty());
}

/// The pixels a toast passes over are put back: once it has risen in and once it has gone, the
/// window is the still window's, pixel for pixel.
#[test]
fn a_toast_coming_and_going_leaves_the_still_windows_pixels() {
    let (mut a, mut b) = pair();
    let text = "Saved, and long enough to cross the text";
    a.docs.set_toast(text);
    a.compose();
    frames_for(&mut a, 48);
    settle(&mut a);
    b.docs.set_toast(text);
    b.compose();
    same(&a, &b, "a toast risen in");
    tick(&mut a, PAST);
    a.compose();
    frames_for(&mut a, 64);
    settle(&mut a);
    b.docs.clear_toast();
    b.compose();
    same(&a, &b, "a toast gone");
}

#[test]
fn a_status_said_again_changes_in_place_and_never_flickers() {
    let mut a = moving();
    a.compose();
    a.docs.set_toast("Zoom 110%");
    a.compose();
    frames_for(&mut a, 48);
    let key = a.motion.toast_frame[0].key;
    let mut t = a.motion.toast_frame[0].t;
    assert!(t < 1.0, "still on its way in");
    // Key repeat: a new readout every frame, on the way in and at rest.
    for n in 0..20 {
        a.docs.set_toast(&format!("Zoom {}%", 120 + n * 10));
        tick(&mut a, 16);
        a.compose();
        let now = &a.motion.toast_frame;
        assert_eq!(now.len(), 1, "one toast, never a second");
        assert_eq!(now[0].key, key, "the same toast");
        assert!(now[0].t >= t, "never fading: {} after {t}", now[0].t);
        assert_eq!(now[0].text, format!("Zoom {}%", 120 + n * 10));
        t = now[0].t;
    }
    assert_eq!(t, 1.0);
}

#[test]
fn a_second_confirmation_stacks_under_the_first_which_moves_up() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    tick(&mut a, 16);
    a.compose();
    let rising = toasts(&a);
    assert_eq!(rising.len(), 2);
    assert_eq!(
        rising[0].0, "Image saved as a.png",
        "the new one at the bottom"
    );
    assert!(
        rising[1].2 > 0.0 && rising[1].2 < STEP,
        "the first on its way up: {rising:?}"
    );
    settle(&mut a);
    assert_eq!(
        toasts(&a),
        [
            ("Image saved as a.png".to_string(), 1.0, 0.0),
            ("File path copied".to_string(), 1.0, STEP),
        ]
    );
}

/// With motion on, the new confirmation is out until the ones above have lifted, then enters
/// ("New waits for the lift", D-252).
#[test]
fn a_new_confirmation_enters_only_once_the_one_above_has_lifted() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    let mut entered = 0;
    for _ in 0..30 {
        tick(&mut a, 16);
        a.compose();
        let f = toasts(&a);
        assert_eq!(f.len(), 2, "{f:?}");
        if f[1].2 < STEP {
            assert_eq!(
                f[0].1, 0.0,
                "in while the one above is still on its way up: {f:?}"
            );
        } else if f[0].1 > 0.0 {
            entered += 1;
        }
    }
    assert!(entered > 0, "and then it enters");
    settle(&mut a);
    assert_eq!(
        toasts(&a),
        [
            ("Image saved as a.png".to_string(), 1.0, 0.0),
            ("File path copied".to_string(), 1.0, STEP),
        ]
    );
}

/// The newest toast is drawn last, over the shadow the one above it throws: the top of its
/// body is the pixels it has when it stands alone.
#[test]
fn the_newest_toast_covers_the_shadow_of_the_one_above() {
    let text = "Image saved as a.png";
    let (mut a, mut b) = pair();
    // A toast pushed onto another waits for its lift, so the moving window is stepped to rest.
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast(text);
    a.compose();
    settle(&mut a);
    assert_eq!(a.motion.toast_frame.len(), 2, "two at rest");
    assert_eq!(a.motion.toast_frame[0].lift, 0.0);
    b.docs.confirm_toast(text);
    b.compose();
    assert_eq!(b.motion.toast_frame.len(), 1, "one alone");
    let rect = {
        let mut p = Painter::new(&mut b.screen, &mut b.gfx, b.scale);
        Bubble::toast(&mut p, text, b.areas.text).rect
    };
    assert_eq!(
        (a.size, a.scale),
        (b.size, b.scale),
        "the two windows are alike"
    );
    // The top of the newest toast's body, clear of its rounded corners: where the shadow of
    // the toast 44 px above it falls (it reaches 8 px past that one's bottom).
    let s = a.scale;
    let w = a.screen.width as usize;
    let (x0, x1) = (
        ((rect.x + 20.0) * s) as usize,
        ((rect.right() - 20.0) * s) as usize,
    );
    let (y0, y1) = (
        ((rect.y + 1.0) * s) as usize,
        ((rect.y + 12.0) * s) as usize,
    );
    let mut differ = 0;
    for y in y0..y1 {
        for x in x0..x1 {
            let i = (y * w + x) * 4;
            if a.screen.pixels[i..i + 4] != b.screen.pixels[i..i + 4] {
                differ += 1;
            }
        }
    }
    assert_eq!(
        differ, 0,
        "the shadow of the toast above shows on the newest one"
    );
}

/// Under `Policy::Never` — a shot, a test, the setting off, reduced motion — a stacked toast is
/// there at once: the wait for the lift is a delay, and a delay must not outlive motion switched
/// off, or the toast would arrive 167 ms late with nothing moving. Shared's `Stack` waits anyway
/// until shared PR 24 lands, so this is left out until the pin moves.
#[test]
#[ignore = "needs shared PR 24"]
fn with_no_motion_two_confirmations_are_both_there_at_once() {
    for how in ["still", "setting", "reduced"] {
        let mut a = match how {
            "still" => still(),
            _ => moving(),
        };
        match how {
            "reduced" => a.motion.set_reduced(true),
            "setting" => a.docs.prefs.animations = false,
            _ => {}
        }
        a.compose();
        a.docs.confirm_toast("File path copied");
        a.compose();
        a.docs.confirm_toast("Image saved as a.png");
        a.compose();
        let now = toasts(&a);
        assert_eq!(now.len(), 2, "{how}");
        assert!(now.iter().all(|t| t.1 == 1.0), "{how}: {now:?}");
        assert_eq!(a.motion.due, None, "{how}: nothing asked for");
    }
}

#[test]
fn a_status_takes_the_place_of_the_confirmations_showing() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    a.docs.confirm_toast("Path copied");
    a.compose();
    settle(&mut a);
    let newest = a.motion.toast_frame[0].key;
    a.docs.set_toast("No more occurrences");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!(now.len(), 1, "the others go at once");
    assert_eq!(
        (now[0].key, now[0].t),
        (newest, 1.0),
        "in the newest one's place"
    );
    assert_eq!(now[0].text, "No more occurrences");
}

/// A status also takes away the toasts still playing their exit: it stands alone, with no echo
/// of what it replaced left fading beside it.
#[test]
fn a_status_takes_away_the_toasts_still_leaving_too() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    a.docs.confirm_toast("Path copied");
    a.compose();
    settle(&mut a);
    tick(&mut a, PAST);
    a.compose();
    tick(&mut a, 32);
    a.compose();
    let leaving = toasts(&a);
    assert!(
        leaving.len() == 2 && leaving.iter().all(|t| t.1 < 1.0),
        "both on their way out: {leaving:?}"
    );
    a.docs.set_toast("No more occurrences");
    a.compose();
    let one = toasts(&a);
    assert_eq!(one.len(), 1, "the leaving ones go at once too: {one:?}");
    assert_eq!(one[0].0, "No more occurrences");
    settle(&mut a);
    assert_eq!(
        toasts(&a),
        [("No more occurrences".to_string(), 1.0, 0.0)],
        "and none comes back"
    );
}

#[test]
fn a_progress_turning_into_its_result_leaves_one_toast() {
    let mut a = moving();
    a.compose();
    a.docs.set_toast("An AI rewrite is running. Esc stops it.");
    a.compose();
    frames_for(&mut a, 32);
    a.docs.set_toast("Stopped the AI rewrite.");
    a.compose();
    settle(&mut a);
    assert_eq!(
        toasts(&a),
        [("Stopped the AI rewrite.".to_string(), 1.0, 0.0)]
    );
}

/// A confirmation said again never stands twice: said while it is leaving, it comes back at the
/// bottom, and the other leaves as it was.
#[test]
fn a_confirmation_said_again_never_stands_twice_even_while_it_leaves() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    a.docs.confirm_toast("Path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    assert_eq!(toasts(&a).len(), 2, "not a third");
    // Leaving: said again, it comes back at the bottom instead of a second coming in.
    tick(&mut a, PAST);
    a.compose();
    tick(&mut a, 32);
    a.compose();
    let leaving = toasts(&a);
    assert!(
        leaving.iter().all(|t| t.1 < 1.0),
        "both leaving: {leaving:?}"
    );
    a.docs.confirm_toast("Path copied");
    a.compose();
    assert_eq!(toasts(&a).len(), 2, "back again, not a second one");
    settle(&mut a);
    assert_eq!(toasts(&a), [("Path copied".to_string(), 1.0, 0.0)]);
}

/// A confirmation said again that is not the newest moves to the bottom, under the ones above:
/// the one the user just triggered is the one in sight, even in a short window.
#[test]
fn a_confirmation_said_again_moves_to_the_bottom() {
    let mut a = moving();
    a.compose();
    for text in ["File path copied", "Path copied", "Image saved as a.png"] {
        a.docs.confirm_toast(text);
        a.compose();
        tick(&mut a, 16);
        a.compose();
    }
    settle(&mut a);
    let bottom_first =
        |a: &App| -> Vec<String> { toasts(a).into_iter().map(|t| t.0).collect::<Vec<_>>() };
    assert_eq!(
        bottom_first(&a),
        ["Image saved as a.png", "Path copied", "File path copied"]
    );
    // The oldest, still standing at the top, is triggered again.
    a.docs.confirm_toast("File path copied");
    a.compose();
    assert_eq!(toasts(&a).len(), 3, "not a fourth: the same one moved");
    assert_eq!(
        toasts(&a)[0].0,
        "File path copied",
        "the one just triggered is at the bottom"
    );
    settle(&mut a);
    let rest = toasts(&a);
    assert_eq!(
        rest.iter().map(|t| t.0.as_str()).collect::<Vec<_>>(),
        ["File path copied", "Image saved as a.png", "Path copied"]
    );
    assert_eq!(
        rest.iter().map(|t| t.2).collect::<Vec<_>>(),
        [0.0, STEP, STEP * 2.0],
        "and the rest stand above it in order"
    );
}

/// A confirmation said again that is already the newest keeps its own toast: the same key, its
/// presence at 1, and nothing moving — a copy pressed twice must not blink, and under a repeat
/// it must not stay invisible.
#[test]
fn a_repeat_of_the_newest_confirmation_keeps_it_at_rest() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    let key = a.motion.toast_frame[0].key;
    for _ in 0..3 {
        a.docs.confirm_toast("File path copied");
        a.compose();
        let now = &a.motion.toast_frame;
        assert_eq!(now.len(), 1);
        assert_eq!(
            (now[0].key, now[0].t),
            (key, 1.0),
            "the same toast, at rest, never rising again"
        );
        assert_eq!(a.motion.due, None, "and nothing asked for");
        tick(&mut a, 16);
        a.compose();
    }
    // The same with one above it: the newest keeps its place under it.
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    settle(&mut a);
    assert_eq!(toasts(&a).len(), 2);
    let (top, bottom) = (a.motion.toast_frame[1].key, a.motion.toast_frame[0].key);
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!(now.len(), 2);
    assert_eq!((now[0].key, now[0].t), (bottom, 1.0), "the newest, at rest");
    assert_eq!((now[1].key, now[1].t), (top, 1.0), "the one above, unmoved");
    assert_eq!(a.motion.due, None, "no lift is asked for");
}

/// A status taking the place of a confirmation that was still waiting for the lift stands there
/// at once: the animator keeps the wait the stack gave up, and nothing would show for it.
#[test]
fn a_status_takes_the_place_of_a_waiting_confirmation_and_shows_at_once() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    tick(&mut a, 16);
    a.compose();
    let waiting = toasts(&a);
    assert_eq!(waiting.len(), 2);
    assert_eq!(waiting[0].0, "Image saved as a.png");
    assert_eq!(waiting[0].1, 0.0, "the confirmation waits for the lift");
    let key = a.motion.toast_frame[0].key;
    a.docs.set_toast("Saved");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!(now.len(), 1, "the one above goes at once");
    assert_eq!(
        (now[0].key, now[0].t, now[0].text.as_str()),
        (key, 1.0, "Saved"),
        "the status is there on the next frame, in its place"
    );
    assert_eq!(a.motion.due, None, "and nothing moves");
}

/// A status taking over a toast that is still rising keeps it rising: only a toast waiting for
/// a lift is put there at once, and a toast that came onto an empty stack is not waiting.
#[test]
fn a_status_taking_over_a_rising_toast_lets_it_keep_rising() {
    // On its first frame: nothing was there to lift for, and the last frame still said 0.
    let mut a = moving();
    a.compose();
    a.docs.set_toast("An AI rewrite is running. Esc stops it.");
    a.compose();
    assert_eq!(toasts(&a)[0].1, 0.0, "it starts on its way in");
    tick(&mut a, 16);
    let key = a.motion.toast_frame[0].key;
    a.docs.set_toast("Stopped the AI rewrite.");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!(now.len(), 1);
    assert_eq!(now[0].key, key, "the same toast, its text changed");
    assert!(
        now[0].t > 0.0 && now[0].t < 1.0,
        "still rising, not standing there at once: {}",
        now[0].t
    );
    settle(&mut a);
    assert_eq!(
        toasts(&a),
        [("Stopped the AI rewrite.".to_string(), 1.0, 0.0)]
    );

    // Part way in: it goes on from where it is.
    let mut a = moving();
    a.compose();
    a.docs.set_toast("A step");
    a.compose();
    tick(&mut a, 32);
    a.compose();
    let key = a.motion.toast_frame[0].key;
    let part = a.motion.toast_frame[0].t;
    assert!(part > 0.0 && part < 1.0, "part way in: {part}");
    a.docs.set_toast("Another step");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!(now[0].key, key);
    assert_eq!(now[0].text, "Another step");
    assert!(now[0].t < 1.0, "not stood there at once: {}", now[0].t);
}

/// A repeat of a toast that is still waiting for its lift does not take that wait away from the
/// animator: the toast goes on waiting, so a status taking it over still stands there at once
/// instead of sitting at nothing until the rest of the delay runs out.
#[test]
fn a_repeat_of_a_waiting_toast_leaves_the_status_standing_at_once() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    assert_eq!(toasts(&a)[0].1, 0.0, "the second waits for the lift");
    let key = a.motion.toast_frame[0].key;
    tick(&mut a, 16);
    // Said again while it waits: the same toast, still waiting.
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    assert_eq!(a.motion.toast_frame[0].key, key);
    assert_eq!(a.motion.toast_frame[0].t, 0.0, "still waiting");
    tick(&mut a, 16);
    a.docs.set_toast("Saved");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!(now.len(), 1);
    assert_eq!(
        (now[0].key, now[0].t, now[0].text.as_str()),
        (key, 1.0, "Saved"),
        "the status stands there at once, in the toast that was waiting"
    );
}

/// A toast past its wait is coming in, and a status taking it over lets it go on from where it
/// is: only one still at nothing is stood up.
#[test]
fn a_status_lets_a_toast_past_its_wait_go_on_rising() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    let key = a.motion.toast_frame[0].key;
    // Past the wait (the lift is 167 ms) and on its way in.
    frames_for(&mut a, 200);
    let part = a.motion.toast_frame[0].t;
    assert!(part > 0.0 && part < 1.0, "coming in: {part}");
    a.docs.set_toast("Saved");
    a.compose();
    let now = &a.motion.toast_frame;
    assert_eq!((now[0].key, now[0].text.as_str()), (key, "Saved"));
    assert!(
        now[0].t < 1.0,
        "it goes on rising, not stood up at once: {}",
        now[0].t
    );
}

#[test]
fn never_more_than_three_toasts_those_leaving_included() {
    let mut a = moving();
    a.compose();
    for text in [
        "one copied",
        "two copied",
        "three copied",
        "four copied",
        "five",
    ] {
        a.docs.confirm_toast(text);
        a.compose();
        tick(&mut a, 16);
        a.compose();
        assert!(a.motion.toast_frame.len() <= 3, "{:?}", toasts(&a));
    }
    settle(&mut a);
    let texts: Vec<String> = toasts(&a).into_iter().map(|t| t.0).collect();
    assert_eq!(texts, ["five", "four copied", "three copied"]);
}

#[test]
fn reduced_motion_the_setting_off_and_a_still_window_show_a_toast_at_once() {
    for how in ["reduced", "setting", "still"] {
        let mut a = match how {
            "still" => still(),
            _ => moving(),
        };
        match how {
            "reduced" => a.motion.set_reduced(true),
            "setting" => a.docs.prefs.animations = false,
            _ => {}
        }
        a.compose();
        a.docs.set_toast("Saved");
        a.compose();
        assert_eq!(toasts(&a), [("Saved".to_string(), 1.0, 0.0)], "{how}");
        assert_eq!(a.motion.due, None, "{how}: no frame asked for");
        // Past its time (a still window has no clock moved by hand: the frame's time is given).
        let past = Instant::now() + Duration::from_millis(PAST);
        tick(&mut a, PAST);
        a.motion.anim.begin_frame(past);
        a.motion.sync_toasts(a.docs.toast_line(), past);
        a.motion.anim.end_frame();
        assert!(toasts(&a).is_empty(), "{how}: gone at once");
    }
}

/// A status cleared takes every toast away at once.
#[test]
fn a_cleared_status_takes_the_toasts_away_at_once() {
    let mut a = moving();
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    a.docs.confirm_toast("Path copied");
    a.compose();
    // Both stand before the clear: the second has waited for the first to lift, so there is
    // something for the clear to take away.
    settle(&mut a);
    assert_eq!(toasts(&a).len(), 2);
    a.docs.clear_toast();
    a.compose();
    assert!(toasts(&a).is_empty());
    assert!(a.motion.toasts.is_empty());
}

/// A message set while a toast stands wakes the window for a frame, though the standing
/// toast's own time is still far off.
#[test]
fn a_message_set_while_a_toast_stands_asks_for_a_frame() {
    let mut a = moving();
    a.compose();
    a.docs.set_toast("Saved");
    a.compose();
    settle(&mut a);
    assert!(a.toast_until.is_some());
    a.docs.confirm_toast("File path copied");
    a.wants_frame = false;
    a.timers(Instant::now());
    assert!(a.wants_frame, "the new message is drawn");
    a.compose();
    a.wants_frame = false;
    a.timers(Instant::now());
    assert!(!a.wants_frame, "and asks no more once taken in");
}

/// A short window: a toast that would reach above the page is left out, but never the bottom
/// one. (With no other overlay, the overlays' bounds are the bubbles drawn.)
#[test]
fn toasts_that_would_reach_above_the_page_are_left_out() {
    for (height, drawn) in [(800, 3), (190, 1), (140, 1)] {
        let mut a = still();
        a.size = (700, height);
        a.compose();
        if height < 190 {
            // A page shorter than a toast and its 28 px of air under it: no lifted toast
            // could be drawn, and the bottom one must be.
            assert!(
                a.areas.text.h < 72.0,
                "{height} px high: the page is {} tall",
                a.areas.text.h
            );
        }
        for text in ["one copied", "two copied", "three copied"] {
            a.docs.confirm_toast(text);
            a.compose();
        }
        assert_eq!(a.motion.toast_frame.len(), 3);
        assert_eq!(a.overlay_bounds.len(), drawn, "{height} px high");
        if height < 190 {
            assert_eq!(
                (
                    a.motion.toast_frame[0].text.as_str(),
                    a.motion.toast_frame[0].lift
                ),
                ("three copied", 0.0),
                "the bottom one, at rest at the bottom"
            );
        }
    }
}

/// The one toast a window shorter than a toast still shows is there at once, not waiting for a
/// lift that nothing moves. Shared's `Stack` keeps the wait with motion off, so this waits for
/// shared PR 24.
#[test]
#[ignore = "needs shared PR 24"]
fn the_one_toast_a_very_short_window_shows_is_there_at_once() {
    let mut a = still();
    a.size = (700, 140);
    a.compose();
    assert!(a.areas.text.h < 72.0, "the page is {} tall", a.areas.text.h);
    for text in ["one copied", "two copied", "three copied"] {
        a.docs.confirm_toast(text);
        a.compose();
    }
    assert_eq!(a.overlay_bounds.len(), 1, "only the bottom one is drawn");
    assert_eq!(
        (
            a.motion.toast_frame[0].text.as_str(),
            a.motion.toast_frame[0].t
        ),
        ("three copied", 1.0),
        "the bottom one is there, not waiting"
    );
}

/// A lifted toast stands on a whole device pixel at any scale: the arithmetic, and a real
/// window at 1.25 with a second confirmation arriving and the one above on its way up.
#[test]
fn a_lifting_toast_stands_on_a_whole_device_pixel() {
    for scale in [1.0_f32, 1.25, 1.5] {
        for lift in [0.0_f32, 3.3, 21.7, 44.0] {
            let y = crate::motion::toast_y(500.0, lift, scale);
            let dev = y * scale;
            assert!((dev - dev.round()).abs() < 1e-3, "{scale} {lift}: {dev}");
        }
    }
    // A toast's bounds start 18 px above its place (16 for its shadow, 2 more for the rise).
    // The toast above the newest is moved up by a whole number of device pixels: what it is
    // drawn at differs from its place at rest by that much, so its text is on the grid it has
    // at rest. (The toasts are drawn top first, the newest last.)
    let mut a = moving();
    a.scale = 1.25;
    a.size = (900, 700);
    a.compose();
    a.docs.confirm_toast("File path copied");
    a.compose();
    settle(&mut a);
    assert_eq!(a.overlay_bounds.len(), 1, "one toast at rest");
    let rest = a.overlay_bounds[0].y;
    a.docs.confirm_toast("Image saved as a.png");
    a.compose();
    let mut lifted = false;
    for _ in 0..6 {
        tick(&mut a, 16);
        a.compose();
        let f = toasts(&a);
        assert_eq!(f.len(), 2, "{f:?}");
        if f[1].2 > 0.0 && f[1].2 < STEP {
            lifted = true;
            assert_eq!(a.overlay_bounds.len(), 2, "the toast above and the new one");
            let dev = (rest - a.overlay_bounds[0].y) * a.scale;
            assert!(
                (dev - dev.round()).abs() < 1e-3,
                "moved by {dev} device px, not a whole number: {:?}",
                a.overlay_bounds
            );
        }
    }
    assert!(lifted, "the toast above never moved");
}

/// A message whose time is already past when the frame takes it in shows nothing: a window
/// resumed from a sleep does not show a stale readout.
#[test]
fn a_message_already_past_its_time_is_not_shown() {
    let mut a = moving();
    a.compose();
    let now = a.motion.now();
    let line = crate::motion::ToastLine {
        serial: 1,
        text: "Saved",
        until: Instant::now() - Duration::from_secs(5),
        stack: false,
    };
    a.motion.anim.begin_frame(now);
    a.motion.sync_toasts(Some(line), now);
    a.motion.anim.end_frame();
    assert!(toasts(&a).is_empty(), "{:?}", toasts(&a));
    assert!(a.motion.toasts.is_empty());
    // The same for a confirmation, which is the case where it would be pushed.
    let line = crate::motion::ToastLine {
        serial: 2,
        stack: true,
        ..line
    };
    a.motion.anim.begin_frame(now);
    a.motion.sync_toasts(Some(line), now);
    a.motion.anim.end_frame();
    assert!(a.motion.toasts.is_empty(), "a confirmation too");
}
