//! The review of what "Write notes" would write: at the left the notes as a tree, each with the
//! pictures it holds; at the right the selected note as it will read. Two answers are drawn: the
//! model made a note per subject (`plan`), and the owner asked for one note (`plan-one`).

use super::{Art, caption, footer_buttons, hline, link, thumb, tick, vline};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{DIALOG_PAD, dialog_card, draw_dialog_card};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;
const CARD_W: f32 = 1120.0;
const CARD_H: f32 = 720.0;
const LEFT_W: f32 = 530.0;

/// A row of the tree.
enum Row {
    /// The group the notes go to.
    Group(&'static str, &'static str),
    /// A section: a heading in the group's file and a folder on disk.
    Section(&'static str),
    /// A note: its title, what it is made from, whether it is the one shown at the right.
    Note(&'static str, &'static str, bool),
    /// A heading inside a note (the one-note answer), shown so the pictures read in order.
    Heading(&'static str),
    /// A picture in the note above: the copy's name, the description, ticked.
    Picture(Art, &'static str, &'static str, bool),
    /// The line over what the model used nowhere.
    Unused,
}

/// A block of the note shown at the right.
pub(super) enum Block {
    H1(&'static str),
    H2(&'static str),
    P(&'static str),
    /// A picture: what it shows, the copy's name, the text in its brackets.
    Pic(Art, &'static str, &'static str),
    /// Text read out of a picture, as a quote.
    Quote(&'static str),
}

const D_ERROR: &str =
    "The sign-in window with a red error: \u{201c}Your session has expired\u{201d}";
const D_RETRY: &str = "The same window after Retry: the password box is empty again";
const D_BUILD: &str = "A terminal: cargo build stops at error E0308 in src/auth.rs";
const D_DISPLAY: &str = "The Display page of Settings: Night light on, scale at 125%";
const D_PHOTO: &str = "A photo of hills under a clear sky. It fits none of the notes.";

const TREE: [Row; 11] = [
    Row::Group("Screenshots", "a new group \u{00B7} library\\Screenshots\\"),
    Row::Section("Sign-in"),
    Row::Note(
        "Sign-in fails after the update",
        "3 pictures \u{00B7} bug-notes.txt",
        true,
    ),
    Row::Picture(Art::Error, "login-error.png", D_ERROR, true),
    Row::Picture(Art::Error, "login-after-retry.png", D_RETRY, true),
    Row::Picture(Art::Terminal, "build-error-e0308.png", D_BUILD, true),
    Row::Section("Settings"),
    Row::Note("Display settings", "1 picture", false),
    Row::Picture(Art::Settings, "display-night-light.png", D_DISPLAY, true),
    Row::Unused,
    Row::Picture(Art::Photo, "IMG_4032.jpg", D_PHOTO, false),
];

const TREE_ONE: [Row; 11] = [
    Row::Group("Screenshots", "a new group \u{00B7} library\\Screenshots\\"),
    Row::Note(
        "Screenshots, sorted and described",
        "4 pictures \u{00B7} bug-notes.txt",
        true,
    ),
    Row::Heading("Sign-in errors"),
    Row::Picture(Art::Error, "login-error.png", D_ERROR, true),
    Row::Picture(Art::Error, "login-after-retry.png", D_RETRY, true),
    Row::Heading("Build errors"),
    Row::Picture(Art::Terminal, "build-error-e0308.png", D_BUILD, true),
    Row::Heading("Settings"),
    Row::Picture(Art::Settings, "display-night-light.png", D_DISPLAY, true),
    Row::Unused,
    Row::Picture(Art::Photo, "IMG_4032.jpg", D_PHOTO, false),
];

pub(super) const NOTE: [Block; 9] = [
    Block::H1("Sign-in fails after the update"),
    Block::P(
        "Since the update of 20 November the sign-in window refuses a password that is right. \
         Three screenshots show it, in the order they were taken.",
    ),
    Block::H2("The error"),
    Block::Pic(Art::Error, "login-error.png", D_ERROR),
    Block::P(
        "Pressing Sign in shows the red line at once; nothing is sent to the server first, so \
         the session is judged expired on this PC.",
    ),
    Block::Quote("Your session has expired. Sign in again to continue."),
    Block::H2("After Retry"),
    Block::Pic(Art::Error, "login-after-retry.png", D_RETRY),
    Block::P("Retry clears the password box and shows the same line again."),
];

const NOTE_ONE: [Block; 8] = [
    Block::H1("Screenshots, sorted and described"),
    Block::P("Four screenshots in three kinds. A fifth picture, a photograph, is left out."),
    Block::H2("Sign-in errors"),
    Block::Pic(Art::Error, "login-error.png", D_ERROR),
    Block::P(
        "The sign-in window of the app, with a red line under the password box. It appears as \
         soon as Sign in is pressed.",
    ),
    Block::Pic(Art::Error, "login-after-retry.png", D_RETRY),
    Block::P("The same window a moment later: Retry has emptied the box, and the line is back."),
    Block::H2("Build errors"),
];

pub fn draw(p: &mut Painter, pal: &Palette, w: f32, h: f32, one: bool) {
    let r = dialog_card(CARD_W, CARD_H, w, h);
    let footer = draw_dialog_card(p, pal, r);
    let x = r.x + PAD;

    // ---- header -----------------------------------------------------------------------------
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 30.0,
        "Notes the AI proposes",
        pal.text,
        600.0,
    );
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + 50.0,
        "From 5 pictures and 1 text file. Nothing is written yet: tick what to keep, F2 renames.",
        pal.muted,
        700.0,
    );
    let head_y = r.y + 64.0;
    hline(p, r.x + 1.0, head_y, r.w - 2.0, pal.popup_line);
    let body = Rect::new(r.x + 1.0, head_y + 1.0, r.w - 2.0, footer.y - head_y - 1.0);
    let left = Rect::new(body.x, body.y, LEFT_W, body.h);
    vline(p, left.right(), body.y, body.h, pal.popup_line);

    // ---- the tree ---------------------------------------------------------------------------
    let rows: &[Row] = if one { &TREE_ONE } else { &TREE };
    let old = p.clip(left);
    let mut y = left.y + 8.0;
    for row in rows {
        match row {
            Row::Group(name, note) => {
                let cy = y + 16.0;
                p.icon(glyph::FOLDER_OPEN, 15.0, left.x + 30.0, cy, pal.folder);
                let nw = p.text(
                    Face::Semibold,
                    13.5,
                    left.x + 46.0,
                    cy,
                    name,
                    pal.text,
                    200.0,
                );
                p.text(
                    Face::Ui,
                    11.5,
                    left.x + 46.0 + nw + 10.0,
                    cy,
                    note,
                    pal.faint,
                    left.w - nw - 80.0,
                );
                y += 32.0;
            }
            Row::Section(name) => {
                let cy = y + 14.0;
                p.icon(glyph::DOWN, 11.0, left.x + 46.0, cy, pal.muted);
                p.icon(glyph::FOLDER, 14.0, left.x + 64.0, cy, pal.folder);
                p.text(
                    Face::Semibold,
                    13.0,
                    left.x + 80.0,
                    cy,
                    name,
                    pal.text,
                    300.0,
                );
                y += 28.0;
            }
            Row::Note(title, from, selected) => {
                let at = Rect::new(left.x + 8.0, y + 1.0, left.w - 16.0, 30.0);
                if *selected {
                    p.rbox(at, 5.0, mix(pal.accent, pal.popup, 0.84));
                }
                let indent = if one { 34.0 } else { 54.0 };
                tick(
                    p,
                    pal,
                    Rect::new(at.x + indent, at.cy() - 8.0, 16.0, 16.0),
                    true,
                    true,
                );
                p.icon(glyph::DOC, 14.0, at.x + indent + 32.0, at.cy(), pal.muted);
                let fw = p.measure(Face::Ui, 11.5, from);
                p.text(
                    Face::Semibold,
                    13.0,
                    at.x + indent + 46.0,
                    at.cy(),
                    title,
                    pal.text,
                    at.w - indent - 46.0 - fw - 24.0,
                );
                p.text_right(Face::Ui, 11.5, at.right() - 10.0, at.cy(), from, pal.muted);
                y += 32.0;
            }
            Row::Heading(name) => {
                let cy = y + 13.0;
                p.text(
                    Face::Semibold,
                    11.5,
                    left.x + 96.0,
                    cy,
                    &format!("## {name}"),
                    pal.muted,
                    300.0,
                );
                y += 26.0;
            }
            Row::Picture(art, name, says, on) => {
                let at = Rect::new(left.x + 8.0, y + 1.0, left.w - 16.0, 36.0);
                let indent = if !*on {
                    34.0
                } else if one {
                    70.0
                } else {
                    88.0
                };
                tick(
                    p,
                    pal,
                    Rect::new(at.x + indent, at.cy() - 8.0, 16.0, 16.0),
                    *on,
                    true,
                );
                let t = Rect::new(at.x + indent + 28.0, at.cy() - 13.0, 40.0, 26.0);
                thumb(p, pal, t, *art);
                let tx = t.right() + 10.0;
                p.text(
                    Face::Ui,
                    12.5,
                    tx,
                    at.y + 11.0,
                    name,
                    if *on { pal.text } else { pal.muted },
                    at.right() - tx - 10.0,
                );
                p.text(
                    Face::Ui,
                    11.0,
                    tx,
                    at.y + 25.0,
                    says,
                    pal.muted,
                    at.right() - tx - 10.0,
                );
                y += 38.0;
            }
            Row::Unused => {
                y += 8.0;
                hline(p, left.x, y, left.w, pal.popup_line);
                caption(
                    p,
                    pal,
                    left.x + PAD,
                    y + 17.0,
                    "NOT USED \u{00B7} IN NO NOTE, SO NOT COPIED \u{00B7} RIGHT-CLICK PUTS IT IN ONE",
                    left.w - 2.0 * PAD,
                );
                y += 32.0;
            }
        }
    }
    p.restore(old);

    // ---- the note ---------------------------------------------------------------------------
    let right = Rect::new(
        left.right() + 1.0,
        body.y,
        body.right() - left.right() - 1.0,
        body.h,
    );
    let strip = Rect::new(right.x, right.y, right.w, 44.0);
    let labels = ["As it will read", "Markdown"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(
        strip.right() - PAD - sw_w,
        strip.cy() - SEGMENT_H / 2.0,
        sw_w,
        SEGMENT_H,
    );
    draw_segments(p, pal, sw, &labels, Some(0), None);
    p.text(
        Face::Mono,
        11.5,
        strip.x + 20.0,
        strip.cy(),
        if one {
            "screenshots-sorted-and-described.md"
        } else {
            "Sign-in\\sign-in-fails-after-the-update.md"
        },
        pal.muted,
        sw.x - strip.x - 36.0,
    );
    hline(p, right.x, strip.bottom(), right.w, pal.popup_line);
    let page_area = Rect::new(
        right.x,
        strip.bottom() + 1.0,
        right.w,
        right.bottom() - strip.bottom() - 1.0,
    );
    p.fill(page_area, pal.page);
    let blocks: &[Block] = if one { &NOTE_ONE } else { &NOTE };
    let used = page(p, pal, page_area, blocks, 28.0, 200.0);
    if let Some((track, thumb_r)) = scrollbar(page_area, used + 260.0, page_area.h, 0.0) {
        draw_scrollbar(p, pal, track, thumb_r, false);
    }

    // ---- footer -----------------------------------------------------------------------------
    let primary = if one { "Write 1 note" } else { "Write 2 notes" };
    let leftmost = footer_buttons(
        p,
        pal,
        r,
        footer,
        &[
            ("Back", false, true),
            ("Arrange again\u{2026}", false, true),
            (primary, true, true),
        ],
    );
    let link_x = link(p, pal, leftmost - 16.0, footer.cy(), "Show what was sent");
    let room = link_x - x - 14.0;
    p.text(
        Face::Semibold,
        12.0,
        x,
        footer.cy() - 8.0,
        if one {
            "1 note and 4 pictures (2.7 MB copied) \u{2192} library\\Screenshots\\"
        } else {
            "2 notes and 4 pictures (2.7 MB copied) \u{2192} library\\Screenshots\\"
        },
        pal.text,
        room,
    );
    p.text(
        Face::Ui,
        11.0,
        x,
        footer.cy() + 9.0,
        "Claude answered 3 requests. Your pictures and bug-notes.txt are not moved or changed.",
        pal.muted,
        room,
    );
}

/// A note as it will read, from the top of `area`, cut off at its bottom. `pad` is the margin and
/// `pic_w` a picture's width. Returns the height the blocks took.
pub(super) fn page(
    p: &mut Painter,
    pal: &Palette,
    area: Rect,
    blocks: &[Block],
    pad: f32,
    pic_w: f32,
) -> f32 {
    let old = p.clip(area);
    let x = area.x + pad;
    let w = area.w - 2.0 * pad - 8.0;
    let mut y = area.y + pad - 6.0;
    for b in blocks {
        match b {
            Block::H1(t) => {
                y += p.paragraph(Face::Semibold, 20.0, x, y, w, t, pal.text) + 10.0;
            }
            Block::H2(t) => {
                y += 6.0;
                y += p.paragraph(Face::Semibold, 15.0, x, y, w, t, pal.text) + 8.0;
            }
            Block::P(t) => {
                y += p.paragraph(Face::Ui, 13.0, x, y, w, t, pal.text) + 10.0;
            }
            Block::Pic(art, name, alt) => {
                let ph = (pic_w * 9.0 / 16.0).round();
                thumb(p, pal, Rect::new(x, y, pic_w, ph), *art);
                // Beside the picture: what the line in the file is, so the owner sees the link
                // is the program's and the words in the brackets are the model's.
                let tx = x + pic_w + 16.0;
                let tw = (w - pic_w - 16.0).max(60.0);
                p.text(
                    Face::Mono,
                    11.0,
                    tx,
                    y + 10.0,
                    &format!("pictures/{name}"),
                    pal.muted,
                    tw,
                );
                p.paragraph(Face::Ui, 11.5, tx, y + 24.0, tw, alt, pal.faint);
                y += ph + 12.0;
            }
            Block::Quote(t) => {
                let h = p.paragraph(Face::Ui, 13.0, x + 14.0, y + 2.0, w - 14.0, t, pal.muted);
                p.fill(Rect::new(x, y, 3.0, h + 4.0), mix(pal.page, pal.text, 0.25));
                y += h + 14.0;
            }
        }
    }
    p.restore(old);
    y - area.y
}
