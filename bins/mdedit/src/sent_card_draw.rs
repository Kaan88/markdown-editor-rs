//! Drawing "What is sent" (`sent_card.rs`): the tabs, a look's pictures as they go, the
//! request's text word for word with its selection, and the footer with "Open the text in a
//! tab" and Close.

use crate::notes_task::Measure;
use crate::sent_card::{
    ARRANGE_HEAD, ARRANGE_LINE, CARD_H, CARD_W, CardHit, LINE_H, ROW_H, SentCard, TEXT_PAD,
    TEXT_SIZE,
};
use crate::task_ui::{FileEntry, size_text};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_card,
};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

/// The card for a `w` x `h` window: a short one when no request would go.
pub fn card(c: &SentCard, w: f32, h: f32) -> Rect {
    if c.tabs.is_empty() {
        dialog_card(720.0, 260.0, w, h)
    } else {
        dialog_card(CARD_W, CARD_H, w, h)
    }
}

/// What a picture is sent as: "JPEG · 1568 × 1176 · 372 KB", and what was done to it.
pub fn sent_as(m: &Measure) -> (String, String) {
    match m {
        Measure::Waiting => ("measuring\u{2026}".to_string(), String::new()),
        Measure::Refused(why) => (String::new(), why.clone()),
        Measure::Ready(info) => {
            let done = if !info.done.is_empty() {
                info.done.join(", ")
            } else if info.smaller() {
                "smaller".to_string()
            } else {
                "as it is".to_string()
            };
            (
                format!(
                    "{} \u{00B7} {} \u{00D7} {} \u{00B7} {}",
                    info.format.to_uppercase(),
                    info.width,
                    info.height,
                    size_text(info.bytes)
                ),
                done,
            )
        }
    }
}

pub fn draw(p: &mut Painter, pal: &Palette, c: &mut SentCard, files: &[FileEntry], w: f32, h: f32) {
    draw_backdrop(p, pal, w, h);
    let r = card(c, w, h);
    let mut hits: Vec<(CardHit, Rect)> = vec![(CardHit::Card, r)];
    let footer = draw_dialog_card(p, pal, r);
    let (hover, pressed) = (c.hover, c.pressed);
    let x = r.x + PAD;
    let inner = r.w - 2.0 * PAD;

    // ---- header --------------------------------------------------------------------------------
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 30.0,
        "What is sent",
        pal.text,
        inner,
    );
    let n = c.tabs.len();
    let under = if c.off {
        "The AI is off: nothing is sent. Write notes makes the notes here.".to_string()
    } else if n == 0 {
        "No file takes part: nothing would be sent.".to_string()
    } else {
        format!(
            "To {}, in {n} request{}. Nothing has been sent by this card.",
            c.via,
            if n == 1 { "" } else { "s" }
        )
    };
    p.text(Face::Ui, 12.5, x, r.y + 50.0, &under, pal.muted, inner);

    // ---- the tabs, one per request ---------------------------------------------------------------
    let ty = r.y + 70.0;
    if n > 0 {
        let labels: Vec<&str> = c.tabs.iter().map(|t| t.label.as_str()).collect();
        let sw = switch_width(p, &labels);
        let facts_w = 300.0;
        let tabs_right = if n <= 4 && sw <= inner - facts_w {
            let hovered = match hover {
                Some(CardHit::Tab(k)) => Some(k),
                _ => None,
            };
            let rects = draw_segments(
                p,
                pal,
                Rect::new(x, ty, sw, SEGMENT_H),
                &labels,
                Some(c.tab),
                hovered,
            );
            for (k, seg) in rects.into_iter().enumerate() {
                hits.push((CardHit::Tab(k), seg));
            }
            x + sw
        } else {
            // Too many requests for a switch: one at a time, with the arrows.
            let prev = Rect::new(x, ty, SEGMENT_H, SEGMENT_H);
            let label = format!(
                "Request {} of {n} \u{00B7} {}",
                c.tab + 1,
                c.tabs[c.tab].label
            );
            let lw = p.measure(Face::Semibold, 13.0, &label) + 28.0;
            let mid = Rect::new(prev.right() + 6.0, ty, lw, SEGMENT_H);
            let next = Rect::new(mid.right() + 6.0, ty, SEGMENT_H, SEGMENT_H);
            for (hit, b, arrow, on) in [
                (CardHit::Prev, prev, "\u{2039}", c.tab > 0),
                (CardHit::Next, next, "\u{203A}", c.tab + 1 < n),
            ] {
                let fill = if on && hover == Some(hit) {
                    pal.hover
                } else {
                    pal.popup
                };
                p.frame(b, 6.0, fill, pal.popup_line);
                p.text_center(
                    Face::Semibold,
                    15.0,
                    b,
                    arrow,
                    if on { pal.text } else { pal.faint },
                );
                if on {
                    hits.push((hit, b));
                }
            }
            p.text_center(Face::Semibold, 13.0, mid, &label, pal.text);
            next.right()
        };
        // What this request holds, at the right of the row.
        let t = &c.tabs[c.tab];
        let text_kb = format!("{} of text", size_text(c.text.len() as u64));
        let facts = if t.arrange {
            format!("no picture \u{00B7} {text_kb}")
        } else {
            let waiting = t
                .pictures
                .iter()
                .any(|&i| files.get(i).is_some_and(|f| f.measure == Measure::Waiting));
            format!(
                "{} picture{} \u{00B7} {} \u{00B7} {text_kb}",
                t.pictures.len(),
                if t.pictures.len() == 1 { "" } else { "s" },
                if waiting {
                    "measuring".to_string()
                } else {
                    format!("{} as sent", size_text(c.bytes(files)))
                }
            )
        };
        let room = x + inner - tabs_right - 16.0;
        if room > 40.0 {
            let facts = p.elide(Face::Ui, 12.0, &facts, room);
            p.text_right(
                Face::Ui,
                12.0,
                x + inner,
                ty + SEGMENT_H / 2.0,
                &facts,
                pal.muted,
            );
        }
    }

    // ---- the body: the pictures, then the text; one scroll ---------------------------------------
    let top = if n > 0 { ty + SEGMENT_H + 12.0 } else { ty };
    let body = Rect::new(r.x + 1.0, top, r.w - 2.0, (footer.y - 8.0 - top).max(0.0));
    c.body = body;
    c.scroll = c.scroll.clamp(0.0, c.max_scroll());
    let old = p.clip(body);
    let origin = body.y - c.scroll;
    let mut y = origin + 4.0;
    if n == 0 {
        let lines: &[&str] = if c.off {
            &[
                "Nothing leaves this computer while the AI is off: no picture, no name, no text.",
                "Choose a connection in Settings \u{203A} AI to have the pictures looked at.",
            ]
        } else {
            &["Tick a picture or a text file in the window to see what would be sent."]
        };
        for line in lines {
            p.text(Face::Ui, 13.0, x, y + 12.0, line, pal.text, inner);
            y += 24.0;
        }
        c.lines.clear();
        c.content = y - origin + 8.0;
    } else {
        let tab = c.tabs[c.tab].clone();
        if tab.arrange {
            p.icon(glyph::INFO, 14.0, x + 7.0, y + 12.0, pal.accent);
            p.text(
                Face::Semibold,
                13.0,
                x + 22.0,
                y + 12.0,
                ARRANGE_HEAD,
                pal.text,
                inner - 22.0,
            );
            y += 24.0;
            y += p.paragraph(
                Face::Ui,
                12.5,
                x + 22.0,
                y,
                inner - 22.0,
                ARRANGE_LINE,
                pal.muted,
            );
            y += 14.0;
        } else {
            caption(
                p,
                pal,
                x,
                y + 8.0,
                "THE PICTURES, AS THEY ARE SENT, IN THIS ORDER",
                inner,
            );
            y += 22.0;
            let table = Rect::new(x, y, inner, 24.0 + tab.pictures.len() as f32 * ROW_H);
            p.frame(table, 8.0, pal.field, pal.popup_line);
            let cols = [table.x + 64.0, table.x + 400.0, table.x + 640.0];
            let head_cy = table.y + 13.0;
            for (cx, label) in cols.iter().zip(["FILE", "AS SENT", "WHAT WAS DONE"]) {
                p.text(Face::Semibold, 10.0, *cx, head_cy, label, pal.faint, 200.0);
            }
            p.text_right(
                Face::Semibold,
                10.0,
                table.right() - 14.0,
                head_cy,
                "ON DISK",
                pal.faint,
            );
            for (k, &i) in tab.pictures.iter().enumerate() {
                let Some(f) = files.get(i) else { continue };
                let row = Rect::new(table.x, table.y + 24.0 + k as f32 * ROW_H, table.w, ROW_H);
                p.fill(
                    Rect::new(row.x + 1.0, row.y, row.w - 2.0, p.px()),
                    pal.popup_line,
                );
                let thumb = Rect::new(row.x + 14.0, row.cy() - 18.0, 36.0, 36.0);
                crate::task_draw::draw_thumb(p, pal, thumb, f.thumb.as_ref());
                p.text(
                    Face::Ui,
                    12.5,
                    cols[0],
                    row.cy(),
                    &f.name,
                    pal.text,
                    cols[1] - cols[0] - 16.0,
                );
                let (as_sent, done) = sent_as(&f.measure);
                p.text(
                    Face::Ui,
                    12.0,
                    cols[1],
                    row.cy(),
                    &as_sent,
                    if f.measure == Measure::Waiting {
                        pal.faint
                    } else {
                        pal.text
                    },
                    cols[2] - cols[1] - 16.0,
                );
                p.text(
                    Face::Ui,
                    12.0,
                    cols[2],
                    row.cy(),
                    &done,
                    pal.muted,
                    table.right() - 100.0 - cols[2],
                );
                p.text_right(
                    Face::Ui,
                    12.0,
                    table.right() - 14.0,
                    row.cy(),
                    &size_text(f.size),
                    pal.muted,
                );
            }
            y = table.bottom() + 20.0;
        }
        caption(
            p,
            pal,
            x,
            y + 8.0,
            "THE TEXT OF THE REQUEST, WORD FOR WORD",
            inner,
        );
        y += 22.0;
        // The lines, wrapped to the box; the scrollbar keeps the body's right edge.
        let tx = x + TEXT_PAD;
        let tw = (inner - 2.0 * TEXT_PAD).max(40.0);
        let mut buf = [0u8; 4];
        let mut measure = |ch: char| p.measure(Face::Mono, TEXT_SIZE, ch.encode_utf8(&mut buf));
        c.wrap(tw, &mut measure);
        let box_h = c.lines.len() as f32 * LINE_H + 2.0 * TEXT_PAD;
        let text_box = Rect::new(x, y, inner, box_h);
        p.frame(text_box, 8.0, pal.page, pal.popup_line);
        c.text_top = y + TEXT_PAD - origin;
        c.text_x = tx;
        let shown_top = text_box.y.max(body.y);
        let shown_bottom = text_box.bottom().min(body.bottom());
        if shown_bottom > shown_top {
            hits.push((
                CardHit::Text,
                Rect::new(text_box.x, shown_top, text_box.w, shown_bottom - shown_top),
            ));
        }
        let first_y = y + TEXT_PAD;
        let first = ((body.y - first_y) / LINE_H).floor().max(0.0) as usize;
        let sel = c.selection();
        let mut k = first;
        while k < c.lines.len() {
            let ly = first_y + k as f32 * LINE_H;
            if ly > body.bottom() {
                break;
            }
            let (a, b) = c.lines[k];
            if let Some((lo, hi)) = sel
                && hi > a
                && lo <= b
            {
                let x0 = tx + c.width_of(a, lo.max(a));
                // The line break is selected too when the selection goes on past it.
                let past = if hi > b && k + 1 < c.lines.len() {
                    4.0
                } else {
                    0.0
                };
                let x1 = tx + c.width_of(a, hi.min(b)) + past;
                if x1 > x0 {
                    p.fill(Rect::new(x0, ly, x1 - x0, LINE_H), pal.selection);
                }
            }
            let line = c.text.get(a..b).unwrap_or("");
            let label = (line == "SYSTEM:" || line == "PROMPT:")
                && (a == 0 || c.text.as_bytes().get(a - 1) == Some(&b'\n'));
            p.text(
                if label { Face::Semibold } else { Face::Mono },
                if label { 11.0 } else { TEXT_SIZE },
                tx,
                ly + LINE_H / 2.0,
                line,
                if label { pal.muted } else { pal.text },
                tw + 8.0,
            );
            k += 1;
        }
        c.content = text_box.bottom() + 12.0 - origin;
    }
    p.restore(old);
    c.scroll = c.scroll.clamp(0.0, c.max_scroll());
    if let Some((track, thumb)) = scrollbar(body, c.content, body.h, c.scroll) {
        draw_scrollbar(
            p,
            pal,
            track,
            thumb,
            hover == Some(CardHit::Track) || c.grab.is_some(),
        );
        hits.push((CardHit::Track, track));
    }

    // ---- footer ------------------------------------------------------------------------------------
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut bx = r.right() - PAD;
    // With no request there is no text to open in a tab.
    let buttons: &[(CardHit, &str, bool)] = if n == 0 {
        &[(CardHit::Close, "Close", true)]
    } else {
        &[
            (CardHit::Close, "Close", true),
            (CardHit::OpenTab, "Open the text in a tab", false),
        ]
    };
    for &(hit, label, primary) in buttons {
        let bw = dialog_button_w(p, label);
        bx -= bw;
        let b = Rect::new(bx, by, bw, 32.0);
        draw_dialog_button(
            p,
            pal,
            b,
            label,
            primary,
            hover == Some(hit),
            pressed == Some(hit),
        );
        hits.push((hit, b));
        bx -= 10.0;
    }
    let room = (bx - x - 12.0).max(0.0);
    let (first, second) = if n == 0 {
        (
            if c.off {
                "Nothing is sent: the AI is off.".to_string()
            } else {
                "Nothing would be sent.".to_string()
            },
            "Esc goes back to the window.".to_string(),
        )
    } else {
        (
            c.summary.clone(),
            "File names are sent; the folders they are in are not. Select text, then Ctrl+C to \
             copy it."
                .to_string(),
        )
    };
    p.text(
        Face::Semibold,
        12.0,
        x,
        footer.cy() - 8.0,
        &first,
        pal.text,
        room,
    );
    p.text(
        Face::Ui,
        11.0,
        x,
        footer.cy() + 9.0,
        &second,
        pal.muted,
        room,
    );
    c.hits = hits;
}

/// A small heading in the dim colour, as the window's are.
fn caption(p: &mut Painter, pal: &Palette, x: f32, cy: f32, text: &str, max: f32) {
    p.text(
        Face::Semibold,
        11.0,
        x,
        cy,
        text,
        mix(pal.faint, pal.muted, 0.5),
        max,
    );
}
