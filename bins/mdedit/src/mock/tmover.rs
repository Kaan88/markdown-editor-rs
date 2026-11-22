//! What opens over the task manager: the context menu of a process, the questions (end a tree,
//! affinity, administrator rights), Settings, the cost popover, the tray flyout and the compact
//! always-on-top window.

use super::tmdata as d;
use super::tmpages::{self as pg, graph};
use super::tmwin::{self as w, DTab, Page, SHIELD, Scene, View, amber, blue, green, red};
use super::ui;
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::menu::{Item, Menu};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_card,
};
use mdshared_ui::theme::Palette;

type M = Menu<u8>;
fn it(id: u8, l: &str) -> Item<u8> {
    Item::new(id, l)
}

fn footer(p: &mut Painter, pal: &Palette, f: Rect, l: f32, r: f32, left: &[&str], right: &[&str]) {
    let by = f.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut x = l;
    for t in left {
        let wd = dialog_button_w(p, t);
        draw_dialog_button(p, pal, Rect::new(x, by, wd, 32.0), t, false, false, false);
        x += wd + 10.0;
    }
    let mut x = r;
    for (i, t) in right.iter().enumerate().rev() {
        let wd = dialog_button_w(p, t);
        let last = i == right.len() - 1;
        draw_dialog_button(
            p,
            pal,
            Rect::new(x - wd, by, wd, 32.0),
            t,
            last,
            false,
            false,
        );
        x -= wd + 10.0;
    }
}

// ---- the context menu -------------------------------------------------------------------------

pub fn context(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let mut sc = Scene::default();
    sc.hover = Some(12712);
    let rows = w::window(p, pal, width, h, &sc);
    let row = rows
        .iter()
        .find(|r| r.0 == 12712)
        .map(|r| r.1)
        .unwrap_or(Rect::new(0.0, 300.0, width, 28.0));
    let items = vec![
        it(0, "End task").icon(w::KILL).hint("Del"),
        it(1, "End process tree").hint("Shift+Del"),
        it(2, "Suspend").icon(w::PAUSE).hint("Ctrl+Space").sep(),
        it(3, "Priority"),
        it(4, "Efficiency mode"),
        it(5, "Set affinity\u{2026}").icon(glyph::SLIDERS).sep(),
        it(6, "Open file location")
            .icon(glyph::FOLDER_OPEN)
            .hint("Ctrl+L")
            .sep(),
        it(7, "Copy command line").icon(glyph::COPY),
        it(8, "Copy path"),
        it(9, "Properties")
            .icon(glyph::INFO)
            .hint("Alt+Enter")
            .sep(),
        it(10, "Create dump file\u{2026}"),
        it(11, "Search online \u{2014} network is off").enabled(false),
    ];
    let mut m = M::new(items, row.x + 330.0, row.cy(), false).access_keys(true);
    m.hover = Some(3);
    m.draw(p, pal, width, h);
    let r = m.row_rect(3);
    p.icon(glyph::RIGHT, 10.0, r.right() - 12.0, r.cy(), pal.muted);
    let mut sub = M::new(
        vec![
            it(20, "Realtime")
                .icon(SHIELD)
                .hint("administrator")
                .enabled(true),
            it(21, "High"),
            it(22, "Above normal"),
            it(23, "Normal").hint("now"),
            it(24, "Below normal").sep(),
            it(25, "Low"),
        ],
        m.rect().right() + 2.0,
        r.y - 4.0,
        false,
    );
    sub.hover = Some(4);
    sub.draw(p, pal, width, h);
}

// ---- the questions ----------------------------------------------------------------------------

pub fn endtree(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let mut sc = Scene::default();
    sc.sel = 12480;
    w::window(p, pal, width, h, &sc);
    draw_backdrop(p, pal, width, h);
    let c = dialog_card(560.0, 400.0, width, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    p.icon(w::KILL, 20.0, l + 10.0, c.y + 40.0, red(pal));
    p.text(
        Face::Semibold,
        16.0,
        l + 36.0,
        c.y + 38.0,
        "End cargo.exe and what it started?",
        pal.text,
        r - l - 40.0,
    );
    p.text(
        Face::Ui,
        12.5,
        l + 36.0,
        c.y + 60.0,
        "3 programs close at once. Anything they have not saved is lost.",
        pal.muted,
        r - l - 40.0,
    );
    let list = Rect::new(l, c.y + 86.0, r - l, 112.0);
    p.frame(list, 6.0, pal.field, pal.popup_line);
    for (k, (n, pid, ind, extra)) in [
        ("cargo.exe", 12480, 0.0, "cargo build --release"),
        ("rustc.exe", 12712, 22.0, "rustc (mdedit)  1.4 GB"),
        ("link.exe", 13004, 22.0, "linker"),
    ]
    .iter()
    .enumerate()
    {
        let cy = list.y + 20.0 + k as f32 * 32.0;
        w::app_icon(p, list.x + 14.0 + ind, cy, n);
        p.text(Face::Ui, 13.0, list.x + 38.0 + ind, cy, n, pal.text, 200.0);
        p.text(
            Face::Ui,
            12.0,
            list.x + 150.0 + ind,
            cy,
            &format!("PID {pid}"),
            pal.muted,
            80.0,
        );
        p.text_right(Face::Ui, 12.0, list.right() - 14.0, cy, extra, pal.muted);
    }
    let note = Rect::new(l, list.bottom() + 14.0, r - l, 76.0);
    p.frame(
        note,
        6.0,
        mix(pal.popup, amber(pal), 0.10),
        mix(pal.popup_line, amber(pal), 0.5),
    );
    p.icon(glyph::WARN, 14.0, note.x + 16.0, note.y + 20.0, amber(pal));
    ui::para(
        p,
        Face::Ui,
        12.0,
        note.x + 34.0,
        note.y + 8.0,
        note.w - 46.0,
        "This stops the process the way a crash would. It cannot run its clean-up, and a file it was writing may be left half written. The terminal that started it, pwsh.exe, stays open.",
        pal.text,
    );
    footer(p, pal, f, l, r, &[], &["Cancel", "End process tree"]);
}

pub fn affinity(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let sc = Scene::default();
    w::window(p, pal, width, h, &sc);
    draw_backdrop(p, pal, width, h);
    let c = dialog_card(520.0, 450.0, width, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    p.text(
        Face::Semibold,
        16.0,
        l,
        c.y + 38.0,
        "Which processors may rustc.exe use?",
        pal.text,
        r - l,
    );
    p.text(
        Face::Ui,
        12.5,
        l,
        c.y + 60.0,
        "PID 12712 \u{b7} now on all 16 logical processors",
        pal.muted,
        r - l,
    );
    ui::check(p, pal, l, c.y + 94.0, false, "All processors", 240.0);
    let mut y = c.y + 124.0;
    for row in 0..4 {
        for col in 0..4 {
            let i = row * 4 + col;
            let x = l + col as f32 * 118.0;
            let on = i < 8;
            ui::check(
                p,
                pal,
                x,
                y + row as f32 * 32.0,
                on,
                &format!("CPU {i}"),
                100.0,
            );
        }
    }
    y += 4.0 * 32.0 + 8.0;
    ui::para(
        p,
        Face::Ui,
        12.0,
        l,
        y,
        r - l,
        "Here: the first 8 only, so the other 8 stay free for the editor while the build runs. Windows forgets this when rustc.exe ends, and programs it starts inherit it. Nothing is saved.",
        pal.muted,
    );
    p.text(Face::Ui, 12.0, l, y + 80.0, "Presets", pal.faint, 80.0);
    let mut x = l + 56.0;
    for t in ["First half", "Second half", "Even", "One per core"] {
        let wd = p.measure(Face::Ui, 12.0, t) + 20.0;
        let b = Rect::new(x, y + 68.0, wd, 24.0);
        p.frame(b, 12.0, pal.popup, pal.popup_line);
        p.text_center(Face::Ui, 12.0, b, t, pal.muted);
        x += wd + 6.0;
    }
    footer(p, pal, f, l, r, &[], &["Cancel", "OK"]);
}

pub fn elevate(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let mut sc = Scene::default();
    sc.sel = 800;
    sc.view = View::Tree;
    sc.cols = w::Cols::Tree;
    w::window(p, pal, width, h, &sc);
    draw_backdrop(p, pal, width, h);
    let c = dialog_card(580.0, 440.0, width, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    p.icon(SHIELD, 24.0, l + 12.0, c.y + 42.0, amber(pal));
    p.text(
        Face::Semibold,
        16.0,
        l + 42.0,
        c.y + 36.0,
        "Setting the priority of lsass.exe",
        pal.text,
        r - l - 44.0,
    );
    p.text(
        Face::Ui,
        12.5,
        l + 42.0,
        c.y + 58.0,
        "needs administrator rights. This window does not have them.",
        pal.muted,
        r - l - 44.0,
    );
    let mut y = c.y + 92.0;
    for (t, sub, sel) in [
        (
            "Do this once",
            "Windows asks (UAC) and a small helper does only this: set priority of PID 800. Then it ends. This window keeps running as you.",
            true,
        ),
        (
            "Restart as administrator",
            "Closes this window and opens it again with full rights, on the same page. Everything you do in it is then done as administrator until you close it.",
            false,
        ),
    ] {
        let b = Rect::new(l, y, r - l, 92.0);
        p.frame(
            b,
            8.0,
            if sel {
                mix(pal.popup, pal.accent, 0.10)
            } else {
                pal.popup
            },
            if sel { pal.accent } else { pal.popup_line },
        );
        ui::radio(p, pal, b.x + 14.0, b.y + 24.0, sel);
        p.text(
            Face::Semibold,
            13.5,
            b.x + 42.0,
            b.y + 24.0,
            t,
            pal.text,
            b.w - 56.0,
        );
        ui::para(
            p,
            Face::Ui,
            12.0,
            b.x + 42.0,
            b.y + 38.0,
            b.w - 58.0,
            sub,
            pal.muted,
        );
        y += 104.0;
    }
    ui::para(
        p,
        Face::Ui,
        12.0,
        l,
        y + 2.0,
        r - l,
        "lsass.exe runs as a protected process on many PCs. Even an administrator may then get \u{201c}Access denied\u{201d}; if so, nothing changes and the helper says so.",
        pal.faint,
    );
    footer(p, pal, f, l, r, &[], &["Cancel", "Continue"]);
}

// ---- settings and cost ------------------------------------------------------------------------

pub fn settings(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let sc = Scene::default();
    w::window(p, pal, width, h, &sc);
    draw_backdrop(p, pal, width, h);
    let c = dialog_card(900.0, 680.0, width, h);
    let f = draw_dialog_card(p, pal, c);
    let side = 190.0;
    p.fill(
        Rect::new(c.x + 1.0, c.y + 1.0, side, c.h - DIALOG_FOOTER - 1.0),
        mix(pal.popup, pal.pane, 0.6),
    );
    p.text(
        Face::Semibold,
        16.0,
        c.x + 20.0,
        c.y + 30.0,
        "Options",
        pal.text,
        150.0,
    );
    for (i, t) in [
        "Sampling",
        "Processes",
        "Tray and compact",
        "Startup and services",
        "Privacy",
        "Advanced",
    ]
    .iter()
    .enumerate()
    {
        let b = Rect::new(c.x + 10.0, c.y + 56.0 + i as f32 * 32.0, side - 20.0, 28.0);
        if i == 0 {
            p.rbox(b, 5.0, pal.on);
            p.fill(Rect::new(b.x, b.y + 7.0, 3.0, 14.0), pal.accent);
        }
        p.text(Face::Ui, 13.0, b.x + 14.0, b.cy(), t, pal.text, 140.0);
    }
    let l = c.x + side + 28.0;
    let r = c.right() - 28.0;
    let mut y = c.y + 28.0;
    p.text(
        Face::Semibold,
        15.0,
        l,
        y + 4.0,
        "Sampling",
        pal.text,
        200.0,
    );
    y += 36.0;
    p.text(Face::Ui, 13.0, l, y + 10.0, "How often", pal.text, 200.0);
    mdshared_ui::segments::draw_segments(
        p,
        pal,
        Rect::new(l + 250.0, y - 2.0, 320.0, 28.0),
        &["0.5 s", "1 s", "2 s", "5 s"],
        Some(1),
        None,
    );
    y += 28.0;
    p.text(Face::Ui, 11.5, l, y + 8.0, "1 s costs about 1 ms of CPU a second. 0.5 s doubles it. Graphs keep the last hour at any rate.", pal.muted, r - l);
    y += 38.0;
    p.text(
        Face::Ui,
        13.0,
        l,
        y + 10.0,
        "When the window is hidden or minimised",
        pal.text,
        r - l,
    );
    y += 30.0;
    for (i, t) in [
        "Stop sampling. The graphs have a gap for the time away.",
        "Sample every 5 s, for the tray icon only.",
        "Keep sampling at the rate above.",
    ]
    .iter()
    .enumerate()
    {
        ui::radio(p, pal, l + 2.0, y + 8.0, i == 0);
        p.text(Face::Ui, 13.0, l + 28.0, y + 8.0, t, pal.text, r - l - 30.0);
        y += 26.0;
    }
    y += 10.0;
    p.fill(Rect::new(l, y, r - l, p.px().max(1.0)), pal.popup_line);
    y += 14.0;
    p.text(
        Face::Semibold,
        13.0,
        l,
        y + 8.0,
        "Costs it keeps itself to",
        pal.text,
        260.0,
    );
    y += 30.0;
    for (a, b) in [
        ("Window hidden, no tray icon", "0 wake-ups, 0% CPU"),
        ("Window shown, 1 s", "under 0.5% of one core"),
        ("Private memory", "under 30 MiB, history included"),
        ("First window", "under 100 ms, as the rest of the family"),
    ] {
        p.text(Face::Ui, 12.5, l + 8.0, y + 8.0, a, pal.muted, 240.0);
        p.text(Face::Ui, 12.5, l + 260.0, y + 8.0, b, pal.text, 300.0);
        y += 22.0;
    }
    y += 12.0;
    ui::check(
        p,
        pal,
        l,
        y + 10.0,
        true,
        "Show the cost of this app in the status bar",
        r - l,
    );
    y += 34.0;
    ui::check(
        p,
        pal,
        l,
        y + 10.0,
        true,
        "Run in Windows\u{2019}s \u{201c}efficiency mode\u{201d} when the window is not in front",
        r - l,
    );
    y += 34.0;
    ui::check(
        p,
        pal,
        l,
        y + 10.0,
        false,
        "Keep the window on top of others",
        r - l,
    );
    footer(p, pal, f, l, r, &["Restore defaults"], &["Close"]);
}

pub fn privacy(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let sc = Scene::default();
    w::window(p, pal, width, h, &sc);
    draw_backdrop(p, pal, width, h);
    let c = dialog_card(900.0, 640.0, width, h);
    let f = draw_dialog_card(p, pal, c);
    let side = 190.0;
    p.fill(
        Rect::new(c.x + 1.0, c.y + 1.0, side, c.h - DIALOG_FOOTER - 1.0),
        mix(pal.popup, pal.pane, 0.6),
    );
    p.text(
        Face::Semibold,
        16.0,
        c.x + 20.0,
        c.y + 30.0,
        "Options",
        pal.text,
        150.0,
    );
    for (i, t) in [
        "Sampling",
        "Processes",
        "Tray and compact",
        "Startup and services",
        "Privacy",
        "Advanced",
    ]
    .iter()
    .enumerate()
    {
        let b = Rect::new(c.x + 10.0, c.y + 56.0 + i as f32 * 32.0, side - 20.0, 28.0);
        if i == 4 {
            p.rbox(b, 5.0, pal.on);
            p.fill(Rect::new(b.x, b.y + 7.0, 3.0, 14.0), pal.accent);
        }
        p.text(Face::Ui, 13.0, b.x + 14.0, b.cy(), t, pal.text, 140.0);
    }
    let l = c.x + side + 28.0;
    let r = c.right() - 28.0;
    let mut y = c.y + 28.0;
    p.text(
        Face::Semibold,
        15.0,
        l,
        y + 4.0,
        "Privacy and the network",
        pal.text,
        300.0,
    );
    y += 36.0;
    y = ui::para(
        p,
        Face::Ui,
        13.0,
        l,
        y,
        r - l,
        "This program makes no network connection of its own, except to look for its own updates. It never sends a process name, a path or a command line anywhere.",
        pal.muted,
    );
    y += 10.0;
    ui::check(
        p,
        pal,
        l,
        y + 10.0,
        false,
        "\u{201c}Search online\u{201d} in the process menu",
        r - l,
    );
    y = ui::para(
        p,
        Face::Ui,
        12.0,
        l + 26.0,
        y + 26.0,
        r - l - 30.0,
        "Off. When on, the menu item opens your browser on a search page for the file name, and only that. The address is built from the name; this program connects to nothing.",
        pal.muted,
    );
    y += 16.0;
    ui::check(
        p,
        pal,
        l,
        y + 10.0,
        false,
        "Check signatures against Microsoft\u{2019}s online lists",
        r - l,
    );
    y = ui::para(
        p,
        Face::Ui,
        12.0,
        l + 26.0,
        y + 26.0,
        r - l - 30.0,
        "Off. Signatures are checked against the copy of the certificates already on this PC, with no revocation lookup. A revoked certificate is therefore not noticed.",
        pal.muted,
    );
    y += 16.0;
    ui::check(
        p,
        pal,
        l,
        y + 10.0,
        true,
        "Keep what I search for and which columns I show, between runs",
        r - l,
    );
    y += 40.0;
    p.fill(Rect::new(l, y, r - l, p.px().max(1.0)), pal.popup_line);
    y += 16.0;
    p.text(
        Face::Semibold,
        13.0,
        l,
        y + 8.0,
        "What needs administrator rights",
        pal.text,
        300.0,
    );
    y += 28.0;
    for (a, b) in [
        (
            "Never",
            "the list, CPU, memory, I/O, threads, handles, GPU of every process; your own processes\u{2019} modules, handles, environment",
        ),
        (
            "Once, on request",
            "end or change another user\u{2019}s process; stop a service; HKLM startup entries; traffic per program",
        ),
        (
            "Never offered",
            "close a handle in another program; load a driver; read a protected process",
        ),
    ] {
        p.text(Face::Semibold, 12.5, l + 8.0, y + 8.0, a, pal.text, 130.0);
        let y2 = ui::para(p, Face::Ui, 12.0, l + 150.0, y, r - l - 160.0, b, pal.muted);
        y = y2 + 8.0;
    }
    footer(p, pal, f, l, r, &[], &["Close"]);
}

pub fn cost(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    let sc = Scene::default();
    w::window(p, pal, width, h, &sc);
    let pw = 460.0;
    let ph = 330.0;
    let r = Rect::new(width - pw - 14.0, h - w::STATUS_H - ph - 8.0, pw, ph);
    p.shadow(r, 10.0, pal.dark);
    p.frame(r, 10.0, pal.popup, pal.popup_line);
    let l = r.x + 20.0;
    let mut y = r.y + 22.0;
    p.text(
        Face::Semibold,
        14.0,
        l,
        y,
        "What this program costs",
        pal.text,
        300.0,
    );
    p.text_right(
        Face::Ui,
        12.0,
        r.right() - 20.0,
        y,
        "measured by itself, every sample",
        pal.faint,
    );
    y += 22.0;
    let rows: [(&str, &str, &str, f32, &str); 4] = [
        (
            "CPU",
            "0.0% now",
            "0.2% over 10 min",
            0.02,
            "budget: 0% when hidden",
        ),
        (
            "Memory",
            "17.8 MiB private",
            "12.4 MiB of it history",
            0.59,
            "budget: 30 MiB",
        ),
        (
            "A sample",
            "1.1 ms",
            "list 0.7 \u{b7} cores 0.1 \u{b7} gpu 0.3",
            0.11,
            "budget: 10 ms",
        ),
        (
            "Wake-ups",
            "1 a second",
            "0 when hidden",
            0.10,
            "budget: none when hidden",
        ),
    ];
    for (a, b, c, f, n) in rows {
        y += 10.0;
        p.text(Face::Semibold, 12.5, l, y + 8.0, a, pal.text, 90.0);
        p.text(Face::Ui, 12.5, l + 90.0, y + 8.0, b, pal.text, 160.0);
        p.text(Face::Ui, 12.0, l + 250.0, y + 8.0, c, pal.muted, 190.0);
        ui::progress(
            p,
            pal,
            Rect::new(l + 90.0, y + 22.0, 220.0, 5.0),
            f,
            green(pal),
            pal.popup,
        );
        p.text(Face::Ui, 11.5, l + 320.0, y + 25.0, n, pal.faint, 130.0);
        y += 40.0;
    }
    y += 6.0;
    p.fill(Rect::new(l, y, r.w - 40.0, p.px().max(1.0)), pal.popup_line);
    y += 12.0;
    ui::para(
        p,
        Face::Ui,
        12.0,
        l,
        y,
        r.w - 40.0,
        "Visible: samples every 1 s. Hidden or minimised: stops, and the graphs show a gap. Tray icon only: every 5 s, about 0.01% of a core. A sampler that does not stop is a process worth ending.",
        pal.muted,
    );
    ui::sbtn(p, pal, l, r.bottom() - 26.0, "Options\u{2026}", false, true);
}

// ---- tray, compact ----------------------------------------------------------------------------

fn tray_icon(
    p: &mut Painter,
    pal: &Palette,
    cx: f32,
    cy: f32,
    size: f32,
    cpu: f32,
    mem: f32,
    warn: bool,
) {
    let r = Rect::new(cx - size / 2.0, cy - size / 2.0, size, size);
    p.rbox(r, size * 0.18, mix(pal.title, pal.text, 0.10));
    let bw = size * 0.30;
    let bh = size * 0.74;
    let base = r.bottom() - size * 0.13;
    let c1 = if warn { red(pal) } else { blue(pal) };
    let c2: Rgb = [0x8b, 0x5c, 0xc8];
    p.fill(
        Rect::new(r.x + size * 0.17, base - bh * cpu, bw, bh * cpu),
        c1,
    );
    p.fill(
        Rect::new(r.x + size * 0.53, base - bh * mem, bw, bh * mem),
        c2,
    );
}

pub fn tray(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    p.fill(
        Rect::new(0.0, 0.0, width, h),
        mix(pal.page, pal.accent, 0.10),
    );
    // The desktop's task bar.
    let tb = Rect::new(0.0, h - 48.0, width, 48.0);
    p.fill(tb, mix(pal.title, pal.page, 0.3));
    p.fill(Rect::new(0.0, tb.y, width, p.px().max(1.0)), pal.line);
    let mut x = width - 200.0;
    p.icon(glyph::SYSTEM, 14.0, x, tb.cy(), pal.muted);
    x += 26.0;
    tray_icon(p, pal, x, tb.cy(), 18.0, 0.11, 0.54, false);
    x += 28.0;
    p.icon(glyph::GLOBE, 14.0, x, tb.cy(), pal.muted);
    p.text_right(
        Face::Ui,
        11.5,
        width - 14.0,
        tb.cy() - 8.0,
        "14:07",
        pal.text,
    );
    p.text_right(
        Face::Ui,
        11.0,
        width - 14.0,
        tb.cy() + 8.0,
        "22.11.2026",
        pal.muted,
    );
    // The flyout.
    let fw = 360.0;
    let fh = 478.0;
    let f = Rect::new(width - fw - 14.0, tb.y - fh - 10.0, fw, fh);
    p.shadow(f, 12.0, pal.dark);
    p.frame(f, 12.0, pal.popup, pal.popup_line);
    let l = f.x + 18.0;
    let mut y = f.y + 24.0;
    w::tile(p, Rect::new(l, y - 12.0, 22.0, 22.0));
    p.text(Face::Semibold, 14.0, l + 32.0, y, "mdtask", pal.text, 120.0);
    p.text_right(
        Face::Ui,
        12.5,
        f.right() - 18.0,
        y,
        "Open  \u{b7}  Compact",
        blue(pal),
    );
    y += 18.0;
    let rows: [(&str, &str, Rgb, f32, u32); 5] = [
        ("CPU", "11%", [0x11, 0x7d, 0xbb], 0.12, 3),
        ("Memory", "17.3 of 32 GB", [0x8b, 0x12, 0xae], 0.54, 8),
        ("Disk", "1.2 MB/s", [0x4d, 0xa1, 0x2f], 0.04, 12),
        (
            "Wi-Fi",
            "\u{2193}1.1  \u{2191}0.1 Mbps",
            [0xa7, 0x4f, 0x01],
            0.10,
            19,
        ),
        ("GPU", "14%", [0x11, 0x7d, 0xbb], 0.16, 24),
    ];
    for (a, b, c, base, seed) in rows {
        let r = Rect::new(l, y + 4.0, f.w - 36.0, 38.0);
        p.text(Face::Ui, 12.5, r.x, r.y + 12.0, a, pal.muted, 70.0);
        p.text(Face::Semibold, 12.5, r.x, r.y + 30.0, b, pal.text, 120.0);
        graph(
            p,
            pal,
            Rect::new(r.x + 130.0, r.y, r.w - 130.0, r.h),
            &d::series(40, base, 0.2, seed),
            c,
            0.25,
            0,
            true,
        );
        y += 48.0;
    }
    y += 6.0;
    p.fill(Rect::new(l, y, f.w - 36.0, p.px().max(1.0)), pal.popup_line);
    y += 14.0;
    p.text(
        Face::Semibold,
        11.5,
        l,
        y + 6.0,
        "BUSIEST NOW",
        pal.faint,
        200.0,
    );
    y += 20.0;
    for (n, v) in [
        ("rustc.exe", "38.6%"),
        ("rust-analyzer.exe", "7.9%"),
        ("firefox.exe", "6.2%"),
    ] {
        w::app_icon(p, l, y + 11.0, n);
        p.text(Face::Ui, 12.5, l + 24.0, y + 11.0, n, pal.text, 200.0);
        p.text_right(Face::Ui, 12.5, f.right() - 18.0, y + 11.0, v, pal.text);
        y += 26.0;
    }
    y += 4.0;
    p.fill(Rect::new(l, y, f.w - 36.0, p.px().max(1.0)), pal.popup_line);
    p.text(
        Face::Ui,
        12.0,
        l,
        y + 20.0,
        "Pause sampling",
        pal.muted,
        120.0,
    );
    p.text(
        Face::Ui,
        12.0,
        l + 130.0,
        y + 20.0,
        "Options",
        pal.muted,
        80.0,
    );
    p.text(Face::Ui, 12.0, l + 210.0, y + 20.0, "Quit", pal.muted, 80.0);
    // The icon, enlarged, in three states: the design is a pair of bars that fit 16 px.
    let z = Rect::new(24.0, 24.0, 286.0, 188.0);
    p.frame(z, 10.0, pal.popup, pal.popup_line);
    p.text(
        Face::Semibold,
        12.5,
        z.x + 16.0,
        z.y + 20.0,
        "The tray icon, 16 px, enlarged",
        pal.text,
        260.0,
    );
    for (i, (cpu, mem, warn, lab)) in [
        (0.11, 0.54, false, "Quiet"),
        (0.92, 0.61, true, "Busy"),
        (0.20, 0.96, true, "No memory"),
    ]
    .iter()
    .enumerate()
    {
        let cx = z.x + 52.0 + i as f32 * 92.0;
        tray_icon(p, pal, cx, z.y + 80.0, 64.0, *cpu, *mem, *warn);
        p.text_center(
            Face::Ui,
            11.5,
            Rect::new(cx - 44.0, z.y + 122.0, 88.0, 16.0),
            lab,
            pal.muted,
        );
    }
    ui::para(
        p,
        Face::Ui,
        11.5,
        z.x + 16.0,
        z.y + 142.0,
        z.w - 32.0,
        "Left bar CPU, right bar memory. The tooltip repeats the numbers.",
        pal.faint,
    );
}

pub fn compact(p: &mut Painter, pal: &Palette, width: f32, h: f32) {
    p.fill(Rect::new(0.0, 0.0, width, h), pal.page);
    // A small title: no page tabs.
    p.fill(Rect::new(0.0, 0.0, width, 36.0), pal.title);
    w::tile(p, Rect::new(12.0, 7.0, 22.0, 22.0));
    p.text(Face::Semibold, 13.0, 44.0, 18.0, "mdtask", pal.text, 120.0);
    p.icon(w::PIN_ON, 14.0, width - 96.0, 18.0, pal.accent);
    p.icon(glyph::FULLSCREEN, 14.0, width - 64.0, 18.0, pal.muted);
    p.icon(glyph::CLOSE, 13.0, width - 26.0, 18.0, pal.muted);
    let mut y = 46.0;
    let rows: [(&str, &str, Rgb, f32, u32, f32); 5] = [
        ("CPU", "11%", [0x11, 0x7d, 0xbb], 0.12, 3, 62.0),
        ("Memory", "17.3 / 32 GB", [0x8b, 0x12, 0xae], 0.54, 8, 44.0),
        ("Disk", "1.2 MB/s", [0x4d, 0xa1, 0x2f], 0.04, 12, 44.0),
        (
            "Wi-Fi",
            "\u{2193}1.1 \u{2191}0.1 Mbps",
            [0xa7, 0x4f, 0x01],
            0.10,
            19,
            44.0,
        ),
        ("GPU", "14%", [0x11, 0x7d, 0xbb], 0.16, 24, 44.0),
    ];
    for (a, b, c, base, seed, rh) in rows {
        let r = Rect::new(12.0, y, width - 24.0, rh);
        p.text(Face::Ui, 12.0, r.x, r.y + 9.0, a, pal.muted, 60.0);
        p.text_right(Face::Semibold, 12.5, r.right(), r.y + 9.0, b, pal.text);
        graph(
            p,
            pal,
            Rect::new(r.x, r.y + 20.0, r.w, rh - 22.0),
            &d::series(50, base, 0.2, seed),
            c,
            0.25,
            0,
            true,
        );
        y += rh + 12.0;
    }
    p.fill(Rect::new(12.0, y, width - 24.0, p.px().max(1.0)), pal.line);
    y += 12.0;
    p.text(
        Face::Semibold,
        11.5,
        12.0,
        y + 6.0,
        "BUSIEST NOW",
        pal.faint,
        200.0,
    );
    y += 20.0;
    for (n, c, m) in [
        ("rustc.exe", "38.6%", "1.4 GB"),
        ("rust-analyzer.exe", "7.9%", "912 MB"),
        ("firefox.exe", "6.2%", "420 MB"),
        ("Code.exe", "3.0%", "330 MB"),
    ] {
        w::app_icon(p, 12.0, y + 11.0, n);
        p.text(Face::Ui, 12.5, 36.0, y + 11.0, n, pal.text, 150.0);
        p.text_right(Face::Ui, 12.5, width - 90.0, y + 11.0, c, pal.text);
        p.text_right(Face::Ui, 12.5, width - 12.0, y + 11.0, m, pal.muted);
        y += 26.0;
    }
    let _ = (Page::Processes, DTab::Overview, pg::Res::Cpu);
}
