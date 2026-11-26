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
    let before = a.screen.pixels.clone();
    let popup = a.ui.popup.as_mut().unwrap();
    popup.root_mut().hover = Some(0);
    a.compose();
    assert!(
        a.screen.pixels != before,
        "the row lights on the first frame"
    );
    assert_eq!(a.motion.due, None, "no hover fade either");
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
    let r = a.ui.popup.as_ref().unwrap().root().rect();
    let at = (r.x + r.w / 2.0, r.y + 60.0);
    a.pointer_moved(at.0, at.1);
    a.compose();
    settle(&mut a);
    b.open_popup(PopupKind::Main);
    b.compose();
    b.pointer_moved(at.0, at.1);
    b.compose();
    assert!(
        a.ui.popup.as_ref().unwrap().root().hover.is_some(),
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
        p.root_mut().hover = p.root().items.iter().position(|i| i.id == Act::ViewMenu);
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
    assert!(a.ui.popup.as_ref().unwrap().root().opened_above());
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
    let menu = a.ui.popup.as_ref().unwrap().root();
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
/// off, or the toast would arrive 167 ms late with nothing moving. Shared's `Stack` keeps the
/// wait only while something animates (its PR 24), which the pin to `09b01c1` carries.
#[test]
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
/// lift that nothing moves.
#[test]
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

// ---- the tooltip -------------------------------------------------------------------------------

/// The control a tip is taken from: the New file button beside the tabs.
fn a_tip_hit() -> crate::ui::Hit {
    crate::ui::Hit::NewTab
}

/// How long the pointer must rest on a control before its tip shows.
const TIP_WAIT: Duration = mdshared_ui::anim::document::TIP_DELAY;

/// With motion off the tip is the whole of itself on its first frame: `t` is 1.0, so the bubble
/// is drawn by `Bubble::draw`, the pixels of before, and nothing asks for a frame after it.
#[test]
fn with_motion_off_the_tip_is_there_on_its_first_frame() {
    let mut a = still();
    // No caret blinking: the only timer left would be the tip's.
    a.focused = false;
    a.compose();
    let t = a.motion.now();
    a.update_tip(Some(a_tip_hit()), (10.0, 10.0), t);
    assert!(a.ui.tip.waiting().is_some(), "it waits for the rest");
    a.timers(t + TIP_WAIT);
    a.compose();
    assert!(a.ui.tip.shown().is_some());
    assert_eq!(a.motion.tip_t, 1.0, "the whole tip, at once");
    assert_eq!(a.motion.due, None, "nothing moves");
    a.wants_frame = false;
    assert_eq!(a.timers(t + TIP_WAIT), None, "and no timer");
    assert!(!a.wants_frame);
}

/// With motion on the tip fades in from nothing, and once it is at rest no timer is left.
#[test]
fn a_tip_fades_in_and_settles_with_no_timer_left() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let t = a.motion.now();
    a.update_tip(Some(a_tip_hit()), (10.0, 10.0), t);
    a.timers(t + TIP_WAIT);
    a.compose();
    let start = a.motion.tip_t;
    assert!(start < 1.0, "it starts to fade: {start}");
    tick(&mut a, 16);
    a.compose();
    let early = a.motion.tip_t;
    assert!(
        early > start && early < 1.0,
        "on its way in: {start} -> {early}"
    );
    for _ in 0..40 {
        if a.motion.due.is_none() {
            break;
        }
        tick(&mut a, 16);
        a.compose();
    }
    assert_eq!(a.motion.tip_t, 1.0, "at rest");
    assert_eq!(a.motion.due, None, "no frame asked for");
    a.wants_frame = false;
    assert_eq!(a.timers(a.motion.now()), None, "and none once settled");
    assert!(!a.wants_frame);
}

/// The delay counts from the pointer coming to rest, not from entering the control: moving on
/// while the tip waits starts the wait again.
#[test]
fn a_tip_waits_from_the_pointer_coming_to_rest() {
    let mut a = still();
    a.compose();
    // The pointer itself, on the New file button: it waits from where it came to rest.
    let b = a.hit_rect(a_tip_hit()).expect("the button");
    let (cx, cy) = (b.cx(), b.cy());
    a.pointer_moved(cx, cy);
    let first = a.ui.tip.deadline().expect("a tip waits");
    assert!(
        first >= Instant::now() + TIP_WAIT - Duration::from_millis(50),
        "the whole delay from entering"
    );
    std::thread::sleep(Duration::from_millis(30));
    a.pointer_moved(cx + 2.0, cy);
    let again = a.ui.tip.deadline().expect("a tip still waits");
    assert!(
        again >= first + Duration::from_millis(20),
        "the pointer moved on the same control: the wait starts again ({first:?} -> {again:?})"
    );
    let t = a.motion.now();
    let hit = a_tip_hit();
    a.update_tip(Some(hit), (10.0, 10.0), t);
    assert_eq!(a.ui.tip.deadline(), Some(t + TIP_WAIT), "from the rest");
    // Still on the same control, the pointer moves: the wait starts again from there.
    let moved = t + Duration::from_millis(300);
    a.update_tip(Some(hit), (12.0, 10.0), moved);
    assert_eq!(
        a.ui.tip.deadline(),
        Some(moved + TIP_WAIT),
        "counted from the last move, not the first"
    );
    a.timers(moved + TIP_WAIT - Duration::from_millis(1));
    assert!(a.ui.tip.shown().is_none(), "not before its time");
    a.timers(moved + TIP_WAIT);
    assert!(a.ui.tip.shown().is_some(), "when the pointer has rested");
}

/// A press takes the tip away, and the control under the pointer shows none again until the
/// pointer has left it.
#[test]
fn a_press_hides_the_tip_until_the_pointer_leaves() {
    let mut a = still();
    a.compose();
    let t = a.motion.now();
    let hit = a_tip_hit();
    a.ui.hover = Some(hit);
    a.update_tip(Some(hit), (10.0, 10.0), t);
    a.timers(t + TIP_WAIT);
    assert!(a.ui.tip.shown().is_some());
    a.left_press();
    assert!(a.ui.tip.shown().is_none(), "the press takes it away");
    // Back on the same control: still nothing.
    a.update_tip(Some(hit), (11.0, 10.0), t + TIP_WAIT * 2);
    a.timers(t + TIP_WAIT * 3);
    assert!(a.ui.tip.shown().is_none(), "until the pointer leaves it");
    // Another control: its own tip, as ever.
    a.ui.hover = Some(hit);
    a.update_tip(
        Some(crate::ui::Hit::WinClose),
        (11.0, 10.0),
        t + TIP_WAIT * 4,
    );
    a.timers(t + TIP_WAIT * 5);
    assert!(a.ui.tip.shown().is_some(), "a neighbour's shows");
}

/// The pixels a frame changed that no damaged rectangle holds (device px, as the screen is).
fn changed_outside(a: &App, before: &[u8], damage: &[(i32, i32, i32, i32)]) -> usize {
    let (w, h) = (a.screen.width as usize, a.screen.height as usize);
    let mut outside = 0;
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) * 4;
            if a.screen.pixels[i..i + 4] == before[i..i + 4] {
                continue;
            }
            let (px, py) = (x as i32, y as i32);
            let inside = damage
                .iter()
                .any(|d| px >= d.0 && px < d.2 && py >= d.1 && py < d.3);
            if !inside {
                outside += 1;
            }
        }
    }
    outside
}

/// A tip taken away half way into its fade goes at once, and the pixels under it come back: the
/// frame that takes it away damages what it covered.
#[test]
fn a_tip_taken_away_mid_fade_goes_at_once_and_the_pixels_come_back() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let before = a.screen.pixels.clone();
    let t = a.motion.now();
    a.update_tip(Some(a_tip_hit()), (10.0, 10.0), t);
    a.timers(t + TIP_WAIT);
    a.compose();
    tick(&mut a, 30);
    a.compose();
    let mid = a.motion.tip_t;
    assert!(mid > 0.0 && mid < 1.0, "part way in: {mid}");
    assert!(a.screen.pixels != before, "the tip is drawn");
    a.ui.tip.hide();
    let (damage, _) = a.compose();
    assert_eq!(a.motion.tip_t, 0.0, "it goes at once, with no fade out");
    assert_eq!(a.motion.due, None, "and nothing is left moving");
    assert!(
        a.screen.pixels == before,
        "the pixels under the tip come back"
    );
    assert_eq!(
        changed_outside(&a, &before, &damage),
        0,
        "nothing changed outside the damage"
    );
}

/// A neighbour's tip shows at once while one shows, at presence 1: it does not fade in again.
#[test]
fn a_neighbours_tip_shows_at_once_without_a_second_fade() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let t = a.motion.now();
    a.update_tip(Some(a_tip_hit()), (10.0, 10.0), t);
    a.timers(t + TIP_WAIT);
    a.compose();
    settle(&mut a);
    assert_eq!(a.motion.tip_t, 1.0, "the first at rest");
    let next = crate::ui::Hit::WinClose;
    a.update_tip(Some(next), (400.0, 10.0), a.motion.now());
    a.compose();
    assert_eq!(
        a.ui.tip.shown().map(|o| o.key),
        Some(next),
        "the neighbour's tip"
    );
    assert_eq!(a.motion.tip_t, 1.0, "at once, no second fade");
    assert_eq!(a.motion.due, None, "and nothing asked for");
}

/// Every frame of the fade damages what the tip covers: no pixel it changed is left undamaged.
#[test]
fn every_frame_of_the_fade_damages_what_the_tip_covers() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let t = a.motion.now();
    a.update_tip(Some(a_tip_hit()), (10.0, 10.0), t);
    a.timers(t + TIP_WAIT);
    a.compose();
    for _ in 0..3 {
        tick(&mut a, 16);
        let before = a.screen.pixels.clone();
        let (damage, _) = a.compose();
        assert!(
            !damage.is_empty(),
            "the frame of the fade redraws something"
        );
        assert_eq!(
            changed_outside(&a, &before, &damage),
            0,
            "every pixel the fade changed is inside the damage"
        );
    }
    assert!(a.motion.tip_t > 0.0 && a.motion.tip_t < 1.0, "still fading");
}

/// A measurement, not a test: what a pointer move costs in `update_tip` — the common case (a
/// move within one control: one `String` from `chrome::tooltip` and `Tooltip::hover`'s two
/// clones of the shown text, three allocations) and the free one (the same control at the same
/// place: skipped). Each batch is timed as a whole and divided, since `Instant` on Windows
/// advances in steps of about 100 ns and one call is shorter than a step. Run it on its own,
/// several times: `cargo test --release -p mdedit -- --ignored --nocapture tip_move_cost`.
#[test]
#[ignore = "a measurement, not a test"]
fn tip_move_cost() {
    let mut a = still();
    a.focused = false;
    a.compose();
    let hit = a_tip_hit();
    // A tip showing: the warm path, with its two clones.
    let t = a.motion.now();
    a.update_tip(Some(hit), (10.0, 10.0), t);
    a.timers(t + TIP_WAIT);
    assert!(a.ui.tip.shown().is_some());
    const CALLS: u32 = 200_000;
    // The whole batch, then divided: one call is under the clock's step.
    let start = Instant::now();
    for i in 0..CALLS {
        let at = (10.0 + (i % 7) as f32 * 0.5, 10.0);
        a.update_tip(Some(hit), at, Instant::now());
    }
    let moving = start.elapsed().as_nanos() as f64 / f64::from(CALLS);
    let at = (10.0, 10.0);
    a.update_tip(Some(hit), at, Instant::now());
    let start = Instant::now();
    for _ in 0..CALLS {
        a.update_tip(Some(hit), at, Instant::now());
    }
    let same = start.elapsed().as_nanos() as f64 / f64::from(CALLS);
    eprintln!("tip_move_cost: moving within a control: {moving:.0} ns a move");
    eprintln!("tip_move_cost: same place, skipped:     {same:.0} ns a move");
}

// ---- the tree's disclosure (M-D) ---------------------------------------------------------------

/// A folder with a subfolder that holds a file, opened as the Files tree's root.
fn tree_app(tag: &str) -> (App, std::path::PathBuf) {
    let dir = scratch(tag);
    std::fs::create_dir_all(dir.join("sub")).expect("sub");
    std::fs::write(
        dir.join("sub").join("a.txt"),
        "one
",
    )
    .expect("a");
    std::fs::write(
        dir.join("b.txt"),
        "two
",
    )
    .expect("b");
    let mut a = moving();
    a.focused = false;
    a.command(crate::commands::CommandId::ToggleFiles);
    a.ui.tree.set_root(Some(dir.clone()));
    a.ui.tree.refresh();
    (a, dir)
}

/// The index of the row whose name is `name`.
fn tree_row(a: &App, name: &str) -> usize {
    a.ui.tree
        .rows()
        .iter()
        .position(|r| r.path.file_name().is_some_and(|n| n == name))
        .expect("the row")
}

/// The name the disclosure of the row at `i` turns under.
fn disclosure_id(a: &App, i: usize) -> mdshared_ui::anim::Id {
    mdshared_ui::anim::Id::new(("files", &a.ui.tree.rows()[i].path)).with("disclosure")
}

/// With motion off the disclosure is the plain chevron, pixel for pixel, at both ends.
#[test]
fn with_motion_off_the_disclosure_is_the_plain_chevron() {
    use mdshared_ui::glyph;
    let (mut a, dir) = tree_app("motion-off-tree");
    a.docs.prefs.animations = false;
    a.compose();
    let i = tree_row(&a, "sub");
    for (open, icon) in [(true, glyph::DOWN), (false, glyph::RIGHT)] {
        a.ui.tree.toggle(i);
        a.compose();
        let row = a.hit_rect(Hit::FilesRow(i)).expect("folder row");
        let pane = a.areas.files.unwrap();
        let depth = a.ui.tree.rows()[i].depth as f32;
        let x = pane.x + 14.0 + (depth + 1.0) * 14.0 - 8.0;
        let y = row.cy();
        let crop = Rect::new(x - 6.0, y - 6.0, 12.0, 12.0);
        let mut plain = mdshared_text::Bitmap::new(a.screen.width, a.screen.height);
        {
            let mut p = Painter::new(&mut plain, &mut a.gfx, a.scale);
            p.fill(crop, a.pal.pane);
            p.icon(icon, 8.0, x, y, a.pal.faint);
        }
        let (x0, y0, x1, y1) = Painter::new(&mut plain, &mut a.gfx, a.scale).dev_rect(crop);
        let stride = a.screen.width as usize * 4;
        for y in y0 as usize..y1 as usize {
            let x = y * stride + x0 as usize * 4..y * stride + x1 as usize * 4;
            assert_eq!(
                &a.screen.pixels[x.clone()],
                &plain.pixels[x],
                "open: {open}"
            );
        }
        assert_eq!(a.motion.due, None, "nothing moves");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A folder opens: its chevron turns from the right chevron to the down one, frame by frame,
/// and once it has arrived no timer is left.
#[test]
fn a_folder_opening_turns_its_chevron_and_then_rests() {
    let (mut a, dir) = tree_app("motion-tree");
    a.compose();
    let i = tree_row(&a, "sub");
    let id = disclosure_id(&a, i);
    assert_eq!(a.motion.anim.get::<f32>(id).unwrap_or(0.0), 0.0, "closed");
    a.ui.tree.toggle(i);
    a.compose();
    let start = a.motion.anim.get::<f32>(id).expect("asked for");
    assert!(start < 1.0, "it starts to turn: {start}");
    tick(&mut a, 16);
    a.compose();
    let early = a.motion.anim.get::<f32>(id).expect("asked for");
    assert!(
        early > start && early < 1.0,
        "on its way from {start} to {early}"
    );
    settle(&mut a);
    assert_eq!(a.motion.anim.get::<f32>(id), Some(1.0), "open");
    a.wants_frame = false;
    assert_eq!(a.timers(a.motion.now()), None, "no timer once settled");
    assert!(!a.wants_frame);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn collapsing_mid_turn_reverses_the_same_chevron() {
    let (mut a, dir) = tree_app("motion-tree-reverse");
    a.compose();
    let i = tree_row(&a, "sub");
    let id = disclosure_id(&a, i);
    a.ui.tree.toggle(i);
    a.compose();
    tick(&mut a, 40);
    a.compose();
    let mid = a.motion.anim.get::<f32>(id).unwrap();
    assert!(0.0 < mid && mid < 1.0, "opening: {mid}");

    a.ui.tree.toggle(i);
    a.compose();
    assert_eq!(a.motion.anim.get::<f32>(id), Some(mid), "no jump");
    tick(&mut a, 32);
    a.compose();
    let reversed = a.motion.anim.get::<f32>(id).unwrap();
    assert!(0.0 < reversed && reversed < mid, "closing: {reversed}");
    // Use frame deadlines for the sidebar; its 16 ms early-frame keep is covered separately.
    for _ in 0..20 {
        if a.motion.due.is_none() {
            break;
        }
        tick(&mut a, 20);
        a.compose();
    }
    assert_eq!(a.motion.anim.get::<f32>(id), Some(0.0), "closed");
    assert_eq!(a.motion.due, None);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A frame that redraws only the text keeps the turn going: the pane it skips is kept (D1), so
/// the chevron goes on from where it was rather than starting again from the target.
#[test]
fn a_frame_that_skips_the_pane_keeps_the_chevrons_turn() {
    let (mut a, dir) = tree_app("motion-tree-keep");
    a.compose();
    let i = tree_row(&a, "sub");
    let id = disclosure_id(&a, i);
    a.ui.tree.toggle(i);
    a.compose();
    tick(&mut a, 16);
    a.compose();
    let before = a.motion.anim.get::<f32>(id).expect("asked for");
    assert!(before < 1.0, "still turning: {before}");
    // Damage only the text: a character typed into the page.
    a.docs.activate(0);
    assert!(a.docs.edit(crate::docs::EditOp::Insert("x"), None));
    a.compose();
    let kept = a.motion.anim.get::<f32>(id);
    assert!(
        kept.is_some_and(|t| t >= before && t < 1.0),
        "the turn goes on from {before}: {kept:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn early_frames_do_not_postpone_the_sidebar_paint() {
    let (mut a, dir) = tree_app("motion-tree-cadence");
    a.compose();
    let i = tree_row(&a, "sub");
    let row = a.hit_rect(Hit::FilesRow(i)).unwrap();
    let x = a.areas.files.unwrap().x + 20.0 + a.ui.tree.rows()[i].depth as f32 * 14.0;
    let chevron = |a: &App| {
        let mut pixels = Vec::new();
        for dy in -6..6 {
            for dx in -6..6 {
                pixels.push(screen_pixel(a, x + dx as f32, row.cy() + dy as f32));
            }
        }
        pixels
    };

    a.ui.tree.toggle(i);
    a.compose();
    let before = chevron(&a);
    let due = a.motion.due.unwrap();
    tick(&mut a, 16);
    assert!(a.compose().0.is_empty(), "the early frame skips the pane");
    assert_eq!(a.motion.due, Some(due));
    tick(&mut a, 16);
    assert!(!a.compose().0.is_empty(), "the next frame paints the turn");
    let mid = chevron(&a);
    assert_ne!(mid, before);
    settle(&mut a);
    assert_ne!(
        chevron(&a),
        mid,
        "reached the endpoint during 16 ms redraws"
    );

    a.ui.tree.toggle(i);
    a.compose();
    assert!(a.motion.due.is_some());
    a.command(crate::commands::CommandId::ToggleFiles);
    a.compose();
    assert!(a.areas.files.is_none());
    assert_eq!(a.motion.due, None, "an absent pane has no deadline");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The frames of the turn damage the chevron that changed: no changed pixel is left undamaged.
#[test]
fn the_frames_of_the_turn_damage_what_changes() {
    let (mut a, dir) = tree_app("motion-tree-damage");
    a.compose();
    let i = tree_row(&a, "sub");
    a.ui.tree.toggle(i);
    a.compose();
    for _ in 0..3 {
        // A frame's pace is a sixtieth of a second: 17 ms of the hand-moved clock reaches the
        // deadline the animator asked for, where 16 would not (and would test nothing).
        tick(&mut a, 17);
        let before = a.screen.pixels.clone();
        let (damage, _) = a.compose();
        assert!(
            !damage.is_empty(),
            "the frame of the turn redraws something"
        );
        assert_eq!(
            changed_outside(&a, &before, &damage),
            0,
            "every pixel the turn changed is inside the damage"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- the Settings form (M-D) --------------------------------------------------------------------

/// A window with Settings open on the Typing page, composed, no caret blinking: the only timer
/// left is the form's.
fn settings_app() -> App {
    let mut a = moving();
    a.focused = false;
    a.open_settings_at(crate::app::modal::TYPING);
    a.compose();
    a
}

/// Toggling through the app with Animations off paints the new position on the first frame.
#[test]
fn with_motion_off_the_settings_dialog_is_one_frame() {
    let mut a = moving();
    a.focused = false;
    a.docs.prefs.animations = false;
    a.compose();
    let mut plain = a.screen.clone();
    a.open_settings_at(crate::app::modal::TYPING);
    a.compose();
    start_settings_slide(&mut a);
    let (w, h) = a.logical();
    a.ui.settings.as_mut().unwrap().draw(
        &mut Painter::new(&mut plain, &mut a.gfx, a.scale),
        &a.pal,
        Rect::new(0.0, 0.0, w, h),
        a.ui.caret_on,
    );
    assert!(
        a.screen.pixels == plain.pixels,
        "same first frame as Form::draw"
    );
    assert_eq!(a.motion.due, None, "nothing moves");
}

fn start_settings_slide(a: &mut App) {
    assert!(
        a.ui.settings
            .as_mut()
            .unwrap()
            .focus_row(crate::app::modal::AUTO_PAIR),
        "the switch"
    );
    a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Space), None);
    a.compose();
}

#[test]
fn settings_keeps_the_slide_on_early_frames() {
    let mut a = settings_app();
    start_settings_slide(&mut a);
    let initial = a.screen.pixels.clone();
    let due = a.motion.due.expect("a slide is scheduled");
    tick(&mut a, 16);
    assert!(a.compose().0.is_empty(), "early frame skips the form");
    assert_eq!(a.motion.due, Some(due), "keep the promised paint");
    tick(&mut a, 16);
    assert!(!a.compose().0.is_empty(), "the next frame draws the form");
    assert!(a.screen.pixels != initial, "the knob moved");
    let mid = a.screen.pixels.clone();
    settle(&mut a);
    assert!(a.screen.pixels != mid, "the knob then reached the endpoint");
}

#[test]
fn switching_settings_pages_discards_the_slide_on_the_old_page() {
    let mut a = settings_app();
    let typing = a.ui.settings.as_ref().unwrap().page();
    start_settings_slide(&mut a);
    tick(&mut a, 40);
    a.compose();
    assert!(a.motion.due.is_some(), "slide still in progress");
    let mid = a.screen.pixels.clone();
    let click = |a: &mut App, page| {
        let row = a.ui.settings.as_ref().unwrap().list_item_rect(page);
        a.ui.mouse = (row.cx(), row.cy());
        a.settings_press();
        a.settings_release();
        a.compose();
        assert_eq!(a.ui.settings.as_ref().unwrap().page(), page);
        assert_eq!(a.motion.due, None, "no old page animation");
    };
    click(&mut a, usize::from(typing == 0));
    assert!(a.screen.pixels != mid, "new page is visible immediately");
    click(&mut a, typing);
}

#[test]
fn closing_settings_mid_slide_restores_the_editor_on_the_first_frame() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let editor = a.screen.pixels.clone();
    a.open_settings_at(crate::app::modal::TYPING);
    a.compose();
    start_settings_slide(&mut a);
    tick(&mut a, 40);
    a.compose();
    assert!(a.motion.due.is_some());
    assert!(a.screen.pixels != editor);

    a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Escape), None);
    a.compose();
    assert!(a.ui.settings.is_none());
    assert!(a.screen.pixels == editor, "no trace of the dialog");
    assert_eq!(a.motion.due, None, "no dialog deadline");
}

/// A switch turned in the dialog slides: frames are asked for, they change the pixels, and once
/// it has arrived no timer is left.
#[test]
fn a_switch_in_the_settings_dialog_slides_and_then_rests() {
    let mut a = settings_app();
    assert_eq!(a.motion.due, None, "a dialog at rest asks for nothing");
    let f = a.ui.settings.as_mut().unwrap();
    assert!(f.focus_row(crate::app::modal::AUTO_PAIR), "the switch");
    let before = a.screen.pixels.clone();
    a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Space), None);
    a.compose();
    assert!(a.motion.due.is_some(), "the knob is on its way");
    // A frame of it changes pixels, and its damage holds every one that changed.
    let mid_before = a.screen.pixels.clone();
    tick(&mut a, 17);
    let (damage, _) = a.compose();
    assert!(!damage.is_empty(), "the frame redraws the window");
    assert!(a.screen.pixels != mid_before, "the switch moved");
    assert_eq!(
        changed_outside(&a, &mid_before, &damage),
        0,
        "every pixel the switch changed is inside the damage"
    );
    assert!(a.screen.pixels != before);
    for _ in 0..80 {
        if a.motion.due.is_none() {
            break;
        }
        tick(&mut a, 17);
        a.compose();
    }
    assert_eq!(a.motion.due, None, "and no timer once it rests");
}

/// Even a scroll that does nothing else must request a frame to erase the tip.
#[test]
fn a_scroll_hides_the_tip() {
    use winit::event::MouseScrollDelta::LineDelta;
    for case in ["title", "zero zoom step", "empty pane", "outside palette"] {
        let mut a = if case == "empty pane" { app() } else { still() };
        a.focused = false;
        if case == "outside palette" {
            a.motion = Motion::moving(ManualClock::new());
            a.open_palette(">fold");
            a.compose();
            settle(&mut a);
        }
        a.compose();
        let at = match case {
            "title" | "outside palette" => (10.0, 20.0),
            _ => (a.areas.text.cx(), a.areas.text.cy()),
        };
        a.ui.mouse = at;
        a.mods.ctrl = case == "zero zoom step";
        let delta = LineDelta(0.0, if a.mods.ctrl { 0.0 } else { -1.0 });
        let clean = a.screen.pixels.clone();
        let t = a.motion.now();
        a.update_tip(Some(a_tip_hit()), at, t);
        a.timers(t + TIP_WAIT);
        a.compose();
        if case == "outside palette" {
            tick(&mut a, 16);
            a.compose();
            assert!(a.motion.tip_t > 0.0 && a.motion.tip_t < 1.0);
        }
        assert!(a.screen.pixels != clean, "{case}: the tip was painted");
        a.wants_frame = false;
        a.wheel(delta);
        assert!(a.wants_frame, "{case}: erase the painted tip");
        assert!(a.ui.tip.shown().is_none(), "the scroll takes it away");
        assert!(a.ui.tip.waiting().is_none(), "and nothing waits");
        a.compose();
        assert!(a.screen.pixels == clean, "{case}: editor pixels restored");
        a.timers(t + TIP_WAIT * 3);
        assert!(a.ui.tip.shown().is_none());

        a.update_tip(Some(a_tip_hit()), at, t + TIP_WAIT * 3);
        assert!(a.ui.tip.waiting().is_some());
        a.wants_frame = false;
        a.wheel(delta);
        assert!(a.ui.tip.waiting().is_none());
        assert!(!a.wants_frame, "{case}: no painted tip to erase");
    }
}

/// Leaving the window and coming back to the same control at the same place arms the tip again
/// with its whole delay: nothing is left behind that would show it at once.
#[test]
fn coming_back_to_the_same_place_waits_the_whole_delay_again() {
    let mut a = still();
    a.focused = false;
    a.compose();
    let b = a.hit_rect(a_tip_hit()).unwrap();
    let device_id = winit::event::DeviceId::dummy();
    let back_at = || WindowEvent::CursorMoved {
        device_id,
        position: PhysicalPosition::new(f64::from(b.cx()), f64::from(b.cy())),
    };
    a.on_window_event(back_at());
    let first = a.ui.tip.deadline().expect("initial rest delay");
    a.timers(first);
    assert!(a.ui.tip.shown().is_some());
    a.on_window_event(WindowEvent::CursorLeft { device_id });
    assert!(a.ui.tip.shown().is_none());
    let back = Instant::now();
    a.on_window_event(back_at());
    let due =
        a.ui.tip
            .deadline()
            .expect("the same spot arms a fresh delay");
    assert!(due >= back + TIP_WAIT, "the whole delay again");
    a.timers(due - Duration::from_millis(1));
    assert!(a.ui.tip.shown().is_none(), "not before its time");
    a.timers(due);
    assert!(a.ui.tip.shown().is_some(), "and then it shows");
}

fn open_second_tab(a: &mut App) {
    let path = scratch("motion-second-tab").join("second.txt");
    std::fs::write(&path, "second tab\n").unwrap();
    let i = open_now(a, &path);
    assert_eq!(i, 1);
    assert_eq!(a.docs.active(), Some(i));
}

#[test]
fn middle_clicking_a_tab_dismisses_its_tooltip() {
    let mut a = still();
    a.focused = false;
    open_second_tab(&mut a);
    a.compose();
    let b = a.hit_rect(Hit::Tab(0)).unwrap();
    a.pointer_moved(b.cx(), b.cy());
    let due = a.ui.tip.deadline().expect("the tab tooltip waits");
    a.timers(due);
    a.compose();
    assert_eq!(a.ui.tip.shown().unwrap().key, Hit::Tab(0));
    let closed = a.docs.tabs[0].id;

    a.on_window_event(WindowEvent::MouseInput {
        device_id: winit::event::DeviceId::dummy(),
        state: ElementState::Pressed,
        button: MouseButton::Middle,
    });
    a.compose();
    assert!(a.docs.index_of_id(closed).is_none(), "the tab closed");
    assert!(a.ui.tip.shown().is_none(), "no tip for the closed tab");
    assert_eq!(a.motion.tip_t, 0.0, "no trace of the tip on the new frame");
}

// ---- the title row (M-E, D4) ---------------------------------------------------------------------

fn screen_pixel(a: &App, x: f32, y: f32) -> [u8; 4] {
    let offset = ((y * a.scale) as usize * a.screen.width as usize + (x * a.scale) as usize) * 4;
    a.screen.pixels[offset..offset + 4].try_into().unwrap()
}

#[test]
fn a_frame_that_skips_the_title_keeps_its_hover_fade() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let b = a.hit_rect(Hit::NewTab).unwrap();
    let at = (b.cx(), b.y + 4.0);
    let rest = screen_pixel(&a, at.0, at.1);
    a.pointer_moved(b.cx(), b.cy());
    a.compose();
    tick(&mut a, 17);
    a.compose();
    let mid = screen_pixel(&a, at.0, at.1);
    assert_ne!(mid, rest, "the fill has started to fade");

    let due = a.motion.due.unwrap();
    tick(&mut a, 16);
    assert!(
        a.compose().0.is_empty(),
        "the early frame skipped the title"
    );
    assert_eq!(screen_pixel(&a, at.0, at.1), mid);
    assert_eq!(a.motion.due, Some(due), "keep the promised paint");
    tick(&mut a, 16);
    a.compose();
    let later = screen_pixel(&a, at.0, at.1);
    assert_ne!(later, mid, "the fade continued");
    settle(&mut a);
    assert_ne!(
        screen_pixel(&a, at.0, at.1),
        later,
        "it did not snap to the end"
    );
}

/// Compare the app's first title frame after each pointer move with the original plain
/// controls, not another invocation of the animated drawing.
#[test]
fn with_motion_off_the_title_row_is_the_plain_drawing() {
    use mdshared_ui::chrome as plain;
    let mut a = moving();
    a.focused = false;
    a.docs.prefs.animations = false;
    open_second_tab(&mut a);
    a.compose();
    for hit in [Hit::WinClose, Hit::Tab(0)] {
        if hit == Hit::WinClose && plain::NATIVE_FRAME {
            // macOS has no app-drawn caption buttons; the quiet-tab branch still runs.
            assert!(a.hit_rect(hit).is_none());
            continue;
        }
        let b = a.hit_rect(hit).expect("a title control");
        for hovered in [true, false] {
            if hovered {
                a.pointer_moved(b.cx(), b.cy());
                assert_eq!(a.ui.hover, Some(hit));
            } else {
                a.pointer_moved(10.0, a.areas.text.cy());
            }
            a.compose();
            let title = a.areas.title;
            let mut expected = mdshared_text::Bitmap::new(a.screen.width, a.screen.height);
            let mut p = Painter::new(&mut expected, &mut a.gfx, a.scale);
            p.fill(title, a.pal.title);
            let crop = if hit == Hit::WinClose {
                plain::draw_win_button(
                    &mut p,
                    &a.pal,
                    plain::WinButton::Close,
                    b,
                    a.maximized,
                    hovered,
                    false,
                );
                b
            } else {
                plain::draw_tab(
                    &mut p,
                    &a.pal,
                    b.x,
                    b.w,
                    title.w,
                    &a.docs.labels()[0],
                    &plain::TabState {
                        hovered,
                        ..Default::default()
                    },
                );
                // Include fill, label and close glyph, but exclude the active neighbour's foot.
                Rect::new(b.x, b.y, b.w, title.bottom() - 8.0 - b.y)
            };
            let (x0, y0, x1, y1) = p.dev_rect(crop);
            let stride = a.screen.width as usize * 4;
            assert!(
                (y0 as usize..y1 as usize).all(|y| {
                    let row = y * stride + x0 as usize * 4..y * stride + x1 as usize * 4;
                    a.screen.pixels[row.clone()] == expected.pixels[row]
                }),
                "{hit:?}, hovered={hovered}: first frame matches the plain drawing"
            );
            assert_eq!(a.motion.due, None, "motion is off");
        }
    }
}

/// Hovering a caption button fades its fill in: frames are asked for, they change the title
/// row, and once it has arrived no timer is left.
#[test]
fn a_caption_button_fades_in_and_then_rests() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let b = a
        .hit_rect(crate::ui::Hit::WinClose)
        .expect("the close button");
    let before = a.screen.pixels.clone();
    a.pointer_moved(b.cx(), b.cy());
    a.compose();
    assert!(a.motion.due.is_some(), "the fill is on its way");
    let mid_before = a.screen.pixels.clone();
    tick(&mut a, 17);
    let (damage, _) = a.compose();
    assert!(a.screen.pixels != mid_before, "the button changed");
    assert!(a.screen.pixels != before);
    assert_eq!(
        changed_outside(&a, &mid_before, &damage),
        0,
        "every pixel the fade changed is inside the damage"
    );
    for _ in 0..80 {
        if a.motion.due.is_none() {
            break;
        }
        tick(&mut a, 17);
        a.compose();
    }
    assert_eq!(a.motion.due, None, "and no timer once it rests");
}

/// The new-tab button's fill fades too, from the tab strip's own group, and the tab's hover with
/// it: the title region is damaged while either moves.
#[test]
fn the_new_tab_button_and_a_tab_fade_with_the_title_row() {
    let mut a = moving();
    a.focused = false;
    open_second_tab(&mut a);
    a.compose();
    let b = a
        .hit_rect(crate::ui::Hit::NewTab)
        .expect("the new tab button");
    a.pointer_moved(b.cx(), b.cy());
    a.compose();
    assert!(a.motion.due.is_some(), "the new-tab fill is on its way");
    let at = (b.cx(), b.y + 4.0);
    let rest = screen_pixel(&a, at.0, at.1);
    let before = a.screen.pixels.clone();
    tick(&mut a, 17);
    let (damage, _) = a.compose();
    assert_ne!(screen_pixel(&a, at.0, at.1), rest, "new-tab fill changed");
    assert_eq!(changed_outside(&a, &before, &damage), 0);
    settle(&mut a);
    a.pointer_moved(10.0, a.areas.text.cy());
    a.compose();
    settle(&mut a);

    let t = a.hit_rect(Hit::Tab(0)).expect("the quiet tab");
    let at = (t.cx(), t.y + 6.0); // inside the fill and above the text and close glyph
    let rest = screen_pixel(&a, at.0, at.1);
    a.pointer_moved(t.cx(), t.cy());
    a.compose();
    assert!(a.motion.due.is_some(), "the tab's hover is on its way");
    assert_eq!(screen_pixel(&a, at.0, at.1), rest, "no flash at t = 0");
    let before = a.screen.pixels.clone();
    tick(&mut a, 35);
    let (damage, _) = a.compose();
    let mid = screen_pixel(&a, at.0, at.1);
    assert_ne!(mid, rest, "the quiet tab's fill changed");
    let (x, y) = ((at.0 * a.scale) as i32, (at.1 * a.scale) as i32);
    assert!(
        damage
            .iter()
            .any(|&(l, t, r, b)| l <= x && x < r && t <= y && y < b)
    );
    assert_eq!(changed_outside(&a, &before, &damage), 0);
    settle(&mut a);
    assert_ne!(screen_pixel(&a, at.0, at.1), mid, "the full hover arrived");
}

#[test]
fn hovering_the_active_tab_does_not_start_a_fade() {
    let mut a = moving();
    a.focused = false;
    open_second_tab(&mut a);
    a.compose();
    let rest = a.screen.pixels.clone();
    let tab = a.hit_rect(Hit::Tab(1)).unwrap();
    a.pointer_moved(tab.cx(), tab.cy());
    assert_eq!(a.ui.hover, Some(Hit::Tab(1)));
    a.compose();
    assert!(a.screen.pixels == rest, "the active tab did not change");
    assert_eq!(a.motion.due, None, "nothing needs a follow-up frame");

    let close = a.hit_rect(Hit::TabClose(1)).unwrap();
    a.pointer_moved(close.cx(), close.cy());
    a.compose();
    assert!(
        a.screen.pixels != rest,
        "the close button still lights at once"
    );
    assert_eq!(a.motion.due, None);
}

// ---- X3: the menu's hover rows, the palette and the notice bars ------------------------------

#[derive(Debug, Clone, Copy)]
enum BannerDemo {
    Update,
    Crash,
    Stale,
    Rename,
}

impl BannerDemo {
    const ALL: [Self; 4] = [Self::Update, Self::Crash, Self::Stale, Self::Rename];

    fn show(self, a: &mut App) -> Hit {
        match self {
            Self::Update => {
                a.docs.updates.preview("available");
                Hit::UpdateGo
            }
            Self::Crash => {
                // A synthetic notice: real startup checks for crash reports at settle,
                // after the first paint. Never open or mark a real report here.
                a.docs.crash = Some("test-report.txt".into());
                Hit::CrashOpen
            }
            Self::Stale => {
                a.session.use_days = (18, 0);
                a.docs.start_day(&mut a.session);
                a.docs.tabs[0].seen = 1;
                a.docs.offer_stale_tabs();
                assert_eq!(a.docs.stale, [a.docs.tabs[0].id]);
                Hit::StaleClose
            }
            Self::Rename => {
                a.ai_made_markdown(a.docs.active_doc().unwrap().id, None);
                Hit::RenameGo
            }
        }
    }
}

fn banner_app() -> App {
    let mut a = moving();
    a.focused = false;
    a.docs.format_bar = false;
    open_second_tab(&mut a);
    a
}

#[test]
fn notices_present_at_start_get_their_full_height_without_motion() {
    for demo in BannerDemo::ALL {
        let mut a = banner_app();
        let hit = demo.show(&mut a);
        a.compose();
        assert_eq!(
            a.areas.text.y,
            a.areas.title.bottom() + crate::ui::BAR_H + view::BANNER_H,
            "{demo:?}: launch reserves the whole strip"
        );
        assert!(a.hit_rect(hit).is_some(), "{demo:?}: controls are drawn");
        assert_eq!(a.motion.due, None, "{demo:?}: no launch animation");
        let top = a.areas.text.y;
        tick(&mut a, 16);
        a.compose();
        assert_eq!(a.areas.text.y, top, "{demo:?}: no second-frame slide");
        assert_eq!(a.motion.due, None);
    }
}

#[test]
fn launch_layout_with_a_stale_notice_matches_the_first_paint_and_goto() {
    let mut a = moving();
    a.docs.prefs.animations = false;
    check_stale_notice_launch_layout(a);
}

#[test]
fn launch_layout_with_animations_on_reserves_the_stale_notice() {
    let a = moving();
    assert!(a.docs.prefs.animations, "test the default preference");
    assert!(a.motion.anim.animates(), "use a moving window");
    check_stale_notice_launch_layout(a);
}

fn check_stale_notice_launch_layout(mut a: App) {
    let path = scratch("stale-goto").join("long.txt");
    std::fs::write(&path, "scroll to a line\n".repeat(240)).unwrap();
    open_now(&mut a, &path);
    a.focused = false;
    BannerDemo::Stale.show(&mut a);
    a.args.goto = Some(crate::args::LineCol { line: 120, col: 1 });
    a.compose_layout_only();
    let text = a.areas.text;
    a.apply_positions();
    let top = a.docs.active_ready().unwrap().view.top;
    a.compose();
    assert_eq!(
        text, a.areas.text,
        "layout and paint reserve the same strip"
    );
    assert_eq!(a.docs.active_ready().unwrap().view.top, top);

    // The same `--goto` in the painted geometry gives the same top line.
    a.docs.active_ready_mut().unwrap().view.top = 0.0;
    a.apply_positions();
    assert_eq!(a.docs.active_ready().unwrap().view.top, top);
}

#[test]
fn motion_off_menu_hover_uses_plain_drawing() {
    let mut a = moving();
    a.focused = false;
    a.docs.prefs.animations = false;
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    let rest = a.screen.pixels.clone();
    let menu = a.ui.popup.as_ref().unwrap().root().rect();
    a.pointer_moved(menu.cx(), menu.y + 26.0);
    a.compose();
    assert!(
        a.screen.pixels != rest,
        "the first hover frame draws the fill"
    );
    let (w, h) = a.logical();
    let mut plain = mdshared_text::Bitmap::new(a.screen.width, a.screen.height);
    let popup = a.ui.popup.as_mut().unwrap();
    let row = popup
        .root()
        .hover
        .expect("an enabled row under the pointer");
    let mut p = Painter::new(&mut plain, &mut a.gfx, a.scale);
    popup
        .root_mut()
        .draw_in(&mut p, &a.pal, menu_room(a.areas.title, w, h));
    let (x0, y0, x1, y1) = p.dev_rect(popup.root().row_rect(row).inset(3.0));
    let stride = a.screen.width as usize * 4;
    assert!(
        (y0 as usize..y1 as usize).all(|y| {
            let span = y * stride + x0 as usize * 4..y * stride + x1 as usize * 4;
            a.screen.pixels[span.clone()] == plain.pixels[span]
        }),
        "hovered row matches draw_in"
    );
    let id = mdshared_ui::anim::Id::new(("row-hover", popup.serial)).with(("row", row));
    assert!(
        a.motion.anim.get::<f32>(id).is_none(),
        "plain drawing creates no row track"
    );
    assert_eq!(a.motion.due, None);
}

#[test]
fn motion_off_command_palette_is_full_on_its_first_frame() {
    let mut a = moving();
    a.focused = false;
    a.docs.prefs.animations = false;
    a.compose();
    let clean = a.screen.pixels.clone();
    a.open_palette(">fold");
    a.compose();
    assert_eq!(a.motion.palette_t, 1.0);
    assert!(
        a.screen.pixels != clean,
        "the palette painted on the first frame"
    );
    assert_eq!(a.motion.due, None);
}

#[test]
fn motion_off_notices_get_full_height_when_they_appear_later() {
    for demo in BannerDemo::ALL {
        let mut a = banner_app();
        a.docs.prefs.animations = false;
        a.compose();
        let top = a.areas.text.y;
        let hit = demo.show(&mut a);
        a.compose();
        assert_eq!(a.areas.text.y, top + view::BANNER_H, "{demo:?}: full strip");
        assert!(a.hit_rect(hit).is_some(), "{demo:?}: controls are visible");
        assert_eq!(a.motion.due, None, "{demo:?}: no follow-up frame");
    }
}

#[test]
fn the_anchored_language_list_opens_without_motion() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let clean = a.screen.pixels.clone();
    a.open_languages();
    a.compose();
    assert!(a.palette.open.as_ref().unwrap().anchored);
    assert_eq!(a.motion.palette_t, 1.0);
    assert!(
        a.screen.pixels != clean,
        "the list painted on the first frame"
    );
    assert_eq!(a.motion.due, None);
}

#[test]
fn closing_the_palette_mid_entrance_erases_it_at_once() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let clean = a.screen.pixels.clone();
    a.open_palette(">fold");
    a.compose();
    tick(&mut a, 40);
    a.compose();
    assert!(a.motion.palette_t > 0.0 && a.motion.palette_t < 1.0);
    assert!(a.screen.pixels != clean);
    a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Escape), None);
    a.compose();
    assert!(!a.palette_open());
    assert!(a.screen.pixels == clean, "nothing of the palette remains");
    assert_eq!(a.motion.due, None);
}

#[test]
fn a_notice_dismissed_mid_entrance_releases_its_strip_at_once() {
    let mut a = banner_app();
    a.compose();
    let top = a.areas.text.y;
    let hit = BannerDemo::Stale.show(&mut a);
    a.compose();
    tick(&mut a, 40);
    a.compose();
    assert!(a.areas.text.y > top);
    assert!(a.hit_rect(hit).is_some());
    a.docs.keep_stale();
    a.compose();
    assert_eq!(a.areas.text.y, top);
    assert!(
        a.hit_rect(hit).is_none(),
        "no live hits from the dismissed notice"
    );
    assert_eq!(a.motion.due, None);
}

#[test]
fn late_crash_stale_and_rename_notices_slide_with_their_hits() {
    for demo in [BannerDemo::Crash, BannerDemo::Stale, BannerDemo::Rename] {
        let mut a = banner_app();
        a.compose();
        let top = a.areas.text.y;
        let hit = demo.show(&mut a);
        a.compose();
        assert_eq!(a.areas.text.y, top, "{demo:?}: no strip before it enters");
        assert!(a.hit_rect(hit).is_none());
        tick(&mut a, 40);
        a.compose();
        let y = a.areas.text.y;
        assert!(top < y && y < top + view::BANNER_H, "{demo:?}: mid-slide");
        let b = a.hit_rect(hit).expect("partly visible button");
        assert!(b.y >= top && b.bottom() <= y, "{demo:?}: clipped hit");
        settle(&mut a);
        assert_eq!(
            a.areas.text.y,
            top + view::BANNER_H,
            "{demo:?}: set at rest"
        );
    }
}

#[test]
fn returning_to_a_rename_offer_does_not_replay_its_entrance() {
    let mut a = banner_app();
    a.compose();
    let top = a.areas.text.y;
    let hit = BannerDemo::Rename.show(&mut a);
    a.compose();
    tick(&mut a, 40);
    a.compose();
    assert!(top < a.areas.text.y && a.areas.text.y < top + view::BANNER_H);

    a.docs.activate(0);
    a.compose();
    assert_eq!(a.areas.text.y, top);
    assert_eq!(a.motion.due, None);

    a.docs.activate(1);
    a.compose_layout_only();
    assert_eq!(
        a.areas.text.y,
        top + view::BANNER_H,
        "layout predicts the return"
    );
    a.compose();
    assert_eq!(
        a.areas.text.y,
        top + view::BANNER_H,
        "the same offer returns at rest"
    );
    assert!(a.hit_rect(hit).is_some());
    assert_eq!(a.motion.due, None);

    a.rename_bar(Hit::RenameDismiss);
    a.compose();
    BannerDemo::Rename.show(&mut a);
    a.compose();
    assert_eq!(a.areas.text.y, top, "a fresh offer can still slide in");
    assert!(a.motion.due.is_some());
}

/// A menu's hovered row fades its fill in (`Menu::draw_in_animated`): once the menu itself has
/// arrived, putting the pointer on a row asks the animator for frames again, the row changes,
/// and it rests with no timer left.
#[test]
fn a_menu_row_fades_its_fill_in_and_then_rests() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    settle(&mut a);
    assert_eq!(a.motion.due, None, "the menu itself has arrived");
    let arrived = a.screen.pixels.clone();
    let menu = a.ui.popup.as_ref().expect("a menu").root().rect();
    a.pointer_moved(menu.cx(), menu.y + 26.0);
    a.compose();
    assert!(a.motion.due.is_some(), "a row's fill is on its way");
    // On the frame the hover begins the row still draws its rest look (shared's `t == 0`).
    assert!(a.screen.pixels == arrived, "nothing yet");
    tick(&mut a, 16);
    let (damage, _) = a.compose();
    assert!(damage.is_empty(), "this frame skips the menu");
    assert!(a.motion.due.is_some(), "skipping keeps the row's deadline");
    tick(&mut a, 16);
    a.compose();
    let mid = a.screen.pixels.clone();
    assert!(mid != arrived, "the row changed");
    for _ in 0..80 {
        if a.motion.due.is_none() {
            break;
        }
        tick(&mut a, 17);
        a.compose();
    }
    assert_eq!(a.motion.due, None, "and no timer once it rests");
    assert!(a.screen.pixels != mid, "the fill arrived");
}

/// The command palette pops in: it is asked for frames while it enters, its card is in the
/// damage, and it rests at 1 with no timer, drawn by `draw_presence` at 1 (its plain drawing).
#[test]
fn the_palette_pops_in_and_then_rests() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    a.open_palette(">fold");
    a.compose();
    assert!(a.motion.palette_t < 1.0, "the card is on its way in");
    assert!(a.motion.due.is_some(), "and a frame is asked for");
    let card = {
        let p = a.palette.open.as_ref().expect("the palette");
        p.palette.presence_bounds()
    };
    let before = a.screen.pixels.clone();
    tick(&mut a, 17);
    let (damage, _) = a.compose();
    // The card's own place is drawn again: some damage reaches it (the palette is over the
    // whole window, so a region's damage reaches it too).
    let reaches = |d: &(i32, i32, i32, i32)| {
        let (x0, y0, x1, y1) = (d.0 as f32, d.1 as f32, d.2 as f32, d.3 as f32);
        x0 <= card.cx() && card.cx() <= x1 && y0 <= card.cy() && card.cy() <= y1
    };
    assert!(
        !damage.is_empty() && damage.iter().any(reaches),
        "the card is in the damage: {damage:?}"
    );
    assert!(a.screen.pixels != before);
    for _ in 0..40 {
        if a.motion.due.is_none() {
            break;
        }
        tick(&mut a, 20);
        a.compose();
    }
    assert_eq!(a.motion.palette_t, 1.0, "at rest");
    assert_eq!(a.motion.due, None, "no timer once it rests");
}

/// A notice bar slides down into its strip (`notice::draw_sliding`): while it moves the strip's
/// top shows the bar cut short, and at rest the bar fills its strip; no timer is left.
#[test]
fn an_update_bar_slides_into_its_strip_and_then_rests() {
    let mut a = moving();
    a.focused = false;
    a.compose();
    let top = a.areas.text.y;
    a.docs.updates.preview("available");
    a.compose();
    assert!(a.motion.due.is_some(), "the bar is on its way down");
    assert_eq!(a.areas.text.y, top, "no empty strip at the start");
    assert!(a.hit_rect(Hit::UpdateGo).is_none());
    tick(&mut a, 40);
    a.compose();
    let mid = a.areas.text.y;
    assert!(top < mid && mid < top + view::BANNER_H);
    let go = a
        .hit_rect(Hit::UpdateGo)
        .expect("the visible update button");
    assert!(
        go.y >= top && go.bottom() <= mid,
        "hits stay in the visible strip"
    );
    let moving = a.screen.pixels.clone();
    settle(&mut a);
    assert_eq!(a.motion.due, None, "at rest");
    assert_eq!(a.areas.text.y, top + view::BANNER_H);
    assert!(a.screen.pixels != moving);
}
