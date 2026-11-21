//! Drawing the AI panels: "Rewrite with instruction" and the review of a result (the changes,
//! or the text before and after drawn by mdview's renderer). They are drawn over everything, as
//! the palette is, and keep their own clickable places in [`Ai::hits`].

use crate::ai::{Ai, Hit, Instruction, Panel, Review};
use mdedit_core::ai::{ReviewLine, SavedPrompt};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_card,
};
use mdshared_ui::textfield::FieldColors;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;
/// A saved prompt's row in the instruction panel.
const PROMPT_ROW: f32 = 46.0;
/// The room the Modify button takes at the right of a saved prompt's row.
const PENCIL: f32 = 104.0;
/// A line of the changes.
pub const LINE_H: f32 = 20.0;
const HEADER_H: f32 = 64.0;

/// The colours of removed and added lines, and their marks.
fn tints(pal: &Palette) -> ([u8; 3], [u8; 3], [u8; 3], [u8; 3]) {
    if pal.dark {
        (
            [0x4a, 0x23, 0x23],
            [0x1f, 0x3d, 0x29],
            [0xf0, 0x8a, 0x80],
            [0x7f, 0xd4, 0x97],
        )
    } else {
        (
            [0xfd, 0xec, 0xea],
            [0xe6, 0xf4, 0xea],
            [0xb4, 0x23, 0x18],
            [0x1a, 0x7f, 0x37],
        )
    }
}

pub fn draw(
    p: &mut Painter,
    pal: &Palette,
    ai: &mut Ai,
    prompts: &[SavedPrompt],
    w: f32,
    h: f32,
    caret_on: bool,
) {
    let mut hits = Vec::new();
    let (hover, pressed) = (ai.hover, ai.pressed);
    match ai.panel.as_mut() {
        Some(Panel::Instruction(i)) => {
            draw_backdrop(p, pal, w, h);
            instruction(
                p, pal, i, prompts, w, h, caret_on, hover, pressed, &mut hits,
            )
        }
        Some(Panel::Review(r)) => {
            draw_backdrop(p, pal, w, h);
            review(p, pal, r, w, h, hover, pressed, &mut hits)
        }
        None => {}
    }
    ai.hits = hits;
}

#[allow(clippy::too_many_arguments)]
fn button(
    p: &mut Painter,
    pal: &Palette,
    hits: &mut Vec<(Hit, Rect)>,
    r: Rect,
    hit: Hit,
    label: &str,
    primary: bool,
    hover: Option<Hit>,
    pressed: Option<Hit>,
) {
    draw_dialog_button(
        p,
        pal,
        r,
        label,
        primary,
        hover == Some(hit),
        pressed == Some(hit),
    );
    hits.push((hit, r));
}

/// The Modify button (a pencil and the word) of a saved prompt's `row` in the instruction panel.
pub fn pencil_rect(row: Rect) -> Rect {
    Rect::new(
        row.right() - 12.0 - MODIFY_W,
        row.cy() - 14.0,
        MODIFY_W,
        28.0,
    )
}

/// The Modify button's width.
pub const MODIFY_W: f32 = 88.0;
/// The AI menu's narrower Modify button: it fits in the room the menu's rows keep at their
/// right (`overlays::MENU_MODIFY_ROOM`), so the menu is no wider for it.
pub const MENU_MODIFY_W: f32 = 64.0;
/// What it says.
pub const MODIFY: &str = "Modify";

/// A saved prompt's Modify button in `r`: a framed button with a pencil and the word, so it
/// reads as a control of its own on a row that runs the prompt. The rewrite panel's rows and
/// the AI menu's (drawn over the shared menu, [`draw_menu_modify`]) both have it.
/// `compact` for the AI menu's narrower button ([`MENU_MODIFY_W`]): the pencil and the word
/// closer together.
pub fn draw_modify(
    p: &mut Painter,
    pal: &Palette,
    r: Rect,
    hovered: bool,
    pressed: bool,
    compact: bool,
) {
    let fill = if hovered && pressed {
        pal.press
    } else if hovered {
        mix(pal.accent, pal.popup, 0.82)
    } else {
        mix(pal.popup, pal.text, 0.06)
    };
    let line = if hovered { pal.accent } else { pal.popup_line };
    p.frame(r, 5.0, fill, line);
    let (icon, text) = if compact { (12.0, 21.0) } else { (17.0, 29.0) };
    p.icon(glyph::EDIT, 13.0, r.x + icon, r.cy(), pal.text);
    p.text(
        Face::Ui,
        12.5,
        r.x + text,
        r.cy(),
        MODIFY,
        pal.text,
        r.w - text - 4.0,
    );
}

/// The Modify button of a saved prompt's `row` in the AI menu.
pub fn menu_modify_rect(row: Rect) -> Rect {
    Rect::new(
        row.right() - 4.0 - MENU_MODIFY_W,
        row.y + 3.0,
        MENU_MODIFY_W,
        row.h - 6.0,
    )
}

/// The AI menu's row whose Modify button is under `at`, if any.
pub fn menu_modify_at(
    menu: &mdshared_ui::menu::Menu<crate::ui::Act>,
    at: (f32, f32),
) -> Option<usize> {
    (0..menu.items.len()).find(|&i| {
        let item = &menu.items[i];
        matches!(item.id, crate::ui::Act::AiPrompt(_))
            && item.enabled
            && menu_modify_rect(menu.row_rect(i)).contains(at.0, at.1)
    })
}

/// Draws the Modify buttons over the AI menu's saved prompts (the shared menu draws a row with
/// one label and one hint): hovered when the pointer `mouse` is on one.
pub fn draw_menu_modify(
    p: &mut Painter,
    pal: &Palette,
    menu: &mdshared_ui::menu::Menu<crate::ui::Act>,
    mouse: (f32, f32),
) {
    let old = p.clip(menu.rect());
    for (i, item) in menu.items.iter().enumerate() {
        if !matches!(item.id, crate::ui::Act::AiPrompt(_)) || !item.enabled {
            continue;
        }
        let r = menu_modify_rect(menu.row_rect(i));
        draw_modify(p, pal, r, r.contains(mouse.0, mouse.1), false, true);
    }
    p.restore(old);
}

/// The instruction panel's card in a `w` x `h` window.
pub fn instruction_card(prompts: usize, w: f32, h: f32) -> Rect {
    let list = if prompts == 0 {
        44.0
    } else {
        28.0 + (prompts as f32).min(6.0) * PROMPT_ROW
    };
    dialog_card(620.0, PAD + 52.0 + 52.0 + list + 16.0 + DIALOG_FOOTER, w, h)
}

#[allow(clippy::too_many_arguments)]
fn instruction(
    p: &mut Painter,
    pal: &Palette,
    i: &mut Instruction,
    prompts: &[SavedPrompt],
    w: f32,
    h: f32,
    caret_on: bool,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let r = instruction_card(prompts.len(), w, h);
    hits.push((Hit::Card, r));
    let footer = draw_dialog_card(p, pal, r);
    let x = r.x + PAD;
    let inner = r.w - 2.0 * PAD;
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + PAD + 10.0,
        "Rewrite with instruction",
        pal.text,
        inner,
    );
    p.text(
        Face::Ui,
        12.5,
        x,
        r.y + PAD + 34.0,
        &i.scope,
        if i.changed { pal.text } else { pal.muted },
        inner,
    );
    // The field.
    let field = Rect::new(x, r.y + PAD + 52.0, inner, 36.0);
    p.frame(field, 6.0, pal.field, pal.accent);
    hits.push((Hit::Field, field));
    let colors = FieldColors {
        text: pal.text,
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    };
    i.field.draw(
        p,
        Rect::new(field.x + 12.0, field.y, field.w - 24.0, field.h),
        Face::Ui,
        13.5,
        &colors,
        "What should change? For example: make the prices a table",
        true,
        caret_on,
    );
    // The saved prompts.
    let mut y = field.bottom() + 20.0;
    p.text(
        Face::Semibold,
        11.0,
        x,
        y,
        "SAVED PROMPTS",
        pal.muted,
        inner,
    );
    y += 14.0;
    let list = Rect::new(r.x + 8.0, y, r.w - 16.0, footer.y - y - 8.0);
    if prompts.is_empty() {
        p.text(
            Face::Ui,
            12.5,
            x,
            y + 16.0,
            "None yet. Settings › AI › Saved prompts › Edit adds some.",
            pal.faint,
            inner,
        );
    } else {
        let old = p.clip(list);
        let content = prompts.len() as f32 * PROMPT_ROW;
        i.scroll = i.scroll.clamp(0.0, (content - list.h).max(0.0));
        if let Some(s) = i.selected {
            let top = s as f32 * PROMPT_ROW;
            if top < i.scroll {
                i.scroll = top;
            } else if top + PROMPT_ROW > i.scroll + list.h {
                i.scroll = top + PROMPT_ROW - list.h;
            }
        }
        for (k, sp) in prompts.iter().enumerate() {
            let row = Rect::new(
                list.x,
                list.y + k as f32 * PROMPT_ROW - i.scroll,
                list.w,
                PROMPT_ROW,
            );
            if row.bottom() < list.y || row.y > list.bottom() {
                continue;
            }
            let on = i.selected == Some(k)
                || hover == Some(Hit::Prompt(k))
                || hover == Some(Hit::EditPrompt(k));
            if on {
                p.rbox(row.inset(2.0), 6.0, pal.hover);
            }
            p.text(
                Face::Semibold,
                13.0,
                row.x + 16.0,
                row.y + 15.0,
                &sp.name,
                pal.text,
                row.w - 32.0 - PENCIL,
            );
            p.text(
                Face::Ui,
                12.0,
                row.x + 16.0,
                row.y + 32.0,
                &sp.summary(),
                pal.muted,
                row.w - 32.0 - PENCIL,
            );
            hits.push((Hit::Prompt(k), row));
            // Modify, a button of its own: the prompt goes into the box above to be changed, and
            // does not run (the rest of the row runs it).
            let pencil = pencil_rect(row);
            let edit = Hit::EditPrompt(k);
            draw_modify(
                p,
                pal,
                pencil,
                hover == Some(edit),
                pressed == Some(edit),
                false,
            );
            hits.push((edit, pencil));
        }
        p.restore(old);
    }
    // The footer: where the text goes, then the buttons.
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let run_w = dialog_button_w(p, "Rewrite");
    let run = Rect::new(r.right() - PAD - run_w, by, run_w, 32.0);
    let cancel_w = dialog_button_w(p, "Cancel");
    let cancel = Rect::new(run.x - 8.0 - cancel_w, by, cancel_w, 32.0);
    let save_w = dialog_button_w(p, "Save prompt");
    let save = Rect::new(cancel.x - 8.0 - save_w, by, save_w, 32.0);
    p.text(
        Face::Ui,
        12.0,
        x,
        footer.cy(),
        &i.via,
        pal.muted,
        save.x - x - 12.0,
    );
    // Only when there is something to save.
    if !i.field.text().trim().is_empty() {
        button(
            p,
            pal,
            hits,
            save,
            Hit::SavePrompt,
            "Save prompt",
            false,
            hover,
            pressed,
        );
    }
    button(
        p,
        pal,
        hits,
        cancel,
        Hit::Cancel,
        "Cancel",
        false,
        hover,
        pressed,
    );
    button(
        p,
        pal,
        hits,
        run,
        Hit::Rewrite,
        "Rewrite",
        true,
        hover,
        pressed,
    );
}

/// The review's card in a `w` x `h` window.
pub fn review_card(w: f32, h: f32) -> Rect {
    dialog_card(1040.0, 688.0, w, h)
}

/// The area under the review's header and above its footer.
pub fn review_body(r: Rect) -> Rect {
    Rect::new(
        r.x + 1.0,
        r.y + HEADER_H,
        r.w - 2.0,
        r.h - HEADER_H - DIALOG_FOOTER,
    )
}

#[allow(clippy::too_many_arguments)]
fn review(
    p: &mut Painter,
    pal: &Palette,
    rv: &mut Review,
    w: f32,
    h: f32,
    hover: Option<Hit>,
    pressed: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let r = review_card(w, h);
    hits.push((Hit::Card, r));
    let footer = draw_dialog_card(p, pal, r);
    let (_, _, minus, plus) = tints(pal);
    let px = p.px();
    // Header: title, what and how, the switch, the counts.
    let x = r.x + PAD;
    let cy = r.y + 30.0;
    let tw = p.measure(Face::Semibold, 17.0, &rv.spec.title).min(360.0);
    p.text(Face::Semibold, 17.0, x, cy, &rv.spec.title, pal.text, 360.0);
    let counts_r = format!("+ {} lines", rv.added);
    let counts_l = format!("− {} lines", rv.removed);
    let mut right = r.right() - PAD;
    if !rv.preview {
        let wr = p.measure(Face::Semibold, 13.0, &counts_r);
        p.text_right(Face::Semibold, 13.0, right, cy, &counts_r, plus);
        right -= wr + 14.0;
        let wl = p.measure(Face::Semibold, 13.0, &counts_l);
        p.text_right(Face::Semibold, 13.0, right, cy, &counts_l, minus);
        right -= wl + 18.0;
    }
    // The switch.
    let seg_w = |p: &mut Painter, s: &str| p.measure(Face::Semibold, 13.0, s) + 28.0;
    let (cw, pw) = (seg_w(p, "Changes"), seg_w(p, "Preview"));
    let sw = Rect::new(right - cw - pw, cy - 14.0, cw + pw, 28.0);
    p.frame(sw, 6.0, pal.popup, pal.popup_line);
    for (k, (label, hit, on)) in [
        ("Changes", Hit::Changes, !rv.preview),
        ("Preview", Hit::Preview, rv.preview),
    ]
    .into_iter()
    .enumerate()
    {
        let seg = if k == 0 {
            Rect::new(sw.x, sw.y, cw, sw.h)
        } else {
            Rect::new(sw.x + cw, sw.y, pw, sw.h)
        };
        if on {
            p.rbox(seg.inset(1.0), 5.0, pal.accent);
            p.text_center(Face::Semibold, 13.0, seg, label, pal.on_accent);
        } else {
            if hover == Some(hit) {
                p.rbox(seg.inset(1.0), 5.0, pal.hover);
            }
            p.text_center(Face::Ui, 13.0, seg, label, pal.text);
        }
        hits.push((hit, seg));
    }
    let sub_x = x + tw + 16.0;
    p.text(
        Face::Ui,
        12.5,
        sub_x,
        cy + 1.0,
        &rv.subtitle,
        pal.muted,
        (sw.x - 16.0 - sub_x).max(0.0),
    );
    p.fill(
        Rect::new(r.x + 1.0, r.y + HEADER_H - px, r.w - 2.0, px),
        pal.popup_line,
    );

    let mut body = review_body(r);
    if let Some(why) = &rv.blocked {
        let note = Rect::new(
            body.x + PAD - 1.0,
            body.y + 12.0,
            body.w - 2.0 * PAD + 2.0,
            40.0,
        );
        p.rbox(note, 6.0, pal.warn_bg);
        p.text(
            Face::Ui,
            12.5,
            note.x + 14.0,
            note.cy(),
            why,
            pal.text,
            note.w - 28.0,
        );
        body = Rect::new(
            body.x,
            note.bottom() + 6.0,
            body.w,
            body.bottom() - note.bottom() - 6.0,
        );
    }
    hits.push((Hit::Body, body));
    if rv.preview {
        previews(p, pal, rv, body);
    } else {
        changes(p, pal, rv, body);
    }

    // Footer.
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut bx = r.right() - PAD;
    let primary = if rv.blocked.is_some() {
        ("Copy result", Hit::Copy)
    } else {
        ("Apply", Hit::Apply)
    };
    // A closed document cannot be run again: Copy result and Discard only.
    let closed = rv.blocked.as_deref() == Some(mdedit_core::ai::BLOCKED_CLOSED);
    let second = if rv.blocked.is_some() {
        ("Run again", Hit::RunAgain)
    } else {
        ("Copy result", Hit::Copy)
    };
    let buttons: Vec<(&str, Hit)> = if closed {
        vec![primary, ("Discard", Hit::Discard)]
    } else {
        vec![primary, second, ("Discard", Hit::Discard)]
    };
    for (k, (label, hit)) in buttons.into_iter().enumerate() {
        let bw = dialog_button_w(p, label);
        bx -= bw;
        let b = Rect::new(bx, by, bw, 32.0);
        button(p, pal, hits, b, hit, label, k == 0, hover, pressed);
        bx -= 10.0;
    }
    let note = if rv.preview {
        "Drawn by mdview's renderer. The two sides scroll together."
    } else if rv.blocked.is_some() {
        ""
    } else {
        "Apply is one step: Ctrl+Z brings the old text back."
    };
    p.text(
        Face::Ui,
        12.0,
        x,
        footer.cy(),
        note,
        pal.muted,
        bx - x - 12.0,
    );
}

/// The changes: removed and added lines with their numbers, unchanged runs folded.
fn changes(p: &mut Painter, pal: &Palette, rv: &mut Review, body: Rect) {
    let (minus_bg, plus_bg, minus, plus) = tints(pal);
    let content = rv.lines.len() as f32 * LINE_H + 16.0;
    rv.content = content;
    rv.scroll = rv.scroll.clamp(0.0, (content - body.h).max(0.0));
    let old = p.clip(body);
    let num_w = 44.0;
    let text_x = body.x + 2.0 * num_w + 28.0;
    let first = (rv.scroll / LINE_H).floor().max(0.0) as usize;
    for (k, line) in rv.lines.iter().enumerate().skip(first) {
        let y = body.y + 8.0 + k as f32 * LINE_H - rv.scroll;
        if y > body.bottom() {
            break;
        }
        let row = Rect::new(body.x, y, body.w, LINE_H);
        let cy = row.cy();
        let num = |p: &mut Painter, col: usize, n: usize| {
            p.text_right(
                Face::Ui,
                11.5,
                body.x + (col + 1) as f32 * num_w - 4.0,
                cy,
                &n.to_string(),
                pal.faint,
            );
        };
        match line {
            ReviewLine::Same {
                old: a,
                new: b,
                text,
            } => {
                num(p, 0, *a);
                num(p, 1, *b);
                p.text(
                    Face::Ui,
                    13.0,
                    text_x,
                    cy,
                    text,
                    pal.text,
                    body.right() - text_x - 12.0,
                );
            }
            ReviewLine::Removed { old: a, text } => {
                p.fill(row, minus_bg);
                num(p, 0, *a);
                p.text_center(
                    Face::Semibold,
                    13.0,
                    Rect::new(body.x + 2.0 * num_w + 2.0, y, 24.0, LINE_H),
                    "−",
                    minus,
                );
                p.text(
                    Face::Ui,
                    13.0,
                    text_x,
                    cy,
                    text,
                    pal.text,
                    body.right() - text_x - 12.0,
                );
            }
            ReviewLine::Added { new: b, text } => {
                p.fill(row, plus_bg);
                num(p, 1, *b);
                p.text_center(
                    Face::Semibold,
                    13.0,
                    Rect::new(body.x + 2.0 * num_w + 2.0, y, 24.0, LINE_H),
                    "+",
                    plus,
                );
                p.text(
                    Face::Ui,
                    13.0,
                    text_x,
                    cy,
                    text,
                    pal.text,
                    body.right() - text_x - 12.0,
                );
            }
            ReviewLine::Fold { lines } => {
                p.fill(
                    Rect::new(row.x, row.y + 2.0, row.w, row.h - 4.0),
                    mix(pal.popup, pal.text, 0.05),
                );
                let s = if *lines == 1 { "" } else { "s" };
                p.text(
                    Face::Ui,
                    12.0,
                    text_x,
                    cy,
                    &format!("··· {lines} unchanged line{s}"),
                    pal.muted,
                    body.right() - text_x - 12.0,
                );
            }
        }
    }
    p.restore(old);
}

/// The text before and after, side by side, as mdview's renderer drew them.
fn previews(p: &mut Painter, pal: &Palette, rv: &mut Review, body: Rect) {
    let half = (body.w / 2.0).floor();
    let label_h = 30.0;
    let (_, plus_bg, _, plus) = tints(pal);
    let px = p.px();
    for (k, (label, fg, bg)) in [
        ("BEFORE", pal.muted, mix(pal.popup, pal.text, 0.04)),
        ("AFTER", plus, plus_bg),
    ]
    .into_iter()
    .enumerate()
    {
        let col = Rect::new(body.x + k as f32 * half, body.y, half, body.h);
        let head = Rect::new(col.x, col.y, col.w, label_h);
        p.fill(head, bg);
        p.text(
            Face::Semibold,
            11.0,
            col.x + PAD,
            head.cy(),
            label,
            fg,
            col.w,
        );
        let page = Rect::new(col.x, head.bottom(), col.w, col.h - label_h);
        p.fill(page, pal.page);
        let Some(panes) = rv.panes.as_mut() else {
            continue;
        };
        let pane = if k == 0 { &mut panes.0 } else { &mut panes.1 };
        let scale = 1.0 / px;
        pane.set_view(
            (page.w * scale).round() as u32,
            (page.h * scale).round() as u32,
            scale,
            pal.dark,
        );
        if let Some(why) = &pane.ended {
            p.text(
                Face::Ui,
                13.0,
                page.x + 16.0,
                page.y + 20.0,
                why,
                pal.muted,
                page.w - 32.0,
            );
        } else if pane.frame.width > 1 {
            let old = p.clip(page);
            let (x0, y0, _, _) = p.dev_rect(page);
            p.blit(&pane.frame, 0, pane.frame.height as i32, x0, y0);
            p.restore(old);
        }
    }
    p.fill(Rect::new(body.x + half, body.y, px, body.h), pal.popup_line);
}
