//! Drawing the import card (`import_ui.rs` holds its state and events). The backdrop, the card, the
//! tick boxes, the list's arithmetic, the scrollbar and the buttons are the ones the split's plan
//! card and the move card draw with, so the three read as one family. The AI step of 5.7 adds two
//! more drawings: the scheme's one-line list, and the wait's own footer.

use crate::import_ui::{
    Geo, IHit, ImportCard, SortStage, count, group_line, left_out_line, preview_note, row_title,
    row_under, subtitle, title,
};
use crate::move_draw::fit_end;
use mdedit_core::import::{self, RowKind};
use mdedit_core::import_ai;
use mdshared_ui::check::{TICK, Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button,
    draw_dialog_button_disabled, draw_dialog_card,
};
use mdshared_ui::scroll::draw_scrollbar;
use mdshared_ui::textfield::FieldColors;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

impl ImportCard {
    /// Draws the whole card over a `w` x `h` window.
    pub fn draw(&mut self, p: &mut Painter, pal: &Palette, w: f32, h: f32, caret_on: bool) {
        self.win = (w, h);
        self.spots.clear();
        let rows = self.rows();
        self.scroll = rows.clamp_scroll(self.scroll);
        draw_backdrop(p, pal, w, h);
        let g = self.geo();
        let footer = draw_dialog_card(p, pal, g.card);
        self.draw_header(p, pal, &g);
        let scheme = matches!(self.sort, SortStage::Scheme(_) | SortStage::Filing { .. });
        if scheme {
            self.draw_scheme_list(p, pal, &g, caret_on);
        } else {
            self.draw_list(p, pal, &g);
        }
        if let Some(preview) = g.preview {
            if scheme {
                self.draw_scheme_preview(p, pal, preview);
            } else {
                self.draw_preview(p, pal, preview);
            }
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
        p.text(Face::Semibold, 17.0, x, y + 32.0, title(), pal.text, max);
        let sub = fit_end(p, Face::Ui, 12.5, &self.subtitle_text(), max);
        p.text(Face::Ui, 12.5, x, y + 56.0, &sub, pal.muted, max);
        let group = fit_end(p, Face::Ui, 12.5, &group_line(&self.plan), max);
        p.text(Face::Ui, 12.5, x, y + 78.0, &group, pal.text, max);
        if let Some(left) = left_out_line(self) {
            let left = fit_end(p, Face::Ui, 12.0, &left, max);
            p.text(Face::Ui, 12.0, x, y + 96.0, &left, pal.faint, max);
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

    /// A link at the right of the list's head; returns the room left of it.
    #[allow(clippy::too_many_arguments)] // as the AI screens' draw helpers are
    fn head_link(
        &mut self,
        p: &mut Painter,
        pal: &Palette,
        right: f32,
        head: Rect,
        hit: IHit,
        label: &str,
        enabled: bool,
    ) -> f32 {
        let w = p.measure(Face::Ui, 12.5, label) + 12.0;
        let r = Rect::new(right - w, head.y, w, head.h);
        let color = if !enabled {
            pal.faint
        } else if self.hovered(hit) {
            pal.accent
        } else {
            pal.muted
        };
        p.text(Face::Ui, 12.5, r.x + 6.0, r.cy(), label, color, w);
        if enabled {
            self.spots.push((hit, r));
        }
        r.x - 4.0
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
        match &self.sort {
            SortStage::Asking => {
                p.text_right(
                    Face::Ui,
                    12.5,
                    right,
                    head.cy(),
                    "Asking the AI\u{2026}",
                    pal.muted,
                );
            }
            SortStage::Filed { stopped, .. } => {
                if stopped.is_some() {
                    right = self.head_link(
                        p,
                        pal,
                        right,
                        head,
                        IHit::AskAi,
                        "Ask the AI again",
                        self.can_ask(),
                    );
                } else {
                    right = self.head_link(
                        p,
                        pal,
                        right,
                        head,
                        IHit::SortSure,
                        "Tick only the sure ones",
                        true,
                    );
                }
                right = self.head_link(p, pal, right, head, IHit::UntickAll, "Untick all", true);
                let _ = self.head_link(p, pal, right, head, IHit::TickAll, "Tick all", true);
            }
            _ => {
                // The AI's offer stands where the tick links are; with the connection off, the
                // reason and a way to the settings stand in its place, so nothing looks pressable
                // that is not.
                let failed = matches!(self.sort, SortStage::Failed(_));
                if !failed {
                    let reason = self.ai_reason.clone();
                    match &reason {
                        Some(why) => {
                            let mut left = right;
                            for (hit, label) in
                                [(IHit::UntickAll, "Untick all"), (IHit::TickAll, "Tick all")]
                            {
                                left = self.head_link(p, pal, left, head, hit, label, true);
                            }
                            left = self.head_link(
                                p,
                                pal,
                                left,
                                head,
                                IHit::Settings,
                                "Settings",
                                true,
                            );
                            let room = (left - 12.0 - head.x).max(0.0);
                            let why = fit_end(p, Face::Ui, 12.5, why, room);
                            p.text_right(Face::Ui, 12.5, left - 12.0, head.cy(), &why, pal.faint);
                        }
                        None => {
                            right = self.head_link(
                                p,
                                pal,
                                right,
                                head,
                                IHit::AskAi,
                                "Ask the AI to sort them\u{2026}",
                                self.can_ask(),
                            );
                            for (hit, label) in
                                [(IHit::UntickAll, "Untick all"), (IHit::TickAll, "Tick all")]
                            {
                                right = self.head_link(p, pal, right, head, hit, label, true);
                            }
                        }
                    }
                } else {
                    for (hit, label) in
                        [(IHit::UntickAll, "Untick all"), (IHit::TickAll, "Tick all")]
                    {
                        right = self.head_link(p, pal, right, head, hit, label, true);
                    }
                }
            }
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
        let unsorted = self
            .plan
            .note_of(i)
            .and_then(|n| self.plan.notes.get(n))
            .is_some_and(|n| n.unsorted);
        let tick = if (already && !section) || unsorted {
            Tick::disabled(false)
        } else {
            Tick::new(r.ticked)
        };
        let box_at = Rect::new(row.x + 14.0, row.y + 18.0 - TICK / 2.0, TICK, TICK);
        draw_tick(p, pal, box_at, tick);
        // A heading with only some of its notes ticked says so with a dash in its box: neither an
        // empty box nor a tick would tell the truth.
        if let RowKind::Section { path } = &r.kind {
            let (all, ticked) = crate::import_ui::section_count(&self.plan, path);
            if ticked > 0 && ticked < all {
                p.fill(
                    Rect::new(box_at.x + 4.0, box_at.cy() - 1.0, TICK - 8.0, 2.0),
                    pal.accent,
                );
            }
        }
        // A section is one line, a file is two; both rows are the same height.
        let depth = match &r.kind {
            RowKind::Section { path } => path.len(),
            RowKind::File(_) => 0,
        };
        let x = row.x + 42.0 + (depth.saturating_sub(1) as f32) * 14.0;
        let max = (row.right() - 14.0 - x).max(0.0);
        let color = if already || unsorted {
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
                if already || unsorted {
                    pal.faint
                } else {
                    pal.muted
                },
                max,
            );
        }
    }

    /// The scheme's list: one line each, a section then its subsections, the selected row's name
    /// box while it is being renamed.
    fn draw_scheme_list(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, caret_on: bool) {
        let head = g.list_head;
        let (label, count_rows) = match &self.sort {
            SortStage::Filing { .. } => ("SORTING THE FILES\u{2026}".to_string(), self.row_count()),
            _ => (
                format!("THE AI\u{2019}S SECTIONS ({})", self.row_count()),
                self.row_count(),
            ),
        };
        p.text(
            Face::Semibold,
            11.0,
            head.x + PAD,
            head.cy(),
            &label,
            pal.muted,
            240.0,
        );
        let _ = count_rows;
        if matches!(self.sort, SortStage::Scheme(_)) {
            let mut right = g.list.right() - 16.0;
            for (hit, label) in [
                (IHit::DeleteSection, "Delete"),
                (IHit::RenameSection, "Rename"),
                (IHit::AddSection, "Add section"),
            ] {
                right =
                    self.head_link(p, pal, right, head, hit, label, self.sel < self.row_count());
            }
        }
        let rows = self.rows();
        let scheme_rows = self.scheme().map(|s| s.rows()).unwrap_or_default();
        let old = p.clip(g.list);
        for i in rows.visible() {
            let rr = rows.rect(g.list, i);
            let row = Rect::new(rr.x + 8.0, rr.y + 2.0, rr.w - 16.0, rr.h - 4.0);
            let hot = matches!(self.hover, Some(IHit::Row(k)) if k == i);
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
            let Some(sr) = scheme_rows.get(i) else {
                continue;
            };
            let depth = sr.path.len().saturating_sub(1);
            let x = row.x + 20.0 + depth as f32 * 18.0;
            let max = (row.right() - 12.0 - x).max(0.0);
            if i == self.sel && self.edit.is_some() {
                let frame = Rect::new(x - 6.0, row.y + 3.0, (max + 8.0).max(60.0), row.h - 6.0);
                p.frame(frame, 5.0, pal.field, pal.accent);
                let colors = FieldColors {
                    text: pal.text,
                    placeholder: pal.faint,
                    selection: pal.selection,
                    caret: pal.text,
                };
                if let Some(field) = self.edit.as_mut() {
                    let area =
                        Rect::new(frame.x + 8.0, frame.y, (frame.w - 16.0).max(10.0), frame.h);
                    field.draw(
                        p,
                        area,
                        Face::Ui,
                        13.0,
                        &colors,
                        "A section's name",
                        true,
                        caret_on,
                    );
                }
                continue;
            }
            if sr.top {
                p.text(
                    Face::Semibold,
                    13.5,
                    x,
                    row.cy(),
                    &sr.name,
                    if self.sel == i { pal.text } else { pal.muted },
                    max,
                );
            } else {
                p.text(
                    Face::Ui,
                    13.0,
                    x,
                    row.cy(),
                    &format!("\u{203A} {}", sr.name),
                    pal.muted,
                    max,
                );
            }
        }
        p.restore(old);
        if let Some((track, thumb)) = rows.thumb(g.list) {
            let active = self.hovered(IHit::Thumb);
            draw_scrollbar(p, pal, track, thumb, active);
            self.spots.push((IHit::Track, track));
            self.spots.push((IHit::Thumb, thumb));
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
                let mut top = y + 58.0;
                if note.already {
                    p.text(
                        Face::Ui,
                        12.0,
                        x,
                        top,
                        "The group lists this file already: its line is not written again.",
                        pal.faint,
                        max,
                    );
                    top += 18.0;
                }
                // The AI's reason, wrapped, in the dim face: the row has one line of it at most.
                if !note.why.is_empty() {
                    let mut at = top;
                    for line in wrap(p, Face::Ui, 12.0, &note.why, max) {
                        p.text(Face::Ui, 12.0, x, at, &line, pal.faint, max);
                        at += 17.0;
                    }
                    top = at + 6.0;
                }
                // The file's own first lines, in the fixed-width face, as many as fit.
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
                let arrow = "\u{2192} ";
                let aw = p.measure(Face::Ui, 11.5, arrow);
                p.text(Face::Ui, 11.5, x, r.bottom() - 10.0, arrow, pal.faint, aw);
                let rest = foot.strip_prefix(arrow).unwrap_or(&foot);
                let rest = fit_end(p, Face::Ui, 11.5, rest, (max - aw).max(0.0));
                p.text(
                    Face::Ui,
                    11.5,
                    x + aw,
                    r.bottom() - 10.0,
                    &rest,
                    pal.faint,
                    (max - aw).max(0.0),
                );
            }
        }
    }

    fn draw_scheme_preview(&self, p: &mut Painter, pal: &Palette, r: Rect) {
        p.fill(Rect::new(r.x, r.y, p.px(), r.h), pal.popup_line);
        let x = r.x + PAD;
        let max = (r.w - 2.0 * PAD).max(0.0);
        let y = r.y + 8.0;
        let Some(scheme) = self.scheme() else {
            return;
        };
        let rows = scheme.rows();
        let Some(row) = rows.get(self.sel) else {
            return;
        };
        p.text(
            Face::Semibold,
            14.0,
            x,
            y + 20.0,
            &row.path.join(" \u{203A} "),
            pal.text,
            max,
        );
        let (line, more) = if row.top {
            (
                format!(
                    "A section{}.",
                    if scheme
                        .sections
                        .iter()
                        .find(|s| s.name == row.name)
                        .is_some_and(|s| !s.subs.is_empty())
                    {
                        ", with its subsections under it"
                    } else {
                        ""
                    }
                ),
                "The files the AI puts here are written under this heading in the group's library file.",
            )
        } else {
            (
                format!(
                    "A subsection of \u{201c}{}\u{201d}.",
                    row.path.first().map(String::as_str).unwrap_or("")
                ),
                "The files the AI puts here are written under both headings, the section's then this one.",
            )
        };
        p.text(Face::Ui, 12.0, x, y + 42.0, &line, pal.muted, max);
        let more = wrap(p, Face::Ui, 12.0, more, max);
        let mut at = y + 64.0;
        for line in more {
            p.text(Face::Ui, 12.0, x, at, &line, pal.faint, max);
            at += 17.0;
        }
        if scheme.dropped > 0 {
            let line = format!(
                "The AI proposed {} more sections or subsections than a scheme keeps; they are left out.",
                scheme.dropped
            );
            at += 8.0;
            for line in wrap(p, Face::Ui, 12.0, &line, max) {
                p.text(Face::Ui, 12.0, x, at, &line, pal.faint, max);
                at += 17.0;
            }
        }
        at += 10.0;
        for line in wrap(
            p,
            Face::Ui,
            12.0,
            "Rename, add or delete before the files are sent; correcting the scheme here costs nothing.",
            max,
        ) {
            p.text(Face::Ui, 12.0, x, at, &line, pal.faint, max);
            at += 17.0;
        }
    }

    fn draw_footer(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, footer: Rect) {
        let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
        let (line, second, buttons, problem) = self.footer_lines();
        let mut x = g.card.right() - PAD;
        let mut places: Vec<(IHit, String, bool, Rect)> = Vec::new();
        for (hit, label, primary) in buttons {
            let bw = dialog_button_w(p, &label);
            x -= bw;
            places.push((hit, label, primary, Rect::new(x, by, bw, 32.0)));
            x -= 8.0;
        }
        let disabled: Option<String> = problem;
        for (hit, label, primary, r) in places {
            let off =
                disabled.is_some() && matches!(hit, IHit::Add | IHit::SortStart | IHit::AskAi);
            if off {
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
        let max = (x - 8.0 - left).max(0.0);
        // The reason the primary is off; else what the last Add was refused with, which leaves
        // the button on, as pressing it again may pass (a tab saved meanwhile).
        let said = disabled.clone().or_else(|| self.refusal.clone());
        match said.as_ref() {
            Some(reason) => {
                let reason = fit_end(p, Face::Ui, 12.5, reason, max);
                p.text(
                    Face::Ui,
                    12.5,
                    left,
                    footer.y + 26.0,
                    &reason,
                    pal.warn,
                    max,
                );
                let line = fit_end(p, Face::Ui, 12.0, &second, max);
                p.text(Face::Ui, 12.0, left, footer.y + 48.0, &line, pal.muted, max);
            }
            None => {
                p.text(Face::Ui, 13.0, left, footer.y + 26.0, &line, pal.text, max);
                let second = fit_end(p, Face::Ui, 12.0, &second, max);
                p.text(
                    Face::Ui,
                    12.0,
                    left,
                    footer.y + 48.0,
                    &second,
                    pal.muted,
                    max,
                );
            }
        }
    }

    /// The footer's two lines, its buttons and, when the primary is disabled, the reason: one
    /// match over the AI stage, so each state says exactly what it does.
    #[allow(clippy::type_complexity)]
    fn footer_lines(&self) -> (String, String, Vec<(IHit, String, bool)>, Option<String>) {
        let count_files = count(self.plan.ticked(), "file", "files");
        let add = (IHit::Add, format!("Add {count_files}"), true);
        let cancel = (IHit::Cancel, "Cancel".to_string(), false);
        match &self.sort {
            SortStage::Asking => (
                "Asking the AI for the sections\u{2026}".to_string(),
                format!(
                    "{} \u{00B7} nothing is written until you have seen the sections.",
                    self.ai_via
                ),
                vec![(IHit::SortStop, "Stop".to_string(), false)],
                None,
            ),
            SortStage::Scheme(scheme) => {
                let rows = scheme.rows();
                let subs = rows.iter().filter(|r| !r.top).count();
                let total = import_ai::ai_notes(&self.plan).len();
                let problem = scheme.problem();
                let line = match &problem {
                    Some(_) => format!(
                        "{} sections \u{00B7} {} subsections",
                        scheme.sections.len(),
                        subs
                    ),
                    None => format!(
                        "{} sections \u{00B7} {} subsections \u{00B7} {} files to sort",
                        scheme.sections.len(),
                        subs,
                        total
                    ),
                };
                let mut second =
                    "Rename, add or delete before the files are sent; nothing is written yet."
                        .to_string();
                if scheme.dropped > 0 {
                    second = format!(
                        "{} {} left out by the AI. Nothing is written yet.",
                        scheme.dropped,
                        if scheme.dropped == 1 {
                            "row was"
                        } else {
                            "rows were"
                        }
                    );
                }
                (
                    line,
                    second,
                    vec![
                        (IHit::SortStart, format!("Sort {total} files"), true),
                        (IHit::SortBack, "Keep the folders".to_string(), false),
                    ],
                    problem,
                )
            }
            SortStage::Filing {
                done,
                total,
                in_flight,
                ..
            } => (
                format!(
                    "Request {} of {total} answered\u{2026}",
                    (*done).min(*total)
                ),
                if *in_flight > 1 {
                    format!(
                        "{} requests in flight \u{00B7} only each file's path, title, start and size are sent.",
                        in_flight
                    )
                } else {
                    "Only each file's path, title, start and size are sent.".to_string()
                },
                vec![(IHit::SortStop, "Stop".to_string(), false)],
                None,
            ),
            SortStage::Filed {
                requests,
                placed,
                unsorted,
                stopped,
            } => {
                let sections = self
                    .plan
                    .notes
                    .iter()
                    .filter(|n| !n.unsorted && !n.already && n.confidence.is_some())
                    .map(|n| n.sections.join("\u{1f}"))
                    .collect::<std::collections::BTreeSet<_>>()
                    .len();
                let line = match stopped {
                    Some(_) => format!(
                        "{placed} placed in {sections} sections \u{00B7} {unsorted} unsorted \u{00B7} {requests} requests"
                    ),
                    None => format!(
                        "The AI placed {placed} files in {sections} sections in {requests} requests \u{00B7} {unsorted} unsorted"
                    ),
                };
                let second = match stopped {
                    Some(why) => format!(
                        "{why} The files it did not reach keep their folders; Add is still one line per ticked file."
                    ),
                    None => "Tick only the sure ones keeps the 0.6 and over. Nothing is written until Add."
                        .to_string(),
                };
                let problem = self.problem();
                (line, second, vec![add, cancel], problem)
            }
            SortStage::Failed(why) => (
                why.clone(),
                "Nothing was written: the files would still be added under their folders."
                    .to_string(),
                vec![
                    (IHit::AskAi, "Try again".to_string(), true),
                    (IHit::SortBack, "Keep the folders".to_string(), false),
                ],
                None,
            ),
            SortStage::Off => {
                let mut line = format!(
                    "{} of {} ticked",
                    self.plan.ticked(),
                    count(self.plan.notes.len(), "file", "files")
                );
                let already = self.plan.already();
                if already > 0 {
                    line.push_str(&format!(" \u{00B7} {already} already in the library"));
                }
                if self.can_ask() && self.ai_estimate.requests > 0 {
                    let more = self
                        .plan
                        .notes
                        .iter()
                        .filter(|n| !n.already)
                        .count()
                        .saturating_sub(self.ai_estimate.files);
                    line.push_str(&format!(
                        " \u{00B7} the AI would send {} in {} requests",
                        size(self.ai_estimate.bytes),
                        self.ai_estimate.requests
                    ));
                    if more > 0 {
                        line.push_str(&format!(
                            ", the first {} files; {} keep their folders",
                            self.ai_estimate.files, more
                        ));
                    }
                }
                let file = self
                    .plan
                    .group_file
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let second = if self.plan.ticked() == 0 {
                    "Writes one line per ticked file in the group's library file. Nothing is copied, moved or changed.".to_string()
                } else {
                    format!(
                        "Writes {} in {file}. Nothing is copied, moved or changed.",
                        count(self.plan.ticked(), "line", "lines")
                    )
                };
                let problem = self.problem();
                (line, second, vec![add, cancel], problem)
            }
        }
    }
}

/// Bytes as the card says them: "912 KB", "1.0 MB".
pub fn size(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{} KB", bytes / 1_000)
    } else {
        format!("{bytes} bytes")
    }
}

/// `text` broken into lines that fit `max` pixels, at spaces, for the dim reason lines.
fn wrap(p: &mut Painter, face: Face, size: f32, text: &str, max: f32) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let try_line = if line.is_empty() {
            word.to_string()
        } else {
            format!("{line} {word}")
        };
        if p.measure(face, size, &try_line) <= max || line.is_empty() {
            line = try_line;
        } else {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}
