//! The text view: font metrics, gutter, line layout (whole lines and chunks), painting of text,
//! selection, find marks and caret, hit testing, both scrollbars. DESIGN.md section 10.
//!
//! Lines are shaped with the view's own parley context in the monospace family and drawn with its
//! own `TextEngine` straight into the window's composed bitmap. A line of at most
//! `WHOLE_LINE_BYTES` is one layout; a longer one is cut by `display::chunk_starts` and only the
//! chunks on screen, plus one each side, are shaped, each placed at its start column times the
//! advance. Layouts are cached by (document, line, chunk, revision, colours, theme).

mod cache;
pub mod metrics;

use crate::chrome::Ctx;
use crate::commands::strings;
use crate::docs::{Doc, DocState, Ready};
use cache::Lru;
use mdedit_core::display::{self, ChunkStart, DisplayLine};
use mdedit_core::limits::{TAB_WIDTH, WHOLE_LINE_BYTES};
use mdedit_highlight::{DocId, Span, foreground};
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

pub const MONO: &str = "Consolas, Cascadia Mono, Noto Sans Mono, DejaVu Sans Mono, monospace";
/// The missing-file banner over the text.
pub const BANNER_H: f32 = 34.0;
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
    pub line_h: i32,
    /// Where the baseline sits below a line's top.
    pub baseline: f32,
}

impl Metrics {
    /// The advance in logical px.
    pub fn advance_l(&self) -> f32 {
        self.advance / self.scale
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
    /// The x of a buffer offset inside this text, device px from its start.
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
    doc: DocId,
    line: usize,
    /// The chunk's start offset.
    chunk: usize,
    revision: u64,
    /// A hash of the line's colour spans; 0 when it has none.
    colors: u64,
    dark: bool,
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
}

impl Geom {
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
    pub fn vbar(&self, top: f64) -> Option<(Rect, Rect)> {
        scroll::scrollbar(
            self.area,
            self.lines as f32,
            self.view_lines as f32,
            top as f32,
        )
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
    pub vbar_active: bool,
    pub hbar_active: bool,
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

/// The chunks of `line`: one for a line of at most `WHOLE_LINE_BYTES`, else cached chunk starts.
fn chunks_of(r: &mut Ready, line: usize) -> Rc<Vec<ChunkStart>> {
    let range = r.buffer.line_range(line);
    if range.len() <= WHOLE_LINE_BYTES {
        return Rc::new(vec![ChunkStart {
            offset: range.start,
            col: 0,
        }]);
    }
    r.chunks
        .entry(line)
        .or_insert_with(|| Rc::new(display::chunk_starts(&r.buffer, line, TAB_WIDTH)))
        .clone()
}

/// The display column of `offset` (on its line).
pub fn column_at(r: &mut Ready, offset: usize) -> usize {
    let line = r.buffer.line_of(offset);
    let chunks = chunks_of(r, line);
    let c = chunks[chunk_at_offset(&chunks, offset)];
    let mut col = c.col;
    let mut text = String::new();
    r.buffer.text(c.offset..offset.max(c.offset), &mut text);
    col += display::columns(&text, c.col, TAB_WIDTH);
    col
}

/// Keeps `offset` in view (with the margin), or centres it when `center` and it is off screen.
/// Uses the view size of the last frame.
pub fn reveal(r: &mut Ready, offset: usize, center: bool) {
    let line = r.buffer.line_of(offset);
    r.top = reveal_top(r.top, line, r.view_lines, r.buffer.len_lines(), center);
    let x = column_at(r, offset) as f64 * r.advance as f64;
    r.left = reveal_left(r.left, x, r.view_w, r.advance, center);
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
        View {
            fonts: shared_fonts(),
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

    #[allow(clippy::too_many_arguments)]
    fn shaped(
        &mut self,
        doc: DocId,
        revision: u64,
        r: &mut Ready,
        line: usize,
        chunks: &[ChunkStart],
        index: usize,
        dark: bool,
    ) -> Rc<Shaped> {
        let range = r.buffer.line_range(line);
        let c = chunks[index];
        let end = chunks.get(index + 1).map_or(range.end, |n| n.offset);
        let spans = r.colors.line(line, dark).filter(|s| !s.is_empty());
        let key = LineKey {
            doc,
            line,
            chunk: c.offset,
            revision,
            colors: spans_hash(spans),
            dark,
        };
        if let Some(s) = self.cache.get(&key) {
            return s.clone();
        }
        let size = self.metrics.map_or(14.0, |m| m.size);
        let mut source = std::mem::take(&mut self.scratch);
        source.clear();
        r.buffer.text(c.offset..end, &mut source);
        let mut dl = DisplayLine::new();
        dl.build(&source, c.offset, c.col, TAB_WIDTH);
        self.scratch = source;
        let mut brushes = Vec::new();
        for s in spans.unwrap_or(&[]) {
            let (a, b) = (range.start + s.start as usize, range.start + s.end as usize);
            let (a, b) = (a.max(c.offset), b.min(end));
            if a < b {
                brushes.push((dl.to_display(a)..dl.to_display(b), s.color));
            }
        }
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
            if doc.missing {
                draw_banner(c, Rect::new(region.x, region.y, region.w, BANNER_H));
            }
        }
        let (id, revision) = (doc.id, doc.revision);
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
        if bands.is_none() {
            c.hit(crate::ui::Hit::Text, area);
        }
        let (x0, y0, x1, y1) = c.p.dev_rect(area);
        let gutter_r = ((area.x + g.gutter_w) * scale).round() as i32;
        let base = current_clip(c.p);
        let text_clip = intersect((gutter_r, y0, x1, y1), base);
        let origin = g.origin_x(scale, r.left);
        let adv = m.advance as f64;
        let lh = m.line_h;
        let band_c = mix(pal.page, pal.text, 0.035);
        let sel = r.sel.range();
        let sel_is_current = r
            .find
            .current
            .and_then(|k| r.find.matches.get(k))
            .is_some_and(|m| *m == sel);
        let caret_line = r.buffer.line_of(r.sel.caret);
        let vis = visible(r.top, g.view_lines, g.lines);
        let cols = {
            let a = (r.left / r.advance as f64).floor().max(0.0) as usize;
            let b = ((r.left + g.view_w as f64) / r.advance as f64)
                .ceil()
                .max(0.0) as usize;
            a..b + 1
        };
        let mark_a = if pal.dark { 0.38 } else { 0.45 };
        let top = r.top;
        let line_top = |i: usize| y0 + line_y(i, top, lh).round() as i32;
        let wanted = |i: usize| bands.is_none_or(|b| b.contains(&i));
        for i in vis.clone() {
            if !wanted(i) {
                continue;
            }
            let y = line_top(i);
            if sel.is_empty() && i == caret_line {
                c.p.fill_dev(x0, y, x1, y + lh, band_c, 1.0);
            } else if bands.is_some() {
                c.p.fill_dev(x0, y, x1, y + lh, pal.page, 1.0);
            }
            let range = r.buffer.line_range(i);
            let with_break = r.buffer.line_range_with_break(i);
            let chunks = chunks_of(r, i);
            let whole = chunks.len() == 1;
            let shown = if whole {
                0..1
            } else {
                chunks_in_view(&chunks, cols.clone())
            };
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
            for k in shown {
                let shaped = self.shaped(id, revision, r, i, &chunks, k, s.dark);
                let ch = chunks[k];
                let end = chunks.get(k + 1).map_or(range.end, |n| n.offset);
                let x = origin + chunk_x(ch.col, m.advance);
                let last = k + 1 == chunks.len();
                if whole {
                    r.widest = r.widest.max(shaped.width / scale);
                }
                let span = |a: usize, b: usize| -> Option<(i32, i32)> {
                    let (a, b) = (a.max(ch.offset), b.min(end));
                    if a > b || (a == b && a != end) {
                        return None;
                    }
                    let xa = x + shaped.x_of(a) as f64;
                    let xb = x + shaped.x_of(b) as f64;
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
                let gy = y as f32 + m.baseline - shaped.baseline;
                self.engine.draw_layout_clipped(
                    c.p.bmp,
                    &shaped.layout,
                    x.round() as f32,
                    gy.round(),
                    text_clip,
                );
            }
        }
        // The caret.
        if s.caret && vis.contains(&caret_line) && wanted(caret_line) {
            let chunks = chunks_of(r, caret_line);
            let k = chunk_at_offset(&chunks, r.sel.caret);
            let shaped = self.shaped(id, revision, r, caret_line, &chunks, k, s.dark);
            let x = origin + chunk_x(chunks[k].col, m.advance) + shaped.x_of(r.sel.caret) as f64;
            let w = (1.5 * scale).round().max(1.0) as i32;
            let (x, y) = (x.round() as i32, line_top(caret_line));
            fill_clipped(c, (x, y, x + w, y + lh), text_clip, pal.text, 1.0);
        }
        // The gutter: same background, numbers right-aligned.
        let gutter = intersect((x0, y0, gutter_r, y1), base);
        let pad_r = (GUTTER_PAD_R * scale).round() as i32;
        for i in vis.clone() {
            if !wanted(i) {
                continue;
            }
            let y = line_top(i);
            let band = if sel.is_empty() && i == caret_line {
                band_c
            } else {
                pal.page
            };
            fill_clipped(c, (x0, y, gutter_r, y + lh), gutter, band, 1.0);
            let color = if i == caret_line { pal.text } else { pal.faint };
            let n = self.number(i + 1, color);
            let x = gutter_r - pad_r - n.0.width().round() as i32;
            let gy = y as f32 + m.baseline - n.1;
            self.engine
                .draw_layout_clipped(c.p.bmp, &n.0, x as f32, gy.round(), gutter);
        }
        // Scrollbars sit over the text; a band repaint draws them again inside its clip.
        if let Some((track, thumb)) = g.vbar(r.top) {
            scroll::draw_scrollbar(c.p, &pal, track, thumb, s.vbar_active);
            if bands.is_none() {
                c.hit(crate::ui::Hit::ScrollBar(crate::ui::Scroller::Text), track);
            }
        }
        if let Some((track, thumb)) = g.hbar(r.left) {
            draw_hscrollbar(c, track, thumb, s.hbar_active);
            if bands.is_none() {
                c.hit(crate::ui::Hit::ScrollBar(crate::ui::Scroller::TextH), track);
            }
        }
        let cap = (vis.len().max(16) * CACHE_SCREENS).max(64);
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
        let (id, revision) = (doc.id, doc.revision);
        let r = doc.ready_mut()?;
        let g = geom(r, area, &m);
        let dy = ((y - area.y) * scale) as f64;
        let line = line_at(dy, r.top, m.line_h, g.lines);
        let rel = (x * scale) as f64 - g.origin_x(scale, r.left);
        let chunks = chunks_of(r, line);
        let col = (rel / m.advance as f64).max(0.0) as usize;
        let k = if chunks.len() == 1 {
            0
        } else {
            chunk_at_col(&chunks, col)
        };
        let dark = r.colors.dark;
        let shaped = self.shaped(id, revision, r, line, &chunks, k, dark);
        let local = (rel - chunk_x(chunks[k].col, m.advance)) as f32;
        let range = r.buffer.line_range(line);
        let offset = if local <= 0.0 {
            chunks[k].offset
        } else if local >= shaped.width {
            chunks.get(k + 1).map_or(range.end, |n| n.offset)
        } else {
            let cur = Cursor::from_point(&shaped.layout, local, shaped.layout.height() / 2.0);
            shaped.dl.to_source(cur.index(), false)
        };
        let offset = r
            .buffer
            .snap_to_grapheme(offset.clamp(range.start, range.end));
        Some(offset.min(range.end))
    }
}

/// The region below the missing-file banner.
pub fn text_area(doc: &Doc, region: Rect) -> Rect {
    if doc.missing {
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
    let extent = extent(r.info.longest_line_chars, adv_l, r.widest);
    r.top = clamp_top(r.top, lines, vl);
    r.left = clamp_left(r.left, extent, view_w);
    r.view_lines = vl;
    r.view_w = view_w;
    r.advance = adv_l;
    Geom {
        area,
        gutter_w,
        lines,
        view_lines: vl,
        view_w,
        extent,
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

fn draw_banner(c: &mut Ctx, banner: Rect) {
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
        strings::FILE_GONE,
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
