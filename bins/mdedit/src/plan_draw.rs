//! Drawing the plan screen (`plan_ui.rs` holds its state and events). The card is
//! `mdshared_ui::overlay`'s; the tick boxes, the level switch, the list's arithmetic and the
//! scrollbars are the shared widgets. Controls whose size depends on their text record their
//! rectangle in `PlanScreen::spots` as they are drawn; the rest is found from the geometry in
//! `plan_ui::geo`, so a hit and a drawing cannot disagree about where a row is.

use crate::chrome::glyph;
use crate::plan_ui::{
    Edit, EditKind, GUTTER, Geo, LINE_H, PHit, Pane, PlanScreen, STRIP_H, lines_text, separated,
    size_text, thousands,
};
use mdshared_app::library::folder_name;
use mdshared_ui::check::{TICK, Tick, draw_tick};
use mdshared_ui::gfx::{Face, Painter, Rect, Rgb, mix};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button, draw_dialog_card,
};
use mdshared_ui::scroll::draw_scrollbar;
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::textfield::FieldColors;
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;
const MONO: f32 = 12.5;

/// The colour of a line the app adds, and its mark, in each theme (the review's).
fn added_tint(pal: &Palette) -> (Rgb, Rgb) {
    if pal.dark {
        ([0x1f, 0x3d, 0x29], [0x7f, 0xd4, 0x97])
    } else {
        ([0xe6, 0xf4, 0xea], [0x1a, 0x7f, 0x37])
    }
}

/// A line of the preview as it is drawn: no line break, a tab as four spaces, no control
/// character.
pub fn shown(text: &str) -> String {
    let body = text.trim_end_matches(['\r', '\n']);
    let mut out = String::with_capacity(body.len());
    for c in body.chars() {
        match c {
            '\t' => out.push_str("    "),
            c if c.is_control() => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// The colours of the strip: a ticked note, no note, and the selected one.
pub fn strip_colors(pal: &Palette) -> (Rgb, Rgb, Rgb) {
    (
        mix(pal.popup, pal.accent, 0.55),
        mix(pal.popup, pal.text, 0.12),
        pal.accent,
    )
}

/// The disabled button, as `replace_ui::dialog_button` draws it: a plain frame, whether it is
/// primary or not. (The shared `draw_dialog_button_disabled` differs: radius 5, and a tinted fill
/// for a primary.)
fn disabled_button(p: &mut Painter, pal: &Palette, r: Rect, label: &str) {
    p.frame(r, 6.0, pal.field, pal.line);
    p.text_center(Face::Semibold, 13.0, r, label, pal.faint);
}

impl PlanScreen {
    /// Draws the whole screen over a `w` x `h` window.
    pub fn draw(&mut self, p: &mut Painter, pal: &Palette, w: f32, h: f32, caret_on: bool) {
        self.win = (w, h);
        self.scale = 1.0 / p.px();
        self.spots.clear();
        let rows = self.rows();
        self.scroll = rows.clamp_scroll(self.scroll);
        let prows = self.prows();
        self.pscroll = prows.clamp_scroll(self.pscroll);
        draw_backdrop(p, pal, w, h);
        let g = self.geo();
        let footer = draw_dialog_card(p, pal, g.card);
        self.draw_header(p, pal, &g, caret_on);
        self.draw_notice(p, pal, &g);
        self.draw_cover(p, pal, &g);
        self.draw_list(p, pal, &g, caret_on);
        if g.preview.is_some() {
            self.draw_preview(p, pal, &g);
        }
        self.draw_footer(p, pal, &g, footer);
        if let Some(pop) = self.menu.as_mut() {
            pop.menu.draw_in(p, pal, Rect::new(0.0, 0.0, w, h));
        }
    }

    fn hovered(&self, hit: PHit) -> bool {
        self.hover == Some(hit)
    }

    // ---- the header -------------------------------------------------------------------------------

    fn draw_header(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, caret_on: bool) {
        let inner = g.card.w - 2.0 * PAD;
        let gap = 28.0;
        let usable = (inner - 2.0 * gap).max(0.0);
        let (c1, c2) = (usable * 0.30, usable * 0.36);
        let c3 = (usable - c1 - c2).max(0.0);
        let x1 = g.card.x + PAD;
        let x2 = x1 + c1 + gap;
        let x3 = x2 + c2 + gap;
        let y = g.header.y;
        let label = |p: &mut Painter, x: f32, text: &str, max: f32| {
            p.text(Face::Semibold, 11.0, x, y + 24.0, text, pal.muted, max);
        };

        // The source document.
        label(p, x1, "SOURCE DOCUMENT", c1);
        p.text(
            Face::Semibold,
            14.0,
            x1,
            y + 48.0,
            &self.source.name,
            pal.text,
            c1,
        );
        let detail = format!(
            "{} \u{00B7} {}",
            lines_text(self.source.lines),
            size_text(self.source.bytes)
        );
        p.text(Face::Ui, 12.5, x1, y + 70.0, &detail, pal.muted, c1);
        if let Some(enc) = &self.source.encoding {
            p.text(Face::Ui, 12.0, x1, y + 88.0, enc, pal.muted, c1);
        }

        // How it was cut: the level switch.
        label(p, x2, "HOW IT WAS CUT", c2);
        let cy = y + 54.0;
        let at = p.text(Face::Ui, 13.0, x2, cy, "at", pal.muted, 40.0);
        let labels = ["#", "##", "###"];
        let sw = switch_width(p, &labels);
        let sr = Rect::new(x2 + at + 8.0, cy - SEGMENT_H / 2.0, sw, SEGMENT_H);
        let selected = mdedit_core::split::SPLIT_LEVELS
            .iter()
            .position(|&l| l == self.plan.level());
        let hovered = (0..labels.len()).find(|&k| self.hovered(PHit::Level(k)));
        let rects = draw_segments(p, pal, sr, &labels, selected, hovered);
        for (k, r) in rects.into_iter().enumerate() {
            self.spots.push((PHit::Level(k), r));
        }
        p.text(
            Face::Ui,
            13.0,
            sr.right() + 8.0,
            cy,
            "headings",
            pal.muted,
            (x2 + c2 - sr.right() - 8.0).max(0.0),
        );
        let cut = format!(
            "{} \u{00B7} {} at this level",
            if self.plan.notes().len() == 1 {
                "1 note".to_string()
            } else {
                format!("{} notes", thousands(self.plan.notes().len()))
            },
            if self.levels.count(self.plan.level()) == 1 {
                "1 heading".to_string()
            } else {
                format!(
                    "{} headings",
                    thousands(self.levels.count(self.plan.level()))
                )
            }
        );
        p.text(Face::Ui, 12.5, x2, y + 82.0, &cut, pal.muted, c2);

        // Where the notes go.
        label(p, x3, "THE NOTES GO TO", c3);
        let b = Rect::new(x3, y + 38.0, c3, 44.0);
        let editing = matches!(self.edit.as_ref().map(|e| e.kind), Some(EditKind::Group));
        if editing {
            if let Some(e) = self.edit.as_mut() {
                draw_field(
                    p,
                    pal,
                    e,
                    b,
                    "Name of the new group",
                    caret_on,
                    &mut self.spots,
                );
            }
        } else {
            let lit = self.hovered(PHit::Dest) || self.menu.is_some() && self.pressed.is_none();
            p.frame(
                b,
                6.0,
                if lit {
                    mix(pal.field, pal.text, 0.04)
                } else {
                    pal.field
                },
                if lit { pal.accent } else { pal.popup_line },
            );
            let inner_w = (b.w - 40.0).max(0.0);
            let name_w = p.text(
                Face::Semibold,
                13.0,
                b.x + 12.0,
                b.y + 15.0,
                &self.dest.name,
                pal.text,
                inner_w,
            );
            if self.dest.is_new {
                p.text(
                    Face::Ui,
                    12.5,
                    b.x + 12.0 + name_w + 6.0,
                    b.y + 15.0,
                    "\u{00B7} a new group",
                    pal.muted,
                    (inner_w - name_w - 6.0).max(0.0),
                );
            }
            p.text(
                Face::Ui,
                12.0,
                b.x + 12.0,
                b.y + 32.0,
                &self.dest_text(),
                pal.faint,
                inner_w,
            );
            p.icon(glyph::DOWN, 10.0, b.right() - 18.0, b.cy(), pal.muted);
            self.spots.push((PHit::Dest, b));
        }
        p.fill(
            Rect::new(g.card.x, g.header.bottom(), g.card.w, p.px()),
            pal.popup_line,
        );
    }

    fn draw_notice(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let (Some(r), Some(n)) = (g.notice, self.notice.clone()) else {
            return;
        };
        let fill = if n.warn {
            pal.warn_bg
        } else {
            mix(pal.popup, pal.accent, 0.07)
        };
        p.fill(Rect::new(r.x + 1.0, r.y, r.w - 2.0, r.h), fill);
        let tint = if n.warn { pal.warn } else { pal.accent };
        p.icon(
            if n.warn { glyph::WARN } else { glyph::INFO },
            14.0,
            r.x + PAD + 7.0,
            r.cy(),
            tint,
        );
        let show_w = if n.show.is_some() {
            p.measure(Face::Semibold, 12.5, "Show") + 12.0
        } else {
            0.0
        };
        let text_w = p.text(
            Face::Ui,
            12.5,
            r.x + PAD + 24.0,
            r.cy(),
            &n.text,
            pal.text,
            (r.w - 2.0 * PAD - 24.0 - show_w - 8.0).max(0.0),
        );
        if n.show.is_some() {
            let link = Rect::new(r.x + PAD + 24.0 + text_w + 8.0, r.y, show_w, r.h);
            let on = self.hovered(PHit::Show);
            p.text(
                Face::Semibold,
                12.5,
                link.x + 6.0,
                link.cy(),
                "Show",
                if on { pal.text } else { pal.accent },
                show_w,
            );
            self.spots.push((PHit::Show, link));
        }
        p.fill(
            Rect::new(r.x, r.bottom() - p.px(), r.w, p.px()),
            pal.popup_line,
        );
    }

    // ---- the coverage strip -------------------------------------------------------------------------

    fn draw_cover(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let c = g.cover;
        p.text(
            Face::Semibold,
            11.0,
            c.x + PAD,
            c.y + 17.0,
            "COVERAGE",
            pal.muted,
            120.0,
        );
        let (ticked, track, selected) = strip_colors(pal);
        // The legend, from the right.
        let mut x = c.right() - PAD;
        for (color, text) in [
            (selected, "selected"),
            (track, "in no note"),
            (ticked, "in a ticked note"),
        ] {
            let w = p.measure(Face::Ui, 11.5, text);
            x -= w;
            p.text(Face::Ui, 11.5, x, c.y + 17.0, text, pal.muted, w + 2.0);
            x -= 16.0;
            p.rbox(Rect::new(x, c.y + 12.0, 10.0, 10.0), 2.0, color);
            x -= 18.0;
        }

        let (sx0, sy0, sx1, sy1) = p.dev_rect(g.strip);
        let width = (sx1 - sx0).max(0) as f32;
        self.bands_for(width);
        p.fill_dev(sx0, sy0, sx1, sy1, track, 1.0);
        let rest_selected = self.is_rest(self.sel);
        for b in &self.bands {
            let on = if b.note {
                b.row == self.sel
            } else {
                rest_selected
            };
            let color = if on {
                Some(selected)
            } else if b.note && b.ticked {
                Some(ticked)
            } else if rest_selected && b.note && !b.ticked {
                Some(selected)
            } else {
                None
            };
            if let Some(color) = color {
                p.fill_dev(sx0 + b.x0 as i32, sy0, sx0 + b.x1 as i32, sy1, color, 1.0);
            }
        }
        if separated(&self.bands) {
            for b in self.bands.iter().filter(|b| b.note && b.x0 > 0.0) {
                let x = sx0 + b.x0 as i32;
                p.fill_dev(x, sy0, x + 1, sy1, pal.popup, 1.0);
            }
        }
        debug_assert!(g.strip.h == STRIP_H);
        p.fill(
            Rect::new(c.x, c.bottom() - p.px(), c.w, p.px()),
            pal.popup_line,
        );
    }

    // ---- the list ---------------------------------------------------------------------------------------

    fn draw_list(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, caret_on: bool) {
        let head = g.list_head;
        p.text(
            Face::Semibold,
            11.0,
            head.x + PAD,
            head.cy(),
            &format!("NOTES ({})", thousands(self.plan.notes().len())),
            pal.muted,
            160.0,
        );
        let mut right = head.right() - 16.0;
        for (hit, label) in [(PHit::UntickAll, "Untick all"), (PHit::TickAll, "Tick all")] {
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
        let notes = self.plan.notes().len();
        let old = p.clip(g.list);
        let mut edit_at: Option<Rect> = None;
        for i in rows.visible() {
            let rr = rows.rect(g.list, i);
            let row = Rect::new(rr.x + 8.0, rr.y + 2.0, rr.w - 16.0, rr.h - 4.0);
            let rest = i >= notes;
            if rest && i > 0 {
                p.fill(
                    Rect::new(rr.x + 16.0, rr.y, rr.w - 32.0, p.px()),
                    pal.popup_line,
                );
            }
            let hot = matches!(self.hover, Some(PHit::Row(k) | PHit::Tick(k)) if k == i);
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
            let ticked = self.ticked(i);
            draw_tick(
                p,
                pal,
                Rect::new(row.x + 14.0, row.cy() - TICK / 2.0, TICK, TICK),
                Tick::new(ticked),
            );
            let editing = match self.edit.as_ref().map(|e| e.kind) {
                Some(EditKind::Rename(k) | EditKind::Section(k)) => k == i,
                _ => false,
            };
            if editing {
                edit_at = Some(Rect::new(
                    row.x + 38.0,
                    row.y + 4.0,
                    (row.w - 38.0 - 10.0).max(0.0),
                    row.h - 8.0,
                ));
                continue;
            }
            self.draw_row(p, pal, row, i, ticked);
        }
        if let (Some(e), Some(r)) = (self.edit.as_mut(), edit_at) {
            let hint = match e.kind {
                EditKind::Section(_) => "Name of the new section",
                _ => "Name of the note",
            };
            draw_field(p, pal, e, r, hint, caret_on, &mut self.spots);
        }
        p.restore(old);
        if let Some((track, thumb)) = rows.thumb(g.list) {
            let active = self.hovered(PHit::Thumb(Pane::List));
            draw_scrollbar(p, pal, track, thumb, active);
            self.spots.push((PHit::Track(Pane::List), track));
            self.spots.push((PHit::Thumb(Pane::List), thumb));
        }
    }

    fn draw_row(&self, p: &mut Painter, pal: &Palette, row: Rect, i: usize, ticked: bool) {
        let x = row.x + 42.0;
        let (c1, c2) = (row.y + 15.0, row.y + 32.0);
        let (title, sub, top, bottom): (String, String, String, String) = match self.plan.note(i) {
            Some(n) => (
                n.title().to_string(),
                n.sections().join(" \u{203A} "),
                format!("lines {}-{}", n.first_line(), n.last_line()),
                match self.mark(i) {
                    Some(m) => m.text(),
                    None => format!(
                        "{} \u{00B7} {}",
                        lines_text(n.lines()),
                        size_text(n.bytes())
                    ),
                },
            ),
            None => {
                let r = &self.rest;
                let places = r.runs().len();
                (
                    r.title().to_string(),
                    if r.lines() == 0 {
                        "Every line is in a ticked note".to_string()
                    } else {
                        format!(
                            "{} in {}",
                            lines_text(r.lines()),
                            if places == 1 {
                                "1 place".to_string()
                            } else {
                                format!("{places} places")
                            }
                        )
                    },
                    String::new(),
                    if ticked {
                        "written as one note".to_string()
                    } else {
                        "stays in the original".to_string()
                    },
                )
            }
        };
        let dim = if self.mark_is_dim(i) {
            pal.faint
        } else {
            pal.muted
        };
        let wt = p.measure(Face::Ui, 12.0, &top);
        let wb = p.measure(Face::Ui, 12.0, &bottom);
        let side = wt.max(wb).min(row.w * 0.45);
        let text_w = (row.right() - 14.0 - side - 16.0 - x).max(0.0);
        p.text(
            Face::Semibold,
            13.5,
            x,
            c1,
            &title,
            if ticked { pal.text } else { pal.muted },
            text_w,
        );
        if !sub.is_empty() {
            p.text(Face::Ui, 12.0, x, c2, &sub, pal.muted, text_w);
        }
        if !top.is_empty() {
            p.text_right(Face::Ui, 12.0, row.right() - 14.0, c1, &top, pal.faint);
        }
        let bw = p.measure(Face::Ui, 12.0, &bottom).min(side);
        p.text(
            Face::Ui,
            12.0,
            row.right() - 14.0 - bw,
            c2,
            &bottom,
            dim,
            side,
        );
    }

    /// A row whose second right-hand text is a mark is in the dim colour; a size is not.
    fn mark_is_dim(&self, i: usize) -> bool {
        self.plan.note(i).is_some() && self.mark(i).is_some()
    }

    // ---- the preview -------------------------------------------------------------------------------------

    fn draw_preview(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let Some(pv) = g.preview else { return };
        p.fill(Rect::new(pv.x, pv.y, p.px(), pv.h), pal.popup_line);
        let head = g.preview_head;
        let x = pv.x + PAD;
        let ticked = self.ticked(self.sel);
        let (title, second, third) = match self.plan.note(self.sel) {
            Some(n) => (
                n.title().to_string(),
                if n.sections().is_empty() {
                    "No section".to_string()
                } else {
                    n.sections().join(" \u{203A} ")
                },
                format!(
                    "\u{2192} {} \u{00B7} lines {}-{} of the source",
                    self.target_text(self.sel),
                    n.first_line(),
                    n.last_line()
                ),
            ),
            None => (
                self.rest.title().to_string(),
                format!(
                    "{} in {} that no ticked note holds",
                    lines_text(self.rest.lines()),
                    if self.rest.runs().len() == 1 {
                        "1 place".to_string()
                    } else {
                        format!("{} places", self.rest.runs().len())
                    }
                ),
                format!(
                    "\u{2192} {}{}.md",
                    self.dest_text(),
                    folder_name(self.rest.title())
                ),
            ),
        };
        let mut right = pv.right() - PAD;
        if !ticked {
            let w = p.measure(Face::Semibold, 10.5, "NOT TICKED") + 16.0;
            let r = Rect::new(right - w, head.y + 11.0, w, 20.0);
            p.frame(r, 5.0, pal.field, pal.popup_line);
            p.text_center(Face::Semibold, 10.5, r, "NOT TICKED", pal.muted);
            right = r.x - 10.0;
        }
        p.text(
            Face::Semibold,
            15.0,
            x,
            head.y + 22.0,
            &title,
            pal.text,
            (right - x).max(0.0),
        );
        p.text(
            Face::Ui,
            12.5,
            x,
            head.y + 43.0,
            &second,
            pal.muted,
            pv.right() - PAD - x,
        );
        p.text(
            Face::Ui,
            12.0,
            x,
            head.y + 60.0,
            &third,
            pal.faint,
            pv.right() - PAD - x,
        );
        p.fill(
            Rect::new(pv.x, head.bottom() - p.px(), pv.w, p.px()),
            pal.popup_line,
        );

        let rows = self.prows();
        let area = Rect::new(
            g.preview_body.x,
            g.preview_body.y + 4.0,
            g.preview_body.w,
            rows.view,
        );
        let (tint, mark) = added_tint(pal);
        let color = if ticked { pal.text } else { pal.muted };
        let old = p.clip(Rect::new(
            g.preview_body.x + 1.0,
            g.preview_body.y,
            g.preview_body.w - 1.0,
            g.preview_body.h,
        ));
        if self.preview.is_empty() {
            p.text_center(
                Face::Ui,
                12.5,
                g.preview_body,
                "Nothing is outside the ticked notes.",
                pal.faint,
            );
        }
        for k in rows.visible() {
            let Some(line) = self.preview.get(k) else {
                break;
            };
            let r = rows.rect(area, k);
            if line.added {
                p.fill(Rect::new(pv.x + 1.0, r.y, pv.w - 1.0, LINE_H), tint);
            }
            let gx = pv.x + GUTTER - 14.0;
            if line.added {
                p.text_right(Face::Mono, 11.5, gx, r.cy(), "+", mark);
            } else if let Some(n) = line.source {
                p.text_right(Face::Mono, 11.5, gx, r.cy(), &n.to_string(), pal.faint);
            }
            let text = shown(&line.text);
            let mut tx = pv.x + GUTTER + 6.0;
            let max = (pv.right() - 12.0 - tx).max(0.0);
            if line.removed > 0 {
                let hashes = "#".repeat(usize::from(line.removed));
                let w = p.text(Face::Mono, MONO, tx, r.cy(), &hashes, pal.faint, max);
                p.strikethrough_range(
                    Face::Mono,
                    MONO,
                    tx,
                    r.cy(),
                    &hashes,
                    0..hashes.len(),
                    pal.text,
                );
                tx += w + 4.0;
            }
            p.text(
                Face::Mono,
                MONO,
                tx,
                r.cy(),
                &text,
                if line.added { mark } else { color },
                (pv.right() - 12.0 - tx).max(0.0),
            );
        }
        p.fill(
            Rect::new(pv.x + GUTTER, g.preview_body.y, p.px(), g.preview_body.h),
            pal.line,
        );
        p.restore(old);
        if let Some((track, thumb)) = rows.thumb(area) {
            let active = self.hovered(PHit::Thumb(Pane::Preview));
            draw_scrollbar(p, pal, track, thumb, active);
            self.spots.push((PHit::Track(Pane::Preview), track));
            self.spots.push((PHit::Thumb(Pane::Preview), thumb));
        }
    }

    // ---- the footer -----------------------------------------------------------------------------------------

    fn draw_footer(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, footer: Rect) {
        let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
        let mut x = g.card.right() - PAD;
        let mut buttons: Vec<(PHit, &str, bool, Rect)> = Vec::new();
        for (hit, label, primary) in [
            (PHit::Write, "Write the notes", true),
            (PHit::Cancel, "Cancel", false),
        ] {
            let bw = dialog_button_w(p, label);
            x -= bw;
            buttons.push((hit, label, primary, Rect::new(x, by, bw, 32.0)));
            x -= 8.0;
        }
        let problem = self.problem();
        for (hit, label, primary, r) in buttons {
            if hit == PHit::Write && problem.is_some() {
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
        let mut limit = x - 8.0;
        if let Some(reason) = &problem {
            let w = p
                .measure(Face::Ui, 12.5, reason)
                .min(limit - g.card.x - PAD - 160.0);
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
        let t = &self.totals;
        let mut line = format!(
            "{} \u{00B7} {} of {} lines",
            if t.notes == 1 {
                "1 note".to_string()
            } else {
                format!("{} notes", thousands(t.notes))
            },
            thousands(t.lines_in_notes),
            thousands(t.document_lines),
        );
        if t.lines_in_no_note > 0 {
            line.push_str(&format!(
                " \u{00B7} {} stay only in the original",
                lines_text(t.lines_in_no_note)
            ));
        }
        let left = g.card.x + PAD;
        let max = (limit - left).max(0.0);
        p.text(Face::Ui, 13.0, left, footer.y + 26.0, &line, pal.text, max);
        let what = format!(
            "Writes one .md per ticked note into {}. The source document is not changed.",
            self.dest_text()
        );
        p.text(Face::Ui, 12.0, left, footer.y + 48.0, &what, pal.muted, max);
    }
}

/// A text field in a framed box over a row or the group button.
fn draw_field(
    p: &mut Painter,
    pal: &Palette,
    e: &mut Edit,
    r: Rect,
    placeholder: &str,
    caret_on: bool,
    spots: &mut Vec<(PHit, Rect)>,
) {
    p.frame(r, 6.0, pal.field, pal.accent);
    let area = Rect::new(r.x + 10.0, r.y, (r.w - 20.0).max(0.0), r.h);
    if let Some(cx) = e.click.take() {
        let stops = p.caret_stops(Face::Ui, 13.5, e.field.text());
        e.field.press(cx - area.x, &stops, false);
    }
    let colors = FieldColors {
        text: pal.text,
        placeholder: pal.faint,
        selection: pal.selection,
        caret: pal.text,
    };
    e.field.draw(
        p,
        area,
        Face::Ui,
        13.5,
        &colors,
        placeholder,
        true,
        caret_on,
    );
    spots.push((PHit::Field, r));
}
