//! The text view's arithmetic, as pure functions of the font metrics: line positions, scroll
//! limits, the gutter's width, the horizontal extent and where chunks of long lines sit. Vertical
//! positions are `f64` lines so a 1.85-million-line file scrolls exactly; device px are integers
//! per line so line tops never drift.

use mdedit_core::display::{ChunkStart, Cols};
use std::ops::Range;

/// The text size at zoom 100, logical px.
pub const FONT_PX: f32 = 14.0;
/// Gutter padding left and right of the numbers, and the gap before the text, logical px.
pub const GUTTER_PAD_L: f32 = 16.0;
pub const GUTTER_PAD_R: f32 = 14.0;
pub const TEXT_PAD: f32 = 6.0;
/// The fold chevron's cell: this wide at the right end of the gutter, one row high. The chevron
/// sits in its middle; the numbers keep the rest of the gutter.
pub const FOLD_CELL_W: f32 = 24.0;
/// The chevron's size, logical px.
pub const FOLD_ICON: f32 = 12.0;

/// The fold cell of a row: the right end of a gutter that starts at `x` and is `gutter_w` wide,
/// logical px; `y` and `h` are the row's.
pub fn fold_cell(x: f32, gutter_w: f32, y: f32, h: f32) -> mdshared_ui::gfx::Rect {
    let w = FOLD_CELL_W.min(gutter_w);
    mdshared_ui::gfx::Rect::new(x + gutter_w - w, y, w, h)
}

/// Where the chevron's centre is in a fold cell: in the middle of the gap between where the
/// numbers end (`GUTTER_PAD_R` before the gutter's edge) and where the text starts (`TEXT_PAD`
/// after it), so it is clear of both.
pub fn fold_icon_centre(cell: &mdshared_ui::gfx::Rect) -> (f32, f32) {
    (cell.right() - (GUTTER_PAD_R - TEXT_PAD) / 2.0, cell.cy())
}

/// The width of the tint behind the chevron under the pointer.
pub const FOLD_HOVER_W: f32 = 12.0;
/// A wheel notch scrolls this many lines; a horizontal notch this many advances.
pub const WHEEL_LINES: f64 = 3.0;
pub const WHEEL_COLS: f64 = 3.0;
/// Keyboard moves keep the caret this far from the edges.
pub const KEEP_LINES: f64 = 2.0;
pub const KEEP_COLS: f64 = 4.0;

/// The zoom percentages the text can have, and one step of Zoom in or out.
pub const ZOOM_MIN: u32 = 50;
pub const ZOOM_MAX: u32 = 400;
pub const ZOOM_STEP: u32 = 10;

/// The zoom one step in (`steps` > 0) or out from `zoom`, on the grid of steps and within limits.
pub fn zoom_stepped(zoom: u32, steps: i32) -> u32 {
    let on_grid = (zoom as i32 + ZOOM_STEP as i32 / 2) / ZOOM_STEP as i32 * ZOOM_STEP as i32;
    (on_grid + steps * ZOOM_STEP as i32).clamp(ZOOM_MIN as i32, ZOOM_MAX as i32) as u32
}

/// The text size in logical px at a zoom percentage.
pub fn font_px(zoom: u32) -> f32 {
    FONT_PX * zoom as f32 / 100.0
}

/// The line pitch in device px for a font of `size` device px: an integer.
pub fn line_height(size: f32) -> i32 {
    ((1.35 * size).round() as i32).max(1)
}

/// Digits the gutter makes room for: those of the line count, at least 3.
pub fn digits(lines: usize) -> usize {
    let mut d = 1;
    let mut n = lines;
    while n >= 10 {
        n /= 10;
        d += 1;
    }
    d.max(3)
}

/// The gutter's width for `lines` lines with an advance of `advance` (both in the same px).
/// `pad` is the padding on both sides together in those px.
pub fn gutter_width(lines: usize, advance: f32, pad: f32) -> f32 {
    digits(lines) as f32 * advance + pad
}

/// Lines that fit in `view_h` device px.
pub fn view_lines(view_h: f32, line_h: i32) -> f64 {
    (view_h.max(0.0) as f64) / line_h.max(1) as f64
}

/// The largest `top`: the last line's bottom at the view's bottom edge.
pub fn max_top(lines: usize, view_lines: f64) -> f64 {
    (lines as f64 - view_lines).max(0.0)
}

pub fn clamp_top(top: f64, lines: usize, view_lines: f64) -> f64 {
    if top.is_nan() {
        return 0.0;
    }
    top.clamp(0.0, max_top(lines, view_lines))
}

/// Line `line`'s top relative to the view's top, device px.
pub fn line_y(line: usize, top: f64, line_h: i32) -> f64 {
    (line as f64 - top) * line_h as f64
}

/// The line under `y` device px below the view's top, clamped to the lines there are.
pub fn line_at(y: f64, top: f64, line_h: i32, lines: usize) -> usize {
    let l = (top + y / line_h.max(1) as f64).floor();
    if l <= 0.0 {
        0
    } else {
        (l as usize).min(lines.saturating_sub(1))
    }
}

/// The lines at least partly inside the view.
pub fn visible(top: f64, view_lines: f64, lines: usize) -> Range<usize> {
    let first = (top.max(0.0).floor() as usize).min(lines);
    let last = ((top + view_lines).ceil().max(0.0) as usize).min(lines);
    first..last.max(first)
}

/// The horizontal extent: the longest line plus two advances, raised to the widest line shaped.
pub fn extent(longest_chars: usize, advance: f32, widest: f32) -> f32 {
    ((longest_chars as f32 + 2.0) * advance).max(widest + 2.0 * advance)
}

pub fn max_left(extent: f32, view_w: f32) -> f64 {
    ((extent - view_w) as f64).max(0.0)
}

pub fn clamp_left(left: f64, extent: f32, view_w: f32) -> f64 {
    if left.is_nan() {
        return 0.0;
    }
    left.clamp(0.0, max_left(extent, view_w))
}

/// The estimated x of a position with the counts `c`, from column 0: narrow columns (tab columns
/// included) at `advance` each, wide characters at `wide`, zero-width ones at nothing. Exact
/// when the narrow glyphs are the monospace face's and the wide ones all as wide as the measured
/// one; glyphs from other fallback fonts make it an estimate.
pub fn cols_x(c: Cols, advance: f32, wide: f32) -> f64 {
    let narrow = c.col.saturating_sub(c.wide + c.zero);
    narrow as f64 * advance as f64 + c.wide as f64 * wide as f64
}

/// The chunk that holds `x`: the last one whose x (`cx`) is at or before it.
pub fn chunk_at_x_in(chunks: &[ChunkStart], x: f64, cx: impl Fn(&ChunkStart) -> f64) -> usize {
    chunks.partition_point(|c| cx(c) <= x).saturating_sub(1)
}

/// The chunk that holds buffer offset `offset`.
pub fn chunk_at_offset(chunks: &[ChunkStart], offset: usize) -> usize {
    chunks
        .partition_point(|c| c.offset <= offset)
        .saturating_sub(1)
}

/// The chunks to build for the x range `xs`: those overlapping it, plus one each side.
pub fn chunks_in_view(
    chunks: &[ChunkStart],
    xs: Range<f64>,
    cx: impl Fn(&ChunkStart) -> f64,
) -> Range<usize> {
    if chunks.is_empty() {
        return 0..0;
    }
    let first = chunk_at_x_in(chunks, xs.start, &cx).saturating_sub(1);
    let last = (chunk_at_x_in(chunks, xs.end, &cx) + 2).min(chunks.len());
    first..last
}

/// The `top` that keeps `line` in a view of `view_lines` lines, with a margin of `KEEP_LINES`
/// when `margin`: unchanged when it is already in, centred when `center` and it was off screen.
pub fn reveal_top(
    top: f64,
    line: usize,
    view_lines: f64,
    lines: usize,
    center: bool,
    margin: bool,
) -> f64 {
    let l = line as f64;
    let inside = l >= top && l + 1.0 <= top + view_lines;
    let next = if center && !inside {
        l + 0.5 - view_lines / 2.0
    } else {
        let keep = if margin { KEEP_LINES } else { 0.0 };
        let margin = keep.min(((view_lines - 1.0) / 2.0).max(0.0)).floor();
        if l < top + margin {
            l - margin
        } else if l + 1.0 + margin > top + view_lines {
            l + 1.0 + margin - view_lines
        } else {
            top
        }
    };
    clamp_top(next, lines, view_lines)
}

/// The same for the horizontal scroll: `x` is the caret's x from column 0, all logical px.
pub fn reveal_left(
    left: f64,
    x: f64,
    view_w: f32,
    advance: f32,
    center: bool,
    margin: bool,
) -> f64 {
    let view = view_w as f64;
    let keep = if margin { KEEP_COLS } else { 0.0 };
    let margin = (keep * advance as f64).min(view / 3.0);
    let inside = x >= left && x <= left + view;
    let next = if center && !inside {
        x - view / 2.0
    } else if x < left + margin {
        x - margin
    } else if x > left + view - margin {
        x - view + margin
    } else {
        left
    };
    next.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_stay_on_the_grid_and_in_range() {
        assert_eq!(zoom_stepped(100, 1), 110);
        assert_eq!(zoom_stepped(100, -1), 90);
        assert_eq!(zoom_stepped(100, 3), 130);
        assert_eq!(zoom_stepped(104, 1), 110);
        assert_eq!(zoom_stepped(ZOOM_MAX, 1), ZOOM_MAX);
        assert_eq!(zoom_stepped(ZOOM_MIN, -5), ZOOM_MIN);
        assert_eq!(font_px(150), 21.0);
    }

    #[test]
    fn the_fold_cell_is_the_gutters_right_end_of_one_row() {
        let cell = fold_cell(40.0, 60.0, 100.0, 19.0);
        // Inside the gutter, ending at its edge, one row high.
        assert_eq!((cell.x, cell.right()), (76.0, 100.0));
        assert_eq!((cell.y, cell.h), (100.0, 19.0));
        // The text starts at the gutter's edge: a point there is not in the cell.
        assert!(cell.contains(99.0, 110.0) && !cell.contains(100.0, 110.0));
        // The number area, left of the cell, keeps its clicks; so do the rows around.
        assert!(!cell.contains(60.0, 110.0));
        assert!(!cell.contains(90.0, 99.0) && !cell.contains(90.0, 119.0));
        // The chevron is in the middle of the gap between the numbers' end and the text's start,
        // with its hover tint at least 3 px clear of both.
        let (cx, cy) = fold_icon_centre(&cell);
        assert!(cell.contains(cx, cy));
        let (numbers_end, text_start) = (100.0 - GUTTER_PAD_R, 100.0 + TEXT_PAD);
        assert_eq!(cx - numbers_end, text_start - cx);
        assert!(cx - FOLD_HOVER_W / 2.0 - numbers_end >= 3.0);
        assert!(text_start - (cx + FOLD_HOVER_W / 2.0) >= 3.0);
        // A narrow gutter gives a narrow cell, never one that leaves it.
        let narrow = fold_cell(0.0, 20.0, 0.0, 10.0);
        assert_eq!((narrow.x, narrow.right()), (0.0, 20.0));
    }

    const LINES: usize = 1_852_171;

    #[test]
    fn line_metrics() {
        assert_eq!(font_px(100), 14.0);
        assert_eq!(line_height(14.0), 19);
        assert_eq!(line_height(21.0), 28);
        assert_eq!(line_y(10, 10.0, 19), 0.0);
        assert_eq!(line_y(12, 10.5, 19), 28.5);
        assert_eq!(line_at(0.0, 10.0, 19, 100), 10);
        assert_eq!(line_at(18.9, 10.0, 19, 100), 10);
        assert_eq!(line_at(19.0, 10.0, 19, 100), 11);
        assert_eq!(line_at(-50.0, 0.0, 19, 100), 0);
        assert_eq!(line_at(1e9, 0.0, 19, 100), 99);
        assert_eq!(visible(0.0, 10.5, 100), 0..11);
        assert_eq!(visible(95.0, 10.0, 100), 95..100);
        assert_eq!(visible(0.0, 10.0, 0), 0..0);
    }

    #[test]
    fn scroll_clamps() {
        assert_eq!(max_top(10, 30.0), 0.0);
        assert_eq!(max_top(100, 30.0), 70.0);
        assert_eq!(clamp_top(-5.0, 100, 30.0), 0.0);
        assert_eq!(clamp_top(500.0, 100, 30.0), 70.0);
        assert_eq!(clamp_top(f64::NAN, 100, 30.0), 0.0);
        // The last line's bottom reaches the view's bottom and no further.
        let vl = view_lines(700.0, 19);
        let top = max_top(100, vl);
        assert!((line_y(100, top, 19) - 700.0).abs() < 1e-9);
        assert_eq!(clamp_left(-1.0, 1000.0, 400.0), 0.0);
        assert_eq!(clamp_left(900.0, 1000.0, 400.0), 600.0);
        assert_eq!(clamp_left(50.0, 300.0, 400.0), 0.0);
    }

    #[test]
    fn huge_file_scrolls_exactly() {
        let vl = view_lines(706.0, 19);
        let end = max_top(LINES, vl);
        let mut top = 0.0;
        // To the end in wheel steps, then back.
        let steps = (end / WHEEL_LINES).ceil() as u64;
        for _ in 0..steps {
            top = clamp_top(top + WHEEL_LINES, LINES, vl);
        }
        assert_eq!(top, end);
        assert_eq!(visible(top, vl, LINES).end, LINES);
        for _ in 0..steps {
            top = clamp_top(top - WHEEL_LINES, LINES, vl);
        }
        assert_eq!(top, 0.0);
        // Line positions near the end are exact to the device pixel.
        let last = LINES - 1;
        let y = line_y(last, end, 19);
        assert!((y - (706.0 - 19.0)).abs() < 1e-6, "{y}");
        assert_eq!(line_at(y + 1.0, end, 19, LINES), last);
    }

    #[test]
    fn gutter() {
        assert_eq!(digits(9), 3);
        assert_eq!(digits(999), 3);
        assert_eq!(digits(1000), 4);
        assert_eq!(digits(LINES), 7);
        let adv = 7.7;
        assert_eq!(gutter_width(9, adv, 30.0), 3.0 * adv + 30.0);
        assert_eq!(gutter_width(999, adv, 30.0), 3.0 * adv + 30.0);
        assert_eq!(gutter_width(1000, adv, 30.0), 4.0 * adv + 30.0);
        assert_eq!(gutter_width(LINES, adv, 30.0), 7.0 * adv + 30.0);
    }

    #[test]
    fn horizontal_extent_and_chunks() {
        assert_eq!(extent(98, 8.0, 0.0), 800.0);
        assert_eq!(extent(98, 8.0, 1000.0), 1016.0);
        let at = |col| Cols {
            col,
            ..Cols::default()
        };
        assert_eq!(cols_x(at(1024), 7.5, 14.0), 7680.0);
        // A 1 MB line at 8 px stays exact in f64.
        assert_eq!(cols_x(at(1_048_576), 8.0, 14.0), 8_388_608.0);
        // Wide characters at their own width, zero-width ones at none.
        let c = Cols {
            col: 10,
            wide: 4,
            zero: 2,
        };
        assert_eq!(cols_x(c, 8.0, 14.0), 4.0 * 8.0 + 4.0 * 14.0);
        let chunks: Vec<ChunkStart> = (0..10)
            .map(|i| ChunkStart {
                offset: i * 1000,
                col: i * 1000,
                ..ChunkStart::default()
            })
            .collect();
        let cx = |c: &ChunkStart| cols_x(c.cols(), 1.0, 2.0);
        assert_eq!(chunk_at_x_in(&chunks, 0.0, cx), 0);
        assert_eq!(chunk_at_x_in(&chunks, 999.0, cx), 0);
        assert_eq!(chunk_at_x_in(&chunks, 1000.0, cx), 1);
        assert_eq!(chunk_at_x_in(&chunks, 1e6, cx), 9);
        assert_eq!(chunk_at_offset(&chunks, 2500), 2);
        assert_eq!(chunks_in_view(&chunks, 2500.0..2600.0, cx), 1..4);
        assert_eq!(chunks_in_view(&chunks, 0.0..100.0, cx), 0..2);
        assert_eq!(chunks_in_view(&chunks, 9500.0..9600.0, cx), 8..10);
        assert_eq!(chunks_in_view(&[], 0.0..10.0, cx), 0..0);
        // Chunks of wide text sit by their wide counts, so they do not overlap.
        let cjk: Vec<ChunkStart> = (0..4)
            .map(|i| ChunkStart {
                offset: i * 1023,
                col: i * 341,
                wide: i * 341,
                zero: 0,
            })
            .collect();
        let cx = |c: &ChunkStart| cols_x(c.cols(), 7.7, 14.0);
        assert_eq!(cx(&cjk[1]), 341.0 * 14.0);
        assert_eq!(chunk_at_x_in(&cjk, 341.0 * 14.0 + 1.0, cx), 1);
    }

    #[test]
    fn reveal() {
        // Already visible: unchanged.
        assert_eq!(reveal_top(10.0, 15, 20.0, 100, false, true), 10.0);
        // Below: just enough to show it with the margin.
        assert_eq!(reveal_top(10.0, 40, 20.0, 100, false, true), 23.0);
        // Above.
        assert_eq!(reveal_top(10.0, 5, 20.0, 100, false, true), 3.0);
        // Centred when it was off screen.
        assert_eq!(reveal_top(0.0, 50, 20.0, 100, true, true), 40.5);
        assert_eq!(reveal_top(0.0, 99, 20.0, 100, true, true), 80.0);
        assert_eq!(reveal_left(0.0, 100.0, 400.0, 8.0, false, true), 0.0);
        assert_eq!(reveal_left(0.0, 500.0, 400.0, 8.0, false, true), 132.0);
        assert_eq!(reveal_left(200.0, 10.0, 400.0, 8.0, false, true), 0.0);
        assert_eq!(reveal_left(0.0, 5000.0, 400.0, 8.0, true, true), 4800.0);
    }

    #[test]
    fn reveal_without_margin_leaves_visible_lines_alone() {
        // A drag over the last two visible lines (the margin's band) does not scroll.
        for line in 100..=136 {
            assert_eq!(reveal_top(100.0, line, 37.0, 1000, false, false), 100.0);
        }
        // With the margin the same lines scroll.
        assert_ne!(reveal_top(100.0, 136, 37.0, 1000, false, true), 100.0);
        // Below the view it scrolls just enough.
        assert_eq!(reveal_top(100.0, 140, 37.0, 1000, false, false), 104.0);
        // Horizontally: inside the view nothing moves, near the edges included.
        assert_eq!(reveal_left(100.0, 101.0, 400.0, 8.0, false, false), 100.0);
        assert_eq!(reveal_left(100.0, 499.0, 400.0, 8.0, false, false), 100.0);
        assert_eq!(reveal_left(100.0, 520.0, 400.0, 8.0, false, false), 120.0);
    }
}
