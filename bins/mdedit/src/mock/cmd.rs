//! The commander-style mockup: two panels, a command line and a function-key bar, drawn in the
//! family's own look (not any commander's), with the places a plugin would show up: an archive
//! or a remote system as a folder, a lister tab, and the plugin manager.

use super::fmdata::{self as d, COMPUTING, E, K, NOT_COMPUTED};
use super::fmover::footer;
use super::fmwin::{self, amber, blue, green, red};
use super::ui;
use mdshared_ui::chrome::{
    TITLE_H, TABS_LEFT, TabState, WIN_BTN_W, WinButton, draw_tab, draw_win_button, tab_natural_width, tab_widths,
    win_button_rect,
};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{dialog_card, draw_backdrop, draw_dialog_card};
use mdshared_ui::theme::Palette;

const MENU_H: f32 = 28.0;
const DRIVES_H: f32 = 32.0;
const CMDLINE_H: f32 = 34.0;
const FN_H: f32 = 36.0;
const HEAD_H: f32 = 28.0;
const COLS_H: f32 = 24.0;
const ROW: f32 = 22.0;
const FOOT_H: f32 = 26.0;

struct Panel {
    path: &'static str,
    /// A plugin type chip next to the path: "WCX", "WFX".
    chip: Option<(&'static str, &'static str)>,
    rows: Vec<Row>,
    cursor: usize,
    active: bool,
    foot: String,
}

struct Row {
    name: String,
    ext: String,
    size: String,
    date: String,
    dir: bool,
    marked: bool,
    k: K,
    /// A column a WDX plugin supplies, drawn instead of the date.
    extra: Option<String>,
}

fn split(name: &str) -> (String, String) {
    match name.rfind('.') {
        Some(i) if i > 0 => (name[..i].to_string(), name[i + 1..].to_string()),
        _ => (name.to_string(), String::new()),
    }
}

fn row_of(e: &E, marked: bool) -> Row {
    let (n, x) = if e.k == K::Dir { (e.name.to_string(), String::new()) } else { split(e.name) };
    let size = match (e.k, e.size) {
        (K::Dir, NOT_COMPUTED) | (K::Dir, COMPUTING) => "<DIR>".to_string(),
        (K::Dir, s) => d::human(s),
        (_, s) => d::human(s),
    };
    Row { name: n, ext: x, size, date: e.date.to_string(), dir: e.k == K::Dir, marked, k: e.k, extra: None }
}

pub fn draw(p: &mut Painter, pal: &Palette, name: &str, w: f32, h: f32) {
    match name {
        "cm-main" => {
            let l: Vec<Row> = d::project()
                .iter()
                .filter(|e| e.flag != d::Flag::Hidden)
                .enumerate()
                .map(|(i, e)| row_of(e, [11, 12, 14].contains(&i)))
                .collect();
            let (dl, _) = d::downloads();
            let r: Vec<Row> = dl.iter().map(|e| row_of(e, false)).collect();
            let lp = Panel {
                path: "C:\\Users\\you\\Projects\\mdedit",
                chip: None,
                rows: l,
                cursor: 12,
                active: true,
                foot: "3 of 15 selected \u{b7} 21.4 KB  \u{b7}  142 GB free".into(),
            };
            let rp = Panel {
                path: "C:\\Users\\you\\Downloads",
                chip: None,
                rows: r,
                cursor: 1,
                active: false,
                foot: "0 of 9 selected  \u{b7}  142 GB free".into(),
            };
            window(p, pal, w, h, lp, rp, "main");
        }
        "cm-vfs" => {
            let mk = |n: &str, s: &str, dt: &str, dir: bool| {
                let (b, x) = if dir { (n.to_string(), String::new()) } else { split(n) };
                Row { name: b, ext: x, size: s.into(), date: dt.into(), dir, marked: false, k: if dir { K::Dir } else { K::Txt }, extra: None }
            };
            let lp = Panel {
                path: "build-logs.zip\\2026-11-19\\",
                chip: Some(("WCX", "7z.wcx64 \u{b7} read-only")),
                rows: vec![
                    mk("..", "", "", true),
                    mk("windows", "<DIR>", "2026-11-19 12:01", true),
                    mk("linux", "<DIR>", "2026-11-19 12:01", true),
                    mk("summary.txt", "2.1 KB", "2026-11-19 12:01", false),
                    mk("ci-run-4412.log", "1.8 MB", "2026-11-19 12:00", false),
                    mk("ci-run-4413.log", "1.9 MB", "2026-11-19 12:00", false),
                    mk("timings.json", "14.2 KB", "2026-11-19 12:01", false),
                ],
                cursor: 4,
                active: true,
                foot: "archive opened in the plugin's helper process  \u{b7}  F5 extracts the tagged files".into(),
            };
            let mut rows = vec![
                mk("..", "", "", true),
                mk("2026", "<DIR>", "2026-11-02 08:00", true),
                mk("raw", "<DIR>", "2026-10-06 21:00", true),
                mk("IMG_0412.png", "3.1 MB", "2026-10-04 11:20", false),
                mk("IMG_0413.png", "2.9 MB", "2026-10-04 11:21", false),
                mk("harbour.png", "4.2 MB", "2026-10-04 12:02", false),
                mk("map.pdf", "840 KB", "2026-10-03 22:10", false),
            ];
            for (r, x) in rows.iter_mut().zip(["", "", "", "4032x3024", "4032x3024", "6000x4000", "12 pages"]) {
                r.extra = Some(x.to_string());
            }
            let rp = Panel {
                path: "\\\\Plugins\\SFTP\\nas.local\\photos\\Trip",
                chip: Some(("WFX", "sftp.wfx64 \u{b7} nas.local \u{b7} connected")),
                rows,
                cursor: 5,
                active: false,
                foot: "extra column from a WDX plugin: Dimensions  \u{b7}  a transfer can be cancelled, a hang cannot".into(),
            };
            window(p, pal, w, h, lp, rp, "vfs");
        }
        "cm-lister" => {
            let (dl, _) = d::downloads();
            let lp = Panel { path: "C:\\Users\\you\\Projects\\mdedit\\data", chip: None, rows: vec![], cursor: 0, active: true, foot: String::new() };
            let _ = dl;
            let rp = Panel { path: "", chip: None, rows: vec![], cursor: 0, active: false, foot: String::new() };
            window(p, pal, w, h, lp, rp, "lister");
        }
        "cm-plugins" => {
            let lp = Panel { path: "C:\\Users\\you\\Projects\\mdedit", chip: None, rows: vec![], cursor: 0, active: true, foot: String::new() };
            let rp = Panel { path: "C:\\Users\\you\\Downloads", chip: None, rows: vec![], cursor: 0, active: false, foot: String::new() };
            window(p, pal, w, h, lp, rp, "plugins");
        }
        other => eprintln!("mdedit: no mock named {other}"),
    }
}

fn title(p: &mut Painter, pal: &Palette, w: f32) {
    p.fill(Rect::new(0.0, 0.0, w, TITLE_H), pal.title);
    fmwin::tile(p, Rect::new(14.0, 9.0, 22.0, 22.0));
    let mut right = w;
    for kind in [WinButton::Close, WinButton::Max, WinButton::Min] {
        let b = win_button_rect(right, 0);
        right -= WIN_BTN_W;
        draw_win_button(p, pal, kind, b, false, false, false);
    }
    let labels = ["mdedit \u{2194} Downloads", "Trip \u{2194} NAS"];
    let avail = (right - 56.0 - TABS_LEFT - 40.0).max(0.0);
    let natural: Vec<f32> = labels.iter().map(|l| tab_natural_width(p, l)).collect();
    let widths = tab_widths(&natural, avail);
    let mut x = TABS_LEFT;
    for (i, l) in labels.iter().enumerate() {
        let st = TabState { active: i == 0, next_quiet: false, ..TabState::default() };
        draw_tab(p, pal, x, widths[i], w, l, &st);
        x += widths[i];
    }
    p.icon(glyph::PLUS, 12.0, x + 17.0, 23.0, pal.muted);
}

fn menu_row(p: &mut Painter, pal: &Palette, r: Rect) {
    p.fill(r, pal.bar);
    let mut x = 10.0;
    for l in ["File", "Mark", "Command", "Network", "Show", "Options", "Help"] {
        let wd = p.measure(Face::Ui, 13.0, l) + 20.0;
        p.text_center(Face::Ui, 13.0, Rect::new(x, r.y, wd, r.h), l, pal.text);
        x += wd;
    }
}

fn drives_row(p: &mut Painter, pal: &Palette, r: Rect, w: f32) {
    p.fill(r, pal.page);
    p.fill(Rect::new(r.x, r.bottom() - p.px().max(1.0), r.w, p.px().max(1.0)), pal.line);
    let half = w / 2.0;
    for (side, act) in [(0.0, "c"), (half, "c")] {
        let mut x = side + 10.0;
        for (i, dv) in ["c", "d", "e", "~", "\u{2605}"].iter().enumerate() {
            let on = *dv == act && (side == 0.0 || i == 0);
            let b = Rect::new(x, r.y + 4.0, 30.0, 24.0);
            if on {
                p.rbox(b, 5.0, pal.on);
            }
            p.text_center(Face::Semibold, 12.5, b, dv, if on { pal.accent } else { pal.muted });
            x += 32.0;
        }
        p.text(Face::Ui, 11.5, x + 8.0, r.cy(), "Plugins \u{25be}", pal.faint, 80.0);
        if side == 0.0 {
            p.text(Face::Ui, 11.5, x + 80.0, r.cy(), "142 GB free of 476 GB", pal.muted, 160.0);
        }
    }
}

fn panel(p: &mut Painter, pal: &Palette, r: Rect, pn: &Panel) {
    p.fill(r, pal.page);
    let px = p.px().max(1.0);
    let head = Rect::new(r.x, r.y, r.w, HEAD_H);
    p.fill(head, if pn.active { pal.bar } else { pal.pane });
    if pn.active {
        p.fill(Rect::new(head.x, head.y, head.w, 2.0), pal.accent);
    }
    let mut x = r.x + 12.0;
    if let Some((k, s)) = pn.chip {
        let wd = p.measure(Face::Semibold, 10.5, k) + 12.0;
        let c = if k == "WCX" { amber(pal) } else { blue(pal) };
        p.rbox(Rect::new(x, head.cy() - 8.0, wd, 16.0), 8.0, mix(pal.bar, c, 0.22));
        p.text_center(Face::Semibold, 10.5, Rect::new(x, head.cy() - 8.0, wd, 16.0), k, c);
        x += wd + 8.0;
        let _ = s;
    }
    p.text(Face::Semibold, 12.5, x, head.cy() + 1.0, pn.path, pal.text, r.w - (x - r.x) - 12.0);
    let cols = Rect::new(r.x, head.bottom(), r.w, COLS_H);
    p.fill(cols, pal.pane);
    p.fill(Rect::new(r.x, cols.bottom() - px, r.w, px), pal.line);
    let size_w = 84.0;
    let date_w = 134.0;
    let ext_w = 54.0;
    let date_x = r.right() - date_w - 12.0;
    let size_x = date_x - size_w;
    let ext_x = size_x - ext_w;
    let cy = cols.cy();
    p.text(Face::Semibold, 11.5, r.x + 30.0, cy, "Name", pal.muted, 60.0);
    p.text(Face::Semibold, 11.5, ext_x + 4.0, cy, "Ext", pal.muted, 40.0);
    p.text_right(Face::Semibold, 11.5, size_x + size_w - 8.0, cy, "Size", pal.muted);
    let extra = pn.rows.iter().any(|r| r.extra.is_some());
    p.text(Face::Semibold, 11.5, date_x + 4.0, cy, if extra { "Dimensions" } else { "Date" }, pal.muted, 100.0);
    let old = p.clip(Rect::new(r.x, cols.bottom(), r.w, r.h - HEAD_H - COLS_H - FOOT_H));
    let mut y = cols.bottom();
    for (i, row) in pn.rows.iter().enumerate() {
        let rr = Rect::new(r.x, y, r.w, ROW);
        y += ROW;
        let cur = i == pn.cursor;
        if cur {
            let f = if pn.active { mix(pal.page, pal.accent, if pal.dark { 0.34 } else { 0.22 }) } else { pal.on };
            p.rbox(Rect::new(rr.x + 3.0, rr.y, rr.w - 6.0, rr.h), 3.0, f);
        }
        let col: Rgb = if row.marked { amber(pal) } else { pal.text };
        let fnt = if row.marked { Face::Semibold } else { Face::Ui };
        let g = if row.dir { glyph::FOLDER } else { glyph::DOC };
        let ic = if row.dir { pal.folder } else { d::tint(row.k) };
        p.icon(g, 13.0, rr.x + 17.0, rr.cy(), ic);
        let nm = if row.dir && row.name != ".." { format!("[{}]", row.name) } else { row.name.clone() };
        p.text(fnt, 12.5, rr.x + 30.0, rr.cy(), &nm, col, ext_x - rr.x - 36.0);
        p.text(fnt, 12.0, ext_x + 4.0, rr.cy(), &row.ext, if row.marked { col } else { pal.muted }, ext_w);
        p.text_right(Face::Mono, 11.5, size_x + size_w - 8.0, rr.cy(), &row.size, if row.marked { col } else { pal.muted });
        let dtxt = row.extra.clone().unwrap_or_else(|| row.date.clone());
        p.text(Face::Ui, 12.0, date_x + 4.0, rr.cy(), &dtxt, if row.marked { col } else { pal.muted }, date_w);
    }
    p.restore(old);
    let foot = Rect::new(r.x, r.bottom() - FOOT_H, r.w, FOOT_H);
    p.fill(foot, pal.pane);
    p.fill(Rect::new(foot.x, foot.y, foot.w, px), pal.line);
    p.text(Face::Ui, 11.5, foot.x + 12.0, foot.cy(), &pn.foot, pal.muted, foot.w - 20.0);
}

fn cmdline(p: &mut Painter, pal: &Palette, r: Rect) {
    p.fill(r, pal.bar);
    p.fill(Rect::new(r.x, r.y, r.w, p.px().max(1.0)), pal.line);
    p.text(Face::Mono, 12.5, r.x + 14.0, r.cy(), "C:\\Users\\you\\Projects\\mdedit>", pal.muted, 300.0);
    p.text(Face::Mono, 12.5, r.x + 232.0, r.cy(), "cargo build --release", pal.text, 400.0);
    let f = Rect::new(r.x + 228.0 + 168.0, r.cy() - 8.0, 2.0, 16.0);
    p.fill(f, pal.accent);
    p.text_right(Face::Ui, 11.5, r.right() - 14.0, r.cy(), "Tab switches panel \u{b7} Ctrl+\u{2191} history", pal.faint);
}

fn fn_bar(p: &mut Painter, pal: &Palette, r: Rect) {
    p.fill(r, pal.page);
    p.fill(Rect::new(r.x, r.y, r.w, p.px().max(1.0)), pal.line);
    let keys = [
        ("F1", "Help"),
        ("F2", "Rename"),
        ("F3", "View"),
        ("F4", "Edit"),
        ("F5", "Copy"),
        ("F6", "Move"),
        ("F7", "Folder"),
        ("F8", "Delete"),
        ("F9", "Menu"),
        ("F10", "Quit"),
    ];
    let cw = r.w / keys.len() as f32;
    for (i, (k, l)) in keys.iter().enumerate() {
        let b = Rect::new(r.x + i as f32 * cw + 4.0, r.y + 5.0, cw - 8.0, r.h - 10.0);
        p.rbox(b, 5.0, mix(pal.page, pal.text, 0.05));
        let kw = p.measure(Face::Semibold, 11.0, k) + 12.0;
        let kr = Rect::new(b.x + 6.0, b.cy() - 9.0, kw, 18.0);
        p.rbox(kr, 4.0, mix(pal.page, pal.text, 0.14));
        p.text_center(Face::Semibold, 11.0, kr, k, pal.muted);
        p.text(Face::Ui, 12.5, kr.right() + 8.0, b.cy(), l, pal.text, b.w - kw - 24.0);
    }
}

fn window(p: &mut Painter, pal: &Palette, w: f32, h: f32, l: Panel, r: Panel, over: &str) {
    p.fill(Rect::new(0.0, 0.0, w, h), pal.page);
    title(p, pal, w);
    let mut y = TITLE_H;
    menu_row(p, pal, Rect::new(0.0, y, w, MENU_H));
    y += MENU_H;
    drives_row(p, pal, Rect::new(0.0, y, w, DRIVES_H), w);
    y += DRIVES_H;
    let bottom = h - CMDLINE_H - FN_H;
    let half = (w / 2.0).floor();
    let a = Rect::new(0.0, y, half, bottom - y);
    let b = Rect::new(half, y, w - half, bottom - y);
    panel(p, pal, a, &l);
    panel(p, pal, b, &r);
    p.fill(Rect::new(half - 0.5, y, p.px().max(1.0), bottom - y), pal.line);
    cmdline(p, pal, Rect::new(0.0, bottom, w, CMDLINE_H));
    fn_bar(p, pal, Rect::new(0.0, bottom + CMDLINE_H, w, FN_H));
    match over {
        "lister" => lister(p, pal, w, h),
        "plugins" => plugins(p, pal, w, h),
        _ => {}
    }
}

fn lister(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(1040.0, 640.0, w, h);
    p.shadow(c, 10.0, pal.dark);
    p.frame(c, 10.0, pal.popup, pal.popup_line);
    let l = c.x + 20.0;
    p.text(Face::Semibold, 14.0, l, c.y + 24.0, "timings.db", pal.text, 300.0);
    p.text(Face::Ui, 12.0, l + 90.0, c.y + 25.0, "SQLite database \u{b7} 412 KB", pal.muted, 300.0);
    // Lister tabs: the built-in ones and the plugin's.
    let mut x = l;
    let ty = c.y + 56.0;
    for (i, (t, plug)) in [("Text", false), ("Hex", false), ("Image", false), ("Table", true)].iter().enumerate() {
        let wd = p.measure(Face::Ui, 12.5, t) + 28.0 + if *plug { 40.0 } else { 0.0 };
        let b = Rect::new(x, ty - 14.0, wd, 28.0);
        if i == 3 {
            p.rbox(b, 6.0, pal.on);
        }
        p.text(Face::Ui, 12.5, b.x + 14.0, ty, t, if i == 3 { pal.text } else { pal.muted }, wd);
        if *plug {
            let r2 = Rect::new(b.right() - 40.0, ty - 8.0, 32.0, 16.0);
            p.rbox(r2, 8.0, mix(pal.popup, amber(pal), 0.22));
            p.text_center(Face::Semibold, 10.0, r2, "WLX", amber(pal));
        }
        x += wd + 4.0;
    }
    let g = Rect::new(l, ty + 22.0, c.w - 40.0, c.h - 130.0);
    p.frame(g, 6.0, pal.field, pal.popup_line);
    let colx = [0.0, 70.0, 270.0, 420.0, 560.0];
    let heads = ["id", "job", "started", "seconds", "result"];
    p.fill(Rect::new(g.x + 1.0, g.y + 1.0, g.w - 2.0, 26.0), mix(pal.field, pal.text, 0.05));
    for (cx, hd) in colx.iter().zip(heads) {
        p.text(Face::Semibold, 12.0, g.x + 14.0 + cx, g.y + 14.0, hd, pal.muted, 120.0);
    }
    let rows = [
        ("4412", "windows / test", "12:00:04", "412.6", "ok"),
        ("4412", "linux / test", "12:00:04", "388.1", "ok"),
        ("4412", "macos-arm / test", "12:00:05", "501.9", "ok"),
        ("4413", "windows / clippy", "12:03:41", "96.2", "ok"),
        ("4413", "windows / shots", "12:03:41", "148.0", "failed"),
        ("4413", "linux / test", "12:03:42", "391.4", "ok"),
        ("4414", "windows / test", "12:20:10", "415.3", "ok"),
    ];
    for (i, r) in rows.iter().enumerate() {
        let y = g.y + 27.0 + i as f32 * 26.0;
        for (k, (cx, v)) in colx.iter().zip([r.0, r.1, r.2, r.3, r.4]).enumerate() {
            let col = if v == "failed" { red(pal) } else if v == "ok" { green(pal) } else { pal.text };
            p.text(if k == 0 || k == 3 { Face::Mono } else { Face::Ui }, 12.0, g.x + 14.0 + cx, y + 13.0, v, col, 200.0);
        }
    }
    let by = c.bottom() - 40.0;
    p.fill(Rect::new(c.x + 1.0, by - 6.0, c.w - 2.0, p.px().max(1.0)), pal.popup_line);
    p.icon(glyph::WARN, 13.0, l + 7.0, by + 14.0, amber(pal));
    p.text(
        Face::Ui,
        12.0,
        l + 24.0,
        by + 14.0,
        "sqlite.wlx64 draws in its own window. It runs in the plugin host: if it hangs, this tab is closed and the file manager goes on.",
        pal.muted,
        c.w - 60.0,
    );
}

fn plugins(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(1000.0, 640.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 24.0;
    let r = c.right() - 24.0;
    p.text(Face::Semibold, 16.0, l, c.y + 30.0, "Options \u{b7} Plugins", pal.text, 400.0);
    p.text(Face::Ui, 12.0, l + 190.0, c.y + 31.0, "each plugin runs in a helper process, never in the window", pal.muted, 460.0);
    let top = c.y + 56.0;
    let lw = (r - l) * 0.62;
    let list = Rect::new(l, top, lw, c.h - DIALOG_FOOTER_H - 136.0);
    p.frame(list, 8.0, pal.field, pal.popup_line);
    p.fill(Rect::new(list.x + 1.0, list.y + 1.0, list.w - 2.0, 26.0), mix(pal.field, pal.text, 0.05));
    let cx = [14.0, 70.0, 220.0, 330.0];
    for (x, t) in cx.iter().zip(["Type", "Plugin", "Handles", "State"]) {
        p.text(Face::Semibold, 11.5, list.x + x, list.y + 14.0, t, pal.muted, 120.0);
    }
    let rows: [(&str, &str, &str, &str, &str); 8] = [
        ("WCX", "7z.wcx64", "7z rar iso cab msi", "Allowed", "ok"),
        ("WCX", "unrar.wcx64", "rar", "Allowed", "ok"),
        ("WDX", "mediainfo.wdx64", "audio video image", "Allowed", "ok"),
        ("WLX", "sqlite.wlx64", "db sqlite", "Allowed", "ok"),
        ("WFX", "sftp.wfx64", "\\\\Plugins\\SFTP", "Allowed", "ok"),
        ("WFX", "registry.wfx", "\\\\Plugins\\Registry", "32-bit: needs the 32-bit host", "warn"),
        ("WCX", "chm.wcx64", "chm", "New file: not allowed yet", "new"),
        ("WLX", "imagine.wlx64", "images", "Hash changed since allowed", "bad"),
    ];
    for (i, (t, n, hd, st, k)) in rows.iter().enumerate() {
        let y = list.y + 28.0 + i as f32 * 34.0;
        let rr = Rect::new(list.x + 3.0, y, list.w - 6.0, 32.0);
        if i == 6 {
            p.rbox(rr, 4.0, fmwin::sel_fill(pal, true));
        }
        let col = match *t {
            "WCX" => amber(pal),
            "WFX" => blue(pal),
            "WLX" => green(pal),
            _ => [0xb0, 0x7a, 0xd8],
        };
        let tw = p.measure(Face::Semibold, 10.5, t) + 12.0;
        let tr = Rect::new(list.x + cx[0], rr.cy() - 8.0, tw, 16.0);
        p.rbox(tr, 8.0, mix(pal.field, col, 0.22));
        p.text_center(Face::Semibold, 10.5, tr, t, col);
        p.text(Face::Mono, 12.0, list.x + cx[1], rr.cy(), n, pal.text, 140.0);
        p.text(Face::Ui, 12.0, list.x + cx[2], rr.cy(), hd, pal.muted, 104.0);
        let sc = match *k {
            "ok" => green(pal),
            "warn" => amber(pal),
            "new" => blue(pal),
            _ => red(pal),
        };
        p.rbox(Rect::new(list.x + cx[3], rr.cy() - 3.5, 7.0, 7.0), 3.5, sc);
        p.text(Face::Ui, 12.0, list.x + cx[3] + 14.0, rr.cy(), st, pal.text, list.w - cx[3] - 22.0);
    }
    // The selected one's page.
    let dx = list.right() + 16.0;
    let dw = r - dx;
    p.text(Face::Semibold, 13.5, dx, top + 12.0, "chm.wcx64", pal.text, dw);
    let mut y = top + 30.0;
    for (k, v) in [
        ("Type", "Packer (WCX), read and write"),
        ("Version", "2.1.0.4 \u{b7} 64-bit"),
        ("File", "plugins\\wcx\\chm\\chm.wcx64"),
        ("SHA-256", "9f2c\u{2026}b71a"),
        ("Signed", "No"),
        ("Runs in", "plugin host (one per plugin)"),
        ("Limits", "256 MB, 30 s per call, no network"),
    ] {
        y = ui::kv(p, pal, dx, y, 70.0, dw, k, v, k == "SHA-256" || k == "File");
    }
    y += 8.0;
    ui::para(
        p,
        Face::Ui,
        12.0,
        dx,
        y,
        dw,
        "Not loaded. It was added to the plugins folder after you last looked. Allowing it pins this exact file: if the file changes, it is asked about again.",
        pal.muted,
    );
    let by = list.bottom() + 14.0;
    for t in ["Add\u{2026}", "Remove", "Configure\u{2026}", "Test"] {
        let _ = t;
    }
    let mut x = l;
    for (t, prim) in [("Allow this file", true), ("Remove", false), ("Configure\u{2026}", false)] {
        let b = ui::sbtn(p, pal, x, by + 14.0, t, prim, true);
        x = b.right() + 8.0;
    }
    ui::check(p, pal, r - 330.0, by + 14.0, true, "Load only plugins I have allowed", 320.0);
    footer(p, pal, f, l, r, &["Plugins folder\u{2026}"], &["Close"]);
}

const DIALOG_FOOTER_H: f32 = 72.0;
