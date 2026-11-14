//! The minimap: a narrow column at the right edge of the text area that shows the document in
//! miniature, one row per line that shows (folded lines are skipped, word wrap is ignored), with
//! the lines the editor shows marked by a translucent band. DESIGN.md section 28.
//!
//! Nothing is laid out as text: a run of non-whitespace characters is a filled rectangle of its
//! colour at 60 % over the page. The column scrolls in proportion to the editor, so that its
//! window is at most the column's height whatever the size of the file: only the rows of the
//! window are read, never the whole document. The window is drawn into a cached bitmap, which a
//! frame that changes only the band's look (the pointer over it) blits again.

use crate::chrome::Ctx;
use crate::docs::Ready;
use mdedit_highlight::{Span, foreground};
use mdshared_text::Bitmap;
use mdshared_ui::gfx::{Rect, Rgb, mix};
use std::hash::{Hash, Hasher};
use std::ops::Range;

/// The column's width, logical px.
pub const WIDTH: f32 = 96.0;
/// The text area keeps at least this width (logical px) beside the minimap, or the minimap hides.
pub const TEXT_MIN: f32 = 320.0;
/// How much of a character's colour shows over the page.
const INK_A: f32 = 0.78;
/// The viewport band: the text colour at this alpha, and while the pointer is over the column or
/// dragging.
const BAND_A: f32 = 0.08;
const BAND_HOT_A: f32 = 0.14;
/// Bytes of a line read for its marks; the column is cut well before this many columns.
const LINE_BYTES: usize = 512;
/// The caret's row: the text colour at this alpha over the full width.
const CARET_A: f32 = 0.22;
/// The selection's colour over the rows.
const SEL_A: f32 = 0.55;
/// Find matches marked at most per drawing.
const MATCHES_MAX: usize = 4096;
/// Lines a request for colours covers at most (folds can hide many lines in the window).
const LINES_MAX: usize = 10_000;

/// The sizes of the marks in device px at one display scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Px {
    /// A row's ink, and the gap under it.
    pub ink: i32,
    pub gap: i32,
    /// A character column's width.
    pub col: i32,
}

impl Px {
    /// 2 px of ink, 1 px of gap and 1 px per column at scale 1.0, scaled and rounded, each at
    /// least 1 px.
    pub fn new(scale: f32) -> Px {
        let at = |v: f32| ((v * scale).round() as i32).max(1);
        Px {
            ink: at(2.0),
            gap: at(1.0),
            col: at(1.0),
        }
    }

    pub fn row_h(self) -> i32 {
        self.ink + self.gap
    }
}

/// One mark of a line: `len` columns from column `col`, in `color` (`None`: the text colour).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seg {
    pub col: u32,
    pub len: u32,
    pub color: Option<Rgb>,
}

/// The marks of a line's characters: each run of non-whitespace characters of one colour is one
/// segment. Tabs go to the next multiple of `tab` columns, every other character takes one, and
/// nothing is made past column `max_cols`. `spans` are the line's colours in bytes, sorted (a
/// character outside every span has none).
pub fn segments(
    text: impl Iterator<Item = char>,
    spans: &[Span],
    tab: usize,
    max_cols: usize,
    out: &mut Vec<Seg>,
) {
    let tab = tab.max(1);
    let (mut col, mut byte, mut si) = (0usize, 0usize, 0usize);
    let mut cur: Option<(usize, Option<Rgb>)> = None;
    let mut flush = |cur: &mut Option<(usize, Option<Rgb>)>, col: usize| {
        if let Some((start, color)) = cur.take() {
            out.push(Seg {
                col: start as u32,
                len: (col - start) as u32,
                color,
            });
        }
    };
    for ch in text {
        if col >= max_cols {
            break;
        }
        if ch.is_whitespace() {
            flush(&mut cur, col);
            col += if ch == '\t' { tab - col % tab } else { 1 };
        } else {
            while si < spans.len() && (spans[si].end as usize) <= byte {
                si += 1;
            }
            let color = spans
                .get(si)
                .filter(|s| (s.start as usize) <= byte)
                .map(|s| s.color);
            if cur.is_some_and(|(_, c)| c != color) {
                flush(&mut cur, col);
            }
            cur.get_or_insert((col, color));
            col += 1;
        }
        byte += ch.len_utf8();
    }
    flush(&mut cur, col.min(max_cols));
}

/// Where the column's window is and where the band sits in it, for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plan {
    /// Rows in the document: the lines that show.
    pub rows: usize,
    /// A row's height, and the column's, device px.
    pub rh: i32,
    pub col_h: i32,
    /// How far the document's rows are scrolled up inside the column, device px.
    pub top_px: i32,
    /// The editor's view in rows: where it starts (fractional) and how many it spans.
    pub band_first: f64,
    pub band_rows: f64,
    /// What the band's first row is counted over to give the scroll fraction: the rows less the
    /// band's, or with word wrap the rows less one, as the editor's own scrollbar has it.
    pub range: f64,
}

impl Plan {
    /// When all rows fit the column the document starts at its top; when they do not, the column
    /// scrolls in proportion to the editor: `top = fraction * (rows_px - column_px)`, where the
    /// fraction is the band's place over its range.
    #[cfg(test)]
    pub fn new(rows: usize, band_first: f64, band_rows: f64, rh: i32, col_h: i32) -> Plan {
        Plan::with_range(
            rows,
            band_first,
            band_rows,
            rows as f64 - band_rows,
            rh,
            col_h,
        )
    }

    /// [`new`](Self::new) with the editor's scroll range given.
    pub fn with_range(
        rows: usize,
        band_first: f64,
        band_rows: f64,
        range: f64,
        rh: i32,
        col_h: i32,
    ) -> Plan {
        let rh = rh.max(1);
        let total = rows as i64 * rh as i64;
        let fraction = if range > 0.0 {
            (band_first / range).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let over = total - col_h as i64;
        let top_px = if over > 0 {
            (fraction * over as f64).round() as i32
        } else {
            0
        };
        Plan {
            rows,
            rh,
            col_h,
            top_px,
            band_first,
            band_rows,
            range,
        }
    }

    /// The first row drawn, and how far (device px) its top is above the column's.
    pub fn first_row(&self) -> usize {
        (self.top_px / self.rh) as usize
    }

    pub fn offset(&self) -> i32 {
        self.top_px % self.rh
    }

    /// Rows that touch the column.
    pub fn row_count(&self) -> usize {
        let n = (self.col_h + self.offset() + self.rh - 1) / self.rh;
        (n.max(0) as usize).min(self.rows.saturating_sub(self.first_row()))
    }

    /// The band's top and bottom in device px from the column's top, kept inside the column.
    pub fn band(&self) -> (i32, i32) {
        let rh = self.rh as f64;
        let y0 = (self.band_first * rh).round() as i64 - self.top_px as i64;
        let y1 = ((self.band_first + self.band_rows) * rh).round() as i64 - self.top_px as i64;
        let (y0, y1) = (
            y0.clamp(0, self.col_h as i64),
            y1.clamp(0, self.col_h as i64),
        );
        (
            y0 as i32,
            y1.max(y0).max((y0 + 1).min(self.col_h as i64)) as i32,
        )
    }

    /// The row under `y` device px from the column's top, as a fraction.
    pub fn row_f(&self, y: f64) -> f64 {
        (y + self.top_px as f64) / self.rh as f64
    }

    /// The row under `y`, clamped to the rows there are.
    pub fn row_at(&self, y: f64) -> usize {
        let r = self.row_f(y).floor();
        if r <= 0.0 {
            0
        } else {
            (r as usize).min(self.rows.saturating_sub(1))
        }
    }

    /// Where the band starts when its top is at `y` device px from the column's top: the inverse
    /// of the column's scrolling, so that dragging moves the band with the pointer.
    pub fn first_for_band_top(&self, y: f64) -> f64 {
        let span = self.range;
        if span <= 0.0 {
            return 0.0;
        }
        let rh = self.rh as f64;
        let over = self.rows as f64 * rh - self.col_h as f64;
        let first = if over <= 0.0 {
            y / rh
        } else {
            let k = rh - over / span;
            if k <= 1e-9 { 0.0 } else { y / k }
        };
        first.clamp(0.0, span)
    }

    /// Where the band starts to centre the view on the row under `y`.
    pub fn first_to_centre_on(&self, y: f64) -> f64 {
        let span = self.range.max(0.0);
        (self.row_at(y) as f64 + 0.5 - self.band_rows / 2.0).clamp(0.0, span)
    }

    /// The rows of the window as the lines to ask colours for, a half window more each side.
    pub fn colour_rows(&self) -> Range<usize> {
        let n = self.row_count();
        let first = self.first_row();
        first.saturating_sub(n / 2)..(first + n + n / 2).min(self.rows)
    }
}

/// The lines of the rows `rows`, as the range of the document's lines they span (at most
/// [`LINES_MAX`] of them): `line_at_row` is the editor's fold mapping.
pub fn lines_of_rows(rows: Range<usize>, line_at_row: impl Fn(usize) -> usize) -> Range<usize> {
    if rows.is_empty() {
        return 0..0;
    }
    let start = line_at_row(rows.start);
    let end = line_at_row(rows.end - 1) + 1;
    start..end.min(start + LINES_MAX)
}

/// The cached drawing of the column's window: rows without the band.
#[derive(Default)]
pub struct Cache {
    key: u64,
    bmp: Option<Bitmap>,
}

fn fill(bmp: &mut Bitmap, x0: i32, y0: i32, x1: i32, y1: i32, c: Rgb) {
    let (w, h) = (bmp.width as i32, bmp.height as i32);
    let (x0, x1, y0, y1) = (x0.max(0), x1.min(w), y0.max(0), y1.min(h));
    if x0 >= x1 {
        return;
    }
    let px = [c[2], c[1], c[0], 255];
    for y in y0..y1 {
        let o = (y as usize * w as usize + x0 as usize) * 4;
        for d in bmp.pixels[o..o + (x1 - x0) as usize * 4]
            .as_chunks_mut::<4>()
            .0
        {
            d.copy_from_slice(&px);
        }
    }
}

/// Draws the column into `rect` (logical px): the window's rows from the cache (made again when
/// something it shows changed), then the band.
pub fn draw(
    c: &mut Ctx,
    cache: &mut Cache,
    r: Option<(&Ready, u64, u64)>,
    plan: Option<&Plan>,
    rect: Rect,
    look: Look,
) {
    let pal = *c.pal;
    let scale = c.p.scale;
    let (x0, y0, x1, y1) = c.p.dev_rect(rect);
    let (w, h) = ((x1 - x0).max(0), (y1 - y0).max(0));
    c.p.fill(rect, pal.page);
    let hair = (scale.round() as i32).max(1);
    c.p.fill_dev(x0, y0, x0 + hair, y1, pal.line, 1.0);
    if look.right_line {
        c.p.fill_dev(x1 - hair, y0, x1, y1, pal.line, 1.0);
    }
    c.hit(look.hit, rect);
    let (Some((r, id, revision)), Some(plan)) = (r, plan) else {
        return;
    };
    if w == 0 || h == 0 {
        return;
    }
    let dark = pal.dark;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (
        id,
        revision,
        plan.top_px,
        plan.rows,
        plan.col_h,
        r.map_colors.generation,
        r.colors.generation,
        dark,
        scale.to_bits(),
        (w, h),
        r.folds.ranges(),
    )
        .hash(&mut hasher);
    let key = hasher.finish();
    let fresh = cache
        .bmp
        .as_ref()
        .is_some_and(|b| b.width as i32 == w && b.height as i32 == h);
    if !fresh || cache.key != key {
        let bmp = match cache.bmp.take() {
            Some(b) if b.width as i32 == w && b.height as i32 == h => b,
            _ => Bitmap::new(w as u32, h as u32),
        };
        let bmp = cache.bmp.insert(bmp);
        bmp.clear(pal.page);
        paint(bmp, r, plan, scale, dark, pal.page, hair);
        cache.key = key;
    }
    if let Some(bmp) = &cache.bmp {
        // The hairline at the left is the region's own.
        c.p.blit(bmp, 0, h, x0, y0);
    }
    c.p.fill_dev(x0, y0, x0 + hair, y1, pal.line, 1.0);
    let right = x1 - if look.right_line { hair } else { 0 };
    marks(c, r, plan, (x0, y0, right), look.keyboard);
    let (b0, b1) = plan.band();
    if b1 > b0 {
        let a = if look.hot { BAND_HOT_A } else { BAND_A };
        c.p.fill_dev(x0 + hair, y0 + b0, right, y0 + b1, pal.text, a);
    }
}

/// How the column is drawn this time.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// The pointer is over the column or a drag runs.
    pub hot: bool,
    /// The pane has the keyboard: its caret's row is marked.
    pub keyboard: bool,
    /// A hairline at the right edge too (the left pane's column, where the panes part).
    pub right_line: bool,
    /// The hit the column is: the pane's own.
    pub hit: crate::ui::Hit,
}

/// The column of byte `byte` of a line's text: characters one column each, tabs to the next
/// multiple of `tab`.
pub fn col_of(text: impl Iterator<Item = char>, byte: usize, tab: usize) -> usize {
    let tab = tab.max(1);
    let (mut col, mut b) = (0, 0);
    for ch in text {
        if b >= byte {
            break;
        }
        col += if ch == '\t' { tab - col % tab } else { 1 };
        b += ch.len_utf8();
    }
    col
}

/// The columns a selection covers of one line. `line` is the line's bytes without its break,
/// `next` where the line after it starts. `None` when nothing of the line is selected; otherwise
/// the first column and where it ends, `None` there meaning the selection runs through the
/// line's end (the break is selected) and the mark takes the rest of the row.
pub fn sel_cols(
    text: &str,
    line: Range<usize>,
    next: usize,
    sel: Range<usize>,
    tab: usize,
) -> Option<(usize, Option<usize>)> {
    if sel.start >= next || sel.end <= line.start {
        return None;
    }
    let through = sel.end > line.end && next > line.end;
    let a = sel.start.saturating_sub(line.start);
    let b = sel.end.min(line.end) - line.start;
    let from = col_of(text.chars(), a, tab);
    if through {
        return Some((from, None));
    }
    let to = col_of(text.chars(), b, tab);
    (to > from).then_some((from, Some(to)))
}

/// Draws the marks over the rows of the window: the caret's row, the selections, the find
/// matches at the right edge, the bookmarks at the left. Only the window's lines are looked at.
/// `at` is the column's left, top and right edge in device px.
fn marks(c: &mut Ctx, r: &Ready, plan: &Plan, at: (i32, i32, i32), keyboard: bool) {
    let count = plan.row_count();
    if count == 0 {
        return;
    }
    let pal = *c.pal;
    let scale = c.p.scale;
    let (x0, y0, x1) = at;
    let px = Px::new(scale);
    let hair = (scale.round() as i32).max(1);
    let pad = ((4.0 * scale).round() as i32).max(1);
    let left = x0 + hair + pad;
    let max_cols = ((x1 - left - pad) / px.col).max(0);
    let first = plan.first_row();
    let (first_line, last_line) = (
        r.folds.line_at_row(first),
        r.folds.line_at_row(first + count - 1),
    );
    let buf = &r.buffer;
    let (ws, we) = (
        buf.line_range(first_line).start,
        buf.line_range_with_break(last_line).end,
    );
    // The top of a line's row, if the line is in the window.
    let row_y = |line: usize| -> Option<i32> {
        if line < first_line || line > last_line || r.folds.is_hidden(line) {
            return None;
        }
        let k = r.folds.row_of(line).checked_sub(first)?;
        (k < count).then(|| y0 + k as i32 * plan.rh - plan.offset())
    };
    let mut fill = |x: i32, y: i32, w: i32, h: i32, col: Rgb, a: f32| {
        c.p.fill_dev(x, y, x + w, y + h, col, a);
    };
    if keyboard && let Some(y) = row_y(buf.line_of(r.sel.caret)) {
        fill(x0 + hair, y, x1 - x0 - hair, px.ink, pal.text, CARET_A);
    }
    // Selections: the primary one and the others of several carets, those in the window.
    let mut sels: Vec<Range<usize>> = std::iter::once(r.sel.range())
        .chain(r.extra.iter().map(|s| s.range()))
        .filter(|s| s.start < we && s.end > ws && s.start < s.end)
        .collect();
    sels.sort_by_key(|s| s.start);
    let mut text = String::new();
    for s in &sels {
        let (a, b) = (buf.line_of(s.start.max(ws)), buf.line_of(s.end.min(we)));
        for line in a.max(first_line)..=b.min(last_line) {
            let Some(y) = row_y(line) else { continue };
            let range = buf.line_range(line);
            let next = buf.line_range_with_break(line).end;
            let end = (range.start + LINE_BYTES).min(range.end);
            let rope = buf.rope();
            let end = rope.char_to_byte(rope.byte_to_char(end));
            text.clear();
            text.extend(rope.byte_slice(range.start..end).chars());
            let Some((from, to)) = sel_cols(
                &text,
                range.clone(),
                next,
                s.clone(),
                crate::live::tab_width(),
            ) else {
                continue;
            };
            let from = (from as i32).min(max_cols);
            let to = to.map_or(max_cols, |t| (t as i32).min(max_cols));
            if to > from {
                fill(
                    left + from * px.col,
                    y,
                    (to - from) * px.col,
                    px.row_h(),
                    pal.selection,
                    SEL_A,
                );
            }
        }
    }
    // Find matches: a mark at the row's right edge.
    let window = r.match_window();
    let all = &r.find.matches;
    let a = all.partition_point(|m| m.end <= ws).max(window.start);
    let b = all.partition_point(|m| m.start < we).min(window.end);
    let mw = ((3.0 * scale).round() as i32).max(1);
    for (k, m) in all[a..b.max(a)].iter().enumerate().take(MATCHES_MAX) {
        let Some(y) = row_y(buf.line_of(m.start)) else {
            continue;
        };
        let col = if r.find.current == Some(a + k) {
            pal.find_current
        } else {
            pal.find_mark
        };
        fill(x1 - mw, y, mw, px.ink, col, 0.95);
    }
    // Bookmarks: a mark at the row's left edge.
    let a = r.bookmarks.partition_point(|&l| l < first_line);
    let b = r.bookmarks.partition_point(|&l| l <= last_line);
    for &line in &r.bookmarks[a..b.max(a)] {
        if let Some(y) = row_y(line) {
            fill(x0 + hair, y, mw, px.ink, pal.accent, 1.0);
        }
    }
}

/// Paints the rows of the window into `bmp`.
fn paint(bmp: &mut Bitmap, r: &Ready, plan: &Plan, scale: f32, dark: bool, page: Rgb, hair: i32) {
    let px = Px::new(scale);
    let w = bmp.width as i32;
    let pad = ((4.0 * scale).round() as i32).max(1);
    let left = hair + pad;
    let max_cols = ((w - left - pad) / px.col).max(0) as usize;
    if max_cols == 0 {
        return;
    }
    let plain = foreground(dark);
    let mut last: Option<(Rgb, Rgb)> = None;
    let mut ink = |c: Rgb| -> Rgb {
        match last {
            Some((from, to)) if from == c => to,
            _ => {
                let to = mix(page, c, INK_A);
                last = Some((c, to));
                to
            }
        }
    };
    let rope = r.buffer.rope();
    let first = plan.first_row();
    let mut segs = Vec::new();
    for k in 0..plan.row_count() {
        let line = r.folds.line_at_row(first + k);
        let y = k as i32 * plan.rh - plan.offset();
        let range = r.buffer.line_range(line);
        let end = (range.start + LINE_BYTES).min(range.end);
        // Down to the start of the character it falls in.
        let end = rope.char_to_byte(rope.byte_to_char(end));
        let chars = rope.byte_slice(range.start..end).chars();
        let spans = r
            .map_colors
            .line(line, dark)
            .or_else(|| r.colors.line(line, dark))
            .unwrap_or(&[]);
        segs.clear();
        segments(chars, spans, crate::live::tab_width(), max_cols, &mut segs);
        for s in &segs {
            let x = left + s.col as i32 * px.col;
            let c = ink(s.color.unwrap_or(plain));
            fill(bmp, x, y, x + s.len as i32 * px.col, y + px.ink, c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: u32, end: u32, color: Rgb) -> Span {
        Span { start, end, color }
    }

    fn segs(text: &str, spans: &[Span], max_cols: usize) -> Vec<(u32, u32, Option<Rgb>)> {
        let mut out = Vec::new();
        segments(text.chars(), spans, 4, max_cols, &mut out);
        out.iter().map(|s| (s.col, s.len, s.color)).collect()
    }

    #[test]
    fn marks_follow_the_scale_and_never_vanish() {
        assert_eq!(
            Px::new(1.0),
            Px {
                ink: 2,
                gap: 1,
                col: 1
            }
        );
        assert_eq!(Px::new(1.0).row_h(), 3);
        assert_eq!(
            Px::new(1.5),
            Px {
                ink: 3,
                gap: 2,
                col: 2
            }
        );
        assert_eq!(
            Px::new(2.0),
            Px {
                ink: 4,
                gap: 2,
                col: 2
            }
        );
        let tiny = Px::new(0.25);
        assert_eq!((tiny.ink, tiny.gap, tiny.col), (1, 1, 1));
    }

    #[test]
    fn a_run_of_non_whitespace_is_one_segment() {
        assert_eq!(segs("fn main", &[], 100), [(0, 2, None), (3, 4, None)]);
        assert_eq!(segs("   ", &[], 100), []);
        assert_eq!(segs("", &[], 100), []);
        assert_eq!(segs("  ab  cd ", &[], 100), [(2, 2, None), (6, 2, None)]);
    }

    #[test]
    fn tabs_advance_to_the_tab_stop() {
        assert_eq!(segs("\tx", &[], 100), [(4, 1, None)]);
        assert_eq!(segs("a\tb", &[], 100), [(0, 1, None), (4, 1, None)]);
        assert_eq!(segs("abcd\te", &[], 100), [(0, 4, None), (8, 1, None)]);
        assert_eq!(segs("\t\tx", &[], 100), [(8, 1, None)]);
    }

    #[test]
    fn the_line_is_cut_at_the_columns_that_fit() {
        assert_eq!(segs("abcdefgh", &[], 5), [(0, 5, None)]);
        assert_eq!(segs("ab cdefgh", &[], 5), [(0, 2, None), (3, 2, None)]);
        // A tab that crosses the edge leaves nothing after it.
        assert_eq!(segs("a\tb", &[], 3), [(0, 1, None)]);
        assert_eq!(segs("abc", &[], 0), []);
        // A long line is not read past the cut.
        let long = "x".repeat(10_000);
        assert_eq!(segs(&long, &[], 87), [(0, 87, None)]);
    }

    #[test]
    fn a_run_takes_the_colour_of_its_characters() {
        let red = [200, 0, 0];
        let blue = [0, 0, 200];
        // "let x" with `let` red and `x` blue; the space has no span.
        let spans = [span(0, 3, red), span(4, 5, blue)];
        assert_eq!(
            segs("let x", &spans, 100),
            [(0, 3, Some(red)), (4, 1, Some(blue))]
        );
        // One run in two colours is two segments, side by side.
        let spans = [span(0, 2, red), span(2, 5, blue)];
        assert_eq!(
            segs("ab(cd", &spans, 100),
            [(0, 2, Some(red)), (2, 3, Some(blue))]
        );
        // Spans are in bytes: a two-byte character is one column.
        let spans = [span(0, 2, red), span(2, 3, blue)];
        assert_eq!(
            segs("\u{e7}b", &spans, 100),
            [(0, 1, Some(red)), (1, 1, Some(blue))]
        );
        // Stale spans that end early leave the rest plain.
        assert_eq!(
            segs("abcd", &[span(0, 2, red)], 100),
            [(0, 2, Some(red)), (2, 2, None)]
        );
    }

    #[test]
    fn columns_follow_tabs_and_bytes() {
        assert_eq!(col_of("abc".chars(), 2, 4), 2);
        assert_eq!(col_of("a\tb".chars(), 2, 4), 4);
        assert_eq!(col_of("\u{e7}b".chars(), 2, 4), 1);
        assert_eq!(col_of("abc".chars(), 99, 4), 3);
        assert_eq!(col_of("".chars(), 0, 4), 0);
    }

    #[test]
    fn a_selection_covers_columns_or_the_rest_of_the_row() {
        // The line "hello world" is bytes 10..21, the next line starts at 22.
        let (t, line, next) = ("hello world", 10..21, 22);
        let s = |r: std::ops::Range<usize>| sel_cols(t, line.clone(), next, r, 4);
        assert_eq!(s(12..15), Some((2, Some(5))));
        assert_eq!(s(10..21), Some((0, Some(11))));
        // Through the break: the rest of the row.
        assert_eq!(s(10..22), Some((0, None)));
        assert_eq!(s(15..40), Some((5, None)));
        // From an earlier line through this one.
        assert_eq!(s(0..16), Some((0, Some(6))));
        assert_eq!(s(0..40), Some((0, None)));
        // Not this line.
        assert_eq!(s(0..10), None);
        assert_eq!(s(22..30), None);
        assert_eq!(s(12..12), None);
        // An empty line selected through its break is a full row.
        assert_eq!(sel_cols("", 30..30, 31, 25..40, 4), Some((0, None)));
        // The last line, with no break: to its end.
        assert_eq!(sel_cols("ab", 5..7, 7, 5..7, 4), Some((0, Some(2))));
        // Tabs count to the stop.
        assert_eq!(sel_cols("a\tb", 0..3, 4, 2..3, 4), Some((4, Some(5))));
    }

    #[test]
    fn a_wrapped_view_scrolls_over_the_rows_less_one() {
        // 1000 rows, the band 10 rows high, in a 600 px column: the fraction is first / 999.
        let p = Plan::with_range(1000, 499.5, 10.0, 999.0, 3, 600);
        assert_eq!(p.top_px, (0.5 * 2400.0) as i32);
        let end = Plan::with_range(1000, 999.0, 1.0, 999.0, 3, 600);
        assert_eq!(end.top_px, 2400);
        // The drag inverts it over the same range.
        let y = p.band().0 as f64;
        assert!((p.first_for_band_top(y) - 499.5).abs() < 2.0);
        assert_eq!(p.first_for_band_top(1.0e9), 999.0);
    }

    #[test]
    fn rows_that_fit_start_at_the_top() {
        // 100 rows of 3 px in a 600 px column.
        let p = Plan::new(100, 40.0, 30.0, 3, 600);
        assert_eq!((p.top_px, p.first_row(), p.offset()), (0, 0, 0));
        assert_eq!(p.row_count(), 100);
        assert_eq!(p.band(), (120, 210));
        // Exactly full.
        let p = Plan::new(200, 170.0, 30.0, 3, 600);
        assert_eq!(p.top_px, 0);
        assert_eq!(p.row_count(), 200);
    }

    #[test]
    fn a_long_document_scrolls_in_proportion() {
        // 1000 rows = 3000 px in a 600 px column, the editor showing 40 rows.
        let top = Plan::new(1000, 0.0, 40.0, 3, 600);
        assert_eq!(top.top_px, 0);
        assert_eq!(top.band(), (0, 120));
        let end = Plan::new(1000, 960.0, 40.0, 3, 600);
        assert_eq!(end.top_px, 2400);
        assert_eq!((end.first_row(), end.offset()), (800, 0));
        assert_eq!(end.row_count(), 200);
        assert_eq!(end.band(), (480, 600));
        // Half way: half of the 2400 px.
        let mid = Plan::new(1000, 480.0, 40.0, 3, 600);
        assert_eq!(mid.top_px, 1200);
        assert_eq!(mid.band(), (240, 360));
        // A partly scrolled row shows its lower part first.
        let part = Plan::new(1000, 481.0, 40.0, 3, 600);
        assert_eq!(part.top_px, (481.0_f64 / 960.0 * 2400.0).round() as i32);
        assert!(part.offset() > 0 || part.top_px % 3 == 0);
        assert_eq!(part.row_count() as i32, (600 + part.offset() + 2) / 3);
        // Past the range, the fraction stops at 1.
        let over = Plan::new(1000, 5000.0, 40.0, 3, 600);
        assert_eq!(over.top_px, 2400);
    }

    #[test]
    fn the_window_is_at_most_the_column_whatever_the_file() {
        // A hundred million lines.
        let p = Plan::new(100_000_000, 50_000_000.0, 40.0, 3, 800);
        assert!(p.row_count() <= 800 / 3 + 2);
        assert!(p.top_px > 0);
        let p = Plan::new(100_000_000, 99_999_960.0, 40.0, 3, 800);
        assert_eq!(p.first_row() + p.row_count(), 100_000_000);
        // Rows and gaps scale the window's size, not the document's.
        let p = Plan::new(100_000_000, 3.0, 40.0, 6, 1600);
        assert!(p.row_count() <= 1600 / 6 + 2);
    }

    #[test]
    fn a_pointer_y_is_a_row() {
        let p = Plan::new(1000, 480.0, 40.0, 3, 600);
        // top_px = 1200: the column's top shows row 400.
        assert_eq!(p.row_at(0.0), 400);
        assert_eq!(p.row_at(2.0), 400);
        assert_eq!(p.row_at(3.0), 401);
        assert_eq!(p.row_at(599.0), 599);
        // Above and below the column the rows clamp.
        assert_eq!(p.row_at(-50_000.0), 0);
        assert_eq!(p.row_at(1.0e9), 999);
        let small = Plan::new(10, 0.0, 5.0, 3, 600);
        assert_eq!(small.row_at(599.0), 9);
        assert_eq!(small.row_at(31.0), 9);
    }

    #[test]
    fn a_row_is_a_line_with_folds() {
        // Lines 4..=7 hidden: rows 0..=3 are lines 0..=3, row 4 is line 8.
        let line_at_row = |row: usize| if row <= 3 { row } else { row + 4 };
        let p = Plan::new(96, 0.0, 10.0, 3, 600);
        assert_eq!(line_at_row(p.row_at(12.0)), 8);
        assert_eq!(line_at_row(p.row_at(9.0)), 3);
        assert_eq!(lines_of_rows(0..10, line_at_row), 0..14);
        assert_eq!(lines_of_rows(2..2, line_at_row), 0..0);
        // Only so many lines are asked for.
        assert_eq!(lines_of_rows(0..5, |r| r * 100_000), 0..LINES_MAX);
    }

    #[test]
    fn dragging_inverts_the_scrolling() {
        for (rows, first) in [
            (1000usize, 480.0),
            (1000, 0.0),
            (1000, 960.0),
            (1000, 123.0),
        ] {
            let p = Plan::new(rows, first, 40.0, 3, 600);
            let (y0, _) = p.band();
            let back = p.first_for_band_top(y0 as f64);
            // The band's top is in whole px, so a row is the best this can match.
            assert!((back - first).abs() < 3.0, "{rows} {first} {back}");
        }
        // Rows that fit: a pixel is a third of a row.
        let p = Plan::new(100, 10.0, 30.0, 3, 600);
        assert_eq!(p.first_for_band_top(60.0), 20.0);
        // The band cannot go past either end.
        assert_eq!(p.first_for_band_top(-100.0), 0.0);
        assert_eq!(p.first_for_band_top(10_000.0), 70.0);
        // Nothing to scroll when the band is the whole document.
        let all = Plan::new(30, 0.0, 30.0, 3, 600);
        assert_eq!(all.first_for_band_top(50.0), 0.0);
    }

    #[test]
    fn a_press_centres_the_view_on_the_line() {
        // Rows fit: the press at row 50 puts the 30-row band round it, 35.5 to 65.5.
        let p = Plan::new(100, 0.0, 30.0, 3, 600);
        assert_eq!(p.first_to_centre_on(150.0 + 1.0), 35.5);
        // Near either end the band stops at the end.
        assert_eq!(p.first_to_centre_on(3.0), 0.0);
        assert_eq!(p.first_to_centre_on(297.0), 70.0);
        // A scrolled column: the row under y is counted from its top.
        let p = Plan::new(1000, 480.0, 40.0, 3, 600);
        assert_eq!(p.first_to_centre_on(300.0), 480.5);
    }

    #[test]
    fn colours_are_asked_for_the_window_and_half_a_window_each_side() {
        let p = Plan::new(1000, 480.0, 40.0, 3, 600);
        let (first, n) = (p.first_row(), p.row_count());
        let want = p.colour_rows();
        assert_eq!(want.start, first - n / 2);
        assert_eq!(want.end, first + n + n / 2);
        let p = Plan::new(100, 0.0, 30.0, 3, 600);
        assert_eq!(p.colour_rows(), 0..100);
    }
}
