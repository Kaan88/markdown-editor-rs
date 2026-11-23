//! Drawing the import card (`import_ui.rs` holds its state and events). The backdrop, the card, the
//! tick boxes, the list's arithmetic, the scrollbar and the buttons are the ones the split's plan
//! card and the move card draw with, so the three read as one family.

use crate::import_ui::{
    Geo, IHit, ImportCard, count, group_line, preview_note, row_title, row_under, subtitle, title,
};
use crate::move_draw::fit_end;
use mdedit_core::import::{self, RowKind};
use mdshared_ui::check::{TICK, Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button,
    draw_dialog_button_disabled, draw_dialog_card,
};
use mdshared_ui::scroll::draw_scrollbar;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

impl ImportCard {
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
        if let Some(preview) = g.preview {
            self.draw_preview(p, pal, preview);
        }
        self.draw_footer(p, pal, &g, footer);
    }

    fn hovered(&self, hit: IHit) -> bool {
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
            title(),
            pal.text,
            max,
        );
        let sub = fit_end(p, Face::Ui, 12.5, &self.subtitle_text(), max);
        p.text(Face::Ui, 12.5, x, y + 56.0, &sub, pal.muted, max);
        let group = fit_end(p, Face::Ui, 12.5, &group_line(&self.plan), max);
        p.text(Face::Ui, 12.5, x, y + 78.0, &group, pal.text, max);
        if self.plan.already() > 0 {
            let n = self.plan.already();
            p.text(
                Face::Ui,
                12.0,
                x,
                y + 96.0,
                &format!(
                    "The group lists {} already, so {} not written again.",
                    count(n, "file", "files"),
                    if n == 1 { "it is" } else { "they are" }
                ),
                pal.faint,
                max,
            );
        }
        p.fill(
            Rect::new(g.card.x, g.header.bottom(), g.card.w, p.px()),
            pal.popup_line,
        );
    }

    /// The header's second line, with the walk's own limits said when it hit one.
    fn subtitle_text(&self) -> String {
        let mut line = subtitle(&self.plan);
        if self.plan.more {
            line.push_str(&format!(
                " \u{00B7} more than {}, not all read",
                mdshared_app::library::MAX_ENTRIES
            ));
        }
        line
    }

    fn draw_list(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let head = g.list_head;
        p.text(
            Face::Semibold,
            11.0,
            head.x + PAD,
            head.cy(),
            &format!("FILES ({})", self.plan.notes.len()),
            pal.muted,
            160.0,
        );
        let mut right = g.list.right() - 16.0;
        for (hit, label) in [(IHit::UntickAll, "Untick all"), (IHit::TickAll, "Tick all")] {
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
            let hot = matches!(self.hover, Some(IHit::Row(k) | IHit::Tick(k)) if k == i);
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
            let active = self.hovered(IHit::Thumb);
            draw_scrollbar(p, pal, track, thumb, active);
            self.spots.push((IHit::Track, track));
            self.spots.push((IHit::Thumb, thumb));
        }
    }

    fn draw_row(&self, p: &mut Painter, pal: &Palette, row: Rect, i: usize) {
        let Some(r) = self.plan.rows.get(i) else {
            return;
        };
        let section = matches!(r.kind, RowKind::Section { .. });
        // A file the library lists already is drawn dim and has no box: its line is there.
        let already = self
            .plan
            .note_of(i)
            .and_then(|n| self.plan.notes.get(n))
            .is_some_and(|n| n.already);
        let tick = if already && !section {
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
        // A section is one line, a file is two; both rows are the same height.
        let depth = match &r.kind {
            RowKind::Section { path } => path.len(),
            RowKind::File(_) => 0,
        };
        let x = row.x + 42.0 + (depth.saturating_sub(1) as f32) * 14.0;
        let max = (row.right() - 14.0 - x).max(0.0);
        let color = if already {
            pal.faint
        } else if r.ticked {
            pal.text
        } else {
            pal.muted
        };
        if section {
            p.text(
                Face::Semibold,
                12.0,
                x,
                row.cy() - 1.0,
                &row_title(&self.plan, i),
                color,
                max,
            );
            p.text(
                Face::Ui,
                11.5,
                x,
                row.cy() + 13.0,
                &row_under(&self.plan, i),
                pal.faint,
                max,
            );
        } else {
            let name = row_title(&self.plan, i);
            p.text(Face::Semibold, 13.0, x, row.y + 17.0, &name, color, max);
            p.text(
                Face::Ui,
                11.5,
                x,
                row.y + 33.0,
                &row_under(&self.plan, i),
                if already { pal.faint } else { pal.muted },
                max,
            );
        }
    }

    fn draw_preview(&self, p: &mut Painter, pal: &Palette, r: Rect) {
        // A line between the list and what is under it.
        p.fill(Rect::new(r.x, r.y, p.px(), r.h), pal.popup_line);
        let x = r.x + PAD;
        let max = (r.w - 2.0 * PAD).max(0.0);
        let y = r.y + 8.0;
        let Some(row) = self.plan.rows.get(self.sel) else {
            return;
        };
        match &row.kind {
            RowKind::Section { path } => {
                let (all, ticked) = crate::import_ui::section_count(&self.plan, path);
                p.text(
                    Face::Semibold,
                    14.0,
                    x,
                    y + 20.0,
                    &path.join(" \u{203A} "),
                    pal.text,
                    max,
                );
                p.text(
                    Face::Ui,
                    12.0,
                    x,
                    y + 42.0,
                    &format!("{} \u{00B7} {} ticked", count(all, "file", "files"), ticked),
                    pal.muted,
                    max,
                );
                p.text(
                    Face::Ui,
                    12.0,
                    x,
                    y + 62.0,
                    "The rows under this heading are ticked and unticked with it.",
                    pal.faint,
                    max,
                );
            }
            RowKind::File(i) => {
                let Some(note) = self.plan.notes.get(*i) else {
                    return;
                };
                p.text(
                    Face::Semibold,
                    14.0,
                    x,
                    y + 20.0,
                    &note.title,
                    pal.text,
                    max,
                );
                let where_it_is = if note.sections.is_empty() {
                    import::shown(&note.path)
                } else {
                    format!(
                        "{} \u{00B7} {}",
                        note.sections.join(" \u{203A} "),
                        import::shown(&note.path)
                    )
                };
                let line = fit_end(p, Face::Ui, 12.0, &where_it_is, max);
                p.text(Face::Ui, 12.0, x, y + 40.0, &line, pal.muted, max);
                if note.already {
                    p.text(
                        Face::Ui,
                        12.0,
                        x,
                        y + 58.0,
                        "The group lists this file already: its line is not written again.",
                        pal.faint,
                        max,
                    );
                }
                // The file's own first lines, in the fixed-width face, as many as fit.
                let top = y + 78.0;
                let room = (r.bottom() - 30.0 - top).max(0.0);
                let lines = (room / 17.0).floor() as usize;
                let old = p.clip(Rect::new(x, top, max, room));
                for (n, text) in note.head.iter().take(lines).enumerate() {
                    p.text(
                        Face::Mono,
                        12.0,
                        x,
                        top + 12.0 + n as f32 * 17.0,
                        text,
                        if n == 0 { pal.text } else { pal.muted },
                        max,
                    );
                }
                p.restore(old);
                let foot = preview_note(&self.plan, note);
                let foot = fit_end(p, Face::Ui, 11.5, &foot, max);
                p.text(
                    Face::Ui,
                    11.5,
                    x,
                    r.bottom() - 10.0,
                    &foot,
                    pal.faint,
                    max,
                );
            }
        }
    }

    fn draw_footer(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, footer: Rect) {
        let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
        let add = format!(
            "Add {}",
            count(self.plan.ticked(), "file", "files")
        );
        let mut x = g.card.right() - PAD;
        let mut buttons: Vec<(IHit, String, bool, Rect)> = Vec::new();
        for (hit, label, primary) in [
            (IHit::Add, add, true),
            (IHit::Cancel, "Cancel".to_string(), false),
        ] {
            let bw = dialog_button_w(p, &label);
            x -= bw;
            buttons.push((hit, label, primary, Rect::new(x, by, bw, 32.0)));
            x -= 8.0;
        }
        let problem = self.problem();
        for (hit, label, primary, r) in buttons {
            if hit == IHit::Add && problem.is_some() {
                draw_dialog_button_disabled(p, pal, r, &label, primary);
            } else {
                draw_dialog_button(
                    p,
                    pal,
                    r,
                    &label,
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
        let total = self.plan.notes.len();
        let mut line = format!("{} of {} ticked", self.plan.ticked(), count(total, "file", "files"));
        let already = self.plan.already();
        if already > 0 {
            line.push_str(&format!(" \u{00B7} {already} already in the library"));
        }
        let max = (limit - left).max(0.0);
        p.text(Face::Ui, 13.0, left, footer.y + 26.0, &line, pal.text, max);
        let file = self
            .plan
            .group_file
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let sentence = if self.plan.ticked() == 0 {
            "Writes one line per ticked file in the group's library file. Nothing is copied, moved or changed.".to_string()
        } else {
            format!(
                "Writes {} in {file}. Nothing is copied, moved or changed.",
                count(self.plan.ticked(), "line", "lines")
            )
        };
        let sentence = fit_end(p, Face::Ui, 12.0, &sentence, max);
        p.text(Face::Ui, 12.0, left, footer.y + 48.0, &sentence, pal.muted, max);
    }
}
