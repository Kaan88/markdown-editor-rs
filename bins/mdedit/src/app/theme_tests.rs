//! Theme blending through the normal compositor, using only the manual frame clock.

use super::*;
use crate::app::files_tests::{app, scratch};
use crate::ui::BarItem;
use mdshared_app::backdrop::Backdrop;
use mdshared_ui::anim::ManualClock;
use winit::dpi::PhysicalSize;

pub(super) fn window(dark: bool) -> App {
    let mut a = app();
    a.motion = crate::motion::Motion::moving(ManualClock::new());
    a.focused = false;
    flip(&mut a, dark);
    a
}

fn flip(a: &mut App, dark: bool) {
    a.set_theme(if dark {
        ThemeChoice::Dark
    } else {
        ThemeChoice::Light
    });
}

fn advance(a: &mut App, ms: u32) {
    for step in super::motion_tests::transition_steps(ms) {
        a.motion.advance_ms(step);
        a.compose();
    }
}

fn pixel(a: &App) -> [u8; 4] {
    let at = ((a.screen.height / 2 * a.screen.width + a.screen.width / 5) * 4) as usize;
    a.screen.pixels[at..at + 4].try_into().unwrap()
}

fn at_rest(a: &App) {
    assert!(a.motion.theme_fade.is_none(), "no retained snapshot");
    assert_eq!(a.motion.due, None, "no animation deadline");
}

#[test]
fn a_theme_flip_crossfades_the_whole_window_over_170_ms_in_either_direction() {
    for from_dark in [false, true] {
        let mut a = window(from_dark);
        a.compose();
        let from = a.screen.pixels.clone();
        let old = pixel(&a);
        let mut still = window(!from_dark);
        still.compose();
        let new = pixel(&still);
        assert_ne!(old, new);

        flip(&mut a, !from_dark);
        a.compose();
        let s = a
            .motion
            .theme_fade
            .as_ref()
            .expect("previous window is retained");
        assert_eq!(s.bitmap_size(), (1280, 800));
        assert_eq!(s.rect(), (0, 0, 1280, 800));
        assert!(
            a.screen.pixels == from,
            "the first frame is entirely the old frame"
        );
        // 5 × 17 ms, not one clock jump (the stall guard clamps it).
        for _ in 0..5 {
            a.motion.advance_ms(17);
            let (damage, _) = a.compose();
            assert!(
                (0..800).all(|y| damage
                    .iter()
                    .any(|&(x0, y0, x1, y1)| x0 == 0 && x1 == 1280 && y0 <= y && y < y1)),
                "each row is presented through the fade"
            );
        }
        let mid = pixel(&a);
        assert_ne!(mid, old);
        assert_ne!(mid, new);
        for (m, (l, r)) in mid[..3].iter().zip(old.into_iter().zip(new)) {
            assert!((i16::from(*m) * 2 - i16::from(l) - i16::from(r)).abs() <= 2);
        }
        advance(&mut a, 84);
        assert!(a.motion.theme_fade.is_some(), "still in flight at 169 ms");
        advance(&mut a, 1);
        assert!(
            a.screen.pixels == still.screen.pixels,
            "pure new palette at 170 ms"
        );
        at_rest(&a);
        assert!(a.compose().0.is_empty());
    }
}

#[test]
fn first_paint_never_snapshots_the_initial_palette() {
    let mut a = window(false);
    a.docs.theme = ThemeChoice::System;
    a.system_dark = false;
    // Exercise the frame guard, not just the bitmap-size guard.
    a.pal = theme::DARK;
    a.screen = Bitmap::new(a.size.0, a.size.1);
    a.compose();
    assert_eq!(a.pal, theme::LIGHT);
    assert_eq!(a.motion.theme_serial, 0, "frame zero did not capture");
    at_rest(&a);
}

#[test]
fn another_flip_uses_the_already_blended_frame() {
    let mut a = window(false);
    a.compose();
    let light = pixel(&a);
    flip(&mut a, true);
    a.compose();
    advance(&mut a, 85);
    let blended = a.screen.pixels.clone();
    assert_ne!(pixel(&a), light);
    let first_serial = a.motion.theme_serial;
    flip(&mut a, false);
    a.compose();
    assert_ne!(a.motion.theme_serial, first_serial);
    assert!(a.screen.pixels == blended, "the interruption does not jump");
    advance(&mut a, 170);
    assert_eq!(pixel(&a), light);
    at_rest(&a);
}

#[test]
fn same_size_resize_keeps_the_fade_but_resize_and_minimise_release_it_without_paint() {
    for new_size in [PhysicalSize::new(1100, 700), PhysicalSize::new(0, 0)] {
        let mut a = window(false);
        a.compose();
        flip(&mut a, true);
        a.compose();
        advance(&mut a, 17);
        let serial = a.motion.theme_serial;
        a.on_window_event(WindowEvent::Resized(PhysicalSize::new(1280, 800)));
        assert!(a.motion.theme_fade.is_some(), "same-size notification");
        advance(&mut a, 17);
        assert_eq!(a.motion.theme_serial, serial);
        a.on_window_event(WindowEvent::Resized(new_size));
        assert!(
            a.motion.theme_fade.is_none(),
            "released before another frame"
        );
        a.compose();
        at_rest(&a);
    }
}

#[test]
fn a_resize_concurrent_with_a_flip_does_not_capture_the_old_size() {
    let mut a = window(false);
    a.compose();
    a.on_window_event(WindowEvent::Resized(PhysicalSize::new(1024, 640)));
    flip(&mut a, true);
    a.compose();
    assert_eq!(
        a.motion.theme_serial, 0,
        "did not capture an old or blank bitmap"
    );
    assert_eq!((a.screen.width, a.screen.height), (1024, 640));
    at_rest(&a);
}

#[test]
fn a_layout_only_pass_drops_a_fade_and_does_not_capture_a_new_one() {
    let mut a = window(false);
    a.compose();
    flip(&mut a, true);
    a.compose();
    advance(&mut a, 17);
    let serial = a.motion.theme_serial;
    assert!(a.motion.theme_fade.is_some());
    a.compose_layout_only();
    assert!(
        a.motion.theme_fade.is_none(),
        "layout-only releases an existing fade"
    );
    flip(&mut a, false);
    a.compose_layout_only();
    assert_eq!(a.motion.theme_serial, serial, "layout-only cannot capture");
    a.compose();
    at_rest(&a);
}

#[test]
fn a_flip_over_the_pixel_cap_snaps() {
    let mut a = window(false);
    a.size = (2561, 1600);
    a.compose();
    let before = pixel(&a);
    flip(&mut a, true);
    a.compose();
    at_rest(&a);
    assert_ne!(pixel(&a), before);
    assert_eq!(a.motion.theme_serial, 0);
}

#[test]
fn off_reduced_and_backdrop_snap_by_the_first_frame_and_keep_no_snapshot() {
    for policy in ["off", "reduced", "backdrop"] {
        let mut a = window(false);
        a.compose();
        match policy {
            "off" => a.docs.prefs.animations = false,
            "reduced" => a.motion.set_reduced(true),
            _ => a.docs.prefs.backdrop = Backdrop::Mica,
        }
        flip(&mut a, true);
        a.compose();
        assert_eq!(a.motion.theme_serial, 0, "{policy}: nothing captured");
        at_rest(&a);
        let first = a.screen.pixels.clone();
        a.full = true;
        a.compose();
        assert!(
            a.screen.pixels == first,
            "{policy}: first frame was already final"
        );
    }
}

#[test]
fn turning_motion_off_or_enabling_backdrop_during_a_flip_drops_it() {
    for backdrop in [false, true] {
        let mut a = window(false);
        a.compose();
        flip(&mut a, true);
        a.compose();
        advance(&mut a, 17);
        assert!(a.motion.theme_fade.is_some());
        if backdrop {
            a.docs.prefs.backdrop = Backdrop::Mica;
        } else {
            a.docs.prefs.animations = false;
        }
        a.compose();
        at_rest(&a);
    }
}

#[test]
fn closing_settings_during_a_theme_flip_leaves_no_image_of_it() {
    let mut a = window(false);
    a.compose();
    a.open_settings();
    a.compose();
    advance(&mut a, 117);
    flip(&mut a, true);
    a.compose();
    advance(&mut a, 17);
    assert!(a.motion.theme_fade.is_some());
    a.close_settings();
    a.compose();
    let mut plain = window(true);
    plain.compose();
    assert!(
        a.screen.pixels == plain.screen.pixels,
        "no ghost of Settings"
    );
    at_rest(&a);
}

#[test]
fn choosing_dark_from_the_palette_fades_from_a_frame_without_it() {
    choose_dark_from_overlay(
        |a| {
            a.open_palette(">dark theme");
            a.compose();
            advance(a, 117);
        },
        |a| {
            a.key_pressed(crate::keys::Key::Name(crate::keys::KeyName::Enter), None);
            assert!(!a.palette_open());
        },
    );
}

#[test]
fn choosing_dark_from_the_main_menu_fades_from_a_frame_without_it() {
    choose_dark_from_overlay(
        |a| {
            a.open_popup(PopupKind::Main);
            a.compose();
            advance(a, 117);
        },
        |a| {
            let menu = a.ui.popup.as_ref().unwrap().root();
            let i = menu
                .items
                .iter()
                .position(|row| row.id == Act::Theme(ThemeChoice::Dark))
                .unwrap();
            let row = menu.row_rect(i);
            a.pointer_moved(row.cx(), row.cy());
            a.left_press();
            a.left_release();
            assert!(a.ui.popup.is_none());
        },
    );
}

fn choose_dark_from_overlay(open: impl Fn(&mut App), choose: impl Fn(&mut App)) {
    let mut dark = window(true);
    dark.compose();
    for enabled in [true, false] {
        let mut a = window(false);
        a.docs.prefs.animations = enabled;
        a.compose();
        let old = a.screen.pixels.clone();
        let old_pixel = pixel(&a);
        open(&mut a);
        assert!(a.screen.pixels != old, "the overlay was painted");
        choose(&mut a);
        assert!(a.dark());
        a.compose();
        if enabled {
            assert!(a.motion.theme_fade.is_some(), "the flip still fades");
            assert!(
                a.screen.pixels == old,
                "first paint has the old palette and theme button but no overlay"
            );
            advance(&mut a, 85);
            assert_ne!(pixel(&a), old_pixel);
            assert_ne!(pixel(&a), pixel(&dark));
            advance(&mut a, 85);
        }
        at_rest(&a);
        assert!(a.screen.pixels == dark.screen.pixels, "exact final drawing");
    }
}

#[test]
fn closing_a_dialog_drops_an_active_theme_snapshot_before_the_next_paint() {
    use crate::commands::CommandId;
    let mut a = window(false);
    a.compose();
    a.command(CommandId::About);
    a.compose();
    assert!(a.ui.dialog.is_some());
    flip(&mut a, true);
    a.compose();
    advance(&mut a, 17);
    assert!(a.motion.theme_fade.is_some());
    a.close_dialog();
    assert!(
        a.motion.theme_fade.is_none(),
        "closing releases the snapshot immediately"
    );
    a.compose();
    at_rest(&a);
    let first = a.screen.pixels.clone();
    a.full = true;
    a.compose();
    assert!(a.screen.pixels == first, "no ghost of the dialog");
}

#[test]
fn replacing_an_open_menu_during_a_system_flip_drops_its_old_image() {
    let mut a = window(false);
    a.set_theme(ThemeChoice::System);
    a.system_dark = false;
    a.compose();
    a.open_popup(PopupKind::Main);
    a.compose();
    advance(&mut a, 117);
    let previous = a.ui.popup.as_ref().unwrap().serial;
    a.system_dark = true;
    a.compose();
    advance(&mut a, 34);
    assert!(a.motion.theme_fade.is_some());

    // The menu is swapped in one input path, with no intervening closed paint.
    a.activate(Hit::Bar(BarItem::AiEdit));
    let current = a.ui.popup.as_ref().unwrap();
    assert_eq!(current.kind, PopupKind::Ai);
    assert_ne!(current.serial, previous);
    a.compose();
    assert!(
        a.motion.theme_fade.is_none(),
        "the replaced menu's snapshot is gone"
    );

    let mut plain = window(true);
    plain.set_theme(ThemeChoice::System);
    plain.system_dark = true;
    plain.compose();
    plain.activate(Hit::Bar(BarItem::AiEdit));
    plain.compose();
    assert!(
        a.screen.pixels == plain.screen.pixels,
        "no trace of the first menu"
    );
}

#[test]
fn changing_settings_page_during_a_flip_drops_the_old_page_image() {
    let mut a = window(false);
    a.compose();
    a.open_settings();
    a.compose();
    advance(&mut a, 117);
    flip(&mut a, true);
    a.compose();
    advance(&mut a, 34);
    assert!(a.motion.theme_fade.is_some());

    let form = a.ui.settings.as_ref().unwrap();
    assert_eq!(form.page(), 0);
    let r = form.list_item_rect(1);
    a.ui.mouse = (r.cx(), r.cy());
    a.settings_press();
    a.settings_release();
    assert_eq!(a.ui.settings.as_ref().unwrap().page(), 1);
    a.compose();
    assert!(
        a.motion.theme_fade.is_none(),
        "the old page's snapshot is gone"
    );
}

#[test]
fn the_system_theme_flip_shot_keeps_settings_on_system_in_both_directions() {
    let dir = scratch("system-theme-with-settings");
    let out = dir.join("theme.png");
    let render = |mut a: App| {
        a.args.dialog = Some(crate::args::ShotDialog::Settings);
        a.args.scale = Some(1.0);
        assert_eq!(shot(a, &out), 0);
        std::fs::read(&out).unwrap()
    };
    let still = |dark| {
        let mut a = app();
        a.docs.theme = ThemeChoice::System;
        a.system_dark = dark;
        render(a)
    };
    let (light, dark) = (still(false), still(true));
    for (target, before, after) in [(true, &light, &dark), (false, &dark, &light)] {
        for (ms, expected) in [(0, before), (170, after)] {
            let mut a = app();
            a.args.theme_flip = Some(target);
            a.args.motion_clock = Some(ms);
            assert!(
                &render(a) == expected,
                "at {ms} ms, the flip to dark={target} matches the System dialog"
            );
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}
