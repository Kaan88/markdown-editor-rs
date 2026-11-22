//! The Hub window: the title row with its tabs, the command bar, the list of apps, the pane under
//! it for the selected app, an optional bar between the two, and the status bar. Every number is a
//! constant, so `report` prints what the picture holds.

use super::data::{self, APPS, St, amber, blue, green, red};
use super::ui;
use mdshared_ui::chrome::{
    BAR_H, DRAG_MIN, TABS_LEFT, TITLE_H, TabState, WIN_BTN_W, WinButton, button_fill, draw_tab,
    draw_win_button, tab_natural_width, tab_widths, win_button_rect,
};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::notice::{OfferBarState, draw_offer_bar};
use mdshared_ui::theme::Palette;

pub const STATUS_H: f32 = 26.0;
pub const ROW_H: f32 = 52.0;
pub const HEAD_H: f32 = 28.0;
pub const BANNER_H: f32 = 34.0;
pub const PANE_TAB_H: f32 = 34.0;
pub const PANE_MIN: f32 = 150.0;
/// Under this width the Released and Download columns go.
pub const COMPACT_W: f32 = 1000.0;

#[derive(Clone, Copy, PartialEq)]
pub enum Bar {
    Updates(usize),
    Busy(&'static str),
    Quiet,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Pane {
    Notes,
    Steps,
    Failure,
    NoBuild,
    HubSelf,
    None,
}

/// How the apps are shown: the list, or a grid of large or small icons.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum View {
    #[default]
    List,
    Large,
    Small,
}

#[derive(Clone)]
pub struct Banner {
    pub text: &'static str,
    pub yes: &'static str,
    pub no: &'static str,
    pub warn: bool,
}

#[derive(Clone)]
pub struct Scene {
    pub st: [St; 8],
    pub inst: [Option<&'static str>; 8],
    pub running: [bool; 8],
    pub sel: usize,
    pub tab: usize,
    pub pane: Pane,
    pub banner: Option<Banner>,
    pub hub_row: bool,
    pub linux: bool,
    pub bar: Bar,
    pub hover: Option<usize>,
    pub checked: &'static str,
    pub view: View,
}

impl Default for Scene {
    fn default() -> Scene {
        Scene {
            st: [
                St::Update,
                St::UpToDate,
                St::Update,
                St::External,
                St::NotInstalled,
                St::Update,
                St::NotInstalled,
                St::UpToDate,
            ],
            inst: [
                Some("1.2.0"),
                Some("1.0.4"),
                Some("1.0.3"),
                Some("1.0.10"),
                None,
                Some("0.8.2"),
                None,
                Some("0.4.0"),
            ],
            running: [false, false, false, false, false, true, false, false],
            sel: data::MDEDIT,
            tab: 0,
            pane: Pane::Notes,
            banner: None,
            hub_row: false,
            linux: false,
            bar: Bar::Updates(3),
            hover: None,
            checked: "Checked 4 minutes ago",
            view: View::List,
        }
    }
}

pub struct Cols {
    pub installed: f32,
    pub latest: f32,
    pub released: f32,
    pub size: f32,
    pub status: f32,
    pub actions: f32,
}

pub fn cols(w: f32) -> Cols {
    if w < COMPACT_W {
        Cols {
            installed: 78.0,
            latest: 70.0,
            released: 0.0,
            size: 0.0,
            status: 200.0,
            actions: 178.0,
        }
    } else {
        Cols {
            installed: 92.0,
            latest: 84.0,
            released: 84.0,
            size: 90.0,
            status: 250.0,
            actions: 200.0,
        }
    }
}

pub fn rows(sc: &Scene) -> Vec<usize> {
    let mut v = Vec::new();
    if sc.hub_row {
        v.push(data::HUB);
    }
    v.extend(0..7);
    v
}

#[derive(Default, Clone)]
pub struct Out {
    pub list: Rect,
    pub pane: Rect,
    pub rows: Vec<Rect>,
    pub name_w: f32,
}

pub fn window(p: &mut Painter, pal: &Palette, w: f32, h: f32, sc: &Scene) -> Out {
    p.fill(Rect::new(0.0, 0.0, w, h), pal.page);
    title(p, pal, w, sc);
    bar(p, pal, w, sc);
    let mut top = TITLE_H + BAR_H;
    if let Some(b) = &sc.banner {
        let r = Rect::new(0.0, top, w, BANNER_H);
        draw_offer_bar(
            p,
            pal,
            r,
            if b.warn { glyph::WARN } else { glyph::DOWNLOAD },
            b.text,
            b.yes,
            b.no,
            OfferBarState::default(),
        );
        top += BANNER_H;
    }
    let status_y = h - STATUS_H;
    let mut out = Out::default();
    let rs = rows(sc);
    let list_h = match sc.view {
        View::List => HEAD_H + rs.len() as f32 * ROW_H,
        v => super::grid::height(v, w, rs.len()),
    };
    let room = status_y - top;
    let pane_h = if sc.pane == Pane::None || room - list_h < PANE_MIN {
        0.0
    } else {
        room - list_h
    };
    let list = Rect::new(0.0, top, w, room - pane_h);
    out.list = list;
    out.rows = match sc.view {
        View::List => list_draw(p, pal, list, sc, &rs, w, &mut out.name_w),
        _ => super::grid::draw(p, pal, list, sc, &rs),
    };
    if pane_h > 0.0 {
        let pane = Rect::new(0.0, list.bottom(), w, pane_h);
        out.pane = pane;
        super::panes::draw(p, pal, pane, sc);
    }
    status(p, pal, Rect::new(0.0, status_y, w, STATUS_H), w, sc);
    out
}

// ---- the title row ----------------------------------------------------------------------------

pub fn title(p: &mut Painter, pal: &Palette, w: f32, sc: &Scene) {
    p.fill(Rect::new(0.0, 0.0, w, TITLE_H), pal.title);
    let tile = Rect::new(14.0, 9.0, 22.0, 22.0);
    ui::tile(p, tile, &APPS[data::HUB], false);
    let mut right = w;
    for kind in [WinButton::Close, WinButton::Max, WinButton::Min] {
        let b = win_button_rect(right, 0);
        right -= WIN_BTN_W;
        draw_win_button(p, pal, kind, b, false, false, false);
    }
    let labels = ["Apps", "Activity"];
    let avail = (right - DRAG_MIN - TABS_LEFT).max(0.0);
    let natural: Vec<f32> = labels.iter().map(|l| tab_natural_width(p, l)).collect();
    let widths = tab_widths(&natural, avail);
    let mut x = TABS_LEFT;
    for (i, label) in labels.iter().enumerate() {
        let st = TabState {
            active: sc.tab == i,
            next_quiet: i + 1 < labels.len() && sc.tab != i && sc.tab != i + 1,
            ..TabState::default()
        };
        draw_tab(p, pal, x, widths[i], w, label, &st);
        x += widths[i];
    }
}

// ---- the command bar --------------------------------------------------------------------------

pub fn bar_button(
    p: &mut Painter,
    pal: &Palette,
    x: f32,
    y: f32,
    g: &str,
    label: Option<&str>,
    enabled: bool,
) -> f32 {
    let cy = y + 16.0;
    let w = match label {
        Some(l) => 44.0 + p.measure(Face::Ui, 13.0, l),
        None => 36.0,
    };
    let b = Rect::new(x, y, w, 32.0);
    if let Some(fill) = button_fill(pal, false, false, false, false).filter(|_| enabled) {
        p.rbox(b, 6.0, fill);
    }
    let gx = if label.is_some() { x + 18.0 } else { b.cx() };
    let color = if enabled { pal.text } else { pal.faint };
    p.icon(g, 15.0, gx, cy, color);
    if let Some(l) = label {
        p.text(Face::Ui, 13.0, x + 34.0, cy, l, color, f32::INFINITY);
    }
    w
}

fn primary(p: &mut Painter, pal: &Palette, x: f32, y: f32, g: &str, label: &str) -> f32 {
    let cy = y + 16.0;
    let wd = 44.0 + p.measure(Face::Semibold, 13.0, label);
    p.rbox(Rect::new(x, y, wd, 32.0), 6.0, pal.accent);
    p.icon(g, 14.0, x + 18.0, cy, pal.on_accent);
    p.text(
        Face::Semibold,
        13.0,
        x + 34.0,
        cy,
        label,
        pal.on_accent,
        f32::INFINITY,
    );
    wd
}

pub fn platform_label(sc: &Scene) -> &'static str {
    if sc.linux {
        "Linux x86_64"
    } else {
        "Windows x64"
    }
}

fn bar(p: &mut Painter, pal: &Palette, w: f32, sc: &Scene) {
    let r = Rect::new(0.0, TITLE_H, w, BAR_H);
    p.fill(r, pal.bar);
    let y = r.y + (BAR_H - 32.0) / 2.0;
    let cy = y + 16.0;
    let sep = |p: &mut Painter, x: f32| -> f32 {
        p.fill(
            Rect::new((x + 6.5).round(), y + 6.0, p.px(), 20.0),
            pal.line,
        );
        13.0
    };
    let mut x = 8.0;
    match sc.bar {
        Bar::Updates(n) => {
            x += primary(p, pal, x, y, glyph::DOWNLOAD, &format!("Update all ({n})")) + 6.0;
            x += sep(p, x);
            bar_button(p, pal, x, y, glyph::RELOAD, Some("Check now"), true);
        }
        Bar::Busy(t) => {
            x += bar_button(p, pal, x, y, super::pages::PAUSE, Some("Pause"), true) + 2.0;
            x += bar_button(p, pal, x, y, glyph::CLOSE, Some("Cancel all"), true) + 2.0;
            x += sep(p, x);
            p.text(Face::Ui, 13.0, x + 6.0, cy, t, pal.muted, 360.0);
        }
        Bar::Quiet => {
            bar_button(p, pal, x, y, glyph::RELOAD, Some("Check now"), true);
        }
    }
    // The right group.
    let mut rx = r.right() - 8.0 - 36.0;
    bar_button(p, pal, rx, y, glyph::MORE, None, true);
    rx -= 2.0 + 36.0;
    bar_button(p, pal, rx, y, glyph::SLIDERS, None, true);
    rx -= 13.0;
    sep(p, rx);
    let lab = platform_label(sc);
    let lw = 44.0 + p.measure(Face::Ui, 13.0, lab) + 8.0;
    rx -= lw;
    bar_button(p, pal, rx, y, glyph::SYSTEM, Some(lab), true);
    p.icon(glyph::DOWN, 9.0, rx + lw - 8.0, cy, pal.muted);
    // The view switch: list, large icons, small icons.
    let seg = 30.0;
    rx -= 8.0 + 3.0 * seg + 2.0;
    let sr = Rect::new(rx, y, 3.0 * seg + 2.0, 32.0);
    p.frame(sr, 6.0, pal.field, pal.line);
    for (k, (g, v)) in [
        (super::grid::LIST, View::List),
        (super::grid::LARGE, View::Large),
        (super::grid::SMALL, View::Small),
    ]
    .into_iter()
    .enumerate()
    {
        let b = Rect::new(sr.x + 1.0 + k as f32 * seg, y + 1.0, seg, 30.0);
        let on = sc.view == v;
        if on {
            p.rbox(
                Rect::new(b.x + 2.0, b.y + 2.0, b.w - 4.0, b.h - 4.0),
                4.0,
                pal.on,
            );
        }
        p.icon(
            g,
            14.0,
            b.cx(),
            b.cy(),
            if on { pal.accent } else { pal.muted },
        );
    }
    if w >= 900.0 {
        rx -= 8.0;
        let fw = 220.0;
        let f = Rect::new(rx - fw, y, fw, 32.0);
        p.frame(f, 6.0, pal.field, pal.line);
        p.icon(glyph::SEARCH, 13.0, f.x + 17.0, cy, pal.muted);
        p.text(
            Face::Ui,
            13.0,
            f.x + 34.0,
            cy,
            "Filter apps",
            pal.faint,
            fw - 44.0,
        );
    }
}

// ---- the list ---------------------------------------------------------------------------------

pub fn dot(p: &mut Painter, x: f32, cy: f32, c: Rgb) {
    p.rbox(Rect::new(x, cy - 3.5, 7.0, 7.0), 3.5, c);
}

pub struct Status {
    pub dot: Option<Rgb>,
    pub line: String,
    pub line_col: Rgb,
    pub sub: String,
    pub sub_col: Rgb,
    pub bar: Option<(f32, Rgb)>,
}

pub fn status_of(pal: &Palette, sc: &Scene, i: usize, compact: bool) -> Status {
    let app = &APPS[i];
    let s = |dot, line: &str, lc, sub: &str, sc_| Status {
        dot,
        line: line.into(),
        line_col: lc,
        sub: sub.into(),
        sub_col: sc_,
        bar: None,
    };
    let run = sc.running[i];
    match sc.st[i] {
        St::UpToDate => s(Some(green(pal)), "Up to date", pal.text, "", pal.muted),
        St::Update => {
            let sub = if run {
                "Open now: asks it to close".to_string()
            } else if compact {
                format!("{} download", app.size)
            } else {
                String::new()
            };
            Status {
                sub,
                ..s(Some(blue(pal)), "Update available", pal.text, "", pal.muted)
            }
        }
        St::NotInstalled => {
            let sub = if sc.linux {
                app.linux.unwrap_or("")
            } else if compact {
                app.setup
            } else {
                ""
            };
            s(None, "Not installed", pal.muted, sub, pal.faint)
        }
        St::External => s(
            Some(pal.muted),
            "Installed elsewhere",
            pal.text,
            "Not installed by Hub",
            pal.muted,
        ),
        St::Downloading(f, t) => Status {
            bar: Some((f, pal.accent)),
            ..s(Some(pal.accent), t, pal.text, "", pal.muted)
        },
        St::Verifying => Status {
            bar: Some((1.0, mix(pal.accent, pal.page, 0.45))),
            ..s(
                Some(pal.accent),
                "Checking SHA-256",
                pal.text,
                "",
                pal.muted,
            )
        },
        St::Waiting(why) => s(Some(amber(pal)), "Waiting", pal.text, why, pal.muted),
        St::Failed(why) => s(Some(red(pal)), "Not updated", red(pal), why, pal.muted),
        St::Restart => s(
            Some(green(pal)),
            "Ready: restart to finish",
            pal.text,
            "Files are in place",
            pal.muted,
        ),
        St::Done(t) => s(Some(green(pal)), "Updated", pal.text, t, pal.muted),
        St::NoBuild => s(
            None,
            "No Linux build",
            pal.muted,
            "Its releases carry a Windows installer only",
            pal.faint,
        ),
        St::Pkg => s(
            Some(pal.muted),
            "Installed with apt",
            pal.text,
            "Update it with your package manager",
            pal.muted,
        ),
    }
}

fn list_draw(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    sc: &Scene,
    rs: &[usize],
    w: f32,
    name_w: &mut f32,
) -> Vec<Rect> {
    p.fill(r, pal.page);
    let c = cols(w);
    let px = p.px().max(1.0);
    let old = p.clip(r);
    let head = Rect::new(r.x, r.y, r.w, HEAD_H);
    p.fill(head, pal.pane);
    p.fill(Rect::new(r.x, head.bottom() - px, r.w, px), pal.line);
    let act_x = r.right() - c.actions;
    let stat_x = act_x - c.status;
    let size_x = stat_x - c.size;
    let rel_x = size_x - c.released;
    let lat_x = rel_x - c.latest;
    let ins_x = lat_x - c.installed;
    *name_w = ins_x - 14.0 - 44.0 - 12.0;
    let hy = head.cy();
    let hd = |p: &mut Painter, x: f32, t: &str| {
        p.text(Face::Semibold, 12.0, x + 10.0, hy, t, pal.muted, 200.0);
    };
    hd(p, r.x + 4.0, "App");
    hd(p, ins_x, "Installed");
    hd(p, lat_x, "Latest");
    if c.released > 0.0 {
        hd(p, rel_x, "Released");
        hd(p, size_x, "Download");
    }
    hd(p, stat_x, "Status");
    let mut out = Vec::new();
    let mut y = head.bottom();
    for &i in rs {
        let rr = Rect::new(r.x, y, r.w, ROW_H);
        out.push(rr);
        y += ROW_H;
        if rr.y > r.bottom() {
            continue;
        }
        let app = &APPS[i];
        let st = sc.st[i];
        let sel = sc.sel == i;
        if sel {
            p.fill(rr, pal.on);
            p.fill(Rect::new(rr.x, rr.y + 2.0, 3.0, rr.h - 4.0), pal.accent);
        } else if sc.hover == Some(i) {
            p.fill(rr, pal.hover);
        }
        if matches!(st, St::Failed(_)) {
            p.fill_a(rr, red(pal), if pal.dark { 0.12 } else { 0.08 });
        }
        p.fill_a(Rect::new(rr.x, rr.bottom() - px, rr.w, px), pal.line, 0.55);
        let bg = if sel { pal.on } else { pal.page };
        let cy = rr.cy();
        let dim = matches!(st, St::NoBuild);
        // The app.
        let tile = Rect::new(rr.x + 14.0, cy - 16.0, 32.0, 32.0);
        ui::tile(p, tile, app, dim);
        let nx = tile.right() + 12.0;
        let nm = if dim { pal.faint } else { pal.text };
        let nw = p.text(Face::Semibold, 13.5, nx, cy - 9.0, app.name, nm, *name_w);
        let mut tx = nx + nw + 10.0;
        if sc.running[i] {
            dot(p, tx, cy - 9.0, green(pal));
            tx += 12.0;
            p.text(Face::Ui, 11.5, tx, cy - 9.0, "running", pal.muted, 80.0);
        }
        p.text(
            Face::Ui,
            11.5,
            nx,
            cy + 9.0,
            app.blurb,
            if dim { pal.faint } else { pal.muted },
            *name_w - 4.0,
        );
        // Versions.
        let newer = matches!(
            st,
            St::Update | St::Downloading(..) | St::Verifying | St::Waiting(_) | St::Failed(_)
        );
        match sc.inst[i] {
            Some(v) => {
                p.text(Face::Mono, 12.0, ins_x + 10.0, cy, v, pal.text, c.installed);
            }
            None => {
                p.text(
                    Face::Ui,
                    13.0,
                    ins_x + 10.0,
                    cy,
                    "\u{2014}",
                    pal.faint,
                    c.installed,
                );
            }
        }
        let latest_col = if dim {
            pal.faint
        } else if newer {
            blue(pal)
        } else {
            pal.muted
        };
        let lat = if matches!(st, St::NoBuild) {
            "\u{2014}"
        } else {
            app.latest
        };
        p.text(
            if newer { Face::Semibold } else { Face::Mono },
            12.0,
            lat_x + 10.0,
            cy,
            lat,
            latest_col,
            c.latest,
        );
        if c.released > 0.0 {
            p.text(
                Face::Ui,
                13.0,
                rel_x + 10.0,
                cy,
                app.date,
                pal.muted,
                c.released,
            );
            let sz = if matches!(st, St::NoBuild | St::UpToDate | St::External | St::Pkg) {
                ""
            } else if sc.linux {
                app.linux_size
            } else if matches!(st, St::NotInstalled) {
                app.setup
            } else {
                app.size
            };
            p.text(Face::Ui, 13.0, size_x + 10.0, cy, sz, pal.muted, c.size);
        }
        // Status.
        let s = status_of(pal, sc, i, c.released == 0.0);
        let sx = stat_x + 10.0;
        let lead = if s.dot.is_some() { 15.0 } else { 0.0 };
        let two = !s.sub.is_empty() || s.bar.is_some();
        let ly = if two { cy - 9.0 } else { cy };
        if let Some(d) = s.dot {
            dot(p, sx, ly, d);
        }
        p.text(
            Face::Ui,
            13.0,
            sx + lead,
            ly,
            &s.line,
            s.line_col,
            c.status - 20.0 - lead,
        );
        if let Some((f, col)) = s.bar {
            ui::progress(
                p,
                pal,
                Rect::new(sx, cy + 8.0, c.status - 28.0, 6.0),
                f,
                col,
                bg,
            );
        } else if !s.sub.is_empty() {
            p.text(
                Face::Ui,
                11.5,
                sx + lead,
                cy + 9.0,
                &s.sub,
                s.sub_col,
                c.status - 20.0 - lead,
            );
        }
        // Actions, from the right.
        let mut ax = rr.right() - 12.0;
        p.icon(glyph::MORE, 15.0, ax - 10.0, cy, pal.muted);
        ax -= 32.0;
        let btn = |p: &mut Painter, ax: &mut f32, l: &str, prim: bool, en: bool| {
            let b = ui::sbtn_r(p, pal, *ax, cy, l, prim, en);
            *ax = b.x - 6.0;
        };
        match st {
            St::Update => {
                if sc.running[i] {
                    btn(p, &mut ax, "Update\u{2026}", true, true);
                } else {
                    btn(p, &mut ax, "Update", true, true);
                    btn(p, &mut ax, "Open", false, true);
                }
            }
            St::UpToDate | St::Done(_) | St::Pkg => btn(p, &mut ax, "Open", false, true),
            St::NotInstalled => btn(p, &mut ax, "Install", false, true),
            St::External => {
                btn(p, &mut ax, "Adopt", false, true);
                btn(p, &mut ax, "Open", false, true);
            }
            St::Downloading(..) | St::Verifying | St::Waiting(_) => {
                btn(p, &mut ax, "Cancel", false, true)
            }
            St::Failed(_) => btn(p, &mut ax, "Try again", true, true),
            St::Restart => btn(p, &mut ax, "Restart Hub", true, true),
            St::NoBuild => btn(p, &mut ax, "Install", false, false),
        }
    }
    p.restore(old);
    out
}

// ---- the status bar ---------------------------------------------------------------------------

fn status(p: &mut Painter, pal: &Palette, r: Rect, w: f32, sc: &Scene) {
    p.fill(r, pal.bar);
    p.fill(Rect::new(r.x, r.y, r.w, p.px().max(1.0)), pal.line);
    let cy = r.cy();
    p.icon(glyph::GLOBE, 12.0, 16.0, cy, pal.muted);
    let left = if w < COMPACT_W {
        "Contacts github.com only"
    } else {
        "Contacts github.com only: release lists and the files you install. No account, no usage data."
    };
    let lw = p.text(Face::Ui, 12.0, 30.0, cy, left, pal.muted, w * 0.62);
    p.text(
        Face::Ui,
        12.0,
        30.0 + lw + 10.0,
        cy,
        "Details",
        blue(pal),
        120.0,
    );
    let hubv = sc.inst[data::HUB].unwrap_or("0.4.0");
    let right = format!("{} \u{b7} Hub {hubv}", sc.checked);
    p.text_right(Face::Ui, 12.0, r.right() - 14.0, cy, &right, pal.muted);
}
