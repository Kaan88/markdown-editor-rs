//! The other pages of the task manager: Performance (six resources), Startup, Services and
//! Files in use, each with its toolbar. Graphs are drawn by `graph`, which a real build would
//! take from a new shared module.

use super::tmdata::{self as d};
use super::tmwin::{self as w, SHIELD, amber, blue, green, red, search_field, seg, sep, tbtn};
use super::ui;
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

pub fn body_rect(width: f32, h: f32) -> Rect {
    let top = mdshared_ui::chrome::TITLE_H + w::TOOL_H;
    Rect::new(0.0, top, width, h - top - w::STATUS_H)
}

// ---- the graph --------------------------------------------------------------------------------

/// An area graph of `s` (0..1) filling `r`. `hlines` horizontal rules, `border` a frame.
#[allow(clippy::too_many_arguments)]
pub fn graph(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    s: &[f32],
    col: Rgb,
    fill: f32,
    hlines: usize,
    border: bool,
) {
    let step = p.px().max(1.0);
    let bg = mix(pal.page, col, 0.035);
    if border {
        p.fill(r, bg);
        let line = mix(bg, col, 0.55);
        let t = step;
        p.fill(Rect::new(r.x, r.y, r.w, t), line);
        p.fill(Rect::new(r.x, r.bottom() - t, r.w, t), line);
        p.fill(Rect::new(r.x, r.y, t, r.h), line);
        p.fill(Rect::new(r.right() - t, r.y, t, r.h), line);
        let gl = mix(bg, col, 0.16);
        let vx = 10;
        for i in 1..vx {
            let x = (r.x + r.w * i as f32 / vx as f32).round();
            p.fill(Rect::new(x, r.y + t, t, r.h - 2.0 * t), gl);
        }
        for i in 1..=hlines {
            let y = (r.y + r.h * i as f32 / (hlines + 1) as f32).round();
            p.fill(Rect::new(r.x + t, y, r.w - 2.0 * t, t), gl);
        }
    }
    if s.len() < 2 {
        return;
    }
    let n = (r.w / step) as usize;
    let mut prev: Option<f32> = None;
    for k in 0..n {
        let t = k as f32 / (n.max(2) - 1) as f32;
        let pos = t * (s.len() - 1) as f32;
        let i = (pos.floor() as usize).min(s.len() - 2);
        let f = pos - i as f32;
        let v = (s[i] * (1.0 - f) + s[i + 1] * f).clamp(0.0, 1.0);
        let y = r.bottom() - v * (r.h - 2.0 * step) - step;
        let x = r.x + k as f32 * step;
        if fill > 0.0 {
            p.fill_a(Rect::new(x, y, step, r.bottom() - y - step), col, fill);
        }
        let y0 = prev.unwrap_or(y);
        let top = y.min(y0);
        let hh = (y - y0).abs().max(1.6 * step);
        p.fill(Rect::new(x, top - 0.3 * step, step, hh), col);
        prev = Some(y);
    }
}

pub fn spark_labels(p: &mut Painter, pal: &Palette, r: Rect, top: &str, left: &str, right: &str) {
    p.text_right(Face::Ui, 11.5, r.right(), r.y - 9.0, top, pal.muted);
    p.text(
        Face::Ui,
        11.5,
        r.x,
        r.bottom() + 11.0,
        left,
        pal.muted,
        200.0,
    );
    p.text_right(
        Face::Ui,
        11.5,
        r.right(),
        r.bottom() + 11.0,
        right,
        pal.muted,
    );
}

// ---- Performance ------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
pub enum Res {
    Cpu,
    Cores,
    Mem,
    Disk,
    Net,
    Gpu,
}

pub fn perf_tool(p: &mut Painter, pal: &Palette, r: Rect, width: f32) {
    p.fill(r, pal.page);
    let cy = r.cy();
    let mut x = 12.0;
    p.text(Face::Semibold, 13.0, x, cy, "History", pal.muted, 80.0);
    x += 62.0;
    x = seg(
        p,
        pal,
        x,
        cy,
        &[
            (glyph::RECENT, "60 s"),
            (glyph::RECENT, "10 min"),
            (glyph::RECENT, "1 hour"),
        ],
        0,
        width < w::COMPACT_W,
    ) + 10.0;
    sep(p, pal, &mut x, cy);
    p.icon(glyph::SLIDERS, 14.0, x + 14.0, cy, pal.muted);
    p.text(
        Face::Ui,
        13.0,
        x + 28.0,
        cy,
        "Sampling every 1 s",
        pal.muted,
        220.0,
    );
    let mut rx = width - 10.0;
    rx -= 34.0;
    p.icon(glyph::FULLSCREEN, 15.0, rx + 16.0, cy, pal.muted);
    rx -= 34.0;
    p.icon(w::PAUSE, 15.0, rx + 16.0, cy, pal.muted);
}

struct Tile {
    res: Res,
    title: &'static str,
    sub: &'static str,
    col: Rgb,
    series: Vec<f32>,
}

fn tiles() -> Vec<Tile> {
    vec![
        Tile {
            res: Res::Cpu,
            title: "CPU",
            sub: "11%  4.52 GHz",
            col: [0x11, 0x7d, 0xbb],
            series: d::series(60, 0.14, 0.2, 3),
        },
        Tile {
            res: Res::Mem,
            title: "Memory",
            sub: "17.3 / 32 GB (54%)",
            col: [0x8b, 0x12, 0xae],
            series: d::series(60, 0.54, 0.03, 8),
        },
        Tile {
            res: Res::Disk,
            title: "Disk 0 (C:)",
            sub: "SSD  1%",
            col: [0x4d, 0xa1, 0x2f],
            series: d::series(60, 0.03, 0.12, 12),
        },
        Tile {
            res: Res::Net,
            title: "Wi-Fi",
            sub: "S 0.1  R 1.1 Mbps",
            col: [0xa7, 0x4f, 0x01],
            series: d::series(60, 0.10, 0.2, 19),
        },
        Tile {
            res: Res::Gpu,
            title: "GPU 0",
            sub: "14%  44\u{b0}",
            col: [0x11, 0x7d, 0xbb],
            series: d::series(60, 0.16, 0.3, 24),
        },
    ]
}

pub fn perf_page(p: &mut Painter, pal: &Palette, b: Rect, sel: Res) {
    // Left: one tile per resource.
    let lw = if b.w < w::COMPACT_W { 190.0 } else { 248.0 };
    let left = Rect::new(b.x, b.y, lw, b.h);
    p.fill(left, pal.pane);
    p.fill(Rect::new(left.right(), b.y, p.px().max(1.0), b.h), pal.line);
    let mut y = b.y + 8.0;
    for t in tiles() {
        let on = match (t.res, sel) {
            (Res::Cpu, Res::Cores) => true,
            (a, bb) => a == bb,
        };
        let r = Rect::new(left.x + 8.0, y, lw - 16.0, 64.0);
        if on {
            p.rbox(r, 6.0, mix(pal.pane, pal.accent, 0.16));
            p.fill(Rect::new(r.x, r.y + 10.0, 3.0, r.h - 20.0), pal.accent);
        }
        let g = Rect::new(r.x + 12.0, r.y + 10.0, 64.0, 44.0);
        graph(p, pal, g, &t.series, t.col, 0.25, 0, true);
        p.text(
            Face::Semibold,
            13.0,
            g.right() + 12.0,
            r.y + 22.0,
            t.title,
            pal.text,
            r.w - 100.0,
        );
        p.text(
            Face::Ui,
            12.0,
            g.right() + 12.0,
            r.y + 42.0,
            t.sub,
            pal.muted,
            r.w - 92.0,
        );
        y += 70.0;
    }
    // Right: the chosen resource.
    let rr = Rect::new(
        left.right() + 22.0,
        b.y + 16.0,
        b.right() - left.right() - 44.0,
        b.h - 20.0,
    );
    match sel {
        Res::Cpu => cpu(p, pal, rr, false),
        Res::Cores => cpu(p, pal, rr, true),
        Res::Mem => mem(p, pal, rr),
        Res::Disk => disk(p, pal, rr),
        Res::Net => net(p, pal, rr),
        Res::Gpu => gpu(p, pal, rr),
    }
}

fn head(p: &mut Painter, pal: &Palette, r: Rect, title: &str, sub: &str) -> f32 {
    p.text(
        Face::Semibold,
        22.0,
        r.x,
        r.y + 16.0,
        title,
        pal.text,
        r.w * 0.5,
    );
    p.text_right(Face::Ui, 14.0, r.right(), r.y + 17.0, sub, pal.muted);
    r.y + 40.0
}

fn stat(p: &mut Painter, pal: &Palette, x: f32, y: f32, label: &str, val: &str, col: Option<Rgb>) {
    p.text(Face::Ui, 12.0, x, y + 8.0, label, pal.muted, 190.0);
    p.text(
        Face::Semibold,
        18.0,
        x,
        y + 30.0,
        val,
        col.unwrap_or(pal.text),
        190.0,
    );
}

fn fact(p: &mut Painter, pal: &Palette, x: f32, y: f32, label: &str, val: &str, lw: f32) {
    p.text(Face::Ui, 12.0, x, y + 8.0, label, pal.muted, lw);
    p.text(Face::Ui, 12.5, x + lw, y + 8.0, val, pal.text, 200.0);
}

fn top5(p: &mut Painter, pal: &Palette, r: Rect, title: &str, key: &str) {
    p.text(
        Face::Semibold,
        11.5,
        r.x,
        r.y + 8.0,
        &title.to_uppercase(),
        pal.faint,
        r.w,
    );
    let v = d::procs();
    let mut idx: Vec<usize> = (0..v.len()).collect();
    let f = |q: &d::P| match key {
        "mem" => q.mem,
        "disk" => q.disk,
        "gpu" => q.gpu,
        "net" => q.net,
        _ => q.cpu,
    };
    idx.sort_by(|&a, &b| f(&v[b]).total_cmp(&f(&v[a])));
    let mut y = r.y + 24.0;
    for &i in idx.iter().take(5) {
        let q = &v[i];
        w::app_icon(p, r.x, y + 11.0, q.name);
        p.text(
            Face::Ui,
            12.5,
            r.x + 24.0,
            y + 11.0,
            q.name,
            pal.text,
            r.w - 120.0,
        );
        let val = match key {
            "mem" => d::fmt_mem(q.mem),
            "disk" => d::fmt_rate(q.disk, "MB/s"),
            "gpu" => w::pct(q.gpu),
            "net" => d::fmt_rate(q.net, "Mbps"),
            _ => w::pct(q.cpu),
        };
        p.text_right(Face::Ui, 12.5, r.right(), y + 11.0, &val, pal.text);
        // A bar behind the number's scale.
        let mx = f(&v[idx[0]]).max(0.01);
        let bar = Rect::new(r.x + 24.0, y + 22.0, r.w - 24.0, 2.0);
        p.fill(bar, mix(pal.page, pal.text, 0.08));
        p.fill(
            Rect::new(bar.x, bar.y, bar.w * (f(q) / mx).clamp(0.0, 1.0), 2.0),
            pal.accent,
        );
        y += 29.0;
    }
}

fn cpu(p: &mut Painter, pal: &Palette, r: Rect, cores: bool) {
    let mut y = head(p, pal, r, "CPU", "8 cores \u{b7} 16 logical processors");
    p.text(
        Face::Ui,
        12.0,
        r.x,
        y + 4.0,
        if cores {
            "% utilization of each logical processor"
        } else {
            "% utilization over 60 seconds"
        },
        pal.muted,
        400.0,
    );
    p.text_right(Face::Ui, 12.0, r.right(), y + 4.0, "100%", pal.muted);
    y += 16.0;
    let gh = if cores { 258.0 } else { 232.0 };
    let col = [0x11, 0x7d, 0xbb];
    if cores {
        let n = 16;
        let cols = 4;
        let gap = 6.0;
        let cw = (r.w - gap * (cols as f32 - 1.0)) / cols as f32;
        let rh = (gh - gap * 3.0) / 4.0;
        for i in 0..n {
            let (cx, cy) = (i % cols, i / cols);
            let g = Rect::new(
                r.x + cx as f32 * (cw + gap),
                y + cy as f32 * (rh + gap),
                cw,
                rh,
            );
            let busy = if i == 5 || i == 13 {
                0.55
            } else if i % 3 == 0 {
                0.16
            } else {
                0.07
            };
            graph(
                p,
                pal,
                g,
                &d::series(60, busy, 0.18, 40 + i as u32),
                col,
                0.25,
                0,
                true,
            );
            p.text(
                Face::Ui,
                10.5,
                g.x + 5.0,
                g.y + 10.0,
                &format!("{i}"),
                pal.muted,
                30.0,
            );
        }
    } else {
        let g = Rect::new(r.x, y, r.w, gh);
        graph(p, pal, g, &d::series(60, 0.13, 0.22, 3), col, 0.28, 3, true);
        // The kernel share, as a darker line.
        graph(
            p,
            pal,
            g,
            &d::series(60, 0.04, 0.08, 91),
            mix(col, pal.text, 0.35),
            0.0,
            0,
            false,
        );
    }
    y += gh + 20.0;
    spark_labels(p, pal, Rect::new(r.x, y - 20.0, r.w, 0.0), "", "60 s", "0");
    y += 8.0;
    // Four numbers, then the facts and the top five.
    let colw = 150.0;
    stat(p, pal, r.x, y, "Utilization", "11%", None);
    stat(p, pal, r.x + colw, y, "Speed", "4.52 GHz", None);
    stat(p, pal, r.x, y + 56.0, "Processes", "147", None);
    stat(p, pal, r.x + colw, y + 56.0, "Threads", "2,340", None);
    stat(p, pal, r.x, y + 112.0, "Handles", "64,810", None);
    stat(p, pal, r.x + colw, y + 112.0, "Up time", "0:03:07:21", None);
    let fx = r.x + colw * 2.0 + 28.0;
    let mut fy = y;
    for (a, b) in [
        ("Base speed", "3.80 GHz"),
        ("Sockets", "1"),
        ("Cores", "8"),
        ("Logical processors", "16"),
        ("L1 / L2 / L3 cache", "512 KB / 4 MB / 32 MB"),
    ] {
        fact(p, pal, fx, fy, a, b, 130.0);
        fy += 22.0;
    }
    let tx = r.right() - 250.0;
    if tx > fx + 300.0 {
        top5(
            p,
            pal,
            Rect::new(tx, y - 6.0, 250.0, 160.0),
            "Busiest now",
            "cpu",
        );
    }
}

fn composition(p: &mut Painter, pal: &Palette, r: Rect) {
    let parts: [(&str, f32, Rgb); 4] = [
        ("In use 17.3 GB", 17.3, [0x8b, 0x12, 0xae]),
        ("Modified 0.2 GB", 0.2, [0xc8, 0x6f, 0xdc]),
        ("Standby 9.8 GB", 9.8, [0x9a, 0x8f, 0xb8]),
        ("Free 4.7 GB", 4.7, mix(pal.page, pal.text, 0.1)),
    ];
    let mut x = r.x;
    for (_, v, c) in &parts {
        let wd = r.w * v / 32.0;
        p.fill(Rect::new(x, r.y, wd.max(2.0), 26.0), *c);
        x += wd;
    }
    let mut lx = r.x;
    for (l, _, c) in &parts {
        p.rbox(Rect::new(lx, r.y + 38.0, 10.0, 10.0), 2.0, *c);
        let wd = p.text(Face::Ui, 12.0, lx + 16.0, r.y + 43.0, l, pal.muted, 200.0);
        lx += wd + 36.0;
    }
}

fn mem(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = head(
        p,
        pal,
        r,
        "Memory",
        "32.0 GB \u{b7} DDR5 4800 MT/s \u{b7} 2 of 4 slots",
    );
    p.text(
        Face::Ui,
        12.0,
        r.x,
        y + 4.0,
        "Memory usage",
        pal.muted,
        300.0,
    );
    p.text_right(Face::Ui, 12.0, r.right(), y + 4.0, "32 GB", pal.muted);
    y += 16.0;
    let g = Rect::new(r.x, y, r.w, 168.0);
    graph(
        p,
        pal,
        g,
        &d::series(60, 0.54, 0.03, 8),
        [0x8b, 0x12, 0xae],
        0.28,
        3,
        true,
    );
    y += 168.0 + 12.0;
    p.text(
        Face::Ui,
        12.0,
        r.x,
        y + 4.0,
        "Memory composition",
        pal.muted,
        300.0,
    );
    y += 14.0;
    composition(p, pal, Rect::new(r.x, y, r.w, 26.0));
    y += 66.0;
    let colw = 170.0;
    stat(p, pal, r.x, y, "In use (compressed)", "17.3 GB (0.9)", None);
    stat(p, pal, r.x + colw, y, "Available", "14.7 GB", None);
    stat(p, pal, r.x, y + 56.0, "Committed", "22.8 / 38.0 GB", None);
    stat(p, pal, r.x + colw, y + 56.0, "Cached", "9.9 GB", None);
    stat(p, pal, r.x, y + 112.0, "Paged pool", "612 MB", None);
    stat(
        p,
        pal,
        r.x + colw,
        y + 112.0,
        "Non-paged pool",
        "482 MB",
        None,
    );
    let tx = r.right() - 270.0;
    top5(
        p,
        pal,
        Rect::new(tx, y - 6.0, 270.0, 160.0),
        "Largest by private working set",
        "mem",
    );
}

fn disk(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = head(p, pal, r, "Disk 0 (C:)", "931 GB \u{b7} SSD (NVMe)");
    p.text(
        Face::Ui,
        12.0,
        r.x,
        y + 4.0,
        "Active time",
        pal.muted,
        300.0,
    );
    p.text_right(Face::Ui, 12.0, r.right(), y + 4.0, "100%", pal.muted);
    y += 16.0;
    graph(
        p,
        pal,
        Rect::new(r.x, y, r.w, 120.0),
        &d::series(60, 0.04, 0.12, 12),
        [0x4d, 0xa1, 0x2f],
        0.28,
        3,
        true,
    );
    y += 120.0 + 14.0;
    p.text(
        Face::Ui,
        12.0,
        r.x,
        y + 4.0,
        "Disk transfer rate",
        pal.muted,
        300.0,
    );
    p.text_right(Face::Ui, 12.0, r.right(), y + 4.0, "100 MB/s", pal.muted);
    y += 16.0;
    let g = Rect::new(r.x, y, r.w, 96.0);
    graph(
        p,
        pal,
        g,
        &d::series(60, 0.09, 0.2, 31),
        [0x4d, 0xa1, 0x2f],
        0.2,
        1,
        true,
    );
    graph(
        p,
        pal,
        g,
        &d::series(60, 0.05, 0.16, 32),
        mix([0x4d, 0xa1, 0x2f], pal.text, 0.5),
        0.0,
        0,
        false,
    );
    y += 96.0 + 24.0;
    let colw = 150.0;
    stat(p, pal, r.x, y, "Active time", "1%", None);
    stat(p, pal, r.x + colw, y, "Average response", "0.4 ms", None);
    stat(p, pal, r.x, y + 56.0, "Read speed", "1.2 MB/s", None);
    stat(
        p,
        pal,
        r.x + colw,
        y + 56.0,
        "Write speed",
        "0.9 MB/s",
        None,
    );
    let fx = r.x + colw * 2.0 + 28.0;
    let mut fy = y;
    for (a, b) in [
        ("Capacity", "931 GB"),
        ("Formatted", "930 GB"),
        ("System disk", "Yes"),
        ("Page file", "Yes"),
        ("Type", "SSD"),
    ] {
        fact(p, pal, fx, fy, a, b, 100.0);
        fy += 22.0;
    }
    top5(
        p,
        pal,
        Rect::new(r.right() - 250.0, y - 6.0, 250.0, 160.0),
        "Most I/O now",
        "disk",
    );
}

fn net(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = head(p, pal, r, "Wi-Fi", "Intel-class AX adapter \u{b7} 866 Mbps");
    p.text(Face::Ui, 12.0, r.x, y + 4.0, "Throughput", pal.muted, 300.0);
    p.text_right(Face::Ui, 12.0, r.right(), y + 4.0, "10 Mbps", pal.muted);
    y += 16.0;
    let g = Rect::new(r.x, y, r.w, 200.0);
    let col = [0xa7, 0x4f, 0x01];
    graph(p, pal, g, &d::series(60, 0.10, 0.2, 19), col, 0.25, 3, true);
    graph(
        p,
        pal,
        g,
        &d::series(60, 0.03, 0.08, 20),
        mix(col, pal.text, 0.5),
        0.0,
        0,
        false,
    );
    y += 200.0 + 24.0;
    let colw = 150.0;
    stat(
        p,
        pal,
        r.x,
        y,
        "Send",
        "0.1 Mbps",
        Some(mix(col, pal.text, 0.4)),
    );
    stat(p, pal, r.x + colw, y, "Receive", "1.1 Mbps", Some(col));
    let fx = r.x + colw * 2.0 + 28.0;
    let mut fy = y;
    for (a, b) in [
        ("Adapter name", "Wi-Fi"),
        ("Connection type", "802.11ax"),
        ("IPv4 address", "192.168.1.24"),
        ("Link speed", "866 / 866 Mbps"),
    ] {
        fact(p, pal, fx, fy, a, b, 130.0);
        fy += 22.0;
    }
    // Per-process bytes need administrator rights: say so where the list would be.
    let nb = Rect::new(r.x, y + 70.0, r.w.min(560.0), 70.0);
    p.frame(nb, 8.0, mix(pal.page, pal.text, 0.03), pal.line);
    p.icon(SHIELD, 18.0, nb.x + 24.0, nb.cy(), amber(pal));
    ui::para(
        p,
        Face::Ui,
        12.5,
        nb.x + 48.0,
        nb.y + 10.0,
        nb.w - 62.0,
        "Per-program traffic is not shown: Windows only gives it to a trace session, and that needs administrator rights. The numbers above are the adapter's own.",
        pal.muted,
    );
    p.text(
        Face::Semibold,
        12.5,
        nb.x + 48.0,
        nb.bottom() + 16.0,
        "Count traffic per program\u{2026}",
        blue(pal),
        300.0,
    );
}

fn gpu(p: &mut Painter, pal: &Palette, r: Rect) {
    let mut y = head(p, pal, r, "GPU 0", "Discrete adapter \u{b7} 12 GB");
    let col = [0x11, 0x7d, 0xbb];
    let gap = 10.0;
    let cw = (r.w - gap) / 2.0;
    let ch = 92.0;
    for (i, (l, seed, base)) in [
        ("3D", 5, 0.14),
        ("Copy", 6, 0.03),
        ("Video decode", 7, 0.20),
        ("Video encode", 8, 0.0),
    ]
    .iter()
    .enumerate()
    {
        let (cx, cy) = (i % 2, i / 2);
        let x = r.x + cx as f32 * (cw + gap);
        let yy = y + cy as f32 * (ch + 26.0);
        p.text(Face::Ui, 12.0, x, yy + 6.0, l, pal.muted, 200.0);
        p.text_right(Face::Ui, 12.0, x + cw, yy + 6.0, "100%", pal.muted);
        graph(
            p,
            pal,
            Rect::new(x, yy + 16.0, cw, ch),
            &d::series(60, *base, 0.18, *seed),
            col,
            0.25,
            1,
            true,
        );
    }
    y += 2.0 * (ch + 26.0) + 4.0;
    p.text(
        Face::Ui,
        12.0,
        r.x,
        y + 6.0,
        "Dedicated GPU memory usage",
        pal.muted,
        300.0,
    );
    p.text_right(Face::Ui, 12.0, r.x + cw, y + 6.0, "12 GB", pal.muted);
    graph(
        p,
        pal,
        Rect::new(r.x, y + 16.0, cw, 80.0),
        &d::series(60, 0.27, 0.02, 14),
        col,
        0.25,
        1,
        true,
    );
    p.text(
        Face::Ui,
        12.0,
        r.x + cw + gap,
        y + 6.0,
        "Shared GPU memory usage",
        pal.muted,
        300.0,
    );
    p.text_right(Face::Ui, 12.0, r.right(), y + 6.0, "15.9 GB", pal.muted);
    graph(
        p,
        pal,
        Rect::new(r.x + cw + gap, y + 16.0, cw, 80.0),
        &d::series(60, 0.03, 0.01, 15),
        col,
        0.25,
        1,
        true,
    );
    y += 120.0;
    let colw = 170.0;
    stat(p, pal, r.x, y, "Utilization", "14%", None);
    stat(
        p,
        pal,
        r.x + colw,
        y,
        "Dedicated memory",
        "3.2 / 12.0 GB",
        None,
    );
    let fx = r.x + colw * 2.0 + 28.0;
    let mut fy = y;
    for (a, b) in [
        ("Driver version", "32.0.15.6094"),
        ("DirectX version", "12 (FL 12_2)"),
        ("Temperature", "not shown"),
    ] {
        fact(p, pal, fx, fy, a, b, 120.0);
        fy += 22.0;
    }
    top5(
        p,
        pal,
        Rect::new(r.right() - 250.0, y - 6.0, 250.0, 160.0),
        "Busiest now",
        "gpu",
    );
}

// ---- Startup ----------------------------------------------------------------------------------

pub fn startup_tool(p: &mut Painter, pal: &Palette, r: Rect, width: f32) {
    p.fill(r, pal.page);
    let cy = r.cy();
    search_field(
        p,
        pal,
        Rect::new(
            10.0,
            cy - 15.0,
            if width < w::COMPACT_W { 170.0 } else { 230.0 },
            30.0,
        ),
        "",
        "Search startup apps",
    );
    let mut x = 10.0 + if width < w::COMPACT_W { 182.0 } else { 242.0 };
    for (l, on) in [
        ("All 10", true),
        ("Run keys", false),
        ("Startup folders", false),
        ("At logon", false),
    ] {
        let wd = p.measure(Face::Ui, 12.5, l) + 20.0;
        let b = Rect::new(x, cy - 13.0, wd, 26.0);
        if on {
            p.rbox(b, 13.0, mix(pal.page, pal.accent, 0.22));
        } else {
            p.frame(b, 13.0, pal.page, pal.line);
        }
        p.text_center(Face::Ui, 12.5, b, l, if on { pal.text } else { pal.muted });
        x += wd + 6.0;
        if width < w::COMPACT_W && x > width - 200.0 {
            break;
        }
    }
    p.text_right(
        Face::Ui,
        12.5,
        width - 14.0,
        cy,
        "Last sign-in: 8.2 s to desktop \u{b7} 4 apps on",
        pal.muted,
    );
}

fn switch(p: &mut Painter, pal: &Palette, x: f32, cy: f32, on: bool, label: &str, shield: bool) {
    let r = Rect::new(x, cy - 9.0, 36.0, 18.0);
    if on {
        p.rbox(r, 9.0, pal.accent);
        p.rbox(
            Rect::new(r.right() - 16.0, r.y + 2.0, 14.0, 14.0),
            7.0,
            pal.on_accent,
        );
    } else {
        p.frame(r, 9.0, mix(pal.page, pal.text, 0.04), pal.muted);
        p.rbox(Rect::new(r.x + 3.0, r.y + 3.0, 12.0, 12.0), 6.0, pal.muted);
    }
    p.text(
        Face::Ui,
        12.5,
        x + 46.0,
        cy,
        label,
        if on { pal.text } else { pal.muted },
        220.0,
    );
    if shield {
        p.icon(SHIELD, 12.0, x + 124.0, cy, amber(pal));
    }
}

pub fn startup_page(p: &mut Painter, pal: &Palette, b: Rect, sel: usize) {
    let dw = if b.w < w::COMPACT_W { 300.0 } else { 410.0 };
    let t = Rect::new(b.x, b.y, b.w - dw, b.h);
    let compact = b.w < w::COMPACT_W;
    // Head.
    p.fill(Rect::new(t.x, t.y + 34.0, t.w, p.px().max(1.0)), pal.line);
    let cols: Vec<(f32, &str)> = if compact {
        vec![
            (0.0, "Name"),
            (96.0, "Source"),
            (84.0, "Impact"),
            (160.0, "Status"),
        ]
    } else {
        vec![
            (0.0, "Name"),
            (150.0, "Publisher"),
            (130.0, "Where it starts"),
            (84.0, "Impact"),
            (160.0, "Status"),
        ]
    };
    let fixed: f32 = cols.iter().map(|c| c.0).sum();
    let nw = t.w - fixed;
    let mut xs = Vec::new();
    let mut x = t.x;
    for (wd, l) in &cols {
        let wd = if *l == "Name" { nw } else { *wd };
        xs.push((x, wd, *l));
        x += wd;
        p.text(
            Face::Ui,
            12.0,
            xs.last().unwrap().0 + if *l == "Name" { 16.0 } else { 10.0 },
            t.y + 17.0,
            l,
            pal.muted,
            wd - 12.0,
        );
    }
    let mut y = t.y + 35.0;
    let list = d::startup();
    for (i, s) in list.iter().enumerate() {
        let r = Rect::new(t.x, y, t.w, 40.0);
        if i == sel {
            p.fill(r, mix(pal.page, pal.accent, 0.16));
            p.fill(Rect::new(r.x, r.y, 3.0, r.h), pal.accent);
        }
        let cy = r.cy();
        for &(x, wd, l) in &xs {
            match l {
                "Name" => {
                    w::app_icon(p, x + 16.0, cy, s.name);
                    p.text(
                        Face::Ui,
                        13.0,
                        x + 40.0,
                        cy,
                        s.name,
                        if s.on { pal.text } else { pal.muted },
                        wd - 48.0,
                    );
                }
                "Publisher" => {
                    let c = if s.publisher == "Not signed" {
                        amber(pal)
                    } else {
                        pal.muted
                    };
                    p.text(Face::Ui, 12.5, x + 10.0, cy, s.publisher, c, wd - 14.0);
                }
                "Where it starts" | "Source" => {
                    p.text(Face::Ui, 12.5, x + 10.0, cy, s.src, pal.muted, wd - 14.0);
                }
                "Impact" => {
                    let c = match s.impact {
                        "High" => red(pal),
                        "Medium" => amber(pal),
                        _ => green(pal),
                    };
                    p.rbox(Rect::new(x + 10.0, cy - 4.0, 8.0, 8.0), 4.0, c);
                    p.text(Face::Ui, 12.5, x + 24.0, cy, s.impact, pal.text, wd - 28.0);
                }
                _ => {
                    switch(
                        p,
                        pal,
                        x + 10.0,
                        cy,
                        s.on,
                        if s.on { "On" } else { "Off" },
                        s.admin,
                    );
                }
            }
        }
        p.fill(
            Rect::new(t.x, r.bottom() - p.px().max(1.0), t.w, p.px().max(1.0)),
            mix(pal.page, pal.line, 0.5),
        );
        y += 40.0;
    }
    // The pane: what this entry is, and exactly what turning it off writes.
    let r = Rect::new(t.right(), b.y, dw, b.h);
    p.fill(r, pal.pane);
    p.fill(Rect::new(r.x, r.y, p.px().max(1.0), r.h), pal.line);
    let s = &list[sel];
    let pad = 18.0;
    let mut y = r.y + 18.0;
    w::app_icon(p, r.x + pad, y + 12.0, s.name);
    p.text(
        Face::Semibold,
        15.0,
        r.x + pad + 28.0,
        y + 12.0,
        s.name,
        pal.text,
        r.w - 80.0,
    );
    y += 32.0;
    p.text(
        Face::Ui,
        12.5,
        r.x + pad,
        y + 8.0,
        s.publisher,
        pal.muted,
        r.w - 36.0,
    );
    y += 28.0;
    switch(
        p,
        pal,
        r.x + pad,
        y + 10.0,
        s.on,
        if s.on {
            "Starts at sign-in"
        } else {
            "Does not start"
        },
        s.admin,
    );
    y += 34.0;
    y = ui::kv(
        p,
        pal,
        r.x + pad,
        y,
        80.0,
        r.w - 2.0 * pad,
        "Command",
        s.cmd,
        true,
    );
    y = ui::kv(
        p,
        pal,
        r.x + pad,
        y,
        80.0,
        r.w - 2.0 * pad,
        "From",
        s.src,
        false,
    );
    y += 8.0;
    p.text(
        Face::Semibold,
        11.5,
        r.x + pad,
        y + 8.0,
        "TURNING IT OFF WRITES",
        pal.faint,
        300.0,
    );
    y += 22.0;
    let code = Rect::new(r.x + pad, y, r.w - 2.0 * pad, 84.0);
    p.frame(code, 6.0, mix(pal.pane, pal.text, 0.05), pal.line);
    let lines = [
        "HKCU\\Software\\Microsoft\\Windows\\",
        "  CurrentVersion\\Explorer\\StartupApproved\\Run",
        &format!(
            "  {} = 03 00 00 00 \u{2026}",
            s.name.split(' ').next().unwrap_or("")
        ),
    ];
    for (k, l) in lines.iter().enumerate() {
        p.text(
            Face::Mono,
            11.0,
            code.x + 10.0,
            code.y + 16.0 + k as f32 * 17.0,
            l,
            pal.text,
            code.w - 16.0,
        );
    }
    y += 94.0;
    y = ui::para(
        p,
        Face::Ui,
        12.0,
        r.x + pad,
        y,
        r.w - 2.0 * pad,
        "The entry itself is not deleted, so turning it on again restores it exactly. This is the same switch Windows Settings uses. A program that is running now keeps running until you end it.",
        pal.muted,
    );
    y += 10.0;
    ui::sbtn(
        p,
        pal,
        r.x + pad,
        y + 14.0,
        "Open file location",
        false,
        true,
    );
    ui::sbtn(
        p,
        pal,
        r.x + pad + 150.0,
        y + 14.0,
        "Registry entry",
        false,
        true,
    );
    y += 44.0;
    if s.admin {
        p.frame(
            Rect::new(r.x + pad, y, r.w - 2.0 * pad, 54.0),
            6.0,
            mix(pal.pane, amber(pal), 0.10),
            mix(pal.line, amber(pal), 0.4),
        );
        p.icon(SHIELD, 15.0, r.x + pad + 20.0, y + 27.0, amber(pal));
        ui::para(
            p,
            Face::Ui,
            12.0,
            r.x + pad + 38.0,
            y + 8.0,
            r.w - 2.0 * pad - 48.0,
            "This entry is for all users (HKLM): changing it asks Windows for administrator rights, once.",
            pal.muted,
        );
    }
}

// ---- Services ---------------------------------------------------------------------------------

pub fn services_tool(p: &mut Painter, pal: &Palette, r: Rect, width: f32) {
    p.fill(r, pal.page);
    let cy = r.cy();
    search_field(
        p,
        pal,
        Rect::new(
            10.0,
            cy - 15.0,
            if width < w::COMPACT_W { 170.0 } else { 230.0 },
            30.0,
        ),
        "",
        "Search services",
    );
    let mut x = 10.0 + if width < w::COMPACT_W { 182.0 } else { 242.0 };
    for (l, on) in [
        ("All", false),
        ("Running", true),
        ("Stopped", false),
        ("Not Microsoft", false),
    ] {
        let wd = p.measure(Face::Ui, 12.5, l) + 20.0;
        let b = Rect::new(x, cy - 13.0, wd, 26.0);
        if on {
            p.rbox(b, 13.0, mix(pal.page, pal.accent, 0.22));
        } else {
            p.frame(b, 13.0, pal.page, pal.line);
        }
        p.text_center(Face::Ui, 12.5, b, l, if on { pal.text } else { pal.muted });
        x += wd + 6.0;
        if width < w::COMPACT_W && x > width - 260.0 {
            break;
        }
    }
    x += 8.0;
    sep(p, pal, &mut x, cy);
    let compact = width < w::COMPACT_W;
    for (g, l) in [
        (glyph::PLAY, "Start"),
        (glyph::STOP, "Stop"),
        (glyph::RELOAD, "Restart"),
    ] {
        tbtn(
            p,
            pal,
            &mut x,
            cy,
            g,
            if compact { None } else { Some(l) },
            true,
            false,
        );
        p.icon(SHIELD, 9.0, x - 6.0, cy - 8.0, amber(pal));
    }
}

pub fn services_page(p: &mut Painter, pal: &Palette, b: Rect, sel: usize) {
    let dw = if b.w < w::COMPACT_W { 300.0 } else { 380.0 };
    let t = Rect::new(b.x, b.y, b.w - dw, b.h);
    let compact = b.w < w::COMPACT_W;
    p.fill(Rect::new(t.x, t.y + 34.0, t.w, p.px().max(1.0)), pal.line);
    let cols: Vec<(f32, &str)> = if compact {
        vec![
            (0.0, "Service"),
            (60.0, "PID"),
            (96.0, "Status"),
            (140.0, "Startup"),
        ]
    } else {
        vec![
            (0.0, "Service"),
            (230.0, "Name"),
            (62.0, "PID"),
            (96.0, "Status"),
            (150.0, "Startup type"),
            (130.0, "Log on as"),
        ]
    };
    let fixed: f32 = cols.iter().map(|c| c.0).sum();
    let mut xs = Vec::new();
    let mut x = t.x;
    for (wd, l) in &cols {
        let wd = if *l == "Service" { t.w - fixed } else { *wd };
        xs.push((x, wd, *l));
        p.text(
            Face::Ui,
            12.0,
            x + if *l == "Service" { 16.0 } else { 10.0 },
            t.y + 17.0,
            l,
            pal.muted,
            wd - 12.0,
        );
        x += wd;
    }
    let list = d::services();
    let mut y = t.y + 35.0;
    for (i, s) in list.iter().filter(|s| s.running).enumerate() {
        let r = Rect::new(t.x, y, t.w, 32.0);
        if i == sel {
            p.fill(r, mix(pal.page, pal.accent, 0.16));
            p.fill(Rect::new(r.x, r.y, 3.0, r.h), pal.accent);
        }
        let cy = r.cy();
        for &(x, wd, l) in &xs {
            match l {
                "Service" => {
                    p.rbox(Rect::new(x + 16.0, cy - 4.0, 8.0, 8.0), 4.0, green(pal));
                    p.text(Face::Ui, 13.0, x + 32.0, cy, s.desc, pal.text, wd - 40.0);
                }
                "Name" => {
                    p.text(Face::Mono, 11.5, x + 10.0, cy, s.name, pal.muted, wd - 14.0);
                }
                "PID" => {
                    p.text(
                        Face::Ui,
                        12.5,
                        x + 10.0,
                        cy,
                        &s.pid.to_string(),
                        pal.muted,
                        wd - 14.0,
                    );
                }
                "Status" => {
                    p.text(
                        Face::Ui,
                        12.5,
                        x + 10.0,
                        cy,
                        "Running",
                        green(pal),
                        wd - 14.0,
                    );
                }
                "Startup type" | "Startup" => {
                    let c = if s.start == "Disabled" {
                        red(pal)
                    } else {
                        pal.text
                    };
                    p.text(Face::Ui, 12.5, x + 10.0, cy, s.start, c, wd - 14.0);
                }
                _ => {
                    p.text(Face::Ui, 12.5, x + 10.0, cy, s.logon, pal.muted, wd - 14.0);
                }
            }
        }
        p.fill(
            Rect::new(t.x, r.bottom() - p.px().max(1.0), t.w, p.px().max(1.0)),
            mix(pal.page, pal.line, 0.5),
        );
        y += 32.0;
    }
    let r = Rect::new(t.right(), b.y, dw, b.h);
    p.fill(r, pal.pane);
    p.fill(Rect::new(r.x, r.y, p.px().max(1.0), r.h), pal.line);
    let running: Vec<&d::Svc> = list.iter().filter(|s| s.running).collect();
    let s = running[sel.min(running.len() - 1)];
    let pad = 18.0;
    let mut y = r.y + 18.0;
    p.text(
        Face::Semibold,
        15.0,
        r.x + pad,
        y + 10.0,
        s.desc,
        pal.text,
        r.w - 36.0,
    );
    y += 26.0;
    p.text(
        Face::Mono,
        11.5,
        r.x + pad,
        y + 8.0,
        s.name,
        pal.muted,
        r.w - 36.0,
    );
    y += 28.0;
    y = ui::para(
        p,
        Face::Ui,
        12.5,
        r.x + pad,
        y,
        r.w - 2.0 * pad,
        "Lets you search for files, e-mail and other content, with fast results, by indexing it in the background.",
        pal.muted,
    );
    y += 8.0;
    let lw = 92.0;
    let ww = r.w - 2.0 * pad;
    y = ui::kv(
        p,
        pal,
        r.x + pad,
        y,
        lw,
        ww,
        "Runs in",
        "SearchIndexer.exe (PID 6724)",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x + pad,
        y,
        lw,
        ww,
        "Binary",
        "C:\\Windows\\system32\\SearchIndexer.exe /Embedding",
        true,
    );
    y = ui::kv(p, pal, r.x + pad, y, lw, ww, "Startup type", s.start, false);
    y = ui::kv(p, pal, r.x + pad, y, lw, ww, "Log on as", s.logon, false);
    y = ui::kv(
        p,
        pal,
        r.x + pad,
        y,
        lw,
        ww,
        "Depends on",
        "RPCSS \u{b7} Dnscache is not needed",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x + pad,
        y,
        lw,
        ww,
        "If it fails",
        "Restart the service (after 1 min)",
        false,
    );
    y += 10.0;
    ui::sbtn(p, pal, r.x + pad, y + 14.0, "Stop", false, true);
    p.icon(SHIELD, 11.0, r.x + pad + 8.0, y + 14.0 - 12.0, amber(pal));
    ui::sbtn(p, pal, r.x + pad + 70.0, y + 14.0, "Restart", false, true);
    ui::sbtn(
        p,
        pal,
        r.x + pad + 150.0,
        y + 14.0,
        "Startup type\u{2026}",
        false,
        true,
    );
    y += 48.0;
    ui::para(
        p,
        Face::Ui,
        12.0,
        r.x + pad,
        y,
        ww,
        "The list is read without administrator rights. Stop, Start and Startup type ask Windows once for each action.",
        pal.faint,
    );
}

// ---- Files in use -----------------------------------------------------------------------------

pub fn files_tool(p: &mut Painter, pal: &Palette, r: Rect, width: f32) {
    p.fill(r, pal.page);
    let cy = r.cy();
    p.text(
        Face::Semibold,
        13.0,
        14.0,
        cy,
        "What is holding a file or folder?",
        pal.text,
        300.0,
    );
    p.text_right(
        Face::Ui,
        12.5,
        width - 14.0,
        cy,
        "Also in Explorer: right-click \u{2192} \u{201c}What\u{2019}s using this?\u{201d}",
        pal.muted,
    );
}

pub fn files_page(p: &mut Painter, pal: &Palette, b: Rect, deep: bool) {
    let pad = 24.0;
    let r = Rect::new(b.x + pad, b.y + 20.0, b.w - 2.0 * pad, b.h - 30.0);
    // The path field with Browse.
    let fy = r.y;
    let path = if deep {
        "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\target"
    } else {
        "C:\\Users\\kaan\\Desktop\\markdown-editor-ai\\target\\release\\mdedit.exe"
    };
    let bw = 100.0;
    ui::field(
        p,
        pal,
        Rect::new(r.x, fy, r.w - bw - 100.0 - 16.0, 36.0),
        path,
        true,
        true,
    );
    ui::sbtn(
        p,
        pal,
        r.right() - bw - 100.0 - 8.0,
        fy + 18.0,
        "Browse\u{2026}",
        false,
        true,
    );
    ui::sbtn_r(p, pal, r.right(), fy + 18.0, "Find", true, true);
    p.text(
        Face::Ui,
        12.0,
        r.x,
        fy + 54.0,
        "Drop a file or a folder here, or paste a path.",
        pal.faint,
        400.0,
    );
    // Quick or deep.
    let sy = fy + 82.0;
    let x = seg(
        p,
        pal,
        r.x,
        sy + 14.0,
        &[(glyph::SEARCH, "Quick"), (glyph::ZOOM, "Deep")],
        if deep { 1 } else { 0 },
        false,
    );
    let msg = if deep {
        "Looks at every open handle on the machine. Finds folders and working directories too. Takes a second or two."
    } else {
        "Asks Windows which programs have this file open or loaded. Instant. Does not see folders."
    };
    p.text(
        Face::Ui,
        12.5,
        x + 16.0,
        sy + 14.0,
        msg,
        pal.muted,
        r.right() - x - 16.0,
    );
    // Results.
    let ty = sy + 48.0;
    let (headline, rows): (String, Vec<(&str, u32, &str, &str, &str)>) = if deep {
        (
            "4 programs hold something inside this folder".into(),
            vec![
                (
                    "rustc.exe",
                    12712,
                    "kaan",
                    "File  target\\release\\deps\\mdedit-3f2a1c.exe  (3 handles)",
                    "Show in Processes",
                ),
                (
                    "cargo.exe",
                    12480,
                    "kaan",
                    "Working directory  target\\release",
                    "Show in Processes",
                ),
                (
                    "pwsh.exe",
                    10100,
                    "kaan",
                    "Current directory  target",
                    "Show in Processes",
                ),
                (
                    "explorer.exe",
                    6120,
                    "kaan",
                    "Folder open in a window  target\\release",
                    "Close window",
                ),
            ],
        )
    } else {
        (
            "3 programs have this file open".into(),
            vec![
                (
                    "mdedit.exe",
                    14832,
                    "kaan",
                    "Running copy of the program \u{b7} application \u{b7} can be restarted",
                    "Close app",
                ),
                (
                    "SearchIndexer.exe",
                    6724,
                    "SYSTEM",
                    "Windows Search \u{b7} service \u{b7} reading it to index",
                    "Stop service",
                ),
                (
                    "MsMpEng.exe",
                    3380,
                    "SYSTEM",
                    "Microsoft Defender \u{b7} service \u{b7} scanning it",
                    "Show in Processes",
                ),
            ],
        )
    };
    p.text(
        Face::Semibold,
        14.0,
        r.x,
        ty + 8.0,
        &headline,
        pal.text,
        r.w,
    );
    let mut y = ty + 28.0;
    for (i, (name, pid, user, why, act)) in rows.iter().enumerate() {
        let rr = Rect::new(r.x, y, r.w, 58.0);
        p.frame(
            rr,
            8.0,
            if i == 0 {
                mix(pal.page, pal.accent, 0.10)
            } else {
                mix(pal.page, pal.text, 0.025)
            },
            pal.line,
        );
        w::app_icon(p, rr.x + 16.0, rr.cy() - 8.0, name);
        p.text(
            Face::Semibold,
            13.5,
            rr.x + 42.0,
            rr.y + 20.0,
            name,
            pal.text,
            300.0,
        );
        let nw = p.measure(Face::Semibold, 13.5, name);
        p.text(
            Face::Ui,
            12.0,
            rr.x + 52.0 + nw,
            rr.y + 20.0,
            &format!("PID {pid} \u{b7} {user}"),
            pal.muted,
            300.0,
        );
        p.text(
            Face::Ui,
            12.5,
            rr.x + 42.0,
            rr.y + 40.0,
            why,
            pal.muted,
            rr.w - 280.0,
        );
        let admin = *act == "Stop service";
        let sys = *user == "SYSTEM";
        let bx = rr.right() - 14.0;
        let wd = (p.measure(Face::Ui, 12.5, act) + 26.0 + if admin { 18.0 } else { 0.0 }).round();
        let b2 = Rect::new(bx - wd, rr.cy() - 14.0, wd, 28.0);
        p.frame(b2, 5.0, mix(pal.page, pal.text, 0.04), pal.line);
        p.text_center(
            Face::Ui,
            12.5,
            Rect::new(
                b2.x + if admin { 16.0 } else { 0.0 },
                b2.y,
                b2.w - if admin { 16.0 } else { 0.0 },
                b2.h,
            ),
            act,
            pal.text,
        );
        if admin {
            p.icon(SHIELD, 11.0, b2.x + 14.0, b2.cy(), amber(pal));
        }
        let ew = if sys { 100.0 } else { 82.0 };
        let end = Rect::new(b2.x - ew - 10.0, rr.cy() - 14.0, ew, 28.0);
        p.frame(
            end,
            5.0,
            mix(pal.page, red(pal), 0.10),
            mix(pal.line, red(pal), 0.5),
        );
        p.text_center(
            Face::Ui,
            12.5,
            Rect::new(
                end.x + if sys { 16.0 } else { 0.0 },
                end.y,
                end.w - if sys { 16.0 } else { 0.0 },
                end.h,
            ),
            "End task",
            red(pal),
        );
        if sys {
            p.icon(SHIELD, 11.0, end.x + 14.0, end.cy(), amber(pal));
        }
        y += 66.0;
    }
    // Footnote: what each method cannot see.
    y += 6.0;
    let note = if deep {
        "Scanned 118,402 handles in 1.4 s. 3 handles in protected processes could not be read, even as administrator. Folders and working directories are the cases the quick search cannot see."
    } else {
        "Not shown: a folder, or a program whose current directory is inside it. Use Deep for those. \u{201c}Close app\u{201d} asks the program to close politely; it saves nothing for you."
    };
    p.icon(glyph::INFO, 14.0, r.x + 8.0, y + 12.0, pal.muted);
    ui::para(
        p,
        Face::Ui,
        12.5,
        r.x + 24.0,
        y,
        r.w - 30.0,
        note,
        pal.muted,
    );
    // Last: the dangerous one, stated, not offered.
    y += 50.0;
    p.frame(
        Rect::new(r.x, y, r.w, 50.0),
        8.0,
        mix(pal.page, red(pal), 0.06),
        mix(pal.line, red(pal), 0.35),
    );
    p.icon(glyph::WARN, 16.0, r.x + 22.0, y + 25.0, red(pal));
    ui::para(
        p,
        Face::Ui,
        12.5,
        r.x + 44.0,
        y + 6.0,
        r.w - 58.0,
        "There is no button that closes a handle inside another program. It corrupts the program\u{2019}s state and can lose its unsaved work. End the program, or close it, instead.",
        pal.muted,
    );
}
