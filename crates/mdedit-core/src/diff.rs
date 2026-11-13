//! Comparing two buffers line by line, for a side-by-side view.
//!
//! Lines are compared by content without their line break (so a CRLF file and an LF file with the
//! same text are equal), after the normalisation [`DiffOptions`] asks for. Each line is hashed
//! once straight from the rope's chunks into a 128-bit key and interned to a small id; from there
//! on only ids are compared. A text that ends with a line break has no empty last line, so "a\n"
//! and "a" compare equal; a file with no lines is empty.
//!
//! The comparison is Myers' O(ND) algorithm in its linear-space form (the middle snake splits the
//! problem in two), after the common prefix and suffix are cut off and lines that occur on only
//! one side are set aside (they are changed for certain, and leaving them out shrinks the
//! problem a great deal when the files differ much). It is bounded: [`DIFF_MAX_LINES`] per side
//! and [`DIFF_WORK`] steps in all. A stretch it cannot finish in the budget stays one changed
//! block (so the rest of the files is still compared well) and [`Diff::approximate`] is set. A
//! block that moved is reported as a deletion and an insertion.
//!
//! The result is a list of [`Hunk`]s that tile both files, and a row alignment for two panes:
//! equal lines share a row, and a deleted or inserted block leaves filler rows opposite it. Rows
//! are computed from the hunks, so nothing is stored per line.

use crate::buffer::Buffer;
use crate::hash::hash128;
use crate::limits::{DIFF_MAX_D, DIFF_MAX_LINES, DIFF_WORK, INTRA_HUNK_LINES, INTRA_LINE_BYTES};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};
use unicode_segmentation::GraphemeCursor;

/// What counts as a difference between two lines.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiffOptions {
    /// Spaces, tabs and other white space at the end of a line do not count.
    pub ignore_trailing_ws: bool,
    /// White space anywhere in a line does not count: "a b" equals "ab" and "a  b".
    pub ignore_ws: bool,
    /// Letters are compared by their lower-case forms.
    pub ignore_case: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HunkKind {
    /// The same lines on both sides (`a` and `b` have the same length).
    Equal,
    /// Lines only in b (`a` is empty).
    Insert,
    /// Lines only in a (`b` is empty).
    Delete,
    /// Lines of a replaced by lines of b (both ranges are not empty).
    Change,
}

/// The kind of a row of the side-by-side view: the kind of the hunk it belongs to.
pub type RowKind = HunkKind;

/// One stretch of the comparison. `a` and `b` are line ranges (0-based) of the two files; the
/// hunks of a [`Diff`] follow each other and cover both files completely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hunk {
    pub kind: HunkKind,
    pub a: Range<usize>,
    pub b: Range<usize>,
}

/// One row of the side-by-side view: the line shown on the left, the one on the right, and what
/// the row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub a: Option<usize>,
    pub b: Option<usize>,
    pub kind: RowKind,
}

/// Counts for a status line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DiffStats {
    /// Lines only in b: inserted blocks, and the surplus of a changed block with more lines in b.
    pub added: usize,
    /// Lines only in a, likewise.
    pub removed: usize,
    /// Lines of a changed block that have a partner on the other side (the smaller side's count).
    pub changed: usize,
    /// Hunks that are not `Equal`.
    pub hunks: usize,
}

#[derive(Clone, Debug)]
pub struct Diff {
    hunks: Vec<Hunk>,
    /// The first row of each hunk, and the row count at the end.
    row_starts: Vec<usize>,
    /// Indexes of the hunks that are not `Equal`.
    changes: Vec<usize>,
    a_lines: usize,
    b_lines: usize,
    /// The comparison hit a bound: some changed blocks may hold lines that match. Everything
    /// reported `Equal` is equal; the hunks still tile the files.
    pub approximate: bool,
    /// `cancel` was set; the result is as coarse as an approximate one.
    pub cancelled: bool,
}

/// The number of lines the comparison sees in a buffer: the buffer's lines without the empty one
/// after a final line break.
pub fn line_count(buf: &Buffer) -> usize {
    let n = buf.len_lines();
    if buf.line_range_with_break(n - 1).is_empty() {
        n - 1
    } else {
        n
    }
}

// ---- keys

/// The key is a hash already.
#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x0100_0000_01B3);
        }
    }

    fn write_u128(&mut self, key: u128) {
        self.0 = (key as u64) ^ ((key >> 64) as u64);
    }
}

type Interner = HashMap<u128, u32, BuildHasherDefault<KeyHasher>>;

/// `raw` as the options see it, appended to `out`.
fn normalize(raw: &str, opts: DiffOptions, out: &mut String) {
    out.clear();
    let raw = if opts.ignore_trailing_ws && !opts.ignore_ws {
        raw.trim_end_matches(char::is_whitespace)
    } else {
        raw
    };
    for c in raw.chars() {
        if opts.ignore_ws && c.is_whitespace() {
            continue;
        }
        if opts.ignore_case {
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
}

/// The ids of the first `n` lines of `buf`; `None` when `cancel` was set.
fn intern(
    buf: &Buffer,
    n: usize,
    opts: DiffOptions,
    interner: &mut Interner,
    cancel: &AtomicBool,
) -> Option<Vec<u32>> {
    let plain = opts == DiffOptions::default();
    let (mut raw, mut norm) = (String::new(), String::new());
    let mut ids = Vec::with_capacity(n);
    interner.reserve(n);
    for (line, l) in buf.rope().lines().take(n).enumerate() {
        if line % 4096 == 0 && cancel.load(Ordering::Relaxed) {
            return None;
        }
        // The line without its break (LF, CRLF or CR), and as one piece when it lies in one chunk.
        let mut len = l.len_bytes();
        match len.checked_sub(1).map(|i| l.byte(i)) {
            Some(b'\n') => {
                len -= 1;
                if len > 0 && l.byte(len - 1) == b'\r' {
                    len -= 1;
                }
            }
            Some(b'\r') => len -= 1,
            _ => {}
        }
        let l = l.byte_slice(..len);
        let text: &str = match l.as_str() {
            Some(t) => t,
            None => {
                raw.clear();
                raw.extend(l.chunks());
                &raw
            }
        };
        let key = if plain {
            hash128(text.as_bytes())
        } else {
            normalize(text, opts, &mut norm);
            hash128(norm.as_bytes())
        };
        let next = interner.len() as u32;
        ids.push(*interner.entry(key).or_insert(next));
    }
    Some(ids)
}

// ---- Myers

/// Equal lines: `len` lines from `a` and from `b`, starting at these lines.
#[derive(Clone, Copy, Debug)]
struct Run {
    a: usize,
    b: usize,
    len: usize,
}

struct Myers<'a> {
    a: &'a [u32],
    b: &'a [u32],
    /// Maps a position in `a` (`b`) to the line of the file, when lines were set aside.
    amap: Option<&'a [usize]>,
    bmap: Option<&'a [usize]>,
    vf: Vec<isize>,
    vb: Vec<isize>,
    runs: Vec<Run>,
    work: usize,
    limit: usize,
    approximate: bool,
    cancelled: bool,
    cancel: &'a AtomicBool,
}

impl Myers<'_> {
    /// Records `len` equal lines starting at `a[x]` and `b[y]` (positions in the sequences
    /// compared), as runs of lines that are also next to each other in the files.
    fn push(&mut self, x: usize, y: usize, len: usize) {
        for i in 0..len {
            let la = self.amap.map_or(x + i, |m| m[x + i]);
            let lb = self.bmap.map_or(y + i, |m| m[y + i]);
            match self.runs.last_mut() {
                Some(r) if r.a + r.len == la && r.b + r.len == lb => r.len += 1,
                _ => self.runs.push(Run {
                    a: la,
                    b: lb,
                    len: 1,
                }),
            }
        }
    }

    fn solve(&mut self, mut a0: usize, mut a1: usize, mut b0: usize, mut b1: usize) {
        let (a, b) = (self.a, self.b);
        let mut p = 0;
        while a0 + p < a1 && b0 + p < b1 && a[a0 + p] == b[b0 + p] {
            p += 1;
        }
        self.push(a0, b0, p);
        (a0, b0) = (a0 + p, b0 + p);
        let mut s = 0;
        while a0 < a1 - s && b0 < b1 - s && a[a1 - 1 - s] == b[b1 - 1 - s] {
            s += 1;
        }
        (a1, b1) = (a1 - s, b1 - s);
        self.work += p + s;
        if a0 < a1 && b0 < b1 {
            match self.split(a0, a1, b0, b1) {
                Some((x0, y0, x1, y1)) => {
                    self.solve(a0, x0, b0, y0);
                    self.push(x0, y0, x1 - x0);
                    self.solve(x1, a1, y1, b1);
                }
                None => self.approximate = true,
            }
        }
        self.push(a1, b1, s);
    }

    /// The middle snake of the edit graph of `a[a0..a1]` and `b[b0..b1]` (both not empty, no
    /// common first or last line): from (x0, y0) to (x1, y1), as positions in `a` and `b`. The
    /// two stretches before and after it each need fewer edits than the whole. `None` when the
    /// work budget or the most edits is used up first.
    fn split(
        &mut self,
        a0: usize,
        a1: usize,
        b0: usize,
        b1: usize,
    ) -> Option<(usize, usize, usize, usize)> {
        let (a, b) = (&self.a[a0..a1], &self.b[b0..b1]);
        let (n, m) = (a.len() as isize, b.len() as isize);
        let delta = n - m;
        let odd = delta & 1 != 0;
        let dmax = ((n + m + 1) / 2).min(DIFF_MAX_D as isize);
        let off = DIFF_MAX_D as isize + 1;
        let (vf, vb) = (&mut self.vf, &mut self.vb);
        let (limit, cancel) = (self.limit, self.cancel);
        vf[(off + 1) as usize] = 0;
        vb[(off + 1) as usize] = 0;
        let at = |k: isize| (off + k) as usize;
        let mut work = 0usize;
        let mut cancelled = false;
        let found = 'search: {
            for d in 0..=dmax {
                work += (d as usize) + 1;
                if self.work + work > limit {
                    break 'search None;
                }
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    break 'search None;
                }
                let mut k = -d;
                while k <= d {
                    let mut x = if k == -d || (k != d && vf[at(k - 1)] < vf[at(k + 1)]) {
                        vf[at(k + 1)]
                    } else {
                        vf[at(k - 1)] + 1
                    };
                    let mut y = x - k;
                    let (sx, sy) = (x, y);
                    while x < n && y < m && a[x as usize] == b[y as usize] {
                        x += 1;
                        y += 1;
                    }
                    work += (x - sx) as usize;
                    vf[at(k)] = x;
                    if odd {
                        let kb = delta - k;
                        if kb > -d && kb < d && x + vb[at(kb)] >= n {
                            break 'search Some((sx, sy, x, y));
                        }
                    }
                    k += 2;
                }
                let mut k = -d;
                while k <= d {
                    let mut x = if k == -d || (k != d && vb[at(k - 1)] < vb[at(k + 1)]) {
                        vb[at(k + 1)]
                    } else {
                        vb[at(k - 1)] + 1
                    };
                    let mut y = x - k;
                    let (sx, sy) = (x, y);
                    while x < n && y < m && a[(n - 1 - x) as usize] == b[(m - 1 - y) as usize] {
                        x += 1;
                        y += 1;
                    }
                    work += (x - sx) as usize;
                    vb[at(k)] = x;
                    if !odd {
                        let kf = delta - k;
                        if kf >= -d && kf <= d && vf[at(kf)] + x >= n {
                            break 'search Some((n - x, m - y, n - sx, m - sy));
                        }
                    }
                    k += 2;
                }
            }
            None
        };
        self.work += work;
        self.cancelled |= cancelled;
        let (x0, y0, x1, y1) = found?;
        let (a0, b0) = (a0 as isize, b0 as isize);
        Some((
            (a0 + x0) as usize,
            (b0 + y0) as usize,
            (a0 + x1) as usize,
            (b0 + y1) as usize,
        ))
    }
}

/// Equal runs of two id sequences, and whether the bound was hit or `cancel` set.
fn match_ids(
    a: &[u32],
    b: &[u32],
    ids: usize,
    limit: usize,
    cancel: &AtomicBool,
) -> (Vec<Run>, bool, bool) {
    // Lines that occur on one side only cannot match: leave them out.
    let mut in_a = vec![false; ids];
    let mut in_b = vec![false; ids];
    for &id in a {
        in_a[id as usize] = true;
    }
    for &id in b {
        in_b[id as usize] = true;
    }
    let keep_a: Vec<usize> = (0..a.len()).filter(|&i| in_b[a[i] as usize]).collect();
    let keep_b: Vec<usize> = (0..b.len()).filter(|&i| in_a[b[i] as usize]).collect();
    let ca: Vec<u32> = keep_a.iter().map(|&i| a[i]).collect();
    let cb: Vec<u32> = keep_b.iter().map(|&i| b[i]).collect();
    let all = keep_a.len() == a.len() && keep_b.len() == b.len();
    let mut my = Myers {
        a: &ca,
        b: &cb,
        amap: (!all).then_some(&keep_a[..]),
        bmap: (!all).then_some(&keep_b[..]),
        vf: vec![0; 2 * DIFF_MAX_D + 3],
        vb: vec![0; 2 * DIFF_MAX_D + 3],
        runs: Vec::new(),
        work: 0,
        limit,
        approximate: false,
        cancelled: false,
        cancel,
    };
    my.solve(0, ca.len(), 0, cb.len());
    (my.runs, my.approximate, my.cancelled)
}

// ---- result

impl Diff {
    fn from_runs(runs: &[Run], a_lines: usize, b_lines: usize) -> Diff {
        let mut hunks = Vec::new();
        let (mut ia, mut ib) = (0, 0);
        let gap = |hunks: &mut Vec<Hunk>, a: usize, b: usize, ia: usize, ib: usize| {
            let kind = match (a > ia, b > ib) {
                (false, false) => return,
                (false, true) => HunkKind::Insert,
                (true, false) => HunkKind::Delete,
                (true, true) => HunkKind::Change,
            };
            hunks.push(Hunk {
                kind,
                a: ia..a,
                b: ib..b,
            });
        };
        for r in runs {
            gap(&mut hunks, r.a, r.b, ia, ib);
            hunks.push(Hunk {
                kind: HunkKind::Equal,
                a: r.a..r.a + r.len,
                b: r.b..r.b + r.len,
            });
            (ia, ib) = (r.a + r.len, r.b + r.len);
        }
        gap(&mut hunks, a_lines, b_lines, ia, ib);
        let mut row_starts = Vec::with_capacity(hunks.len() + 1);
        let mut changes = Vec::new();
        let mut row = 0;
        for (i, h) in hunks.iter().enumerate() {
            row_starts.push(row);
            row += h.a.len().max(h.b.len());
            if h.kind != HunkKind::Equal {
                changes.push(i);
            }
        }
        row_starts.push(row);
        Diff {
            hunks,
            row_starts,
            changes,
            a_lines,
            b_lines,
            approximate: false,
            cancelled: false,
        }
    }

    /// The hunks in order; they tile both files.
    pub fn hunks(&self) -> &[Hunk] {
        &self.hunks
    }

    /// Lines of the first and of the second file, as the comparison counts them.
    pub fn lines(&self) -> (usize, usize) {
        (self.a_lines, self.b_lines)
    }

    /// Whether the two files are equal under the options (no hunk but `Equal`).
    pub fn is_identical(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn stats(&self) -> DiffStats {
        let mut s = DiffStats {
            hunks: self.changes.len(),
            ..DiffStats::default()
        };
        for &i in &self.changes {
            let h = &self.hunks[i];
            let (na, nb) = (h.a.len(), h.b.len());
            s.changed += na.min(nb);
            s.added += nb.saturating_sub(na);
            s.removed += na.saturating_sub(nb);
        }
        s
    }

    /// The number of rows of the side-by-side view.
    pub fn row_count(&self) -> usize {
        self.row_starts.last().copied().unwrap_or(0)
    }

    fn hunk_of_row(&self, row: usize) -> Option<usize> {
        if row >= self.row_count() {
            return None;
        }
        Some(self.row_starts.partition_point(|&s| s <= row) - 1)
    }

    fn row_in(&self, i: usize, row: usize) -> Row {
        let h = &self.hunks[i];
        let k = row - self.row_starts[i];
        Row {
            a: (k < h.a.len()).then(|| h.a.start + k),
            b: (k < h.b.len()).then(|| h.b.start + k),
            kind: h.kind,
        }
    }

    /// What row `row` shows; `None` past the last row.
    pub fn row(&self, row: usize) -> Option<Row> {
        Some(self.row_in(self.hunk_of_row(row)?, row))
    }

    /// The rows in order.
    pub fn rows(&self) -> impl Iterator<Item = Row> + '_ {
        self.hunks.iter().enumerate().flat_map(move |(i, h)| {
            let first = self.row_starts[i];
            (0..h.a.len().max(h.b.len())).map(move |k| self.row_in(i, first + k))
        })
    }

    /// The row that shows line `line` of the first file; `None` past its last line.
    pub fn row_of_a(&self, line: usize) -> Option<usize> {
        if line >= self.a_lines {
            return None;
        }
        let i = self.hunks.partition_point(|h| h.a.end <= line);
        Some(self.row_starts[i] + (line - self.hunks[i].a.start))
    }

    /// The row that shows line `line` of the second file; `None` past its last line.
    pub fn row_of_b(&self, line: usize) -> Option<usize> {
        if line >= self.b_lines {
            return None;
        }
        let i = self.hunks.partition_point(|h| h.b.end <= line);
        Some(self.row_starts[i] + (line - self.hunks[i].b.start))
    }

    /// The rows a hunk covers (by its index in `hunks()`).
    pub fn hunk_rows(&self, hunk: usize) -> Range<usize> {
        self.row_starts[hunk]..self.row_starts[hunk + 1]
    }

    /// The index of the hunk (not `Equal`) that contains row `row`, if it is in one.
    pub fn hunk_at(&self, row: usize) -> Option<usize> {
        let i = self.hunk_of_row(row)?;
        (self.hunks[i].kind != HunkKind::Equal).then_some(i)
    }

    /// The first changed hunk that starts after row `row` (an index into `hunks()`; its first
    /// row is `hunk_rows(i).start`).
    pub fn next_hunk(&self, row: usize) -> Option<usize> {
        let at = self.changes.partition_point(|&i| self.row_starts[i] <= row);
        self.changes.get(at).copied()
    }

    /// The last changed hunk that starts before row `row`.
    pub fn prev_hunk(&self, row: usize) -> Option<usize> {
        let at = self.changes.partition_point(|&i| self.row_starts[i] < row);
        at.checked_sub(1).map(|j| self.changes[j])
    }

    /// For a row of a changed block with as many lines on each side (at most
    /// `INTRA_HUNK_LINES`), the part of each line that differs: byte ranges in the two lines
    /// (line break excluded), found by cutting the characters the two lines share at the start
    /// and at the end. Cuts fall between grapheme clusters. `None` for any other row, for a line
    /// longer than `INTRA_LINE_BYTES`, and for a pair that is equal under the options.
    pub fn intra_line(
        &self,
        row: usize,
        a: &Buffer,
        b: &Buffer,
        opts: DiffOptions,
    ) -> Option<(Range<usize>, Range<usize>)> {
        let i = self.hunk_of_row(row)?;
        let h = &self.hunks[i];
        if h.kind != HunkKind::Change || h.a.len() != h.b.len() || h.a.len() > INTRA_HUNK_LINES {
            return None;
        }
        let r = self.row_in(i, row);
        let (la, lb) = (r.a?, r.b?);
        let (ra, rb) = (a.line_range(la), b.line_range(lb));
        if ra.len() > INTRA_LINE_BYTES || rb.len() > INTRA_LINE_BYTES {
            return None;
        }
        let (mut sa, mut sb) = (String::new(), String::new());
        a.text(ra, &mut sa);
        b.text(rb, &mut sb);
        let (x, y) = char_diff(&sa, &sb, opts.ignore_case);
        (!x.is_empty() || !y.is_empty()).then_some((x, y))
    }
}

fn is_gb(s: &str, i: usize) -> bool {
    i == 0 || i >= s.len() || GraphemeCursor::new(i, s.len(), true).is_boundary(s, 0) != Ok(false)
}

fn same(x: char, y: char, fold: bool) -> bool {
    x == y || (fold && x.to_lowercase().eq(y.to_lowercase()))
}

/// The ranges of `a` and `b` left after the characters they share at the start and at the end
/// are cut; cuts are moved to grapheme boundaries in both.
fn char_diff(a: &str, b: &str, fold: bool) -> (Range<usize>, Range<usize>) {
    let (mut pa, mut pb) = (0, 0);
    let (mut ia, mut ib) = (a.chars(), b.chars());
    loop {
        match (ia.next(), ib.next()) {
            (Some(x), Some(y)) if same(x, y, fold) => {
                pa += x.len_utf8();
                pb += y.len_utf8();
            }
            _ => break,
        }
    }
    // Back to a boundary in both, a character of each at a time (they pair up).
    while pa > 0 && !(is_gb(a, pa) && is_gb(b, pb)) {
        pa -= a[..pa].chars().next_back().map_or(0, char::len_utf8);
        pb -= b[..pb].chars().next_back().map_or(0, char::len_utf8);
    }
    let (mut ea, mut eb) = (a.len(), b.len());
    let (mut ra, mut rb) = (a[pa..].chars().rev(), b[pb..].chars().rev());
    loop {
        match (ra.next(), rb.next()) {
            (Some(x), Some(y)) if same(x, y, fold) => {
                ea -= x.len_utf8();
                eb -= y.len_utf8();
            }
            _ => break,
        }
    }
    while ea < a.len() && !(is_gb(a, ea) && is_gb(b, eb)) {
        ea += a[ea..].chars().next().map_or(0, char::len_utf8);
        eb += b[eb..].chars().next().map_or(0, char::len_utf8);
    }
    (pa..ea.max(pa), pb..eb.max(pb))
}

/// Compares the lines of `a` and `b` (see the module doc). Reads each line of each buffer once.
/// `cancel` is looked at while lines are hashed and while the comparison runs; when it is set
/// the result is coarse and `cancelled`.
pub fn diff(a: &Buffer, b: &Buffer, opts: DiffOptions, cancel: &AtomicBool) -> Diff {
    let (na, nb) = (line_count(a), line_count(b));
    let coarse = |cancelled: bool| {
        let run: &[Run] = &[];
        let mut d = Diff::from_runs(run, na, nb);
        d.approximate = na > 0 && nb > 0;
        d.cancelled = cancelled;
        d
    };
    if na > DIFF_MAX_LINES || nb > DIFF_MAX_LINES {
        return coarse(false);
    }
    let mut interner = Interner::default();
    let Some(ia) = intern(a, na, opts, &mut interner, cancel) else {
        return coarse(true);
    };
    let Some(ib) = intern(b, nb, opts, &mut interner, cancel) else {
        return coarse(true);
    };
    let (runs, approximate, cancelled) = match_ids(&ia, &ib, interner.len(), DIFF_WORK, cancel);
    let mut d = Diff::from_runs(&runs, na, nb);
    d.approximate = approximate || cancelled;
    d.cancelled = cancelled;
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::Rng;
    use std::time::Instant;

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    fn cmp(a: &str, b: &str, opts: DiffOptions) -> Diff {
        diff(
            &Buffer::from_text(a),
            &Buffer::from_text(b),
            opts,
            &no_cancel(),
        )
    }

    fn plain(a: &str, b: &str) -> Diff {
        cmp(a, b, DiffOptions::default())
    }

    fn kinds(d: &Diff) -> Vec<(HunkKind, Range<usize>, Range<usize>)> {
        d.hunks()
            .iter()
            .map(|h| (h.kind, h.a.clone(), h.b.clone()))
            .collect()
    }

    fn lines_of(text: &str) -> Vec<String> {
        let b = Buffer::from_text(text);
        (0..line_count(&b))
            .map(|i| {
                let mut s = String::new();
                b.line_text(i, &mut s);
                s
            })
            .collect()
    }

    /// The hunks tile both files, equal hunks hold equal lines, and applying them to `a`'s
    /// lines gives `b`'s.
    fn check(d: &Diff, a: &[String], b: &[String]) {
        assert_eq!(d.lines(), (a.len(), b.len()));
        let (mut ia, mut ib) = (0, 0);
        let mut out: Vec<&String> = Vec::new();
        for h in d.hunks() {
            assert_eq!(
                (h.a.start, h.b.start),
                (ia, ib),
                "hunks must follow each other"
            );
            match h.kind {
                HunkKind::Equal => {
                    assert_eq!(h.a.len(), h.b.len());
                    assert!(!h.a.is_empty());
                    for k in 0..h.a.len() {
                        assert_eq!(a[h.a.start + k], b[h.b.start + k]);
                    }
                }
                HunkKind::Insert => assert!(h.a.is_empty() && !h.b.is_empty()),
                HunkKind::Delete => assert!(!h.a.is_empty() && h.b.is_empty()),
                HunkKind::Change => assert!(!h.a.is_empty() && !h.b.is_empty()),
            }
            match h.kind {
                HunkKind::Equal => out.extend(&a[h.a.clone()]),
                _ => out.extend(&b[h.b.clone()]),
            }
            (ia, ib) = (h.a.end, h.b.end);
        }
        assert_eq!((ia, ib), (a.len(), b.len()), "hunks must cover both files");
        assert!(
            out.iter().copied().eq(b.iter()),
            "applying the hunks to a must give b"
        );
        // Rows: every line shows once, in order, on its own side.
        let (mut ra, mut rb) = (0, 0);
        let mut n = 0;
        for (i, row) in d.rows().enumerate() {
            assert_eq!(d.row(i), Some(row));
            assert!(row.a.is_some() || row.b.is_some());
            if let Some(l) = row.a {
                assert_eq!(l, ra);
                assert_eq!(d.row_of_a(l), Some(i));
                ra += 1;
            }
            if let Some(l) = row.b {
                assert_eq!(l, rb);
                assert_eq!(d.row_of_b(l), Some(i));
                rb += 1;
            }
            n += 1;
        }
        assert_eq!((ra, rb, n), (a.len(), b.len(), d.row_count()));
        assert_eq!(d.row(n), None);
        assert_eq!(d.row_of_a(a.len()), None);
        assert_eq!(d.row_of_b(b.len()), None);
    }

    fn check_text(d: &Diff, a: &str, b: &str) {
        check(d, &lines_of(a), &lines_of(b));
    }

    fn lcs(a: &[u32], b: &[u32]) -> usize {
        let mut t = vec![vec![0usize; b.len() + 1]; a.len() + 1];
        for i in 0..a.len() {
            for j in 0..b.len() {
                t[i + 1][j + 1] = if a[i] == b[j] {
                    t[i][j] + 1
                } else {
                    t[i][j + 1].max(t[i + 1][j])
                };
            }
        }
        t[a.len()][b.len()]
    }

    #[test]
    fn identical_and_empty() {
        let d = plain("a\nb\nc\n", "a\nb\nc\n");
        assert!(d.is_identical() && !d.approximate);
        assert_eq!(kinds(&d), vec![(HunkKind::Equal, 0..3, 0..3)]);
        assert_eq!(d.stats(), DiffStats::default());
        let d = plain("", "");
        assert!(d.hunks().is_empty() && d.row_count() == 0 && d.is_identical());
        let d = plain("", "x\ny\n");
        assert_eq!(kinds(&d), vec![(HunkKind::Insert, 0..0, 0..2)]);
        let d = plain("x\ny", "");
        assert_eq!(kinds(&d), vec![(HunkKind::Delete, 0..2, 0..0)]);
        assert_eq!(d.stats().removed, 2);
        // One empty line is a line; the empty text has none.
        let d = plain("\n", "");
        assert_eq!(kinds(&d), vec![(HunkKind::Delete, 0..1, 0..0)]);
        // A final line break does not make a line.
        assert!(plain("a\n", "a").is_identical());
    }

    #[test]
    fn inserts_and_deletes() {
        let d = plain("b\nc\n", "a\nb\nc\n");
        assert_eq!(
            kinds(&d),
            vec![
                (HunkKind::Insert, 0..0, 0..1),
                (HunkKind::Equal, 0..2, 1..3)
            ]
        );
        let d = plain("a\nb\nc\n", "a\nc\n");
        assert_eq!(
            kinds(&d),
            vec![
                (HunkKind::Equal, 0..1, 0..1),
                (HunkKind::Delete, 1..2, 1..1),
                (HunkKind::Equal, 2..3, 1..2)
            ]
        );
        let d = plain("a\nb\n", "a\nb\nc\nd\n");
        assert_eq!(
            kinds(&d),
            vec![
                (HunkKind::Equal, 0..2, 0..2),
                (HunkKind::Insert, 2..2, 2..4)
            ]
        );
        let d = plain("a\nb\nc\nd\n", "a\nb\n");
        assert_eq!(
            kinds(&d),
            vec![
                (HunkKind::Equal, 0..2, 0..2),
                (HunkKind::Delete, 2..4, 2..2)
            ]
        );
        check_text(&d, "a\nb\nc\nd\n", "a\nb\n");
        let s = d.stats();
        assert_eq!((s.added, s.removed, s.changed, s.hunks), (0, 2, 0, 1));
    }

    #[test]
    fn replaced_block() {
        let (a, b) = ("a\nb\nc\nd\ne\n", "a\nX\nY\nZ\ne\n");
        let d = plain(a, b);
        assert_eq!(
            kinds(&d),
            vec![
                (HunkKind::Equal, 0..1, 0..1),
                (HunkKind::Change, 1..4, 1..4),
                (HunkKind::Equal, 4..5, 4..5)
            ]
        );
        check_text(&d, a, b);
        let s = d.stats();
        assert_eq!((s.added, s.removed, s.changed, s.hunks), (0, 0, 3, 1));
        // Uneven: 2 lines become 3.
        let d = plain("a\nb\nc\n", "a\nX\nY\nZ\n");
        let s = d.stats();
        assert_eq!((s.added, s.removed, s.changed, s.hunks), (1, 0, 2, 1));
        let rows: Vec<_> = d.rows().collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            (rows[3].a, rows[3].b, rows[3].kind),
            (None, Some(3), HunkKind::Change)
        );
    }

    #[test]
    fn moved_block_is_delete_and_insert() {
        let a = "1\n2\n3\n4\n5\n6\n";
        let b = "5\n6\n1\n2\n3\n4\n";
        let d = plain(a, b);
        check_text(&d, a, b);
        let k = kinds(&d);
        assert!(
            k.iter().all(|(kind, ..)| *kind != HunkKind::Change),
            "{k:?}"
        );
        assert_eq!(
            k,
            vec![
                (HunkKind::Insert, 0..0, 0..2),
                (HunkKind::Equal, 0..4, 2..6),
                (HunkKind::Delete, 4..6, 6..6)
            ]
        );
    }

    #[test]
    fn white_space_and_case_options() {
        let a = "x = 1;  \nif (a)  b\nHello\n";
        let b = "x = 1;\nif (a) b\nHELLO\n";
        assert_eq!(plain(a, b).stats().changed, 3);
        let tr = DiffOptions {
            ignore_trailing_ws: true,
            ..Default::default()
        };
        let d = cmp(a, b, tr);
        // Only the first line is equal then: "if (a)  b" has inner blanks, "Hello" has case.
        assert_eq!(d.stats().changed, 2);
        let ws = DiffOptions {
            ignore_ws: true,
            ..Default::default()
        };
        let d = cmp(a, b, ws);
        assert_eq!(d.stats().changed, 1);
        assert_eq!(kinds(&d)[1], (HunkKind::Change, 2..3, 2..3));
        let all = DiffOptions {
            ignore_ws: true,
            ignore_case: true,
            ..Default::default()
        };
        let d = cmp(a, b, all);
        assert!(d.is_identical() && !d.approximate);
        let ic = DiffOptions {
            ignore_case: true,
            ..Default::default()
        };
        assert_eq!(cmp("Straße\nİ\n", "STRASSE\ni̇\n", ic).stats().changed, 1);
        assert!(cmp("ÄÖ\n", "äö\n", ic).is_identical());
        // Blank-only lines are equal to empty ones when white space is ignored.
        assert!(cmp(" \t\n", "\n", tr).is_identical());
        assert!(!plain(" \t\n", "\n").is_identical());
    }

    #[test]
    fn line_endings_do_not_matter() {
        let d = plain("a\r\nb\r\nc\r\n", "a\nb\nc\n");
        assert!(d.is_identical());
        assert!(plain("a\rb\rc", "a\r\nb\nc\r\n").is_identical());
        let d = plain("a\r\nb\r\nc", "a\nX\nc\n");
        assert_eq!(kinds(&d)[1], (HunkKind::Change, 1..2, 1..2));
    }

    #[test]
    fn random_small_cases_are_minimal() {
        let mut rng = Rng::new(7);
        for _ in 0..3000 {
            let alpha = 1 + rng.below(4);
            let na = rng.below(14);
            let nb = rng.below(14);
            let a: Vec<u32> = (0..na).map(|_| rng.below(alpha) as u32).collect();
            let b: Vec<u32> = (0..nb).map(|_| rng.below(alpha) as u32).collect();
            let text = |v: &[u32]| v.iter().map(|x| format!("{x}\n")).collect::<String>();
            let (ta, tb) = (text(&a), text(&b));
            let d = plain(&ta, &tb);
            check_text(&d, &ta, &tb);
            assert!(!d.approximate);
            let equal: usize = d
                .hunks()
                .iter()
                .filter(|h| h.kind == HunkKind::Equal)
                .map(|h| h.a.len())
                .sum();
            assert_eq!(equal, lcs(&a, &b), "{a:?} vs {b:?}");
        }
    }

    #[test]
    fn rows_navigation_and_mapping() {
        let a = "k\n1\nm\n2\nn\n3\n";
        let b = "k\nm\nX\nY\nn\n3\nZ\n";
        let d = plain(a, b);
        check_text(&d, a, b);
        let k = kinds(&d);
        assert_eq!(
            k,
            vec![
                (HunkKind::Equal, 0..1, 0..1),
                (HunkKind::Delete, 1..2, 1..1),
                (HunkKind::Equal, 2..3, 1..2),
                (HunkKind::Change, 3..4, 2..4),
                (HunkKind::Equal, 4..6, 4..6),
                (HunkKind::Insert, 6..6, 6..7)
            ]
        );
        let rows: Vec<_> = d.rows().map(|r| (r.a, r.b, r.kind)).collect();
        assert_eq!(rows[1], (Some(1), None, HunkKind::Delete));
        assert_eq!(rows[2], (Some(2), Some(1), HunkKind::Equal));
        assert_eq!(rows[3], (Some(3), Some(2), HunkKind::Change));
        assert_eq!(rows[4], (None, Some(3), HunkKind::Change));
        assert_eq!(rows.len(), 8);
        assert_eq!(d.row_of_a(3), Some(3));
        assert_eq!(d.row_of_b(3), Some(4));
        assert_eq!(d.row_of_b(6), Some(7));
        // Navigation: changed hunks are 1, 3 and 5 (rows 1, 3..5, 7).
        assert_eq!(d.next_hunk(0), Some(1));
        assert_eq!(d.next_hunk(1), Some(3));
        assert_eq!(d.next_hunk(2), Some(3));
        assert_eq!(d.next_hunk(3), Some(5));
        assert_eq!(d.next_hunk(7), None);
        assert_eq!(d.next_hunk(1000), None);
        assert_eq!(d.prev_hunk(0), None);
        assert_eq!(d.prev_hunk(1), None);
        assert_eq!(d.prev_hunk(2), Some(1));
        assert_eq!(d.prev_hunk(4), Some(3));
        assert_eq!(d.prev_hunk(7), Some(3));
        assert_eq!(d.prev_hunk(8), Some(5));
        assert_eq!(d.prev_hunk(9), Some(5));
        assert_eq!(d.hunk_rows(3), 3..5);
        assert_eq!(d.hunk_at(4), Some(3));
        assert_eq!(d.hunk_at(2), None);
        assert_eq!(d.hunk_at(99), None);
        assert_eq!(d.stats().hunks, 3);
    }

    #[test]
    fn intra_line_ranges() {
        let (ta, tb) = (
            "keep\nlet value = 10;\nend\n",
            "keep\nlet value = 200;\nend\n",
        );
        let (a, b) = (Buffer::from_text(ta), Buffer::from_text(tb));
        let o = DiffOptions::default();
        let d = diff(&a, &b, o, &no_cancel());
        assert_eq!(d.intra_line(0, &a, &b, o), None);
        let (x, y) = d.intra_line(1, &a, &b, o).unwrap();
        assert_eq!(&"let value = 10;"[x.clone()], "1");
        assert_eq!(&"let value = 200;"[y.clone()], "20");
        // Common start and end are not shared: all of the lines differ.
        let (a, b) = (Buffer::from_text("abc\n"), Buffer::from_text("xyz\n"));
        let d = diff(&a, &b, o, &no_cancel());
        assert_eq!(d.intra_line(0, &a, &b, o), Some((0..3, 0..3)));
        // Pure insertion inside a line: an empty range on the left.
        let (x, y) = char_diff("ab", "aXb", false);
        assert_eq!((x, y), (1..1, 1..2));
        // Different counts, or a row of an equal hunk: nothing.
        let (a, b) = (Buffer::from_text("a\nb\n"), Buffer::from_text("x\n"));
        let d = diff(&a, &b, o, &no_cancel());
        assert_eq!(d.intra_line(0, &a, &b, o), None);
    }

    #[test]
    fn intra_line_respects_characters_and_graphemes() {
        // Multi-byte letters: the cut is never inside a char.
        let (x, y) = char_diff("héllo wörld", "héllo wörld!", false);
        assert_eq!((x, y), (13..13, 13..14));
        let (x, y) = char_diff("aéb", "aèb", false);
        assert_eq!(
            ("aéb".get(x.clone()), "aèb".get(y.clone())),
            (Some("é"), Some("è"))
        );
        // 'e' + combining acute against 'e' + combining grave: the base letter is not shared
        // alone, the whole cluster is replaced.
        let (a, b) = ("ae\u{301}z", "ae\u{300}z");
        let (x, y) = char_diff(a, b, false);
        assert_eq!((&a[x], &b[y]), ("e\u{301}", "e\u{300}"));
        // Emoji with a skin tone modifier against another tone.
        let (a, b) = ("x\u{1F44D}\u{1F3FB}y", "x\u{1F44D}\u{1F3FF}y");
        let (x, y) = char_diff(a, b, false);
        assert_eq!((&a[x], &b[y]), ("\u{1F44D}\u{1F3FB}", "\u{1F44D}\u{1F3FF}"));
        // Case folding.
        let (x, y) = char_diff("Hello World", "hello there", true);
        assert_eq!((&"Hello World"[x], &"hello there"[y]), ("World", "there"));
        // Never panics, whatever the pair.
        let mut rng = Rng::new(3);
        let alphabet = ['a', 'é', '\u{301}', '\u{1F44D}', '\u{200D}', 'ß', ' '];
        for _ in 0..2000 {
            let s = |rng: &mut Rng| -> String {
                (0..rng.below(8)).map(|_| *rng.pick(&alphabet)).collect()
            };
            let (a, b) = (s(&mut rng), s(&mut rng));
            let (x, y) = char_diff(&a, &b, rng.chance(50));
            assert!(a.is_char_boundary(x.start) && a.is_char_boundary(x.end));
            assert!(b.is_char_boundary(y.start) && b.is_char_boundary(y.end));
            assert!(x.start <= x.end && y.start <= y.end);
            assert!(x.end <= a.len() && y.end <= b.len());
        }
    }

    #[test]
    fn bound_falls_back_to_a_coarse_block() {
        // A trimmed middle that is hard (alternating, so no line is left out) with a budget too
        // small for it: one changed block, flagged, and the ends still compared exactly.
        let mut rng = Rng::new(11);
        let mid =
            |rng: &mut Rng| -> Vec<u32> { (0..400).map(|_| rng.below(3) as u32 + 10).collect() };
        let mut a = vec![1, 2, 3];
        a.extend(mid(&mut rng));
        a.extend([7, 8, 9]);
        let mut b = vec![1, 2, 3];
        b.extend(mid(&mut rng));
        b.extend([7, 8, 9]);
        let (runs, approx, cancelled) = match_ids(&a, &b, 20, 200, &no_cancel());
        assert!(approx && !cancelled);
        let d = Diff::from_runs(&runs, a.len(), b.len());
        let k = kinds(&d);
        assert_eq!(k.first().unwrap().0, HunkKind::Equal);
        assert_eq!(k.last().unwrap().0, HunkKind::Equal);
        assert!(d.hunks().first().unwrap().a.len() >= 3);
        // With the real budget it is exact.
        let (runs, approx, _) = match_ids(&a, &b, 20, DIFF_WORK, &no_cancel());
        assert!(!approx);
        let equal: usize = runs.iter().map(|r| r.len).sum();
        assert_eq!(equal, lcs(&a, &b));
    }

    #[test]
    fn too_many_lines_is_one_block() {
        let big = "x\n".repeat(DIFF_MAX_LINES + 1);
        let a = Buffer::from_text(&big);
        let b = Buffer::from_text("y\nx\n");
        let d = diff(&a, &b, DiffOptions::default(), &no_cancel());
        assert!(d.approximate);
        assert_eq!(
            kinds(&d),
            vec![(HunkKind::Change, 0..DIFF_MAX_LINES + 1, 0..2)]
        );
        let d = diff(
            &a,
            &Buffer::from_text(""),
            DiffOptions::default(),
            &no_cancel(),
        );
        assert!(!d.approximate);
        assert_eq!(d.stats().removed, DIFF_MAX_LINES + 1);
    }

    #[test]
    fn cancel_gives_a_coarse_result() {
        let yes = AtomicBool::new(true);
        let (a, b) = (Buffer::from_text("a\nb\n"), Buffer::from_text("a\nc\n"));
        let d = diff(&a, &b, DiffOptions::default(), &yes);
        assert!(d.cancelled && d.approximate);
        check(&d, &lines_of("a\nb\n"), &lines_of("a\nc\n"));
    }

    /// `n` lines with `edits` random changes (replace, insert, delete, in blocks); returns both
    /// texts.
    fn random_files(n: usize, edits: usize, seed: u64) -> (String, String) {
        let mut rng = Rng::new(seed);
        let a: Vec<String> = (0..n)
            .map(|i| format!("line {i} {}", rng.below(1000)))
            .collect();
        let mut b = a.clone();
        for _ in 0..edits {
            if b.is_empty() {
                break;
            }
            let at = rng.below(b.len());
            let len = 1 + rng.below(4);
            match rng.below(3) {
                0 => {
                    for l in b.iter_mut().skip(at).take(len) {
                        l.push_str(" edited");
                    }
                }
                1 => {
                    let end = (at + len).min(b.len());
                    b.drain(at..end);
                }
                _ => {
                    for j in 0..len {
                        b.insert(at, format!("new {} {}", rng.below(100_000), j));
                    }
                }
            }
        }
        let join = |v: &[String]| v.iter().map(|l| format!("{l}\n")).collect::<String>();
        (join(&a), join(&b))
    }

    #[test]
    fn two_hundred_thousand_lines_with_random_edits() {
        let (ta, tb) = random_files(200_000, 2000, 5);
        let started = Instant::now();
        let d = plain(&ta, &tb);
        let took = started.elapsed();
        check_text(&d, &ta, &tb);
        assert!(!d.approximate);
        assert!(d.stats().hunks > 500);
        eprintln!("diff of 200k lines, 2000 edits: {took:?}");
    }

    #[test]
    fn many_equal_lines_and_repeats() {
        // Few distinct lines: ids repeat a lot.
        let mut rng = Rng::new(21);
        let mk = |rng: &mut Rng, n: usize| -> String {
            (0..n).map(|_| format!("{}\n", rng.below(3))).collect()
        };
        for _ in 0..40 {
            let (a, b) = (mk(&mut rng, 300), mk(&mut rng, 280));
            let d = plain(&a, &b);
            check_text(&d, &a, &b);
        }
    }

    #[test]
    fn cancel_flag_during_a_big_diff() {
        let (ta, tb) = random_files(20_000, 500, 9);
        let (a, b) = (Buffer::from_text(&ta), Buffer::from_text(&tb));
        let yes = AtomicBool::new(true);
        let d = diff(&a, &b, DiffOptions::default(), &yes);
        assert!(d.cancelled);
    }

    /// `cargo test --release -p mdedit-core diff_perf -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn diff_perf() {
        let time = |name: &str, ta: &str, tb: &str| {
            let (a, b) = (Buffer::from_text(ta), Buffer::from_text(tb));
            let started = Instant::now();
            let d = diff(&a, &b, DiffOptions::default(), &no_cancel());
            let took = started.elapsed();
            let s = d.stats();
            eprintln!(
                "{name}: {took:?}, hunks {}, +{} -{} ~{}, approximate {}",
                s.hunks, s.added, s.removed, s.changed, d.approximate
            );
        };
        let (a, b) = random_files(200_000, 2000, 5);
        time("200k lines, 2000 edits", &a, &b);
        let (a, b) = random_files(200_000, 40_000, 6);
        time("200k lines, 40000 edits", &a, &b);
        let (a, b) = random_files(2_000_000, 2000, 7);
        {
            let buf = Buffer::from_text(&a);
            let t = Instant::now();
            let mut it = Interner::default();
            let n = line_count(&buf);
            intern(&buf, n, DiffOptions::default(), &mut it, &no_cancel());
            eprintln!("  intern, 2M lines: {:?}", t.elapsed());
        }
        time("2M lines, 2000 edits", &a, &b);
        let (a, b) = random_files(2_000_000, 10_000, 7);
        time("2M lines, 10000 edits", &a, &b);
        // Worst case for the bound: both sides draw from 1000 lines, so nothing is left out
        // and there is no common prefix.
        let mut rng = Rng::new(1);
        let mk = |rng: &mut Rng| -> String {
            (0..2_000_000)
                .map(|_| format!("v{}\n", rng.below(1000)))
                .collect()
        };
        let (a, b) = (mk(&mut rng), mk(&mut rng));
        time("2M vs 2M random lines from 1000 values (worst)", &a, &b);
        let mk = |rng: &mut Rng| -> String {
            (0..200_000)
                .map(|_| format!("v{}\n", rng.below(4)))
                .collect()
        };
        let (a, b) = (mk(&mut rng), mk(&mut rng));
        time("200k vs 200k random lines from 4 values", &a, &b);
        // Same file twice, and a file against an empty one.
        time("2M identical", &a.repeat(10), &a.repeat(10));
    }
}
