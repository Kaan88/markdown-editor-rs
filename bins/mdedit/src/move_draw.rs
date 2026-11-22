//! Drawing the move card (`move_ui.rs` holds its state and events). The backdrop, the card, the
//! tick boxes, the list's arithmetic, the scrollbar and the buttons are the ones the split's plan
//! card draws with, so the two cards read as one family.

use crate::move_ui::{Geo, MHit, MoveCard, aside, from_to, name, subtitle, title, why};
use mdshared_ui::check::{TICK, Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button, draw_dialog_card,
};
use mdshared_ui::scroll::draw_scrollbar;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

/// `text` cut from the front to fit `max`, with "…" where it was cut: a path keeps its end, the
/// folder and the file's name, which is what tells two rows apart.
pub fn fit_end(p: &mut Painter, face: Face, size: f32, text: &str, max: f32) -> String {
    if p.measure(face, size, text) <= max {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    // The fewest characters cut that fit, found by halves.
    let (mut lo, mut hi) = (1usize, chars.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        let s: String = std::iter::once('\u{2026}')
            .chain(chars[mid..].iter().copied())
            .collect();
        if p.measure(face, size, &s) <= max {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    std::iter::once('\u{2026}')
        .chain(chars[lo.min(chars.len())..].iter().copied())
        .collect()
}

/// The disabled button, as the plan card and `replace_ui::dialog_button` draw it.
fn disabled_button(p: &mut Painter, pal: &Palette, r: Rect, label: &str) {
    p.frame(r, 6.0, pal.field, pal.line);
    p.text_center(Face::Semibold, 13.0, r, label, pal.faint);
}

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

impl MoveCard {
    /// Draws the whole card over a `w` x `h` window.
    pub fn draw(&mut self, p: &mut Painter, pal: &Palette, w: f32, h: f32) {
        self.win = (w, h);
        self.spots.clear();
        let rows = self.rows();
        self.scroll = rows.clamp_scroll(self.scroll);
        draw_backdrop(p, pal, w, h);
        let g = self.geo();
        let footer = draw_dialog_card(p, pal, g.card);
        self.draw_header(p, pal, &g);
        self.draw_list(p, pal, &g);
        self.draw_footer(p, pal, &g, footer);
    }

    fn hovered(&self, hit: MHit) -> bool {
        self.hover == Some(hit)
    }

    fn draw_header(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let x = g.card.x + PAD;
        let max = (g.card.w - 2.0 * PAD).max(0.0);
        let y = g.header.y;
        p.text(
            Face::Semibold,
            17.0,
            x,
            y + 32.0,
            &title(&self.plan),
            pal.text,
            max,
        );
        let sub = subtitle(&self.plan);
        let sub = fit_end(p, Face::Ui, 12.5, &sub, max);
        p.text(Face::Ui, 12.5, x, y + 56.0, &sub, pal.muted, max);
        let mut notes: Vec<String> = Vec::new();
        if self.plan.missing > 0 {
            notes.push(format!(
                "{} the library lists {} not on disk and {} as {}.",
                count(self.plan.missing, "note", "notes"),
                if self.plan.missing == 1 { "is" } else { "are" },
                if self.plan.missing == 1 {
                    "stays"
                } else {
                    "stay"
                },
                if self.plan.missing == 1 {
                    "it is"
                } else {
                    "they are"
                },
            ));
        }
        if self.plan.others > 0 {
            notes.push(format!(
                "{}{} in the old folder {} not the library's and {} there.",
                count(self.plan.others, "other file", "other files"),
                if self.plan.others_capped {
                    " or more"
                } else {
                    ""
                },
                if self.plan.others == 1 { "is" } else { "are" },
                if self.plan.others == 1 {
                    "stays"
                } else {
                    "stay"
                },
            ));
        }
        if self.plan.omitted > 0 {
            notes.push(format!(
                "{} past the reader's limit {} where {}.",
                count(self.plan.omitted, "library file", "library files"),
                if self.plan.omitted == 1 {
                    "stays"
                } else {
                    "stay"
                },
                if self.plan.omitted == 1 {
                    "it is"
                } else {
                    "they are"
                },
            ));
        }
        if !notes.is_empty() {
            p.text(
                Face::Ui,
                12.0,
                x,
                y + 76.0,
                &notes.join(" "),
                pal.faint,
                max,
            );
        }
        p.fill(
            Rect::new(g.card.x, g.header.bottom(), g.card.w, p.px()),
            pal.popup_line,
        );
    }

    fn draw_list(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let head = g.list_head;
        p.text(
            Face::Semibold,
            11.0,
            head.x + PAD,
            head.cy(),
            &format!("FILES ({})", self.plan.rows.len()),
            pal.muted,
            160.0,
        );
        let mut right = head.right() - 16.0;
        for (hit, label) in [(MHit::UntickAll, "Untick all"), (MHit::TickAll, "Tick all")] {
            let w = p.measure(Face::Ui, 12.5, label) + 12.0;
            let r = Rect::new(right - w, head.y, w, head.h);
            let color = if self.hovered(hit) {
                pal.accent
            } else {
                pal.muted
            };
            p.text(Face::Ui, 12.5, r.x + 6.0, r.cy(), label, color, w);
            self.spots.push((hit, r));
            right = r.x - 4.0;
        }

        let rows = self.rows();
        let old = p.clip(g.list);
        for i in rows.visible() {
            let rr = rows.rect(g.list, i);
            let row = Rect::new(rr.x + 8.0, rr.y + 2.0, rr.w - 16.0, rr.h - 4.0);
            let hot = matches!(self.hover, Some(MHit::Row(k) | MHit::Tick(k)) if k == i);
            if i == self.sel {
                p.rbox(row, 6.0, pal.on);
                p.rbox(
                    Rect::new(row.x, row.y + 7.0, 3.0, row.h - 14.0),
                    1.5,
                    pal.accent,
                );
            } else if hot {
                p.rbox(row, 6.0, pal.hover);
            }
            self.draw_row(p, pal, row, i);
        }
        p.restore(old);
        if let Some((track, thumb)) = rows.thumb(g.list) {
            let active = self.hovered(MHit::Thumb);
            draw_scrollbar(p, pal, track, thumb, active);
            self.spots.push((MHit::Track, track));
            self.spots.push((MHit::Thumb, thumb));
        }
    }

    fn draw_row(&self, p: &mut Painter, pal: &Palette, row: Rect, i: usize) {
        let Some(r) = self.plan.rows.get(i) else {
            return;
        };
        let blocked = r.verdict.blocked();
        let tick = if blocked {
            Tick::disabled(false)
        } else {
            Tick::new(r.ticked)
        };
        draw_tick(
            p,
            pal,
            Rect::new(row.x + 14.0, row.y + 18.0 - TICK / 2.0, TICK, TICK),
            tick,
        );
        let x = row.x + 42.0;
        let max = (row.right() - 14.0 - x).max(0.0);
        let (c1, c2, c3) = (row.y + 16.0, row.y + 34.0, row.y + 51.0);
        // The name, what kind of file it is when it is not a note, and what a note carries and
        // rewrites (D-091).
        let nw = p.text(
            Face::Semibold,
            13.5,
            x,
            c1,
            &name(r),
            if r.ticked { pal.text } else { pal.muted },
            max,
        );
        if let Some(kind) = aside(r) {
            p.text(
                Face::Ui,
                12.0,
                x + nw + 8.0,
                c1,
                &kind,
                pal.faint,
                (max - nw - 8.0).max(0.0),
            );
        }
        // Where it is → where it goes, each cut from the front when the two do not fit.
        let (from, to) = from_to(&self.plan, r);
        let arrow = " \u{2192} ";
        let aw = p.measure(Face::Ui, 12.0, arrow);
        let half = ((max - aw) / 2.0).max(0.0);
        let fw = p.measure(Face::Ui, 12.0, &from);
        let tw = p.measure(Face::Ui, 12.0, &to);
        // Room the shorter one does not use goes to the other.
        let (fmax, tmax) = if fw + aw + tw <= max {
            (fw, tw)
        } else if fw < half {
            (fw, max - aw - fw)
        } else if tw < half {
            (max - aw - tw, tw)
        } else {
            (half, half)
        };
        let from = fit_end(p, Face::Ui, 12.0, &from, fmax.max(0.0));
        let to = fit_end(p, Face::Ui, 12.0, &to, tmax.max(0.0));
        let w1 = p.text(Face::Ui, 12.0, x, c2, &from, pal.muted, fmax + 1.0);
        let w2 = p.text(Face::Ui, 12.0, x + w1, c2, arrow, pal.faint, aw + 1.0);
        p.text(
            Face::Ui,
            12.0,
            x + w1 + w2,
            c2,
            &to,
            if blocked { pal.muted } else { pal.text },
            tmax + 1.0,
        );
        // Why it moves, or why it cannot, or where it lives.
        let line = why(r);
        let color = if blocked {
            pal.warn
        } else if r.outside {
            pal.text
        } else {
            pal.faint
        };
        p.text(Face::Ui, 12.0, x, c3, &line, color, max);
    }

    fn draw_footer(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, footer: Rect) {
        let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
        let mut x = g.card.right() - PAD;
        let mut buttons: Vec<(MHit, &str, bool, Rect)> = Vec::new();
        for (hit, label, primary) in [(MHit::Move, "Move", true), (MHit::Cancel, "Cancel", false)] {
            let bw = dialog_button_w(p, label);
            x -= bw;
            buttons.push((hit, label, primary, Rect::new(x, by, bw, 32.0)));
            x -= 8.0;
        }
        let problem = self.problem();
        for (hit, label, primary, r) in buttons {
            if hit == MHit::Move && problem.is_some() {
                disabled_button(p, pal, r, label);
            } else {
                draw_dialog_button(
                    p,
                    pal,
                    r,
                    label,
                    primary,
                    self.hovered(hit),
                    self.pressed == Some(hit),
                );
                self.spots.push((hit, r));
            }
        }
        let left = g.card.x + PAD;
        let mut limit = x - 8.0;
        if let Some(reason) = problem.as_ref().or(self.refusal.as_ref()) {
            let room = (limit - left - 200.0).max(0.0);
            let w = p.measure(Face::Ui, 12.5, reason).min(room);
            p.text(
                Face::Ui,
                12.5,
                limit - w,
                footer.y + 36.0,
                reason,
                pal.warn,
                w + 2.0,
            );
            limit -= w + 16.0;
        }
        let total = self.plan.rows.len();
        let cannot = total - self.movable();
        let mut line = format!(
            "{} of {}",
            self.plan.ticked(),
            count(total, "file", "files")
        );
        if cannot > 0 {
            line.push_str(&format!(" \u{00B7} {cannot} cannot move"));
        }
        let max = (limit - left).max(0.0);
        p.text(Face::Ui, 13.0, left, footer.y + 26.0, &line, pal.text, max);
        p.text(
            Face::Ui,
            12.0,
            left,
            footer.y + 48.0,
            "Moves each ticked file and rewrites its line in the library. Nothing is copied over or replaced.",
            pal.muted,
            max,
        );
    }
}
