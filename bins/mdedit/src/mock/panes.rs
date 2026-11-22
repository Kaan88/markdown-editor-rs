//! The pane under the list: what the selected app's update brings, how an update is going, what
//! went wrong, or why there is nothing to install.

use super::data::{self, APPS, amber, blue, green, red};
use super::ui;
use super::win::{PANE_TAB_H, Pane, Scene};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::theme::Palette;

const PAD: f32 = 20.0;

fn tabs(pane: Pane) -> &'static [&'static str] {
    match pane {
        Pane::Notes => &["What's new", "Files", "Installed"],
        Pane::Steps => &["Progress", "What's new", "Files"],
        Pane::Failure => &["What happened", "What's new", "Files"],
        Pane::NoBuild => &["Platforms", "What's new"],
        Pane::HubSelf => &["Updating Hub", "What's new"],
        Pane::None => &[],
    }
}

fn step(p: &mut Painter, pal: &Palette, x: f32, cy: f32, state: u8) {
    // 0 done, 1 doing, 2 to do
    let r = Rect::new(x, cy - 8.0, 16.0, 16.0);
    match state {
        0 => {
            p.rbox(r, 8.0, green(pal));
            p.icon(glyph::CHECK, 11.0, r.cx(), r.cy(), pal.page);
        }
        1 => {
            p.rbox(r, 8.0, pal.accent);
            p.rbox(
                Rect::new(r.x + 5.0, r.y + 5.0, 6.0, 6.0),
                3.0,
                pal.on_accent,
            );
        }
        _ => p.frame(r, 8.0, pal.page, pal.faint),
    }
}

pub fn draw(p: &mut Painter, pal: &Palette, r: Rect, sc: &Scene) {
    p.fill(r, pal.pane);
    let px = p.px().max(1.0);
    p.fill(Rect::new(r.x, r.y, r.w, px), pal.line);
    let app = &APPS[sc.sel];
    // The tab strip.
    let mut x = r.x + 16.0;
    let cy = r.y + PANE_TAB_H / 2.0 + 1.0;
    for (i, t) in tabs(sc.pane).iter().enumerate() {
        let wd = p.measure(Face::Ui, 13.0, t);
        let on = i == 0;
        p.text(
            if on { Face::Semibold } else { Face::Ui },
            13.0,
            x,
            cy,
            t,
            if on { pal.text } else { pal.muted },
            200.0,
        );
        if on {
            p.fill(
                Rect::new(x - 4.0, r.y + PANE_TAB_H - 2.0, wd + 8.0, 2.0),
                pal.accent,
            );
        }
        x += wd + 28.0;
    }
    p.fill(
        Rect::new(r.x, r.y + PANE_TAB_H, r.w, px),
        mix(pal.pane, pal.line, 0.7),
    );
    let inst = sc.inst[sc.sel].unwrap_or("not installed");
    let head = match sc.pane {
        Pane::Steps | Pane::Notes | Pane::Failure => {
            format!("{}  {} \u{2192} {}", app.name, inst, app.latest)
        }
        _ => app.name.to_string(),
    };
    p.text_right(Face::Ui, 12.5, r.right() - 16.0, cy, &head, pal.muted);

    let body = Rect::new(
        r.x + PAD,
        r.y + PANE_TAB_H + 14.0,
        r.w - 2.0 * PAD,
        r.h - PANE_TAB_H - 22.0,
    );
    let old = p.clip(Rect::new(
        r.x,
        r.y + PANE_TAB_H + px,
        r.w,
        r.h - PANE_TAB_H - px,
    ));
    let split = (body.w * 0.56).floor();
    let left = Rect::new(body.x, body.y, split - 24.0, body.h);
    let right = Rect::new(body.x + split, body.y, body.w - split, body.h);
    p.fill(
        Rect::new(right.x - 12.0, body.y, px, body.h),
        mix(pal.pane, pal.line, 0.7),
    );
    match sc.pane {
        Pane::Notes => notes(p, pal, left, right, sc),
        Pane::Steps => steps(p, pal, left, right),
        Pane::Failure => failure(p, pal, left, right),
        Pane::NoBuild => no_build(p, pal, left, right),
        Pane::HubSelf => hub_self(p, pal, left, right),
        Pane::None => {}
    }
    p.restore(old);
}

fn heading(p: &mut Painter, pal: &Palette, r: Rect, t: &str, sub: &str, c: Rgb) -> f32 {
    p.text(Face::Semibold, 14.0, r.x, r.y + 9.0, t, c, r.w);
    p.text(Face::Ui, 12.0, r.x, r.y + 29.0, sub, pal.muted, r.w);
    r.y + 46.0
}

fn notes(p: &mut Painter, pal: &Palette, l: Rect, r: Rect, sc: &Scene) {
    let a = &APPS[sc.sel];
    let y = heading(
        p,
        pal,
        l,
        &format!("{} {}", a.name, a.latest),
        "Released 21 Nov 2026 \u{b7} one release newer than yours",
        pal.text,
    );
    let y = ui::para(
        p,
        Face::Ui,
        12.5,
        l.x,
        y,
        l.w,
        "An option to leave out tabs you barely looked at, and a tidier AI menu.",
        pal.text,
    );
    p.text(Face::Semibold, 11.5, l.x, y + 14.0, "NEW", pal.faint, 100.0);
    let y = ui::para(
        p,
        Face::Ui,
        12.5,
        l.x,
        y + 24.0,
        l.w,
        "Don't reopen tabs barely looked at: tabs you looked at for less than a number of seconds you choose are not opened again at start. Off by default.",
        pal.muted,
    );
    p.text(
        Face::Semibold,
        11.5,
        l.x,
        y + 12.0,
        "FIXED",
        pal.faint,
        100.0,
    );
    let y = ui::para(
        p,
        Face::Ui,
        12.5,
        l.x,
        y + 22.0,
        l.w,
        "In the AI menu, a long saved prompt's name no longer runs under its Modify button.",
        pal.muted,
    );
    p.text(
        Face::Ui,
        12.5,
        l.x,
        y + 12.0,
        "Read all the notes since 1.2.0",
        blue(pal),
        l.w,
    );

    let mut y = r.y;
    let lw = 92.0;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Installs",
        "mdedit-1.2.1-windows-x86_64.exe",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Beside it",
        "mdview-engine.exe, mdedit-ai.exe",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "From",
        "github.com/Kaan88/markdown-editor-rs",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Checked with",
        "SHA-256 listed in mdedit-update-2.txt",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Into",
        "Programs\\mdedit, where it is now",
        false,
    );
    ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Left alone",
        "settings, session, snippets",
        false,
    );
}

fn steps(p: &mut Painter, pal: &Palette, l: Rect, r: Rect) {
    let rows: [(&str, &str, u8); 4] = [
        (
            "Download",
            "mdedit-1.2.1-windows-x86_64.exe, then the two engines",
            1,
        ),
        (
            "Check the SHA-256 of each file",
            "against the values in mdedit-update-2.txt",
            2,
        ),
        (
            "Replace the files",
            "the old exe is kept beside it, for Roll back",
            2,
        ),
        (
            "Start mdedit again if it was open",
            "it is not open now, so nothing to restart",
            2,
        ),
    ];
    let mut y = l.y;
    for (i, (t, sub, st)) in rows.iter().enumerate() {
        step(p, pal, l.x, y + 10.0, *st);
        p.text(
            Face::Semibold,
            13.0,
            l.x + 28.0,
            y + 9.0,
            t,
            if *st == 2 { pal.muted } else { pal.text },
            l.w - 28.0,
        );
        p.text(
            Face::Ui,
            12.0,
            l.x + 28.0,
            y + 28.0,
            sub,
            pal.muted,
            l.w - 28.0,
        );
        if i == 0 {
            ui::progress(
                p,
                pal,
                Rect::new(l.x + 28.0, y + 42.0, l.w - 28.0 - 110.0, 6.0),
                0.64,
                pal.accent,
                pal.pane,
            );
            p.text_right(
                Face::Ui,
                12.0,
                l.right(),
                y + 45.0,
                "22.5 of 35.2 MB \u{b7} 6.2 MB/s",
                pal.muted,
            );
            y += 22.0;
        }
        y += 48.0;
    }
    let lw = 100.0;
    let mut y = r.y;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Manifest",
        "mdedit-update-2.txt, from the latest release",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Expected",
        "9c1e4b7a\u{2026}d302 (exe)",
        true,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Staged in",
        "Hub\\cache\\mdedit-1.2.1",
        false,
    );
    ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Left alone",
        "settings, session, snippets",
        false,
    );
}

fn failure(p: &mut Painter, pal: &Palette, l: Rect, r: Rect) {
    let y = heading(
        p,
        pal,
        l,
        "Sprawl 1.0.5 was not installed",
        "The download does not match what the release promises",
        red(pal),
    );
    let y = ui::para(
        p,
        Face::Ui,
        12.5,
        l.x,
        y,
        l.w,
        "The SHA-256 of the file differs from the one in sprawl-update.json, so Hub did not run it or copy it anywhere.",
        pal.text,
    );
    let mut y = y + 6.0;
    for (k, v) in [
        ("Expected", "4be1f09c\u{2026}77a2"),
        ("Received", "91c7d3e8\u{2026}0b5f"),
    ] {
        p.text(Face::Ui, 12.0, l.x, y + 9.0, k, pal.muted, 80.0);
        p.text(
            Face::Mono,
            12.0,
            l.x + 80.0,
            y + 9.0,
            v,
            pal.text,
            l.w - 80.0,
        );
        y += 20.0;
    }
    y += 6.0;
    for t in [
        "Sprawl 1.0.3 is still installed, unchanged",
        "The downloaded file was deleted",
    ] {
        step(p, pal, l.x, y + 9.0, 0);
        p.text(Face::Ui, 12.5, l.x + 26.0, y + 9.0, t, pal.text, l.w - 26.0);
        y += 22.0;
    }
    let lw = 100.0;
    let mut y = r.y;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Manifest",
        "signature valid (sprawl-update.json.sig)",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Download",
        "github.com/Kaan88/sprawl/releases/download/v1.0.5/sprawl.exe",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Tried",
        "once, 2 minutes ago",
        false,
    );
    let by = y + 8.0;
    let b = ui::sbtn(p, pal, r.x, by + 14.0, "Try again", true, true);
    let b = ui::sbtn(
        p,
        pal,
        b.right() + 8.0,
        by + 14.0,
        "Copy details",
        false,
        true,
    );
    ui::sbtn(
        p,
        pal,
        b.right() + 8.0,
        by + 14.0,
        "Open the release page",
        false,
        true,
    );
}

fn no_build(p: &mut Painter, pal: &Palette, l: Rect, r: Rect) {
    let y = heading(
        p,
        pal,
        l,
        "Sprawl has no Linux build",
        "Windows x64 only, so far",
        pal.text,
    );
    let y = ui::para(
        p,
        Face::Ui,
        12.5,
        l.x,
        y,
        l.w,
        "Hub installs only what a release carries for this platform, and it does not guess. When the next Sprawl release has a Linux file, this row turns into an Install button.",
        pal.text,
    );
    let by = y + 12.0;
    let b = ui::sbtn(
        p,
        pal,
        l.x,
        by + 14.0,
        "Open the releases page",
        false,
        true,
    );
    ui::sbtn(
        p,
        pal,
        b.right() + 8.0,
        by + 14.0,
        "Tell me when there is one",
        false,
        true,
    );
    let lw = 100.0;
    let mut y = r.y;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Release files",
        "Sprawl-Setup-1.0.5.exe, sprawl.exe, Microsoft.WindowsAppRuntime.Bootstrap.dll, sprawl-update.json",
        false,
    );
    y = ui::kv(p, pal, r.x, y, lw, r.w, "Windows", "yes", false);
    y = ui::kv(p, pal, r.x, y, lw, r.w, "Linux", "no", false);
    ui::kv(p, pal, r.x, y, lw, r.w, "macOS", "no", false);
}

fn hub_self(p: &mut Painter, pal: &Palette, l: Rect, r: Rect) {
    let rows: [(&str, &str, u8); 4] = [
        (
            "Hub 0.4.0 is downloaded and checked",
            "SHA-256 from hub-update.txt",
            0,
        ),
        (
            "Hub finishes last",
            "it never restarts while an app is being installed",
            0,
        ),
        (
            "Restart Hub to finish",
            "it swaps its own exe, keeps the old one, and starts again",
            1,
        ),
        (
            "Nothing else changes",
            "the window, the list and the settings stay as they are",
            2,
        ),
    ];
    let mut y = l.y;
    for (t, sub, st) in rows {
        step(p, pal, l.x, y + 10.0, st);
        p.text(
            Face::Semibold,
            13.0,
            l.x + 28.0,
            y + 9.0,
            t,
            if st == 2 { pal.muted } else { pal.text },
            l.w - 28.0,
        );
        p.text(
            Face::Ui,
            12.0,
            l.x + 28.0,
            y + 28.0,
            sub,
            pal.muted,
            l.w - 28.0,
        );
        y += 40.0;
    }
    let lw = 100.0;
    let mut y = r.y;
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Version",
        "0.3.0 \u{2192} 0.4.0",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "Replaces",
        "hub.exe, by the staged-file swap every app here uses",
        false,
    );
    y = ui::kv(
        p,
        pal,
        r.x,
        y,
        lw,
        r.w,
        "If it fails",
        "the old exe is put back and Hub starts as before",
        false,
    );
    let _ = (amber(pal), data::HUB, y);
}
