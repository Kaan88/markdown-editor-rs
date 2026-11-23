//! Drawing the AI panels: "Rewrite with instruction" and the review of a result (the changes,
//! or the text before and after drawn by mdview's renderer). They are drawn over everything, as
//! the palette is, and keep their own clickable places in [`Ai::hits`].

use crate::ai::{Ai, Hit, Instruction, Panel, Review};
use crate::hunks_ui;
use mdedit_core::ai::SavedPrompt;
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::glyph;
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, dialog_card, draw_backdrop, draw_dialog_button,
    draw_dialog_button_disabled, draw_dialog_card,
};
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::textfield::FieldColors;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;
/// A saved prompt's row in the instruction panel.
const PROMPT_ROW: f32 = 46.0;
/// The room the Modify button takes at the right of a saved prompt's row.
const PENCIL: f32 = 104.0;
const HEADER_H: f32 = 64.0;
/// The bar under the header: the changes' arrows, where they are, Tick all.
const BAR_H: f32 = 46.0;

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
    // The same panel asks for a split of a plan's document (5.2): its own words, its own prompts.
    let split = i.split.is_some();
    let (title, placeholder, run_label) = if split {
        (
            "Split into notes by AI",
            "How should it be cut? For example: one note per chapter",
            "Split",
        )
    } else {
        (
            "Rewrite with instruction",
            "What should change? For example: make the prices a table",
            "Rewrite",
        )
    };
    p.text(
        Face::Semibold,
        17.0,
        x,
        r.y + PAD + 10.0,
        title,
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
        placeholder,
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
            if split {
                "None of this kind yet. Save prompt keeps what you type for next time."
            } else {
                "None yet. Settings › AI › Saved prompts › Edit adds some."
            },
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
    let run_w = dialog_button_w(p, run_label);
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
        run_label,
        true,
        hover,
        pressed,
    );
}

/// The review's card in a `w` x `h` window.
pub fn review_card(w: f32, h: f32) -> Rect {
    dialog_card(1040.0, 736.0, w, h)
}

/// The area under the review's header and its bar of changes, above its footer.
pub fn review_body(r: Rect) -> Rect {
    Rect::new(
        r.x + 1.0,
        r.y + HEADER_H + BAR_H,
        r.w - 2.0,
        r.h - HEADER_H - BAR_H - DIALOG_FOOTER,
    )
}

/// "lines 32–34", "line 5" or "after line 7": where change `c` is in the old text.
pub fn lines_of(c: &mdedit_core::hunks::Change) -> String {
    if c.a.is_empty() {
        format!("after line {}", c.a.start)
    } else if c.a.len() == 1 {
        format!("line {}", c.a.start + 1)
    } else {
        format!("lines {}–{}", c.a.start + 1, c.a.end)
    }
}

/// The footer's first line: how many changes, how many go in, what that adds and takes away.
pub fn summary(rv: &Review) -> String {
    let n = rv.hunks.len();
    let k = rv.hunks.ticked();
    let (added, removed) = rv.hunks.taken_totals();
    let s = if n == 1 { "" } else { "s" };
    if k == 0 {
        format!("{n} change{s} · none to apply")
    } else {
        format!("{n} change{s} · {k} to apply · +{added} −{removed} lines")
    }
}

/// What the Apply button says.
pub fn apply_label(rv: &Review) -> String {
    match rv.hunks.ticked() {
        0 => "Nothing is ticked".into(),
        1 => "Apply 1 change".into(),
        k => format!("Apply {k} changes"),
    }
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
    let (_, _, minus, plus) = hunks_ui::tints(pal);
    let px = p.px();
    // Header: title, what and how, the switch, the counts.
    let x = r.x + PAD;
    let cy = r.y + 30.0;
    let tw = p.measure(Face::Semibold, 17.0, &rv.spec.title).min(360.0);
    p.text(Face::Semibold, 17.0, x, cy, &rv.spec.title, pal.text, 360.0);
    let (total_added, total_removed) = rv.hunks.totals();
    let counts_r = format!("+ {total_added} lines");
    let counts_l = format!("− {total_removed} lines");
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
    let labels = ["Changes", "Preview"];
    let sw_w = switch_width(p, &labels);
    let sw = Rect::new(right - sw_w, cy - SEGMENT_H / 2.0, sw_w, SEGMENT_H);
    let hovered = [Hit::Changes, Hit::Preview]
        .iter()
        .position(|h| hover == Some(*h));
    let segs = draw_segments(p, pal, sw, &labels, Some(usize::from(rv.preview)), hovered);
    hits.push((Hit::Changes, segs[0]));
    hits.push((Hit::Preview, segs[1]));
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
    changes_bar(
        p,
        pal,
        rv,
        Rect::new(r.x + 1.0, r.y + HEADER_H, r.w - 2.0, BAR_H),
        hover,
        hits,
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
        let list_hover = match hover {
            Some(Hit::List(l)) => Some(l),
            _ => None,
        };
        let mut list_hits = Vec::new();
        hunks_ui::draw(
            p,
            pal,
            &rv.hunks,
            &mut rv.list,
            body,
            list_hover,
            &mut list_hits,
        );
        hits.extend(list_hits.into_iter().map(|(h, r)| (Hit::List(h), r)));
    }

    // Footer: what goes in, then the buttons.
    let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
    let mut bx = r.right() - PAD;
    // A closed document cannot be run again: Copy result and Discard only.
    let closed = rv.blocked.as_deref() == Some(mdedit_core::ai::BLOCKED_CLOSED);
    let nothing = rv.blocked.is_none() && rv.hunks.ticked() == 0;
    let primary = if rv.blocked.is_some() {
        ("Copy result".to_string(), Hit::Copy)
    } else {
        (apply_label(rv), Hit::Apply)
    };
    let second = if rv.blocked.is_some() {
        ("Run again".to_string(), Hit::RunAgain)
    } else {
        ("Copy result".to_string(), Hit::Copy)
    };
    let buttons: Vec<(String, Hit)> = if closed {
        vec![primary, ("Discard".into(), Hit::Discard)]
    } else {
        vec![primary, second, ("Discard".into(), Hit::Discard)]
    };
    for (k, (label, hit)) in buttons.into_iter().enumerate() {
        let bw = dialog_button_w(p, &label);
        bx -= bw;
        let b = Rect::new(bx, by, bw, 32.0);
        if k == 0 && nothing {
            // Nothing to put in: the button says so and cannot be pressed.
            draw_dialog_button_disabled(p, pal, b, &label, true);
        } else {
            button(p, pal, hits, b, hit, &label, k == 0, hover, pressed);
        }
        bx -= 10.0;
    }
    let note = if rv.blocked.is_some() {
        ""
    } else if rv.file.is_some() {
        "Apply writes the file in place. Its original is kept in AI runs, where Undo brings it back."
    } else if rv.preview {
        "The preview shows only the ticked changes. Apply is one step: Ctrl+Z brings the old text back."
    } else {
        "Unticked changes keep the old text. Apply is one step: Ctrl+Z brings the old text back."
    };
    let room = bx - x - 12.0;
    let sum = summary(rv);
    if note.is_empty() {
        p.text(Face::Semibold, 12.5, x, footer.cy(), &sum, pal.text, room);
    } else {
        p.text(
            Face::Semibold,
            12.5,
            x,
            footer.cy() - 9.0,
            &sum,
            pal.text,
            room,
        );
        p.text(Face::Ui, 11.5, x, footer.cy() + 10.0, note, pal.muted, room);
    }
}

/// The strip under the header: the arrows to the previous and the next change, which one this is
/// and how big, the strip with a mark per change (where it is in the text, ticked or not, the
/// current one taller), Tick all and Untick all.
fn changes_bar(
    p: &mut Painter,
    pal: &Palette,
    rv: &Review,
    bar: Rect,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let (_, _, minus, plus) = hunks_ui::tints(pal);
    let px = p.px();
    p.fill(bar, mix(pal.popup, pal.text, 0.025));
    p.fill(
        Rect::new(bar.x, bar.bottom() - px, bar.w, px),
        pal.popup_line,
    );
    let n = rv.hunks.len();
    let cur = rv.list.current.min(n.saturating_sub(1));
    // The arrows. In Preview there is no list to scroll and the pages have no block rows
    // (D-108), so they are drawn off and take no click.
    let a = Rect::new(bar.x + PAD, bar.cy() - 14.0, 28.0, 28.0);
    let b = Rect::new(a.right() + 6.0, a.y, 28.0, 28.0);
    for (r, icon, hit, on) in [
        (a, glyph::UP, Hit::Prev, !rv.preview && cur > 0),
        (b, glyph::DOWN, Hit::Next, !rv.preview && cur + 1 < n),
    ] {
        let hov = hover == Some(hit) && on;
        let fill = if hov {
            pal.hover
        } else {
            mix(pal.popup, pal.text, 0.04)
        };
        p.frame(r, 5.0, fill, pal.popup_line);
        p.icon(
            icon,
            16.0,
            r.cx(),
            r.cy(),
            if on { pal.text } else { pal.faint },
        );
        if on {
            hits.push((hit, r));
        }
    }
    // Which change.
    let mut x = b.right() + 18.0;
    if let Some(c) = rv.hunks.change(cur) {
        let which = format!("Change {} of {n}", cur + 1);
        x += p.text(Face::Semibold, 13.0, x, bar.cy(), &which, pal.text, 200.0) + 12.0;
        x += p.text(Face::Ui, 12.0, x, bar.cy(), &lines_of(c), pal.muted, 160.0) + 12.0;
        if c.added() > 0 {
            let t = format!("+{}", c.added());
            x += p.text(Face::Semibold, 12.0, x, bar.cy(), &t, plus, 60.0) + 8.0;
        }
        if c.removed() > 0 {
            let t = format!("−{}", c.removed());
            p.text(Face::Semibold, 12.0, x, bar.cy(), &t, minus, 60.0);
        }
    }
    // Tick all, Untick all, then the strip with its word.
    let mut right = bar.right() - PAD;
    for (label, hit) in [("Untick all", Hit::UntickAll), ("Tick all", Hit::TickAll)] {
        let tw = p.measure(Face::Ui, 12.5, label);
        let r = Rect::new(right - tw - 8.0, bar.cy() - 12.0, tw + 8.0, 24.0);
        let col = if hover == Some(hit) {
            mix(pal.accent, pal.text, 0.25)
        } else {
            pal.accent
        };
        p.text(Face::Ui, 12.5, r.x + 4.0, bar.cy(), label, col, tw + 4.0);
        hits.push((hit, r));
        right = r.x - 14.0;
    }
    let strip = Rect::new(right - 300.0, bar.cy() - 5.0, 300.0, 10.0);
    if strip.x > x + 60.0 && n > 0 {
        p.text_right(Face::Ui, 11.5, strip.x - 10.0, bar.cy(), "where", pal.faint);
        p.rbox(strip, 5.0, mix(pal.popup, pal.text, 0.09));
        let lines = rv.hunks.old_lines().max(1) as f32;
        for i in 0..n {
            let Some(c) = rv.hunks.change(i) else {
                continue;
            };
            let at = (c.a.start as f32 / lines).clamp(0.0, 1.0);
            let mx = strip.x + 3.0 + at * (strip.w - 9.0);
            let on = rv.hunks.is_ticked(i);
            let (mh, col) = if i == cur {
                (18.0, pal.text)
            } else if on {
                (12.0, mix(pal.accent, pal.popup, 0.15))
            } else {
                (12.0, mix(pal.popup, pal.text, 0.28))
            };
            p.rbox(Rect::new(mx, bar.cy() - mh / 2.0, 3.0, mh), 1.5, col);
            hits.push((
                Hit::Mark(i),
                Rect::new(mx - 3.0, bar.y + 4.0, 9.0, bar.h - 8.0),
            ));
        }
    }
}

/// The text before and after, side by side, as mdview's renderer drew them.
fn previews(p: &mut Painter, pal: &Palette, rv: &mut Review, body: Rect) {
    let half = (body.w / 2.0).floor();
    let label_h = 30.0;
    let (_, plus_bg, _, plus) = hunks_ui::tints(pal);
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
        if k == 1 {
            // The page shows the ticked changes only; the header says how many.
            let taken = format!(
                "{} of {} change{} taken",
                rv.hunks.ticked(),
                rv.hunks.len(),
                if rv.hunks.len() == 1 { "" } else { "s" }
            );
            p.text_right(Face::Ui, 11.5, col.right() - PAD, head.cy(), &taken, fg);
        }
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
