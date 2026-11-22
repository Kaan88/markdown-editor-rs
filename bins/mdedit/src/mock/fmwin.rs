//! The file manager window: title row with folder tabs, address row, command strip, navigation
//! pane, one or two panes (details, list or icons), an optional preview pane, the status bar.
//! Every number is a constant here so the layout can be argued from the pictures.

use super::fmdata::{self as d, COMPUTING, Cmp, E, Flag, K, NOT_COMPUTED};
use mdshared_ui::chrome::{
    BAR_H, DRAG_MIN, TABS_LEFT, TITLE_H, TabState, WIN_BTN_W, WinButton, button_fill, draw_tab,
    draw_win_button, tab_natural_width, tab_widths, win_button_rect,
};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

pub const MENUBAR_H: f32 = 28.0;
pub const CMD_H: f32 = 38.0;
pub const STATUS_H: f32 = 26.0;
pub const HEAD_H: f32 = 28.0;
pub const ROW_H: f32 = 26.0;
pub const PANE_HEAD_H: f32 = 32.0;
pub const NAV_W: f32 = 224.0;
pub const NAV_W_SMALL: f32 = 176.0;
pub const NAV_RAIL: f32 = 44.0;
pub const PREVIEW_W: f32 = 300.0;
pub const SPLIT_W: f32 = 7.0;
/// Under this width the Type column goes, the navigation pane narrows, and a split window
/// shows the navigation pane as a rail.
pub const COMPACT_W: f32 = 1000.0;

pub const VIEW_DETAILS: &str = "M2 3.5h12M2 8h12M2 12.5h12M5 3.5v9";
pub const VIEW_LIST: &str = "M2.5 3.5h4M2.5 8h4M2.5 12.5h4M9.5 3.5h4M9.5 8h4M9.5 12.5h4";
pub const VIEW_ICONS: &str = "M2.5 2.5h4v4h-4zM9.5 2.5h4v4h-4zM2.5 9.5h4v4h-4zM9.5 9.5h4v4h-4z";
pub const PIN_OUT: &str = "M5.5 2.5h5M6.5 2.5v3.5L4.5 9.5h7L9.5 6V2.5M8 9.5v4";
pub const DRIVE: &str = "M2 9.5h12v3.5a.5 .5 0 0 1-.5 .5h-11a.5 .5 0 0 1-.5-.5zM2 9.5l1.6-5.6a.8 .8 0 0 1 .8-.6h7.2a.8 .8 0 0 1 .8 .6L14 9.5M11.5 11.4h.01";
pub const HOME: &str = "M2 8l6-5.5L14 8M3.5 7v6.5h3.5V10h2v3.5h3.5V7";
pub const BIN: &str = "M2.5 4h11M6 4V2.5a.5 .5 0 0 1 .5-.5h3a.5 .5 0 0 1 .5 .5V4M4 4l.6 9.4a.6 .6 0 0 0 .6 .6h5.6a.6 .6 0 0 0 .6-.6L12 4M6.7 6.6v4.8M9.3 6.6v4.8";
pub const NET: &str = "M6 2.5h4v3H6zM1.5 10.5h4v3h-4zM10.5 10.5h4v3h-4zM8 5.5v2.5M3.5 10.5V8h9v2.5";
pub const SPLIT_PANE: &str = glyph::SPLIT;
pub const GROUP: &str = "M2.5 3h11M2.5 6h6M2.5 10h11M2.5 13h6";
pub const COPY_TO: &str = "M2.5 3.5h6v6h-6zM6 6.5h6v6H6zM11 3v3M9.5 4.5h3";
pub const COMPARE: &str = "M2.5 2.5h4.5v11H2.5zM9 2.5h4.5v11H9zM4 6h1.5M4 9h1.5M10.5 6H12M10.5 9H12";
pub const RENAME: &str = "M2 5.5h12v5H2zM5 3v10M3.2 3h3.6M3.2 13h3.6";
pub const TERM: &str = "M2 3h12a.5 .5 0 0 1 .5 .5v9a.5 .5 0 0 1-.5 .5H2a.5 .5 0 0 1-.5-.5v-9A.5 .5 0 0 1 2 3zM4.5 6.5L7 8.5l-2.5 2M8.5 10.7h3";
pub const HASH: &str = "M5.5 2.5l-1 11M11.5 2.5l-1 11M2.5 6h11M2 10h11";
pub const PAUSE: &str = "M5 3h2v10H5zM9 3h2v10H9z";
pub const SHARE: &str = "M3 8.5v4.5h10V8.5M8 10V2.5M5.2 5.2L8 2.5l2.8 2.7";
pub const ZIP: &str = "M3 2.5h10a.5 .5 0 0 1 .5 .5v10a.5 .5 0 0 1-.5 .5H3a.5 .5 0 0 1-.5-.5V3a.5 .5 0 0 1 .5-.5zM8 2.5v1.5M8 5v1.5M8 7.5v1.5M7 10.2h2v2.3H7z";
pub const NEWFOLDER: &str = "M1.5 11.5V4a1 1 0 0 1 1-1h3.2l1.5 1.5h5.3a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1zM8 6.5v4M6 8.5h4";

#[derive(Clone, Copy, PartialEq, Default)]
pub enum View {
    #[default]
    Details,
    List,
    Icons,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Content {
    Project,
    Downloads,
    Pictures,
    Compare,
    Find,
    Bin,
}

#[derive(Clone, PartialEq)]
pub struct PaneSt {
    pub content: Content,
    pub view: View,
    pub sel: Vec<usize>,
    pub hover: Option<usize>,
}

impl PaneSt {
    pub fn new(content: Content) -> PaneSt {
        let (view, sel) = match content {
            Content::Project => (View::Details, d::project_sel()),
            Content::Downloads => (View::Details, vec![1]),
            Content::Pictures => (View::Icons, vec![3]),
            _ => (View::Details, vec![]),
        };
        PaneSt {
            content,
            view,
            sel,
            hover: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Strip {
    None,
    Copy,
    Compare,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Prev {
    Md,
    Img,
    Many,
}

#[derive(Clone)]
pub struct Scene {
    pub panes: [PaneSt; 2],
    pub split: bool,
    pub active: usize,
    pub preview: Option<Prev>,
    pub tab: usize,
    pub menubar: Option<usize>,
    /// A progress strip at the foot of the window: the copy queue's summary.
    pub strip: Strip,
    pub nav_sel: usize,
    pub status: Option<&'static str>,
    pub hidden: bool,
}

impl Default for Scene {
    fn default() -> Scene {
        Scene {
            panes: [PaneSt::new(Content::Project), PaneSt::new(Content::Downloads)],
            split: false,
            active: 0,
            preview: None,
            tab: 0,
            menubar: None,
            strip: Strip::None,
            nav_sel: 5,
            status: None,
            hidden: false,
        }
    }
}

#[derive(Default, Clone)]
pub struct Out {
    pub panes: Vec<Rect>,
    pub rows: [Vec<Rect>; 2],
    pub menubar: Vec<Rect>,
    pub cmd: Vec<(&'static str, Rect)>,
    pub status: Rect,
    pub nav: Rect,
    pub preview: Rect,
}

pub fn blue(pal: &Palette) -> Rgb {
    if pal.dark { [0x6c, 0xb6, 0xff] } else { [0x00, 0x67, 0xc0] }
}
pub fn green(pal: &Palette) -> Rgb {
    if pal.dark { [0x6c, 0xcb, 0x5f] } else { [0x0f, 0x7b, 0x0f] }
}
pub fn amber(pal: &Palette) -> Rgb {
    if pal.dark { [0xfc, 0xe1, 0x00] } else { [0x9d, 0x5d, 0x00] }
}
pub fn red(pal: &Palette) -> Rgb {
    if pal.dark { [0xff, 0x99, 0xa4] } else { [0xc4, 0x2b, 0x1c] }
}

pub fn tile(p: &mut Painter, r: Rect) {
    p.rbox(r, r.w * 0.22, [0x2a, 0x8a, 0x8a]);
    p.rbox_a(Rect::new(r.x, r.y, r.w, r.h / 2.0), r.w * 0.22, [255, 255, 255], 0.08);
    p.icon(glyph::FOLDER_OPEN, r.w * 0.58, r.cx(), r.cy(), [255, 255, 255]);
}

pub fn menubar_labels() -> [&'static str; 6] {
    ["File", "Edit", "View", "Go", "Tools", "Help"]
}

pub fn window(p: &mut Painter, pal: &Palette, w: f32, h: f32, sc: &Scene) -> Out {
    let mut out = Out::default();
    p.fill(Rect::new(0.0, 0.0, w, h), pal.page);
    title(p, pal, w, sc);
    let mut y = TITLE_H;
    if let Some(open) = sc.menubar {
        out.menubar = menubar(p, pal, Rect::new(0.0, y, w, MENUBAR_H), open);
        y += MENUBAR_H;
    }
    address(p, pal, Rect::new(0.0, y, w, BAR_H), sc);
    y += BAR_H;
    out.cmd = command(p, pal, Rect::new(0.0, y, w, CMD_H), sc, w);
    y += CMD_H;
    let status_y = h - STATUS_H;
    let body = Rect::new(0.0, y, w, status_y - y);
    let compact = w < COMPACT_W;
    // The navigation pane: full, narrow, or (split and compact) a rail.
    let nav_w = if compact && sc.split {
        NAV_RAIL
    } else if compact {
        NAV_W_SMALL
    } else {
        NAV_W
    };
    let nav = Rect::new(0.0, body.y, nav_w, body.h);
    out.nav = nav;
    nav_pane(p, pal, nav, sc, nav_w == NAV_RAIL);
    let mut right = w;
    if let Some(pv) = sc.preview {
        let pw = if compact { 240.0 } else { PREVIEW_W };
        let r = Rect::new(w - pw, body.y, pw, body.h);
        out.preview = r;
        preview(p, pal, r, sc, pv);
        right = r.x;
    }
    let area = Rect::new(nav.right(), body.y, right - nav.right(), body.h);
    if sc.split {
        let pw = ((area.w - SPLIT_W) / 2.0).floor();
        let a = Rect::new(area.x, area.y, pw, area.h);
        let b = Rect::new(area.x + pw + SPLIT_W, area.y, area.w - pw - SPLIT_W, area.h);
        // The splitter.
        let sp = Rect::new(a.right(), area.y, SPLIT_W, area.h);
        p.fill(sp, pal.pane);
        p.fill(Rect::new(sp.cx() - 0.5, sp.y, p.px().max(1.0), sp.h), pal.line);
        p.rbox(Rect::new(sp.cx() - 1.5, sp.cy() - 14.0, 3.0, 28.0), 1.5, pal.faint);
        out.panes = vec![a, b];
    } else {
        out.panes = vec![area];
    }
    for i in 0..out.panes.len() {
        let r = out.panes[i];
        out.rows[i] = pane(p, pal, r, sc, i);
    }
    match sc.strip {
        Strip::Copy => queue_strip(p, pal, Rect::new(0.0, status_y - 30.0, w, 30.0)),
        Strip::Compare => compare_strip(p, pal, Rect::new(0.0, status_y - 36.0, w, 36.0)),
        Strip::None => {}
    }
    let st = Rect::new(0.0, status_y, w, STATUS_H);
    out.status = st;
    status(p, pal, st, sc, w);
    out
}

// ---- the title row ----------------------------------------------------------------------------

fn title(p: &mut Painter, pal: &Palette, w: f32, sc: &Scene) {
    p.fill(Rect::new(0.0, 0.0, w, TITLE_H), pal.title);
    tile(p, Rect::new(14.0, 9.0, 22.0, 22.0));
    let mut right = w;
    for kind in [WinButton::Close, WinButton::Max, WinButton::Min] {
        let b = win_button_rect(right, 0);
        right -= WIN_BTN_W;
        draw_win_button(p, pal, kind, b, false, false, false);
    }
    let labels = ["mdedit", "Downloads", "Trip"];
    let avail = (right - DRAG_MIN - TABS_LEFT - 40.0).max(0.0);
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
    p.icon(glyph::PLUS, 12.0, x + 17.0, 23.0, pal.muted);
}

fn menubar(p: &mut Painter, pal: &Palette, r: Rect, open: usize) -> Vec<Rect> {
    p.fill(r, pal.bar);
    let mut x = 10.0;
    let mut out = Vec::new();
    for (i, l) in menubar_labels().iter().enumerate() {
        let wd = p.measure(Face::Ui, 13.0, l) + 20.0;
        let b = Rect::new(x, r.y + 2.0, wd, r.h - 4.0);
        if i == open {
            p.rbox(b, 5.0, pal.on);
        }
        p.text_center(Face::Ui, 13.0, b, l, pal.text);
        out.push(b);
        x += wd;
    }
    out
}

// ---- the address row --------------------------------------------------------------------------

fn ibtn(p: &mut Painter, pal: &Palette, x: f32, cy: f32, g: &str, on: bool) {
    let b = Rect::new(x, cy - 16.0, 32.0, 32.0);
    if let Some(f) = button_fill(pal, false, false, false, false).filter(|_| on) {
        p.rbox(b, 6.0, f);
    }
    p.icon(g, 15.0, b.cx(), cy, if on { pal.text } else { pal.faint });
}

pub const CRUMBS: [&str; 4] = ["This PC", "Projects", "mdedit", ""];

pub fn crumbs_for(c: Content) -> Vec<&'static str> {
    match c {
        Content::Project => vec!["This PC", "Projects", "mdedit"],
        Content::Downloads => vec!["This PC", "Downloads"],
        Content::Pictures => vec!["This PC", "Pictures", "Trip"],
        Content::Compare => vec!["D:", "Backup", "mdedit"],
        Content::Find => vec!["Search results in mdedit"],
        Content::Bin => vec!["Recycle Bin"],
    }
}

fn crumb_row(p: &mut Painter, pal: &Palette, r: Rect, crumbs: &[&str]) {
    let cy = r.cy();
    let mut x = r.x + 12.0;
    for (i, c) in crumbs.iter().enumerate() {
        let last = i + 1 == crumbs.len();
        let f = if last { Face::Semibold } else { Face::Ui };
        let wd = p.text(f, 13.0, x, cy, c, if last { pal.text } else { pal.muted }, r.w - 40.0);
        x += wd;
        if !last {
            p.icon(glyph::RIGHT, 9.0, x + 9.0, cy, pal.faint);
            x += 18.0;
        }
    }
}

fn address(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene) {
    p.fill(r, pal.bar);
    let cy = r.cy();
    let mut x = 8.0;
    for (g, on) in [(glyph::BACK, true), (glyph::FORWARD, false), (glyph::UP, true), (glyph::RELOAD, true)] {
        let g = if g == glyph::UP { "M8 13V3M4 7l4-4 4 4" } else { g };
        ibtn(p, pal, x, cy, g, on);
        x += 34.0;
    }
    x += 4.0;
    let sw = 232.0_f32.min(r.w * 0.22);
    let search = Rect::new(r.right() - 10.0 - sw, cy - 16.0, sw, 32.0);
    let field = Rect::new(x, cy - 16.0, search.x - 10.0 - x, 32.0);
    p.frame(field, 6.0, pal.field, pal.line);
    if sc.split {
        // In a split window each pane carries its own path; this one names the two.
        p.icon(SPLIT_PANE, 13.0, field.x + 18.0, cy, pal.accent);
        p.text(
            Face::Ui,
            13.0,
            field.x + 36.0,
            cy,
            "Split view",
            pal.text,
            120.0,
        );
        p.text(
            Face::Ui,
            12.0,
            field.x + 118.0,
            cy,
            "each pane has its own path, history and view; Tab switches",
            pal.faint,
            field.w - 130.0,
        );
    } else {
        p.icon(glyph::FOLDER, 14.0, field.x + 18.0, cy, pal.folder);
        crumb_row(
            p,
            pal,
            Rect::new(field.x + 24.0, field.y, field.w - 30.0, field.h),
            &crumbs_for(sc.panes[0].content),
        );
    }
    p.frame(search, 6.0, pal.field, pal.line);
    p.icon(glyph::SEARCH, 13.0, search.x + 17.0, cy, pal.muted);
    let ph = match sc.panes[sc.active].content {
        Content::Find => "recycle bin",
        Content::Project => "Search mdedit",
        _ => "Search this folder",
    };
    let col = if sc.panes[sc.active].content == Content::Find {
        pal.text
    } else {
        pal.faint
    };
    p.text(Face::Ui, 13.0, search.x + 34.0, cy, ph, col, search.w - 44.0);
}

// ---- the command strip ------------------------------------------------------------------------

fn command(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, w: f32) -> Vec<(&'static str, Rect)> {
    p.fill(r, pal.page);
    p.fill(Rect::new(r.x, r.bottom() - p.px().max(1.0), r.w, p.px().max(1.0)), pal.line);
    let cy = r.cy();
    let mut out = Vec::new();
    let mut x = 8.0;
    // New is the one primary action.
    let nw = 30.0 + p.measure(Face::Semibold, 13.0, "New") + 22.0;
    let nb = Rect::new(x, cy - 14.0, nw, 28.0);
    p.rbox(nb, 6.0, pal.accent);
    p.icon(glyph::PLUS, 12.0, x + 17.0, cy, pal.on_accent);
    p.text(Face::Semibold, 13.0, x + 31.0, cy, "New", pal.on_accent, 80.0);
    p.icon(glyph::DOWN, 8.0, nb.right() - 12.0, cy, pal.on_accent);
    out.push(("new", nb));
    x += nw + 8.0;
    let sep = |p: &mut Painter, x: &mut f32| {
        p.fill(Rect::new((*x + 3.0).round(), cy - 10.0, p.px().max(1.0), 20.0), pal.line);
        *x += 9.0;
    };
    sep(p, &mut x);
    let sel = !sc.panes[sc.active].sel.is_empty();
    let mut btn = |p: &mut Painter, x: &mut f32, name: &'static str, g: &str, on: bool, label: Option<&str>| {
        let wd = match label {
            Some(l) => 40.0 + p.measure(Face::Ui, 13.0, l),
            None => 32.0,
        };
        let b = Rect::new(*x, cy - 14.0, wd, 28.0);
        let c = if on { pal.text } else { pal.faint };
        p.icon(g, 15.0, if label.is_some() { b.x + 18.0 } else { b.cx() }, cy, c);
        if let Some(l) = label {
            p.text(Face::Ui, 13.0, b.x + 32.0, cy, l, c, 200.0);
        }
        out.push((name, b));
        *x += wd + 2.0;
    };
    btn(p, &mut x, "cut", glyph::CUT, sel, None);
    btn(p, &mut x, "copy", glyph::COPY, sel, None);
    btn(p, &mut x, "paste", glyph::PASTE, true, None);
    btn(p, &mut x, "rename", RENAME, sel, None);
    btn(p, &mut x, "share", SHARE, sel, None);
    btn(p, &mut x, "delete", glyph::DELETE, sel, None);
    sep(p, &mut x);
    if w >= COMPACT_W {
        btn(p, &mut x, "sort", glyph::SORT, true, Some("Sort"));
        btn(p, &mut x, "group", GROUP, true, Some("Group"));
        btn(p, &mut x, "view", VIEW_DETAILS, true, Some("View"));
    } else {
        btn(p, &mut x, "sort", glyph::SORT, true, None);
        btn(p, &mut x, "group", GROUP, true, None);
        btn(p, &mut x, "view", VIEW_DETAILS, true, None);
    }
    if sc.split {
        sep(p, &mut x);
        if w >= COMPACT_W {
            btn(p, &mut x, "copyto", COPY_TO, true, Some("Copy to other pane"));
            btn(p, &mut x, "compare", COMPARE, true, Some("Compare"));
        } else {
            btn(p, &mut x, "copyto", COPY_TO, true, None);
            btn(p, &mut x, "compare", COMPARE, true, None);
        }
    }
    // The right group: split, preview, more.
    let mut rx = r.right() - 8.0;
    for (name, g, on) in [
        ("more", glyph::MORE, false),
        ("preview", glyph::SIDEBAR, sc.preview.is_some()),
        ("split", SPLIT_PANE, sc.split),
    ] {
        let b = Rect::new(rx - 32.0, cy - 14.0, 32.0, 28.0);
        if on {
            p.rbox(b, 6.0, pal.on);
        }
        p.icon(g, 15.0, b.cx(), cy, if on { pal.accent } else { pal.muted });
        out.push((name, b));
        rx -= 34.0;
    }
    out
}

// ---- the navigation pane ----------------------------------------------------------------------

fn nav_head(p: &mut Painter, pal: &Palette, x: f32, y: f32, label: &str) {
    p.text(Face::Semibold, 11.0, x + 14.0, y + 12.0, label, pal.faint, 160.0);
}

fn nav_pane(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, rail: bool) {
    p.fill(r, pal.pane);
    p.fill(Rect::new(r.right() - p.px().max(1.0), r.y, p.px().max(1.0), r.h), pal.line);
    let places: [(&str, &str, Rgb, &str); 6] = [
        (HOME, "Home", pal.accent, ""),
        (glyph::FOLDER, "Desktop", pal.folder, ""),
        (glyph::DOC, "Documents", pal.folder, ""),
        (glyph::DOWNLOAD, "Downloads", [0x4a, 0xb0, 0x7a], "3"),
        (glyph::IMAGE, "Pictures", [0x4a, 0xb0, 0x7a], ""),
        (glyph::FOLDER, "Projects", pal.folder, ""),
    ];
    if rail {
        let mut y = r.y + 10.0;
        for (i, (g, _, c, _)) in places.iter().enumerate() {
            let b = Rect::new(r.x + 4.0, y, r.w - 8.0, 32.0);
            if i == sc.nav_sel {
                p.rbox(b, 5.0, pal.on);
                p.fill(Rect::new(b.x, b.y + 7.0, 3.0, 18.0), pal.accent);
            }
            p.icon(g, 16.0, b.cx(), b.cy(), *c);
            y += 34.0;
        }
        y += 10.0;
        for g in [DRIVE, DRIVE, DRIVE, BIN] {
            p.icon(g, 16.0, r.cx(), y + 16.0, pal.muted);
            y += 34.0;
        }
        return;
    }
    let old = p.clip(r);
    let mut y = r.y + 8.0;
    nav_head(p, pal, r.x, y, "PLACES");
    y += 24.0;
    for (i, (g, l, c, badge)) in places.iter().enumerate() {
        let b = Rect::new(r.x + 6.0, y, r.w - 12.0, 28.0);
        if i == sc.nav_sel {
            p.rbox(b, 5.0, pal.on);
            p.fill(Rect::new(b.x, b.y + 7.0, 3.0, 14.0), pal.accent);
        }
        p.icon(g, 15.0, b.x + 20.0, b.cy(), *c);
        p.text(Face::Ui, 13.0, b.x + 38.0, b.cy(), l, pal.text, 120.0);
        if !badge.is_empty() {
            p.rbox(Rect::new(b.right() - 36.0, b.cy() - 8.0, 22.0, 16.0), 8.0, mix(pal.pane, pal.accent, 0.25));
            p.text_center(Face::Semibold, 10.5, Rect::new(b.right() - 36.0, b.cy() - 8.0, 22.0, 16.0), badge, blue(pal));
        } else if i == 5 {
            p.icon(PIN_OUT, 11.0, b.right() - 18.0, b.cy(), pal.faint);
        }
        y += 28.0;
    }
    y += 10.0;
    nav_head(p, pal, r.x, y, "THIS PC");
    y += 24.0;
    let drives: [(&str, &str, f32, &str); 3] = [
        ("Windows (C:)", "142 GB free of 476 GB", 0.70, ""),
        ("Data (D:)", "1.2 TB free of 1.8 TB", 0.33, ""),
        ("Backup (E:)", "88 GB free of 232 GB", 0.62, "USB"),
    ];
    for (l, sub, f, tag) in drives {
        let b = Rect::new(r.x + 6.0, y, r.w - 12.0, 44.0);
        p.icon(DRIVE, 15.0, b.x + 20.0, b.y + 12.0, pal.muted);
        p.text(Face::Ui, 13.0, b.x + 38.0, b.y + 12.0, l, pal.text, 130.0);
        if !tag.is_empty() {
            let tw = p.measure(Face::Semibold, 10.0, tag) + 10.0;
            let tr = Rect::new(b.right() - tw - 6.0, b.y + 4.0, tw, 16.0);
            p.rbox(tr, 8.0, mix(pal.pane, pal.text, 0.12));
            p.text_center(Face::Semibold, 10.0, tr, tag, pal.muted);
        }
        let bar = Rect::new(b.x + 38.0, b.y + 24.0, b.w - 52.0, 4.0);
        p.rbox(bar, 2.0, mix(pal.pane, pal.text, 0.14));
        let c = if f > 0.9 { red(pal) } else { pal.accent };
        p.rbox(Rect::new(bar.x, bar.y, bar.w * f, bar.h), 2.0, c);
        p.text(Face::Ui, 11.0, b.x + 38.0, b.y + 37.0, sub, pal.faint, b.w - 44.0);
        y += 48.0;
    }
    y += 6.0;
    for (g, l) in [(NET, "Network"), (BIN, "Recycle Bin")] {
        let b = Rect::new(r.x + 6.0, y, r.w - 12.0, 28.0);
        p.icon(g, 15.0, b.x + 20.0, b.cy(), pal.muted);
        p.text(Face::Ui, 13.0, b.x + 38.0, b.cy(), l, pal.text, 120.0);
        if l == "Recycle Bin" {
            p.text_right(Face::Ui, 11.5, b.right() - 12.0, b.cy(), "14 items", pal.faint);
        }
        y += 28.0;
    }
    p.restore(old);
}

// ---- a pane -----------------------------------------------------------------------------------

fn pane(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, i: usize) -> Vec<Rect> {
    let ps = &sc.panes[i];
    let active = sc.active == i;
    p.fill(r, pal.page);
    let mut top = r.y;
    if sc.split {
        let head = Rect::new(r.x, r.y, r.w, PANE_HEAD_H);
        p.fill(head, if active { pal.bar } else { pal.pane });
        p.fill(Rect::new(head.x, head.bottom() - p.px().max(1.0), head.w, p.px().max(1.0)), pal.line);
        if active {
            p.fill(Rect::new(head.x, head.y, head.w, 2.0), pal.accent);
        }
        p.icon(glyph::FOLDER, 13.0, head.x + 20.0, head.cy() + 1.0, pal.folder);
        crumb_row(
            p,
            pal,
            Rect::new(head.x + 20.0, head.y + 1.0, head.w - 80.0, head.h),
            &crumbs_for(if ps.content == Content::Compare && i == 0 { Content::Project } else { ps.content }),
        );
        p.icon(glyph::CLOSE, 8.0, head.right() - 16.0, head.cy() + 1.0, pal.faint);
        top = head.bottom();
    }
    let body = Rect::new(r.x, top, r.w, r.bottom() - top);
    match ps.content {
        Content::Project => {
            let es = d::project();
            let es: Vec<E> = es.into_iter().filter(|e| sc.hidden || e.flag != Flag::Hidden).collect();
            let sel = if sc.hidden { ps.sel.clone() } else { ps.sel.iter().map(|s| s - 2).collect() };
            view(p, pal, body, &es, ps, &sel, active, &[], None)
        }
        Content::Downloads => {
            let (es, groups) = d::downloads();
            view(p, pal, body, &es, ps, &ps.sel.clone(), active, &groups, None)
        }
        Content::Pictures => {
            let es = d::pictures();
            view(p, pal, body, &es, ps, &ps.sel.clone(), active, &[], None)
        }
        Content::Compare => compare(p, pal, body, active, i),
        Content::Find => super::fmover::find_results(p, pal, body),
        Content::Bin => Vec::new(),
    }
}

pub struct Opts {
    pub type_col: bool,
}

#[allow(clippy::too_many_arguments)]
fn view(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    es: &[E],
    ps: &PaneSt,
    sel: &[usize],
    active: bool,
    groups: &[(usize, &'static str)],
    marks: Option<&[Cmp]>,
) -> Vec<Rect> {
    match ps.view {
        View::Details => details(p, pal, r, es, sel, ps.hover, active, groups, marks),
        View::List => list(p, pal, r, es, sel, active),
        View::Icons => icons(p, pal, r, es, sel, active),
    }
}

pub fn sel_fill(pal: &Palette, active: bool) -> Rgb {
    if active { mix(pal.page, pal.accent, if pal.dark { 0.30 } else { 0.20 }) } else { pal.on }
}

/// A file's small icon.
pub fn small_icon(p: &mut Painter, pal: &Palette, e: &E, cx: f32, cy: f32) {
    let c = if e.k == K::Dir { pal.folder } else { d::tint(e.k) };
    let g = match e.k {
        K::Dir => glyph::FOLDER,
        K::Img => glyph::IMAGE,
        K::Zip => ZIP,
        K::Exe => glyph::SYSTEM,
        _ => glyph::DOC,
    };
    p.icon(g, 16.0, cx, cy, c);
    match e.flag {
        Flag::Cloud => {
            p.rbox(Rect::new(cx + 2.0, cy + 2.0, 9.0, 9.0), 4.5, pal.page);
            p.icon(glyph::GLOBE, 8.0, cx + 6.5, cy + 6.5, blue(pal));
        }
        Flag::Link => p.icon(glyph::LINK, 8.0, cx + 6.0, cy + 6.0, pal.text),
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn details(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    es: &[E],
    sel: &[usize],
    hover: Option<usize>,
    active: bool,
    groups: &[(usize, &'static str)],
    marks: Option<&[Cmp]>,
) -> Vec<Rect> {
    let px = p.px().max(1.0);
    let old = p.clip(r);
    let wide = r.w >= 780.0;
    let (size_w, date_w, type_w) = (if r.w < 500.0 { 84.0 } else { 120.0 }, if r.w < 500.0 { 124.0 } else { 140.0 }, 120.0);
    let type_w = if wide { type_w } else { 0.0 };
    let gutter = if marks.is_some() { 26.0 } else { 0.0 };
    let size_x = r.right() - size_w - 14.0;
    let date_x = size_x - date_w;
    let type_x = date_x - type_w;
    let head = Rect::new(r.x, r.y, r.w, HEAD_H);
    p.fill(head, pal.pane);
    p.fill(Rect::new(r.x, head.bottom() - px, r.w, px), pal.line);
    let hy = head.cy();
    p.text(Face::Semibold, 12.0, r.x + 44.0 + gutter, hy, "Name", pal.text, 100.0);
    p.icon(glyph::UP, 9.0, r.x + 44.0 + gutter + 44.0, hy, pal.muted);
    p.text(Face::Semibold, 12.0, date_x + 8.0, hy, "Date modified", pal.muted, date_w);
    if wide {
        p.text(Face::Semibold, 12.0, type_x + 8.0, hy, "Type", pal.muted, type_w);
    }
    p.text_right(Face::Semibold, 12.0, size_x + size_w - 6.0, hy, "Size", pal.muted);
    for x in [date_x, type_x, size_x] {
        if x > r.x + 200.0 && !(x == type_x && !wide) {
            p.fill(Rect::new(x, head.y + 6.0, px, head.h - 12.0), pal.line);
        }
    }
    let max_dir = es.iter().filter(|e| e.k == K::Dir).map(|e| e.size).max().unwrap_or(1).max(1);
    let mut rows = Vec::new();
    let mut y = head.bottom();
    for (i, e) in es.iter().enumerate() {
        if let Some(&(_, g)) = groups.iter().find(|(gi, _)| *gi == i) {
            let n = groups
                .iter()
                .position(|(gi, _)| *gi == i)
                .map(|k| groups.get(k + 1).map(|x| x.0).unwrap_or(es.len()) - i)
                .unwrap_or(0);
            p.text(Face::Semibold, 12.0, r.x + 14.0, y + 15.0, g, blue(pal), 200.0);
            let gw = p.measure(Face::Semibold, 12.0, g);
            p.text(Face::Ui, 11.5, r.x + 24.0 + gw, y + 15.0, &format!("{n}"), pal.faint, 40.0);
            p.fill(Rect::new(r.x + 14.0, y + 27.0, r.w - 28.0, px), pal.line);
            y += 32.0;
        }
        let rr = Rect::new(r.x, y, r.w, ROW_H);
        y += ROW_H;
        rows.push(rr);
        if rr.y > r.bottom() {
            continue;
        }
        let selected = sel.contains(&i);
        if selected {
            p.rbox(Rect::new(rr.x + 4.0, rr.y + 1.0, rr.w - 8.0, rr.h - 2.0), 4.0, sel_fill(pal, active));
        } else if hover == Some(i) {
            p.rbox(Rect::new(rr.x + 4.0, rr.y + 1.0, rr.w - 8.0, rr.h - 2.0), 4.0, pal.hover);
        }
        let cy = rr.cy();
        let faint = e.flag == Flag::Hidden;
        if let Some(m) = marks {
            let (g, c) = match m[i] {
                Cmp::Same => ("M3 8h10", pal.faint),
                Cmp::LeftOnly => (glyph::RIGHT, green(pal)),
                Cmp::RightOnly => (glyph::BACK, green(pal)),
                Cmp::LeftNewer => (glyph::FORWARD, blue(pal)),
                Cmp::RightNewer => (glyph::BACK, blue(pal)),
                Cmp::Differs => ("M4 5.5h8M4 10.5h8", amber(pal)),
            };
            p.icon(g, 11.0, rr.x + 22.0, cy, c);
        }
        if e.name.is_empty() {
            p.text(Face::Ui, 12.0, rr.x + 44.0 + gutter, cy, "(missing here)", pal.faint, 140.0);
            continue;
        }
        small_icon(p, pal, e, rr.x + 24.0 + gutter + 2.0, cy);
        let tc = if faint { pal.faint } else { pal.text };
        let nw = p.text(Face::Ui, 13.0, rr.x + 44.0 + gutter, cy, e.name, tc, type_x.min(date_x) - rr.x - 54.0 - gutter);
        match e.flag {
            Flag::Cloud => {
                let cw = p.measure(Face::Ui, 11.0, "online only") + 12.0;
                let cr = Rect::new(rr.x + 44.0 + gutter + nw + 8.0, cy - 8.0, cw, 16.0);
                p.rbox(cr, 8.0, mix(pal.page, blue(pal), 0.2));
                p.text_center(Face::Ui, 11.0, cr, "online only", blue(pal));
            }
            Flag::ReadOnly => {
                p.text(Face::Ui, 11.0, rr.x + 52.0 + gutter + nw, cy, "read-only", pal.faint, 80.0);
            }
            _ => {}
        }
        p.text(Face::Ui, 12.5, date_x + 8.0, cy, e.date, pal.muted, date_w - 8.0);
        if wide {
            p.text(Face::Ui, 12.5, type_x + 8.0, cy, d::type_name(e.k), pal.muted, type_w - 8.0);
        }
        // Size: a folder's is computed on a worker, and says so until it is.
        let cell = Rect::new(size_x, rr.y + 3.0, size_w - 2.0, rr.h - 6.0);
        match (e.k, e.size) {
            (K::Dir, NOT_COMPUTED) => {
                p.text_right(Face::Ui, 12.5, cell.right() - 4.0, cy, "\u{2014}", pal.faint);
            }
            (K::Dir, COMPUTING) => {
                p.icon(glyph::RELOAD, 11.0, cell.x + cell.w - 60.0, cy, pal.accent);
                p.text_right(Face::Ui, 12.0, cell.right() - 4.0, cy, "counting", pal.muted);
            }
            (K::Dir, s) => {
                let f = (s as f32 / max_dir as f32).clamp(0.02, 1.0);
                p.rbox_a(Rect::new(cell.x, cell.y, cell.w * f, cell.h), 3.0, pal.accent, 0.20);
                p.text_right(Face::Ui, 12.5, cell.right() - 4.0, cy, &d::human(s), pal.text);
            }
            (_, s) => {
                p.text_right(Face::Ui, 12.5, cell.right() - 4.0, cy, &d::human(s), pal.muted);
            }
        }
    }
    p.restore(old);
    rows
}

fn list(p: &mut Painter, pal: &Palette, r: Rect, es: &[E], sel: &[usize], active: bool) -> Vec<Rect> {
    let old = p.clip(r);
    let colw = 232.0;
    let per = ((r.h - 12.0) / 24.0).floor().max(1.0) as usize;
    let mut rows = Vec::new();
    // The same entries as often as it takes to fill the view, as a long folder would.
    let mut all: Vec<(usize, E)> = es.iter().copied().enumerate().collect();
    let base = all.len();
    for k in 0..(per * 3).saturating_sub(base) {
        let mut e = es[k % es.len()];
        let names = ["archive", "build", "notes", "old", "report", "scratch", "tmp", "todo"];
        e.name = names[k % names.len()];
        all.push((base + k, e));
    }
    for (n, (i, e)) in all.iter().enumerate() {
        let col = n / per;
        let row = n % per;
        let rr = Rect::new(r.x + 8.0 + col as f32 * colw, r.y + 6.0 + row as f32 * 24.0, colw - 8.0, 24.0);
        if rr.x > r.right() {
            break;
        }
        if sel.contains(i) && *i < base {
            p.rbox(rr, 4.0, sel_fill(pal, active));
        }
        small_icon(p, pal, e, rr.x + 14.0, rr.cy());
        p.text(
            Face::Ui,
            13.0,
            rr.x + 30.0,
            rr.cy(),
            e.name,
            if e.flag == Flag::Hidden { pal.faint } else { pal.text },
            rr.w - 36.0,
        );
        if *i < base {
            rows.push(rr);
        }
    }
    // Column rules.
    p.restore(old);
    rows
}

fn thumb_colors(i: usize, dark: bool) -> [Rgb; 3] {
    let sets: [[Rgb; 3]; 6] = [
        [[0x6a, 0xa8, 0xe0], [0xe8, 0xc0, 0x80], [0x3a, 0x6a, 0x4a]],
        [[0xe0, 0x8a, 0x6a], [0xf0, 0xd0, 0x8a], [0x4a, 0x4a, 0x6a]],
        [[0x8a, 0xc0, 0xe8], [0xc8, 0xe0, 0xf0], [0x6a, 0x8a, 0x5a]],
        [[0x4a, 0x6a, 0xa0], [0xe0, 0x9a, 0x5a], [0x2a, 0x3a, 0x5a]],
        [[0xe0, 0xb0, 0x8a], [0xd0, 0x7a, 0x6a], [0x5a, 0x4a, 0x3a]],
        [[0x2a, 0x2a, 0x5a], [0xe0, 0x6a, 0x4a], [0x1a, 0x1a, 0x2a]],
    ];
    let mut c = sets[i % 6];
    if dark {
        for x in c.iter_mut() {
            *x = mix(*x, [0, 0, 0], 0.12);
        }
    }
    c
}

/// A synthetic thumbnail of the picture `i` (a sky, a sun, a hill) in `r`.
pub fn thumb(p: &mut Painter, r: Rect, i: usize, dark: bool) {
    let c = thumb_colors(i, dark);
    let old = p.clip(r);
    p.rbox(r, 4.0, c[0]);
    p.rbox(Rect::new(r.x, r.y + r.h * 0.45, r.w, r.h * 0.55), 4.0, mix(c[0], c[1], 0.5));
    let s = r.h * 0.22;
    p.rbox(Rect::new(r.x + r.w * (0.55 + 0.05 * (i % 3) as f32), r.y + r.h * 0.18, s, s), s / 2.0, c[1]);
    p.polygon(
        &[
            (r.x, r.bottom()),
            (r.x + r.w * 0.25, r.y + r.h * 0.5),
            (r.x + r.w * 0.55, r.bottom()),
        ],
        c[2],
    );
    p.polygon(
        &[
            (r.x + r.w * 0.35, r.bottom()),
            (r.x + r.w * 0.7, r.y + r.h * 0.6),
            (r.right(), r.bottom()),
        ],
        mix(c[2], c[0], 0.2),
    );
    p.restore(old);
}

fn icons(p: &mut Painter, pal: &Palette, r: Rect, es: &[E], sel: &[usize], active: bool) -> Vec<Rect> {
    let old = p.clip(r);
    let (tw, th) = (126.0, 124.0);
    let per = ((r.w - 8.0) / tw).floor().max(1.0) as usize;
    let mut rows = Vec::new();
    for (i, e) in es.iter().enumerate() {
        let (cx, cy) = (i % per, i / per);
        let t = Rect::new(r.x + 8.0 + cx as f32 * tw, r.y + 8.0 + cy as f32 * th, tw - 6.0, th - 6.0);
        rows.push(t);
        if t.y > r.bottom() {
            continue;
        }
        if sel.contains(&i) {
            p.rbox(t, 6.0, sel_fill(pal, active));
        }
        let ib = Rect::new(t.x + 11.0, t.y + 8.0, t.w - 22.0, 66.0);
        match e.k {
            K::Img => {
                thumb(p, ib, i, pal.dark);
                p.frame(ib, 4.0, [0, 0, 0], pal.line);
                thumb(p, ib, i, pal.dark);
            }
            K::Dir => {
                p.icon(glyph::FOLDER, 58.0, ib.cx(), ib.cy(), pal.folder);
            }
            K::Mp4 => {
                p.rbox(ib, 4.0, [0x22, 0x26, 0x30]);
                p.rbox_a(ib, 4.0, [255, 255, 255], 0.04);
                p.rbox(Rect::new(ib.cx() - 15.0, ib.cy() - 15.0, 30.0, 30.0), 15.0, [0xff, 0xff, 0xff]);
                p.icon(glyph::PLAY, 14.0, ib.cx() + 1.0, ib.cy(), [0x22, 0x26, 0x30]);
            }
            _ => {
                let pg = Rect::new(ib.cx() - 24.0, ib.y, 48.0, ib.h);
                p.frame(pg, 4.0, mix(pal.page, [255, 255, 255], if pal.dark { 0.12 } else { 0.9 }), pal.line);
                p.fill(Rect::new(pg.x, pg.y + 8.0, pg.w, 6.0), d::tint(e.k));
                for k in 0..4 {
                    p.fill(Rect::new(pg.x + 7.0, pg.y + 22.0 + k as f32 * 8.0, pg.w - 14.0 - (k % 2) as f32 * 8.0, 2.0), pal.faint);
                }
            }
        }
        let name = p.elide(Face::Ui, 12.5, e.name, t.w - 8.0);
        p.text_center(Face::Ui, 12.5, Rect::new(t.x, t.y + 80.0, t.w, 18.0), &name, pal.text);
        let sub = if e.k == K::Dir { String::new() } else { d::human(e.size) };
        p.text_center(Face::Ui, 11.0, Rect::new(t.x, t.y + 98.0, t.w, 16.0), &sub, pal.faint);
        if e.flag == Flag::Cloud {
            p.rbox(Rect::new(t.right() - 22.0, t.y + 6.0, 16.0, 16.0), 8.0, pal.page);
            p.icon(glyph::GLOBE, 11.0, t.right() - 14.0, t.y + 14.0, blue(pal));
        }
    }
    p.restore(old);
    rows
}

// ---- compare and the bin ----------------------------------------------------------------------

fn compare(p: &mut Painter, pal: &Palette, r: Rect, active: bool, side: usize) -> Vec<Rect> {
    let left = d::compare_left();
    let marks: Vec<Cmp> = left.iter().map(|x| x.1).collect();
    let blank = E { name: "", k: K::Txt, size: 0, date: "", flag: Flag::None };
    let es: Vec<E> = if side == 0 {
        left.iter().map(|x| x.0.unwrap_or(blank)).collect()
    } else {
        d::compare_right().into_iter().map(|e| e.unwrap_or(blank)).collect()
    };
    details(p, pal, r, &es, &[10], None, active, &[], Some(&marks))
}

fn compare_strip(p: &mut Painter, pal: &Palette, r: Rect) {
    p.fill(r, pal.bar);
    p.fill(Rect::new(r.x, r.y, r.w, p.px().max(1.0)), pal.line);
    let cy = r.cy();
    let mut x = 16.0;
    for (g, c, t) in [
        (glyph::FORWARD, blue(pal), "4 newer on the left"),
        (glyph::BACK, blue(pal), "1 newer on the right"),
        (glyph::RIGHT, green(pal), "1 only on the left"),
        ("M4 5.5h8M4 10.5h8", amber(pal), "1 differs"),
        ("M3 8h10", pal.faint, "5 same"),
    ] {
        p.icon(g, 11.0, x + 5.0, cy, c);
        x += 16.0 + p.text(Face::Ui, 12.0, x + 16.0, cy, t, pal.text, 200.0) + 14.0;
    }
    let mut rx = r.right() - 14.0;
    for (t, prim) in [("Synchronise\u{2026}", true), ("Copy to right", false), ("Compare contents", false)] {
        let b = super::ui::sbtn_r(p, pal, rx, cy, t, prim, true);
        rx = b.x - 8.0;
    }
}

// ---- the preview pane -------------------------------------------------------------------------

fn preview(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, pv: Prev) {
    p.fill(r, pal.pane);
    p.fill(Rect::new(r.x, r.y, p.px().max(1.0), r.h), pal.line);
    let old = p.clip(r);
    let x = r.x + 18.0;
    let w = r.w - 36.0;
    let mut y = r.y + 16.0;
    match pv {
        Prev::Md => {
            p.icon(glyph::DOC, 15.0, x + 6.0, y + 9.0, d::tint(K::Md));
            p.text(Face::Semibold, 13.5, x + 22.0, y + 9.0, "README.md", pal.text, w - 24.0);
            y += 22.0;
            p.text(Face::Ui, 11.5, x, y + 8.0, "Markdown \u{b7} 4.8 KB \u{b7} 2026-11-21 18:02", pal.muted, w);
            y += 26.0;
            p.fill(Rect::new(x, y, w, p.px().max(1.0)), pal.line);
            y += 14.0;
            p.text(Face::Semibold, 20.0, x, y + 12.0, "mdedit", pal.text, w);
            y += 34.0;
            let lines = [
                "A light, fast text editor with syntax highlighting, the companion of mdview.",
                "",
                "Opens any text file. Saves it in its own encoding and line endings.",
            ];
            for l in lines {
                if l.is_empty() {
                    y += 8.0;
                    continue;
                }
                y = super::ui::para(p, Face::Ui, 12.5, x, y, w, l, pal.text) + 2.0;
            }
            y += 8.0;
            p.text(Face::Semibold, 14.0, x, y + 8.0, "Install", pal.text, w);
            y += 26.0;
            let code = Rect::new(x, y, w, 62.0);
            p.rbox(code, 5.0, mix(pal.pane, pal.text, 0.06));
            for (k, (s, c)) in [
                ("cargo build --release", pal.text),
                ("target\\release\\mdedit.exe", blue(pal)),
                ("  --no-session --behind", pal.muted),
            ]
            .into_iter()
            .enumerate()
            {
                p.text(Face::Mono, 11.5, code.x + 10.0, code.y + 14.0 + k as f32 * 17.0, s, c, w - 16.0);
            }
            y += 76.0;
            for s in ["Fast to open and light", "No network, except its own updates", "Not an IDE"] {
                p.fill_a(Rect::new(x + 2.0, y + 8.0, 4.0, 4.0), pal.muted, 1.0);
                p.text(Face::Ui, 12.5, x + 14.0, y + 10.0, s, pal.text, w - 16.0);
                y += 20.0;
            }
            y += 8.0;
            p.text(Face::Ui, 11.0, x, y + 8.0, "Rendered by mdview's renderer", pal.faint, w);
        }
        Prev::Img => {
            let ib = Rect::new(x, y, w, w * 0.66);
            thumb(p, ib, 3, pal.dark);
            y += ib.h + 14.0;
            p.text(Face::Semibold, 13.5, x, y + 9.0, "harbour.png", pal.text, w);
            y += 26.0;
            for (k, v) in [
                ("Type", "PNG image"),
                ("Size", "4.2 MB"),
                ("Dimensions", "4032 \u{d7} 3024"),
                ("Taken", "2026-10-04 12:02"),
                ("Modified", "2026-10-04 12:02"),
            ] {
                p.text(Face::Ui, 12.0, x, y + 9.0, k, pal.muted, 90.0);
                p.text(Face::Ui, 12.0, x + 90.0, y + 9.0, v, pal.text, w - 90.0);
                y += 20.0;
            }
        }
        Prev::Many => {
            p.text(Face::Semibold, 14.0, x, y + 9.0, "3 items selected", pal.text, w);
            y += 24.0;
            p.text(Face::Ui, 12.0, x, y + 9.0, "21.4 KB in all", pal.muted, w);
            y += 30.0;
            for e in d::project().iter().filter(|e| ["Cargo.toml", "CLAUDE.md", "README.md"].contains(&e.name)) {
                small_icon(p, pal, e, x + 8.0, y + 10.0);
                p.text(Face::Ui, 12.5, x + 24.0, y + 10.0, e.name, pal.text, w - 90.0);
                p.text_right(Face::Ui, 11.5, x + w, y + 10.0, &d::human(e.size), pal.muted);
                y += 22.0;
            }
        }
    }
    let _ = sc;
    p.restore(old);
}

// ---- the copy queue's strip and the status bar ------------------------------------------------

fn queue_strip(p: &mut Painter, pal: &Palette, r: Rect) {
    p.fill(r, pal.bar);
    p.fill(Rect::new(r.x, r.y, r.w, p.px().max(1.0)), pal.line);
    let cy = r.cy();
    p.icon(glyph::COPY, 13.0, 18.0, cy, pal.accent);
    p.text(Face::Semibold, 12.5, 34.0, cy, "Copying 1,204 files to E:\\Backup", pal.text, 260.0);
    let b = Rect::new(310.0, cy - 3.0, (r.w - 310.0 - 330.0).max(80.0), 6.0);
    super::ui::progress(p, pal, b, 0.38, pal.accent, pal.bar);
    p.text(Face::Ui, 12.0, b.right() + 12.0, cy, "38%  \u{b7}  46 MB/s  \u{b7}  2 min left", pal.muted, 200.0);
    let px = r.right() - 28.0;
    p.icon(PAUSE, 13.0, px - 30.0, cy, pal.muted);
    p.icon(glyph::CLOSE, 11.0, px, cy, pal.muted);
}

fn status(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, w: f32) {
    p.fill(r, pal.bar);
    p.fill(Rect::new(r.x, r.y, r.w, p.px().max(1.0)), pal.line);
    let cy = r.cy();
    let ps = &sc.panes[sc.active];
    let text = if let Some(t) = sc.status {
        t.to_string()
    } else {
        match ps.content {
            Content::Project => "17 items  \u{b7}  3 selected, 21.4 KB".to_string(),
            Content::Downloads => "9 items  \u{b7}  1 selected, 188 KB".to_string(),
            Content::Pictures => "12 items  \u{b7}  1 selected, 4.2 MB".to_string(),
            Content::Compare => "12 pairs  \u{b7}  1 selected".to_string(),
            Content::Find => "6 matches in 4,812 files  \u{b7}  searched in 0.4 s".to_string(),
            Content::Bin => "14 items".to_string(),
        }
    };
    p.text(Face::Ui, 12.0, 16.0, cy, &text, pal.muted, w * 0.5);
    let sw = 3.0 * 26.0;
    let mut x = r.right() - 12.0 - sw;
    for (g, v) in [(VIEW_DETAILS, View::Details), (VIEW_LIST, View::List), (VIEW_ICONS, View::Icons)] {
        let b = Rect::new(x, cy - 10.0, 26.0, 20.0);
        if ps.view == v {
            p.rbox(b, 4.0, pal.on);
        }
        p.icon(g, 12.0, b.cx(), cy, if ps.view == v { pal.accent } else { pal.muted });
        x += 26.0;
    }
    let fr = "142 GB free on C:";
    let fw = p.measure(Face::Ui, 12.0, fr);
    p.text(Face::Ui, 12.0, r.right() - sw - 28.0 - fw, cy, fr, pal.muted, 200.0);
}
