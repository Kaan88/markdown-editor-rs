//! The two screens that now draw their tick boxes and their two-way switch with the shared
//! widgets: the Replace in files confirmation and the AI review. The screen is composed for real,
//! and the widget's own area is compared, pixel for pixel, with the same painting made by the
//! private code that used to sit in `replace_ui` and `ai_ui` (kept here as the reference). The
//! comparison is on the screen, so a widget that draws a pixel elsewhere, or vanishes, fails.

use super::files_tests::{app, open_now, scratch};
use super::*;
use crate::ai::{Hit as AiHit, Panel};
use mdshared_ui::gfx::{Face, mix};
use mdshared_ui::glyph;

/// `replace_ui::tick_box` as the confirmation drew it before the shared tick box: the shape its
/// two states must keep.
fn before_tick(p: &mut Painter, pal: &Palette, r: Rect, on: bool) {
    if on {
        p.rbox(r, 4.0, pal.accent);
        p.icon(glyph::CHECK, 11.0, r.cx(), r.cy(), pal.on_accent);
    } else {
        p.frame(r, 4.0, pal.field, mix(pal.line, pal.text, 0.3));
    }
}

/// The "Changes / Preview" switch as `ai_ui::review` drew it inline before the shared segmented
/// switch: the shape its two choices must keep.
fn before_switch(p: &mut Painter, pal: &Palette, right: f32, y: f32, preview: bool, hovered: bool) {
    let seg_w = |p: &mut Painter, s: &str| p.measure(Face::Semibold, 13.0, s) + 28.0;
    let (cw, pw) = (seg_w(p, "Changes"), seg_w(p, "Preview"));
    let sw = Rect::new(right - cw - pw, y, cw + pw, 28.0);
    p.frame(sw, 6.0, pal.popup, pal.popup_line);
    for (k, (label, on)) in [("Changes", !preview), ("Preview", preview)]
        .into_iter()
        .enumerate()
    {
        let seg = if k == 0 {
            Rect::new(sw.x, sw.y, cw, sw.h)
        } else {
            Rect::new(sw.x + cw, sw.y, pw, sw.h)
        };
        if on {
            p.rbox(seg.inset(1.0), 5.0, pal.accent);
            p.text_center(Face::Semibold, 13.0, seg, label, pal.on_accent);
        } else {
            // The pointer rests on the segment that is not chosen, never on the chosen one.
            if hovered {
                p.rbox(seg.inset(1.0), 5.0, pal.hover);
            }
            p.text_center(Face::Ui, 13.0, seg, label, pal.text);
        }
    }
}

/// Every pixel of the screen in `area` (grown by `margin`) equals the same painting on a popup
/// background, and that area is not blank.
fn same_area(
    screen: &Bitmap,
    pal: &Palette,
    area: Rect,
    margin: f32,
    draw: impl FnOnce(&mut Painter<'_>),
) {
    let mut want = Bitmap::new(screen.width, screen.height);
    let mut gfx = Gfx::new();
    {
        let mut p = Painter::new(&mut want, &mut gfx, 1.0);
        p.fill(
            Rect::new(0.0, 0.0, screen.width as f32, screen.height as f32),
            pal.popup,
        );
        draw(&mut p);
    }
    let bg = want.pixels[0..3].to_vec();
    let x0 = (area.x - margin).floor().max(0.0) as u32;
    let y0 = (area.y - margin).floor().max(0.0) as u32;
    let x1 = ((area.right() + margin).ceil() as u32).min(screen.width);
    let y1 = ((area.bottom() + margin).ceil() as u32).min(screen.height);
    let mut painted = false;
    for y in y0..y1 {
        for x in x0..x1 {
            let o = ((y * screen.width + x) * 4) as usize;
            assert_eq!(
                &screen.pixels[o..o + 3],
                &want.pixels[o..o + 3],
                "pixel ({x}, {y}) of {area:?}"
            );
            painted |= want.pixels[o..o + 3] != bg;
        }
    }
    assert!(painted, "nothing was drawn in {area:?}");
}

/// Waits for the replace workers, as the event loop's wakes do.
fn pump(a: &mut App, until: fn(&App) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !until(a) && Instant::now() < deadline {
        a.ui.results.poll();
        a.replace_poll();
        a.docs
            .wait_load(Instant::now() + Duration::from_millis(5), None, false);
    }
    assert!(until(a), "timed out");
}

/// The confirmation of a replace in files over three files, open and composed as the window
/// draws it.
fn confirmated(tag: &str) -> (App, PathBuf) {
    let dir = scratch(tag);
    for n in ["a.txt", "b.txt", "c.txt"] {
        std::fs::write(dir.join(n), "one foo two\nfoo\n").unwrap();
    }
    let mut a = app();
    a.set_folder(dir.clone());
    open_now(&mut a, &dir.join("a.txt"));
    a.ui.find.set_text("foo");
    a.docs.find_set("foo", a.ui.options());
    a.find_in_files();
    pump(&mut a, |a| {
        matches!(a.ui.results.state, crate::results::State::Done { .. })
    });
    a.ui.results.replace.field.set_text("bar");
    a.replace_all_in_files();
    pump(&mut a, |a| a.ui.results.replace.confirm.is_some());
    (a, dir)
}

/// A tick box of the confirmation's first list row and the backup option's, as the code that
/// draws them places them: the row's hit keeps the pointer, the box sits 10 px in from its
/// left edge and is `TICK` px square, centred on the row; the option's box is 2 px in from the
/// options rectangle's left edge, which is where its hit starts.
fn confirmation_ticks(a: &App) -> [Rect; 2] {
    let row = a
        .hit_rect(Hit::DlgOpen(1))
        .expect("the second file's row takes the pointer");
    let option = a
        .hit_rect(Hit::DlgButton(2))
        .expect("the backup option takes the pointer");
    let tick = mdshared_ui::check::TICK;
    [
        Rect::new(row.x + 10.0, row.cy() - tick / 2.0, tick, tick),
        Rect::new(option.x + 2.0, option.cy() - tick / 2.0, tick, tick),
    ]
}

#[test]
fn the_replace_confirmation_draws_its_tick_boxes_as_it_did_by_hand() {
    let (mut a, dir) = confirmated("pixeltick");
    for on in [false, true] {
        {
            let c = a.ui.results.replace.confirm.as_mut().expect("the plan");
            // The second row: the first carries the keyboard's quiet fill behind it.
            c.ticks[1] = on;
            c.backup = on;
        }
        a.compose();
        let pal = a.pal;
        for tick in confirmation_ticks(&a) {
            same_area(&a.screen, &pal, tick, 3.0, |p| {
                before_tick(p, &pal, tick, on)
            });
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The AI review with a result, composed as the window draws it.
fn reviewed(tag: &str) -> (App, PathBuf) {
    let dir = scratch(tag);
    std::fs::write(dir.join("a.md"), "plain words\nand more\n").unwrap();
    let mut a = app();
    open_now(&mut a, &dir.join("a.md"));
    a.ai_demo("review");
    assert!(
        matches!(a.ai.panel, Some(Panel::Review(_))),
        "a review is open"
    );
    (a, dir)
}

fn segment(a: &App, hit: AiHit) -> Rect {
    a.ai.hits
        .iter()
        .find(|(h, _)| *h == hit)
        .map(|(_, r)| *r)
        .expect("the switch's segment takes the pointer")
}

#[test]
fn the_review_draws_its_changes_and_preview_switch_as_it_did_by_hand() {
    let (mut a, dir) = reviewed("pixelswitch");
    for preview in [false, true] {
        // No hover, then the pointer on the segment that is not chosen: the hover look.
        for hovered in [false, true] {
            a.ai.review().expect("the review").preview = preview;
            a.ai.hover = hovered.then_some(if preview {
                AiHit::Changes
            } else {
                AiHit::Preview
            });
            a.compose();
            let pal = a.pal;
            let changes = segment(&a, AiHit::Changes);
            let previews = segment(&a, AiHit::Preview);
            let sw = Rect::new(
                changes.x,
                changes.y,
                previews.right() - changes.x,
                changes.h,
            );
            assert_eq!((sw.y, sw.h), (previews.y, previews.h));
            same_area(&a.screen, &pal, sw, 3.0, |p| {
                before_switch(p, &pal, sw.right(), sw.y, preview, hovered)
            });
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
