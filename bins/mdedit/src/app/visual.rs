//! Visual editing (Ctrl+E): a Markdown document shown as its page, the preview pane taking the
//! whole text area. A click on a block opens it: the renderer leaves room for it
//! (`mdview_preview` `Request::Gap`) and the document's own view is drawn there, scrolled to the
//! block's lines, so typing, undo, find and every command work as in the text. Esc, a click
//! elsewhere on the page or the caret leaving the block closes it. The mode is the window's: it
//! follows the active tab while the page can show it (DESIGN.md section 72).

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
    /// A question for the page, asked once it has the document's current text.
    pub pending: Option<Ask>,
    /// The caret offset the page was last scrolled for, so the reader's own scrolling is left
    /// alone until the caret moves.
    pub caret_seen: Option<usize>,
    /// Visual editing opened the preview pane: ending it closes the pane again. A pane that was
    /// open before visual editing stays.
    pub pane_opened: bool,
    /// The page has still to be placed at this document's text: it is asked once the pane has
    /// that text, which a tab whose file is still being read has not sent yet.
    pub place: bool,
}

impl Visual {
    /// Takes the question held for the page, when the active document is still the one it was
    /// asked of. On the frame a tab is switched, `preview_sync` runs before `follow_tab` clears
    /// it: the old document's question is dropped then, not asked of the new one (audit 4, F13).
    pub(crate) fn take_pending(&mut self, active: Option<DocId>) -> Option<Ask> {
        let ask = self.pending.take();
        ask.filter(|_| active == Some(self.doc))
    }
}

/// A question for the page about the current text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Ask {
    /// The block holding this line (from 1).
    Line(u32),
    /// The block at this point of the pane (device px).
    At(f32, f32),
}

/// The open block: its first line (from 0), and how many lines follow it to the end of the
/// text. Edits inside the block change neither, so its last line is always known.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Block {
    pub first: usize,
    pub tail: usize,
    /// The text's line count when the block opened: the page keeps that text while it is open.
    pub lines: usize,
    /// The text when the block opened (a cheap copy), and the version checked against it last:
    /// an edit outside the block (Replace All, a reload, an AI edit, trimming on save) closes it.
    pub opened: ropey::Rope,
    pub seen: (u64, u64),
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
/// Whether `line` has nothing but white space.
fn blank(buffer: &mdedit_core::buffer::Buffer, line: usize) -> bool {
    let mut s = String::new();
    buffer.text(buffer.line_range(line), &mut s);
    s.trim().is_empty()
}

/// The text before the block's first line and after its last is as it was when it opened:
/// only the block itself was edited.
fn outside_unchanged(block: &Block, buffer: &mdedit_core::buffer::Buffer) -> bool {
    // Lines counted as the buffer counts them (a final line break makes no line of its own).
    let old = mdedit_core::buffer::Buffer::from_rope(block.opened.clone());
    let (old_lines, lines) = (old.len_lines(), buffer.len_lines());
    if block.first >= old_lines || block.first >= lines || block.tail >= lines {
        return false;
    }
    let head = old.line_start(block.first);
    if head != buffer.line_start(block.first)
        || old.rope().byte_slice(..head) != buffer.rope().byte_slice(..head)
    {
        return false;
    }
    // The tail: the same number of lines after the block, the same text. With none, the end.
    let end = |b: &mdedit_core::buffer::Buffer, n: usize| {
        if block.tail == 0 {
            b.len_bytes()
        } else {
            b.line_start(n - block.tail)
        }
    };
    let (old_tail, new_tail) = (end(&old, old_lines), end(buffer, lines));
    old.rope().byte_slice(old_tail..) == buffer.rope().byte_slice(new_tail..)
}

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
        let pane_was_open = self.preview.is_some();
        if !pane_was_open {
            self.toggle_preview_pane();
        }
        if self.preview.is_none() {
            return;
        }
        self.visual = Some(Visual {
            doc: id,
            block: None,
            pending: None,
            caret_seen: None,
            pane_opened: !pane_was_open,
            place: true,
        });
        self.docs.visual_open = true;
        // The page starts where the text was; from then on the reader scrolls it, and a new
        // text or size leaves it there.
        if let Some(p) = self.preview.as_mut() {
            p.editor_leads();
            p.unfollow();
        }
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// Back to the text: the mode goes, the open block closes, and the preview pane is left as
    /// it was before visual editing: closed if visual editing opened it, open if it was open.
    pub(super) fn end_visual(&mut self) {
        let opened_pane = self.visual.as_ref().is_some_and(|v| v.pane_opened);
        self.stop_visual();
        if opened_pane && self.preview.take().is_some() {
            self.docs.pane_open = false;
        }
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// Visual editing stops: the open block closes and the mode's own state goes. The pane is
    /// left alone; [`end_visual`](Self::end_visual) gives it back as it was.
    pub(super) fn stop_visual(&mut self) {
        self.close_block();
        self.visual = None;
        self.docs.visual_open = false;
    }

    /// Visual editing is on for the active document.
    pub(super) fn visual_on(&self) -> bool {
        self.visual.is_some()
    }

    /// The open block, if any.
    pub(super) fn visual_block(&self) -> Option<Block> {
        self.visual.as_ref()?.block.clone()
    }

    /// Select All in visual editing: the open block's lines, which are all the box shows.
    pub(super) fn visual_select_all(&mut self) {
        let Some(block) = self.visual_block() else {
            return;
        };
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let last = block.last(r.buffer.len_lines());
        r.view.extra.clear();
        r.view.sel = Selection {
            anchor: r.buffer.line_range(block.first).start,
            caret: line_end(&r.buffer, last),
            goal_col: None,
        };
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
        // Asked after the page has the current text (`preview_sync` sends it first), or the
        // answer would be in the lines of an older one.
        if let Some(v) = self.visual.as_mut() {
            v.pending = Some(Ask::At(px, py));
        }
        self.redraw();
    }

    /// Keeps visual editing in step, once per frame before the layout: follows the active tab
    /// while the page can show it (ending it when the page cannot, and when there is no tab),
    /// opens the block the page named, follows edits in the open block (its last line, the
    /// room's height) and closes it when the caret left it.
    pub(super) fn visual_sync(&mut self) {
        let Some(v) = self.visual.clone() else {
            return;
        };
        let current = self.docs.active_doc().map(|d| (d.id, previewable(d)));
        if self.preview.is_none() {
            self.end_visual();
            return;
        }
        // A tab still being read has no language yet, so the page cannot yet say whether it can
        // show it: the mode waits for the read instead of ending. A block open on the document
        // the page still shows belongs to the old tab, and closes before the wait; a question
        // still held for that block is not asked either (`preview_sync` asks nothing while no
        // document is ready, and `follow_tab` clears it once the tab is followed).
        if self
            .docs
            .active_doc()
            .is_some_and(|d| d.ready().is_none() && d.is_loading())
        {
            if !current.is_some_and(|(id, _)| id == v.doc) {
                self.close_block();
            }
            return;
        }
        match current {
            Some((id, true)) if id == v.doc => {}
            // Visual editing is a mode of the window, not of one file: another document the
            // page can show is followed.
            Some((id, true)) => self.follow_tab(id),
            // A document the page cannot show (a .rs file), or no tab at all.
            _ => {
                self.end_visual();
                return;
            }
        }
        if self.visual.as_ref().is_some_and(|v| v.place) {
            self.place_page();
        }
        let answer = self.preview.as_mut().and_then(|p| p.take_block());
        if let Some(Some(b)) = answer {
            self.open_block(b.first, b.next, b.at);
        }
        let Some(block) = self.visual_block() else {
            return;
        };
        let line_h = self.view.metrics(self.scale).line_h as f32;
        let version = self
            .docs
            .active_doc()
            .and_then(|d| d.ready().map(|r| (d.revision, r.buffer.revision())));
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        let lines = r.buffer.len_lines();
        let last = block.last(lines);
        let caret = r.buffer.line_of(r.view.sel.caret);
        if let Some(version) = version
            && version != block.seen
        {
            if !outside_unchanged(&block, &r.buffer) {
                // Something changed the text around the block: the page gets the new text and
                // the block under the caret opens again.
                let line = caret.min(lines.saturating_sub(1));
                self.close_block();
                if let Some(v) = self.visual.as_mut() {
                    v.pending = Some(Ask::Line(line as u32 + 1));
                }
                return;
            }
            if let Some(b) = self.visual.as_mut().and_then(|v| v.block.as_mut()) {
                b.seen = version;
            }
        }
        let Some(r) = self.docs.active_ready_mut() else {
            return;
        };
        if caret < block.first || caret > last {
            // The caret went out of the block (the arrows, a find): the block it went to opens,
            // over the blank lines between them. Past the first or the last block, with only
            // blank lines there, it stays in the block.
            let up = caret < block.first;
            let target = next_text_line(&r.buffer, caret, up);
            if blank(&r.buffer, target) {
                r.view.sel = Selection::at(if up {
                    r.buffer.line_range(block.first).start
                } else {
                    line_end(&r.buffer, last)
                });
                return;
            }
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
                v.pending = Some(Ask::Line(target as u32 + 1));
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
        // The caret's row in the box, counted the same way, to keep it in sight.
        let caret_offset = r.view.sel.caret;
        let caret_rows = if r.wrap && r.view.view_w >= 100.0 {
            let before: usize = (block.first..caret)
                .map(|l| crate::view::wrap::rows(r, l).len().max(1))
                .sum();
            (before + 1) as f32
        } else {
            (caret - block.first + 1) as f32
        };
        let s = self.scale;
        let pad = (BOX_PAD * s).round() + (BOX_GAP * s).round();
        let height = rows * line_h + 2.0 * pad;
        // In the page's lines: it keeps the text the block opened on until it closes.
        let page_last = block.last(block.lines);
        let pane_h = self.areas.preview.map_or(0.0, |p| p.h * s);
        let moved = self.visual.as_ref().and_then(|v| v.caret_seen) != Some(caret_offset);
        let Some(p) = self.preview.as_mut() else {
            return;
        };
        p.set_gap(Some((block.first as u32 + 1, page_last as u32 + 1, height)));
        // A caret that moved out of the pane (typing at the bottom of a tall block, a find)
        // brings the page along, a line of room kept; the reader's own scrolling is left alone.
        if moved
            && pane_h > 0.0
            && let Some(gap_y) = p.gap_y
        {
            let bottom = gap_y + pad + caret_rows * line_h;
            let top = bottom - line_h;
            let by = if bottom + line_h > p.top + pane_h {
                bottom + line_h - (p.top + pane_h)
            } else if top - line_h < p.top {
                top - line_h - p.top
            } else {
                0.0
            };
            if by != 0.0 {
                p.scroll(by);
            }
            if let Some(v) = self.visual.as_mut() {
                v.caret_seen = Some(caret_offset);
            }
        }
    }

    /// Visual editing moves to the document of the tab just made active: the old document's
    /// block closes, and the page starts where the new document's text was, as `toggle_visual`
    /// starts it.
    fn follow_tab(&mut self, id: DocId) {
        // The open block is the old document's: it goes, and the question about it with it.
        self.close_block();
        if let Some(v) = self.visual.as_mut() {
            v.doc = id;
            v.block = None;
            v.pending = None;
            v.caret_seen = None;
            v.place = true;
        }
        if let Some(p) = self.preview.as_mut() {
            p.editor_leads();
            p.unfollow();
        }
        self.relayout_areas();
        self.full = true;
        self.redraw();
    }

    /// Puts the page at the active document's caret, once the pane has that document's text:
    /// what `toggle_visual` starts a fresh pane with. The pane forgets the line afterwards, so
    /// the reader's scrolling is left alone from there on.
    fn place_page(&mut self) {
        let Some(doc) = self.docs.active_doc() else {
            return;
        };
        let Some(r) = doc.ready() else {
            return;
        };
        let line = r.buffer.line_of(r.view.sel.caret);
        let column = r.view.sel.caret - r.buffer.line_range(line).start;
        let Some(p) = self.preview.as_mut() else {
            return;
        };
        // A tab whose file is still being read has sent no text yet: it comes next frame.
        if p.shown() != Some(doc.id) || p.held {
            return;
        }
        p.follow(line as u32 + 1, column as u32);
        p.unfollow();
        if let Some(v) = self.visual.as_mut() {
            v.place = false;
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
        // The box shows every line of the block: folds over it open.
        for line in first..=last {
            r.view.folds.reveal(line);
        }
        crate::view::set_source_top(r, first as f64);
        let opened = r.buffer.snapshot();
        let seen = self
            .docs
            .active_doc()
            .and_then(|d| d.ready().map(|r| (d.revision, r.buffer.revision())))
            .unwrap_or_default();
        if let Some(v) = self.visual.as_mut() {
            v.block = Some(Block {
                first,
                tail: lines - 1 - last,
                lines,
                opened,
                seen,
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

    /// Audit 4, F13: the question held for the page is asked only of the document it was asked
    /// of; on the frame of a tab switch it is dropped, not asked of the new tab's text.
    #[test]
    fn a_held_question_is_not_asked_of_another_document() {
        let mut v = Visual {
            doc: 1,
            block: None,
            pending: Some(Ask::Line(4)),
            caret_seen: None,
            pane_opened: false,
            place: false,
        };
        assert_eq!(v.take_pending(Some(1)), Some(Ask::Line(4)));
        assert_eq!(v.pending, None, "taken once");
        v.pending = Some(Ask::At(3.0, 5.0));
        assert_eq!(v.take_pending(Some(2)), None, "another tab is active");
        assert_eq!(v.pending, None, "and the question is gone");
        v.pending = Some(Ask::Line(1));
        assert_eq!(v.take_pending(None), None, "no tab at all");
    }

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
            opened: ropey::Rope::new(),
            seen: (0, 0),
        };
        assert_eq!(block.last(8), 4);
        assert_eq!(block.last(10), 6);
        assert_eq!(block.last(3), 2);
    }

    #[test]
    fn an_edit_outside_the_block_is_told_from_one_inside_it() {
        let text = "# T

- a
- b

Last
";
        let block = Block {
            first: 2,
            tail: 2,
            lines: 6,
            opened: Buffer::from_text(text).snapshot(),
            seen: (0, 0),
        };
        let inside = Buffer::from_text(
            "# T

- a
- b, more
- c

Last
",
        );
        assert!(outside_unchanged(&block, &inside));
        let before = Buffer::from_text(
            "# Title

- a
- b

Last
",
        );
        assert!(!outside_unchanged(&block, &before));
        let after = Buffer::from_text(
            "# T

- a
- b

Last.
",
        );
        assert!(!outside_unchanged(&block, &after));
        let fewer = Buffer::from_text(
            "# T
- a
- b

Last
",
        );
        assert!(!outside_unchanged(&block, &fewer));
    }
}
