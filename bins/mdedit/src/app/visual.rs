//! Visual editing (Ctrl+E): a Markdown document shown as its page, the preview pane taking the
//! whole text area. A click on a block opens it: the renderer leaves room for it
//! (`mdview_preview` `Request::Gap`) and the document's own view is drawn there, scrolled to the
//! block's lines, so typing, undo, find and every command work as in the text. Esc, a click
//! elsewhere on the page or the caret leaving the block closes it. DESIGN.md section 70.

use super::App;
use crate::app::preview::previewable;
use mdedit_core::selection::Selection;
use mdedit_highlight::DocId;
use mdshared_ui::gfx::Rect;

/// Room above and below the block's lines inside the box, logical px.
pub(crate) const BOX_PAD: f32 = 10.0;
/// How far the box reaches out from the page's text column on each side, logical px.
const BOX_OUT: f32 = 10.0;
/// Space kept between the box and the blocks above and below it, logical px.
const BOX_GAP: f32 = 6.0;

/// The page's side margin and text column width (logical px), as mdview's renderer lays the
/// page out (`Theme::margin`, `Theme::reading_width`).
#[cfg(feature = "embedded-preview")]
const PAGE: (f32, f32) = (
    mdview_render::LIGHT.margin,
    mdview_render::LIGHT.reading_width,
);
#[cfg(not(feature = "embedded-preview"))]
const PAGE: (f32, f32) = (40.0, 700.0);

/// The box's fill: the page tinted toward the text, as mdview shades code.
pub(crate) fn box_fill(pal: &mdshared_ui::theme::Palette) -> mdshared_ui::gfx::Rgb {
    mdshared_ui::gfx::mix(pal.page, pal.text, 0.04)
}

/// Visual editing of one document.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Visual {
    pub doc: DocId,
    pub block: Option<Block>,
    /// A line (from 1) to ask the page's block of once it has the text.
    pub pending: Option<u32>,
}

/// The open block: its first line (from 0), and how many lines follow it to the end of the
/// text. Edits inside the block change neither, so its last line is always known.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Block {
    pub first: usize,
    pub tail: usize,
    /// The text's line count when the block opened: the page keeps that text while it is open.
    pub lines: usize,
}

impl Block {
    /// The block's last line (from 0) in a text of `lines` lines.
    pub(crate) fn last(&self, lines: usize) -> usize {
        lines.saturating_sub(1 + self.tail).max(self.first)
    }
}

/// The lines of a block the page reported, `first` to before `next` (from 1), as lines from 0
/// with the blank lines at its end left out. `None` when nothing is left.
pub(crate) fn trimmed(
    buffer: &mdedit_core::buffer::Buffer,
    first: u32,
    next: Option<u32>,
) -> Option<(usize, usize)> {
    let lines = buffer.len_lines();
    let first = (first as usize).checked_sub(1)?;
    // `next` counts from 1: the block ends on the line before it, `next - 2` from 0.
    let end = next.map_or(lines, |n| (n as usize).saturating_sub(1));
    let mut last = end.min(lines).checked_sub(1)?;
    let mut s = String::new();
    while last > first {
        s.clear();
        buffer.text(buffer.line_range(last), &mut s);
        if !s.trim().is_empty() {
            break;
        }
        last -= 1;
    }
    (first < lines && last >= first).then_some((first, last))
}

/// The end of line `line`, before its line break.
fn line_end(buffer: &mdedit_core::buffer::Buffer, line: usize) -> usize {
    let range = buffer.line_range(line);
    let mut s = String::new();
    buffer.text(range.clone(), &mut s);
    range.start + s.trim_end_matches(['\n', '\r']).len()
}

/// From `line`, the first line with text on it going up or down (`line` itself when it has
/// text, or when there is none that way).
fn next_text_line(buffer: &mdedit_core::buffer::Buffer, line: usize, up: bool) -> usize {
    let mut s = String::new();
    let mut at = line;
    loop {
        s.clear();
        buffer.text(buffer.line_range(at), &mut s);
        if !s.trim().is_empty() {
            return at;
        }
        match up {
            true if at > 0 => at -= 1,
            false if at + 1 < buffer.len_lines() => at += 1,
            _ => return line,
        }
    }
}

/// The offset near a click `down` and `across` its block (fractions of the block's height and
/// of the page's text column), in lines `first` to `last`: the line by height, the column by
/// width, on a character boundary.
fn near(
    buffer: &mdedit_core::buffer::Buffer,
    first: usize,
    last: usize,
    down: f32,
    across: f32,
) -> usize {
    let n = last - first + 1;
    let line = first + ((down.clamp(0.0, 1.0) * n as f32) as usize).min(n - 1);
    let start = buffer.line_range(line).start;
    let mut s = String::new();
    buffer.text(start..line_end(buffer, line), &mut s);
    let chars = s.chars().count();
    let k = (across.clamp(0.0, 1.0) * chars as f32).round() as usize;
    start + s.char_indices().nth(k).map_or(s.len(), |(i, _)| i)
}

impl App {
    /// Turns visual editing on for the active Markdown document, opening the preview pane for
    /// it, or off again.
    pub(super) fn toggle_visual(&mut self) {
        if self.visual.is_some() {
            self.end_visual();
            return;
        }
        let Some(doc) = self.docs.active_doc().filter(|d| previewable(d)) else {
            return;
        };
        let id = doc.id;
        if self.preview.is_none() {
            self.toggle_preview_pane();
        }
        if self.preview.is_none() {
            return;
        }
        self.visual = Some(Visual {
            doc: id,
            block: None,
            pending: None,
        });
        self.docs.visual_open = true;
        // The page starts where the text was.
        if let Some(p) = self.preview.as_mut() {
            p.editor_leads();
        }
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// Back to the text: the open block is closed and the pane stays as it was before.
    pub(super) fn end_visual(&mut self) {
        self.close_block();
        self.visual = None;
        self.docs.visual_open = false;
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// Visual editing is on for the active document.
    pub(super) fn visual_on(&self) -> bool {
        self.visual.is_some()
    }

    /// The open block, if any.
    pub(super) fn visual_block(&self) -> Option<Block> {
        self.visual.as_ref()?.block
    }

    /// Closes the open block: the page draws it again.
    pub(super) fn close_block(&mut self) {
        let Some(v) = self.visual.as_mut() else {
            return;
        };
        if v.block.take().is_some() {
            if let Some(p) = self.preview.as_mut() {
                p.set_gap(None);
            }
            self.relayout_areas();
            self.full = true;
            self.redraw();
        }
    }

    /// A press on the page at `x, y` (window, logical px) outside the open block: closes it;
    /// with none open, opens the block there.
    pub(super) fn visual_press(&mut self, x: f32, y: f32) {
        if self.visual_block().is_some() {
            self.close_block();
            return;
        }
        let Some(r) = self.areas.preview else {
            return;
        };
        let s = self.scale;
        let (px, py) = (((x - r.x) * s).max(0.0), ((y - r.y) * s).max(0.0));
        if let Some(p) = self.preview.as_mut() {
            p.block_at(px, py);
        }
    }

    /// Keeps visual editing in step, once per frame before the layout: ends it when its
    /// document is no longer the active one, opens the block the page named, follows edits in
    /// the open block (its last line, the room's height) and closes it when the caret left it.
    pub(super) fn visual_sync(&mut self) {
        let Some(v) = self.visual.clone() else {
            return;
        };
        let current = self
            .docs
            .active_doc()
            .is_some_and(|d| d.id == v.doc && previewable(d));
        if !current || self.preview.is_none() {
            self.end_visual();
            return;
        }
        let answer = self.preview.as_mut().and_then(|p| p.take_block());
        if let Some(Some(b)) = answer {
            self.open_block(b.first, b.next, b.at);
        }
        let Some(block) = self.visual_block() else {
            return;
        };
        let line_h = self.view.metrics(self.scale).line_h as f32;
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let lines = r.buffer.len_lines();
        let last = block.last(lines);
        let caret = r.buffer.line_of(r.view.sel.caret);
        if caret < block.first || caret > last {
            // The caret went out of the block (the arrows, a find): the block it went to opens,
            // over the blank lines between them.
            let up = caret < block.first;
            let target = next_text_line(&r.buffer, caret, up);
            let start = r.buffer.line_range(target).start;
            if target != caret {
                r.view.sel = Selection::at(if up {
                    line_end(&r.buffer, target)
                } else {
                    start
                });
            }
            self.close_block();
            // Asked once the page has the edited text (`preview_sync` sends it first).
            if let Some(v) = self.visual.as_mut() {
                v.pending = Some(target as u32 + 1);
            }
            return;
        }
        // The view shows the block's lines from its top, whatever scrolled it.
        crate::view::set_source_top(r, block.first as f64);
        r.view.left = 0.0;
        // Rows by the box's width once the view has been laid out in it; lines before that.
        let rows = if r.wrap && r.view.view_w >= 100.0 {
            (block.first..=last)
                .map(|l| crate::view::wrap::rows(r, l).len().max(1))
                .sum::<usize>() as f32
        } else {
            (last - block.first + 1) as f32
        };
        let s = self.scale;
        let height = rows * line_h + 2.0 * ((BOX_PAD * s).round() + (BOX_GAP * s).round());
        // In the page's lines: it keeps the text the block opened on until it closes.
        let page_last = block.last(block.lines);
        if let Some(p) = self.preview.as_mut() {
            p.set_gap(Some((block.first as u32 + 1, page_last as u32 + 1, height)));
        }
    }

    /// Opens the block the page reported (lines from 1). After a click (`at`, where in the
    /// block it was) the caret goes near it; otherwise it stays where it is, kept inside.
    fn open_block(&mut self, first: u32, next: Option<u32>, at: Option<(f32, f32)>) {
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let Some((first, last)) = trimmed(&r.buffer, first, next) else {
            return;
        };
        let lines = r.buffer.len_lines();
        let caret = match at {
            Some((down, across)) => near(&r.buffer, first, last, down, across),
            None => {
                let c = r.view.sel.caret;
                let line = r.buffer.line_of(c);
                if line < first {
                    r.buffer.line_range(first).start
                } else if line > last {
                    line_end(&r.buffer, last)
                } else {
                    c
                }
            }
        };
        r.view.sel = Selection::at(caret);
        r.view.extra.clear();
        crate::view::set_source_top(r, first as f64);
        if let Some(v) = self.visual.as_mut() {
            v.block = Some(Block {
                first,
                tail: lines - 1 - last,
                lines,
            });
        }
        self.caret_reset();
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// The open block's box on the window (logical px): the page's text column widened a
    /// little, from the room's top on the page, as tall as the room. `None` until the page has
    /// laid the room out.
    pub(super) fn visual_box(&self, pane: Rect) -> Option<Rect> {
        let block = self.visual_block()?;
        let p = self.preview.as_ref()?;
        let gap_y = p.gap_y?;
        let (_, height) = p.sent_gap()?;
        let _ = block;
        let s = self.scale;
        let width = (pane.w * s - 1.0).max(1.0) / s;
        let column = (width - 2.0 * PAGE.0).max(80.0).min(PAGE.1);
        let left = ((width - column) / 2.0 * s).round() / s;
        let x = (pane.x + 1.0 / s + left - BOX_OUT).max(pane.x);
        let w = (column + 2.0 * BOX_OUT).min(pane.right() - x);
        let gap = (BOX_GAP * s).round();
        let y = pane.y + (gap_y + gap - p.top) / s;
        Some(Rect::new(x, y, w, (height - 2.0 * gap).max(0.0) / s))
    }

    /// In visual editing the page takes the text area and the text view only the open block's
    /// box (an empty area at the top while no block is open).
    pub(super) fn with_visual(&self, mut a: crate::ui::Areas) -> crate::ui::Areas {
        crate::live::set_bare(self.visual_on());
        if !self.visual_on() {
            return a;
        }
        let page = a.text;
        a.preview = Some(page);
        a.other = None;
        a.divider = None;
        a.minimap = None;
        a.minimap_other = None;
        a.text = match self.visual_box(page) {
            Some(b) => self.visual_text(b),
            None => Rect::new(page.x, page.y, 0.0, 0.0),
        };
        a
    }

    /// The text view's area inside the box: the block's lines, under the box's padding.
    pub(super) fn visual_text(&self, b: Rect) -> Rect {
        // The text starts `TEXT_PAD` into its area: so it lines up with the page's column.
        let pad = (BOX_PAD * self.scale).round() / self.scale;
        let side = BOX_OUT - crate::view::metrics::TEXT_PAD;
        Rect::new(
            b.x + side,
            b.y + pad,
            (b.w - 2.0 * side).max(0.0),
            (b.h - 2.0 * pad).max(0.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdedit_core::buffer::Buffer;

    #[test]
    fn a_block_s_blank_lines_at_its_end_are_left_out() {
        let b = Buffer::from_text("# T\n\n- a\n- b\n\n\nLast\n");
        assert_eq!(trimmed(&b, 3, Some(7)), Some((2, 3)));
        assert_eq!(trimmed(&b, 1, Some(3)), Some((0, 0)));
        assert_eq!(trimmed(&b, 7, None), Some((6, 6)));
        assert_eq!(trimmed(&b, 9, None), None);
    }

    #[test]
    fn the_last_line_follows_lines_added_inside_the_block() {
        let block = Block {
            first: 2,
            tail: 3,
            lines: 10,
        };
        assert_eq!(block.last(8), 4);
        assert_eq!(block.last(10), 6);
        assert_eq!(block.last(3), 2);
    }
}
