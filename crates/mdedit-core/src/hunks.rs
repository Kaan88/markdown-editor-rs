//! The review of a rewrite as changes with a tick each (`docs/ai-library-plan.md` 5.5, D-077).
//!
//! [`Hunks`] keeps the text before and the text the model sent back, the line comparison of the
//! two ([`diff::diff`], the one Compare uses), and one tick per change. [`Hunks::compose`] makes
//! the text that Apply puts in: an unchanged run gives the old lines, a change gives the new
//! lines when it is ticked and the old ones when it is not. With every change ticked that is the
//! model's result to the byte, with none ticked the original to the byte, and the text in
//! between is made in one pass, so Apply is one edit and one undo step.
//!
//! Nothing here reads anything from outside but the two strings, and nothing can panic on them:
//! every index comes from the comparison of those two strings.

use crate::buffer::Buffer;
use crate::diff::{self, DiffOptions, Hunk, HunkKind};
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

/// Every `Hunks` made in the process gets its own number, so a list's layout can tell when the
/// comparison it was made from has been replaced (even by one of the same size).
static NEXT_STAMP: AtomicU64 = AtomicU64::new(1);

/// One part of the whole, in order: lines that are the same on both sides, or one change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seg {
    /// Line ranges (0-based) of the old and the new text; the same lines on both sides.
    Same { a: Range<usize>, b: Range<usize> },
    /// The change with this number (0-based, in order).
    Change(usize),
}

/// A change: the old lines it takes away and the new ones it puts there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub kind: HunkKind,
    /// Line ranges (0-based) in the old and the new text.
    pub a: Range<usize>,
    pub b: Range<usize>,
    /// The hunk's number in the comparison, for [`Hunks::intra`].
    hunk: usize,
}

impl Change {
    /// Lines the change takes away.
    pub fn removed(&self) -> usize {
        self.a.len()
    }

    /// Lines the change puts in.
    pub fn added(&self) -> usize {
        self.b.len()
    }
}

/// The text before, the text after, their changes and which of them are ticked.
pub struct Hunks {
    original: String,
    result: String,
    a: Buffer,
    b: Buffer,
    diff: diff::Diff,
    changes: Vec<Change>,
    ticks: Vec<bool>,
    stamp: u64,
}

impl Hunks {
    /// The changes from `original` to `result`, every one ticked.
    pub fn new(original: &str, result: &str) -> Hunks {
        let a = Buffer::from_text(original);
        let b = Buffer::from_text(result);
        let d = diff::diff(&a, &b, DiffOptions::default(), &Default::default());
        let changes: Vec<Change> = d
            .hunks()
            .iter()
            .enumerate()
            .filter(|(_, h)| h.kind != HunkKind::Equal)
            .map(|(i, h): (usize, &Hunk)| Change {
                kind: h.kind,
                a: h.a.clone(),
                b: h.b.clone(),
                hunk: i,
            })
            .collect();
        let ticks = vec![true; changes.len()];
        Hunks {
            original: original.to_string(),
            result: result.to_string(),
            a,
            b,
            diff: d,
            changes,
            ticks,
            stamp: NEXT_STAMP.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// An identity for this comparison, changed with every one made: a layout made from one is
    /// never used for another.
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn result(&self) -> &str {
        &self.result
    }

    /// The number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn change(&self, i: usize) -> Option<&Change> {
        self.changes.get(i)
    }

    pub fn is_ticked(&self, i: usize) -> bool {
        self.ticks.get(i).copied().unwrap_or(false)
    }

    pub fn set_ticked(&mut self, i: usize, on: bool) {
        if let Some(t) = self.ticks.get_mut(i) {
            *t = on;
        }
    }

    pub fn toggle(&mut self, i: usize) {
        if let Some(t) = self.ticks.get_mut(i) {
            *t = !*t;
        }
    }

    pub fn set_all(&mut self, on: bool) {
        self.ticks.iter_mut().for_each(|t| *t = on);
    }

    /// How many changes are ticked.
    pub fn ticked(&self) -> usize {
        self.ticks.iter().filter(|t| **t).count()
    }

    /// Lines the whole result adds and removes, ticked or not: the counts of the model's work.
    pub fn totals(&self) -> (usize, usize) {
        self.sum(|_| true)
    }

    /// Lines added and removed by the ticked changes: what Apply does.
    pub fn taken_totals(&self) -> (usize, usize) {
        self.sum(|i| self.ticks[i])
    }

    fn sum(&self, keep: impl Fn(usize) -> bool) -> (usize, usize) {
        let mut t = (0, 0);
        for (i, c) in self.changes.iter().enumerate() {
            if keep(i) {
                t.0 += c.added();
                t.1 += c.removed();
            }
        }
        t
    }

    /// The old lines, then the new ones, as parts in order: what the list draws.
    pub fn segments(&self) -> Vec<Seg> {
        let mut out = Vec::with_capacity(self.diff.hunks().len());
        let mut n = 0;
        for h in self.diff.hunks() {
            if h.kind == HunkKind::Equal {
                out.push(Seg::Same {
                    a: h.a.clone(),
                    b: h.b.clone(),
                });
            } else {
                out.push(Seg::Change(n));
                n += 1;
            }
        }
        out
    }

    /// Lines in the old text.
    pub fn old_lines(&self) -> usize {
        self.diff.lines().0
    }

    /// Lines in the new text.
    pub fn new_lines(&self) -> usize {
        self.diff.lines().1
    }

    /// An old line's text, without its line break (`line` from 0).
    pub fn old_line(&self, line: usize) -> String {
        text_of(&self.a, line)
    }

    /// A new line's text, without its line break (`line` from 0).
    pub fn new_line(&self, line: usize) -> String {
        text_of(&self.b, line)
    }

    /// For line `j` of change `i`, when the change has as many old lines as new ones: the part
    /// of the old line and of the new line that differs, byte ranges in each line's text.
    pub fn intra(&self, i: usize, j: usize) -> Option<(Range<usize>, Range<usize>)> {
        let c = self.changes.get(i)?;
        if j >= c.a.len() {
            return None;
        }
        let row = self.diff.hunk_rows(c.hunk).start + j;
        self.diff
            .intra_line(row, &self.a, &self.b, DiffOptions::default())
    }

    /// The text with the ticked changes made and the others left as they were.
    pub fn compose(&self) -> String {
        let n = self.ticks.len();
        let ticked = self.ticked();
        if ticked == n {
            return self.result.clone();
        }
        if ticked == 0 {
            return self.original.clone();
        }
        let eol = if self.original.contains("\r\n") || self.result.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let mut out = String::with_capacity(self.original.len().max(self.result.len()));
        let mut c = 0;
        for h in self.diff.hunks() {
            let piece = if h.kind == HunkKind::Equal {
                slice(&self.a, &h.a)
            } else {
                let take = self.ticks.get(c).copied().unwrap_or(false);
                c += 1;
                if take {
                    slice(&self.b, &h.b)
                } else {
                    slice(&self.a, &h.a)
                }
            };
            if piece.is_empty() {
                continue;
            }
            // A last line that has no break (the old text's or the new one's) never runs into
            // the lines that follow it.
            if !out.is_empty() && !out.ends_with(['\n', '\r']) {
                out.push_str(eol);
            }
            out.push_str(&piece);
        }
        // When neither text ends in a line break, nor does the text made of them: a ticked
        // deletion of the last lines leaves the break that only parted them from the line
        // before (audit 4, B n2).
        if !ends_in_break(&self.original) && !ends_in_break(&self.result) {
            if out.ends_with("\r\n") {
                out.truncate(out.len() - 2);
            } else if out.ends_with(['\n', '\r']) {
                out.truncate(out.len() - 1);
            }
        }
        out
    }
}

/// Whether a text's last line ends in a line break (LF, CRLF or CR).
fn ends_in_break(text: &str) -> bool {
    text.ends_with(['\n', '\r'])
}

/// A line's text without its break.
fn text_of(buf: &Buffer, line: usize) -> String {
    if line >= diff::line_count(buf) {
        return String::new();
    }
    let mut s = String::new();
    buf.text(buf.line_range(line), &mut s);
    s
}

/// The text of the lines `lines` of `buf`, line breaks included.
fn slice(buf: &Buffer, lines: &Range<usize>) -> String {
    let total = diff::line_count(buf);
    if lines.is_empty() || lines.start >= total {
        return String::new();
    }
    let last = lines.end.min(total) - 1;
    let start = buf.line_range_with_break(lines.start).start;
    let end = buf.line_range_with_break(last).end;
    let mut s = String::new();
    buf.text(start..end, &mut s);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "# T\n\nalpha\nbeta\ngamma\n\ndelta\nepsilon\nzeta\n";
    const NEW: &str = "# T\n\nALPHA\nbeta\ngamma\n\ndelta\nepsilon\nzeta\neta\n";

    #[test]
    fn every_change_ticked_is_the_result_to_the_byte() {
        let h = Hunks::new(OLD, NEW);
        assert_eq!(h.len(), 2);
        assert_eq!(h.compose(), NEW);
    }

    #[test]
    fn no_change_ticked_is_the_original_to_the_byte() {
        let mut h = Hunks::new(OLD, NEW);
        h.set_all(false);
        assert_eq!(h.compose(), OLD);
    }

    #[test]
    fn a_subset_takes_only_those_changes() {
        let mut h = Hunks::new(OLD, NEW);
        h.set_ticked(0, false);
        assert_eq!(
            h.compose(),
            "# T\n\nalpha\nbeta\ngamma\n\ndelta\nepsilon\nzeta\neta\n"
        );
        h.set_ticked(0, true);
        h.set_ticked(1, false);
        assert_eq!(
            h.compose(),
            "# T\n\nALPHA\nbeta\ngamma\n\ndelta\nepsilon\nzeta\n"
        );
        assert_eq!((h.ticked(), h.len()), (1, 2));
        assert_eq!(h.taken_totals(), (1, 1));
        assert_eq!(h.totals(), (2, 1));
    }

    #[test]
    fn a_line_with_no_break_never_runs_into_the_next() {
        // The new text ends with no break; a later change that is not taken keeps old lines.
        let mut h = Hunks::new("a\nb\nc\nd\n", "A\nb\nc\nD");
        assert_eq!(h.len(), 2);
        h.set_ticked(0, false);
        assert_eq!(h.compose(), "a\nb\nc\nD");
        let mut h = Hunks::new("x\ny\nz\n", "X");
        assert!(!h.is_empty());
        h.set_ticked(0, true);
        assert_eq!(h.compose(), "X");
    }

    /// Audit 4, B n2: when neither text ends in a line break, a subset that takes a deletion of
    /// the last lines does not end in one either, with LF or CRLF.
    #[test]
    fn a_deletion_at_the_end_of_texts_with_no_final_break_leaves_none() {
        for eol in ["\n", "\r\n"] {
            let old = ["x", "a", "b"].join(eol);
            let new = ["X", "a"].join(eol);
            let mut h = Hunks::new(&old, &new);
            assert_eq!(h.len(), 2, "{eol:?}");
            h.set_ticked(0, false);
            assert_eq!(h.compose(), ["x", "a"].join(eol), "the deletion only");
            h.set_ticked(0, true);
            h.set_ticked(1, false);
            assert_eq!(h.compose(), ["X", "a", "b"].join(eol), "the change only");
        }
        // A text that ends in a break keeps it.
        let mut h = Hunks::new("x\na\nb\n", "X\na\n");
        h.set_ticked(0, false);
        assert_eq!(h.compose(), "x\na\n");
    }

    #[test]
    fn crlf_text_is_composed_with_its_breaks() {
        let old = "a\r\nb\r\nc\r\nd\r\n";
        let new = "A\r\nb\r\nc\r\nD\r\n";
        let mut h = Hunks::new(old, new);
        h.set_ticked(1, false);
        assert_eq!(h.compose(), "A\r\nb\r\nc\r\nd\r\n");
    }

    #[test]
    fn an_insertion_and_a_deletion_are_changes_of_their_own() {
        let h = Hunks::new("a\nb\nc\n", "a\nX\nb\n");
        let kinds: Vec<_> = (0..h.len()).map(|i| h.change(i).unwrap().kind).collect();
        assert_eq!(kinds, [HunkKind::Insert, HunkKind::Delete]);
        let mut h = h;
        h.set_ticked(1, false);
        assert_eq!(h.compose(), "a\nX\nb\nc\n");
    }

    #[test]
    fn identical_texts_have_no_changes() {
        let h = Hunks::new("same\n", "same\n");
        assert!(h.is_empty());
        assert_eq!(h.compose(), "same\n");
    }

    #[test]
    fn the_segments_tile_the_text_and_name_the_changes_in_order() {
        let h = Hunks::new(OLD, NEW);
        let segs = h.segments();
        let changes: Vec<usize> = segs
            .iter()
            .filter_map(|s| match s {
                Seg::Change(i) => Some(*i),
                _ => None,
            })
            .collect();
        assert_eq!(changes, [0, 1]);
        assert_eq!(h.old_lines(), 9);
        assert_eq!(h.new_lines(), 10);
        assert_eq!(h.old_line(2), "alpha");
        assert_eq!(h.new_line(2), "ALPHA");
        assert_eq!(h.new_line(99), "");
    }

    #[test]
    fn a_changed_pair_tells_which_words_differ() {
        let h = Hunks::new("one two three\n", "one 2 three\n");
        let (a, b) = h.intra(0, 0).expect("a pair");
        assert_eq!(&h.old_line(0)[a], "two");
        assert_eq!(&h.new_line(0)[b], "2");
        assert!(h.intra(0, 5).is_none());
    }

    /// Any ticks of any change give text made of whole old and new lines only, in the order the
    /// segments put them, with no line lost and none made up, on a spread of small texts. The
    /// expected lines are walked from the segments and the comparison's own line texts, not from
    /// `compose`.
    #[test]
    fn every_subset_of_a_spread_of_texts_composes_without_losing_a_line() {
        let olds = ["", "a\n", "a\nb\nc\n", "a\nb\nc\nd\ne\nf\n", "x"];
        let news = ["", "b\n", "a\nX\nc\nY\n", "f\ne\nd\nc\nb\na\n", "x\ny"];
        for o in olds {
            for n in news {
                let mut h = Hunks::new(o, n);
                assert_eq!(h.compose(), n);
                for mask in 0..(1u32 << h.len().min(8)) {
                    for i in 0..h.len().min(8) {
                        h.set_ticked(i, mask >> i & 1 == 1);
                    }
                    let mut want: Vec<String> = Vec::new();
                    for seg in h.segments() {
                        match seg {
                            Seg::Same { a, .. } => {
                                for l in a {
                                    want.push(h.old_line(l));
                                }
                            }
                            Seg::Change(i) => {
                                let c = h.change(i).expect("a change of the segments");
                                let lines = if h.is_ticked(i) { &c.b } else { &c.a };
                                for l in lines.clone() {
                                    let line = if h.is_ticked(i) {
                                        h.new_line(l)
                                    } else {
                                        h.old_line(l)
                                    };
                                    want.push(line);
                                }
                            }
                        }
                    }
                    let t = h.compose();
                    let got: Vec<String> = t.lines().map(str::to_string).collect();
                    assert_eq!(got, want, "mask {mask:b} from {o:?} to {n:?}: {:?}", t);
                }
            }
        }
    }
}
