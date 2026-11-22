//! Small drawing helpers the window and the dialogs share: wrapped text, buttons, tiles,
//! progress bars, fields, check boxes.

use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

pub fn wrap(p: &mut Painter, face: Face, size: f32, text: &str, w: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let t = if cur.is_empty() {
            word.to_string()
        } else {
            format!("{cur} {word}")
        };
        if !cur.is_empty() && p.measure(face, size, &t) > w {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        } else {
            cur = t;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// A paragraph; returns the y below it.
#[allow(clippy::too_many_arguments)]
pub fn para(
    p: &mut Painter,
    face: Face,
    size: f32,
    x: f32,
    y: f32,
    w: f32,
    text: &str,
    color: Rgb,
) -> f32 {
    let lh = (size * 1.45).round();
    let mut y = y;
    for l in wrap(p, face, size, text, w) {
        p.text(face, size, x, y + lh / 2.0, &l, color, w + 4.0);
        y += lh;
    }
    y
}

/// A 28 px button at `x`, centred on `cy`; returns its rectangle.
pub fn sbtn(
    p: &mut Painter,
    pal: &Palette,
    x: f32,
    cy: f32,
    label: &str,
    primary: bool,
    enabled: bool,
) -> Rect {
    let w = (p.measure(Face::Semibold, 12.5, label) + 26.0).round();
    let r = Rect::new(x, cy - 14.0, w, 28.0);
    if primary && enabled {
        p.rbox(r, 5.0, pal.accent);
        p.text_center(Face::Semibold, 12.5, r, label, pal.on_accent);
    } else {
        p.frame(r, 5.0, mix(pal.page, pal.text, 0.04), pal.line);
        p.text_center(
            Face::Ui,
            12.5,
            r,
            label,
            if enabled { pal.text } else { pal.faint },
        );
    }
    r
}

/// Same, ending at `right`.
pub fn sbtn_r(
    p: &mut Painter,
    pal: &Palette,
    right: f32,
    cy: f32,
    label: &str,
    primary: bool,
    enabled: bool,
) -> Rect {
    let w = (p.measure(Face::Semibold, 12.5, label) + 26.0).round();
    sbtn(p, pal, right - w, cy, label, primary, enabled)
}

pub fn ibtn(p: &mut Painter, pal: &Palette, cx: f32, cy: f32, g: &str) {
    p.icon(g, 15.0, cx, cy, pal.muted);
}

pub fn chip(p: &mut Painter, pal: &Palette, x: f32, cy: f32, label: &str, c: Rgb) -> f32 {
    let w = p.measure(Face::Semibold, 11.0, label) + 14.0;
    let r = Rect::new(x, cy - 9.0, w, 18.0);
    p.rbox(r, 9.0, mix(pal.page, c, 0.20));
    p.text_center(Face::Semibold, 11.0, r, label, c);
    w
}

pub fn chip_on(p: &mut Painter, bg: Rgb, x: f32, cy: f32, label: &str, c: Rgb) -> f32 {
    let w = p.measure(Face::Semibold, 11.0, label) + 14.0;
    let r = Rect::new(x, cy - 9.0, w, 18.0);
    p.rbox(r, 9.0, mix(bg, c, 0.20));
    p.text_center(Face::Semibold, 11.0, r, label, c);
    w
}

pub fn progress(p: &mut Painter, pal: &Palette, r: Rect, f: f32, c: Rgb, bg: Rgb) {
    p.rbox(r, r.h / 2.0, mix(bg, pal.text, 0.14));
    if f > 0.0 {
        p.rbox(Rect::new(r.x, r.y, (r.w * f).max(r.h), r.h), r.h / 2.0, c);
    }
}

pub fn field(p: &mut Painter, pal: &Palette, r: Rect, text: &str, mono: bool, focus: bool) {
    p.frame(r, 6.0, pal.field, pal.line);
    if focus {
        p.fill(
            Rect::new(r.x + 4.0, r.bottom() - 2.0, r.w - 8.0, 2.0),
            pal.accent,
        );
    }
    p.text(
        if mono { Face::Mono } else { Face::Ui },
        if mono { 12.0 } else { 13.0 },
        r.x + 11.0,
        r.cy(),
        text,
        pal.text,
        r.w - 22.0,
    );
}

pub fn check(p: &mut Painter, pal: &Palette, x: f32, cy: f32, on: bool, label: &str, max: f32) {
    let b = Rect::new(x, cy - 8.0, 16.0, 16.0);
    if on {
        p.rbox(b, 4.0, pal.accent);
        p.icon(glyph::CHECK, 12.0, b.cx(), b.cy(), pal.on_accent);
    } else {
        p.frame(b, 4.0, pal.field, pal.muted);
    }
    p.text(Face::Ui, 13.0, x + 26.0, cy, label, pal.text, max);
}

pub fn radio(p: &mut Painter, pal: &Palette, x: f32, cy: f32, on: bool) {
    let b = Rect::new(x, cy - 8.0, 16.0, 16.0);
    if on {
        p.rbox(b, 8.0, pal.accent);
        p.rbox(
            Rect::new(b.x + 5.0, b.y + 5.0, 6.0, 6.0),
            3.0,
            pal.on_accent,
        );
    } else {
        p.frame(b, 8.0, pal.field, pal.muted);
    }
}

/// `label` muted at `x`, `value` at `x + lw`; returns the y below.
#[allow(clippy::too_many_arguments)]
pub fn kv(
    p: &mut Painter,
    pal: &Palette,
    x: f32,
    y: f32,
    lw: f32,
    w: f32,
    label: &str,
    value: &str,
    mono: bool,
) -> f32 {
    p.text(Face::Ui, 12.0, x, y + 9.0, label, pal.muted, lw);
    let f = if mono { Face::Mono } else { Face::Ui };
    let s = if mono { 11.5 } else { 12.5 };
    let lines = wrap(p, f, s, value, w - lw);
    let mut yy = y;
    for l in &lines {
        p.text(f, s, x + lw, yy + 9.0, l, pal.text, w - lw + 4.0);
        yy += 18.0;
    }
    yy.max(y + 18.0) + 4.0
}
