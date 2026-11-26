//! Headless tests of the frosted chrome: which pixels of a composed frame carry the mark that
//! lets the backdrop through. There is no window, so the system is a stand-in that says yes.

use super::files_tests::app;
use super::*;
use crate::frost::frosted;
use crate::ui::PopupKind;
use mdshared_app::backdrop::Backdrop;
use mdshared_ui::frost::{MARK, System};

struct Yes;

impl System for Yes {
    fn available(&mut self) -> bool {
        true
    }
    fn set(&mut self, _: Backdrop) -> bool {
        true
    }
}

/// An app whose chrome is frosted, as after `sync_frost` in a focused window on Windows 11.
fn frosted_app() -> App {
    let mut app = app();
    app.docs.new_untitled();
    app.ui.panes.files = true;
    app.docs.prefs.backdrop = Backdrop::Mica;
    app.focused = true;
    let pal = if app.dark() {
        theme::DARK
    } else {
        theme::LIGHT
    };
    assert!(app.frost.sync(Backdrop::Mica, true, &pal, &mut Yes));
    app.full = true;
    app
}

/// The alpha byte at device pixel (x, y).
fn alpha(app: &App, x: usize, y: usize) -> u8 {
    app.screen.pixels[(y * app.screen.width as usize + x) * 4 + 3]
}

/// Whether device pixel (x, y) lies in a frosted region of the last frame.
fn in_chrome(app: &App, x: usize, y: usize) -> bool {
    app.frost
        .areas
        .iter()
        .any(|a| x >= a.0 && x < a.2 && y >= a.1 && y < a.3)
}

/// (marked pixels outside the chrome, unmarked pixels inside it, pixels that are neither 255 nor
/// the mark).
fn census(app: &App) -> (usize, usize, usize) {
    let (w, h) = (app.screen.width as usize, app.screen.height as usize);
    let (mut stray, mut opaque_chrome, mut other) = (0, 0, 0);
    for y in 0..h {
        for x in 0..w {
            match (alpha(app, x, y), in_chrome(app, x, y)) {
                (MARK, false) => stray += 1,
                (255, true) => opaque_chrome += 1,
                (MARK, true) | (255, false) => {}
                _ => other += 1,
            }
        }
    }
    (stray, opaque_chrome, other)
}

#[test]
fn the_chrome_is_marked_and_the_text_is_not() {
    for scale in [1.0, 1.5] {
        let mut app = frosted_app();
        app.scale = scale;
        app.compose();
        let frosted_regions = REGIONS
            .iter()
            .filter(|r| frosted(**r) && app.areas.region(**r).is_some())
            .count();
        assert_eq!(app.frost.areas.len(), frosted_regions);
        assert_eq!(frosted_regions, 4, "title, bar, files pane, status");
        assert_eq!(census(&app), (0, 0, 0), "scale {scale}");
        // The text area's own pixels are opaque.
        let t = app.areas.text;
        let (x, y) = ((t.cx() * scale) as usize, (t.cy() * scale) as usize);
        assert_eq!(alpha(&app, x, y), 255);
        // A frame in which only the caret's line is repainted leaves the marks as they are.
        app.ui.caret_on = !app.ui.caret_on;
        app.compose();
        assert_eq!(census(&app), (0, 0, 0));
    }
}

#[test]
fn a_menu_over_the_chrome_is_opaque_and_its_shadow_is_not() {
    let mut app = frosted_app();
    app.compose();
    app.open_popup(PopupKind::Recent);
    app.compose();
    let menu = app
        .ui
        .popup
        .as_ref()
        .expect("the menu is open")
        .root()
        .rect();
    let scale = app.scale;
    let (stray, opaque_chrome, other) = census(&app);
    assert_eq!((stray, other), (0, 0));
    assert!(opaque_chrome > 0, "the menu covers part of the chrome");
    // Every opaque chrome pixel is the menu's, and the middle of the menu is opaque wherever
    // it lies over the chrome.
    let (w, h) = (app.screen.width as usize, app.screen.height as usize);
    let mut inside_marked = 0;
    for y in 0..h {
        for x in 0..w {
            if !in_chrome(&app, x, y) {
                continue;
            }
            let (lx, ly) = ((x as f32 + 0.5) / scale, (y as f32 + 0.5) / scale);
            if alpha(&app, x, y) == 255 {
                assert!(menu.contains(lx, ly), "opaque chrome at {x},{y}");
            } else if menu.inset(6.0).contains(lx, ly) {
                inside_marked += 1;
            }
        }
    }
    assert_eq!(inside_marked, 0);
    // The shadow just outside the menu, where it falls on the chrome, keeps the mark.
    let (sx, sy) = (
        ((menu.x - 3.0) * scale) as usize,
        (menu.cy() * scale) as usize,
    );
    if in_chrome(&app, sx, sy) {
        assert_eq!(alpha(&app, sx, sy), MARK);
    }
    // Closed again: the chrome under it is marked again.
    app.ui.popup = None;
    app.compose();
    assert_eq!(census(&app), (0, 0, 0));
}

#[test]
fn a_dialog_dims_the_chrome_without_making_it_opaque() {
    let mut app = frosted_app();
    app.compose();
    app.open_settings();
    app.compose();
    let (stray, _, other) = census(&app);
    assert_eq!((stray, other), (0, 0));
    // The title row's left end is outside the card: dimmed, still marked.
    assert_eq!(alpha(&app, 4, 4), MARK);
}

#[test]
fn without_the_keyboard_or_the_setting_every_pixel_is_opaque() {
    let all_opaque = |app: &App| {
        app.screen
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| p[3] == 255)
    };
    // Focus lost: what `sync_frost` does when the look changes, then a frame.
    let mut app = frosted_app();
    app.compose();
    assert!(!all_opaque(&app));
    let pal = app.pal;
    assert!(app.frost.sync(Backdrop::Mica, false, &pal, &mut Yes));
    for px in app.screen.pixels.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    app.full = true;
    app.compose();
    assert!(all_opaque(&app));
    assert!(app.frost.areas.is_empty());
    // The setting off from the start: nothing is ever marked.
    let mut app = super::files_tests::app();
    app.docs.new_untitled();
    app.ui.panes.files = true;
    app.focused = true;
    app.compose();
    assert!(all_opaque(&app));
    assert!(app.frost.key().is_none());
}
