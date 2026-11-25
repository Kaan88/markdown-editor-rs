//! Drawing the Markdown note card (`note_ui.rs` holds its state and events). The backdrop, the
//! card, the switch and the buttons are the ones the plan and move cards draw with, so the three
//! read as one family; the changes list is drawn as the AI review draws it.

use crate::note_ui::Row;
use crate::note_ui::{Geo, Hit, LINE_H, NoteCard, Phase, View};
use mdshared_ui::gfx::{Face, Painter, Rect, mix};
use mdshared_ui::overlay::{
    DIALOG_FOOTER, DIALOG_PAD, dialog_button_w, draw_backdrop, draw_dialog_button, draw_dialog_card,
};
use mdshared_ui::scroll::draw_scrollbar;
use mdshared_ui::segments::{SEGMENT_H, draw_segments, switch_width};
use mdshared_ui::theme::Palette;

const PAD: f32 = DIALOG_PAD;

/// The colours of removed and added lines, and their marks, as the AI review has them.
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

/// `text` cut from the front to fit `max`, with "…" where it was cut: a path keeps its end, the
/// folder and the file's name, which is what tells two paths apart.
fn fit_end(p: &mut Painter, face: Face, size: f32, text: &str, max: f32) -> String {
    if p.measure(face, size, text) <= max {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
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

fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

impl NoteCard {
    /// Draws the whole card over a `w` x `h` window.
    pub fn draw(&mut self, p: &mut Painter, pal: &Palette, w: f32, h: f32) {
        self.win = (w, h);
        self.spots.clear();
        if self.phase == Phase::Changes {
            self.scroll = self.rows().clamp_scroll(self.scroll);
        }
        draw_backdrop(p, pal, w, h);
        let g = self.geo();
        let footer = draw_dialog_card(p, pal, g.card);
        self.draw_header(p, pal, &g);
        match self.phase {
            Phase::Ready => self.draw_ready(p, pal, &g),
            Phase::Asking => self.draw_asking(p, pal, &g),
            Phase::Changes => self.draw_list(p, pal, &g),
        }
        self.draw_footer(p, pal, &g, footer);
    }

    fn hovered(&self, hit: Hit) -> bool {
        self.hover == Some(hit)
    }

    fn button(
        &mut self,
        p: &mut Painter,
        pal: &Palette,
        r: Rect,
        hit: Hit,
        label: &str,
        primary: bool,
    ) {
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

    fn draw_header(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let x = g.card.x + PAD;
        let max = (g.card.w - 2.0 * PAD).max(0.0);
        let y = g.header.y;
        let title = format!("A Markdown note from {}", self.source.name);
        p.text(Face::Semibold, 17.0, x, y + 30.0, &title, pal.text, max);
        let mut sub = format!(
            "{} \u{00B7} {} \u{00B7} goes to {} \u{00B7} {}",
            count(self.source.lines, "line", "lines"),
            crate::plan_ui::size_text(self.source.text.len()),
            self.target.group,
            self.target
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
        if let Some(enc) = &self.source.encoding {
            sub.push_str(&format!(" \u{00B7} {enc}"));
        }
        let sub = fit_end(p, Face::Ui, 12.5, &sub, max);
        p.text(Face::Ui, 12.5, x, y + 54.0, &sub, pal.muted, max);
        // The two views of an answer, when there is one.
        if self.phase == Phase::Changes {
            let counts_r = format!("+ {}", count(self.added, "line", "lines"));
            let counts_l = format!("\u{2212} {}", count(self.removed, "line", "lines"));
            let (_, _, minus, plus) = tints(pal);
            let cy = y + 76.0;
            let mut right = g.card.right() - PAD;
            let wr = p.measure(Face::Semibold, 13.0, &counts_r);
            p.text_right(Face::Semibold, 13.0, right, cy, &counts_r, plus);
            right -= wr + 14.0;
            let wl = p.measure(Face::Semibold, 13.0, &counts_l);
            p.text_right(Face::Semibold, 13.0, right, cy, &counts_l, minus);
            right -= wl + 18.0;
            let labels = ["Changes", "Result"];
            let sw_w = switch_width(p, &labels);
            let sw = Rect::new(right - sw_w, cy - SEGMENT_H / 2.0, sw_w, SEGMENT_H);
            let hovered = [Hit::Changes, Hit::Result]
                .iter()
                .position(|h| self.hover == Some(*h));
            let segs = draw_segments(
                p,
                pal,
                sw,
                &labels,
                Some(usize::from(self.view == View::Result)),
                hovered,
            );
            self.spots.push((Hit::Changes, segs[0]));
            self.spots.push((Hit::Result, segs[1]));
            let note = if self.view == View::Result {
                "The note as it will be written."
            } else {
                "What changes to make the note."
            };
            p.text(
                Face::Ui,
                12.0,
                x,
                cy,
                note,
                pal.muted,
                (sw.x - 16.0 - x).max(0.0),
            );
        }
        p.fill(
            Rect::new(
                g.card.x + 1.0,
                g.header.bottom() - p.px(),
                g.card.w - 2.0,
                p.px(),
            ),
            pal.popup_line,
        );
    }

    /// Before anything is sent: what would be, and where the note would go.
    fn draw_ready(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let x = g.card.x + PAD + 8.0;
        let max = (g.card.w - 2.0 * (PAD + 8.0)).max(0.0);
        let mut y = g.body.y + 22.0;
        p.text(Face::Semibold, 11.5, x, y, "WHAT IS SENT", pal.faint, max);
        y += 26.0;
        let sent = format!(
            "The whole file, in one request of {} with the instruction, to {}.",
            crate::plan_ui::size_text(self.prompt.len()),
            self.via
        );
        p.text(Face::Ui, 13.5, x, y, &sent, pal.text, max);
        y += 30.0;
        p.text(
            Face::Semibold,
            11.5,
            x,
            y,
            "WHAT IT IS ASKED TO DO",
            pal.faint,
            max,
        );
        y += 22.0;
        let h = p.paragraph(Face::Ui, 13.0, x, y, max, mdedit_core::ai::FORMAT, pal.text);
        y += h + 30.0;
        p.text(
            Face::Semibold,
            11.5,
            x,
            y,
            "WHAT IS WRITTEN",
            pal.faint,
            max,
        );
        y += 22.0;
        let goes = format!(
            "{} \u{2014} a new note in {}. {} is not changed, and nothing is written over.",
            self.target.path.display(),
            self.target.group,
            self.source.name
        );
        p.paragraph(Face::Ui, 13.0, x, y, max, &goes, pal.text);
        if let Some(notice) = &self.notice {
            let band = Rect::new(x - 8.0, g.body.bottom() - 56.0, max + 16.0, 40.0);
            p.rbox(band, 6.0, pal.warn_bg);
            p.icon(
                mdshared_ui::glyph::WARN,
                13.0,
                band.x + 22.0,
                band.cy(),
                pal.warn,
            );
            p.text(
                Face::Ui,
                12.5,
                band.x + 40.0,
                band.cy(),
                notice,
                pal.text,
                band.w - 52.0,
            );
        }
    }

    fn draw_asking(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let x = g.card.x + PAD + 8.0;
        let max = (g.card.w - 2.0 * (PAD + 8.0)).max(0.0);
        let chars = self.asking.as_ref().map(|a| a.chars).unwrap_or(0);
        let (line, dim) = if chars == 0 {
            (
                format!("Asking {} for the note\u{2026}", self.via),
                "The whole file is on its way; nothing is written until you say so.".to_string(),
            )
        } else {
            (
                format!("Asking {}: {chars} characters so far\u{2026}", self.via),
                "Nothing is written until you say so.".to_string(),
            )
        };
        p.text(
            Face::Semibold,
            15.0,
            x,
            g.body.y + 34.0,
            &line,
            pal.text,
            max,
        );
        p.text(Face::Ui, 12.5, x, g.body.y + 62.0, &dim, pal.muted, max);
        p.text(
            Face::Ui,
            12.5,
            x,
            g.body.y + 92.0,
            "Esc stops the request; the card stays, and nothing of the file has changed.",
            pal.muted,
            max,
        );
    }

    /// The changes, or the note as it will be written.
    fn draw_list(&mut self, p: &mut Painter, pal: &Palette, g: &Geo) {
        let body = Rect::new(g.body.x, g.body.y + 6.0, g.body.w, g.body.h - 10.0);
        let rows = self.rows();
        let (minus_bg, plus_bg, minus, plus) = tints(pal);
        let old = p.clip(body);
        match self.view {
            View::Changes => {
                let num_w = 44.0;
                let text_x = body.x + 2.0 * num_w + 28.0;
                let first = (self.scroll / LINE_H).floor().max(0.0) as usize;
                for (k, line) in self.rows.iter().enumerate().skip(first) {
                    let y = body.y + 8.0 + k as f32 * LINE_H - self.scroll;
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
                        Row::Same {
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
                        Row::Removed { old: a, text } => {
                            p.fill(row, minus_bg);
                            num(p, 0, *a);
                            p.text_center(
                                Face::Semibold,
                                13.0,
                                Rect::new(body.x + 2.0 * num_w + 2.0, y, 24.0, LINE_H),
                                "\u{2212}",
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
                        Row::Added { new: b, text } => {
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
                        Row::Fold { lines } => {
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
                                &format!("\u{00B7}\u{00B7}\u{00B7} {lines} unchanged line{s}"),
                                pal.muted,
                                body.right() - text_x - 12.0,
                            );
                        }
                    }
                }
            }
            View::Result => {
                // The note's own lines, with their numbers: blank lines included, so the note
                // reads as the file it will become.
                let num_w = 44.0;
                let text_x = body.x + num_w + 16.0;
                let first = (self.scroll / LINE_H).floor().max(0.0) as usize;
                for (k, text) in self.result.lines().enumerate().skip(first) {
                    let y = body.y + 8.0 + k as f32 * LINE_H - self.scroll;
                    if y > body.bottom() {
                        break;
                    }
                    p.text_right(
                        Face::Ui,
                        11.5,
                        body.x + num_w - 4.0,
                        y + LINE_H / 2.0,
                        &(k + 1).to_string(),
                        pal.faint,
                    );
                    p.text(
                        Face::Ui,
                        13.0,
                        text_x,
                        y + LINE_H / 2.0,
                        text,
                        pal.text,
                        body.right() - text_x - 16.0,
                    );
                }
            }
        }
        p.restore(old);
        if let Some((track, thumb)) = rows
            .thumb(body)
            .filter(|_| rows.content() > rows.view + 1.0)
        {
            draw_scrollbar(
                p,
                pal,
                track,
                thumb,
                self.drag.is_some()
                    || self.hover == Some(Hit::Thumb)
                    || self.hover == Some(Hit::Track),
            );
            self.spots.push((Hit::Thumb, thumb));
            self.spots.push((Hit::Track, track));
        }
    }

    fn draw_footer(&mut self, p: &mut Painter, pal: &Palette, g: &Geo, footer: Rect) {
        let x = g.card.x + PAD;
        let by = footer.y + (DIALOG_FOOTER - 32.0) / 2.0;
        let mut bx = g.card.right() - PAD;
        let (primary, label) = match self.phase {
            Phase::Ready => (Some(Hit::Primary), "Make the note"),
            Phase::Asking => (Some(Hit::Stop), "Stop"),
            Phase::Changes => (Some(Hit::Primary), "Write the note"),
        };
        if let Some(hit) = primary {
            let bw = dialog_button_w(p, label);
            bx -= bw;
            let r = Rect::new(bx, by, bw, 32.0);
            let enabled = self.problem().is_none() || hit == Hit::Stop;
            if enabled {
                self.button(p, pal, r, hit, label, true);
            } else {
                p.frame(r, 6.0, pal.field, pal.line);
                p.text_center(Face::Semibold, 13.0, r, label, pal.faint);
            }
            bx -= 10.0;
        }
        let label = "Cancel";
        let bw = dialog_button_w(p, label);
        bx -= bw;
        let r = Rect::new(bx, by, bw, 32.0);
        self.button(p, pal, r, Hit::Cancel, label, false);

        // The footer's sentence, or why the button cannot be pressed.
        let text = if let Some(why) = &self.blocked {
            why.clone()
        } else if let Some(notice) = &self.notice {
            notice.clone()
        } else {
            match self.phase {
                Phase::Ready => format!(
                    "Writes {} and lists it in {}. {} is not changed.",
                    self.target.path.display(),
                    self.target.group,
                    self.source.name
                ),
                Phase::Asking => "Esc stops the request.".to_string(),
                Phase::Changes => format!(
                    "{} lines removed, {} added. Writes {} \u{2014} {} is not changed.",
                    self.removed,
                    self.added,
                    self.target.path.display(),
                    self.source.name
                ),
            }
        };
        let warn = self.blocked.is_some() || (self.notice.is_some() && self.phase != Phase::Ready);
        p.text(
            Face::Ui,
            12.0,
            x,
            footer.cy(),
            &text,
            if warn { pal.warn } else { pal.muted },
            (bx - x - 12.0).max(0.0),
        );
    }
}
