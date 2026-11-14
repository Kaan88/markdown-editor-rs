//! Folding by indentation: a line heads a block when the lines below it are indented more than
//! it, and folding the block hides them. Nothing here knows a language, so a closing brace at
//! the header's own indentation stays in view under the folded line.
//!
//! A block is found by looking at the lines around one line, when asked: nothing scans the
//! document as it is typed in. [`Folds`] is the set of folded blocks of one document.

use crate::buffer::{Buffer, Change};

/// Lines looked at, above or below a line, for the ends of its block.
pub const SCAN_MAX: usize = 200_000;
/// Bytes of a line looked at for its indentation; a line blank that far counts as text there.
const INDENT_MAX: usize = 4096;

/// The indentation of `line` in columns (a tab goes to the next multiple of `tab_width`), or
/// `None` for a line with nothing but spaces and tabs.
pub fn indent(buf: &Buffer, line: usize, tab_width: usize) -> Option<usize> {
    let range = buf.line_range(line);
    let end = range.end.min(range.start + INDENT_MAX);
    let tab = tab_width.max(1);
    let mut col = 0;
    for b in buf.rope().byte_slice(range.start..end).bytes() {
        match b {
            b' ' => col += 1,
            b'\t' => col += tab - col % tab,
            _ => return Some(col),
        }
    }
    (end < range.end).then_some(col)
}

/// The last line of the block `line` heads: the lines below it that are indented more, with the
/// blank lines between them. `None` when `line` is blank or the next line with text is not
/// indented more.
pub fn block_end(buf: &Buffer, line: usize, tab_width: usize) -> Option<usize> {
    let base = indent(buf, line, tab_width)?;
    let stop = buf.len_lines().min(line.saturating_add(1 + SCAN_MAX));
    let mut last = None;
    for l in line + 1..stop {
        match indent(buf, l, tab_width) {
            None => {}
            Some(i) if i > base => last = Some(l),
            Some(_) => break,
        }
    }
    last
}

/// Whether `line` heads a block, that is whether `block_end` would find one, without looking
/// for its end: the next line with text must be indented more. Cheap enough to ask for every
/// visible line, even when the block is huge.
pub fn heads_block(buf: &Buffer, line: usize, tab_width: usize) -> bool {
    let Some(base) = indent(buf, line, tab_width) else {
        return false;
    };
    let stop = buf.len_lines().min(line.saturating_add(1 + SCAN_MAX));
    for l in line + 1..stop {
        if let Some(i) = indent(buf, l, tab_width) {
            return i > base;
        }
    }
    false
}

/// The block to fold for a caret on `line`: the one the line heads, or else the nearest block
/// above that holds it. The header and the last line.
pub fn block_at(buf: &Buffer, line: usize, tab_width: usize) -> Option<(usize, usize)> {
    if let Some(end) = block_end(buf, line, tab_width) {
        return Some((line, end));
    }
    let mut inner = indent(buf, line, tab_width).unwrap_or(usize::MAX);
    for l in (line.saturating_sub(SCAN_MAX)..line).rev() {
        let Some(i) = indent(buf, l, tab_width) else {
            continue;
        };
        if i < inner {
            if let Some(end) = block_end(buf, l, tab_width).filter(|&end| end >= line) {
                return Some((l, end));
            }
            inner = i;
        }
        if i == 0 {
            break;
        }
    }
    None
}

/// The blocks headed by lines with no indentation, for "Fold all". The bool is true when the
/// document has more lines than are looked at (`SCAN_MAX`) and the rest was left alone.
pub fn top_blocks(buf: &Buffer, tab_width: usize) -> (Vec<(usize, usize)>, bool) {
    let lines = buf.len_lines();
    let stop = lines.min(SCAN_MAX);
    let mut out = Vec::new();
    let mut l = 0;
    while l < stop {
        match block_end(buf, l, tab_width) {
            Some(end) if indent(buf, l, tab_width) == Some(0) => {
                out.push((l, end));
                l = end + 1;
            }
            _ => l += 1,
        }
    }
    (out, stop < lines)
}

/// The folded blocks of a document: each a header line, which shows, and the last of the lines
/// under it, which do not. Sorted, and no block is inside another: folding around folded blocks
/// takes them in.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Folds {
    ranges: Vec<(usize, usize)>,
    /// For each fold, the lines hidden by the folds before it; then the total.
    cum: Vec<usize>,
    /// The folds are kept and follow edits, but nothing is hidden (compare mode shows every
    /// line). Every question below answers as if there were no folds.
    suspended: bool,
}

impl Folds {
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty() || self.suspended
    }

    /// Hides no line while `on`, keeping the folds (they still follow edits).
    pub fn suspend(&mut self, on: bool) {
        self.suspended = on;
    }

    pub fn suspended(&self) -> bool {
        self.suspended
    }

    pub fn ranges(&self) -> &[(usize, usize)] {
        &self.ranges
    }

    pub fn clear(&mut self) {
        self.ranges.clear();
        self.recount();
    }

    fn recount(&mut self) {
        self.cum.clear();
        let mut n = 0;
        for r in &self.ranges {
            self.cum.push(n);
            n += r.1 - r.0;
        }
        if !self.ranges.is_empty() {
            self.cum.push(n);
        }
    }

    /// How many lines the folds hide.
    pub fn hidden(&self) -> usize {
        if self.suspended {
            return 0;
        }
        self.cum.last().copied().unwrap_or(0)
    }

    /// The row of `line` among the lines that show, counted from 0; a hidden line has its
    /// fold's header's row.
    pub fn row_of(&self, line: usize) -> usize {
        if self.suspended {
            return line;
        }
        let line = self.shown(line);
        let k = self.ranges.partition_point(|r| r.0 < line);
        line - self.cum.get(k).copied().unwrap_or(0)
    }

    /// The line that shows as row `row`. Rows past the last give lines past the last.
    pub fn line_at_row(&self, row: usize) -> usize {
        if self.suspended {
            return row;
        }
        // The folds whose header is above `row`: a header's row is its line less what is
        // hidden above it.
        let (mut lo, mut hi) = (0, self.ranges.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.ranges[mid].0 - self.cum[mid] < row {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        row + self.cum.get(lo).copied().unwrap_or(0)
    }

    /// The fold that hides `line`.
    pub fn hiding(&self, line: usize) -> Option<(usize, usize)> {
        if self.suspended {
            return None;
        }
        let k = self.ranges.partition_point(|r| r.0 < line);
        let r = *self.ranges.get(k.checked_sub(1)?)?;
        (line <= r.1).then_some(r)
    }

    pub fn is_hidden(&self, line: usize) -> bool {
        self.hiding(line).is_some()
    }

    /// The fold `line` heads.
    pub fn headed(&self, line: usize) -> Option<(usize, usize)> {
        if self.suspended {
            return None;
        }
        let k = self.ranges.binary_search_by_key(&line, |r| r.0).ok()?;
        Some(self.ranges[k])
    }

    /// The line that shows for `line`: itself, or the header of the fold that hides it.
    pub fn shown(&self, line: usize) -> usize {
        self.hiding(line).map_or(line, |r| r.0)
    }

    /// The line shown below `line`, which shows itself. It can be one past the last line.
    pub fn next(&self, line: usize) -> usize {
        self.headed(line).map_or(line, |r| r.1) + 1
    }

    /// The line shown above `line`, which shows itself.
    pub fn prev(&self, line: usize) -> Option<usize> {
        Some(self.shown(line.checked_sub(1)?))
    }

    /// Folds the block from `header` to `end`. Folds inside it become part of it; inside a
    /// fold, nothing happens. False when nothing was folded.
    pub fn fold(&mut self, header: usize, end: usize) -> bool {
        if end <= header || self.suspended || self.is_hidden(header) {
            return false;
        }
        let a = self.ranges.partition_point(|r| r.0 < header);
        let b = self.ranges.partition_point(|r| r.0 <= end);
        let end = self.ranges[a..b].iter().fold(end, |e, r| e.max(r.1));
        self.ranges.splice(a..b, [(header, end)]);
        self.recount();
        true
    }

    /// Opens the fold `header` heads. False when it heads none.
    pub fn unfold(&mut self, header: usize) -> bool {
        if self.suspended {
            return false;
        }
        match self.ranges.binary_search_by_key(&header, |r| r.0) {
            Ok(k) => {
                self.ranges.remove(k);
                self.recount();
                true
            }
            Err(_) => false,
        }
    }

    /// Opens the fold that hides `line`. False when none does.
    pub fn reveal(&mut self, line: usize) -> bool {
        self.hiding(line).is_some_and(|r| self.unfold(r.0))
    }

    /// Follows an edit: folds below the lines it touched move with them, and a fold it
    /// touched, by adding or removing lines in it, opens.
    pub fn shift(&mut self, change: &Change) {
        if self.ranges.is_empty() || change.lines_before == change.lines_after {
            return;
        }
        let (first, before, after) = (change.first_line, change.lines_before, change.lines_after);
        self.ranges.retain_mut(|r| {
            if r.0 >= first + before {
                *r = (r.0 + after - before, r.1 + after - before);
                true
            } else {
                r.1 < first
            }
        });
        self.recount();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;
    use crate::selection::Selection;

    const CODE: &str = "\
fn a() {
    one();

    if x {
\t\ttwo();
    }
}

fn b() {}
    stray
";

    fn buf() -> Buffer {
        Buffer::from_text(CODE)
    }

    #[test]
    fn indentation_counts_tabs_and_knows_blank_lines() {
        let b = buf();
        assert_eq!(indent(&b, 0, 4), Some(0));
        assert_eq!(indent(&b, 1, 4), Some(4));
        assert_eq!(indent(&b, 2, 4), None);
        assert_eq!(indent(&b, 4, 4), Some(8));
        assert_eq!(indent(&b, 4, 8), Some(16));
        let b = Buffer::from_text("  \t x\n   \n");
        assert_eq!(indent(&b, 0, 4), Some(5));
        assert_eq!(indent(&b, 1, 4), None);
        // The last line, which is empty.
        assert_eq!(indent(&b, 2, 4), None);
        // A line blank for longer than is looked at counts as text.
        let b = Buffer::from_text(&" ".repeat(INDENT_MAX + 10));
        assert_eq!(indent(&b, 0, 4), Some(INDENT_MAX));
    }

    #[test]
    fn a_block_is_the_deeper_lines_below() {
        let b = buf();
        // The closing brace is not part of the block; the blank line inside is.
        assert_eq!(block_end(&b, 0, 4), Some(5));
        assert_eq!(block_end(&b, 3, 4), Some(4));
        assert_eq!(block_end(&b, 1, 4), None);
        assert_eq!(block_end(&b, 2, 4), None);
        assert_eq!(block_end(&b, 6, 4), None);
        assert_eq!(block_end(&b, 8, 4), Some(9));
        // A blank line at the end of a block is left out.
        let b = Buffer::from_text("a\n  b\n\nc\n");
        assert_eq!(block_end(&b, 0, 4), Some(1));
    }

    #[test]
    fn the_block_at_a_line_is_its_own_or_the_one_around_it() {
        let b = buf();
        assert_eq!(block_at(&b, 0, 4), Some((0, 5)));
        assert_eq!(block_at(&b, 1, 4), Some((0, 5)));
        // A blank line inside.
        assert_eq!(block_at(&b, 2, 4), Some((0, 5)));
        assert_eq!(block_at(&b, 4, 4), Some((3, 4)));
        assert_eq!(block_at(&b, 5, 4), Some((0, 5)));
        // The closing brace and the blank line after it are in no block.
        assert_eq!(block_at(&b, 6, 4), None);
        assert_eq!(block_at(&b, 7, 4), None);
        assert_eq!(top_blocks(&b, 4), (vec![(0, 5), (8, 9)], false));
    }

    #[test]
    fn heads_block_agrees_with_block_end() {
        let samples = [
            "a\n  b\n\n  c\nd\n\n\ne\n\tf\n",
            "x\n\n\n",
            "x\n    \n  y",
            "if a {\n    b;\n}\n\nfn f() {\n}\n",
            "",
            "\n\n  a\nb\n",
        ];
        for text in samples {
            let b = Buffer::from_text(text);
            for line in 0..b.len_lines() {
                assert_eq!(
                    heads_block(&b, line, 4),
                    block_end(&b, line, 4).is_some(),
                    "{text:?} line {line}"
                );
            }
        }
    }

    #[test]
    fn folds_hide_lines_and_take_in_folds_inside() {
        let mut f = Folds::default();
        assert!(f.is_empty() && !f.is_hidden(3));
        assert!(f.fold(3, 4));
        assert!(!f.fold(4, 9) && !f.fold(5, 5));
        assert_eq!(f.hiding(3), None);
        assert_eq!(f.hiding(4), Some((3, 4)));
        assert_eq!(f.headed(3), Some((3, 4)));
        assert_eq!((f.next(2), f.next(3), f.next(5)), (3, 5, 6));
        assert_eq!((f.prev(5), f.prev(3), f.prev(0)), (Some(3), Some(2), None));
        assert_eq!((f.shown(4), f.shown(5)), (3, 5));
        assert!(f.fold(8, 9));
        // The outer block takes the inner fold in.
        assert!(f.fold(0, 5));
        assert_eq!(f.ranges(), [(0, 5), (8, 9)]);
        assert!(f.reveal(2) && !f.reveal(2));
        assert_eq!(f.ranges(), [(8, 9)]);
        assert!(!f.unfold(9) && f.unfold(8) && f.is_empty());
    }

    #[test]
    fn rows_count_the_lines_that_show() {
        let mut f = Folds::default();
        assert_eq!((f.hidden(), f.row_of(7), f.line_at_row(7)), (0, 7, 7));
        f.fold(3, 5);
        f.fold(8, 9);
        assert_eq!(f.hidden(), 3);
        // Lines 0 1 2 3 [4 5] 6 7 8 [9] 10 show as rows 0 to 7.
        let rows: Vec<usize> = (0..11).map(|l| f.row_of(l)).collect();
        assert_eq!(rows, [0, 1, 2, 3, 3, 3, 4, 5, 6, 6, 7]);
        let lines: Vec<usize> = (0..9).map(|r| f.line_at_row(r)).collect();
        assert_eq!(lines, [0, 1, 2, 3, 6, 7, 8, 10, 11]);
        f.unfold(3);
        assert_eq!((f.hidden(), f.row_of(10), f.line_at_row(9)), (1, 9, 10));
        f.clear();
        assert_eq!((f.hidden(), f.row_of(10)), (0, 10));
    }

    #[test]
    fn folds_follow_edits() {
        let mut b = buf();
        let mut hist = History::new();
        let mut f = Folds::default();
        f.fold(3, 4);
        f.fold(8, 9);
        // A line put in above both moves both.
        let out = crate::edit::insert(&mut b, &mut hist, Selection::at(0), "// x\n");
        f.shift(out.change.as_ref().unwrap());
        assert_eq!(f.ranges(), [(4, 5), (9, 10)]);
        // Typing in a line moves nothing.
        let out = crate::edit::insert(&mut b, &mut hist, Selection::at(0), "zz");
        f.shift(out.change.as_ref().unwrap());
        assert_eq!(f.ranges(), [(4, 5), (9, 10)]);
        // A line break in the first fold's header opens it; the fold below moves.
        let at = b.line_range(4).end;
        let out = crate::edit::insert(&mut b, &mut hist, Selection::at(at), "\n");
        f.shift(out.change.as_ref().unwrap());
        assert_eq!(f.ranges(), [(10, 11)]);
        // Undo moves it back.
        let out = crate::edit::undo(&mut b, &mut hist).unwrap();
        f.shift(out.change.as_ref().unwrap());
        assert_eq!(f.ranges(), [(9, 10)]);
    }

    #[test]
    fn suspended_folds_hide_nothing_and_keep_following_edits() {
        let mut f = Folds::default();
        assert!(f.fold(2, 5));
        assert_eq!((f.hidden(), f.row_of(7), f.line_at_row(3)), (3, 4, 6));
        f.suspend(true);
        assert!(f.is_empty() && f.suspended());
        assert_eq!((f.hidden(), f.row_of(7), f.line_at_row(3)), (0, 7, 3));
        assert!(f.hiding(3).is_none() && f.headed(2).is_none());
        assert_eq!((f.shown(3), f.next(2)), (3, 3));
        assert!(!f.fold(8, 9) && !f.unfold(2));
        // The fold is still there, and goes on again.
        f.suspend(false);
        assert_eq!(f.ranges(), &[(2, 5)]);
        assert_eq!(f.hidden(), 3);
    }
}
