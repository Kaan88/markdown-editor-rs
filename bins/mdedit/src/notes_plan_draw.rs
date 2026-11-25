//! Drawing Write notes' plan card (`notes_plan_ui.rs`): the tree at the left, the note shown at
//! the right as it will read or as Markdown, the footer, and over them "Arrange again…" and the
//! row menu. Laid out as the pictures plan's `plan` drawing (section 5), from the shared tick,
//! switch, scrollbar, field and dialog footer.

use crate::notes_plan_ui::{
    self as ui, Bar, Block, CARD_H, CARD_W, HEAD_H, Hit, INDENT, LEFT_W, NotesPlan, Row, STRIP_H,
};
use crate::notes_task::Run;
use crate::task_ui::TaskScreen;
use mdshared_text::Bitmap;
use mdshared_ui::check::{Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_button_disabled, draw_dialog_card,
};
use mdshared_ui::scroll::{draw_scrollbar, scrollbar};
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::textfield::{FieldColors, TextField};
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;
/// The margin of the note at the right.
const PAGE_PAD: f32 = 28.0;
/// The largest a picture of the note is drawn, from its thumbnail.
const PIC_W: f32 = 200.0;
const PIC_H: f32 = 150.0;

/// The card in a `w` x `h` window.
pub fn card(w: f32, h: f32) -> Rect {
    dialog_card(CARD_W, CARD_H, w, h)
}

fn hline(p: &mut Painter, x: f32, y: f32, w: f32, c: [u8; 3]) {
    let px = p.px();
    p.fill(Rect::new(x, y, w, px), c);
}

/// `a` cut to `b`, for a hit that must not reach past the list it is in.
fn within(a: Rect, b: Rect) -> Rect {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0))
}

fn colors(pal: &Palette) -> FieldColors {
    FieldColors {
        text: pal.text,
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    }
}

pub fn draw(
    p: &mut Painter,
    pal: &Palette,
    c: &mut NotesPlan,
    s: &TaskScreen,
    w: f32,
    h: f32,
    caret_on: bool,
) {
    let Some(run) = s.notes_run.as_ref() else {
        return;
    };
    let r = card(w, h);
    let mut hits: Vec<(Hit, Rect)> = vec![(Hit::Card, r)];
    draw_backdrop(p, pal, w, h);
    let footer = draw_dialog_card(p, pal, r);
    let x = r.x + PAD;

    // ---- header ---------------------------------------------------------------------------------
    let title = if s.ai_on && run.requests > 0 {
        "Notes the AI proposes"
    } else {
        "Notes made here"
    };
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + 30.0,
        title,
        pal.text,
        r.w - 2.0 * PAD,
    );
    let mut from = ui::count(c.pictures.len(), "picture", "pictures");
    if !run.texts.is_empty() {
        from = format!(
            "{from} and {}",
            ui::count(run.texts.len(), "text file", "text files")
        );
    }
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + 50.0,
        &format!(
            "From {from}. Nothing is written yet: tick what to keep, F2 renames, right-click for \
             more."
        ),
        pal.muted,
        r.w - 2.0 * PAD,
    );
    let head_y = r.y + HEAD_H;
    hline(p, r.x + 1.0, head_y, r.w - 2.0, pal.popup_line);
    let body = Rect::new(r.x + 1.0, head_y + 1.0, r.w - 2.0, footer.y - head_y - 1.0);
    let left_w = (r.w * LEFT_W / CARD_W).round().clamp(180.0, LEFT_W);
    let left = Rect::new(body.x, body.y, left_w, body.h);
    p.fill(
        Rect::new(left.right(), body.y, p.px(), body.h),
        pal.popup_line,
    );
    draw_tree(p, pal, c, s, run, left, &mut hits, caret_on);
    let right = Rect::new(
        left.right() + 1.0,
        body.y,
        body.right() - left.right() - 1.0,
        body.h,
    );
    draw_note(p, pal, c, s, run, right, &mut hits);
    let again_at = draw_footer(p, pal, c, s, run, r, footer, &mut hits);
    if c.again.is_some() {
        draw_again(p, pal, c, r, footer, again_at, &mut hits, caret_on);
    }
    if let Some(m) = c.menu.as_mut() {
        m.menu.draw_in(p, pal, Rect::new(0.0, 0.0, w, h));
    }
    c.hits = hits;
}

/// Where a row's tick is, from the row's left: a note's by its depth, a picture's under its
/// note's, a picture in no note at the left.
fn tick_x(row: &Row) -> f32 {
    match row {
        Row::Note { depth, .. } => 34.0 + INDENT * *depth as f32,
        Row::Picture { key: None, .. } => 34.0,
        Row::Picture { depth, .. } | Row::Heading { depth, .. } => {
            68.0 + INDENT * depth.saturating_sub(1) as f32
        }
        _ => 34.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tree(
    p: &mut Painter,
    pal: &Palette,
    c: &mut NotesPlan,
    s: &TaskScreen,
    run: &Run,
    area: Rect,
    hits: &mut Vec<(Hit, Rect)>,
    caret_on: bool,
) {
    let rows = c.rows();
    c.tree_area = area;
    c.tree_content = rows.iter().map(Row::height).sum::<f32>() + 16.0;
    c.tree_scroll = c.tree_scroll.clamp(0.0, c.max_scroll(Bar::Tree));
    let bar = scrollbar(area, c.tree_content, area.h, c.tree_scroll);
    let old = p.clip(area);
    let mut y = area.y + 8.0 - c.tree_scroll;
    let (group, group_note) = ui::group_line(s);
    for (i, row) in rows.iter().enumerate() {
        let h = row.height();
        let at = Rect::new(area.x + 8.0, y, area.w - 16.0, h);
        y += h;
        if at.bottom() < area.y || at.y > area.bottom() {
            continue;
        }
        let sel = row.sel();
        if sel.is_some() {
            if sel == c.sel {
                p.rbox(at, 5.0, mix(pal.accent, pal.popup, 0.84));
            } else if c.hover == Some(Hit::Row(i)) || c.hover == Some(Hit::Tick(i)) {
                p.rbox(at, 5.0, pal.hover);
            }
            hits.push((Hit::Row(i), within(at, area)));
        }
        let editing = |c: &NotesPlan| {
            c.edit
                .as_ref()
                .is_some_and(|(e, _)| Some(e) == sel.as_ref())
        };
        match row {
            Row::Group => {
                p.icon(glyph::FOLDER_OPEN, 15.0, area.x + 30.0, at.cy(), pal.folder);
                let nw = p.text(
                    Face::Semibold,
                    13.5,
                    area.x + 46.0,
                    at.cy(),
                    &group,
                    pal.text,
                    area.w * 0.5,
                );
                p.text(
                    Face::Ui,
                    11.5,
                    area.x + 56.0 + nw,
                    at.cy(),
                    &group_note,
                    pal.faint,
                    at.right() - (area.x + 56.0 + nw) - 8.0,
                );
            }
            Row::Section { path } => {
                let off = INDENT * path.len().saturating_sub(1) as f32;
                p.icon(glyph::DOWN, 11.0, area.x + 46.0 + off, at.cy(), pal.muted);
                p.icon(
                    glyph::FOLDER,
                    14.0,
                    area.x + 64.0 + off,
                    at.cy(),
                    pal.folder,
                );
                let tx = area.x + 80.0 + off;
                if editing(c) {
                    draw_edit(p, pal, c, at, tx, caret_on);
                } else {
                    p.text(
                        Face::Semibold,
                        13.0,
                        tx,
                        at.cy(),
                        path.last().map_or("", String::as_str),
                        pal.text,
                        at.right() - tx - 10.0,
                    );
                }
            }
            Row::Note { key, on, .. } => {
                let Some(e) = c.entry(*key) else { continue };
                let tx = at.x + tick_x(row);
                let tick = Rect::new(tx, at.cy() - 8.0, 16.0, 16.0);
                draw_tick(p, pal, tick, Tick::new(*on));
                hits.push((Hit::Tick(i), within(tick.inset(-5.0), area)));
                p.icon(glyph::DOC, 14.0, tx + 32.0, at.cy(), pal.muted);
                let n = e.draft.pictures().len();
                let what = if *on {
                    ui::count(n, "picture", "pictures")
                } else {
                    "left out: not written".to_string()
                };
                let fw = p.measure(Face::Ui, 11.5, &what);
                if editing(c) {
                    draw_edit(p, pal, c, at, tx + 44.0, caret_on);
                } else {
                    let title = e.draft.title.clone();
                    p.text(
                        Face::Semibold,
                        13.0,
                        tx + 46.0,
                        at.cy(),
                        &title,
                        if *on { pal.text } else { pal.muted },
                        at.right() - tx - 46.0 - fw - 24.0,
                    );
                    p.text_right(Face::Ui, 11.5, at.right() - 10.0, at.cy(), &what, pal.muted);
                }
            }
            Row::Heading { text, .. } => {
                p.text(
                    Face::Semibold,
                    11.5,
                    at.x + tick_x(row) + 2.0,
                    at.cy(),
                    &format!("## {text}"),
                    pal.muted,
                    at.right() - at.x - tick_x(row) - 10.0,
                );
            }
            Row::Picture { id, key, on, .. } => {
                let enabled = key.is_none_or(|k| c.entry(k).is_some_and(|e| e.on));
                let tx = at.x + tick_x(row);
                let tick = Rect::new(tx, at.cy() - 8.0, 16.0, 16.0);
                draw_tick(
                    p,
                    pal,
                    tick,
                    Tick {
                        on: *on,
                        enabled,
                        focused: false,
                    },
                );
                if enabled {
                    hits.push((Hit::Tick(i), within(tick.inset(-5.0), area)));
                }
                let t = Rect::new(tx + 28.0, at.cy() - 13.0, 40.0, 26.0);
                draw_thumb(p, pal, t, s.files.get(*id).and_then(|f| f.thumb.as_ref()));
                let nx = t.right() + 10.0;
                let name = ui::copy_name(s, run, *id);
                if editing(c) {
                    draw_edit(
                        p,
                        pal,
                        c,
                        Rect::new(at.x, at.y, at.w, 26.0),
                        nx - 2.0,
                        caret_on,
                    );
                } else {
                    p.text(
                        Face::Ui,
                        12.5,
                        nx,
                        at.y + 12.0,
                        &name,
                        if *on { pal.text } else { pal.muted },
                        at.right() - nx - 10.0,
                    );
                }
                let says = match run.look(*id) {
                    Some(l) if !l.says.is_empty() => l.says.clone(),
                    Some(l) => l.title.clone(),
                    None => s
                        .files
                        .get(*id)
                        .map(|f| format!("{}, not looked at", f.name))
                        .unwrap_or_default(),
                };
                p.text(
                    Face::Ui,
                    11.0,
                    nx,
                    at.y + 28.0,
                    &says,
                    pal.muted,
                    at.right() - nx - 10.0,
                );
            }
            Row::Unused => {
                hline(p, area.x, at.y + 8.0, area.w, pal.popup_line);
                p.text(
                    Face::Semibold,
                    10.5,
                    area.x + PAD,
                    at.y + 26.0,
                    "NOT USED \u{00B7} IN NO NOTE, SO NOT COPIED \u{00B7} A TICK OR RIGHT-CLICK PUTS IT IN ONE",
                    pal.muted,
                    area.w - 2.0 * PAD,
                );
            }
        }
    }
    p.restore(old);
    if let Some((track, thumb)) = bar {
        let active = matches!(c.grab, Some((Bar::Tree, _))) || c.hover == Some(Hit::TreeTrack);
        draw_scrollbar(p, pal, track, thumb, active);
        hits.push((Hit::TreeTrack, track));
    }
    c.laid = rows;
}

/// The name being typed for a row, in a field from `x` to the row's right.
fn draw_edit(p: &mut Painter, pal: &Palette, c: &mut NotesPlan, at: Rect, x: f32, caret_on: bool) {
    let Some((_, field)) = c.edit.as_mut() else {
        return;
    };
    let r = Rect::new(x, at.cy() - 13.0, (at.right() - 8.0 - x).max(60.0), 26.0);
    p.frame(r, 5.0, pal.field, pal.accent);
    field.draw(
        p,
        Rect::new(r.x + 8.0, r.y, r.w - 16.0, r.h),
        Face::Ui,
        13.0,
        &colors(pal),
        "",
        true,
        caret_on,
    );
}

fn draw_note(
    p: &mut Painter,
    pal: &Palette,
    c: &mut NotesPlan,
    s: &TaskScreen,
    run: &Run,
    right: Rect,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let strip = Rect::new(right.x, right.y, right.w, STRIP_H);
    let labels = ["As it will read", "Markdown"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(
        strip.right() - PAD - sw_w,
        strip.cy() - SEGMENT_H / 2.0,
        sw_w,
        SEGMENT_H,
    );
    let hovered = match c.hover {
        Some(Hit::View(k)) => Some(k),
        _ => None,
    };
    let segs = draw_segments(p, pal, sw, &labels, Some(usize::from(c.markdown)), hovered);
    for (k, seg) in segs.into_iter().enumerate() {
        hits.push((Hit::View(k), seg));
    }
    let shown = c.shown_note();
    let entry = shown.and_then(|k| c.entry(k)).cloned();
    if let Some(e) = entry.as_ref() {
        p.text(
            Face::Mono,
            11.5,
            strip.x + 20.0,
            strip.cy(),
            &ui::note_path(&e.draft),
            pal.muted,
            sw.x - strip.x - 36.0,
        );
    }
    hline(p, right.x, strip.bottom(), right.w, pal.popup_line);
    let page = Rect::new(
        right.x,
        strip.bottom() + 1.0,
        right.w,
        right.bottom() - strip.bottom() - 1.0,
    );
    p.fill(page, pal.page);
    c.page_area = page;
    let Some(e) = entry else {
        p.text_center(
            Face::Ui,
            13.0,
            page,
            "No note is in the plan: Back, or Arrange again\u{2026}",
            pal.muted,
        );
        c.page_content = 0.0;
        return;
    };
    c.page_scroll = c.page_scroll.clamp(0.0, c.max_scroll(Bar::Page));
    let old = p.clip(page);
    let x = page.x + PAGE_PAD;
    let w = page.w - 2.0 * PAGE_PAD - 8.0;
    let top = page.y + PAGE_PAD - 6.0 - c.page_scroll;
    let mut y = top;
    if !e.on {
        let band = Rect::new(page.x, y - PAGE_PAD + 6.0, page.w, 30.0);
        p.fill(band, pal.warn_bg);
        p.text(
            Face::Ui,
            12.0,
            x,
            band.cy(),
            "Left out: this note is not written. Tick it to keep it.",
            pal.text,
            w,
        );
        y += 30.0;
    }
    if c.markdown {
        for line in ui::markdown(s, run, &e.draft).lines() {
            if line.is_empty() {
                y += 10.0;
            } else {
                y += p.paragraph(Face::Mono, 12.0, x, y, w, line, pal.text) + 2.0;
            }
        }
    } else {
        for b in ui::blocks(s, run, &e.draft) {
            y += draw_block(p, pal, s, run, &b, x, y, w);
        }
    }
    p.restore(old);
    c.page_content = y - top + PAGE_PAD;
    if let Some((track, thumb)) = scrollbar(page, c.page_content, page.h, c.page_scroll) {
        let active = matches!(c.grab, Some((Bar::Page, _))) || c.hover == Some(Hit::PageTrack);
        draw_scrollbar(p, pal, track, thumb, active);
        hits.push((Hit::PageTrack, track));
    }
}

/// One block of the note as it will read, at `y`; returns the height it took.
#[allow(clippy::too_many_arguments)]
fn draw_block(
    p: &mut Painter,
    pal: &Palette,
    s: &TaskScreen,
    run: &Run,
    b: &Block,
    x: f32,
    y: f32,
    w: f32,
) -> f32 {
    match b {
        Block::Heading(1, t) => p.paragraph(Face::Semibold, 20.0, x, y, w, t, pal.text) + 10.0,
        Block::Heading(2, t) => {
            6.0 + p.paragraph(Face::Semibold, 15.0, x, y + 6.0, w, t, pal.text) + 8.0
        }
        Block::Heading(_, t) => {
            4.0 + p.paragraph(Face::Semibold, 13.5, x, y + 4.0, w, t, pal.text) + 6.0
        }
        Block::Para(t) => p.paragraph(Face::Ui, 13.0, x, y, w, t, pal.text) + 10.0,
        Block::Quote(t) => {
            let h = p.paragraph(Face::Ui, 13.0, x + 14.0, y + 2.0, w - 14.0, t, pal.muted);
            p.fill(Rect::new(x, y, 3.0, h + 4.0), mix(pal.page, pal.text, 0.25));
            h + 14.0
        }
        Block::Code(t) => {
            let h = p.paragraph(Face::Mono, 12.0, x + 10.0, y + 8.0, w - 20.0, t, pal.text);
            // Drawn under the text after it is measured: the box is the field colour, so the
            // text stays on top by being drawn again over it.
            let r = Rect::new(x, y, w, h + 16.0);
            p.frame(r, 5.0, pal.field, pal.line);
            p.paragraph(Face::Mono, 12.0, x + 10.0, y + 8.0, w - 20.0, t, pal.text);
            h + 26.0
        }
        Block::Picture(id, words) => {
            let f = s.files.get(*id);
            let thumb = f
                .and_then(|f| f.thumb.as_ref())
                .filter(|t| t.1 > 0 && t.2 > 0);
            let (pw, ph) = match thumb {
                Some((_, tw, th)) => {
                    let fit = (PIC_W / *tw as f32).min(PIC_H / *th as f32);
                    ((*tw as f32 * fit).round(), (*th as f32 * fit).round())
                }
                None => (PIC_W, (PIC_W * 9.0 / 16.0).round()),
            };
            let pw = pw.min(w * 0.5).max(40.0);
            draw_thumb(p, pal, Rect::new(x, y, pw, ph), thumb);
            // Beside it, the line of the file: the address is the app's, the words the model's.
            let tx = x + pw + 16.0;
            let tw = (w - pw - 16.0).max(60.0);
            let address = if s.notes.copy {
                format!(
                    "{}/{}",
                    mdedit_core::notes_ai::PICTURES_DIR,
                    ui::copy_name(s, run, *id)
                )
            } else {
                format!(
                    "linked where it is: {}",
                    f.map(|f| f.name.as_str()).unwrap_or("")
                )
            };
            p.text(Face::Mono, 11.0, tx, y + 10.0, &address, pal.muted, tw);
            let th = p.paragraph(Face::Ui, 11.5, tx, y + 24.0, tw, words, pal.faint);
            ph.max(24.0 + th) + 12.0
        }
    }
}

/// The footer: what is written and where, and the buttons. Returns where "Arrange again…" is,
/// for its box to stand over.
#[allow(clippy::too_many_arguments)]
fn draw_footer(
    p: &mut Painter,
    pal: &Palette,
    c: &NotesPlan,
    s: &TaskScreen,
    run: &Run,
    r: Rect,
    footer: Rect,
    hits: &mut Vec<(Hit, Rect)>,
) -> Rect {
    let x = r.x + PAD;
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let blocked = ui::write_blocked(s, c);
    let write = format!("Write {}", ui::count(c.on_notes(), "note", "notes"));
    // (hit, label, primary, enabled)
    let mut buttons: Vec<(Hit, String, bool, bool)> = vec![
        (Hit::Discard, "Discard".to_string(), false, true),
        (Hit::Back, "Back".to_string(), false, true),
        (
            Hit::Again,
            "Arrange again\u{2026}".to_string(),
            false,
            s.ai_on,
        ),
        (Hit::Write, write, true, blocked.is_none()),
    ];
    // A narrow card keeps the buttons that cannot be had otherwise: Esc is Back.
    let room = r.w - 2.0 * PAD - 160.0;
    let width = |p: &mut Painter, b: &[(Hit, String, bool, bool)]| -> f32 {
        b.iter()
            .map(|(_, l, _, _)| dialog_button_w(p, l) + 10.0)
            .sum()
    };
    for drop in [Hit::Back, Hit::Discard] {
        if width(p, &buttons) > room {
            buttons.retain(|(h, _, _, _)| *h != drop);
        }
    }
    let mut bx = r.right() - PAD;
    let mut again_at = Rect::new(bx, by, 0.0, 32.0);
    for (hit, label, primary, enabled) in buttons.iter().rev() {
        let bw = dialog_button_w(p, label);
        bx -= bw;
        let b = Rect::new(bx, by, bw, 32.0);
        if *enabled {
            draw_dialog_button(
                p,
                pal,
                b,
                label,
                *primary,
                c.hover == Some(*hit),
                c.pressed == Some(*hit),
            );
            hits.push((*hit, b));
        } else {
            draw_dialog_button_disabled(p, pal, b, label, *primary);
        }
        if *hit == Hit::Again {
            again_at = b;
        }
        bx -= 10.0;
    }
    let room = bx - x - 8.0;
    let (line1, line2) = match blocked {
        Some(why) => (why, String::new()),
        None => {
            let (_, folder) = ui::group_line(s);
            let folder = folder
                .rsplit(" \u{00B7} ")
                .next()
                .unwrap_or(&folder)
                .to_string();
            (
                format!(
                    "{} and {} \u{2192} {folder}",
                    ui::count(c.on_notes(), "note", "notes"),
                    ui::count(c.on_pictures(), "picture", "pictures")
                ),
                String::new(),
            )
        }
    };
    let line2 = if !line2.is_empty() {
        line2
    } else if s.ai_on && run.requests > 0 {
        format!(
            "{} answered {}. Your pictures and files are not moved or changed.",
            s.via.split(" \u{00B7} ").next().unwrap_or(&s.via),
            ui::count(run.requests as usize, "request", "requests")
        )
    } else {
        "Made here, with no request. Your pictures and files are not moved or changed.".to_string()
    };
    p.text(
        Face::Semibold,
        12.0,
        x,
        footer.cy() - 8.0,
        &line1,
        pal.text,
        room,
    );
    p.text(
        Face::Ui,
        11.0,
        x,
        footer.cy() + 9.0,
        &line2,
        pal.muted,
        room,
    );
    again_at
}

/// "Arrange again…": the instruction, over the footer, above its button.
#[allow(clippy::too_many_arguments)]
fn draw_again(
    p: &mut Painter,
    pal: &Palette,
    c: &mut NotesPlan,
    r: Rect,
    footer: Rect,
    at: Rect,
    hits: &mut Vec<(Hit, Rect)>,
    caret_on: bool,
) {
    let bw = 480.0_f32.min(r.w - 2.0 * PAD);
    let bh = 156.0;
    let right = at.right().max(r.x + PAD + bw).min(r.right() - PAD);
    let b = Rect::new(right - bw, footer.y - 8.0 - bh, bw, bh);
    p.shadow(b, 8.0, pal.dark);
    p.frame(b, 8.0, pal.popup, pal.popup_line);
    hits.push((Hit::Card, b));
    let x = b.x + 16.0;
    let w = b.w - 32.0;
    p.text(
        Face::Semibold,
        13.5,
        x,
        b.y + 22.0,
        "Arrange again",
        pal.text,
        w,
    );
    p.paragraph(
        Face::Ui,
        11.5,
        x,
        b.y + 34.0,
        w,
        "What the look said and the text files go again with this instruction. No picture is \
         sent again.",
        pal.muted,
    );
    let field = Rect::new(x, b.y + 72.0, w, 30.0);
    p.frame(field, 6.0, pal.field, pal.accent);
    hits.push((Hit::AgainField, field));
    if let Some(f) = c.again.as_mut() {
        f.draw(
            p,
            Rect::new(field.x + 10.0, field.y, field.w - 20.0, field.h),
            Face::Ui,
            13.0,
            &colors(pal),
            "How the notes should be arranged",
            true,
            caret_on,
        );
    }
    let go = "Arrange again";
    let gw = dialog_button_w(p, go);
    let g = Rect::new(b.right() - 16.0 - gw, b.bottom() - 44.0, gw, 32.0);
    let empty = c.again.as_ref().is_none_or(TextField::is_empty);
    if empty {
        draw_dialog_button_disabled(p, pal, g, go, true);
    } else {
        draw_dialog_button(
            p,
            pal,
            g,
            go,
            true,
            c.hover == Some(Hit::AgainGo),
            c.pressed == Some(Hit::AgainGo),
        );
        hits.push((Hit::AgainGo, g));
    }
    let cw = dialog_button_w(p, "Cancel");
    let cb = Rect::new(g.x - 10.0 - cw, g.y, cw, 32.0);
    draw_dialog_button(
        p,
        pal,
        cb,
        "Cancel",
        false,
        c.hover == Some(Hit::AgainClose),
        c.pressed == Some(Hit::AgainClose),
    );
    hits.push((Hit::AgainClose, cb));
}

/// A thumbnail in its box, kept to its shape on the field's colour; an empty box when there is
/// none. Drawn through a bitmap of its own so the list's clip holds for it.
fn draw_thumb(p: &mut Painter, pal: &Palette, r: Rect, thumb: Option<&(Vec<u8>, u32, u32)>) {
    p.frame(r, 4.0, pal.field, pal.line);
    let Some((rgba, w, h)) = thumb.filter(|t| t.1 > 0 && t.2 > 0) else {
        p.icon(glyph::IMAGE, 14.0, r.cx(), r.cy(), pal.faint);
        return;
    };
    let (x0, y0, x1, y1) = p.dev_rect(r.inset(2.0));
    let (dw, dh) = (x1 - x0, y1 - y0);
    if dw <= 0 || dh <= 0 || rgba.len() < *w as usize * *h as usize * 4 {
        return;
    }
    let fit = (dw as f32 / *w as f32).min(dh as f32 / *h as f32);
    let (iw, ih) = (*w as f32 * fit, *h as f32 * fit);
    let mut bmp = Bitmap::new(dw as u32, dh as u32);
    bmp.clear(pal.field);
    bmp.image(
        rgba,
        *w,
        *h,
        (dw as f32 - iw) / 2.0,
        (dh as f32 - ih) / 2.0,
        iw,
        ih,
    );
    p.blit(&bmp, 0, dh, x0, y0);
}
