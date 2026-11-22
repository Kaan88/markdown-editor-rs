//! The icon views of the Hub window: the same apps, states and actions as the list, as a grid of
//! tiles. Large tiles carry the version, the status and the one action of the row; small tiles carry
//! the icon, the name and one word, and leave the rest to the pane under the grid and the row menu.
//! The view switch is in the command bar (`win::bar`), and Settings > General keeps the choice.

use super::data::{APPS, St, amber, blue, green, red};
use super::ui;
use super::win::{self, HEAD_H, Scene, View};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

pub const LIST: &str = glyph::OUTLINE;
pub const LARGE: &str =
    "M2.5 2.5h4.5v4.5h-4.5zM9 2.5h4.5v4.5H9zM2.5 9h4.5v4.5h-4.5zM9 9h4.5v4.5H9z";
pub const SMALL: &str = "M2 2h3v3H2zM6.5 2h3v3h-3zM11 2h3v3h-3zM2 6.5h3v3H2zM6.5 6.5h3v3h-3zM11 6.5h3v3h-3zM2 11h3v3H2zM6.5 11h3v3h-3zM11 11h3v3h-3z";

pub const MARGIN: f32 = 16.0;
pub const GAP: f32 = 8.0;
pub const PAD: f32 = 12.0;
pub const LARGE_MIN_W: f32 = 164.0;
pub const LARGE_H: f32 = 196.0;
pub const SMALL_MIN_W: f32 = 108.0;
pub const SMALL_H: f32 = 104.0;

/// Columns, the width of a tile and its height, at window width `w`.
pub fn geometry(view: View, w: f32) -> (usize, f32, f32) {
    let (min, h) = match view {
        View::Small => (SMALL_MIN_W, SMALL_H),
        _ => (LARGE_MIN_W, LARGE_H),
    };
    let avail = w - 2.0 * MARGIN;
    let cols = (((avail + GAP) / (min + GAP)).floor() as usize).max(1);
    let cw = ((avail - GAP * (cols as f32 - 1.0)) / cols as f32).floor();
    (cols, cw, h)
}

/// The height the grid wants for `n` apps.
pub fn height(view: View, w: f32, n: usize) -> f32 {
    let (cols, _, h) = geometry(view, w);
    let rows = n.div_ceil(cols);
    HEAD_H + PAD + rows as f32 * (h + GAP) - GAP + PAD
}

/// The one action a tile shows: the first button of the row in the list.
fn action(sc: &Scene, i: usize) -> (&'static str, bool, bool) {
    match sc.st[i] {
        St::Update if sc.running[i] => ("Update\u{2026}", true, true),
        St::Update => ("Update", true, true),
        St::UpToDate | St::Done(_) | St::Pkg => ("Open", false, true),
        St::NotInstalled => ("Install", false, true),
        St::External => ("Adopt", false, true),
        St::Downloading(..) | St::Verifying | St::Waiting(_) => ("Cancel", false, true),
        St::Failed(_) => ("Try again", true, true),
        St::Restart => ("Restart Hub", true, true),
        St::NoBuild => ("Install", false, false),
    }
}

/// The one word under a small tile, and its colour.
fn word(pal: &Palette, sc: &Scene, i: usize) -> (&'static str, Rgb) {
    match sc.st[i] {
        St::UpToDate => (sc.inst[i].unwrap_or(""), pal.muted),
        St::Update => ("Update", blue(pal)),
        St::NotInstalled => ("Not installed", pal.faint),
        St::External => ("Not by Hub", pal.muted),
        St::Downloading(..) => ("", pal.muted),
        St::Verifying => ("Checking", pal.accent),
        St::Waiting(_) => ("Waiting", amber(pal)),
        St::Failed(_) => ("Not updated", red(pal)),
        St::Restart => ("Restart", green(pal)),
        St::Done(_) => ("Updated", green(pal)),
        St::NoBuild => ("No build", pal.faint),
        St::Pkg => ("apt", pal.muted),
    }
}

/// The badges on a tile: an update (blue, an arrow), a failure (red, !), and running (a green
/// dot at the foot of the icon). `bg` is the colour behind the tile, for the ring.
fn badges(p: &mut Painter, pal: &Palette, t: Rect, sc: &Scene, i: usize, bg: Rgb) {
    let d = (t.w * 0.36).clamp(14.0, 20.0);
    let ring = |p: &mut Painter, cx: f32, cy: f32, d: f32, c: Rgb| {
        let o = Rect::new(cx - d / 2.0 - 2.0, cy - d / 2.0 - 2.0, d + 4.0, d + 4.0);
        p.rbox(o, o.w / 2.0, bg);
        p.rbox(Rect::new(cx - d / 2.0, cy - d / 2.0, d, d), d / 2.0, c);
    };
    let (cx, cy) = (t.right() - 2.0, t.y + 2.0);
    let newer = matches!(
        sc.st[i],
        St::Update | St::Downloading(..) | St::Verifying | St::Waiting(_)
    );
    if newer {
        ring(p, cx, cy, d, pal.accent);
        p.icon(glyph::DOWNLOAD, d * 0.62, cx, cy, pal.on_accent);
    } else if matches!(sc.st[i], St::Failed(_)) {
        ring(p, cx, cy, d, red(pal));
        p.text_center(
            Face::Semibold,
            d * 0.7,
            Rect::new(cx - d / 2.0, cy - d / 2.0, d, d),
            "!",
            [255, 255, 255],
        );
    } else if matches!(sc.st[i], St::Restart | St::Done(_)) {
        ring(p, cx, cy, d, green(pal));
        p.icon(glyph::CHECK, d * 0.62, cx, cy, [255, 255, 255]);
    }
    if sc.running[i] {
        ring(p, t.right() - 2.0, t.bottom() - 2.0, d * 0.6, green(pal));
    }
}

pub fn draw(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, rs: &[usize]) -> Vec<Rect> {
    p.fill(r, pal.page);
    let px = p.px().max(1.0);
    let old = p.clip(r);
    // The strip above the tiles, in the place of the list's column heads.
    let head = Rect::new(r.x, r.y, r.w, HEAD_H);
    p.fill(head, pal.pane);
    p.fill(Rect::new(r.x, head.bottom() - px, r.w, px), pal.line);
    let n_upd = rs
        .iter()
        .filter(|&&i| {
            matches!(
                sc.st[i],
                St::Update | St::Downloading(..) | St::Verifying | St::Waiting(_) | St::Failed(_)
            )
        })
        .count();
    let left = format!(
        "{} apps \u{b7} {} installed \u{b7} {} with an update",
        rs.len(),
        rs.iter().filter(|&&i| sc.inst[i].is_some()).count(),
        n_upd
    );
    p.text(
        Face::Semibold,
        12.0,
        r.x + 14.0,
        head.cy(),
        &left,
        pal.muted,
        r.w * 0.6,
    );
    p.text_right(
        Face::Ui,
        12.0,
        r.right() - 30.0,
        head.cy(),
        "Sort: newest release",
        pal.muted,
    );
    p.icon(glyph::DOWN, 9.0, r.right() - 20.0, head.cy(), pal.muted);

    let (cols, cw, ch) = geometry(sc.view, r.w);
    let mut out = Vec::new();
    for (k, &i) in rs.iter().enumerate() {
        let (col, row) = (k % cols, k / cols);
        let c = Rect::new(
            r.x + MARGIN + col as f32 * (cw + GAP),
            head.bottom() + PAD + row as f32 * (ch + GAP),
            cw,
            ch,
        );
        out.push(c);
        if c.y > r.bottom() {
            continue;
        }
        match sc.view {
            View::Small => small(p, pal, c, sc, i),
            _ => large(p, pal, c, sc, i),
        }
    }
    p.restore(old);
    out
}

/// The tile's ground: selected, hovered, failed, or plain; returns the colour behind its content.
fn ground(p: &mut Painter, pal: &Palette, c: Rect, sc: &Scene, i: usize, framed: bool) -> Rgb {
    let sel = sc.sel == i;
    let failed = matches!(sc.st[i], St::Failed(_));
    let base = if sel {
        pal.on
    } else if sc.hover == Some(i) {
        pal.hover
    } else if framed {
        mix(pal.page, pal.text, 0.025)
    } else {
        pal.page
    };
    let bg = if failed {
        mix(base, red(pal), if pal.dark { 0.12 } else { 0.08 })
    } else {
        base
    };
    if sel {
        p.frame(c, 8.0, bg, pal.accent);
    } else if framed {
        p.frame(c, 8.0, bg, mix(pal.line, pal.page, 0.3));
    } else if bg != pal.page {
        p.rbox(c, 8.0, bg);
    }
    bg
}

fn large(p: &mut Painter, pal: &Palette, c: Rect, sc: &Scene, i: usize) {
    let app = &APPS[i];
    let st = sc.st[i];
    let bg = ground(p, pal, c, sc, i, true);
    let dim = matches!(st, St::NoBuild);
    let not_in = matches!(st, St::NotInstalled | St::NoBuild);
    let cx = c.cx();
    // The icon.
    let t = Rect::new(cx - 28.0, c.y + 16.0, 56.0, 56.0);
    ui::tile(p, t, app, dim || not_in);
    badges(p, pal, t, sc, i, bg);
    p.icon(glyph::MORE, 15.0, c.right() - 16.0, c.y + 16.0, pal.muted);
    // Name and blurb.
    let tw = c.w - 20.0;
    let line = |y: f32| Rect::new(c.x + 10.0, y - 10.0, tw, 20.0);
    p.text_center(
        Face::Semibold,
        13.5,
        line(c.y + 88.0),
        app.name,
        if dim { pal.faint } else { pal.text },
    );
    p.text_center(
        Face::Ui,
        11.5,
        line(c.y + 105.0),
        app.blurb,
        if dim { pal.faint } else { pal.muted },
    );
    // Versions, or the progress bar while it downloads.
    let vy = c.y + 126.0;
    match st {
        St::Downloading(f, _) => {
            ui::progress(
                p,
                pal,
                Rect::new(c.x + 22.0, vy - 3.0, c.w - 44.0, 6.0),
                f,
                pal.accent,
                bg,
            );
        }
        St::Verifying => {
            ui::progress(
                p,
                pal,
                Rect::new(c.x + 22.0, vy - 3.0, c.w - 44.0, 6.0),
                1.0,
                mix(pal.accent, pal.page, 0.45),
                bg,
            );
        }
        _ => {
            let newer = matches!(st, St::Update | St::Waiting(_) | St::Failed(_));
            let (a, b) = match (sc.inst[i], newer) {
                (Some(v), true) => (v.to_string(), Some(app.latest)),
                (Some(v), false) => (v.to_string(), None),
                (None, _) if dim => ("no Linux build".to_string(), None),
                (None, _) => (
                    format!(
                        "{} \u{b7} {}",
                        app.latest,
                        if sc.linux { app.linux_size } else { app.setup }
                    ),
                    None,
                ),
            };
            let face = if sc.inst[i].is_some() {
                Face::Mono
            } else {
                Face::Ui
            };
            let aw = p.measure(face, 12.0, &a);
            let arrow = " \u{2192} ";
            let bw = b.map_or(0.0, |b| {
                p.measure(Face::Ui, 12.0, arrow) + p.measure(Face::Semibold, 12.0, b)
            });
            let mut x = cx - (aw + bw) / 2.0;
            x += p.text(face, 12.0, x, vy, &a, pal.muted, tw);
            if let Some(b) = b {
                x += p.text(Face::Ui, 12.0, x, vy, arrow, pal.faint, 40.0);
                p.text(Face::Semibold, 12.0, x, vy, b, blue(pal), 80.0);
            }
        }
    }
    // The status line.
    let s = win::status_of(pal, sc, i, true);
    let sy = c.y + 146.0;
    let (text, col) = match st {
        // The tile has no room for a second line, so the reason comes first.
        St::Waiting(_) => ("Waiting: it is open".to_string(), pal.text),
        St::Failed(why) => (why.to_string(), red(pal)),
        St::Update if sc.running[i] => ("Open now: asks it to close".to_string(), pal.text),
        _ => (s.line.clone(), s.line_col),
    };
    let lead = if s.dot.is_some() { 13.0 } else { 0.0 };
    let w = p.measure(Face::Ui, 12.0, &text).min(tw - lead);
    let x0 = cx - (w + lead) / 2.0;
    if let Some(d) = s.dot {
        win::dot(p, x0, sy, d);
    }
    p.text(Face::Ui, 12.0, x0 + lead, sy, &text, col, tw - lead);
    // The action.
    let (label, prim, en) = action(sc, i);
    let bw = (p.measure(Face::Semibold, 12.5, label) + 26.0).round();
    ui::sbtn(
        p,
        pal,
        (cx - bw / 2.0).round(),
        c.y + 174.0,
        label,
        prim,
        en,
    );
}

fn small(p: &mut Painter, pal: &Palette, c: Rect, sc: &Scene, i: usize) {
    let app = &APPS[i];
    let st = sc.st[i];
    let bg = ground(p, pal, c, sc, i, false);
    let dim = matches!(st, St::NoBuild | St::NotInstalled);
    let cx = c.cx();
    let t = Rect::new(cx - 22.0, c.y + 12.0, 44.0, 44.0);
    ui::tile(p, t, app, dim);
    badges(p, pal, t, sc, i, bg);
    let tw = c.w - 12.0;
    let line = |y: f32| Rect::new(c.x + 6.0, y - 9.0, tw, 18.0);
    p.text_center(
        Face::Semibold,
        12.5,
        line(c.y + 70.0),
        app.name,
        if matches!(st, St::NoBuild) {
            pal.faint
        } else {
            pal.text
        },
    );
    match st {
        St::Downloading(f, _) => ui::progress(
            p,
            pal,
            Rect::new(c.x + 18.0, c.y + 84.0, c.w - 36.0, 5.0),
            f,
            pal.accent,
            bg,
        ),
        _ => {
            let (wd, col) = word(pal, sc, i);
            let face = if matches!(st, St::UpToDate) {
                Face::Mono
            } else {
                Face::Ui
            };
            p.text_center(face, 11.5, line(c.y + 87.0), wd, col);
        }
    }
}
