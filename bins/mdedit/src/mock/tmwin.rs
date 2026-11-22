//! The task manager window: title row with page tabs, a toolbar, the content of one page, the
//! status bar with the app's own cost. This file holds the frame and the Processes page (grouped,
//! tree and flat) and its details pane; the other pages are in `tmpages.rs`.

use super::tmdata::{self as d, G, P, Sig, St};
use super::ui;
use mdshared_ui::chrome::{
    DRAG_MIN, TABS_LEFT, TITLE_H, TabState, WIN_BTN_W, WinButton, draw_tab, draw_win_button,
    tab_natural_width, tab_widths, win_button_rect,
};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

pub const TOOL_H: f32 = 42.0;
pub const STATUS_H: f32 = 28.0;
pub const HEAD_H: f32 = 46.0;
pub const ROW_H: f32 = 28.0;
pub const COMPACT_W: f32 = 1000.0;

pub const SHIELD: &str = "M8 1.8l5 1.8v4c0 3.2-2.1 5.4-5 6.6-2.9-1.2-5-3.4-5-6.6v-4z";
pub const PAUSE: &str = "M5 3h2v10H5zM9 3h2v10H9z";
pub const PIN_ON: &str = "M5.5 2.5h5M6.5 2.5v3.5L4.5 9.5h7L9.5 6V2.5M8 9.5v4";
pub const MINI: &str = "M2.5 2.5h11v11h-11zM2.5 6.5h11";
pub const TREE_G: &str = "M3 3h4M5 3v9M5 7h3.5M5 12h3.5M10 5.5h3.5v3H10zM10 10.5h3.5v3H10z";
pub const LIST_G: &str = "M2.5 3.5h11M2.5 8h11M2.5 12.5h11";
pub const GROUP_G: &str = "M2.5 3h11M2.5 6h6M2.5 10h11M2.5 13h6";
pub const SIDEBAR_R: &str = glyph::SIDEBAR;
pub const KILL: &str = "M3.5 3.5l9 9M12.5 3.5l-9 9";

#[derive(Clone, Copy, PartialEq)]
pub enum Page {
    Processes,
    Performance,
    Startup,
    Services,
    Files,
}

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    Grouped,
    Tree,
    Flat,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Cols {
    Default,
    Tree,
    Wide,
    Small,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DTab {
    Overview,
    Modules,
    Handles,
    Network,
    Environment,
}

#[derive(Clone)]
pub struct Scene {
    pub page: Page,
    pub view: View,
    pub cols: Cols,
    pub sel: u32,
    pub hover: Option<u32>,
    pub collapsed: Vec<u32>,
    pub details: Option<(DTab, bool)>,
    pub sort: &'static str,
    pub search: &'static str,
    pub elevated: bool,
    pub paused: bool,
}

impl Default for Scene {
    fn default() -> Scene {
        Scene {
            page: Page::Processes,
            view: View::Grouped,
            cols: Cols::Default,
            sel: 12712,
            hover: None,
            collapsed: vec![10312, 7280],
            details: None,
            sort: "cpu",
            search: "",
            elevated: false,
            paused: false,
        }
    }
}

pub fn blue(pal: &Palette) -> Rgb {
    if pal.dark {
        [0x6c, 0xb6, 0xff]
    } else {
        [0x00, 0x67, 0xc0]
    }
}
pub fn green(pal: &Palette) -> Rgb {
    if pal.dark {
        [0x6c, 0xcb, 0x5f]
    } else {
        [0x0f, 0x7b, 0x0f]
    }
}
pub fn amber(pal: &Palette) -> Rgb {
    if pal.dark {
        [0xfc, 0xc4, 0x00]
    } else {
        [0x9d, 0x5d, 0x00]
    }
}
pub fn red(pal: &Palette) -> Rgb {
    if pal.dark {
        [0xff, 0x99, 0xa4]
    } else {
        [0xc4, 0x2b, 0x1c]
    }
}

pub fn tile(p: &mut Painter, r: Rect) {
    p.rbox(r, r.w * 0.22, [0x6a, 0x4f, 0xc4]);
    p.rbox_a(
        Rect::new(r.x, r.y, r.w, r.h / 2.0),
        r.w * 0.22,
        [255, 255, 255],
        0.08,
    );
    let bw = r.w * 0.12;
    for (i, hgt) in [0.30, 0.55, 0.40, 0.72].iter().enumerate() {
        let x = r.x + r.w * 0.20 + i as f32 * r.w * 0.17;
        let hh = r.h * hgt;
        p.rbox(
            Rect::new(x, r.bottom() - r.h * 0.2 - hh, bw, hh),
            1.0,
            [255, 255, 255],
        );
    }
}

pub fn page_labels() -> [&'static str; 5] {
    [
        "Processes",
        "Performance",
        "Startup",
        "Services",
        "Files in use",
    ]
}

pub fn page_index(pg: Page) -> usize {
    match pg {
        Page::Processes => 0,
        Page::Performance => 1,
        Page::Startup => 2,
        Page::Services => 3,
        Page::Files => 4,
    }
}

pub fn title(p: &mut Painter, pal: &Palette, w: f32, active: usize) {
    p.fill(Rect::new(0.0, 0.0, w, TITLE_H), pal.title);
    tile(p, Rect::new(14.0, 9.0, 22.0, 22.0));
    let mut right = w;
    for kind in [WinButton::Close, WinButton::Max, WinButton::Min] {
        let b = win_button_rect(right, 0);
        right -= WIN_BTN_W;
        draw_win_button(p, pal, kind, b, false, false, false);
    }
    // Always on top, as a pin before the window buttons.
    p.icon(PIN_ON, 14.0, right - 22.0, 20.0, pal.muted);
    right -= 40.0;
    let labels = page_labels();
    let avail = (right - DRAG_MIN - TABS_LEFT - 10.0).max(0.0);
    let natural: Vec<f32> = labels.iter().map(|l| tab_natural_width(p, l)).collect();
    let widths = tab_widths(&natural, avail);
    let mut x = TABS_LEFT;
    for (i, label) in labels.iter().enumerate() {
        let st = TabState {
            active: active == i,
            next_quiet: i + 1 < labels.len() && active != i && active != i + 1,
            ..TabState::default()
        };
        draw_tab(p, pal, x, widths[i], w, label, &st);
        x += widths[i];
    }
}

pub fn sep(p: &mut Painter, pal: &Palette, x: &mut f32, cy: f32) {
    p.fill(
        Rect::new((*x + 3.0).round(), cy - 10.0, p.px().max(1.0), 20.0),
        pal.line,
    );
    *x += 9.0;
}

pub fn tbtn(
    p: &mut Painter,
    pal: &Palette,
    x: &mut f32,
    cy: f32,
    g: &str,
    label: Option<&str>,
    on: bool,
    danger: bool,
) -> Rect {
    let wd = match label {
        Some(l) => 40.0 + p.measure(Face::Ui, 13.0, l),
        None => 32.0,
    };
    let b = Rect::new(*x, cy - 14.0, wd, 28.0);
    let c = if !on {
        pal.faint
    } else if danger {
        red(pal)
    } else {
        pal.text
    };
    p.icon(
        g,
        15.0,
        if label.is_some() { b.x + 18.0 } else { b.cx() },
        cy,
        c,
    );
    if let Some(l) = label {
        p.text(Face::Ui, 13.0, b.x + 32.0, cy, l, c, 200.0);
    }
    *x += wd + 2.0;
    b
}

/// A segmented control; returns its right edge.
pub fn seg(
    p: &mut Painter,
    pal: &Palette,
    x: f32,
    cy: f32,
    items: &[(&str, &str)],
    active: usize,
    icons_only: bool,
) -> f32 {
    let widths: Vec<f32> = items
        .iter()
        .map(|(g, l)| {
            if icons_only {
                34.0
            } else {
                let _ = g;
                p.measure(Face::Ui, 12.5, l) + 40.0
            }
        })
        .collect();
    let total: f32 = widths.iter().sum();
    let r = Rect::new(x, cy - 14.0, total, 28.0);
    p.frame(r, 6.0, mix(pal.page, pal.text, 0.03), pal.line);
    let mut xx = x;
    for (i, ((g, l), wd)) in items.iter().zip(&widths).enumerate() {
        let b = Rect::new(xx + 2.0, cy - 12.0, wd - 4.0, 24.0);
        if i == active {
            p.rbox(b, 4.0, mix(pal.page, pal.accent, 0.20));
        }
        let c = if i == active { pal.text } else { pal.muted };
        if icons_only {
            p.icon(g, 14.0, b.cx(), cy, c);
        } else {
            p.icon(g, 14.0, b.x + 14.0, cy, c);
            p.text(Face::Ui, 12.5, b.x + 28.0, cy, l, c, 120.0);
        }
        xx += wd;
    }
    r.right()
}

pub fn search_field(p: &mut Painter, pal: &Palette, r: Rect, text: &str, hint: &str) {
    p.frame(r, 6.0, pal.field, pal.line);
    p.icon(glyph::SEARCH, 13.0, r.x + 16.0, r.cy(), pal.muted);
    if text.is_empty() {
        p.text(
            Face::Ui,
            13.0,
            r.x + 32.0,
            r.cy(),
            hint,
            pal.faint,
            r.w - 44.0,
        );
    } else {
        p.text(
            Face::Ui,
            13.0,
            r.x + 32.0,
            r.cy(),
            text,
            pal.text,
            r.w - 44.0,
        );
        p.fill(
            Rect::new(r.x + 4.0, r.bottom() - 2.0, r.w - 8.0, 2.0),
            pal.accent,
        );
        p.icon(glyph::CLOSE, 10.0, r.right() - 16.0, r.cy(), pal.muted);
    }
}

pub fn heat(pal: &Palette, base: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    if t < 0.01 {
        return base;
    }
    mix(base, pal.accent, 0.07 + 0.40 * t)
}

// ---- the frame --------------------------------------------------------------------------------

/// Totals of the machine, as the status bar and the column heads show them.
pub struct Totals {
    pub cpu: f32,
    pub mem_pct: f32,
    pub disk: f32,
    pub net: f32,
    pub gpu: f32,
}
pub const TOT: Totals = Totals {
    cpu: 11.0,
    mem_pct: 54.0,
    disk: 2.0,
    net: 1.0,
    gpu: 14.0,
};

pub fn status_bar(p: &mut Painter, pal: &Palette, w: f32, h: f32, sc: &Scene) {
    let r = Rect::new(0.0, h - STATUS_H, w, STATUS_H);
    p.fill(r, pal.bar);
    p.fill(Rect::new(0.0, r.y, w, p.px().max(1.0)), pal.line);
    let cy = r.cy();
    let compact = w < COMPACT_W;
    let mut x = 14.0;
    let items: Vec<String> = if compact {
        vec!["CPU 11%".into(), "Memory 17.3 GB".into(), "Disk 2%".into()]
    } else {
        vec![
            "CPU 11%".into(),
            "Memory 17.3 of 32 GB".into(),
            "Disk 2%".into(),
            "Network 1.2 Mbps".into(),
            "GPU 14%".into(),
            "147 processes".into(),
        ]
    };
    for s in &items {
        let wd = p.text(Face::Ui, 12.0, x, cy, s, pal.muted, 220.0);
        x += wd + 18.0;
    }
    // This app's own cost, right.
    let mut right = w - 14.0;
    let (shield, lab) = if sc.elevated {
        (green(pal), "Administrator")
    } else {
        (pal.muted, "Standard user")
    };
    let wd = p.measure(Face::Ui, 12.0, lab);
    p.text(
        Face::Ui,
        12.0,
        right - wd,
        cy,
        lab,
        if sc.elevated { shield } else { pal.muted },
        wd + 4.0,
    );
    right -= wd + 20.0;
    p.icon(SHIELD, 13.0, right + 8.0, cy, shield);
    right -= 14.0;
    let own = if sc.paused {
        "mdtask paused \u{b7} 0 ms".to_string()
    } else if compact {
        "mdtask 0.0% \u{b7} 17.8 MB".to_string()
    } else {
        "Sampling 1 s \u{b7} mdtask 0.0% CPU \u{b7} 17.8 MB \u{b7} 1.1 ms a sample".to_string()
    };
    let wd = p.measure(Face::Ui, 12.0, &own);
    let c = if sc.paused { amber(pal) } else { pal.muted };
    p.text(Face::Ui, 12.0, right - wd, cy, &own, c, wd + 4.0);
    p.fill(
        Rect::new(right - wd - 12.0, cy - 8.0, p.px().max(1.0), 16.0),
        pal.line,
    );
}

pub fn window(p: &mut Painter, pal: &Palette, w: f32, h: f32, sc: &Scene) -> Vec<(u32, Rect)> {
    p.fill(Rect::new(0.0, 0.0, w, h), pal.page);
    title(p, pal, w, page_index(sc.page));
    let body = Rect::new(0.0, TITLE_H + TOOL_H, w, h - TITLE_H - TOOL_H - STATUS_H);
    let tool = Rect::new(0.0, TITLE_H, w, TOOL_H);
    let mut rows = Vec::new();
    match sc.page {
        Page::Processes => {
            toolbar_procs(p, pal, tool, sc, w);
            rows = procs_page(p, pal, body, sc);
        }
        Page::Performance => super::tmpages::perf_tool(p, pal, tool, w),
        Page::Startup => super::tmpages::startup_tool(p, pal, tool, w),
        Page::Services => super::tmpages::services_tool(p, pal, tool, w),
        Page::Files => super::tmpages::files_tool(p, pal, tool, w),
    }
    p.fill(
        Rect::new(0.0, tool.bottom() - p.px().max(1.0), w, p.px().max(1.0)),
        pal.line,
    );
    status_bar(p, pal, w, h, sc);
    rows
}

fn toolbar_procs(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, w: f32) {
    p.fill(r, pal.page);
    let cy = r.cy();
    let compact = w < COMPACT_W;
    let sw = if compact { 170.0 } else { 250.0 };
    search_field(
        p,
        pal,
        Rect::new(10.0, cy - 15.0, sw, 30.0),
        sc.search,
        "Search processes (Ctrl+F)",
    );
    let mut x = 10.0 + sw + 12.0;
    let active = match sc.view {
        View::Grouped => 0,
        View::Tree => 1,
        View::Flat => 2,
    };
    x = seg(
        p,
        pal,
        x,
        cy,
        &[(GROUP_G, "Grouped"), (TREE_G, "Tree"), (LIST_G, "Flat")],
        active,
        compact,
    ) + 8.0;
    sep(p, pal, &mut x, cy);
    let has = true;
    tbtn(
        p,
        pal,
        &mut x,
        cy,
        KILL,
        if compact { None } else { Some("End task") },
        has,
        true,
    );
    tbtn(
        p,
        pal,
        &mut x,
        cy,
        PAUSE,
        if compact { None } else { Some("Suspend") },
        has,
        false,
    );
    tbtn(
        p,
        pal,
        &mut x,
        cy,
        glyph::SLIDERS,
        if compact { None } else { Some("Priority") },
        has,
        false,
    );
    // Right side: columns, details pane, pause sampling, compact mode.
    let mut rx = w - 8.0;
    for (g, on) in [
        (glyph::FULLSCREEN, false),
        (PAUSE, sc.paused),
        (SIDEBAR_R, sc.details.is_some()),
    ] {
        rx -= 34.0;
        let b = Rect::new(rx, cy - 14.0, 32.0, 28.0);
        if on {
            p.rbox(b, 6.0, mix(pal.page, pal.accent, 0.2));
        }
        p.icon(g, 15.0, b.cx(), cy, if on { pal.text } else { pal.muted });
    }
    rx -= 34.0;
    p.icon(glyph::MORE, 15.0, rx + 16.0, cy, pal.muted);
}

// ---- the process table ------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum C {
    Name,
    Pid,
    Status,
    Cpu,
    Mem,
    Commit,
    Disk,
    Net,
    Gpu,
    Handles,
    Threads,
    User,
    Publisher,
}

fn columns(c: Cols) -> Vec<(C, f32)> {
    match c {
        Cols::Default => vec![
            (C::Name, 0.0),
            (C::Pid, 64.0),
            (C::Status, 104.0),
            (C::Cpu, 78.0),
            (C::Mem, 96.0),
            (C::Disk, 86.0),
            (C::Net, 86.0),
            (C::Gpu, 76.0),
        ],
        Cols::Tree => vec![
            (C::Name, 0.0),
            (C::Pid, 64.0),
            (C::Cpu, 74.0),
            (C::Mem, 92.0),
            (C::Disk, 82.0),
            (C::Net, 80.0),
            (C::Gpu, 70.0),
            (C::Publisher, 168.0),
        ],
        Cols::Wide => vec![
            (C::Name, 0.0),
            (C::Pid, 60.0),
            (C::Cpu, 72.0),
            (C::Mem, 88.0),
            (C::Commit, 88.0),
            (C::Disk, 80.0),
            (C::Net, 78.0),
            (C::Gpu, 66.0),
            (C::Handles, 74.0),
            (C::Threads, 70.0),
            (C::User, 110.0),
            (C::Publisher, 150.0),
        ],
        Cols::Small => vec![
            (C::Name, 0.0),
            (C::Cpu, 66.0),
            (C::Mem, 84.0),
            (C::Disk, 76.0),
            (C::Net, 74.0),
            (C::Gpu, 62.0),
        ],
    }
}

fn head_title(c: C) -> &'static str {
    match c {
        C::Name => "Name",
        C::Pid => "PID",
        C::Status => "Status",
        C::Cpu => "CPU",
        C::Mem => "Memory",
        C::Commit => "Commit",
        C::Disk => "Disk",
        C::Net => "Network",
        C::Gpu => "GPU",
        C::Handles => "Handles",
        C::Threads => "Threads",
        C::User => "User",
        C::Publisher => "Publisher",
    }
}

fn head_total(c: C) -> Option<String> {
    match c {
        C::Cpu => Some(format!("{:.0}%", TOT.cpu)),
        C::Mem => Some(format!("{:.0}%", TOT.mem_pct)),
        C::Disk => Some(format!("{:.0}%", TOT.disk)),
        C::Net => Some(format!("{:.0}%", TOT.net)),
        C::Gpu => Some(format!("{:.0}%", TOT.gpu)),
        _ => None,
    }
}

fn is_num(c: C) -> bool {
    !matches!(c, C::Name | C::Status | C::User | C::Publisher)
}

enum Row {
    Group(&'static str, usize, bool),
    Proc {
        i: usize,
        depth: usize,
        has_kids: bool,
        open: bool,
    },
}

fn push_tree(v: &[P], i: usize, depth: usize, sc: &Scene, out: &mut Vec<Row>, only: Option<G>) {
    let kids: Vec<usize> = if only.is_some() && v[i].pid == 6120 {
        Vec::new()
    } else {
        d::kids(v, v[i].pid)
            .into_iter()
            .filter(|&k| only.is_none_or(|g| v[k].g == g))
            .collect()
    };
    let open = !sc.collapsed.contains(&v[i].pid);
    out.push(Row::Proc {
        i,
        depth,
        has_kids: !kids.is_empty(),
        open,
    });
    if open {
        for k in kids {
            push_tree(v, k, depth + 1, sc, out, only);
        }
    }
}

fn build_rows(v: &[P], sc: &Scene) -> Vec<Row> {
    let mut out = Vec::new();
    let has_filter = !sc.search.is_empty();
    match sc.view {
        View::Tree => {
            for (i, x) in v.iter().enumerate() {
                if x.ppid == 0 || !v.iter().any(|y| y.pid == x.ppid) {
                    push_tree(v, i, 0, sc, &mut out, None);
                }
            }
        }
        View::Grouped => {
            for (g, label) in [
                (G::App, "Apps"),
                (G::Back, "Background processes"),
                (G::Win, "Windows processes"),
            ] {
                let roots: Vec<usize> = v
                    .iter()
                    .enumerate()
                    .filter(|(_, x)| {
                        x.g == g
                            && (x.ppid == 6120 || !v.iter().any(|y| y.pid == x.ppid && y.g == g))
                    })
                    .map(|(i, _)| i)
                    .collect();
                let mut rows = Vec::new();
                for i in roots {
                    push_tree(v, i, 0, sc, &mut rows, Some(g));
                }
                let n = rows
                    .iter()
                    .filter(|r| matches!(r, Row::Proc { depth: 0, .. }))
                    .count();
                // Apps are shown without the Explorer children; a group of Windows processes
                // collapses its service hosts into one line each.
                out.push(Row::Group(label, n, true));
                out.extend(rows);
            }
        }
        View::Flat => {
            let mut idx: Vec<usize> = (0..v.len()).collect();
            if sc.sort == "mem" {
                idx.sort_by(|&a, &b| v[b].mem.total_cmp(&v[a].mem));
            } else {
                idx.sort_by(|&a, &b| v[b].cpu.total_cmp(&v[a].cpu));
            }
            for i in idx {
                out.push(Row::Proc {
                    i,
                    depth: 0,
                    has_kids: false,
                    open: false,
                });
            }
        }
    }
    if has_filter {
        let q = sc.search.to_lowercase();
        out.retain(|r| match r {
            Row::Proc { i, .. } => {
                v[*i].name.to_lowercase().contains(&q) || v[*i].desc.to_lowercase().contains(&q)
            }
            Row::Group(..) => false,
        });
    }
    out
}

fn proc_color(name: &str) -> Rgb {
    let mut h: u32 = 2166136261;
    for b in name.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    let pal: [Rgb; 8] = [
        [0x2a, 0x8a, 0x8a],
        [0x6a, 0x4f, 0xc4],
        [0xd9, 0x6b, 0x2b],
        [0x3b, 0x82, 0xc4],
        [0x4c, 0x9a, 0x4c],
        [0xc2, 0x4d, 0x7a],
        [0x7a, 0x7f, 0x8c],
        [0xb0, 0x8a, 0x1e],
    ];
    pal[(h % 8) as usize]
}

pub fn app_icon(p: &mut Painter, x: f32, cy: f32, name: &str) {
    let r = Rect::new(x, cy - 8.0, 16.0, 16.0);
    p.rbox(r, 4.0, proc_color(name));
    let ch: String = name
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase().to_string())
        .unwrap_or_default();
    p.text_center(Face::Semibold, 10.0, r, &ch, [255, 255, 255]);
}

fn agg(v: &[P], i: usize, collapsed: bool, f: impl Fn(&P) -> f32) -> f32 {
    if collapsed {
        d::subtree(v, i).iter().map(|&k| f(&v[k])).sum()
    } else {
        f(&v[i])
    }
}

pub fn pct(v: f32) -> String {
    if v < 0.05 {
        "0%".into()
    } else {
        format!("{:.1}%", v)
    }
}

/// Draws the table; returns the rectangle of each process row (by pid) for overlays.
pub fn procs_page(p: &mut Painter, pal: &Palette, body: Rect, sc: &Scene) -> Vec<(u32, Rect)> {
    let v = d::procs();
    let detail_w = if sc.details.is_some() {
        if body.w < COMPACT_W { 300.0 } else { 392.0 }
    } else {
        0.0
    };
    let tbl = Rect::new(body.x, body.y, body.w - detail_w, body.h);
    let out = table(p, pal, tbl, sc, &v);
    if let Some((tab, denied)) = sc.details {
        let r = Rect::new(tbl.right(), body.y, detail_w, body.h);
        let i = d::idx(&v, sc.sel);
        details(p, pal, r, &v[i], tab, denied);
    }
    out
}

fn table(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene, v: &[P]) -> Vec<(u32, Rect)> {
    let cols = columns(sc.cols);
    let fixed: f32 = cols.iter().map(|c| c.1).sum();
    let name_w = (r.w - fixed - 6.0).max(180.0);
    let mut xs = Vec::new();
    let mut x = r.x;
    for (c, wd) in &cols {
        let wd = if *c == C::Name { name_w } else { *wd };
        xs.push((*c, x, wd));
        x += wd;
    }
    // Head.
    let head = Rect::new(r.x, r.y, r.w, HEAD_H);
    p.fill(head, pal.page);
    p.fill(
        Rect::new(r.x, head.bottom() - p.px().max(1.0), r.w, p.px().max(1.0)),
        pal.line,
    );
    let sorted = match sc.sort {
        "mem" => C::Mem,
        "name" => C::Name,
        _ => C::Cpu,
    };
    for &(c, x, wd) in &xs {
        let num = is_num(c);
        let pad = 10.0;
        let hot = c == sorted;
        if let Some(t) = head_total(c) {
            p.text_right(
                Face::Semibold,
                15.0,
                x + wd - pad - if hot { 12.0 } else { 0.0 },
                head.y + 15.0,
                &t,
                pal.text,
            );
            p.text_right(
                Face::Ui,
                12.0,
                x + wd - pad - if hot { 12.0 } else { 0.0 },
                head.y + 33.0,
                head_title(c),
                pal.muted,
            );
            if hot {
                p.icon(
                    glyph::DOWN,
                    9.0,
                    x + wd - pad - 3.0,
                    head.y + 33.0,
                    pal.muted,
                );
            }
        } else if num {
            p.text_right(
                Face::Ui,
                12.0,
                x + wd - pad,
                head.y + 33.0,
                head_title(c),
                pal.muted,
            );
        } else {
            let lx = if c == C::Name { x + 14.0 } else { x + pad };
            p.text(
                Face::Ui,
                12.0,
                lx,
                head.y + 33.0,
                head_title(c),
                pal.muted,
                wd - 16.0,
            );
        }
        if c != C::Name || true {
            p.fill(
                Rect::new(x, head.y + 8.0, p.px().max(1.0), HEAD_H - 16.0),
                mix(pal.page, pal.line, 0.5),
            );
        }
    }
    // Body.
    let body = Rect::new(r.x, head.bottom(), r.w, r.bottom() - head.bottom());
    let clip = p.clip(body);
    let rows = build_rows(v, sc);
    let mut y = body.y;
    let mut out = Vec::new();
    for row in &rows {
        if y > body.bottom() {
            break;
        }
        let rr = Rect::new(r.x, y, r.w, ROW_H);
        match row {
            Row::Group(label, n, open) => {
                let rr = Rect::new(r.x, y + 4.0, r.w, ROW_H);
                p.icon(
                    if *open { glyph::DOWN } else { glyph::RIGHT },
                    9.0,
                    r.x + 20.0,
                    rr.cy(),
                    pal.muted,
                );
                let wd = p.text(
                    Face::Semibold,
                    13.0,
                    r.x + 34.0,
                    rr.cy(),
                    label,
                    blue(pal),
                    300.0,
                );
                p.text(
                    Face::Ui,
                    13.0,
                    r.x + 34.0 + wd + 6.0,
                    rr.cy(),
                    &format!("({n})"),
                    pal.muted,
                    60.0,
                );
                y += ROW_H + 4.0;
            }
            Row::Proc {
                i,
                depth,
                has_kids,
                open,
            } => {
                let pr = &v[*i];
                let selected = pr.pid == sc.sel;
                let hovered = sc.hover == Some(pr.pid);
                let bg = if selected {
                    mix(pal.page, pal.accent, 0.16)
                } else if hovered {
                    pal.hover
                } else {
                    pal.page
                };
                p.fill(rr, bg);
                if selected {
                    p.fill(Rect::new(rr.x, rr.y, 3.0, rr.h), pal.accent);
                }
                let collapsed = *has_kids && !*open;
                for &(c, x, wd) in &xs {
                    let cell = Rect::new(x, rr.y, wd, rr.h);
                    let cy = rr.cy();
                    let heat_t = match c {
                        C::Cpu => agg(v, *i, collapsed, |q| q.cpu) / 40.0,
                        C::Mem => agg(v, *i, collapsed, |q| q.mem) / 1600.0,
                        C::Disk => agg(v, *i, collapsed, |q| q.disk) / 10.0,
                        C::Net => agg(v, *i, collapsed, |q| q.net) / 6.0,
                        C::Gpu => agg(v, *i, collapsed, |q| q.gpu) / 30.0,
                        _ => 0.0,
                    };
                    if heat_t > 0.0 {
                        p.fill(
                            Rect::new(cell.x, cell.y, cell.w, cell.h),
                            heat(pal, bg, heat_t),
                        );
                    }
                    let num_x = x + wd - 10.0;
                    let (face, size) = (Face::Ui, 12.5);
                    match c {
                        C::Name => {
                            let ind = 14.0 + *depth as f32 * 18.0;
                            if *has_kids {
                                p.icon(
                                    if *open { glyph::DOWN } else { glyph::RIGHT },
                                    9.0,
                                    x + ind + 5.0,
                                    cy,
                                    pal.muted,
                                );
                            }
                            app_icon(p, x + ind + 16.0, cy, pr.name);
                            let mut tx = x + ind + 40.0;
                            let label = if sc.view == View::Grouped && !pr.desc.is_empty() {
                                pr.desc
                            } else {
                                pr.name
                            };
                            let n_kids = if collapsed {
                                d::subtree(v, *i).len() - 1
                            } else {
                                0
                            };
                            let avail = wd - (tx - x) - 8.0;
                            let used = p.text(
                                Face::Ui,
                                13.0,
                                tx,
                                cy,
                                label,
                                pal.text,
                                avail - if n_kids > 0 { 30.0 } else { 0.0 },
                            );
                            tx += used;
                            if n_kids > 0 {
                                p.text(
                                    Face::Ui,
                                    12.0,
                                    tx + 6.0,
                                    cy,
                                    &format!("({})", n_kids + 1),
                                    pal.muted,
                                    40.0,
                                );
                                tx += 34.0;
                            }
                            if sc.view != View::Grouped
                                && !pr.desc.is_empty()
                                && avail - used > 70.0
                            {
                                p.text(
                                    Face::Ui,
                                    12.0,
                                    tx + 10.0,
                                    cy,
                                    pr.desc,
                                    pal.faint,
                                    (wd - (tx - x) - 18.0).max(0.0),
                                );
                            }
                            if pr.pid == 15244 {
                                ui::chip_on(
                                    p,
                                    bg,
                                    (x + wd - 64.0).max(tx + 8.0),
                                    cy,
                                    "this app",
                                    blue(pal),
                                );
                            }
                        }
                        C::Pid => {
                            p.text_right(face, size, num_x, cy, &pr.pid.to_string(), pal.muted);
                        }
                        C::Status => match pr.st {
                            St::Run => {}
                            St::Suspended => {
                                ui::chip_on(p, bg, x + 10.0, cy, "Suspended", amber(pal));
                            }
                            St::Hung => {
                                ui::chip_on(p, bg, x + 10.0, cy, "Not responding", red(pal));
                            }
                            St::Efficiency => {
                                ui::chip_on(p, bg, x + 10.0, cy, "Efficiency", green(pal));
                            }
                        },
                        C::Cpu => {
                            p.text_right(
                                face,
                                size,
                                num_x,
                                cy,
                                &pct(agg(v, *i, collapsed, |q| q.cpu)),
                                pal.text,
                            );
                        }
                        C::Mem => {
                            p.text_right(
                                face,
                                size,
                                num_x,
                                cy,
                                &d::fmt_mem(agg(v, *i, collapsed, |q| q.mem)),
                                pal.text,
                            );
                        }
                        C::Commit => {
                            p.text_right(
                                face,
                                size,
                                num_x,
                                cy,
                                &d::fmt_mem(agg(v, *i, collapsed, |q| q.commit)),
                                pal.muted,
                            );
                        }
                        C::Disk => {
                            p.text_right(
                                face,
                                size,
                                num_x,
                                cy,
                                &d::fmt_rate(agg(v, *i, collapsed, |q| q.disk), "MB/s"),
                                pal.text,
                            );
                        }
                        C::Net => {
                            let t = if sc.elevated || true {
                                d::fmt_rate(agg(v, *i, collapsed, |q| q.net), "Mbps")
                            } else {
                                "\u{2014}".into()
                            };
                            p.text_right(face, size, num_x, cy, &t, pal.text);
                        }
                        C::Gpu => {
                            p.text_right(
                                face,
                                size,
                                num_x,
                                cy,
                                &pct(agg(v, *i, collapsed, |q| q.gpu)),
                                pal.text,
                            );
                        }
                        C::Handles => {
                            p.text_right(face, size, num_x, cy, &pr.handles.to_string(), pal.muted);
                        }
                        C::Threads => {
                            p.text_right(face, size, num_x, cy, &pr.threads.to_string(), pal.muted);
                        }
                        C::User => {
                            p.text(face, 12.5, x + 10.0, cy, pr.user, pal.muted, wd - 14.0);
                        }
                        C::Publisher => {
                            let (t, c) = match pr.sig {
                                Sig::None => ("Not signed", amber(pal)),
                                _ => (pr.publisher, pal.muted),
                            };
                            p.text(face, 12.5, x + 10.0, cy, t, c, wd - 14.0);
                        }
                    }
                }
                out.push((pr.pid, rr));
                y += ROW_H;
            }
        }
    }
    p.restore(clip);
    out
}

// ---- the details pane -------------------------------------------------------------------------

fn dtab_labels() -> [&'static str; 5] {
    ["Overview", "Modules", "Handles", "Network", "Environment"]
}

fn dtab_index(t: DTab) -> usize {
    match t {
        DTab::Overview => 0,
        DTab::Modules => 1,
        DTab::Handles => 2,
        DTab::Network => 3,
        DTab::Environment => 4,
    }
}

fn details(p: &mut Painter, pal: &Palette, r: Rect, pr: &P, tab: DTab, denied: bool) {
    p.fill(r, pal.pane);
    p.fill(Rect::new(r.x, r.y, p.px().max(1.0), r.h), pal.line);
    // Header.
    let pad = 16.0;
    app_icon(p, r.x + pad, r.y + 26.0, pr.name);
    p.text(
        Face::Semibold,
        14.0,
        r.x + pad + 26.0,
        r.y + 19.0,
        pr.name,
        pal.text,
        r.w - 90.0,
    );
    let sub = if pr.desc.is_empty() { "" } else { pr.desc };
    p.text(
        Face::Ui,
        12.0,
        r.x + pad + 26.0,
        r.y + 37.0,
        sub,
        pal.muted,
        r.w - 70.0,
    );
    p.icon(glyph::CLOSE, 11.0, r.right() - 16.0, r.y + 20.0, pal.muted);
    // Tabs.
    let ty = r.y + 62.0;
    let mut x = r.x + pad;
    let labels = dtab_labels();
    let compact = r.w < 340.0;
    for (i, l) in labels.iter().enumerate() {
        let l = if compact && *l == "Environment" {
            "Env"
        } else {
            l
        };
        let wd = p.measure(Face::Ui, 12.5, l);
        let on = i == dtab_index(tab);
        p.text(
            Face::Ui,
            12.5,
            x,
            ty,
            l,
            if on { pal.text } else { pal.muted },
            wd + 4.0,
        );
        if on {
            p.fill(Rect::new(x, ty + 12.0, wd, 2.0), pal.accent);
        }
        x += wd + if compact { 12.0 } else { 17.0 };
    }
    p.fill(Rect::new(r.x, ty + 14.0, r.w, p.px().max(1.0)), pal.line);
    let body = Rect::new(
        r.x + pad,
        ty + 26.0,
        r.w - 2.0 * pad,
        r.bottom() - ty - 34.0,
    );
    let clip = p.clip(Rect::new(r.x, ty + 15.0, r.w, r.bottom() - ty - 15.0));
    if denied {
        denied_card(p, pal, body, pr);
    } else {
        match tab {
            DTab::Overview => overview(p, pal, body, pr),
            DTab::Modules => modules(p, pal, body),
            DTab::Handles => handles(p, pal, body),
            DTab::Network => network(p, pal, body),
            DTab::Environment => environment(p, pal, body),
        }
    }
    p.restore(clip);
}

fn denied_card(p: &mut Painter, pal: &Palette, r: Rect, pr: &P) {
    let c = Rect::new(r.x, r.y, r.w, 214.0);
    p.frame(c, 8.0, mix(pal.pane, pal.text, 0.03), pal.line);
    p.icon(SHIELD, 22.0, c.x + 28.0, c.y + 34.0, amber(pal));
    p.text(
        Face::Semibold,
        13.5,
        c.x + 52.0,
        c.y + 34.0,
        "Needs administrator rights",
        pal.text,
        c.w - 64.0,
    );
    let y = ui::para(
        p,
        Face::Ui,
        12.5,
        c.x + 16.0,
        c.y + 56.0,
        c.w - 32.0,
        &format!(
            "{} belongs to SYSTEM. A standard user cannot open it to list its modules, handles or environment. The list, the CPU and memory figures above still work: they are read for every process without opening it.",
            pr.name
        ),
        pal.muted,
    );
    let by = y + 20.0;
    ui::sbtn(
        p,
        pal,
        c.x + 16.0,
        by,
        "      Show this once\u{2026}",
        true,
        true,
    );
    p.icon(SHIELD, 13.0, c.x + 16.0 + 20.0, by, pal.on_accent);
    ui::para(
        p,
        Face::Ui,
        12.0,
        c.x + 16.0,
        by + 26.0,
        c.w - 32.0,
        "Windows asks once (UAC). Only this one read runs as administrator; the window stays as it is.",
        pal.faint,
    );
}

fn section(p: &mut Painter, pal: &Palette, x: f32, y: f32, label: &str) -> f32 {
    p.text(
        Face::Semibold,
        11.5,
        x,
        y + 8.0,
        &label.to_uppercase(),
        pal.faint,
        300.0,
    );
    y + 22.0
}

fn spark_card(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    label: &str,
    val: &str,
    series: &[f32],
    col: Rgb,
) {
    p.frame(r, 6.0, pal.page, pal.line);
    p.text(
        Face::Ui,
        11.5,
        r.x + 10.0,
        r.y + 12.0,
        label,
        pal.muted,
        r.w - 20.0,
    );
    p.text_right(
        Face::Semibold,
        12.5,
        r.right() - 10.0,
        r.y + 12.0,
        val,
        pal.text,
    );
    super::tmpages::graph(
        p,
        pal,
        Rect::new(r.x + 6.0, r.y + 22.0, r.w - 12.0, r.h - 28.0),
        series,
        col,
        0.20,
        0,
        false,
    );
}

fn overview(p: &mut Painter, pal: &Palette, r: Rect, pr: &P) {
    let mut y = r.y;
    // Three actions.
    let mut x = r.x;
    for (l, danger) in [("End task", true), ("End tree", false), ("Location", false)] {
        let wd = (p.measure(Face::Ui, 12.5, l) + 22.0).round();
        let b = Rect::new(x, y, wd, 28.0);
        if danger {
            p.frame(
                b,
                5.0,
                mix(pal.page, red(pal), 0.12),
                mix(pal.line, red(pal), 0.5),
            );
            p.text_center(Face::Ui, 12.5, b, l, red(pal));
        } else {
            p.frame(b, 5.0, mix(pal.page, pal.text, 0.04), pal.line);
            p.text_center(Face::Ui, 12.5, b, l, pal.text);
        }
        x += wd + 8.0;
    }
    y += 40.0;
    let hw = (r.w - 8.0) / 2.0;
    let cpu = d::series(60, (pr.cpu / 40.0).clamp(0.02, 0.9), 0.25, pr.pid);
    let mem = d::series(60, 0.55, 0.06, pr.pid + 7);
    spark_card(
        p,
        pal,
        Rect::new(r.x, y, hw, 64.0),
        "CPU",
        &pct(pr.cpu),
        &cpu,
        blue(pal),
    );
    spark_card(
        p,
        pal,
        Rect::new(r.x + hw + 8.0, y, hw, 64.0),
        "Memory",
        &d::fmt_mem(pr.mem),
        &mem,
        [0x8a, 0x5c, 0xc8],
    );
    y += 78.0;
    let lw = 78.0;
    let w = r.w;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "Path",
        if pr.path.is_empty() {
            "C:\\Windows\\System32\\"
        } else {
            pr.path
        },
        false,
    );
    if !pr.cmd.is_empty() {
        y = ui::kv(p, pal, r.x, y, lw, w, "Command", pr.cmd, true);
    }
    let parent = pr.ppid;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "PID",
        &format!("{} \u{b7} parent {}", pr.pid, parent),
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "User",
        &format!("{} \u{b7} session 1 \u{b7} medium integrity", pr.user),
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "Started",
        "14:02:11 \u{b7} 3 h 12 min ago",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "Priority",
        "Normal \u{b7} all 16 processors",
        false,
    );
    let sig = match pr.sig {
        Sig::None => "Not signed".to_string(),
        _ => format!("Valid \u{b7} {}", pr.publisher),
    };
    y = ui::kv(p, pal, r.x, y, lw, w, "Signature", &sig, false);
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "Memory",
        &format!(
            "private working set {} \u{b7} commit {}",
            d::fmt_mem(pr.mem),
            d::fmt_mem(pr.commit)
        ),
        false,
    );
    ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        w,
        "I/O",
        &format!(
            "{} \u{b7} {} handles \u{b7} {} threads",
            d::fmt_rate(pr.disk, "MB/s"),
            pr.handles,
            pr.threads
        ),
        false,
    );
}

fn list_row(p: &mut Painter, pal: &Palette, r: Rect, a: &str, b: &str, bmono: bool, zebra: bool) {
    if zebra {
        p.fill(r, mix(pal.pane, pal.text, 0.035));
    }
    p.text(Face::Ui, 12.0, r.x + 6.0, r.cy(), a, pal.muted, 72.0);
    p.text(
        if bmono { Face::Mono } else { Face::Ui },
        11.5,
        r.x + 82.0,
        r.cy(),
        b,
        pal.text,
        r.w - 88.0,
    );
}

fn modules(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = section(p, pal, r.x, r.y, "212 modules loaded");
    let rows = [
        ("rustc.exe", "C:\\Users\\kaan\\.rustup\\...\\bin\\rustc.exe"),
        ("ntdll.dll", "C:\\Windows\\System32\\ntdll.dll"),
        ("KERNEL32.DLL", "C:\\Windows\\System32\\KERNEL32.DLL"),
        ("KERNELBASE.dll", "C:\\Windows\\System32\\KERNELBASE.dll"),
        (
            "rustc_driver-1b3.dll",
            "C:\\Users\\kaan\\.rustup\\...\\bin\\rustc_driver-1b3.dll",
        ),
        (
            "LLVM-C.dll",
            "C:\\Users\\kaan\\.rustup\\...\\lib\\LLVM-C.dll",
        ),
        ("ucrtbase.dll", "C:\\Windows\\System32\\ucrtbase.dll"),
        ("ADVAPI32.dll", "C:\\Windows\\System32\\ADVAPI32.dll"),
        (
            "bcryptprimitives.dll",
            "C:\\Windows\\System32\\bcryptprimitives.dll",
        ),
        ("ws2_32.dll", "C:\\Windows\\System32\\ws2_32.dll"),
        (
            "VCRUNTIME140.dll",
            "C:\\Windows\\System32\\VCRUNTIME140.dll",
        ),
        ("msvcp_win.dll", "C:\\Windows\\System32\\msvcp_win.dll"),
    ];
    for (i, (a, b)) in rows.iter().enumerate() {
        list_row(p, pal, Rect::new(r.x, y, r.w, 24.0), a, b, true, i % 2 == 0);
        y += 24.0;
    }
    ui::para(
        p,
        Face::Ui,
        12.0,
        r.x,
        y + 10.0,
        r.w,
        "Read with EnumProcessModulesEx: no administrator rights for your own 64-bit and 32-bit processes.",
        pal.faint,
    );
}

fn handles(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = section(p, pal, r.x, r.y, "640 handles \u{b7} 41 files");
    // Type filter chips.
    let mut x = r.x;
    for (l, on) in [
        ("Files", true),
        ("Keys", false),
        ("Events", false),
        ("All", false),
    ] {
        let wd = p.measure(Face::Ui, 12.0, l) + 18.0;
        let b = Rect::new(x, y, wd, 22.0);
        if on {
            p.rbox(b, 11.0, mix(pal.pane, pal.accent, 0.22));
        } else {
            p.frame(b, 11.0, pal.pane, pal.line);
        }
        p.text_center(Face::Ui, 12.0, b, l, if on { pal.text } else { pal.muted });
        x += wd + 6.0;
    }
    y += 32.0;
    let rows = [
        (
            "File",
            "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\target\\release\\deps\\mdedit-3f2a1c.exe",
        ),
        (
            "File",
            "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\bins\\mdedit\\src\\app.rs",
        ),
        (
            "File",
            "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\target\\release\\.fingerprint\\mdedit-3f2a1c\\dep-bin-mdedit",
        ),
        (
            "File",
            "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\target\\release\\deps\\libmdshared_ui-91ce.rlib",
        ),
        (
            "File",
            "C:\\Users\\kaan\\.cargo\\registry\\src\\index.crates.io-1949cf8c6b5b557f\\winit-0.30.12\\src\\lib.rs",
        ),
        ("File", "\\Device\\ConDrv\\Connect"),
        ("File", "C:\\Windows\\Fonts\\segoeui.ttf"),
        (
            "File",
            "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\Cargo.lock",
        ),
    ];
    for (i, (a, b)) in rows.iter().enumerate() {
        let h = 40.0;
        let rr = Rect::new(r.x, y, r.w, h);
        if i % 2 == 0 {
            p.fill(rr, mix(pal.pane, pal.text, 0.035));
        }
        p.text(Face::Ui, 11.5, rr.x + 6.0, rr.y + 12.0, a, pal.faint, 40.0);
        let chars: Vec<char> = b.chars().collect();
        let short = if chars.len() > 44 {
            format!(
                "\u{2026}{}",
                chars[chars.len() - 42..].iter().collect::<String>()
            )
        } else {
            b.to_string()
        };
        p.text(
            Face::Mono,
            11.0,
            rr.x + 46.0,
            rr.y + 12.0,
            &short,
            pal.text,
            rr.w - 52.0,
        );
        p.text(
            Face::Ui,
            11.0,
            rr.x + 46.0,
            rr.y + 28.0,
            "granted: read, write  \u{b7}  handle 0x4a8",
            pal.faint,
            rr.w - 52.0,
        );
        y += h;
    }
    ui::para(
        p,
        Face::Ui,
        12.0,
        r.x,
        y + 8.0,
        r.w,
        "Names are read by duplicating each handle into this app with a 100 ms limit, so a handle that blocks never stops the window.",
        pal.faint,
    );
}

fn network(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = section(p, pal, r.x, r.y, "Connections of this process");
    let rows = [
        (
            "TCP",
            "192.168.1.24:51822",
            "140.82.112.4:443",
            "Established",
        ),
        (
            "TCP",
            "192.168.1.24:51840",
            "151.101.1.55:443",
            "Established",
        ),
        ("TCP", "192.168.1.24:51907", "20.50.80.209:443", "Time wait"),
        ("UDP", "0.0.0.0:5353", "*", ""),
        ("TCP", "127.0.0.1:9229", "0.0.0.0:0", "Listening"),
    ];
    for (i, (a, b, c, s)) in rows.iter().enumerate() {
        let rr = Rect::new(r.x, y, r.w, 38.0);
        if i % 2 == 0 {
            p.fill(rr, mix(pal.pane, pal.text, 0.035));
        }
        p.text(
            Face::Semibold,
            11.5,
            rr.x + 6.0,
            rr.y + 12.0,
            a,
            pal.muted,
            40.0,
        );
        p.text(
            Face::Mono,
            11.0,
            rr.x + 44.0,
            rr.y + 12.0,
            &format!("{b}  \u{2192}  {c}"),
            pal.text,
            rr.w - 50.0,
        );
        p.text(
            Face::Ui,
            11.5,
            rr.x + 44.0,
            rr.y + 28.0,
            s,
            pal.muted,
            120.0,
        );
        y += 38.0;
    }
    y += 10.0;
    p.frame(
        Rect::new(r.x, y, r.w, 62.0),
        6.0,
        mix(pal.pane, pal.text, 0.03),
        pal.line,
    );
    p.icon(SHIELD, 14.0, r.x + 18.0, y + 20.0, amber(pal));
    ui::para(
        p,
        Face::Ui,
        12.0,
        r.x + 34.0,
        y + 8.0,
        r.w - 44.0,
        "Bytes per connection need a Windows event trace, which needs administrator rights. The list itself does not.",
        pal.muted,
    );
}

fn environment(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = section(p, pal, r.x, r.y, "38 variables");
    let rows = [
        ("CARGO_HOME", "C:\\Users\\kaan\\.cargo"),
        ("CARGO_TARGET_DIR", "target\\fast"),
        ("CARGO_PROFILE_RELEASE_LTO", "off"),
        ("COMPUTERNAME", "KAAN-PC"),
        ("HOMEPATH", "\\Users\\kaan"),
        ("LOCALAPPDATA", "C:\\Users\\kaan\\AppData\\Local"),
        ("NUMBER_OF_PROCESSORS", "16"),
        ("OS", "Windows_NT"),
        (
            "PATH",
            "C:\\Users\\kaan\\.cargo\\bin;C:\\Windows\\system32;C:\\Windows;C:\\Program Files\\Git\\cmd",
        ),
        ("TEMP", "C:\\Users\\kaan\\AppData\\Local\\Temp"),
    ];
    for (i, (k, v)) in rows.iter().enumerate() {
        let lines = ui::wrap(p, Face::Mono, 11.0, v, r.w - 12.0);
        let h = 18.0 + 15.0 * lines.len().max(1) as f32;
        let rr = Rect::new(r.x, y, r.w, h);
        if i % 2 == 0 {
            p.fill(rr, mix(pal.pane, pal.text, 0.035));
        }
        p.text(
            Face::Mono,
            11.0,
            rr.x + 6.0,
            rr.y + 11.0,
            k,
            pal.muted,
            rr.w - 12.0,
        );
        for (n, l) in lines.iter().enumerate() {
            p.text(
                Face::Mono,
                11.0,
                rr.x + 6.0,
                rr.y + 26.0 + n as f32 * 15.0,
                l,
                pal.text,
                rr.w - 12.0,
            );
        }
        y += h;
    }
    ui::para(
        p,
        Face::Ui,
        12.0,
        r.x,
        y + 8.0,
        r.w,
        "Read from the process's own memory (needs the same user). A 32-bit process is read through its WoW64 block.",
        pal.faint,
    );
}
