//! Word wrap: a line shows as one or more rows no wider than the view. Rows are found from
//! column counts (`mdedit_core::display::wrap_rows`), not from shaped text, so scrolling, a
//! click and a caret move need no layout; a row is then shaped and drawn like a chunk of a long
//! line. The scroll position is a line and a row in it, and every move walks rows from there:
//! nothing counts the rows of the whole document.

use super::metrics::KEEP_LINES;
use crate::docs::Ready;
use mdedit_core::display::{ChunkStart, Cols, wrap_rows};
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

/// The row starts of one line; the first is the line's start.
pub type Rows = Rc<[ChunkStart]>;
/// Lines whose rows are kept, the most recently used.
pub const ROW_CACHE: usize = 512;
/// A document read with a line longer than this many characters is not wrapped: every edit in
/// such a line finds all its rows again, about 20 ms for each 1 MB of it.
pub const WRAP_LINE_MAX: usize = 1 << 18;
/// Kept free at the right of a row, logical px: the caret after the last character and the
/// scrollbar over the text.
const WRAP_PAD: f32 = 16.0;

/// A row of the document: a line and the row in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: usize,
    pub row: usize,
}

/// The rows of `line` at the view's width as last drawn, kept until the text or the width
/// changes.
pub fn rows(r: &mut Ready, line: usize) -> Rows {
    let stamp = (
        r.revision,
        r.view.view_w.to_bits(),
        r.view.advance.to_bits(),
        r.view.wide_advance.to_bits(),
    );
    if r.view.rows_stamp != stamp {
        r.view.rows.clear();
        r.view.rows_stamp = stamp;
    }
    if let Some(v) = r.view.rows.get(&line) {
        return v.clone();
    }
    let range = r.buffer.line_range(line);
    let mut text = String::new();
    r.buffer.text(range.clone(), &mut text);
    let max = (r.view.view_w - WRAP_PAD).max(0.0);
    let v: Rows = wrap_rows(
        &text,
        range.start,
        crate::live::tab_width(),
        r.view.advance,
        r.view.wide_advance,
        max,
    )
    .into();
    r.view.rows.insert(line, v.clone());
    v
}

/// Where row `p` starts and the offset it ends at.
pub fn row(r: &mut Ready, p: Pos) -> (ChunkStart, usize) {
    let v = rows(r, p.line);
    let k = p.row.min(v.len() - 1);
    let end = v
        .get(k + 1)
        .map_or_else(|| r.buffer.line_range(p.line).end, |n| n.offset);
    (v[k], end)
}

/// The row that shows `offset`: at the place a line wraps, the row that starts there.
pub fn pos_of(r: &mut Ready, offset: usize) -> Pos {
    let line = r.buffer.line_of(offset);
    let v = rows(r, line);
    let row = v.partition_point(|s| s.offset <= offset).saturating_sub(1);
    Pos { line, row }
}

/// The row `n` rows after `p` (before it when negative), stopping at the document's ends.
pub fn step(r: &mut Ready, p: Pos, n: isize) -> Pos {
    let last = r.buffer.len_lines().saturating_sub(1);
    let mut p = Pos {
        line: r.view.folds.shown(p.line.min(last)),
        row: p.row,
    };
    p.row = p.row.min(rows(r, p.line).len() - 1);
    let mut left = n.unsigned_abs();
    if n >= 0 {
        loop {
            let len = rows(r, p.line).len();
            if p.row + left < len {
                p.row += left;
                return p;
            }
            // Lines inside a fold have no rows.
            let next = r.view.folds.next(p.line);
            if next > last {
                p.row = len - 1;
                return p;
            }
            left -= len - p.row;
            p = Pos { line: next, row: 0 };
        }
    }
    loop {
        if left <= p.row {
            p.row -= left;
            return p;
        }
        let Some(prev) = r.view.folds.prev(p.line) else {
            p.row = 0;
            return p;
        };
        left -= p.row + 1;
        p.line = prev;
        p.row = rows(r, p.line).len() - 1;
    }
}

/// How many rows `to` is after `from`, when it is at most `limit`.
pub fn rows_between(r: &mut Ready, from: Pos, to: Pos, limit: usize) -> Option<usize> {
    if to < from {
        return None;
    }
    let (mut cur, mut n) = (from, 0usize);
    while cur.line < to.line {
        n += rows(r, cur.line).len().saturating_sub(cur.row);
        if n > limit {
            return None;
        }
        cur = Pos {
            line: r.view.folds.next(cur.line),
            row: 0,
        };
    }
    n += to.row.saturating_sub(cur.row);
    (n <= limit).then_some(n)
}

/// The row at the view's top.
pub fn top(r: &mut Ready) -> Pos {
    let last = r.buffer.len_lines().saturating_sub(1);
    let line = if r.view.top.is_finite() && r.view.top > 0.0 {
        (r.view.top as usize).min(last)
    } else {
        0
    };
    // A top line that a fold has since hidden: the fold's first line.
    let shown = r.view.folds.shown(line);
    if shown != line {
        return Pos {
            line: shown,
            row: 0,
        };
    }
    let row = r.view.top_row.min(rows(r, line).len() - 1);
    Pos { line, row }
}

pub fn set_top(r: &mut Ready, p: Pos) {
    r.view.top = p.line as f64;
    r.view.top_row = p.row;
}

/// Scrolls by rows, down when positive. Parts of a row add up between calls.
pub fn scroll(r: &mut Ready, by: f64) {
    if !by.is_finite() {
        return;
    }
    r.view.row_rest += by;
    let n = r.view.row_rest.trunc();
    r.view.row_rest -= n;
    if n != 0.0 {
        let from = top(r);
        let to = step(r, from, n as isize);
        set_top(r, to);
    }
}

/// Keeps the row of `offset` in view, as `view::reveal` does for a line.
pub fn reveal(r: &mut Ready, offset: usize, center: bool, margin: bool) {
    let view = (r.view.view_lines.floor() as usize).max(1);
    let keep = if margin { KEEP_LINES as usize } else { 0 }.min((view - 1) / 2);
    let (t, target) = (top(r), pos_of(r, offset));
    let below = rows_between(r, t, target, view + keep);
    let inside = below.is_some_and(|d| d < view);
    let next = if center && !inside {
        step(r, target, -((view / 2) as isize))
    } else {
        match below {
            // Above the view, or in its top margin.
            None if target < t => step(r, target, -(keep as isize)),
            Some(d) if d < keep => step(r, target, -(keep as isize)),
            Some(d) if d + 1 + keep > view => step(r, t, (d + 1 + keep - view) as isize),
            Some(_) => t,
            // Far below.
            None => step(r, target, -((view - 1 - keep) as isize)),
        }
    };
    set_top(r, next);
    r.view.left = 0.0;
}

/// The caret moved `n` rows down (up when negative), keeping its column in the row: the new
/// offset and the column aimed for, which the next such move is given back as `goal`. At the
/// first row it goes to the document's start, at the last to its end, as without wrap.
pub fn vertical(r: &mut Ready, caret: usize, goal: Option<usize>, n: isize) -> (usize, usize) {
    let p = pos_of(r, caret);
    let (start, _) = row(r, p);
    let mut text = String::new();
    r.buffer
        .text(start.offset..caret.max(start.offset), &mut text);
    let here = start.cols().after(&text, crate::live::tab_width()).col - start.col;
    let want = goal.unwrap_or(here);
    let q = step(r, p, n);
    if q == p {
        let at = if n < 0 { 0 } else { r.buffer.len_bytes() };
        return (at, want);
    }
    let (s, end) = row(r, q);
    let last_row = q.row + 1 == rows(r, q.line).len();
    text.clear();
    r.buffer.text(s.offset..end, &mut text);
    let mut cols: Cols = s.cols();
    let mut at = end;
    let mut prev = 0;
    for (i, g) in text.grapheme_indices(true) {
        let next = cols.after(g, crate::live::tab_width());
        if next.col - s.col > want {
            at = s.offset + i;
            break;
        }
        cols = next;
        prev = i;
    }
    // The end of a row that is not its line's last shows on the next row: stay before it.
    if at == end && !last_row && !text.is_empty() {
        at = s.offset + prev;
    }
    (at, want)
}
