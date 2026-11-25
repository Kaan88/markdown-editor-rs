//! The window's animator and the menus' fade: the deadline that wakes the window while a menu
//! enters and none after it (or at idle), the frames that draw only the menus, and everything
//! that keeps a menu instant (reduced motion, the Animations setting, the completion list, a
//! shot or a test).

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::motion::{MENUS, Motion};
use mdshared_ui::anim::{ManualClock, Policy};
use mdshared_ui::menu::{Item, Menu};

/// A window with a file open, whose motion moves on a clock moved by hand.
fn moving() -> App {
    let dir = scratch("motion");
    let path = dir.join("a.txt");
    std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
    let mut a = app();
    a.motion = Motion::moving(ManualClock::new());
    open_now(&mut a, &path);
    a
}

/// The same window, still (as every test and shot is).
fn still() -> App {
    let mut a = moving();
    a.motion = Motion::new(true);
    a
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
    // 167 ms at 16 ms a frame, and the frame that shows it arrived.
    assert!((10..=13).contains(&frames), "{frames} frames");
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
    let mut shot = app();
    shot.motion = Motion::new(true);
    assert_eq!(shot.motion.policy(), Policy::Never);
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
    let system = crate::platform::motion_reduced();
    a.motion.set_reduced(!system);
    assert_eq!(a.motion.anim.animates(), system);
    a.wants_frame = false;
    a.woken(Wake::Motion);
    assert_eq!(a.motion.anim.animates(), !system, "the system's answer now");
    assert!(a.wants_frame, "and a frame to show it");
}
