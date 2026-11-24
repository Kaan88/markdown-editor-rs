//! Throwaway mockups of the screens of `docs/ai-pictures-plan.md`, drawn by the app's own widgets
//! for `--shot` with `--mock <name>` and never reachable otherwise. Like `spikes/` and the branch
//! `mock/ai-review`, this exists to make pictures and to prove the layouts are reachable with the
//! widgets we have; every word and number in them is hard-coded sample data. Not for `main`.
//!
//! - `window`, `options`, `looking`, `limit`, `failed`, `no-vision`: the AI task window with the
//!   new task "Write notes" and pictures in it (`mock/window.rs`).
//! - `plan`, `plan-one`: the review of the notes the AI proposes, as a tree, and as one note
//!   (`mock/plan.rs`).
//! - `sent`, `undo`, `result`: what is sent, the undo question, and the finished library
//!   (`mock/after.rs`).
//! - `add`, `from-notes`, `file-copy`, `file-move`, `tree-select`: the one way into the window,
//!   the window opened on notes selected in the Notes pane, File them copying and moving the
//!   files with their pictures, and several notes selected in the Notes pane.
//! - `tree-row`, `from-section`: the main way in from the library: one row's own menu (a
//!   section, meaning all its notes), and the window opened from it.

mod after;
mod plan;
mod window;

use mdshared_ui::check::{Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button,
    draw_dialog_button_disabled,
};
use mdshared_ui::theme::Palette;

pub fn draw(p: &mut Painter, pal: &Palette, name: &str, w: f32, h: f32) {
    if !matches!(name, "result" | "tree-select" | "tree-row") {
        draw_backdrop(p, pal, w, h);
    }
    match name {
        "window" => window::draw(p, pal, w, h, window::Face_::Ready),
        "options" => window::draw(p, pal, w, h, window::Face_::Options),
        "looking" => window::draw(p, pal, w, h, window::Face_::Looking),
        "limit" => window::draw(p, pal, w, h, window::Face_::Limit),
        "failed" => window::draw(p, pal, w, h, window::Face_::Failed),
        "no-vision" => window::draw(p, pal, w, h, window::Face_::NoVision),
        "from-notes" => window::draw(p, pal, w, h, window::Face_::FromNotes),
        "add" => window::draw(p, pal, w, h, window::Face_::AddMenu),
        "file-copy" => window::draw(p, pal, w, h, window::Face_::FileCopy),
        "file-move" => window::draw(p, pal, w, h, window::Face_::FileMove),
        "tree-select" => after::tree_select(p, pal, w, h),
        "tree-row" => after::tree_row(p, pal, w, h),
        "from-section" => window::draw(p, pal, w, h, window::Face_::FromSection),
        "plan" => plan::draw(p, pal, w, h, false),
        "plan-one" => plan::draw(p, pal, w, h, true),
        "sent" => after::sent(p, pal, w, h),
        "undo" => after::undo(p, pal, w, h),
        "result" => after::result(p, pal, w, h),
        other => eprintln!("mdedit: no mock named {other}"),
    }
}

// ---- small pieces the screens share ------------------------------------------------------------

pub(crate) fn tick(p: &mut Painter, pal: &Palette, r: Rect, on: bool, enabled: bool) {
    draw_tick(
        p,
        pal,
        r,
        Tick {
            on,
            enabled,
            focused: false,
        },
    );
}

/// Buttons at the right of a dialog footer, `labels` in reading order: (label, primary,
/// enabled). Returns the x the leftmost one starts at.
pub(crate) fn footer_buttons(
    p: &mut Painter,
    pal: &Palette,
    card: Rect,
    footer: Rect,
    labels: &[(&str, bool, bool)],
) -> f32 {
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut x = card.right() - DIALOG_PAD;
    for (label, primary, enabled) in labels.iter().rev() {
        let bw = dialog_button_w(p, label);
        x -= bw;
        let r = Rect::new(x, by, bw, 32.0);
        if *enabled {
            draw_dialog_button(p, pal, r, label, *primary, false, false);
        } else {
            draw_dialog_button_disabled(p, pal, r, label, *primary);
        }
        x -= 10.0;
    }
    x + 10.0
}

/// A text link in a footer, its right edge at `right`. Returns its left edge.
pub(crate) fn link(p: &mut Painter, pal: &Palette, right: f32, cy: f32, text: &str) -> f32 {
    let w = p.measure(Face::Ui, 12.5, text);
    p.text(
        Face::Ui,
        12.5,
        right - w,
        cy,
        text,
        mix(pal.accent, pal.text, 0.25),
        w + 4.0,
    );
    right - w
}

pub(crate) fn hline(p: &mut Painter, x: f32, y: f32, w: f32, c: Rgb) {
    let px = p.px();
    p.fill(Rect::new(x, y, w, px), c);
}

pub(crate) fn vline(p: &mut Painter, x: f32, y: f32, h: f32, c: Rgb) {
    let px = p.px();
    p.fill(Rect::new(x, y, px, h), c);
}

/// A small caps label as the dialogs have them ("SAVED PROMPTS").
pub(crate) fn caption(p: &mut Painter, pal: &Palette, x: f32, cy: f32, text: &str, max: f32) {
    p.text(Face::Semibold, 10.5, x, cy, text, pal.muted, max);
}

/// A rounded label, its left edge at `x`. Returns its right edge.
pub(crate) fn chip(p: &mut Painter, x: f32, cy: f32, text: &str, fill: Rgb, fg: Rgb) -> f32 {
    let w = p.measure(Face::Ui, 11.0, text) + 14.0;
    let r = Rect::new(x, cy - 9.0, w, 18.0);
    p.rbox(r, 9.0, fill);
    p.text(Face::Ui, 11.0, x + 7.0, cy, text, fg, w);
    r.right()
}

/// What a sample picture shows, for [`thumb`]. The real thumbnails come from the engine's
/// `prepare` answer; these are drawn so no picture file is needed for a shot.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Art {
    /// A screenshot of a window with an error box in it.
    Error,
    /// A screenshot of a settings page: a side list and rows of switches.
    Settings,
    /// A screenshot of a terminal.
    Terminal,
    /// A screenshot of a chart.
    Chart,
    /// A photograph.
    Photo,
}

/// A stand-in for a picture's thumbnail, filling `r`.
pub(crate) fn thumb(p: &mut Painter, pal: &Palette, r: Rect, art: Art) {
    let old = p.clip(r);
    let u = r.w / 48.0;
    let paper: Rgb = [0xf3, 0xf4, 0xf6];
    let ink: Rgb = [0xb8, 0xbd, 0xc6];
    match art {
        Art::Error => {
            p.fill(r, paper);
            p.fill(Rect::new(r.x, r.y, r.w, 6.0 * u), [0xd9, 0xdc, 0xe2]);
            let d = Rect::new(r.x + 9.0 * u, r.y + 11.0 * u, 30.0 * u, 18.0 * u);
            p.rbox(d, 2.0 * u, [0xff, 0xff, 0xff]);
            p.fill(Rect::new(d.x, d.y, d.w, 4.0 * u), [0xc9, 0x3a, 0x3a]);
            p.fill(
                Rect::new(d.x + 3.0 * u, d.y + 7.0 * u, 20.0 * u, 1.6 * u),
                ink,
            );
            p.fill(
                Rect::new(d.x + 3.0 * u, d.y + 10.5 * u, 14.0 * u, 1.6 * u),
                ink,
            );
            p.rbox(
                Rect::new(d.right() - 10.0 * u, d.bottom() - 5.0 * u, 8.0 * u, 3.4 * u),
                u,
                [0x2f, 0x6f, 0xd0],
            );
        }
        Art::Settings => {
            p.fill(r, paper);
            p.fill(Rect::new(r.x, r.y, 13.0 * u, r.h), [0xe2, 0xe5, 0xea]);
            for k in 0..4 {
                let y = r.y + (5.0 + 7.0 * k as f32) * u;
                p.fill(Rect::new(r.x + 2.5 * u, y, 8.0 * u, 1.6 * u), ink);
                p.fill(Rect::new(r.x + 17.0 * u, y, 15.0 * u, 1.6 * u), ink);
                p.rbox(
                    Rect::new(r.right() - 10.0 * u, y - 1.0 * u, 6.5 * u, 3.6 * u),
                    1.8 * u,
                    if k % 2 == 0 {
                        [0x2f, 0x6f, 0xd0]
                    } else {
                        [0xc4, 0xc8, 0xd0]
                    },
                );
            }
        }
        Art::Terminal => {
            p.fill(r, [0x1b, 0x1f, 0x27]);
            p.fill(Rect::new(r.x, r.y, r.w, 5.0 * u), [0x2c, 0x32, 0x3d]);
            let widths = [26.0, 34.0, 18.0, 30.0, 12.0];
            for (k, w) in widths.iter().enumerate() {
                p.fill(
                    Rect::new(
                        r.x + 3.0 * u,
                        r.y + (9.0 + 5.0 * k as f32) * u,
                        w * u,
                        1.6 * u,
                    ),
                    if k == 3 {
                        [0xe0, 0x6c, 0x6c]
                    } else {
                        [0x8f, 0xd1, 0x9a]
                    },
                );
            }
        }
        Art::Chart => {
            p.fill(r, [0xff, 0xff, 0xff]);
            let base = r.bottom() - 5.0 * u;
            p.fill(Rect::new(r.x + 5.0 * u, base, r.w - 10.0 * u, u), ink);
            let bars = [9.0, 15.0, 12.0, 21.0, 17.0, 25.0];
            for (k, bh) in bars.iter().enumerate() {
                p.fill(
                    Rect::new(
                        r.x + (7.0 + 6.0 * k as f32) * u,
                        base - bh * u,
                        4.0 * u,
                        bh * u,
                    ),
                    if k == 5 {
                        [0xe0, 0x8a, 0x2a]
                    } else {
                        [0x4f, 0x8b, 0xd9]
                    },
                );
            }
        }
        Art::Photo => {
            p.fill(r, [0xa9, 0xcf, 0xee]);
            p.rbox(
                Rect::new(r.x + 31.0 * u, r.y + 5.0 * u, 8.0 * u, 8.0 * u),
                4.0 * u,
                [0xff, 0xe2, 0x8a],
            );
            p.polygon(
                &[
                    (r.x, r.bottom()),
                    (r.x, r.y + 0.62 * r.h),
                    (r.x + 0.3 * r.w, r.y + 0.42 * r.h),
                    (r.x + 0.55 * r.w, r.y + 0.66 * r.h),
                    (r.x + 0.78 * r.w, r.y + 0.5 * r.h),
                    (r.right(), r.y + 0.7 * r.h),
                    (r.right(), r.bottom()),
                ],
                [0x5d, 0x8f, 0x5a],
            );
        }
    }
    p.restore(old);
    let px = p.px();
    let edge = mix(pal.popup, pal.text, 0.22);
    p.fill(Rect::new(r.x, r.y, r.w, px), edge);
    p.fill(Rect::new(r.x, r.bottom() - px, r.w, px), edge);
    p.fill(Rect::new(r.x, r.y, px, r.h), edge);
    p.fill(Rect::new(r.right() - px, r.y, px, r.h), edge);
}
