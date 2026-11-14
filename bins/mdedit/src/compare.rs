//! Compare two files side by side, in the split view.
//!
//! The comparison is `mdedit_core::diff`; this module is what the window needs of it. [`Align`]
//! sits in each compared document (`Ready::align`) and says which line is on which row, with
//! filler rows opposite the other side's extra lines, so the view, the scroll bars and the
//! minimap count rows where they counted lines. [`Compare`] is the state of the mode: the two
//! documents, the debounce, the worker, and the diff it has shown. Nothing here exists until a
//! comparison starts, and nothing runs while the two texts are unchanged.
//!
//! While the texts change, the diff on screen stays until the next one is ready (a worker makes
//! it a short while after the last edit, on copies of the two texts, so typing never waits). A
//! line the old diff has no row for is shown after the last row, and the marks inside a line are
//! left out until the diff is current.

use crate::app::Wake;
use crate::commands::CommandId as C;
use crate::docs::{Docs, Waker};
use mdedit_core::buffer::Buffer;
use mdedit_core::diff::{self, Diff, DiffOptions, HunkKind};
use mdedit_highlight::DocId;
use mdshared_ui::gfx::{Rgb, mix, rgb};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the texts must rest before the comparison is made again.
pub const DEBOUNCE: Duration = Duration::from_millis(150);

/// The finished comparison and the two texts it was made from (kept for the marks inside lines).
pub struct Data {
    pub diff: Diff,
    pub a: Buffer,
    pub b: Buffer,
    pub opts: DiffOptions,
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

/// One document's view of the comparison. Without `data` (before the first result) the rows are
/// the lines.
#[derive(Clone)]
pub struct Align {
    /// Changes whenever the diff does: part of what a repaint is keyed by.
    pub stamp: u64,
    /// This document is the second file (`b`) of the comparison.
    pub right: bool,
    /// The diff is of the text as it is now.
    pub fresh: bool,
    data: Option<Arc<Data>>,
}

impl Align {
    pub fn pending(right: bool) -> Align {
        Align {
            stamp: GENERATION.fetch_add(1, Ordering::Relaxed),
            right,
            fresh: false,
            data: None,
        }
    }

    fn with(data: Arc<Data>, right: bool) -> Align {
        Align {
            stamp: GENERATION.fetch_add(1, Ordering::Relaxed),
            right,
            fresh: false,
            data: Some(data),
        }
    }

    /// Lines of this side the diff knows.
    fn side_lines(&self) -> usize {
        self.data.as_ref().map_or(0, |d| {
            let (a, b) = d.diff.lines();
            if self.right { b } else { a }
        })
    }

    /// The rows of the view for a document of `lines` lines.
    pub fn rows(&self, lines: usize) -> usize {
        match &self.data {
            None => lines,
            Some(d) => d.diff.row_count() + lines.saturating_sub(self.side_lines()),
        }
    }

    /// The line on row `row`; `None` for a filler row.
    pub fn line_at(&self, row: usize, lines: usize) -> Option<usize> {
        let Some(d) = &self.data else {
            return (row < lines).then_some(row);
        };
        let n = d.diff.row_count();
        let line = if row < n {
            let r = d.diff.row(row)?;
            if self.right { r.b } else { r.a }?
        } else {
            self.side_lines() + (row - n)
        };
        (line < lines).then_some(line)
    }

    /// The line on row `row`, or for a filler row the line that follows it.
    pub fn line_near(&self, row: usize, lines: usize) -> usize {
        let last = lines.saturating_sub(1);
        if let Some(l) = self.line_at(row, lines) {
            return l;
        }
        let Some(d) = &self.data else { return last };
        d.diff.hunk_at(row).map_or(last, |h| {
            let hunk = &d.diff.hunks()[h];
            let end = if self.right { hunk.b.end } else { hunk.a.end };
            end.min(last)
        })
    }

    /// The row of line `line`.
    pub fn row_of(&self, line: usize) -> usize {
        let Some(d) = &self.data else { return line };
        let n = self.side_lines();
        if line < n {
            let r = if self.right {
                d.diff.row_of_b(line)
            } else {
                d.diff.row_of_a(line)
            };
            r.unwrap_or(0)
        } else {
            d.diff.row_count() + (line - n)
        }
    }

    /// What kind of change row `row` is part of; `None` beyond the diff's rows.
    pub fn kind(&self, row: usize) -> Option<HunkKind> {
        self.data.as_ref()?.diff.row(row).map(|r| r.kind)
    }

    /// The bytes of the line on row `row` (counted from the line's start) that differ from the
    /// line opposite, when the diff is current.
    pub fn intra(&self, row: usize) -> Option<Range<usize>> {
        let d = self.data.as_ref().filter(|_| self.fresh)?;
        let (a, b) = d.diff.intra_line(row, &d.a, &d.b, d.opts)?;
        Some(if self.right { b } else { a })
    }

    /// The first row of the first difference that starts after `row`.
    pub fn next_change(&self, row: usize) -> Option<usize> {
        let d = self.data.as_ref()?;
        let h = d.diff.next_hunk(row)?;
        Some(d.diff.hunk_rows(h).start)
    }

    /// The first row of the last difference that starts before `row`.
    pub fn prev_change(&self, row: usize) -> Option<usize> {
        let d = self.data.as_ref()?;
        let h = d.diff.prev_hunk(row)?;
        Some(d.diff.hunk_rows(h).start)
    }

    /// The status strip's words: "3 differences", "No differences", "Comparing…".
    pub fn summary(&self) -> String {
        let Some(d) = self.data.as_ref().filter(|_| self.fresh) else {
            return "Comparing\u{2026}".to_string();
        };
        let mut s = match d.diff.stats().hunks {
            0 => "No differences".to_string(),
            1 => "1 difference".to_string(),
            n => format!("{n} differences"),
        };
        if d.diff.approximate {
            s.push_str(", approximate");
        }
        s
    }
}

/// The colours of the marks, for the light or the dark theme.
#[derive(Clone, Copy)]
pub struct Tints {
    pub removed: Rgb,
    pub added: Rgb,
    pub changed: Rgb,
    /// The row opposite a block that has no lines on this side.
    pub filler: Rgb,
}

pub fn tints(dark: bool, page: Rgb, text: Rgb) -> Tints {
    let (removed, added, changed) = if dark {
        (rgb(0xe5534b), rgb(0x57ab5a), rgb(0xc69026))
    } else {
        (rgb(0xd1242f), rgb(0x1a7f37), rgb(0x9a6700))
    };
    Tints {
        removed,
        added,
        changed,
        filler: mix(page, text, 0.06),
    }
}

impl Tints {
    /// The colour of a row of this kind on a side (`right`: the second file), if it is marked.
    pub fn of(&self, kind: HunkKind, right: bool) -> Option<Rgb> {
        match (kind, right) {
            (HunkKind::Equal, _) => None,
            (HunkKind::Change, _) => Some(self.changed),
            (HunkKind::Delete, false) => Some(self.removed),
            (HunkKind::Insert, true) => Some(self.added),
            (HunkKind::Delete, true) | (HunkKind::Insert, false) => None,
        }
    }
}

/// Which compare commands can run now.
pub fn enabled(id: C, docs: &Docs) -> bool {
    let compared = docs.active_ready().is_some_and(|r| r.align.is_some());
    match id {
        C::CompareOther => !compared && docs.other().is_some() && docs.active_ready().is_some(),
        C::CompareFile => docs.active_ready().is_some() && !compared,
        C::CompareSaved => {
            !compared
                && docs
                    .active_doc()
                    .is_some_and(|d| d.path.is_some() && d.modified())
        }
        C::StopCompare | C::NextDifference | C::PreviousDifference => compared,
        _ => true,
    }
}

/// A comparison being made: the worker's answer lands in `slot`.
struct Job {
    cancel: Arc<AtomicBool>,
    slot: Arc<Mutex<Option<Arc<Data>>>>,
    revisions: (u64, u64),
}

/// What the window keeps while two documents are compared.
pub struct Compare {
    /// The first file (the left pane when the comparison started) and the second.
    pub a: DocId,
    pub b: DocId,
    /// The read-only copy of the file on disk made for "Compare with saved": closed with the
    /// comparison.
    pub saved: Option<DocId>,
    pub opts: DiffOptions,
    /// The revisions of the two texts the diff on screen was made from.
    shown: Option<(u64, u64)>,
    /// The revisions last seen, and when they have rested long enough.
    seen: (u64, u64),
    due: Option<Instant>,
    job: Option<Job>,
    /// Where the two panes were left, to see which of them was scrolled.
    pub synced: (f64, f64),
}

impl Compare {
    pub fn new(a: DocId, b: DocId, saved: Option<DocId>) -> Compare {
        Compare {
            a,
            b,
            saved,
            opts: DiffOptions::default(),
            shown: None,
            seen: (0, 0),
            due: Some(Instant::now()),
            job: None,
            synced: (0.0, 0.0),
        }
    }

    pub fn has(&self, id: DocId) -> bool {
        self.a == id || self.b == id
    }

    /// The diff on screen is of the texts as they are, and none is being made.
    pub fn settled(&self) -> bool {
        self.job.is_none() && self.due.is_none() && self.shown == Some(self.seen)
    }

    /// Stops the worker.
    pub fn cancel(&mut self) {
        if let Some(j) = self.job.take() {
            j.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn revisions(&self, docs: &Docs) -> Option<(u64, u64)> {
        let get = |id| {
            let i = docs.index_of_id(id)?;
            Some(docs.tabs[i].ready()?.revision)
        };
        Some((get(self.a)?, get(self.b)?))
    }

    /// Takes in what the worker finished, starts one when the texts have rested, and marks the
    /// diffs fresh or not. Returns when it next needs to run, if it does, and whether what is
    /// drawn changed.
    pub fn step(&mut self, docs: &mut Docs, now: Instant, wake: &Waker) -> (Option<Instant>, bool) {
        let Some(cur) = self.revisions(docs) else {
            return (None, false);
        };
        let mut changed = false;
        if cur != self.seen {
            self.seen = cur;
            self.due = Some(now + DEBOUNCE);
            // What is being made is out of date.
            self.cancel();
        }
        let done = self
            .job
            .as_ref()
            .and_then(|j| j.slot.lock().unwrap().take().map(|d| (d, j.revisions)));
        if let Some((data, revisions)) = done {
            self.job = None;
            if revisions == cur {
                self.install(docs, data);
                self.shown = Some(cur);
                self.due = None;
                changed = true;
            } else {
                self.due = Some(now);
            }
        }
        let mut next = None;
        if self.job.is_none()
            && self.shown != Some(cur)
            && let Some(due) = self.due
        {
            if now >= due {
                self.due = None;
                self.start(docs, cur, wake.clone());
            } else {
                next = Some(due);
            }
        }
        changed |= self.mark_fresh(docs, cur);
        (next, changed)
    }

    fn start(&mut self, docs: &Docs, revisions: (u64, u64), wake: Waker) {
        let buffers = |id| {
            let i = docs.index_of_id(id)?;
            Some(docs.tabs[i].ready()?.buffer.clone())
        };
        let (Some(a), Some(b)) = (buffers(self.a), buffers(self.b)) else {
            return;
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let slot = Arc::new(Mutex::new(None));
        self.job = Some(Job {
            cancel: cancel.clone(),
            slot: slot.clone(),
            revisions,
        });
        let opts = self.opts;
        std::thread::spawn(move || {
            let diff = diff::diff(&a, &b, opts, &cancel);
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            *slot.lock().unwrap() = Some(Arc::new(Data { diff, a, b, opts }));
            wake(Wake::Compare);
        });
    }

    /// Puts the diff in both documents, keeping each one's top line where it was.
    fn install(&mut self, docs: &mut Docs, data: Arc<Data>) {
        for (id, right) in [(self.a, false), (self.b, true)] {
            let Some(i) = docs.index_of_id(id) else {
                continue;
            };
            if let Some(r) = docs.tabs[i].ready_mut() {
                let lines = r.buffer.len_lines();
                let old = r.align.take();
                let top = if r.top.is_finite() {
                    r.top.max(0.0)
                } else {
                    0.0
                };
                let row = top.floor() as usize;
                let line = old.as_ref().map_or(row, |o| o.line_near(row, lines));
                let new = Align::with(data.clone(), right);
                r.top = new.row_of(line) as f64 + (top - row as f64);
                r.align = Some(new);
            }
        }
        // Both panes at the same row: the active one leads.
        let lead = docs
            .active_doc()
            .map(|d| d.id)
            .filter(|&id| self.has(id))
            .unwrap_or(self.a);
        let top = docs
            .index_of_id(lead)
            .and_then(|i| docs.tabs[i].ready())
            .map_or(0.0, |r| r.top);
        self.set_tops(docs, top);
    }

    fn set_tops(&mut self, docs: &mut Docs, v: f64) {
        for id in [self.a, self.b] {
            if let Some(i) = docs.index_of_id(id)
                && let Some(r) = docs.tabs[i].ready_mut()
            {
                r.top = v;
            }
        }
        self.synced = (v, v);
    }

    fn mark_fresh(&self, docs: &mut Docs, cur: (u64, u64)) -> bool {
        let fresh = self.shown == Some(cur);
        let mut changed = false;
        for id in [self.a, self.b] {
            if let Some(i) = docs.index_of_id(id)
                && let Some(r) = docs.tabs[i].ready_mut()
                && let Some(al) = r.align.as_mut()
                && al.fresh != fresh
            {
                al.fresh = fresh;
                changed = true;
            }
        }
        changed
    }

    /// The panes scroll together by row: whichever moved since the last frame leads, the active
    /// document if both did.
    pub fn sync_scroll(&mut self, docs: &mut Docs) {
        let top = |id| {
            let i = docs.index_of_id(id)?;
            Some(docs.tabs[i].ready()?.top)
        };
        let (Some(ta), Some(tb)) = (top(self.a), top(self.b)) else {
            return;
        };
        let lead = match (ta != self.synced.0, tb != self.synced.1) {
            (false, false) => return,
            (true, false) => self.a,
            (false, true) => self.b,
            (true, true) => docs
                .active_doc()
                .map(|d| d.id)
                .filter(|&id| self.has(id))
                .unwrap_or(self.a),
        };
        let v = if lead == self.a { ta } else { tb };
        self.set_tops(docs, v);
    }

    /// Ends the comparison in both documents: the lines are the rows again, each document's top
    /// at the line that was at its top.
    pub fn release(&mut self, docs: &mut Docs) {
        self.cancel();
        for id in [self.a, self.b] {
            let Some(i) = docs.index_of_id(id) else {
                continue;
            };
            if let Some(r) = docs.tabs[i].ready_mut() {
                if let Some(al) = r.align.take() {
                    let lines = r.buffer.len_lines();
                    let top = if r.top.is_finite() {
                        r.top.max(0.0)
                    } else {
                        0.0
                    };
                    let row = top.floor() as usize;
                    r.top = al.line_near(row, lines) as f64 + (top - row as f64);
                }
                r.folds.suspend(false);
            }
        }
    }
}

impl crate::docs::Ready {
    /// The row of line `line`: the line's place among the lines that show (folds) or among the
    /// rows of a comparison.
    pub fn row_of_line(&self, line: usize) -> usize {
        match &self.align {
            Some(al) => al.row_of(line),
            None => self.folds.row_of(line),
        }
    }

    /// The line on row `row`, or the line that follows a filler row.
    pub fn line_at_row(&self, row: usize) -> usize {
        match &self.align {
            Some(al) => al.line_near(row, self.buffer.len_lines()),
            None => self.folds.line_at_row(row),
        }
    }

    /// The line on row `row`; `None` for a filler row of a comparison.
    pub fn line_on_row(&self, row: usize) -> Option<usize> {
        match &self.align {
            Some(al) => al.line_at(row, self.buffer.len_lines()),
            None => Some(self.folds.line_at_row(row)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(a: &str, b: &str) -> Arc<Data> {
        let (a, b) = (Buffer::from_text(a), Buffer::from_text(b));
        let opts = DiffOptions::default();
        let diff = diff::diff(&a, &b, opts, &AtomicBool::new(false));
        Arc::new(Data { diff, a, b, opts })
    }

    fn pair(a: &str, b: &str) -> (Align, Align, usize, usize) {
        let d = data(a, b);
        let (na, nb) = (d.a.len_lines(), d.b.len_lines());
        let mut l = Align::with(d.clone(), false);
        let mut r = Align::with(d, true);
        l.fresh = true;
        r.fresh = true;
        (l, r, na, nb)
    }

    #[test]
    fn rows_pair_the_lines_with_fillers_opposite_extra_lines() {
        // b has "x" and "y" inserted after "two", and lost "four".
        let (l, r, na, nb) = pair("one\ntwo\nthree\nfour\n", "one\ntwo\nx\ny\nthree\n");
        assert_eq!(l.rows(na), r.rows(nb));
        // Row 2 and 3: a has no line (filler), b has x and y.
        assert_eq!(l.line_at(1, na), Some(1));
        assert_eq!(l.line_at(2, na), None);
        assert_eq!(r.line_at(2, nb), Some(2));
        assert_eq!(r.line_at(3, nb), Some(3));
        assert_eq!(l.line_at(4, na), Some(2));
        assert_eq!(r.line_at(4, nb), Some(4));
        // The line that follows a filler row, and each line's row.
        assert_eq!(l.line_near(2, na), 2);
        assert_eq!(l.row_of(2), 4);
        assert_eq!(r.row_of(4), 4);
        assert_eq!(l.kind(0), Some(HunkKind::Equal));
        assert_eq!(l.kind(2), Some(HunkKind::Insert));
    }

    #[test]
    fn differences_are_counted_and_found_from_a_row() {
        let (l, _, _, _) = pair("a\nb\nc\nd\ne\nf\n", "a\nB\nc\nd\ne\nF\n");
        assert_eq!(l.summary(), "2 differences");
        assert_eq!(l.next_change(0), Some(1));
        assert_eq!(l.next_change(1), Some(5));
        assert_eq!(l.next_change(5), None);
        assert_eq!(l.prev_change(5), Some(1));
        assert_eq!(l.prev_change(1), None);
        let (l, _, _, _) = pair("same\n", "same\n");
        assert_eq!(l.summary(), "No differences");
        let mut p = Align::pending(false);
        assert_eq!(p.summary(), "Comparing\u{2026}");
        p.fresh = true;
        assert_eq!(
            (p.rows(7), p.line_at(3, 7), p.line_at(7, 7), p.row_of(4)),
            (7, Some(3), None, 4)
        );
    }

    #[test]
    fn marks_inside_a_line_need_a_current_diff() {
        let (mut l, r, _, _) = pair("let x = 1;\n", "let x = 2;\n");
        assert_eq!(l.intra(0), Some(8..9));
        assert_eq!(r.intra(0), Some(8..9));
        l.fresh = false;
        assert_eq!(l.intra(0), None);
        assert_eq!(l.summary(), "Comparing\u{2026}");
    }

    #[test]
    fn a_line_the_diff_does_not_know_goes_after_its_rows() {
        let (l, _, na, _) = pair("a\nb\n", "a\nb\n");
        // The text grew by two lines since the diff.
        assert_eq!(l.rows(na + 2), l.rows(na) + 2);
        assert_eq!(l.line_at(l.rows(na), na + 2), Some(na));
        assert_eq!(l.row_of(na + 1), l.rows(na) + 1);
        // A shorter text: rows past its end are fillers.
        assert_eq!(l.line_at(1, 1), None);
    }

    #[test]
    fn tints_mark_deleted_lines_on_the_left_and_inserted_ones_on_the_right() {
        let t = tints(false, [255; 3], [0; 3]);
        assert_eq!(t.of(HunkKind::Delete, false), Some(t.removed));
        assert_eq!(t.of(HunkKind::Delete, true), None);
        assert_eq!(t.of(HunkKind::Insert, true), Some(t.added));
        assert_eq!(t.of(HunkKind::Insert, false), None);
        assert_eq!(t.of(HunkKind::Change, true), Some(t.changed));
        assert_eq!(t.of(HunkKind::Equal, false), None);
    }
}
