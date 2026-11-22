//! The dialogs and menus over the Hub window: an app that is open, roll back, release notes, the
//! row menu, first run, the tray, and Settings.

use super::data::{self, APPS, St, amber, blue, green};
use super::engine;
use super::pages;
use super::ui;
use super::win::{self, Bar, Scene};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::menu::{Item, Menu};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_card,
};
use mdshared_ui::theme::Palette;

/// Footer buttons: `left` at the left edge, `right` from the right edge, the last one primary.
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

// ---- an app that is open ---------------------------------------------------------------------

pub fn running(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.running[data::MDEDIT] = true;
    win::window(p, pal, w, h, &sc);
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(620.0, 540.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    let tile = Rect::new(l, c.y + 24.0, 40.0, 40.0);
    ui::tile(p, tile, &APPS[data::MDEDIT], false);
    p.text(
        Face::Semibold,
        17.0,
        l + 54.0,
        c.y + 34.0,
        "mdedit is open",
        pal.text,
        400.0,
    );
    p.text(
        Face::Ui,
        12.5,
        l + 54.0,
        c.y + 56.0,
        "1.2.0 \u{2192} 1.2.1 \u{b7} 2 tabs have text that is not saved",
        pal.muted,
        r - l - 54.0,
    );
    let mut y = ui::para(
        p,
        Face::Ui,
        13.0,
        l,
        c.y + 84.0,
        r - l,
        "A program cannot be replaced while it runs. Hub never ends a program: it asks the window to close, exactly as the X button does, so mdedit's own Save questions come first.",
        pal.text,
    );
    y += 10.0;
    let opts: [(&str, &str); 3] = [
        (
            "Close mdedit, update it, and open it again",
            "mdedit keeps its session: the two tabs come back as they were, unsaved text included.",
        ),
        (
            "Update when I close mdedit myself",
            "The files are downloaded and checked now. Hub installs the moment mdedit quits, and does not open it.",
        ),
        (
            "Not now",
            "mdedit stays at 1.2.0 and Hub asks again at the next check.",
        ),
    ];
    for (i, (t, sub)) in opts.iter().enumerate() {
        let hh = 72.0;
        let box_ = Rect::new(l, y, r - l, hh);
        if i == 0 {
            p.frame(box_, 8.0, mix(pal.popup, pal.accent, 0.10), pal.accent);
        } else {
            p.frame(box_, 8.0, mix(pal.popup, pal.text, 0.02), pal.popup_line);
        }
        ui::radio(p, pal, box_.x + 14.0, box_.y + 22.0, i == 0);
        p.text(
            Face::Semibold,
            13.0,
            box_.x + 42.0,
            box_.y + 21.0,
            t,
            pal.text,
            box_.w - 56.0,
        );
        ui::para(
            p,
            Face::Ui,
            12.0,
            box_.x + 42.0,
            box_.y + 33.0,
            box_.w - 60.0,
            sub,
            pal.muted,
        );
        y += hh + 8.0;
    }
    p.icon(glyph::WARN, 13.0, l + 7.0, y + 12.0, amber(pal));
    ui::para(
        p,
        Face::Ui,
        12.0,
        l + 22.0,
        y + 3.0,
        r - l - 22.0,
        "If mdedit shows a Save question, answer it in mdedit. Hub waits two minutes for the window to close and then leaves everything as it was.",
        pal.muted,
    );
    footer(p, pal, f, l, r, &["Cancel"], &["Close mdedit and update"]);
}

// ---- roll back -------------------------------------------------------------------------------

pub fn rollback(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.st[data::MDEDIT] = St::UpToDate;
    sc.inst[data::MDEDIT] = Some("1.2.1");
    sc.bar = Bar::Updates(2);
    win::window(p, pal, w, h, &sc);
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(640.0, 450.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    let tile = Rect::new(l, c.y + 24.0, 40.0, 40.0);
    ui::tile(p, tile, &APPS[data::MDEDIT], false);
    p.text(
        Face::Semibold,
        17.0,
        l + 54.0,
        c.y + 34.0,
        "Go back to an earlier mdedit",
        pal.text,
        r - l - 54.0,
    );
    p.text(
        Face::Ui,
        12.5,
        l + 54.0,
        c.y + 56.0,
        "Now: 1.2.1, installed 22 Nov. Settings and session stay as they are.",
        pal.muted,
        r - l - 54.0,
    );
    let rows: [(&str, &str, &str, &str); 4] = [
        (
            "1.2.0",
            "kept copy",
            "Replaced by 1.2.1. It is still on disk, so no download.",
            "8.0 MB",
        ),
        (
            "1.1.2",
            "download",
            "20 Nov. From that release, checked against its own SHA-256.",
            "35.1 MB",
        ),
        (
            "1.1.1",
            "download",
            "19 Nov. From that release, checked against its own SHA-256.",
            "35.0 MB",
        ),
        (
            "1.0.6",
            "download",
            "18 Nov. The last one before the AI engine.",
            "34.6 MB",
        ),
    ];
    let top = c.y + 84.0;
    let box_ = Rect::new(l, top, r - l, rows.len() as f32 * 54.0);
    p.frame(box_, 8.0, mix(pal.popup, pal.text, 0.02), pal.popup_line);
    for (i, (v, tag, sub, size)) in rows.iter().enumerate() {
        let ry = box_.y + i as f32 * 54.0;
        if i > 0 {
            p.fill_a(
                Rect::new(box_.x + 10.0, ry, box_.w - 20.0, 1.0),
                pal.popup_line,
                0.8,
            );
        }
        if i == 0 {
            p.rbox(
                Rect::new(box_.x + 3.0, ry + 3.0, box_.w - 6.0, 48.0),
                6.0,
                mix(pal.popup, pal.accent, 0.12),
            );
        }
        ui::radio(p, pal, box_.x + 14.0, ry + 27.0, i == 0);
        let vw = p.text(
            Face::Semibold,
            13.5,
            box_.x + 42.0,
            ry + 18.0,
            v,
            pal.text,
            100.0,
        );
        let col: Rgb = if i == 0 { green(pal) } else { pal.muted };
        ui::chip_on(p, pal.popup, box_.x + 42.0 + vw + 10.0, ry + 18.0, tag, col);
        p.text_right(
            Face::Ui,
            12.0,
            box_.right() - 14.0,
            ry + 18.0,
            size,
            pal.muted,
        );
        p.text(
            Face::Ui,
            12.0,
            box_.x + 42.0,
            ry + 37.0,
            sub,
            pal.muted,
            box_.w - 60.0,
        );
    }
    let mut y = box_.bottom() + 20.0;
    ui::check(
        p,
        pal,
        l,
        y,
        true,
        "Do not offer 1.2.1 again; offer the next version after it",
        r - l,
    );
    y += 14.0;
    ui::para(
        p,
        Face::Ui,
        12.0,
        l + 26.0,
        y,
        r - l - 26.0,
        "Hub keeps the previous version of each app. Going back further is a normal install of that release: download, SHA-256 check, replace.",
        pal.muted,
    );
    footer(p, pal, f, l, r, &["Cancel"], &["Go back to 1.2.0"]);
}

// ---- release notes ---------------------------------------------------------------------------

pub fn notes(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    win::window(p, pal, w, h, &Scene::default());
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(940.0, 640.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    let tile = Rect::new(l, c.y + 22.0, 32.0, 32.0);
    ui::tile(p, tile, &APPS[data::MDEDIT], false);
    p.text(
        Face::Semibold,
        17.0,
        l + 44.0,
        c.y + 30.0,
        "mdedit: what's new",
        pal.text,
        400.0,
    );
    p.text(
        Face::Ui,
        12.5,
        l + 44.0,
        c.y + 49.0,
        "You have 1.2.0. 1.2.1 is the only release newer than that.",
        pal.muted,
        400.0,
    );
    let top = c.y + 74.0;
    let px = p.px().max(1.0);
    // The versions.
    let lw = 210.0;
    let list: [(&str, &str, &str); 5] = [
        ("1.2.1", "21 Nov", "newer"),
        ("1.2.0", "21 Nov", "yours"),
        ("1.1.2", "20 Nov", ""),
        ("1.1.1", "19 Nov", ""),
        ("1.0.6", "18 Nov", ""),
    ];
    for (i, (v, d, tag)) in list.iter().enumerate() {
        let it = Rect::new(l, top + i as f32 * 40.0, lw, 36.0);
        if i == 0 {
            p.rbox(it, 6.0, pal.on);
            p.rbox(
                Rect::new(it.x + 4.0, it.cy() - 8.0, 3.0, 16.0),
                1.5,
                pal.accent,
            );
        }
        p.text(
            Face::Semibold,
            13.0,
            it.x + 18.0,
            it.cy(),
            v,
            pal.text,
            80.0,
        );
        p.text(Face::Ui, 12.0, it.x + 76.0, it.cy(), d, pal.muted, 60.0);
        if !tag.is_empty() {
            let col = if *tag == "newer" {
                blue(pal)
            } else {
                green(pal)
            };
            let tw = p.measure(Face::Semibold, 11.0, tag) + 14.0;
            ui::chip_on(
                p,
                if i == 0 { pal.on } else { pal.popup },
                it.right() - tw - 8.0,
                it.cy(),
                tag,
                col,
            );
        }
    }
    p.fill(
        Rect::new(
            l + lw + 12.0,
            top,
            px,
            c.bottom() - DIALOG_FOOTER - top - 12.0,
        ),
        pal.popup_line,
    );
    // The notes of 1.2.1, then of 1.2.0.
    let x = l + lw + 32.0;
    let wd = r - x;
    p.text(
        Face::Semibold,
        15.0,
        x,
        top + 10.0,
        "mdedit 1.2.1",
        pal.text,
        wd,
    );
    let mut y = ui::para(
        p,
        Face::Ui,
        13.0,
        x,
        top + 28.0,
        wd,
        "An option to leave out tabs you barely looked at, and a tidier AI menu.",
        pal.muted,
    );
    p.text(Face::Semibold, 11.5, x, y + 16.0, "NEW", pal.faint, 100.0);
    y = ui::para(
        p,
        Face::Ui,
        13.0,
        x,
        y + 26.0,
        wd,
        "Don't reopen tabs barely looked at. A new setting in Settings > Session: at start, tabs you looked at for less than a number of seconds you choose (10 by default) are not opened again. Only the time a tab is in front while mdedit has the focus counts. The tab you left on, pinned tabs and tabs with unsaved text always come back. Off by default.",
        pal.text,
    );
    p.text(Face::Semibold, 11.5, x, y + 14.0, "FIXED", pal.faint, 100.0);
    y = ui::para(
        p,
        Face::Ui,
        13.0,
        x,
        y + 24.0,
        wd,
        "In the AI menu, a long saved prompt's name no longer runs under its Modify button; the menu keeps its width.",
        pal.text,
    );
    y += 14.0;
    p.fill(Rect::new(x, y, wd, px), pal.popup_line);
    p.text(
        Face::Semibold,
        15.0,
        x,
        y + 24.0,
        "mdedit 1.2.0",
        pal.text,
        wd,
    );
    p.text(
        Face::Ui,
        12.0,
        x + 100.0,
        y + 24.0,
        "installed on this PC",
        pal.muted,
        200.0,
    );
    y = ui::para(
        p,
        Face::Ui,
        13.0,
        x,
        y + 40.0,
        wd,
        "Edit Markdown on the rendered page (Ctrl+E), choose a file's language, several custom AI providers with or without a key, and fixes from the second audit.",
        pal.muted,
    );
    p.text(
        Face::Ui,
        12.0,
        x,
        y + 14.0,
        "Notes are the release's own text, fetched when this window opens.",
        pal.faint,
        wd,
    );
    footer(
        p,
        pal,
        f,
        l,
        r,
        &["Open on GitHub"],
        &["Close", "Update to 1.2.1"],
    );
}

// ---- the row menu ----------------------------------------------------------------------------

pub fn row_menu(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let sc = Scene {
        hover: Some(data::MDEDIT),
        ..Scene::default()
    };
    let out = win::window(p, pal, w, h, &sc);
    let row = out.rows[data::MDEDIT];
    let items = vec![
        Item::new(0u32, "Update to 1.2.1").icon(glyph::DOWNLOAD),
        Item::new(1, "Open").icon(glyph::OPEN).sep(),
        Item::new(2, "Release notes\u{2026}").icon(glyph::INFO),
        Item::new(3, "Roll back to 1.1.2\u{2026}").icon(glyph::UNDO),
        Item::new(4, "Skip 1.2.1").icon(glyph::FORWARD).sep(),
        Item::new(5, "Show in folder").icon(glyph::FOLDER_OPEN),
        Item::new(6, "Open on GitHub").icon(glyph::LINK),
        Item::new(7, "Uninstall\u{2026}").icon(glyph::DELETE).sep(),
    ];
    let mut m = Menu::new(items, row.right() - 14.0, row.cy() + 8.0, true);
    let room = Rect::new(0.0, 0.0, w, h);
    m.place_in(p, room);
    let hv = m.row_rect(2);
    m.pointer(hv.cx(), hv.cy());
    m.draw_in(p, pal, room);
}

// ---- first run -------------------------------------------------------------------------------

pub fn first_run(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    p.fill(Rect::new(0.0, 0.0, w, h), pal.page);
    win::title(p, pal, w, &Scene::default());
    let top = mdshared_ui::chrome::TITLE_H;
    p.fill(Rect::new(0.0, top, w, h - top), pal.bar);
    let col_w = (w - 64.0).min(980.0);
    let x0 = ((w - col_w) / 2.0).round();
    let mut y = top + 36.0;
    p.text(
        Face::Semibold,
        22.0,
        x0,
        y + 12.0,
        "Set up your apps",
        pal.text,
        col_w,
    );
    y += 36.0;
    y = ui::para(
        p,
        Face::Ui,
        13.5,
        x0,
        y,
        col_w,
        "Hub found 7 apps with a Windows installer on github.com/Kaan88. Pick the ones you want; each one is downloaded, checked against its SHA-256 and installed for you alone, with no administrator rights.",
        pal.muted,
    );
    y += 16.0;
    let narrow = col_w < 760.0;
    let side_w = if narrow { 0.0 } else { 300.0 };
    let list_w = col_w - side_w - if narrow { 0.0 } else { 24.0 };
    let list = Rect::new(x0, y, list_w, 7.0 * 54.0);
    p.frame(list, 8.0, pal.page, pal.line);
    let picks = [true, true, true, false, false, false, false];
    for i in 0..7 {
        let a = &APPS[i];
        let ry = list.y + i as f32 * 54.0;
        if i > 0 {
            p.fill_a(
                Rect::new(list.x + 10.0, ry, list.w - 20.0, 1.0),
                pal.line,
                0.7,
            );
        }
        let cy = ry + 27.0;
        let found = i == data::NEWSQUEUE;
        if found {
            p.frame(
                Rect::new(list.x + 14.0, cy - 8.0, 16.0, 16.0),
                4.0,
                pal.field,
                pal.faint,
            );
        } else {
            let b = Rect::new(list.x + 14.0, cy - 8.0, 16.0, 16.0);
            if picks[i] {
                p.rbox(b, 4.0, pal.accent);
                p.icon(glyph::CHECK, 12.0, b.cx(), b.cy(), pal.on_accent);
            } else {
                p.frame(b, 4.0, pal.field, pal.muted);
            }
        }
        let tile = Rect::new(list.x + 44.0, cy - 16.0, 32.0, 32.0);
        ui::tile(p, tile, a, false);
        let nx = tile.right() + 12.0;
        let nw = p.text(Face::Semibold, 13.5, nx, cy - 9.0, a.name, pal.text, 200.0);
        if found {
            ui::chip(
                p,
                pal,
                nx + nw + 10.0,
                cy - 9.0,
                "Found on this PC: 1.0.10",
                amber(pal),
            );
        }
        let sub = if found {
            "Installed in Program Files. Hub can take it over, or leave it alone."
        } else {
            a.blurb
        };
        p.text(Face::Ui, 11.5, nx, cy + 9.0, sub, pal.muted, list.w - 200.0);
        p.text_right(
            Face::Ui,
            12.5,
            list.right() - 14.0,
            cy - 8.0,
            a.setup,
            pal.muted,
        );
        p.text_right(
            Face::Ui,
            11.5,
            list.right() - 14.0,
            cy + 9.0,
            a.latest,
            pal.faint,
        );
    }
    if !narrow {
        let sx = list.right() + 24.0;
        let card = Rect::new(sx, y, side_w, 7.0 * 54.0);
        p.frame(card, 8.0, pal.page, pal.line);
        let l = card.x + 18.0;
        let r = card.right() - 18.0;
        let mut cy = card.y + 22.0;
        p.text(Face::Semibold, 13.0, l, cy, "Install into", pal.text, r - l);
        cy += 18.0;
        let f = Rect::new(l, cy, r - l - 76.0, 30.0);
        ui::field(p, pal, f, "%LOCALAPPDATA%\\Programs", true, false);
        ui::sbtn(p, pal, f.right() + 8.0, f.cy(), "Browse", false, true);
        cy += 46.0;
        ui::check(p, pal, l, cy, true, "Start Hub with Windows", r - l);
        cy += 28.0;
        ui::check(p, pal, l, cy, true, "Tell me when updates are found", r - l);
        cy += 28.0;
        ui::check(p, pal, l, cy, false, "Include pre-releases", r - l);
        cy += 26.0;
        p.fill_a(Rect::new(l, cy, r - l, 1.0), pal.line, 0.8);
        cy += 12.0;
        p.text(
            Face::Semibold,
            13.0,
            l,
            cy + 8.0,
            "3 apps selected",
            pal.text,
            r - l,
        );
        p.text(
            Face::Ui,
            12.0,
            l,
            cy + 27.0,
            "28.7 MB to download",
            pal.muted,
            r - l,
        );
        cy += 44.0;
        let b = Rect::new(l, cy, r - l, 34.0);
        p.rbox(b, 6.0, pal.accent);
        p.text_center(Face::Semibold, 13.0, b, "Install 3 apps", pal.on_accent);
        cy += 46.0;
        ui::para(
            p,
            Face::Ui,
            11.5,
            l,
            cy,
            r - l,
            "Hub talks to github.com and nothing else. Nothing leaves this PC about you or your apps.",
            pal.faint,
        );
    }
    let sy = list.bottom() + 18.0;
    p.icon(glyph::INFO, 13.0, x0 + 7.0, sy + 8.0, pal.muted);
    ui::para(
        p,
        Face::Ui,
        12.0,
        x0 + 22.0,
        sy,
        col_w - 22.0,
        "Skip this and Hub still lists every app; you can install or adopt any of them later. It never touches an app it did not install, unless you press Adopt.",
        pal.muted,
    );
}

// ---- the tray --------------------------------------------------------------------------------

pub fn tray(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let top: Rgb = if pal.dark {
        [0x1b, 0x2a, 0x3d]
    } else {
        [0xa9, 0xc7, 0xe6]
    };
    let bot: Rgb = if pal.dark {
        [0x10, 0x14, 0x1c]
    } else {
        [0xe6, 0xd5, 0xcf]
    };
    let bands = 40;
    for i in 0..bands {
        let y = h * i as f32 / bands as f32;
        p.fill(
            Rect::new(0.0, y, w, h / bands as f32 + 1.0),
            mix(top, bot, i as f32 / bands as f32),
        );
    }
    let tb_h = 48.0;
    let tb = Rect::new(0.0, h - tb_h, w, tb_h);
    let tbc: Rgb = if pal.dark {
        [0x1c, 0x1c, 0x1c]
    } else {
        [0xee, 0xee, 0xee]
    };
    p.fill(tb, tbc);
    p.fill(
        Rect::new(0.0, tb.y, w, p.px().max(1.0)),
        if pal.dark {
            [0x30, 0x30, 0x30]
        } else {
            [0xd0, 0xd0, 0xd0]
        },
    );
    for i in 0..5 {
        let b = Rect::new(w / 2.0 - 120.0 + i as f32 * 48.0, tb.y + 6.0, 36.0, 36.0);
        p.rbox(
            b,
            6.0,
            mix(
                tbc,
                if pal.dark { [255, 255, 255] } else { [0, 0, 0] },
                if i == 2 { 0.12 } else { 0.05 },
            ),
        );
        p.rbox(
            Rect::new(b.cx() - 7.0, b.cy() - 7.0, 14.0, 14.0),
            4.0,
            mix(tbc, pal.muted, 0.7),
        );
    }
    let ink = if pal.dark {
        [0xf0, 0xf0, 0xf0]
    } else {
        [0x20, 0x20, 0x20]
    };
    let mut x = w - 150.0;
    let icon = Rect::new(x - 8.0, tb.y + 8.0, 32.0, 32.0);
    p.rbox(icon, 6.0, mix(tbc, ink, 0.14));
    let tile = Rect::new(icon.cx() - 9.0, icon.cy() - 9.0, 18.0, 18.0);
    ui::tile(p, tile, &APPS[data::HUB], false);
    // The badge: how many updates wait.
    let badge = Rect::new(icon.right() - 12.0, icon.y + 1.0, 14.0, 14.0);
    p.rbox(badge, 7.0, [0xd8, 0x3b, 0x2f]);
    p.text_center(Face::Semibold, 10.0, badge, "3", [255, 255, 255]);
    p.icon(glyph::UP, 12.0, x - 40.0, tb.cy(), ink);
    x += 40.0;
    p.icon(glyph::GLOBE, 14.0, x, tb.cy(), ink);
    p.icon(glyph::SYSTEM, 14.0, x + 30.0, tb.cy(), ink);
    p.text_right(Face::Ui, 12.0, w - 14.0, tb.cy() - 8.0, "12:44", ink);
    p.text_right(Face::Ui, 12.0, w - 14.0, tb.cy() + 9.0, "22.11.2026", ink);

    let items = vec![
        Item::new(0u32, "3 updates available")
            .icon(glyph::DOWNLOAD)
            .enabled(false),
        Item::new(1, "Update all").icon(glyph::DOWNLOAD),
        Item::new(2, "Update mdedit to 1.2.1").sep(),
        Item::new(3, "Update Sprawl to 1.0.5"),
        Item::new(4, "Update Inbound to 0.9.0 (open)"),
        Item::new(5, "Open mdedit").icon(glyph::EDIT).sep(),
        Item::new(6, "Open mdview").icon(glyph::VIEW),
        Item::new(7, "Open Hub")
            .icon(glyph::SYSTEM)
            .hint("Click")
            .sep(),
        Item::new(8, "Check now").icon(glyph::RELOAD),
        Item::new(9, "Pause checks for a day").checked(false),
        Item::new(10, "Quit Hub").icon(glyph::POWER).sep(),
    ];
    let mut m = Menu::new(items, icon.right() + 40.0, tb.y - 8.0, true).above();
    let room = Rect::new(0.0, 0.0, w, tb.y);
    m.place_in(p, room);
    let rc = m.rect();
    let tip = Rect::new(rc.x, rc.y - 58.0, rc.w, 48.0);
    p.shadow(tip, 8.0, pal.dark);
    p.frame(tip, 8.0, pal.popup, pal.popup_line);
    p.text(
        Face::Semibold,
        13.0,
        tip.x + 14.0,
        tip.y + 16.0,
        "Hub: 3 updates",
        pal.text,
        tip.w - 28.0,
    );
    p.text(
        Face::Ui,
        12.0,
        tip.x + 14.0,
        tip.y + 34.0,
        "Checked 4 minutes ago \u{b7} next check in 6 h",
        pal.muted,
        tip.w - 28.0,
    );
    let hv = m.row_rect(1);
    m.pointer(hv.cx(), hv.cy());
    m.draw_in(p, pal, room);
}

// ---- Settings --------------------------------------------------------------------------------

pub fn settings(p: &mut Painter, pal: &Palette, w: f32, h: f32, page: usize) {
    win::window(p, pal, w, h, &Scene::default());
    draw_backdrop(p, pal, w, h);
    let pgs = pages::pages();
    engine::draw(p, pal, &pgs, &pages::opts(page), w, h);
}
