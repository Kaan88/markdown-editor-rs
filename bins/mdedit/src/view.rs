//! The text view: font metrics, gutter, line layout (whole lines and chunks), painting of text,
//! selection, find marks and caret, hit testing, both scrollbars. DESIGN.md section 10.
//!
//! Lines are shaped with the view's own parley context in the monospace family and drawn with its
//! own `TextEngine` straight into the window's composed bitmap. A line of at most
//! `WHOLE_LINE_BYTES` is one layout; a longer one is cut by a `display::ChunkIndex` and only the
//! chunks on screen, plus one each side, are shaped. A chunk is placed at the x its start's counts
//! give: narrow columns times the advance plus wide characters times the measured width of a
//! wide glyph, zero-width characters nothing. Layouts are cached by what they show (a hash of the
//! text, its start column, colours, theme), not by where it is, so an edit re-shapes only the
//! lines whose text changed and equal lines share one layout.

pub mod cache;
pub mod metrics;
pub mod wrap;

use crate::chrome::Ctx;
use crate::docs::{Doc, DocState, Ready};
use cache::Lru;
use mdedit_core::display::{ChunkIndex, ChunkStart, Cols, DisplayLine};
use mdedit_core::fold::heads_block;
use mdedit_core::limits::TWIN_LINE_MAX;
use mdedit_core::limits::{TAB_WIDTH, WHOLE_LINE_BYTES};
use mdedit_highlight::{Span, foreground};
use mdshared_text::{TextEngine, shared_fonts};
use mdshared_ui::gfx::{Face, Rect, Rgb, mix};
use mdshared_ui::glyph;
use mdshared_ui::scroll;
use metrics::*;
use parley::{
    Affinity, Alignment, AlignmentOptions, Cursor, FontContext, FontFamily, Layout, LayoutContext,
    StyleProperty,
};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub type Brush = [u8; 4];

#[cfg(windows)]
pub const MONO: &str = "Consolas, Cascadia Mono, Noto Sans Mono, DejaVu Sans Mono, monospace";
/// The colour emoji families come before the `monospace` generic: that generic can reach a
/// monochrome font that has the emoji before any emoji font is tried.
#[cfg(not(windows))]
pub const MONO: &str =
    "Consolas, Cascadia Mono, Noto Sans Mono, DejaVu Sans Mono, Noto Color Emoji, monospace";
/// What a wide glyph's width is measured on.
const WIDE_SAMPLE: &str =
    "\u{6F22}\u{5B57}\u{6F22}\u{5B57}\u{6F22}\u{5B57}\u{6F22}\u{5B57}\u{6F22}\u{5B57}";
/// Long-line chunk indexes a document keeps (one per line).
pub const CHUNK_INDEXES: usize = 256;
/// The banner over the text.
pub const BANNER_H: f32 = 34.0;
/// What shows at a line's end with Show whitespace on: LF, CRLF, CR.
const EOL_MARKS: [&str; 3] = ["\u{2193}", "\u{21B5}", "\u{2190}"];
/// The horizontal scrollbar's band at the bottom of the text area.
const HBAR_H: f32 = 12.0;
/// Screens of lines the layout cache holds.
const CACHE_SCREENS: usize = 4;

/// Font metrics at one scale and zoom, device px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    pub scale: f32,
    pub zoom: u32,
    pub size: f32,
    pub advance: f32,
    /// The width of a wide (CJK) glyph, measured in the fallback font the system gives.
    pub wide: f32,
    pub line_h: i32,
    /// Where the baseline sits below a line's top.
    pub baseline: f32,
}

impl Metrics {
    /// The advance in logical px.
    pub fn advance_l(&self) -> f32 {
        self.advance / self.scale
    }

    /// Where a chunk starts, device px from column 0.
    fn chunk_x(&self, c: &ChunkStart) -> f64 {
        cols_x(c.cols(), self.advance, self.wide)
    }
}

/// One shaped line or chunk, and the map between its display text and the buffer.
pub struct Shaped {
    pub layout: Layout<Brush>,
    pub dl: DisplayLine,
    /// Device px.
    pub width: f32,
    /// The layout's first baseline, device px from its top.
    pub baseline: f32,
}

impl Shaped {
    /// The x of an offset inside this text (bytes from its start), device px from its start.
    pub fn x_of(&self, offset: usize) -> f32 {
        let i = self.dl.to_display(offset);
        if i == 0 {
            return 0.0;
        }
        if i >= self.dl.text.len() {
            return self.width;
        }
        Cursor::from_byte_index(&self.layout, i, Affinity::Downstream)
            .geometry(&self.layout, 0.0)
            .x0 as f32
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
struct LineKey {
    /// A hash of the text shaped, and its length.
    text: u64,
    len: usize,
    /// The display column the text starts at (tab stops depend on it).
    col: usize,
    /// Where the text starts in its line, which the colour spans are relative to.
    in_line: usize,
    /// A hash of the line's colour spans; 0 when it has none.
    colors: u64,
    dark: bool,
}

fn text_hash(text: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Where things are in the text region this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Geom {
    /// The text region below the banner, logical px.
    pub area: Rect,
    pub gutter_w: f32,
    /// Lines in the buffer.
    pub lines: usize,
    pub view_lines: f64,
    /// Text width (gutter excluded), logical px.
    pub view_w: f32,
    pub extent: f32,
    /// Word wrap: `top` is a line and a row in it, and nothing scrolls sideways.
    pub wrapped: bool,
    /// Lines that show: all but those inside folds.
    pub rows: usize,
    /// The view's top as the vertical scrollbar measures it: a row among those that show, or
    /// with word wrap the top line's number.
    pub top: f64,
}

/// One line as drawn: its place and its parts, each a chunk with the device x of its start
/// from column 0 and its y. Without word wrap the parts are the chunks in view, side by side;
/// with it they are the line's rows in view, one below the other.
struct ShownLine {
    line: usize,
    /// The row it is on (the line's own number unless compare mode has fillers).
    row: usize,
    y: i32,
    h: i32,
    /// The line's start is among the parts (the line number goes beside it).
    first: bool,
    parts: Vec<(Chunk, f64, i32)>,
}

/// Scrolls the text by `by` rows, down when positive. The next layout pass clamps it.
pub fn scroll_rows(r: &mut Ready, by: f64) {
    if r.wrapped() {
        wrap::scroll(r, by);
    } else if r.folds.is_empty() {
        r.top += by;
    } else {
        let v = top_rows(r) + by;
        set_top_rows(r, v);
    }
}

/// The view's top as a row among the lines that show. `Ready::top` is a line of the document,
/// so that it means the same when folds open and close; without folds the two are one.
fn top_rows(r: &Ready) -> f64 {
    if r.folds.is_empty() || !r.top.is_finite() || r.top <= 0.0 {
        return r.top;
    }
    let line = r.top.floor();
    let shown = r.folds.shown(line as usize);
    let part = if shown == line as usize {
        r.top - line
    } else {
        0.0
    };
    r.folds.row_of(shown) as f64 + part
}

fn set_top_rows(r: &mut Ready, v: f64) {
    if r.folds.is_empty() || !v.is_finite() || v <= 0.0 {
        r.top = v;
        return;
    }
    let row = v.floor();
    r.top = r.folds.line_at_row(row as usize) as f64 + (v - row);
}

/// Lines that show: all but those inside folds.
fn shown_rows(r: &Ready) -> usize {
    let lines = r.buffer.len_lines().max(1);
    if let Some(al) = &r.align {
        return al.rows(lines);
    }
    lines - r.folds.hidden().min(lines - 1)
}

/// How strongly a changed row is tinted, and the part of it that differs.
const ROW_TINT: f32 = 0.13;
const ROW_TINT_DARK: f32 = 0.2;
const INTRA_TINT: f32 = 0.32;

/// Most lines a view asks colours for when folds are on screen.
const SHOWN_LINES_MAX: usize = 10_000;

/// The lines the view shows, from its top to its bottom, hidden ones between them included:
/// what colours are asked for. Past [`SHOWN_LINES_MAX`] lines it stops, so the text below a
/// larger fold stays plain while the fold is on screen.
pub fn shown_lines(r: &Ready) -> Range<usize> {
    let lines = r.buffer.len_lines();
    if let Some(al) = &r.align {
        // Compare mode: the top is a row, and fillers have no line.
        let rows = visible(r.top, r.view_lines, al.rows(lines));
        if rows.is_empty() {
            return 0..0;
        }
        let last = al.line_near(rows.end - 1, lines);
        return al.line_near(rows.start, lines)..(last + 1).min(lines);
    }
    let vis = visible(r.top, r.view_lines, lines);
    if r.folds.is_empty() {
        return vis;
    }
    let rows = r.view_lines.ceil().max(1.0) as usize + 1;
    let end = r.folds.line_at_row(r.folds.row_of(vis.start) + rows);
    vis.start..end.clamp(vis.start, lines.min(vis.start + SHOWN_LINES_MAX))
}

/// What the minimap needs of the view: the rows that show, where the view is among them (its
/// first row, fractional, and how many rows it spans) and the range its scroll position runs
/// over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapScene {
    pub rows: usize,
    pub first: f64,
    pub span: f64,
    pub range: f64,
}

/// The view as the minimap draws it. Without word wrap it is the rows in view exactly. With it
/// the minimap still has one row per line, and the view is placed in them to a wrapped row: a
/// line's row is shared equally by its wrapped rows, so the band starts and ends at the fraction
/// of a line the top and the bottom row of the view are. (The bottom row may be only partly in
/// view and counts whole, so the band can be up to one wrapped row too tall.) The scroll range
/// is the one of the editor's own scrollbar with wrap: the top line, over the lines less one.
pub fn map_scene(r: &mut Ready, g: &Geom) -> MapScene {
    if !g.wrapped {
        return MapScene {
            rows: g.rows,
            first: g.top,
            span: g.view_lines,
            range: g.rows as f64 - g.view_lines,
        };
    }
    let at = |r: &mut Ready, p: wrap::Pos, after: bool| {
        let n = wrap::rows(r, p.line).len().max(1) as f64;
        let part = (p.row + usize::from(after)) as f64 / n;
        r.folds.row_of(r.folds.shown(p.line)) as f64 + part
    };
    let top = wrap::top(r);
    let first = at(r, top, false);
    let n = (g.view_lines.ceil() as isize - 1).max(0);
    let bottom = wrap::step(r, top, n);
    let last = at(r, bottom, true);
    MapScene {
        rows: g.rows,
        first,
        span: (last - first).max(f64::MIN_POSITIVE),
        range: (g.rows as f64 - 1.0).max(0.0),
    }
}

/// Puts the view's first row at `first` (a row among those that show) for the minimap; with word
/// wrap, at the fraction of that row's line, in wrapped rows.
pub fn map_scroll(r: &mut Ready, g: &Geom, first: f64) {
    if g.wrapped {
        let f = if first.is_finite() {
            first.max(0.0)
        } else {
            0.0
        };
        let row = (f.floor() as usize).min(g.rows.saturating_sub(1));
        let line = r.folds.line_at_row(row);
        let n = wrap::rows(r, line).len().max(1);
        let part = (f - row as f64).clamp(0.0, 1.0);
        r.top = line as f64;
        r.top_row = ((part * n as f64) as usize).min(n - 1);
        r.row_rest = 0.0;
    } else {
        scroll_to(r, g, first);
    }
}

/// Puts line `top` (fractional without word wrap) at the view's top: a scrollbar's answer.
pub fn scroll_to(r: &mut Ready, g: &Geom, top: f64) {
    if g.wrapped {
        let last = g.lines.saturating_sub(1) as f64;
        let line = if top.is_finite() {
            top.floor().clamp(0.0, last)
        } else {
            0.0
        };
        r.top = r.folds.shown(line as usize) as f64;
        r.top_row = 0;
        r.row_rest = 0.0;
    } else {
        set_top_rows(r, clamp_top(top, g.rows, g.view_lines));
    }
}

impl Geom {
    /// What the vertical scrollbar measures: content and view, in lines. With word wrap the
    /// thumb's place is the top line's number, so the last line can reach the top.
    pub fn vscroll(&self) -> (f32, f32) {
        let view = self.view_lines as f32;
        if self.wrapped {
            ((self.lines as f32 - 1.0).max(0.0) + view, view)
        } else {
            (self.rows as f32, view)
        }
    }

    /// Device x of display column 0 at the current horizontal scroll.
    fn origin_x(&self, scale: f32, left: f64) -> f64 {
        ((self.area.x + self.gutter_w + TEXT_PAD) * scale) as f64 - (left * scale as f64).round()
    }

    fn text_rect(&self) -> Rect {
        Rect::new(
            self.area.x + self.gutter_w,
            self.area.y,
            (self.area.w - self.gutter_w).max(0.0),
            self.area.h,
        )
    }

    /// The vertical scrollbar's track and thumb.
    pub fn vbar(&self) -> Option<(Rect, Rect)> {
        let (content, view) = self.vscroll();
        scroll::scrollbar(self.area, content, view, self.top as f32)
    }

    /// The horizontal scrollbar's track and thumb, the same geometry turned on its side.
    pub fn hbar(&self, left: f64) -> Option<(Rect, Rect)> {
        let t = self.text_rect();
        hscrollbar(
            Rect::new(t.x, t.y, (t.w - HBAR_H).max(0.0), t.h),
            self.extent,
            self.view_w,
            left as f32,
        )
    }
}

/// The horizontal track and thumb for `area`, or `None` when everything fits.
pub fn hscrollbar(area: Rect, content: f32, view: f32, offset: f32) -> Option<(Rect, Rect)> {
    if content <= view + 0.5 || area.w < 24.0 || area.h < HBAR_H {
        return None;
    }
    let track = Rect::new(area.x + 2.0, area.bottom() - HBAR_H, area.w - 4.0, HBAR_H);
    let len = (track.w * view / content).clamp(28.0_f32.min(track.w), track.w);
    let t = (offset / (content - view)).clamp(0.0, 1.0);
    let thumb = Rect::new(track.x + t * (track.w - len), track.y, len, track.h);
    Some((track, thumb))
}

/// The scroll offset that puts the horizontal thumb's left edge at `thumb_x`.
pub fn offset_for_hthumb(track: Rect, thumb_w: f32, thumb_x: f32, content: f32, view: f32) -> f32 {
    let span = (track.w - thumb_w).max(1.0);
    ((thumb_x - track.x) / span).clamp(0.0, 1.0) * (content - view).max(0.0)
}

fn draw_hscrollbar(c: &mut Ctx, track: Rect, thumb: Rect, active: bool) {
    let h = if active { 8.0 } else { 3.0 };
    if active {
        c.p.rbox_a(
            Rect::new(track.x, track.bottom() - h - 3.0, track.w, h),
            h / 2.0,
            c.pal.thumb,
            0.10,
        );
    }
    let r = Rect::new(thumb.x, track.bottom() - h - 3.0, thumb.w, h);
    c.p.rbox_a(r, h / 2.0, c.pal.thumb, if active { 0.85 } else { 0.55 });
}

/// What the text region draws that is not in the document.
#[derive(Clone, Copy, Debug, Default)]
pub struct DrawState {
    pub dark: bool,
    pub caret: bool,
    /// The caret's line has its band and its number stands out: the pane has the keyboard.
    pub band: bool,
    /// Spaces, tabs and line endings show as marks.
    pub whitespace: bool,
    pub vbar_active: bool,
    pub hbar_active: bool,
    /// The second pane of a split view: its scrollbars hit as its own.
    pub other: bool,
}

fn spans_hash(spans: Option<&[Span]>) -> u64 {
    let Some(spans) = spans.filter(|s| !s.is_empty()) else {
        return 0;
    };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for s in spans {
        (s.start, s.end, s.color).hash(&mut h);
    }
    h.finish() | 1
}

/// One chunk of a line: where it starts and the offset it ends at.
#[derive(Clone, Copy, Debug)]
struct Chunk {
    start: ChunkStart,
    end: usize,
}

/// Runs `f` on the chunk index of `line`, made (and kept) on first use. `None` for a line short
/// enough to be shaped whole.
fn with_index<T>(
    r: &mut Ready,
    line: usize,
    f: impl FnOnce(&mut ChunkIndex, &mdedit_core::Buffer) -> T,
) -> Option<T> {
    if r.buffer.line_range(line).len() <= WHOLE_LINE_BYTES {
        return None;
    }
    if r.chunks.get_mut(&line).is_none() {
        r.chunks.insert(line, ChunkIndex::new(&r.buffer, line));
    }
    let index = r.chunks.get_mut(&line)?;
    Some(f(index, &r.buffer))
}

/// The chunk `k` of a complete enough index (its end is known).
fn chunk_k(index: &ChunkIndex, k: usize, line_end: usize) -> Chunk {
    let s = index.starts();
    Chunk {
        start: s[k],
        end: s.get(k + 1).map_or(line_end, |n| n.offset),
    }
}

fn whole(r: &Ready, line: usize) -> Chunk {
    let range = r.buffer.line_range(line);
    Chunk {
        start: ChunkStart {
            offset: range.start,
            ..ChunkStart::default()
        },
        end: range.end,
    }
}

/// The chunk of `line` that holds `offset`.
fn chunk_at_offset(r: &mut Ready, line: usize, offset: usize) -> Chunk {
    let end = r.buffer.line_range(line).end;
    with_index(r, line, |index, buf| {
        index.extend_to(buf, TAB_WIDTH, offset);
        let k = metrics::chunk_at_offset(index.starts(), offset);
        chunk_k(index, k, end)
    })
    .unwrap_or_else(|| whole(r, line))
}

/// The chunk of `line` under `x` (px from column 0, in the units of `advance` and `wide`).
fn chunk_at_x(r: &mut Ready, line: usize, x: f64, advance: f32, wide: f32) -> Chunk {
    let end = r.buffer.line_range(line).end;
    let cx = |c: &ChunkStart| cols_x(c.cols(), advance, wide);
    with_index(r, line, |index, buf| {
        index.extend_while(buf, TAB_WIDTH, |last| cx(last) <= x);
        let k = chunk_at_x_in(index.starts(), x, cx);
        let mut extra = 0;
        index.extend_while(buf, TAB_WIDTH, |_| {
            extra += 1;
            extra <= 1
        });
        chunk_k(index, k, end)
    })
    .unwrap_or_else(|| whole(r, line))
}

/// The chunks of `line` to draw for the device x range `xs`: those overlapping it plus one each
/// side.
fn chunks_in_view(r: &mut Ready, line: usize, xs: Range<f64>, m: &Metrics) -> Vec<Chunk> {
    let end = r.buffer.line_range(line).end;
    let cx = |c: &ChunkStart| m.chunk_x(c);
    with_index(r, line, |index, buf| {
        index.extend_while(buf, TAB_WIDTH, |last| cx(last) <= xs.end);
        let mut extra = 0;
        index.extend_while(buf, TAB_WIDTH, |_| {
            extra += 1;
            extra <= 2
        });
        let shown = metrics::chunks_in_view(index.starts(), xs.clone(), cx);
        shown.map(|k| chunk_k(index, k, end)).collect()
    })
    .unwrap_or_else(|| vec![whole(r, line)])
}

/// The estimated x of `offset` from its line's column 0, logical px: the chunk's start plus the
/// counts of the text before `offset` in it, with the advances of the last frame.
pub fn x_of(r: &mut Ready, offset: usize) -> f64 {
    let line = r.buffer.line_of(offset);
    let c = chunk_at_offset(r, line, offset);
    let mut text = String::new();
    r.buffer
        .text(c.start.offset..offset.max(c.start.offset), &mut text);
    let cols: Cols = c.start.cols().after(&text, TAB_WIDTH);
    cols_x(cols, r.advance, r.wide_advance)
}

/// Keeps `offset` in view (with the margin), or centres it when `center` and it is off screen.
/// Uses the view size of the last frame.
pub fn reveal(r: &mut Ready, offset: usize, center: bool) {
    reveal_with(r, offset, center, true);
}

/// [`reveal`] without the margin: scrolls only when `offset` is outside the view (a selection
/// drag, whose pointer inside the text must not move it).
pub fn reveal_no_margin(r: &mut Ready, offset: usize) {
    reveal_with(r, offset, false, false);
}

fn reveal_with(r: &mut Ready, offset: usize, center: bool, margin: bool) {
    // What is brought into view is not left inside a fold.
    let line = r.buffer.line_of(offset);
    r.folds.reveal(line);
    if r.wrapped() {
        wrap::reveal(r, offset, center, margin);
        return;
    }
    let v = reveal_top(
        top_rows(r),
        r.row_of_line(line),
        r.view_lines,
        shown_rows(r),
        center,
        margin,
    );
    set_top_rows(r, v);
    let x = x_of(r, offset);
    r.left = reveal_left(r.left, x, r.view_w, r.advance, center, margin);
}

/// A shaped line number and its width.
type Number = Rc<(Layout<Brush>, f32)>;

pub struct View {
    fonts: FontContext,
    lcx: LayoutContext<Brush>,
    engine: TextEngine,
    pub metrics: Option<Metrics>,
    pub zoom: u32,
    cache: Lru<LineKey, Rc<Shaped>>,
    numbers: Lru<(usize, Rgb), Number>,
    scratch: String,
}

impl View {
    pub fn new() -> View {
        let mut engine = TextEngine::new(4 << 20);
        engine.set_subpixel(crate::platform::subpixel_order());
        // Off Windows `shared_fonts` already has the shared fallbacks; on Windows the editor adds
        // them to its own copy so the chrome's text is unchanged.
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut fonts = shared_fonts();
        #[cfg(windows)]
        mdshared_text::add_fallbacks(&mut fonts);
        View {
            fonts,
            lcx: LayoutContext::new(),
            engine,
            metrics: None,
            zoom: 100,
            cache: Lru::new(400),
            numbers: Lru::new(400),
            scratch: String::new(),
        }
    }

    /// Drops every layout (theme, scale or zoom changed).
    pub fn clear(&mut self) {
        self.cache.clear();
        self.numbers.clear();
    }

    fn build(
        &mut self,
        text: &str,
        size: f32,
        fg: Rgb,
        brushes: &[(Range<usize>, Rgb)],
    ) -> Layout<Brush> {
        let mut b = self.lcx.ranged_builder(&mut self.fonts, text, 1.0, true);
        b.push_default(StyleProperty::FontFamily(FontFamily::from(MONO)));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(StyleProperty::Brush([fg[0], fg[1], fg[2], 255]));
        for (r, c) in brushes {
            if r.start < r.end {
                b.push(StyleProperty::Brush([c[0], c[1], c[2], 255]), r.clone());
            }
        }
        let mut layout = b.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());
        layout
    }

    /// The metrics at `scale`, measured once per scale and zoom.
    pub fn metrics(&mut self, scale: f32) -> Metrics {
        if let Some(m) = self
            .metrics
            .filter(|m| m.scale == scale && m.zoom == self.zoom)
        {
            return m;
        }
        let size = font_px(self.zoom) * scale;
        let sample = self.build("0000000000", size, [0, 0, 0], &[]);
        let advance = sample.width() / 10.0;
        let wide_sample = self.build(WIDE_SAMPLE, size, [0, 0, 0], &[]);
        let wide = (wide_sample.width() / 10.0).max(advance);
        let line_h = line_height(size);
        let first = sample
            .lines()
            .next()
            .map(|l| l.metrics().baseline)
            .unwrap_or(size);
        let baseline = (((line_h as f32 - sample.height()) / 2.0) + first).round();
        let m = Metrics {
            scale,
            zoom: self.zoom,
            size,
            advance,
            wide,
            line_h,
            baseline,
        };
        self.metrics = Some(m);
        self.clear();
        m
    }

    /// Lays out the region without drawing: clamps the scroll and records the view size in the
    /// document, so the colours for the visible lines can be asked for before painting.
    pub fn layout(&mut self, doc: &mut Doc, region: Rect, scale: f32) -> Option<Geom> {
        let m = self.metrics(scale);
        let area = text_area(doc, region);
        let r = doc.ready_mut()?;
        Some(geom(r, area, &m))
    }

    /// The layout of one line or chunk. Offsets in it are bytes from the chunk's start.
    fn shaped(&mut self, r: &mut Ready, line: usize, ch: Chunk, dark: bool) -> Rc<Shaped> {
        let range = r.buffer.line_range(line);
        let (c, end) = (ch.start, ch.end);
        let spans = r.colors.line(line, dark).filter(|s| !s.is_empty());
        let mut source = std::mem::take(&mut self.scratch);
        source.clear();
        r.buffer.text(c.offset..end, &mut source);
        let in_line = c.offset - range.start;
        let key = LineKey {
            text: text_hash(&source),
            len: source.len(),
            col: c.col,
            in_line,
            colors: spans_hash(spans),
            dark,
        };
        if let Some(s) = self.cache.get(&key) {
            self.scratch = source;
            return s.clone();
        }
        let size = self.metrics.map_or(14.0, |m| m.size);
        let mut dl = DisplayLine::new();
        dl.build(&source, 0, c.col, TAB_WIDTH);
        let mut brushes = Vec::new();
        for s in spans.unwrap_or(&[]) {
            // Spans are bytes in the line; stale ones (the line was just edited) may run past
            // its end or sit inside a character, so they are cut to the text and to boundaries.
            let cut = |x: u32| {
                let mut x = (x as usize).saturating_sub(in_line).min(source.len());
                while !source.is_char_boundary(x) {
                    x -= 1;
                }
                x
            };
            let (a, b) = (cut(s.start), cut(s.end));
            if (s.end as usize) > in_line && a < b {
                brushes.push((dl.to_display(a)..dl.to_display(b), s.color));
            }
        }
        self.scratch = source;
        let layout = self.build(&dl.text, size, foreground(dark), &brushes);
        let width = layout.width();
        let baseline = layout.lines().next().map_or(0.0, |l| l.metrics().baseline);
        let shaped = Rc::new(Shaped {
            layout,
            dl,
            width,
            baseline,
        });
        self.cache.insert(key, shaped.clone());
        shaped
    }

    /// The mark of a line ending (`EOL_MARKS`), kept with the line numbers under keys no line
    /// number has.
    fn eol_mark(&mut self, kind: usize, color: Rgb) -> Number {
        let key = (usize::MAX - kind, color);
        if let Some(l) = self.numbers.get(&key) {
            return l.clone();
        }
        let size = self.metrics.map_or(14.0, |m| m.size);
        let layout = self.build(EOL_MARKS[kind], size, color, &[]);
        let baseline = layout.lines().next().map_or(0.0, |l| l.metrics().baseline);
        let v = Rc::new((layout, baseline));
        self.numbers.insert(key, v.clone());
        v
    }

    fn number(&mut self, n: usize, color: Rgb) -> Number {
        if let Some(l) = self.numbers.get(&(n, color)) {
            return l.clone();
        }
        let size = self.metrics.map_or(14.0, |m| m.size);
        let layout = self.build(&n.to_string(), size, color, &[]);
        let baseline = layout.lines().next().map_or(0.0, |l| l.metrics().baseline);
        let v = Rc::new((layout, baseline));
        self.numbers.insert((n, color), v.clone());
        v
    }

    /// Draws the text region. `bands`, when given, limits drawing to those lines (a caret blink
    /// or move); the caller has clipped the painter to them. Returns the time spent.
    pub fn draw(
        &mut self,
        c: &mut Ctx,
        doc: &mut Doc,
        region: Rect,
        s: &DrawState,
        bands: Option<&[usize]>,
    ) -> Duration {
        let started = Instant::now();
        let pal = *c.pal;
        let scale = c.p.scale;
        let m = self.metrics(scale);
        let area = text_area(doc, region);
        if bands.is_none() {
            c.p.fill(region, pal.page);
            if let Some(text) = doc.banner() {
                draw_banner(c, Rect::new(region.x, region.y, region.w, BANNER_H), text);
            }
        }
        let name = doc.name();
        let (done, total) = doc.progress();
        let r = match &mut doc.state {
            DocState::Ready(r) => r,
            DocState::Pending | DocState::Loading => {
                let pct = (done * 100).checked_div(total).unwrap_or(0);
                let text = format!("Loading {name}… {pct}%");
                c.p.text_center(Face::Ui, 13.5, area, &text, pal.muted);
                return started.elapsed();
            }
            DocState::Failed(e) => {
                let text = format!("{name}: {e}");
                c.p.text_center(Face::Ui, 13.5, area, &text, pal.muted);
                return started.elapsed();
            }
        };
        let g = geom(r, area, &m);
        if bands.is_none() && !s.other {
            c.hit(crate::ui::Hit::Text, area);
            if s.band {
                c.hit(
                    crate::ui::Hit::Gutter,
                    Rect::new(area.x, area.y, g.gutter_w, area.h),
                );
            }
        }
        // The pointer is in the gutter of the pane that has the keyboard: chevrons show.
        let gutter_hot = s.band
            && matches!(
                c.hover,
                Some(crate::ui::Hit::Gutter | crate::ui::Hit::Fold(_))
            );
        let (x0, y0, x1, y1) = c.p.dev_rect(area);
        let gutter_r = ((area.x + g.gutter_w) * scale).round() as i32;
        let base = current_clip(c.p);
        let text_clip = intersect((gutter_r, y0, x1, y1), base);
        let origin = g.origin_x(scale, r.left);
        let adv = m.advance as f64;
        let lh = m.line_h;
        let band_c = mix(pal.page, pal.text, 0.035);
        // The lines the replace strip limits Replace to: a faint tint while it is open.
        let scope_c = mix(pal.page, pal.accent, 0.07);
        let scope_lines = r.scope().map(|s| {
            let last =
                if s.end > s.start && r.buffer.line_range(r.buffer.line_of(s.end)).start == s.end {
                    s.end - 1
                } else {
                    s.end
                };
            r.buffer.line_of(s.start)..=r.buffer.line_of(last)
        });
        let sel = r.sel.range();
        let sel_is_current = r
            .find
            .current
            .and_then(|k| r.find.matches.get(k))
            .is_some_and(|m| *m == sel);
        let caret_line = r.buffer.line_of(r.sel.caret);
        let band_line = s.band.then_some(caret_line);
        // The other selections of several carets, in document order.
        let len = r.buffer.len_bytes();
        let extras: Vec<(Range<usize>, usize)> = r
            .extra
            .iter()
            .map(|s| {
                let range = s.range();
                (range.start.min(len)..range.end.min(len), s.caret.min(len))
            })
            .collect();
        let alone = extras.is_empty();
        // The bracket pair at the caret, and the selected word whose other occurrences show.
        let brackets = r.brackets().map(|(a, b)| [a, b]);
        let twin = (!sel_is_current).then(|| r.twin_word()).flatten();
        let mut line_text = String::new();
        let bracket_c = mix(pal.page, pal.text, 0.22);
        let ws_c = mix(pal.page, pal.text, 0.32);
        let mut ws_text = String::new();
        let dot = ((scale * 1.6).round() as i32).max(1);
        let hair = (scale.round() as i32).max(1);
        // The horizontal view, device px from column 0.
        let xs = {
            let s = scale as f64;
            (r.left * s).max(0.0)..(r.left + g.view_w as f64) * s + adv
        };
        let mark_a = if pal.dark { 0.38 } else { 0.45 };
        let fold_c = mix(pal.page, pal.text, 0.5);
        let wanted = |i: usize| bands.is_none_or(|b| b.contains(&i));
        let mut shown_lines: Vec<ShownLine> = Vec::new();
        // Compare mode: the diff, its colours, and the filler rows in view.
        let align = r.align.clone();
        let tints = align
            .as_ref()
            .map(|_| crate::compare::tints(pal.dark, pal.page, pal.text));
        let tint_of = |row: usize| {
            let (al, t) = (align.as_ref()?, tints.as_ref()?);
            t.of(al.kind(row)?, al.right)
        };
        let mut fillers: Vec<i32> = Vec::new();
        let row_tint = if pal.dark { ROW_TINT_DARK } else { ROW_TINT };
        if g.wrapped {
            let mut p = wrap::top(r);
            let mut y = y0;
            while y < y1 && p.line < g.lines {
                let rows = wrap::rows(r, p.line);
                let end = r.buffer.line_range(p.line).end;
                let line_y = y;
                let mut parts = Vec::new();
                for k in p.row.min(rows.len() - 1)..rows.len() {
                    if y >= y1 {
                        break;
                    }
                    let chunk = Chunk {
                        start: rows[k],
                        end: rows.get(k + 1).map_or(end, |n| n.offset),
                    };
                    parts.push((chunk, 0.0, y));
                    y += lh;
                }
                shown_lines.push(ShownLine {
                    line: p.line,
                    row: p.line,
                    y: line_y,
                    h: y - line_y,
                    first: p.row == 0,
                    parts,
                });
                p = wrap::Pos {
                    line: r.folds.next(p.line),
                    row: 0,
                };
            }
        } else {
            for row in visible(g.top, g.view_lines, g.rows) {
                let y = y0 + line_y(row, g.top, lh).round() as i32;
                let Some(i) = r.line_on_row(row) else {
                    // A filler row: nothing on this side, opposite lines on the other.
                    if bands.is_none() {
                        fillers.push(y);
                    }
                    continue;
                };
                if !wanted(i) {
                    continue;
                }
                let parts = chunks_in_view(r, i, xs.clone(), &m)
                    .into_iter()
                    .map(|ch| (ch, m.chunk_x(&ch.start), y))
                    .collect();
                shown_lines.push(ShownLine {
                    line: i,
                    row,
                    y,
                    h: lh,
                    first: true,
                    parts,
                });
            }
        }
        if let Some(t) = &tints {
            for &y in &fillers {
                c.p.fill_dev(x0, y, x1, y + lh, t.filler, 1.0);
            }
        }
        for shown_line in &shown_lines {
            let (i, y, h) = (shown_line.line, shown_line.y, shown_line.h);
            if sel.is_empty() && alone && Some(i) == band_line {
                c.p.fill_dev(x0, y, x1, y + h, band_c, 1.0);
            } else if scope_lines.as_ref().is_some_and(|s| s.contains(&i)) {
                c.p.fill_dev(x0, y, x1, y + h, scope_c, 1.0);
            } else if bands.is_some() {
                c.p.fill_dev(x0, y, x1, y + h, pal.page, 1.0);
            }
            let tint = tint_of(shown_line.row);
            if let Some(col) = tint {
                c.p.fill_dev(x0, y, x1, y + h, col, row_tint);
            }
            let intra = tint.and_then(|col| {
                align
                    .as_ref()?
                    .intra(shown_line.row)
                    .filter(|m| !m.is_empty())
                    .map(|m| (m, col))
            });
            let range = r.buffer.line_range(i);
            let with_break = r.buffer.line_range_with_break(i);
            let folded = r.folds.headed(i).is_some();
            // Find marks overlapping the line.
            let marks: &[Range<usize>] = {
                let all = &r.find.matches;
                let a = all.partition_point(|m| m.end <= range.start);
                let b = all.partition_point(|m| m.start < with_break.end.max(range.start + 1));
                &all[a..b.max(a)]
            };
            let first_mark = r.find.matches.partition_point(|m| m.end <= range.start);
            let marks: Vec<(Range<usize>, bool)> = marks
                .iter()
                .enumerate()
                .map(|(k, m)| (m.clone(), r.find.current == Some(first_mark + k)))
                .collect();
            // Other occurrences of the selected word on this line, as whole words.
            let mut twins: Vec<Range<usize>> = Vec::new();
            if let Some(word) = twin.as_deref().filter(|_| range.len() <= TWIN_LINE_MAX) {
                line_text.clear();
                r.buffer.text(range.clone(), &mut line_text);
                let word_char = |c: char| c.is_alphanumeric() || c == '_';
                for (k, _) in line_text.match_indices(word) {
                    let end = k + word.len();
                    let whole = !line_text[..k].chars().next_back().is_some_and(word_char)
                        && !line_text[end..].chars().next().is_some_and(word_char);
                    if whole && range.start + k != sel.start {
                        twins.push(range.start + k..range.start + end);
                    }
                }
            }
            for &(chunk, cx, y) in &shown_line.parts {
                let shaped = self.shaped(r, i, chunk, s.dark);
                let (ch, end) = (chunk.start, chunk.end);
                let x = origin + cx;
                let last = end == range.end;
                if !g.wrapped {
                    r.widest = r
                        .widest
                        .max(((cx + shaped.width as f64) / scale as f64) as f32);
                }
                let span = |a: usize, b: usize| -> Option<(i32, i32)> {
                    let (a, b) = (a.max(ch.offset), b.min(end));
                    if a > b || (a == b && a != end) {
                        return None;
                    }
                    let xa = x + shaped.x_of(a - ch.offset) as f64;
                    let xb = x + shaped.x_of(b - ch.offset) as f64;
                    Some((xa.round() as i32, xb.round() as i32))
                };
                for (mr, current) in &marks {
                    if let Some((a, b)) = span(mr.start, mr.end) {
                        let (col, alpha) = if *current {
                            (pal.find_current, 0.6)
                        } else {
                            (pal.find_mark, mark_a)
                        };
                        let b = if b <= a {
                            a + (adv / 2.0).round() as i32
                        } else {
                            b
                        };
                        fill_clipped(c, (a, y, b, y + lh), text_clip, col, alpha);
                    }
                }
                if let Some((m, col)) = &intra
                    && let Some((a, b)) = span(range.start + m.start, range.start + m.end)
                {
                    fill_clipped(c, (a, y, b, y + lh), text_clip, *col, INTRA_TINT);
                }
                for t in &twins {
                    if let Some((a, b)) = span(t.start, t.end) {
                        fill_clipped(c, (a, y, b, y + lh), text_clip, pal.selection, 0.16);
                    }
                }
                for &o in brackets.iter().flatten() {
                    if range.contains(&o)
                        && o >= ch.offset
                        && o < end
                        && let Some((a, b)) = span(o, o + 1)
                    {
                        fill_clipped(c, (a, y, b, y + lh), text_clip, bracket_c, 1.0);
                    }
                }
                // A selection that is the current match shows as the match alone.
                if !sel.is_empty()
                    && !sel_is_current
                    && sel.start <= end
                    && sel.end >= ch.offset
                    && let Some((a, mut b)) = span(sel.start, sel.end)
                {
                    // A selected line break shows as half an advance past the line's end.
                    if last
                        && sel.end > range.end
                        && sel.start <= range.end
                        && with_break.end > range.end
                    {
                        b += (adv / 2.0).round() as i32;
                    }
                    fill_clipped(c, (a, y, b, y + lh), text_clip, pal.selection, 0.35);
                }
                // The other selections that reach into this part.
                let from = extras.partition_point(|e| e.0.end < ch.offset);
                for (er, _) in extras[from..].iter().take_while(|e| e.0.start <= end) {
                    if !er.is_empty()
                        && let Some((a, b)) = span(er.start, er.end)
                    {
                        fill_clipped(c, (a, y, b, y + lh), text_clip, pal.selection, 0.35);
                    }
                }
                let gy = y as f32 + m.baseline - shaped.baseline;
                self.engine.draw_layout_clipped(
                    c.p.bmp,
                    &shaped.layout,
                    x.round() as f32,
                    gy.round(),
                    text_clip,
                );
                // The other carets in this part: one at a part's end shows there only when the
                // line ends with it.
                if s.caret {
                    let w = (1.5 * scale).round().max(1.0) as i32;
                    for (_, at) in extras[from..].iter().take_while(|e| e.0.start <= end) {
                        if *at >= ch.offset && (*at < end || (*at == end && last)) {
                            let cx = (x + shaped.x_of(*at - ch.offset) as f64).round() as i32;
                            fill_clipped(c, (cx, y, cx + w, y + lh), text_clip, pal.text, 1.0);
                        }
                    }
                }
                // A folded block: a box with three dots after its first line.
                if last && folded {
                    let gap = if s.whitespace { 2.2 } else { 1.0 };
                    let a = (x + shaped.width as f64 + adv * gap).round() as i32;
                    let (w, h) = (
                        (adv * 2.6).round() as i32,
                        (lh as f32 * 0.56).round() as i32,
                    );
                    let top = y + (lh - h) / 2;
                    let hit = crate::ui::Hit::FoldBox(i);
                    let box_c = mix(
                        pal.page,
                        pal.text,
                        if c.hover == Some(hit) { 0.18 } else { 0.10 },
                    );
                    fill_clipped(c, (a, top, a + w, top + h), text_clip, box_c, 1.0);
                    if bands.is_none() && s.band {
                        let sc = scale;
                        c.hit(
                            hit,
                            Rect::new(a as f32 / sc, top as f32 / sc, w as f32 / sc, h as f32 / sc),
                        );
                    }
                    for k in 1..4 {
                        let (dx, dy) = (a + w * k / 4 - dot / 2, top + (h - dot) / 2);
                        fill_clipped(c, (dx, dy, dx + dot, dy + dot), text_clip, fold_c, 1.0);
                    }
                }
                if !s.whitespace {
                    continue;
                }
                // A dot in each space and a dash with a tick in each tab. A run of spaces is
                // measured once: its spaces are equally wide.
                ws_text.clear();
                r.buffer.text(ch.offset..end, &mut ws_text);
                let bytes = ws_text.as_bytes();
                let mid = y + lh / 2;
                let mut k = 0;
                while k < bytes.len() {
                    let byte = bytes[k];
                    if byte != b' ' && byte != b'\t' {
                        k += 1;
                        continue;
                    }
                    let mut e = k + 1;
                    while byte == b' ' && e < bytes.len() && bytes[e] == b' ' {
                        e += 1;
                    }
                    if let Some((xa, xb)) = span(ch.offset + k, ch.offset + e) {
                        let n = (e - k) as i32;
                        for j in 0..n {
                            let (a, b) = (xa + (xb - xa) * j / n, xa + (xb - xa) * (j + 1) / n);
                            if byte == b' ' {
                                let (dx, dy) = ((a + b - dot) / 2, mid - dot / 2);
                                let at = (dx, dy, dx + dot, dy + dot);
                                fill_clipped(c, at, text_clip, ws_c, 1.0);
                            } else {
                                let pad = ((adv * 0.2) as i32).max(1);
                                let (a, b) = (a + pad, (b - pad).max(a + pad + hair));
                                fill_clipped(c, (a, mid, b, mid + hair), text_clip, ws_c, 1.0);
                                let tick = (b - hair, mid - dot, b, mid + hair + dot);
                                fill_clipped(c, tick, text_clip, ws_c, 1.0);
                            }
                        }
                    }
                    k = e;
                }
                // The line ending, after the last of the line's text.
                if last && with_break.end > range.end {
                    let first = r.buffer.rope().byte(range.end);
                    let kind = match (with_break.end - range.end, first) {
                        (1, b'\n') => Some(0),
                        (2, b'\r') => Some(1),
                        (1, b'\r') => Some(2),
                        _ => None,
                    };
                    if let Some(kind) = kind {
                        let mark = self.eol_mark(kind, ws_c);
                        let mx = x + shaped.width as f64 + adv * 0.3;
                        let my = y as f32 + m.baseline - mark.1;
                        self.engine.draw_layout_clipped(
                            c.p.bmp,
                            &mark.0,
                            mx.round() as f32,
                            my.round(),
                            text_clip,
                        );
                    }
                }
            }
        }
        // The caret.
        let caret = r.sel.caret;
        let caret_part = shown_lines
            .iter()
            .find(|l| l.line == caret_line)
            .and_then(|l| {
                if g.wrapped {
                    // The row that holds the caret, when it is among those drawn.
                    let part = l.parts.iter().rev().find(|p| p.0.start.offset <= caret);
                    part.filter(|p| caret <= p.0.end).copied()
                } else {
                    let chunk = chunk_at_offset(r, caret_line, caret);
                    Some((chunk, m.chunk_x(&chunk.start), l.y))
                }
            });
        if let Some((chunk, cx, y)) = caret_part.filter(|_| s.caret) {
            let shaped = self.shaped(r, caret_line, chunk, s.dark);
            let in_chunk = caret.saturating_sub(chunk.start.offset);
            let x = origin + cx + shaped.x_of(in_chunk) as f64;
            let w = (1.5 * scale).round().max(1.0) as i32;
            let x = x.round() as i32;
            fill_clipped(c, (x, y, x + w, y + lh), text_clip, pal.text, 1.0);
        }
        // The gutter: same background, numbers right-aligned.
        let gutter = intersect((x0, y0, gutter_r, y1), base);
        let pad_r = (GUTTER_PAD_R * scale).round() as i32;
        for shown_line in &shown_lines {
            let (i, y) = (shown_line.line, shown_line.y);
            let band = if sel.is_empty() && alone && Some(i) == band_line {
                band_c
            } else {
                pal.page
            };
            fill_clipped(c, (x0, y, gutter_r, y + shown_line.h), gutter, band, 1.0);
            if let Some(col) = tint_of(shown_line.row) {
                fill_clipped(
                    c,
                    (x0, y, gutter_r, y + shown_line.h),
                    gutter,
                    col,
                    row_tint,
                );
            }
            if !shown_line.first {
                continue;
            }
            // A bookmark: a dot left of the number.
            if r.bookmarks.binary_search(&i).is_ok() {
                let d = (7.0 * scale).round() as i32;
                let (bx, by) = (x0 + (5.0 * scale).round() as i32, y + (lh - d) / 2);
                let dot = Rect::new(
                    bx as f32 / scale,
                    by as f32 / scale,
                    d as f32 / scale,
                    d as f32 / scale,
                );
                c.p.rbox(dot, d as f32 / scale / 2.0, pal.accent);
            }
            // A line that starts a block, or is folded: a chevron right of the number, shown
            // faintly while the pointer is in the gutter and always for a folded one. Only the
            // visible lines are asked.
            let folded = r.folds.headed(i).is_some();
            if folded
                || (gutter_hot && !r.folds.suspended() && heads_block(&r.buffer, i, TAB_WIDTH))
            {
                let cell =
                    metrics::fold_cell(area.x, g.gutter_w, y as f32 / scale, lh as f32 / scale);
                let hit = crate::ui::Hit::Fold(i);
                let hovered = c.hover == Some(hit);
                let (cx, cy) = metrics::fold_icon_centre(&cell);
                if hovered && let Some(f) = c.fill_for(hit, false, false) {
                    let b = Rect::new(cx - 7.0, cell.y + 1.0, 14.0, cell.h - 2.0);
                    c.p.rbox(b, 4.0, f);
                }
                let color = if hovered {
                    pal.text
                } else if folded {
                    pal.muted
                } else {
                    mix(pal.page, pal.faint, 0.8)
                };
                let g_name = if folded { glyph::RIGHT } else { glyph::DOWN };
                c.p.icon(g_name, metrics::FOLD_ICON, cx, cy, color);
                if bands.is_none() {
                    c.hit(hit, cell);
                }
            }
            let color = if Some(i) == band_line {
                pal.text
            } else {
                pal.faint
            };
            let n = self.number(i + 1, color);
            let x = gutter_r - pad_r - n.0.width().round() as i32;
            let gy = y as f32 + m.baseline - n.1;
            self.engine
                .draw_layout_clipped(c.p.bmp, &n.0, x as f32, gy.round(), gutter);
        }
        let (vscroller, hscroller) = if s.other {
            (
                crate::ui::Scroller::OtherText,
                crate::ui::Scroller::OtherTextH,
            )
        } else {
            (crate::ui::Scroller::Text, crate::ui::Scroller::TextH)
        };
        // Scrollbars sit over the text; a band repaint draws them again inside its clip.
        if let Some((track, thumb)) = g.vbar() {
            scroll::draw_scrollbar(c.p, &pal, track, thumb, s.vbar_active);
            if bands.is_none() {
                c.hit(crate::ui::Hit::ScrollBar(vscroller), track);
            }
        }
        if let Some((track, thumb)) = g.hbar(r.left) {
            draw_hscrollbar(c, track, thumb, s.hbar_active);
            if bands.is_none() {
                c.hit(crate::ui::Hit::ScrollBar(hscroller), track);
            }
        }
        let rows: usize = shown_lines.iter().map(|l| l.parts.len().max(1)).sum();
        let cap = (rows.max(16) * CACHE_SCREENS).max(64);
        if self.cache.cap() < cap {
            self.cache.set_cap(cap);
        }
        started.elapsed()
    }

    /// The buffer offset under logical `(x, y)` in the text region: y gives the line, x the
    /// chunk and then the position in its layout, snapped to a grapheme boundary.
    pub fn hit(
        &mut self,
        doc: &mut Doc,
        region: Rect,
        scale: f32,
        x: f32,
        y: f32,
    ) -> Option<usize> {
        let m = self.metrics(scale);
        let area = text_area(doc, region);
        let r = doc.ready_mut()?;
        let g = geom(r, area, &m);
        let dy = ((y - area.y) * scale) as f64;
        let rel = (x * scale) as f64 - g.origin_x(scale, r.left);
        let (line, chunk, cx) = if g.wrapped {
            // The row under y, counted from the one at the top.
            let n = (dy.max(0.0) / m.line_h.max(1) as f64).floor() as isize;
            let t = wrap::top(r);
            let p = wrap::step(r, t, n);
            let (start, end) = wrap::row(r, p);
            (p.line, Chunk { start, end }, 0.0)
        } else {
            let line = r.line_at_row(line_at(dy, g.top, m.line_h, g.rows));
            let chunk = chunk_at_x(r, line, rel.max(0.0), m.advance, m.wide);
            (line, chunk, m.chunk_x(&chunk.start))
        };
        let dark = r.colors.dark;
        let shaped = self.shaped(r, line, chunk, dark);
        let local = (rel - cx) as f32;
        let range = r.buffer.line_range(line);
        let offset = if local <= 0.0 {
            chunk.start.offset
        } else if local >= shaped.width {
            chunk.end
        } else {
            let cur = Cursor::from_point(&shaped.layout, local, shaped.layout.height() / 2.0);
            chunk.start.offset + shaped.dl.to_source(cur.index(), false)
        };
        let mut offset = r
            .buffer
            .snap_to_grapheme(offset.clamp(range.start, range.end));
        // The end of a row that is not its line's last shows on the next row: a click past it
        // stays on this one.
        if g.wrapped && offset == chunk.end && offset < range.end && offset > chunk.start.offset {
            offset = r.buffer.prev_grapheme(offset);
        }
        Some(offset.min(range.end))
    }
}

/// The region below the banner (file gone, or changed on disk under unsaved changes).
pub fn text_area(doc: &Doc, region: Rect) -> Rect {
    if doc.banner().is_some() {
        Rect::new(
            region.x,
            region.y + BANNER_H,
            region.w,
            (region.h - BANNER_H).max(0.0),
        )
    } else {
        region
    }
}

/// Computes the geometry, clamps the scroll and records the view size in `r`.
fn geom(r: &mut Ready, area: Rect, m: &Metrics) -> Geom {
    let scale = m.scale;
    let lines = r.buffer.len_lines().max(1);
    let adv_l = m.advance_l();
    let gutter_w = gutter_width(lines, adv_l, GUTTER_PAD_L + GUTTER_PAD_R);
    let (y0, y1) = ((area.y * scale).round(), (area.bottom() * scale).round());
    let vl = view_lines(y1 - y0, m.line_h);
    let view_w = (area.w - gutter_w - TEXT_PAD).max(0.0);
    r.unfold_carets();
    let rows = shown_rows(r);
    let wrapped = r.wrapped();
    let extent = if wrapped {
        0.0
    } else {
        extent(r.info.longest_line_chars, adv_l, r.widest)
    };
    r.view_lines = vl;
    r.view_w = view_w;
    r.advance = adv_l;
    r.wide_advance = m.wide / scale;
    if wrapped {
        let p = wrap::top(r);
        wrap::set_top(r, p);
        r.left = 0.0;
    } else {
        let v = clamp_top(top_rows(r), rows, vl);
        set_top_rows(r, v);
        r.left = clamp_left(r.left, extent, view_w);
    }
    let top = if wrapped { r.top } else { top_rows(r) };
    Geom {
        area,
        gutter_w,
        lines,
        view_lines: vl,
        view_w,
        extent,
        wrapped,
        rows,
        top,
    }
}

/// The painter's clip, read by setting a clip that changes nothing.
fn current_clip(p: &mut mdshared_ui::gfx::Painter) -> (i32, i32, i32, i32) {
    let old = p.clip(Rect::new(-1e7, -1e7, 2e7, 2e7));
    p.restore(old);
    old
}

fn intersect(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
    (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3))
}

fn fill_clipped(
    c: &mut Ctx,
    r: (i32, i32, i32, i32),
    clip: (i32, i32, i32, i32),
    col: Rgb,
    a: f32,
) {
    let (x0, y0, x1, y1) = (
        r.0.max(clip.0),
        r.1.max(clip.1),
        r.2.min(clip.2),
        r.3.min(clip.3),
    );
    if x0 < x1 && y0 < y1 {
        c.p.fill_dev(x0, y0, x1, y1, col, a);
    }
}

fn draw_banner(c: &mut Ctx, banner: Rect, text: &str) {
    let pal = *c.pal;
    c.p.fill(banner, pal.warn_bg);
    let px = c.p.px();
    c.p.fill(
        Rect::new(banner.x, banner.bottom() - px, banner.w, px),
        mix(pal.warn_bg, pal.warn, 0.3),
    );
    c.p.icon(glyph::WARN, 13.0, banner.x + 22.0, banner.cy(), pal.warn);
    c.p.text(
        Face::Ui,
        12.5,
        banner.x + 38.0,
        banner.cy(),
        text,
        pal.text,
        banner.w - 54.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horizontal_scrollbar_geometry() {
        let area = Rect::new(0.0, 0.0, 404.0, 300.0);
        assert!(hscrollbar(area, 300.0, 400.0, 0.0).is_none());
        let (track, thumb) = hscrollbar(area, 800.0, 400.0, 0.0).unwrap();
        assert_eq!(track.w, 400.0);
        assert_eq!(track.bottom(), 300.0);
        assert_eq!(thumb.w, 200.0);
        assert_eq!(thumb.x, track.x);
        let (_, end) = hscrollbar(area, 800.0, 400.0, 400.0).unwrap();
        assert_eq!(end.right(), track.right());
        let back = offset_for_hthumb(track, end.w, end.x, 800.0, 400.0);
        assert!((back - 400.0).abs() < 0.01);
        let (_, tiny) = hscrollbar(area, 1e7, 400.0, 0.0).unwrap();
        assert_eq!(tiny.w, 28.0);
    }

    #[test]
    fn spans_hash_tells_colours_apart() {
        let a = [Span {
            start: 0,
            end: 3,
            color: [1, 2, 3],
        }];
        let b = [Span {
            start: 0,
            end: 3,
            color: [1, 2, 4],
        }];
        assert_eq!(spans_hash(None), 0);
        assert_eq!(spans_hash(Some(&[])), 0);
        assert_ne!(spans_hash(Some(&a)), 0);
        assert_ne!(spans_hash(Some(&a)), spans_hash(Some(&b)));
    }
}
