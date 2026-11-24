//! Three screens around a "Write notes" run: what is sent, with the pictures as they go; the
//! question before a run is undone; and the library afterwards, with a note open beside its
//! preview.

use super::plan::{NOTE, page};
use super::{Art, caption, footer_buttons, hline, thumb, vline};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{DIALOG_PAD, dialog_card, draw_dialog_card};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

/// name, on disk, as sent, what was done, the request it goes in.
const SENT: [(Art, &str, &str, &str, &str, &str); 5] = [
    (
        Art::Error,
        "Screenshot 2026-11-20 143022.png",
        "412 KB",
        "PNG \u{00B7} 1920 \u{00D7} 1080 \u{00B7} 412 KB",
        "as it is",
        "1",
    ),
    (
        Art::Error,
        "Screenshot 2026-11-20 143107.png",
        "388 KB",
        "PNG \u{00B7} 1920 \u{00D7} 1080 \u{00B7} 388 KB",
        "as it is",
        "1",
    ),
    (
        Art::Terminal,
        "Screenshot 2026-11-21 091455.png",
        "1.6 MB",
        "PNG \u{00B7} 1568 \u{00D7} 882 \u{00B7} 204 KB",
        "made smaller",
        "1",
    ),
    (
        Art::Settings,
        "settings-before.png",
        "310 KB",
        "PNG \u{00B7} 1568 \u{00D7} 882 \u{00B7} 141 KB",
        "made smaller",
        "1",
    ),
    (
        Art::Photo,
        "IMG_4032.jpg",
        "6.8 MB",
        "JPEG \u{00B7} 1568 \u{00D7} 1176 \u{00B7} 372 KB",
        "made smaller, details removed",
        "2",
    ),
];

const REQUEST: [&str; 13] = [
    "system",
    "  You look at pictures for a notes library. For each picture answer what it shows in one",
    "  or two sentences, a title of at most 8 words, and up to 3 subjects. Answer with one",
    "  JSON object and nothing else: {\"version\":1,\"pictures\":[{\"id\":0,\"title\":\"\u{2026}\",",
    "  \"says\":\"\u{2026}\",\"subjects\":[\"\u{2026}\"]}]}. Never write a path.",
    "prompt",
    "  The instruction: categorize and describe the screenshots",
    "  The pictures, in order:",
    "  0. Screenshot 2026-11-20 143022.png      [picture 0: 412,004 bytes follow]",
    "  1. Screenshot 2026-11-20 143107.png      [picture 1: 388,112 bytes follow]",
    "  2. Screenshot 2026-11-21 091455.png      [picture 2: 204,377 bytes follow]",
    "  3. settings-before.png                   [picture 3: 141,020 bytes follow]",
    "",
];

pub fn sent(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let r = dialog_card(980.0, 704.0, w, h);
    let footer = draw_dialog_card(p, pal, r);
    let x = r.x + PAD;
    let inner = r.w - 2.0 * PAD;
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 30.0,
        "What is sent",
        pal.text,
        inner,
    );
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + 50.0,
        "To Claude (the Claude API), model claude-sonnet-5-5, in 3 requests. Nothing has been sent yet.",
        pal.muted,
        inner,
    );

    // ---- the pictures -----------------------------------------------------------------------
    let mut y = r.y + 76.0;
    caption(
        p,
        pal,
        x,
        y + 8.0,
        "THE PICTURES, AS THEY ARE SENT \u{00B7} 5 \u{00B7} 1.5 MB (9.5 MB ON DISK)",
        inner,
    );
    y += 22.0;
    let table = Rect::new(x, y, inner, 26.0 + 5.0 * 36.0);
    p.frame(table, 8.0, pal.field, pal.popup_line);
    let cols = [
        table.x + 66.0,
        table.x + 356.0,
        table.x + 440.0,
        table.x + 660.0,
    ];
    let head_cy = table.y + 13.0;
    for (cx, label) in cols
        .iter()
        .zip(["FILE", "ON DISK", "AS SENT", "WHAT WAS DONE"])
    {
        p.text(Face::Semibold, 10.0, *cx, head_cy, label, pal.faint, 200.0);
    }
    p.text_right(
        Face::Semibold,
        10.0,
        table.right() - 14.0,
        head_cy,
        "REQUEST",
        pal.faint,
    );
    for (k, (art, name, disk, as_sent, done, req)) in SENT.iter().enumerate() {
        let row = Rect::new(table.x, table.y + 26.0 + k as f32 * 36.0, table.w, 36.0);
        hline(p, row.x + 1.0, row.y, row.w - 2.0, pal.popup_line);
        thumb(
            p,
            pal,
            Rect::new(row.x + 14.0, row.cy() - 13.0, 40.0, 26.0),
            *art,
        );
        p.text(
            Face::Ui,
            12.5,
            cols[0],
            row.cy(),
            name,
            pal.text,
            cols[1] - cols[0] - 12.0,
        );
        p.text(Face::Ui, 12.0, cols[1], row.cy(), disk, pal.muted, 80.0);
        p.text(
            Face::Ui,
            12.0,
            cols[2],
            row.cy(),
            as_sent,
            pal.text,
            cols[3] - cols[2] - 12.0,
        );
        p.text(
            Face::Ui,
            12.0,
            cols[3],
            row.cy(),
            done,
            pal.muted,
            table.right() - 60.0 - cols[3],
        );
        p.text_right(
            Face::Ui,
            12.0,
            table.right() - 14.0,
            row.cy(),
            req,
            pal.muted,
        );
    }

    // ---- the text ---------------------------------------------------------------------------
    y = table.bottom() + 18.0;
    caption(
        p,
        pal,
        x,
        y + 14.0,
        "THE TEXT, WORD FOR WORD \u{00B7} REQUEST 1 OF 3 \u{00B7} 4 PICTURES AND 0.9 KB",
        inner - 260.0,
    );
    let labels = ["1 \u{00B7} look", "2 \u{00B7} look", "3 \u{00B7} arrange"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(x + inner - sw_w, y, sw_w, SEGMENT_H);
    draw_segments(p, pal, sw, &labels, Some(0), None);
    y += SEGMENT_H + 8.0;
    let box_ = Rect::new(x, y, inner, footer.y - 16.0 - y);
    p.frame(box_, 8.0, pal.page, pal.popup_line);
    let old = p.clip(box_.inset(1.0));
    for (k, line) in REQUEST.iter().enumerate() {
        let cy = box_.y + 18.0 + k as f32 * 18.0;
        let head = !line.starts_with(' ') && !line.is_empty();
        p.text(
            if head { Face::Semibold } else { Face::Mono },
            if head { 11.0 } else { 11.5 },
            box_.x + 14.0,
            cy,
            line,
            if head { pal.muted } else { pal.text },
            box_.w - 28.0,
        );
    }
    p.restore(old);

    let left = footer_buttons(
        p,
        pal,
        r,
        footer,
        &[
            ("Copy the text", false, true),
            ("Open the text in a tab", false, true),
            ("Close", true, true),
        ],
    );
    let room = left - x - 14.0;
    p.text(
        Face::Semibold,
        12.0,
        x,
        footer.cy() - 8.0,
        "5 pictures (1.5 MB) and 4.6 KB of text, to api.anthropic.com",
        pal.text,
        room,
    );
    p.text(
        Face::Ui,
        11.0,
        x,
        footer.cy() + 9.0,
        "File names are sent; the folders they are in are not. Request 3 holds no picture.",
        pal.muted,
        room,
    );
}

pub fn undo(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let r = dialog_card(620.0, 344.0, w, h);
    let footer = draw_dialog_card(p, pal, r);
    let x = r.x + PAD;
    let inner = r.w - 2.0 * PAD;
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 32.0,
        "Undo this run?",
        pal.text,
        inner,
    );
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + 54.0,
        "Write notes \u{00B7} 5 pictures \u{2192} Screenshots \u{00B7} today 14:03",
        pal.muted,
        inner,
    );
    let lines: [(&str, &str, bool); 4] = [
        (
            glyph::DELETE,
            "Removes 1 note this run wrote and its line in Screenshots, and the 3 copies of \
             pictures it shows. They go to the Recycle Bin.",
            false,
        ),
        (
            glyph::CHECK,
            "Your 5 pictures are where they were, each still the file that was copied. A copy \
             whose original is gone is never removed.",
            false,
        ),
        (
            glyph::INFO,
            "Kept: Display settings, which you changed since, and the picture it shows.",
            true,
        ),
        (
            glyph::FOLDER,
            "Removes the folders the run made that are then empty: 2 of 4. The group stays, \
             because a note is left in it.",
            false,
        ),
    ];
    let mut y = r.y + 82.0;
    for (icon, text, warn) in lines {
        p.icon(
            icon,
            14.0,
            x + 8.0,
            y + 9.0,
            if warn { pal.warn } else { pal.muted },
        );
        let hgt = p.paragraph(Face::Ui, 13.0, x + 28.0, y, inner - 28.0, text, pal.text);
        y += hgt + 12.0;
    }
    footer_buttons(
        p,
        pal,
        r,
        footer,
        &[("Cancel", false, true), ("Undo the run", true, true)],
    );
}

const SOURCE: [&str; 21] = [
    "# Sign-in fails after the update",
    "",
    "Since the update of 20 November the sign-in window",
    "refuses a password that is right. Three screenshots",
    "show it, in the order they were taken.",
    "",
    "## The error",
    "",
    "![The sign-in window with a red error: \u{201c}Your session",
    "has expired\u{201d}](pictures/login-error.png)",
    "",
    "Pressing Sign in shows the red line at once; nothing",
    "is sent to the server first, so the session is judged",
    "expired on this PC.",
    "",
    "> Your session has expired. Sign in again to continue.",
    "",
    "## After Retry",
    "",
    "![The same window after Retry: the password box is",
    "empty again](pictures/login-after-retry.png)",
];

/// The library after the run: the Notes pane with the new group, the note as text, and the note
/// as it reads. Drawn over the page of the window, between the toolbar and the status strip.
pub fn result(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    let top = 85.0;
    let area = Rect::new(0.0, top, w, h - top - 26.0);
    p.fill(area, pal.page);

    // ---- the Notes pane ---------------------------------------------------------------------
    let pane = Rect::new(0.0, top, 290.0, area.h);
    p.fill(pane, pal.pane);
    vline(p, pane.right(), pane.y, pane.h, pal.line);
    let labels = ["Files", "Notes", "Functions"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(pane.x + 12.0, pane.y + 10.0, sw_w, SEGMENT_H);
    draw_segments(p, pal, sw, &labels, Some(1), None);
    // (indent, icon, text, bold, selected, open)
    let rows: [(f32, &str, &str, bool, bool); 10] = [
        (0.0, glyph::RIGHT, "Recipes", true, false),
        (0.0, glyph::DOWN, "Screenshots", true, false),
        (1.0, glyph::DOWN, "Sign-in", true, false),
        (
            2.0,
            glyph::DOC,
            "Sign-in fails after the update",
            false,
            true,
        ),
        (1.0, glyph::DOWN, "Settings", true, false),
        (2.0, glyph::DOC, "Display settings", false, false),
        (0.0, glyph::RIGHT, "Work", true, false),
        (0.0, "", "", false, false),
        (0.0, "", "", false, false),
        (0.0, "", "", false, false),
    ];
    let mut y = sw.bottom() + 12.0;
    for (indent, icon, text, bold, selected) in rows {
        if text.is_empty() {
            continue;
        }
        let row = Rect::new(pane.x + 6.0, y, pane.w - 12.0, 26.0);
        if selected {
            p.rbox(row, 5.0, pal.on);
        }
        let ix = row.x + 14.0 + indent * 16.0;
        p.icon(
            icon,
            if icon == glyph::DOC { 13.0 } else { 11.0 },
            ix,
            row.cy(),
            pal.muted,
        );
        p.text(
            if bold { Face::Semibold } else { Face::Ui },
            13.0,
            ix + 14.0,
            row.cy(),
            text,
            pal.text,
            row.right() - ix - 22.0,
        );
        y += 26.0;
    }
    // The pane's foot: the run that just finished, one row, as the plan's 5.10 has it.
    let foot = Rect::new(pane.x, pane.bottom() - 52.0, pane.w, 52.0);
    hline(p, foot.x, foot.y, foot.w, pal.line);
    p.text(
        Face::Ui,
        12.0,
        foot.x + 14.0,
        foot.y + 17.0,
        "Wrote 2 notes, copied 4 pictures.",
        pal.text,
        foot.w - 28.0,
    );
    let lw = p.text(
        Face::Ui,
        12.0,
        foot.x + 14.0,
        foot.y + 36.0,
        "Undo this run\u{2026}",
        mix(pal.accent, pal.text, 0.25),
        140.0,
    );
    p.text(
        Face::Ui,
        12.0,
        foot.x + 14.0 + lw + 16.0,
        foot.y + 36.0,
        "AI runs\u{2026}",
        mix(pal.accent, pal.text, 0.25),
        100.0,
    );

    // ---- the note as text -------------------------------------------------------------------
    let rest = area.w - pane.w - 1.0;
    let source = Rect::new(pane.right() + 1.0, top, (rest / 2.0).round(), area.h);
    let old = p.clip(source);
    for (k, line) in SOURCE.iter().enumerate() {
        let cy = source.y + 20.0 + k as f32 * 21.0;
        p.text_right(
            Face::Mono,
            11.5,
            source.x + 38.0,
            cy,
            &format!("{}", k + 1),
            pal.faint,
        );
        let colour = if line.starts_with('#') {
            pal.accent
        } else if line.starts_with('!') || line.contains("](pictures/") {
            mix(pal.accent, pal.text, 0.45)
        } else if line.starts_with('>') {
            pal.muted
        } else {
            pal.text
        };
        p.text(
            if line.starts_with('#') {
                Face::Semibold
            } else {
                Face::Mono
            },
            13.0,
            source.x + 54.0,
            cy,
            line,
            colour,
            source.w - 70.0,
        );
    }
    p.restore(old);
    vline(p, source.right(), source.y, source.h, pal.line);

    // ---- the note as it reads ---------------------------------------------------------------
    let view = Rect::new(
        source.right() + 1.0,
        top,
        area.right() - source.right() - 1.0,
        area.h,
    );
    let used = page(p, pal, view, &NOTE, 30.0, 190.0);
    if let Some((track, thumb_r)) = scrollbar(view, used + 200.0, view.h, 0.0) {
        draw_scrollbar(p, pal, track, thumb_r, false);
    }
}

const CARBONARA: [&str; 13] = [
    "# Carbonara",
    "",
    "For two. Twenty minutes.",
    "",
    "## Ingredients",
    "",
    "- 200 g spaghetti",
    "- 100 g guanciale, in strips",
    "- 3 egg yolks and 1 whole egg",
    "- 50 g pecorino, grated fine",
    "- black pepper, a lot",
    "",
    "## Method",
];

/// Several notes selected in the Notes pane: a whole section by one Ctrl+click on its row, two
/// more notes one at a time, and the menu of the selection. Drawn over the page of the window.
pub fn tree_select(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    tree(p, pal, w, h, true);
}

/// The main way in from the library: one row, here a section, and its own menu with the AI task
/// on all the notes under it. Nothing is selected; the row is the one under the pointer.
pub fn tree_row(p: &mut Painter, pal: &Palette, w: f32, h: f32) {
    tree(p, pal, w, h, false);
}

fn tree(p: &mut Painter, pal: &Palette, w: f32, h: f32, multi: bool) {
    let top = 85.0;
    let area = Rect::new(0.0, top, w, h - top - 26.0);
    p.fill(area, pal.page);
    let pane = Rect::new(0.0, top, 300.0, area.h);
    p.fill(pane, pal.pane);
    vline(p, pane.right(), pane.y, pane.h, pal.line);
    let labels = ["Files", "Notes", "Functions"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(pane.x + 12.0, pane.y + 10.0, sw_w, SEGMENT_H);
    draw_segments(p, pal, sw, &labels, Some(1), None);

    // (indent, icon, text, bold, selected, the keyboard's row, the count a selected section shows)
    let rows: [(f32, &str, &str, bool, bool, bool, &str); 13] = [
        (0.0, glyph::DOWN, "Recipes", true, false, false, ""),
        (1.0, glyph::DOWN, "Pasta", true, true, false, "all 4"),
        (2.0, glyph::DOC, "Carbonara", false, true, false, ""),
        (2.0, glyph::DOC, "Cacio e pepe", false, true, false, ""),
        (2.0, glyph::DOC, "Lasagne", false, true, false, ""),
        (2.0, glyph::DOC, "Rag\u{00F9}", false, true, false, ""),
        (1.0, glyph::DOWN, "Bread", true, false, false, ""),
        (2.0, glyph::DOC, "Sourdough starter", false, true, false, ""),
        (2.0, glyph::DOC, "Focaccia", false, false, false, ""),
        (2.0, glyph::DOC, "Rye loaf", false, true, true, ""),
        (1.0, glyph::RIGHT, "Soups", true, false, false, ""),
        (0.0, glyph::RIGHT, "Travel", true, false, false, ""),
        (0.0, glyph::RIGHT, "Work", true, false, false, ""),
    ];
    let tint = mix(pal.accent, pal.pane, 0.80);
    let mut y = sw.bottom() + 12.0;
    let mut menu_at = (0.0, 0.0);
    for (indent, icon, text, bold, selected, cursor, count) in rows {
        // One row only: the section under the pointer, and no selection anywhere.
        let (selected, cursor, count) = if multi {
            (selected, cursor, count)
        } else {
            (false, text == "Pasta", "")
        };
        let row = Rect::new(pane.x + 6.0, y, pane.w - 12.0, 26.0);
        if selected {
            p.rbox(Rect::new(row.x, row.y + 1.0, row.w, row.h - 2.0), 5.0, tint);
        }
        if cursor {
            p.frame(
                Rect::new(row.x, row.y + 1.0, row.w, row.h - 2.0),
                5.0,
                if multi { tint } else { pal.hover },
                if multi { pal.accent } else { pal.hover },
            );
            menu_at = (row.x + 150.0, row.bottom() - 4.0);
        }
        let ix = row.x + 14.0 + indent * 16.0;
        p.icon(
            icon,
            if icon == glyph::DOC { 13.0 } else { 11.0 },
            ix,
            row.cy(),
            pal.muted,
        );
        p.text(
            if bold { Face::Semibold } else { Face::Ui },
            13.0,
            ix + 14.0,
            row.cy(),
            text,
            pal.text,
            row.right() - ix - 70.0,
        );
        if !count.is_empty() {
            p.text_right(
                Face::Ui,
                11.5,
                row.right() - 10.0,
                row.cy(),
                count,
                pal.muted,
            );
        }
        y += 26.0;
    }

    if multi {
        // The pane's foot while more than one note is selected.
        let foot = Rect::new(pane.x, pane.bottom() - 52.0, pane.w, 52.0);
        hline(p, foot.x, foot.y, foot.w, pal.line);
        p.text(
            Face::Semibold,
            12.0,
            foot.x + 14.0,
            foot.y + 17.0,
            "6 notes selected",
            pal.text,
            foot.w - 28.0,
        );
        let blue = mix(pal.accent, pal.text, 0.25);
        let lw = p.text(
            Face::Ui,
            12.0,
            foot.x + 14.0,
            foot.y + 36.0,
            "AI task on them\u{2026}",
            blue,
            160.0,
        );
        p.text(
            Face::Ui,
            12.0,
            foot.x + 14.0 + lw + 16.0,
            foot.y + 36.0,
            "Clear (Esc)",
            blue,
            100.0,
        );
    }

    // ---- the document that was open: selecting opens nothing --------------------------------
    let source = Rect::new(pane.right() + 1.0, top, area.w - pane.w - 1.0, area.h);
    let old = p.clip(source);
    for (k, line) in CARBONARA.iter().enumerate() {
        let cy = source.y + 20.0 + k as f32 * 21.0;
        p.text_right(
            Face::Mono,
            11.5,
            source.x + 38.0,
            cy,
            &format!("{}", k + 1),
            pal.faint,
        );
        let head = line.starts_with('#');
        p.text(
            if head { Face::Semibold } else { Face::Mono },
            13.0,
            source.x + 54.0,
            cy,
            line,
            if head { pal.accent } else { pal.text },
            source.w - 70.0,
        );
    }
    p.restore(old);

    // ---- the menu of the selection ----------------------------------------------------------
    let items: [(&str, &str, bool); 5] = if multi {
        [
            ("AI task on these 6 notes\u{2026}", "", false),
            ("Move the notes\u{2026}", "", false),
            ("Copy their paths", "", true),
            ("Remove 6 from the library\u{2026}", "", false),
            ("Clear the selection", "Esc", false),
        ]
    } else {
        // A section's menu as it is today, with one line added at its head.
        [
            ("AI task on these 4 notes\u{2026}", "", true),
            ("New note here\u{2026}", "", false),
            ("New subsection\u{2026}", "", false),
            ("Rename", "F2", true),
            ("Remove this section\u{2026}", "", false),
        ]
    };
    let mw = 290.0;
    let seps = items.iter().filter(|i| i.2).count() as f32;
    let mh = 30.0 * items.len() as f32 + 8.0 + 9.0 * seps;
    let m = Rect::new(menu_at.0, menu_at.1, mw, mh);
    p.shadow(m, 8.0, pal.dark);
    p.frame(m, 8.0, pal.popup, pal.popup_line);
    let mut my = m.y + 4.0;
    for (k, (label, key, sep_after)) in items.iter().enumerate() {
        let row = Rect::new(m.x + 4.0, my, m.w - 8.0, 30.0);
        if k == 0 {
            p.rbox(row, 5.0, pal.hover);
        }
        p.text(
            Face::Ui,
            13.0,
            row.x + 12.0,
            row.cy(),
            label,
            pal.text,
            220.0,
        );
        p.text_right(Face::Ui, 12.0, row.right() - 12.0, row.cy(), key, pal.faint);
        my += 30.0;
        if *sep_after {
            hline(p, m.x + 1.0, my + 4.0, m.w - 2.0, pal.popup_line);
            my += 9.0;
        }
    }
    let _ = (thumb, Art::Photo);
}
