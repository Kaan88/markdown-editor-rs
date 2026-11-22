//! What opens over the file manager window: menus, the context menu with and without the Windows
//! shell's entries, the transfer queue and its conflict question, batch rename, delete, Settings,
//! and the results of a find.

use super::fmdata::{self as d, K};
use super::fmwin::{self, Content, Out, PaneSt, Prev, Scene, Strip, View, amber, blue, green, red};
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

/// Footer buttons: `left` at the left edge, `right` from the right edge, the last one primary.
pub fn footer(p: &mut Painter, pal: &Palette, f: Rect, l: f32, r: f32, left: &[&str], right: &[&str]) {
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
        draw_dialog_button(p, pal, Rect::new(x - wd, by, wd, 32.0), t, last, false, false);
        x -= wd + 10.0;
    }
}

fn arrow_at(p: &mut Painter, pal: &Palette, m: &M, i: usize) {
    let r = m.row_rect(i);
    p.icon(glyph::RIGHT, 10.0, r.right() - 12.0, r.cy(), pal.muted);
}

// ---- the menu bar's menus ---------------------------------------------------------------------

pub fn menu_tools(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.menubar = Some(4);
    sc.panes[0].hover = None;
    let out = fmwin::window(p, pal, w, h, &sc);
    let b = out.menubar[4];
    let items = vec![
        it(0, "Batch rename\u{2026}").icon(fmwin::RENAME).hint("Ctrl+F2"),
        it(1, "Compare folders\u{2026}").icon(fmwin::COMPARE).hint("Ctrl+Alt+C"),
        it(2, "Synchronise folders\u{2026}").icon(glyph::REPLACE),
        it(3, "Find files\u{2026}").icon(glyph::SEARCH).hint("Ctrl+Shift+F").sep(),
        it(4, "Calculate folder sizes").icon(glyph::SLIDERS),
        it(5, "Checksums\u{2026}").icon(fmwin::HASH),
        it(6, "Transfer queue").icon(glyph::DOWNLOAD).hint("Ctrl+J").sep(),
        it(7, "Open terminal here").icon(fmwin::TERM).hint("Ctrl+`"),
        it(8, "Open in mdedit").icon(glyph::EDIT).hint("Ctrl+E").sep(),
        it(9, "Open in mdview").icon(glyph::VIEW),
        it(10, "Options\u{2026}").icon(glyph::SLIDERS).hint("Ctrl+,").sep(),
    ];
    let mut m = M::new(items, b.x, b.bottom() + 2.0, false).access_keys(true);
    m.hover = Some(0);
    m.draw(p, pal, w, h);
}

pub fn menu_view(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.menubar = Some(2);
    let out = fmwin::window(p, pal, w, h, &sc);
    let b = out.menubar[2];
    let items = vec![
        it(0, "Details").icon(fmwin::VIEW_DETAILS).hint("Ctrl+1").checked(true),
        it(1, "List").icon(fmwin::VIEW_LIST).hint("Ctrl+2").checked(false),
        it(2, "Icons").icon(fmwin::VIEW_ICONS).hint("Ctrl+3").checked(false),
        it(3, "Sort by").icon(glyph::SORT).sep(),
        it(4, "Group by").icon(fmwin::GROUP),
        it(5, "File extensions").hint("Ctrl+Alt+E").checked(true).sep(),
        it(6, "Hidden files").hint("Ctrl+H").checked(false),
        it(7, "Folders first").checked(true),
        it(8, "Navigation pane").icon(glyph::SIDEBAR).hint("Ctrl+B").checked(true).sep(),
        it(9, "Preview pane").hint("Alt+P").checked(false),
        it(10, "Menu bar").hint("Alt").checked(true),
        it(11, "Split view").icon(fmwin::SPLIT_PANE).hint("F3").sep().checked(false),
        it(12, "Swap panes").hint("Ctrl+U").enabled(false),
        it(13, "Refresh").icon(glyph::RELOAD).hint("F5").sep(),
    ];
    let mut m = M::new(items, b.x, b.bottom() + 2.0, false).access_keys(true);
    m.hover = Some(3);
    m.draw(p, pal, w, h);
    let r = m.row_rect(3);
    let mut sub = M::new(
        vec![
            it(20, "Name").checked(true),
            it(21, "Date modified"),
            it(22, "Size"),
            it(23, "Type"),
            it(24, "Extension"),
            it(25, "Natural order (2 before 10)").checked(true).sep(),
            it(26, "Ascending").checked(true),
            it(27, "Descending"),
        ],
        m.rect().right() + 2.0,
        r.y - 4.0,
        false,
    );
    sub.draw(p, pal, w, h);
    arrow_at(p, pal, &m, 3);
    arrow_at(p, pal, &m, 4);
}

// ---- the context menu -------------------------------------------------------------------------

fn context_items(split: bool) -> Vec<Item<u8>> {
    let mut v = vec![
        it(0, "Open").icon(glyph::OPEN).hint("Enter"),
        it(1, "Open with"),
        it(2, "Edit in mdedit").icon(glyph::EDIT),
        it(3, "View in mdview").icon(glyph::VIEW),
        it(4, "Cut").icon(glyph::CUT).hint("Ctrl+X").sep(),
        it(5, "Copy").icon(glyph::COPY).hint("Ctrl+C"),
        it(6, "Copy as path").icon(glyph::LINK).hint("Ctrl+Shift+C"),
        it(7, "Copy to\u{2026}").icon(fmwin::COPY_TO).sep(),
        it(8, "Move to\u{2026}"),
    ];
    if split {
        v.push(it(9, "Copy to other pane").hint("F5"));
    }
    v.extend([
        it(10, "Rename 3 files\u{2026}").icon(fmwin::RENAME).hint("F2").sep(),
        it(11, "Compress to ZIP").icon(fmwin::ZIP),
        it(12, "Checksums"),
        it(13, "Properties").icon(glyph::INFO).hint("Alt+Enter"),
        it(14, "Delete").icon(glyph::DELETE).hint("Del").sep(),
    ]);
    v
}

pub fn context(p: &mut Painter, pal: &Palette, w: f32, h: f32, shell: bool, split: bool) {
    let mut sc = Scene::default();
    if split {
        sc.split = true;
        sc.panes[1].view = View::Details;
    }
    sc.panes[0].hover = Some(12);
    let out = fmwin::window(p, pal, w, h, &sc);
    let row = out.rows[0][12];
    let (x, y) = (row.x + 150.0, row.cy());
    let mut items = context_items(split);
    if shell {
        items.extend([
            it(29, "Windows shell extensions").sep().enabled(false),
            it(30, "7-Zip"),
            it(31, "CRC SHA"),
            it(32, "Git Bash Here"),
            it(33, "Open in Terminal"),
            it(34, "Scan with Microsoft Defender\u{2026}"),
            it(35, "Share"),
        ]);
    } else {
        items.push(it(30, "More options\u{2026}").hint("Shift+F10").sep());
    }
    let mut m = M::new(items, x, y, false).access_keys(true);
    m.hover = Some(1);
    m.draw(p, pal, w, h);
    arrow_at(p, pal, &m, 1);
    if let Some(i) = m.items.iter().position(|x| x.label == "Checksums") {
        arrow_at(p, pal, &m, i);
    }
    if shell {
        if let Some(i) = m.items.iter().position(|x| x.label == "7-Zip") {
            arrow_at(p, pal, &m, i);
        }
    }
    // The submenu of "Open with".
    let r = m.row_rect(1);
    let mut sub = M::new(
        vec![
            it(40, "mdedit").icon(glyph::EDIT).hint("default"),
            it(41, "mdview").icon(glyph::VIEW),
            it(42, "Visual Studio Code"),
            it(43, "Notepad"),
            it(44, "Choose another app\u{2026}").sep(),
        ],
        m.rect().right() + 2.0,
        r.y - 4.0,
        false,
    );
    sub.hover = Some(0);
    sub.draw(p, pal, w, h);
}

pub fn context_bg(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.panes[0].sel = vec![];
    let out = fmwin::window(p, pal, w, h, &sc);
    let r = out.panes[0];
    let items = vec![
        it(0, "New").sep().icon(glyph::PLUS),
        it(1, "Paste").icon(glyph::PASTE).hint("Ctrl+V"),
        it(2, "Paste shortcut").enabled(false),
        it(3, "View").icon(glyph::VIEW).sep(),
        it(4, "Sort by").icon(glyph::SORT),
        it(5, "Group by").icon(fmwin::GROUP),
        it(6, "Refresh").icon(glyph::RELOAD).hint("F5"),
        it(7, "Open terminal here").icon(fmwin::TERM).sep(),
        it(8, "Open in mdedit").icon(glyph::EDIT),
        it(9, "Properties").icon(glyph::INFO).hint("Alt+Enter").sep(),
    ];
    let mut m = M::new(items, r.x + 340.0, r.y + 150.0, false).access_keys(true);
    m.hover = Some(0);
    m.draw(p, pal, w, h);
    for i in [0, 3, 4, 5] {
        arrow_at(p, pal, &m, i);
    }
    let r0 = m.row_rect(0);
    let mut sub = M::new(
        vec![
            it(20, "Folder").icon(fmwin::NEWFOLDER).hint("Ctrl+Shift+N"),
            it(21, "Text file").icon(glyph::NEW_FILE),
            it(22, "Markdown file").icon(glyph::DOC),
            it(23, "Shortcut"),
            it(24, "Compressed (zipped) folder").icon(fmwin::ZIP).sep(),
        ],
        m.rect().right() + 2.0,
        r0.y - 4.0,
        false,
    );
    sub.hover = Some(0);
    sub.draw(p, pal, w, h);
}

// ---- the transfer queue and its question ------------------------------------------------------

struct Job {
    title: &'static str,
    sub: &'static str,
    frac: f32,
    state: &'static str,
    paused: bool,
    waiting: bool,
}

fn jobs() -> Vec<Job> {
    vec![
        Job {
            title: "Copy 1,204 items to E:\\Backup\\mdedit",
            sub: "from C:\\Users\\you\\Projects\\mdedit  \u{b7}  docs\\DESIGN.md  \u{b7}  612 MB of 1.6 GB",
            frac: 0.38,
            state: "46 MB/s \u{b7} 2 min left",
            paused: false,
            waiting: false,
        },
        Job {
            title: "Move 3 items to D:\\Archive",
            sub: "from C:\\Users\\you\\Downloads  \u{b7}  holiday.mp4  \u{b7}  waits for the job above (same drive)",
            frac: 0.0,
            state: "Waiting",
            paused: false,
            waiting: true,
        },
        Job {
            title: "Copy 48 items to \\\\nas\\photos\\Trip",
            sub: "network share  \u{b7}  paused by you  \u{b7}  resumes where it stopped",
            frac: 0.71,
            state: "Paused",
            paused: true,
            waiting: false,
        },
    ]
}

fn job_row(p: &mut Painter, pal: &Palette, r: Rect, j: &Job, bg: Rgb) {
    let c = if j.paused { amber(pal) } else { pal.accent };
    p.icon(if j.waiting { glyph::RECENT } else { glyph::COPY }, 15.0, r.x + 14.0, r.y + 18.0, c);
    p.text(Face::Semibold, 13.0, r.x + 34.0, r.y + 18.0, j.title, pal.text, r.w - 200.0);
    p.text(Face::Ui, 11.5, r.x + 34.0, r.y + 38.0, j.sub, pal.muted, r.w - 200.0);
    ui::progress(p, pal, Rect::new(r.x + 34.0, r.y + 52.0, r.w - 200.0, 6.0), j.frac, c, bg);
    p.text(Face::Ui, 11.5, r.x + 34.0, r.y + 70.0, j.state, if j.paused { amber(pal) } else { pal.muted }, 200.0);
    let g = if j.paused { glyph::PLAY } else { fmwin::PAUSE };
    let bx = r.right() - 14.0;
    p.icon(glyph::CLOSE, 12.0, bx - 6.0, r.y + 38.0, pal.muted);
    p.icon(g, 13.0, bx - 40.0, r.y + 38.0, pal.muted);
    p.icon(glyph::UP, 12.0, bx - 74.0, r.y + 38.0, pal.muted);
}

pub fn queue(p: &mut Painter, pal: &Palette, w: f32, h: f32, conflict: bool) {
    let mut sc = Scene::default();
    sc.split = true;
    sc.active = 0;
    sc.strip = Strip::Copy;
    sc.panes[1] = PaneSt::new(Content::Compare);
    fmwin::window(p, pal, w, h, &sc);
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(760.0, 520.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 24.0;
    p.text(Face::Semibold, 16.0, l, c.y + 30.0, "Transfers", pal.text, 300.0);
    p.text(Face::Ui, 12.0, l + 100.0, c.y + 31.0, "3 jobs \u{b7} one at a time per drive, so two copies do not fight over a disk", pal.muted, c.w - 160.0);
    p.icon(glyph::CLOSE, 11.0, c.right() - 24.0, c.y + 30.0, pal.muted);
    let mut y = c.y + 56.0;
    for (i, j) in jobs().iter().enumerate() {
        let r = Rect::new(l - 8.0, y, c.w - 32.0, 84.0);
        if i == 0 {
            p.rbox(r, 6.0, mix(pal.popup, pal.text, 0.04));
        }
        job_row(p, pal, r, j, if i == 0 { mix(pal.popup, pal.text, 0.04) } else { pal.popup });
        y += 88.0;
    }
    y += 4.0;
    p.fill(Rect::new(l, y, c.w - 48.0, p.px().max(1.0)), pal.popup_line);
    y += 14.0;
    p.text(Face::Semibold, 12.0, l, y + 8.0, "Finished today", pal.muted, 200.0);
    y += 24.0;
    for (t, s, ok) in [
        ("Copy 6 items to D:\\Archive", "done 12:04 \u{b7} 214 MB \u{b7} verified", true),
        ("Delete 3 items", "done 11:40 \u{b7} in the Recycle Bin \u{b7} Undo", true),
    ] {
        p.icon(glyph::CHECK, 12.0, l + 6.0, y + 9.0, if ok { green(pal) } else { red(pal) });
        p.text(Face::Ui, 12.5, l + 26.0, y + 9.0, t, pal.text, 300.0);
        p.text(Face::Ui, 11.5, l + 290.0, y + 9.0, s, pal.muted, c.w - 340.0);
        y += 22.0;
    }
    footer(p, pal, f, l, c.right() - 24.0, &["Pause all"], &["Close"]);
    if conflict {
        draw_backdrop(p, pal, w, h);
        conflict_card(p, pal, w, h);
    }
}

fn conflict_card(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let c = dialog_card(620.0, 470.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    p.icon(glyph::WARN, 18.0, l + 9.0, c.y + 38.0, amber(pal));
    p.text(Face::Semibold, 16.0, l + 30.0, c.y + 36.0, "CLAUDE.md is already in E:\\Backup\\mdedit", pal.text, r - l - 30.0);
    p.text(Face::Ui, 12.5, l + 30.0, c.y + 58.0, "Copy 1,204 items \u{b7} 12 files conflict so far", pal.muted, r - l - 30.0);
    let mut y = c.y + 84.0;
    let cw = (r - l - 12.0) / 2.0;
    for (i, (head, sz, dt, note, nc)) in [
        ("Copying", "9.2 KB", "2026-11-22 13:55", "Newer", green(pal)),
        ("Already there", "8.9 KB", "2026-11-20 17:40", "Older", pal.muted),
    ]
    .iter()
    .enumerate()
    {
        let b = Rect::new(l + i as f32 * (cw + 12.0), y, cw, 118.0);
        p.frame(b, 8.0, mix(pal.popup, pal.text, 0.03), pal.popup_line);
        p.text(Face::Semibold, 12.0, b.x + 14.0, b.y + 20.0, head, pal.muted, cw);
        p.icon(glyph::DOC, 22.0, b.x + 30.0, b.y + 62.0, d::tint(K::Md));
        p.text(Face::Semibold, 13.0, b.x + 54.0, b.y + 52.0, "CLAUDE.md", pal.text, cw - 64.0);
        p.text(Face::Ui, 12.0, b.x + 54.0, b.y + 72.0, sz, pal.muted, cw - 64.0);
        p.text(Face::Ui, 12.0, b.x + 54.0, b.y + 90.0, dt, pal.muted, cw - 64.0);
        ui::chip_on(p, mix(pal.popup, pal.text, 0.03), b.right() - 56.0, b.y + 20.0, note, *nc);
    }
    y += 132.0;
    let opts: [(&str, &str, bool); 4] = [
        ("Replace", "The copy in E:\\Backup is overwritten; it is not kept", false),
        ("Replace only if newer", "Older and same-time files are skipped, with no more questions", true),
        ("Skip", "Keep the file that is there", false),
        ("Keep both", "Copy as CLAUDE (2).md", false),
    ];
    for (t, s, on) in opts {
        ui::radio(p, pal, l + 4.0, y + 12.0, on);
        p.text(Face::Semibold, 13.0, l + 32.0, y + 10.0, t, pal.text, r - l - 40.0);
        p.text(Face::Ui, 11.5, l + 32.0, y + 28.0, s, pal.muted, r - l - 40.0);
        y += 38.0;
    }
    ui::check(p, pal, l + 4.0, y + 12.0, true, "Do this for the other conflicts in this job (11 more)", r - l - 40.0);
    footer(p, pal, f, l, c.right() - 28.0, &["Cancel the copy"], &["Pause", "Continue"]);
}

// ---- batch rename -----------------------------------------------------------------------------

struct RenRow {
    a: [&'static str; 3],
    b: [&'static str; 3],
    note: &'static str,
}

fn seg(p: &mut Painter, pal: &Palette, x: &mut f32, cy: f32, parts: [&str; 3], hi: Rgb, col: Rgb, max: f32, x0: f32) {
    for (i, s) in parts.iter().enumerate() {
        if s.is_empty() {
            continue;
        }
        let wd = p.measure(Face::Mono, 12.0, s);
        if i == 1 {
            p.rbox(Rect::new(*x - 1.0, cy - 10.0, wd + 2.0, 20.0), 3.0, mix(pal.popup, hi, 0.28));
        }
        p.text(Face::Mono, 12.0, *x, cy, s, col, max - (*x - x0));
        *x += wd;
    }
}

pub fn rename(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.panes[0] = PaneSt::new(Content::Pictures);
    sc.panes[0].view = View::Details;
    sc.panes[0].sel = vec![0, 1, 2, 3, 4];
    fmwin::window(p, pal, w, h, &sc);
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(980.0, 620.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 24.0;
    let r = c.right() - 24.0;
    p.text(Face::Semibold, 16.0, l, c.y + 30.0, "Rename 8 files", pal.text, 300.0);
    p.text(Face::Ui, 12.0, l + 130.0, c.y + 31.0, "in Pictures\\Trip \u{b7} nothing changes until Rename", pal.muted, 400.0);
    let narrow = c.w < 820.0;
    let lw = if narrow { 0.0 } else { 300.0 };
    let top = c.y + 56.0;
    // The rules, one card each, in the order they run.
    if !narrow {
        let mut y = top;
        for (n, title, a, b, extra) in [
            (1, "Replace text", "IMG_(\\d+)", "trip-$1", "Regular expression"),
            (2, "Extension", "lower case", "", ""),
            (3, "Counter", "none", "", ""),
        ] {
            let hh = if n == 1 { 132.0 } else { 72.0 };
            let b_ = Rect::new(l, y, lw, hh);
            p.frame(b_, 8.0, mix(pal.popup, pal.text, 0.03), pal.popup_line);
            p.rbox(Rect::new(b_.x + 10.0, b_.y + 10.0, 20.0, 20.0), 10.0, pal.accent);
            p.text_center(Face::Semibold, 11.0, Rect::new(b_.x + 10.0, b_.y + 10.0, 20.0, 20.0), &n.to_string(), pal.on_accent);
            p.text(Face::Semibold, 13.0, b_.x + 40.0, b_.y + 20.0, title, pal.text, lw - 80.0);
            p.icon(glyph::CLOSE, 9.0, b_.right() - 16.0, b_.y + 20.0, pal.faint);
            if n == 1 {
                p.text(Face::Ui, 11.5, b_.x + 14.0, b_.y + 46.0, "Find", pal.muted, 60.0);
                ui::field(p, pal, Rect::new(b_.x + 56.0, b_.y + 34.0, lw - 70.0, 26.0), a, true, false);
                p.text(Face::Ui, 11.5, b_.x + 14.0, b_.y + 78.0, "With", pal.muted, 60.0);
                ui::field(p, pal, Rect::new(b_.x + 56.0, b_.y + 66.0, lw - 70.0, 26.0), b, true, true);
                ui::check(p, pal, b_.x + 14.0, b_.y + 108.0, true, extra, lw - 40.0);
            } else {
                p.text(Face::Ui, 12.5, b_.x + 14.0, b_.y + 50.0, a, pal.muted, lw - 30.0);
            }
            y += hh + 8.0;
        }
        let b_ = Rect::new(l, y, lw, 30.0);
        p.frame(b_, 6.0, pal.popup, pal.popup_line);
        p.icon(glyph::PLUS, 11.0, b_.x + 16.0, b_.cy(), pal.accent);
        p.text(Face::Ui, 12.5, b_.x + 30.0, b_.cy(), "Add a rule: case, trim, date taken, insert\u{2026}", pal.muted, lw - 40.0);
        y += 44.0;
        ui::check(p, pal, l + 2.0, y + 8.0, true, "Only the selected files", lw - 30.0);
        ui::check(p, pal, l + 2.0, y + 32.0, false, "Include subfolders", lw - 30.0);
    }
    // The preview.
    let px = l + lw + if narrow { 0.0 } else { 16.0 };
    let pw = r - px;
    let pr = Rect::new(px, top, pw, c.bottom() - DIALOG_FOOTER - top - 40.0);
    p.frame(pr, 8.0, pal.field, pal.popup_line);
    p.text(Face::Semibold, 12.0, pr.x + 14.0, pr.y + 18.0, "Before", pal.muted, 100.0);
    p.text(Face::Semibold, 12.0, pr.x + pr.w / 2.0 + 14.0, pr.y + 18.0, "After", pal.muted, 100.0);
    p.fill(Rect::new(pr.x, pr.y + 34.0, pr.w, p.px().max(1.0)), pal.popup_line);
    let rows = [
        RenRow { a: ["IMG_", "", "0412.png"], b: ["", "trip-", "0412.png"], note: "" },
        RenRow { a: ["IMG_", "", "0413.png"], b: ["", "trip-", "0413.png"], note: "already exists" },
        RenRow { a: ["IMG_", "", "0414.png"], b: ["", "trip-", "0414.png"], note: "" },
        RenRow { a: ["IMG_0415.", "JPG", ""], b: ["trip-0415.", "jpg", ""], note: "" },
        RenRow { a: ["IMG_0416.", "JPG", ""], b: ["trip-0416.", "jpg", ""], note: "" },
        RenRow { a: ["harbour.png", "", ""], b: ["harbour.png", "", ""], note: "unchanged" },
        RenRow { a: ["market.png", "", ""], b: ["market.png", "", ""], note: "unchanged" },
        RenRow { a: ["sunset.PNG", "", ""], b: ["sunset.", "png", ""], note: "" },
    ];
    let mut y = pr.y + 36.0;
    for (i, rr) in rows.iter().enumerate() {
        let rect = Rect::new(pr.x + 2.0, y, pr.w - 4.0, 30.0);
        let cy = rect.cy();
        let conflict = rr.note == "already exists";
        let same = rr.note == "unchanged";
        if conflict {
            p.fill_a(rect, red(pal), 0.12);
        } else if i % 2 == 1 {
            p.fill_a(rect, pal.text, 0.03);
        }
        let mut x = pr.x + 14.0;
        let dim = if same { pal.faint } else { pal.text };
        // Old parts: for the replaced text, the removed part is the one that differs.
        let old = if rr.a[1].is_empty() && rr.a[0] == "IMG_" { ["", "IMG_", rr.a[2]] } else { rr.a };
        seg(p, pal, &mut x, cy, old, red(pal), dim, pw / 2.0 - 20.0, pr.x + 14.0);
        let mut x2 = pr.x + pr.w / 2.0 + 14.0;
        p.icon(glyph::FORWARD, 10.0, pr.x + pr.w / 2.0 - 4.0, cy, pal.faint);
        seg(p, pal, &mut x2, cy, rr.b, green(pal), if conflict { red(pal) } else { dim }, pw / 2.0 - 100.0, pr.x + pr.w / 2.0 + 14.0);
        if !rr.note.is_empty() {
            let col = if conflict { red(pal) } else { pal.faint };
            p.text_right(Face::Ui, 11.5, rect.right() - 12.0, cy, rr.note, col);
        }
        y += 30.0;
    }
    ui::check(p, pal, l + lw + 18.0, c.bottom() - DIALOG_FOOTER - 20.0, true, "Keep an undo for this rename", 300.0);
    p.icon(glyph::WARN, 13.0, px + 220.0 + 100.0, c.bottom() - DIALOG_FOOTER - 20.0, red(pal));
    p.text(Face::Ui, 12.0, px + 340.0, c.bottom() - DIALOG_FOOTER - 20.0, "7 will be renamed, 1 skipped: trip-0413.png exists", red(pal), pw - 330.0);
    footer(p, pal, f, l, r, &["Save these rules\u{2026}"], &["Cancel", "Rename 7 files"]);
}

// ---- delete -----------------------------------------------------------------------------------

pub fn delete(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.panes[0] = PaneSt::new(Content::Downloads);
    sc.panes[0].sel = vec![1, 2, 4];
    fmwin::window(p, pal, w, h, &sc);
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(560.0, 430.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let l = c.x + 28.0;
    let r = c.right() - 28.0;
    p.icon(glyph::DELETE, 20.0, l + 10.0, c.y + 40.0, pal.text);
    p.text(Face::Semibold, 16.0, l + 36.0, c.y + 38.0, "Move 3 items to the Recycle Bin?", pal.text, r - l - 40.0);
    p.text(Face::Ui, 12.5, l + 36.0, c.y + 60.0, "You can put them back from the Recycle Bin, or press Ctrl+Z.", pal.muted, r - l - 40.0);
    let mut y = c.y + 86.0;
    let list = Rect::new(l, y, r - l, 96.0);
    p.frame(list, 6.0, pal.field, pal.popup_line);
    let es = d::downloads().0;
    for (k, i) in [1usize, 2, 4].into_iter().enumerate() {
        let e = &es[i];
        let cy = list.y + 18.0 + k as f32 * 28.0;
        fmwin::small_icon(p, pal, e, list.x + 20.0, cy);
        p.text(Face::Ui, 13.0, list.x + 40.0, cy, e.name, pal.text, 280.0);
        p.text_right(Face::Ui, 12.0, list.right() - 14.0, cy, &d::human(e.size), pal.muted);
    }
    y += 112.0;
    // The sharp edge: an online-only file.
    let note = Rect::new(l, y, r - l, 66.0);
    p.frame(note, 6.0, mix(pal.popup, amber(pal), 0.10), mix(pal.popup_line, amber(pal), 0.5));
    p.icon(glyph::WARN, 14.0, note.x + 16.0, note.y + 20.0, amber(pal));
    ui::para(
        p,
        Face::Ui,
        12.0,
        note.x + 34.0,
        note.y + 8.0,
        note.w - 46.0,
        "tax-papers.zip is online only (OneDrive). Deleting it here deletes it from OneDrive too, and OneDrive's own bin keeps it for 30 days. Items on a network drive or over 4 GB are removed for good: the Recycle Bin cannot hold them.",
        pal.text,
    );
    y += 82.0;
    ui::check(p, pal, l + 4.0, y + 8.0, false, "Do not ask again for items that fit the Recycle Bin", r - l - 30.0);
    footer(p, pal, f, l, r, &["Delete permanently\u{2026}"], &["Cancel", "Move to Recycle Bin"]);
}

// ---- settings ---------------------------------------------------------------------------------

pub fn settings(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let sc = Scene::default();
    fmwin::window(p, pal, w, h, &sc);
    draw_backdrop(p, pal, w, h);
    let c = dialog_card(900.0, 640.0, w, h);
    let f = draw_dialog_card(p, pal, c);
    let side = 190.0;
    p.fill(Rect::new(c.x + 1.0, c.y + 1.0, side, c.h - DIALOG_FOOTER - 1.0), mix(pal.popup, pal.pane, 0.6));
    p.text(Face::Semibold, 16.0, c.x + 20.0, c.y + 30.0, "Options", pal.text, 150.0);
    for (i, t) in ["Browsing", "Operations", "Menus", "Preview", "Search", "Integration", "Advanced"].iter().enumerate() {
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
    p.text(Face::Semibold, 15.0, l, y + 4.0, "Browsing", pal.text, 200.0);
    y += 34.0;
    let rows: [(&str, &str, &str); 8] = [
        ("c", "Show file extensions", "On. A name is what it is; Explorer hides the part that says what the file is."),
        ("o", "Show hidden and system files", "Off. Ctrl+H toggles it for this window; the bar says when it is on."),
        ("c", "Folders first, natural order", "file2 before file10; case does not matter. Sort and group are kept per window."),
        ("r", "Remember the view of each folder", ""),
        ("c", "Calculate folder sizes", ""),
        ("c", "Sizes in", ""),
        ("c", "Dates as", ""),
        ("c", "Open items with", ""),
    ];
    for (i, (kind, t, sub)) in rows.iter().enumerate() {
        match i {
            0 | 2 => ui::check(p, pal, l, y + 10.0, true, t, 400.0),
            1 => ui::check(p, pal, l, y + 10.0, false, t, 400.0),
            _ => {
                p.text(Face::Ui, 13.0, l, y + 10.0, t, pal.text, 260.0);
            }
        }
        let _ = kind;
        if !sub.is_empty() {
            p.text(Face::Ui, 11.5, l + 26.0, y + 30.0, sub, pal.muted, r - l - 30.0);
            y += 54.0;
        } else {
            let vx = l + 270.0;
            match i {
                3 => {
                    mdshared_ui::segments::draw_segments(
                        p,
                        pal,
                        Rect::new(vx, y - 2.0, 300.0, 28.0),
                        &["Never", "Only if I set it", "Always"],
                        Some(1),
                        None,
                    );
                    p.text(Face::Ui, 11.5, l, y + 34.0, "Never: one default view everywhere. Only if I set it: View > Remember for this folder.", pal.muted, r - l);
                    y += 22.0;
                }
                4 => {
                    mdshared_ui::segments::draw_segments(p, pal, Rect::new(vx, y - 2.0, 300.0, 28.0), &["When I ask", "On hover", "Always"], Some(0), None);
                }
                5 => {
                    mdshared_ui::segments::draw_segments(p, pal, Rect::new(vx, y - 2.0, 300.0, 28.0), &["KB (1024)", "kB (1000)", "Bytes"], Some(0), None);
                }
                6 => {
                    mdshared_ui::segments::draw_segments(p, pal, Rect::new(vx, y - 2.0, 300.0, 28.0), &["2026-11-22 14:07", "22 Nov 2026", "System"], Some(0), None);
                }
                _ => {
                    mdshared_ui::segments::draw_segments(p, pal, Rect::new(vx, y - 2.0, 300.0, 28.0), &["Double click", "Single click"], Some(0), None);
                }
            }
            y += 42.0;
        }
    }
    y += 6.0;
    p.fill(Rect::new(l, y, r - l, p.px().max(1.0)), pal.popup_line);
    y += 14.0;
    p.text(Face::Semibold, 13.0, l, y + 8.0, "Where mdfiles starts", pal.text, 260.0);
    y += 28.0;
    for (i, t) in ["Where I left off: the tabs and both panes of last time", "Home"].iter().enumerate() {
        ui::radio(p, pal, l + 2.0, y + 8.0, i == 0);
        p.text(Face::Ui, 13.0, l + 28.0, y + 8.0, t, pal.text, r - l - 30.0);
        y += 26.0;
    }
    footer(p, pal, f, l, r, &["Restore defaults"], &["Close"]);
}

// ---- find results -----------------------------------------------------------------------------

pub fn find_results(p: &mut Painter, pal: &Palette, r: Rect) -> Vec<Rect> {
    let bar = Rect::new(r.x, r.y, r.w, 44.0);
    p.fill(bar, pal.pane);
    p.fill(Rect::new(r.x, bar.bottom() - p.px().max(1.0), r.w, p.px().max(1.0)), pal.line);
    let cy = bar.cy();
    let mut x = r.x + 14.0;
    p.text(Face::Ui, 12.5, x, cy, "In", pal.muted, 20.0);
    x += 24.0;
    let w1 = 150.0;
    ui::field(p, pal, Rect::new(x, cy - 14.0, w1, 28.0), "mdedit", false, false);
    x += w1 + 14.0;
    for (t, on) in [("File names", true), ("Contents", true), ("Subfolders", true), ("Case", false), ("Regex", false)] {
        let wd = p.measure(Face::Ui, 12.5, t) + 40.0;
        ui::check(p, pal, x, cy, on, t, wd);
        x += wd;
    }
    if r.w > 960.0 {
        p.text_right(Face::Ui, 12.0, r.right() - 14.0, cy, "skips target, .git, node_modules", pal.faint);
    }
    let old = p.clip(r);
    let mut y = bar.bottom();
    let hh = Rect::new(r.x, y, r.w, 26.0);
    p.text(Face::Semibold, 12.0, r.x + 44.0, hh.cy(), "Name", pal.muted, 100.0);
    p.text(Face::Semibold, 12.0, r.x + 300.0, hh.cy(), "Match", pal.muted, 100.0);
    p.fill(Rect::new(r.x, hh.bottom() - p.px().max(1.0), r.w, p.px().max(1.0)), pal.line);
    y += 26.0;
    let mut rows = Vec::new();
    for (i, h_) in d::hits().iter().enumerate() {
        let rr = Rect::new(r.x, y, r.w, 46.0);
        rows.push(rr);
        if i == 1 {
            p.rbox(Rect::new(rr.x + 4.0, rr.y + 1.0, rr.w - 8.0, rr.h - 2.0), 4.0, fmwin::sel_fill(pal, true));
        }
        let e = d::E { name: h_.name, k: h_.k, size: 0, date: "", flag: d::Flag::None };
        fmwin::small_icon(p, pal, &e, rr.x + 24.0, rr.y + 15.0);
        let nw = p.text(Face::Semibold, 13.0, rr.x + 44.0, rr.y + 15.0, h_.name, pal.text, 240.0);
        p.text(Face::Ui, 11.5, rr.x + 44.0 + nw + 10.0, rr.y + 16.0, h_.path, pal.faint, 200.0);
        if let Some(n) = h_.line {
            p.text_right(Face::Ui, 11.5, rr.right() - 14.0, rr.y + 15.0, &format!("line {n}"), pal.muted);
            let sx = rr.x + 44.0;
            let pre: String = h_.snippet.chars().take(h_.at.0).collect();
            let mid: String = h_.snippet.chars().skip(h_.at.0).take(h_.at.1 - h_.at.0).collect();
            let pw = p.measure(Face::Mono, 12.0, &pre);
            let mw = p.measure(Face::Mono, 12.0, &mid);
            p.rbox(Rect::new(sx + pw - 1.0, rr.y + 24.0, mw + 2.0, 18.0), 3.0, pal.find_mark);
            p.text(Face::Mono, 12.0, sx, rr.y + 33.0, h_.snippet, pal.text, rr.w - 70.0);
        } else {
            p.text(Face::Ui, 12.0, rr.x + 44.0, rr.y + 33.0, "file name matches", pal.muted, 200.0);
        }
        y += 46.0;
    }
    p.restore(old);
    rows
}

pub fn find(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let mut sc = Scene::default();
    sc.panes[0] = PaneSt::new(Content::Find);
    sc.preview = Some(Prev::Md);
    fmwin::window(p, pal, w, h, &sc);
}

pub fn blue_(pal: &Palette) -> Rgb {
    blue(pal)
}
pub fn out_unused(_: &Out) {}
